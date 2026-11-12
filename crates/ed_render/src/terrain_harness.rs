//! Terrain GPU harness (Phase 2 W3): real heights from `.earthdrop-cache/` around Zermatt, a
//! scripted ground-to-orbit camera path (1 m AGL to 1000 km), the provider's own passes driven
//! through a frame graph (no `Renderer` wiring yet), hole and crack counts read back from the
//! visibility buffer, CPU-shaded captures and the terrain pass GPU time at 1440p.
//!
//! Run (needs a GPU and the cache; offline, never downloads):
//! `EARTHDROP_CACHE=.earthdrop-cache ED_TERRAIN_SHOTS=dir cargo test -p ed_render --release terrain::harness -- --ignored --nocapture`
//!
//! Heights: a pure function of the point, so every level agrees on shared lattice points (the
//! property `ed_data::node::parent_heights` gives the real pages): bilinear in the finest cached
//! Mapterhorn zoom (14 down to 8) whose 2 x 2 footprint is on disk, else 0 m (logged). Lattice
//! points map to lat/lon exactly as `ed_data::node::node_heights` does. Only the samples the
//! 64-cell mesh reads (every 4th) are computed; the rest of each page is interpolated.

use super::*;
use crate::graph::{FrameTimings, TextureInfo, TransientPool};
use crate::provider::{ShadowTargets, SharedInputs, VisTargets};
use crate::scene::InstancePools;
use ed_data::DiskCache;
use ed_data::elevation::HeightTile;
use ed_geo::tile::TileId;
use ed_geo::{BodyId, WGS84_A, WGS84_B, WGS84_E2};
use ed_gpu::{BufferDesc, BufferId, DeviceDesc, Format, Subresource};
use ed_planet::{ChunkMask, FacePos, Planet, View};
use glam::{DQuat, Mat4, Vec4};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

const W: u32 = 1280;
const H: u32 = 720;
const FOV: f32 = 60.0 * std::f32::consts::PI / 180.0;
const ZERMATT: (f64, f64) = (46.02, 7.75);
const MATTERHORN: (f64, f64) = (45.9763, 7.6586);
const PATH_POINTS: usize = 24;
const SHADOW_TILE: u32 = 1024;

// ---------------------------------------------------------------------------------------------
// Heights from the cache
// ---------------------------------------------------------------------------------------------

type TileKey = (u8, u32, u32);

struct Source {
    root: PathBuf,
    cache: DiskCache,
    tiles: Mutex<BTreeMap<TileKey, Option<Arc<HeightTile>>>>,
    /// Nodes with no cached data at all (drawn flat at 0 m).
    flat_nodes: AtomicU32,
}

impl Source {
    fn new(root: PathBuf) -> Self {
        ed_data::http::set_offline(true);
        Self { cache: DiskCache::new(root.clone()), root, tiles: Mutex::new(BTreeMap::new()), flat_nodes: AtomicU32::new(0) }
    }

    fn tile(&self, z: u8, x: u32, y: u32) -> Option<Arc<HeightTile>> {
        if let Some(t) = self.tiles.lock().unwrap().get(&(z, x, y)) {
            return t.clone();
        }
        let path = self.root.join(format!("mapterhorn/{}/{z}/{x}/{y}.webp", ed_data::DATA_VERSION));
        let t = if path.exists() { ed_data::elevation::fetch(&self.cache, TileId::new(z, x, y)).ok().map(Arc::new) } else { None };
        self.tiles.lock().unwrap().insert((z, x, y), t.clone());
        t
    }

    /// Metres at lon/lat (degrees): bilinear (pixel centres at +0.5) in the finest cached zoom.
    fn height(&self, lon: f64, lat: f64) -> Option<f64> {
        let lat = lat.clamp(-ed_geo::tile::MAX_LAT, ed_geo::tile::MAX_LAT).to_radians();
        let gx = (lon + 180.0) / 360.0;
        let gy = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0;
        'zoom: for z in (8..=14u8).rev() {
            let n = 1i64 << z;
            let t0 = self.tile(z, (gx * n as f64).floor().clamp(0.0, (n - 1) as f64) as u32, (gy * n as f64).floor().clamp(0.0, (n - 1) as f64) as u32);
            let Some(t0) = t0 else { continue };
            let size = t0.size as i64;
            let (fx, fy) = (gx * (n * size) as f64 - 0.5, gy * (n * size) as f64 - 0.5);
            let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
            let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
            let mut px = [0.0f64; 4];
            for (k, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                let x = (x0 + dx).rem_euclid(n * size);
                let y = (y0 + dy).clamp(0, n * size - 1);
                let Some(t) = self.tile(z, (x / size) as u32, (y / size) as u32) else { continue 'zoom };
                if t.size as i64 != size {
                    continue 'zoom;
                }
                px[k] = t.heights[((y % size) * size + x % size) as usize] as f64;
            }
            let a = px[0] * (1.0 - tx) + px[1] * tx;
            let b = px[2] * (1.0 - tx) + px[3] * tx;
            return Some(a * (1.0 - ty) + b * ty);
        }
        None
    }

    /// Height at a body direction (ed_data's lattice-to-geodetic rule).
    fn height_dir(&self, d: DVec3) -> Option<f64> {
        let (lat, lon) = latlon(d);
        self.height(lon, lat)
    }

    /// The node's page: exact at every 4th sample, bilinear between.
    fn page(&self, id: NodeId) -> HeightPage {
        let step = ed_geo::HEIGHT_STEP_M as f64;
        let mut q4 = vec![0i64; (SIDE * SIDE) as usize];
        let mut any = false;
        for j in 0..SIDE {
            for i in 0..SIDE {
                if let Some(h) = self.height_dir(id.grid_point(4 * i, 4 * j, 256)) {
                    any = true;
                    q4[(j * SIDE + i) as usize] = (h / step).round() as i64;
                }
            }
        }
        if !any {
            self.flat_nodes.fetch_add(1, Ordering::Relaxed);
        }
        let mut q = Vec::with_capacity(PAGE_SAMPLES);
        for j in 0..PAGE_N {
            for i in 0..PAGE_N {
                let (ci, cj) = ((i / 4).min(63), (j / 4).min(63));
                let (fx, fy) = ((i - ci * 4) as f64 / 4.0, (j - cj * 4) as f64 / 4.0);
                let at = |a: usize, b: usize| q4[b * SIDE as usize + a] as f64;
                let a = at(ci, cj) * (1.0 - fx) + at(ci + 1, cj) * fx;
                let b = at(ci, cj + 1) * (1.0 - fx) + at(ci + 1, cj + 1) * fx;
                q.push((a * (1.0 - fy) + b * fy).round() as i64);
            }
        }
        let lo = *q.iter().min().unwrap();
        let hi = *q.iter().max().unwrap();
        let offset = (lo as f64 * step) as f32;
        HeightPage { samples: q.iter().map(|&v| (v - lo).min(u16::MAX as i64) as u16).collect(), scale: ed_geo::HEIGHT_STEP_M, offset, min_m: offset, max_m: (hi as f64 * step) as f32 }
    }
}

/// ed_data's `lattice_geodetic`: the ellipsoid point straight along `d` (`d * r(d)`, where
/// ed_planet places the lattice sample), Bowring. (lat, lon) deg.
fn latlon(d: DVec3) -> (f64, f64) {
    let r = 1.0 / ((d.x * d.x + d.y * d.y) / (WGS84_A * WGS84_A) + d.z * d.z / (WGS84_B * WGS84_B)).sqrt();
    let (x, y, z) = (d.x * r, d.y * r, d.z * r);
    let lon = y.atan2(x);
    let r = (x * x + y * y).sqrt();
    if r < 1e-6 {
        return (if z >= 0.0 { 90.0 } else { -90.0 }, 0.0);
    }
    let ep2 = (WGS84_A * WGS84_A - WGS84_B * WGS84_B) / (WGS84_B * WGS84_B);
    let theta = (z * WGS84_A).atan2(r * WGS84_B);
    let (st, ct) = theta.sin_cos();
    let lat = (z + ep2 * WGS84_B * st * st * st).atan2(r - WGS84_E2 * WGS84_A * ct * ct * ct);
    (lat.to_degrees(), lon.to_degrees())
}

/// Inverse of [`latlon`] at the surface: the body direction whose data is at (lat, lon).
fn dir_of(lat: f64, lon: f64) -> DVec3 {
    let e = ed_geo::Geodetic::new(lat, lon, 0.0).to_ecef();
    DVec3::new(e.x, e.y, e.z).normalize()
}

struct Resident {
    pages: BTreeMap<NodeId, Arc<HeightPage>>,
}

impl Residency for Resident {
    fn bytes(&self, _id: NodeId) -> u64 {
        (PAGE_SAMPLES * 2) as u64
    }
    fn chunks(&self, id: NodeId) -> ChunkMask {
        let mut m = ChunkMask::terrain();
        if self.pages.contains_key(&id) {
            m.resident = m.required;
            m.uploaded = m.required;
        }
        m
    }
    fn height_page(&self, id: NodeId) -> Option<Arc<HeightPage>> {
        self.pages.get(&id).cloned()
    }
    fn page_slot(&self, _id: NodeId) -> Option<u32> {
        None
    }
}

/// Builds the missing pages in parallel (results in input order).
fn load(src: &Source, res: &mut Resident, want: &[NodeId]) -> usize {
    let missing: Vec<NodeId> = want.iter().copied().filter(|id| !res.pages.contains_key(id)).collect();
    if missing.is_empty() {
        return 0;
    }
    let threads = std::thread::available_parallelism().map_or(8, |n| n.get()).min(16);
    let chunk = missing.len().div_ceil(threads);
    let built: Vec<Vec<HeightPage>> = std::thread::scope(|s| {
        let hs: Vec<_> = missing.chunks(chunk).map(|c| s.spawn(move || c.iter().map(|&id| src.page(id)).collect::<Vec<_>>())).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for (id, p) in missing.iter().zip(built.into_iter().flatten()) {
        res.pages.insert(*id, Arc::new(p));
    }
    missing.len()
}

// ---------------------------------------------------------------------------------------------
// Scene: world frame, camera path
// ---------------------------------------------------------------------------------------------

struct World {
    body: Body,
    pose: BodyPose,
    planet: Planet,
    res: Resident,
    src: Source,
}

impl World {
    fn new(root: PathBuf) -> Self {
        let body = Body::earth();
        // World = ENU at Zermatt (x east, y up, z south), origin on the ellipsoid.
        let d0 = dir_of(ZERMATT.0, ZERMATT.1);
        let f = body.frame(d0);
        let q = DQuat::from_mat3(&DMat3::from_cols(f.x_axis, f.z_axis, -f.y_axis).transpose());
        let pose = BodyPose { id: body.id, rotation: q, position: -(q * body.point(d0, 0.0)) };
        Self { planet: Planet::new(vec![body.clone()]), body, pose, res: Resident { pages: BTreeMap::new() }, src: Source::new(root) }
    }

    fn world(&self, body_p: DVec3) -> DVec3 {
        self.pose.rotation * body_p + self.pose.position
    }

    /// Selects and loads until the selection is complete (or 12 rounds).
    fn settle(&mut self, view: &View) -> Selection {
        let cfg = SelectConfig::default();
        let poses = [self.pose];
        let mut sel = self.planet.select(view, &poses, &self.res, &cfg);
        for _ in 0..12 {
            let n = load(&self.src, &mut self.res, &sel.wanted);
            sel = self.planet.select(view, &poses, &self.res, &cfg);
            if n == 0 && sel.holes.is_empty() {
                break;
            }
        }
        sel
    }

    /// Height of the drawn (unmorphed) mesh under body direction `d`: the finest drawn node.
    fn mesh_height(&self, sel: &Selection, d: DVec3) -> f64 {
        let fp = FacePos::from_dir(self.body.id, d);
        let drawn: BTreeSet<NodeId> = sel.draw.iter().map(|x| x.id).collect();
        for level in (0..=NodeId::MAX_LEVEL).rev() {
            let (id, u, v) = fp.node(level);
            if !drawn.contains(&id) {
                continue;
            }
            let Some(p) = self.res.height_page(id) else { return 0.0 };
            let (fx, fy) = ((u * 64.0).min(64.0 - 1e-9), (v * 64.0).min(64.0 - 1e-9));
            let (i, j) = (fx.floor() as usize, fy.floor() as usize);
            let (tx, ty) = (fx - i as f64, fy - j as f64);
            let h = |a: usize, b: usize| p.height(4 * a, 4 * b);
            let (h00, h10, h01, h11) = (h(i, j), h(i + 1, j), h(i, j + 1), h(i + 1, j + 1));
            return if tx >= ty { h00 + (h10 - h00) * tx + (h11 - h10) * ty } else { h00 + (h11 - h01) * tx + (h01 - h00) * ty };
        }
        0.0
    }
}

#[derive(Clone, Copy, Debug)]
struct Shot {
    eye: DVec3,
    rot: DQuat,
}

fn ed_view(s: &Shot, w: u32, h: u32) -> View {
    View { position: s.eye, rotation: s.rot, velocity: DVec3::ZERO, fov_y: FOV as f64, viewport: [w, h], look_ahead_s: 0.0 }
}

/// Camera `agl` metres above the drawn mesh over Zermatt, looking toward the Matterhorn.
fn shot(world: &mut World, agl: f64, t: f64, w: u32, h: u32) -> (Shot, Selection) {
    let d0 = dir_of(ZERMATT.0, ZERMATT.1);
    let target = world.world(world.body.point(dir_of(MATTERHORN.0, MATTERHORN.1), 4000.0));
    let pitch = (8.0 - 68.0 * t).to_radians();
    let (body, pose) = (world.body.clone(), world.pose);
    let make = |ground: f64| {
        let eye = pose.rotation * body.point(d0, ground + agl) + pose.position;
        let to = target - eye;
        let yaw = (-to.x).atan2(-to.z);
        Shot { eye, rot: DQuat::from_rotation_y(yaw) * DQuat::from_rotation_x(pitch) }
    };
    // Ground guess from the data, then from the drawn mesh (twice: the selection moves).
    let mut s = make(world.src.height_dir(d0).unwrap_or(0.0));
    let mut sel = world.settle(&ed_view(&s, w, h));
    for _ in 0..2 {
        s = make(world.mesh_height(&sel, d0));
        sel = world.settle(&ed_view(&s, w, h));
    }
    (s, sel)
}

// ---------------------------------------------------------------------------------------------
// GPU frames
// ---------------------------------------------------------------------------------------------

struct Gpu {
    dev: Box<dyn Device>,
    frame: Tracked,
    pools: InstancePools,
    transients: TransientPool,
    readback: BufferId,
    atlas_rb: BufferId,
    scratch: Vec<ed_gpu::Timing>,
}

impl Gpu {
    fn new() -> Option<Self> {
        let dev = ed_gpu::device_or_skip(ed_gpu::default_backend(), &DeviceDesc { validation: true, frames_in_flight: 2 })?;
        let frame = Tracked::new(&*dev, "terrain harness frame", (gpu::FRAME_WORDS * 4) as u64, Usage { shader_read: true, cpu_write: true, ..Default::default() }).unwrap();
        let rb = |label, size| dev.create_buffer(&BufferDesc { label, size, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } }).unwrap();
        let readback = rb("terrain vis readback", (VIS_FORMAT.copy_row_pitch(2560) * 1440) as u64);
        let atlas_rb = rb("terrain atlas readback", (Format::Depth32Float.copy_row_pitch(2 * SHADOW_TILE) * 2 * SHADOW_TILE) as u64);
        Some(Self { dev, frame, pools: InstancePools::new(0, 0), transients: TransientPool::new(), readback, atlas_rb, scratch: Vec::new() })
    }

    /// One frame of the terrain passes at w x h; reads the visibility buffer (and the atlas).
    fn frame(&mut self, tp: &mut TerrainProvider, s: &Shot, sun: Vec3, w: u32, h: u32, read_atlas: bool) {
        tp.flush(&*self.dev, &self.pools).unwrap();
        let dev = &*self.dev;
        let mut enc = dev.begin_frame().unwrap();
        let cam_rot = s.rot.as_quat();
        let proj = crate::reverse_z_projection(FOV, w as f32 / h as f32, 0.1);
        let view_m = Mat4::from_quat(cam_rot.inverse());
        let vpm = proj * view_m;
        let mut fd = vec![0u32; gpu::FRAME_WORDS];
        let put = |fd: &mut [u32], at: usize, v: &[f32]| {
            for (i, x) in v.iter().enumerate() {
                fd[at + i] = x.to_bits();
            }
        };
        put(&mut fd, 0, &vpm.to_cols_array());
        put(&mut fd, 16, &vpm.inverse().to_cols_array());
        put(&mut fd, 32, &view_m.to_cols_array());
        // Four nested ortho cascades around the camera (camera-relative), sun-facing z.
        for c in 0..4 {
            put(&mut fd, gpu::FD_LIGHT + c * 16, &cascade(sun, 500.0 * 4f32.powi(c as i32)).to_cols_array());
        }
        tp.prepare(dev, &self.pools, w, h, &mut fd).unwrap();
        let fbytes = gpu::bytes(&fd);
        dev.fill_frame_buffer(self.frame.id, &mut |dst| dst[..fbytes.len()].copy_from_slice(&fbytes)).unwrap();

        let mut g = FrameGraph::new();
        let fr = g.import_buffer("frame", self.frame.id, self.frame.state, None);
        let vis = g.create_texture("vis", TextureInfo { format: VIS_FORMAT, width: w, height: h });
        let depth = g.create_texture("depth", TextureInfo { format: DEPTH_FORMAT, width: w, height: h });
        let atlas = g.create_texture("atlas", TextureInfo { format: DEPTH_FORMAT, width: 2 * SHADOW_TILE, height: 2 * SHADOW_TILE });
        let inp = SharedInputs { frame: (fr, self.frame.bindless.0), instances: (fr, 0), nodes: (fr, 0), verts: (fr, 0), indices: (fr, 0, self.frame.id), slots: 0 };
        let out = VisTargets { depth, vis, shadow: Some(ShadowTargets { atlas, cascades: 4, tile: SHADOW_TILE }), final_hzb: false, preloaded: false };
        tp.record(&mut g, 0, &inp, out);
        let rb = g.import_buffer("vis readback", self.readback, ResourceState::Undefined, None);
        let pitch = VIS_FORMAT.copy_row_pitch(w);
        g.add_pass("vis readback", PassKind::Manual).read(vis, ResourceState::CopySrc).write(rb, ResourceState::CopyDst).exec(move |ctx| {
            let (t, b) = (ctx.texture(vis), ctx.buffer(rb));
            ctx.enc.copy_texture_to_buffer(t, Subresource { mip: 0, layer: 0 }, b, 0, pitch);
        });
        // The atlas is read back (or at least kept alive) so the shadow pass is never culled.
        let arb = g.import_buffer("atlas readback", self.atlas_rb, ResourceState::Undefined, None);
        let apitch = Format::Depth32Float.copy_row_pitch(2 * SHADOW_TILE);
        g.add_pass("atlas readback", PassKind::Manual).read(atlas, ResourceState::CopySrc).write(arb, ResourceState::CopyDst).exec(move |ctx| {
            if read_atlas {
                let (t, b) = (ctx.texture(atlas), ctx.buffer(arb));
                ctx.enc.copy_texture_to_buffer(t, Subresource { mip: 0, layer: 0 }, b, 0, apitch);
            }
        });
        let plan = g.execute(dev, &mut *enc, &mut self.transients).unwrap();
        tp.finish(&plan);
        self.frame.state = plan.final_states[fr.index()];
        dev.end_frame(enc).expect("end_frame (validation)");
    }

    fn read_vis(&mut self, w: u32, h: u32) -> Vec<[u32; 2]> {
        self.dev.wait_idle();
        let mut raw = Vec::new();
        self.dev.read_buffer(self.readback, &mut raw).unwrap();
        let pitch = VIS_FORMAT.copy_row_pitch(w) as usize;
        let mut out = Vec::with_capacity((w * h) as usize);
        for y in 0..h as usize {
            for x in 0..w as usize {
                let o = y * pitch + x * 8;
                out.push([u32::from_ne_bytes(raw[o..o + 4].try_into().unwrap()), u32::from_ne_bytes(raw[o + 4..o + 8].try_into().unwrap())]);
            }
        }
        out
    }

    /// Atlas texels written (depth > 0) per cascade.
    fn read_atlas(&mut self) -> [u32; 4] {
        self.dev.wait_idle();
        let mut raw = Vec::new();
        self.dev.read_buffer(self.atlas_rb, &mut raw).unwrap();
        let pitch = Format::Depth32Float.copy_row_pitch(2 * SHADOW_TILE) as usize;
        let mut n = [0u32; 4];
        for y in 0..(2 * SHADOW_TILE) as usize {
            for x in 0..(2 * SHADOW_TILE) as usize {
                let o = y * pitch + x * 4;
                if f32::from_ne_bytes(raw[o..o + 4].try_into().unwrap()) > 0.0 {
                    n[(y / SHADOW_TILE as usize) * 2 + x / SHADOW_TILE as usize] += 1;
                }
            }
        }
        n
    }

    fn destroy(mut self) {
        self.dev.wait_idle();
        self.transients.destroy(&*self.dev);
        self.frame.destroy(&*self.dev);
        self.dev.destroy_buffer(self.readback);
        self.dev.destroy_buffer(self.atlas_rb);
        self.dev.wait_idle();
    }
}

/// Camera-relative world -> cascade clip: x, y in -1..1 over `half` metres, z 0..1 (1 = sun side).
fn cascade(sun: Vec3, half: f32) -> Mat4 {
    let s = sun.normalize();
    let r = s.cross(Vec3::Y).try_normalize().unwrap_or(Vec3::X);
    let u = r.cross(s);
    let range = 20_000.0f32;
    Mat4::from_cols(
        Vec4::new(r.x / half, u.x / half, s.x / (2.0 * range), 0.0),
        Vec4::new(r.y / half, u.y / half, s.y / (2.0 * range), 0.0),
        Vec4::new(r.z / half, u.z / half, s.z / (2.0 * range), 0.0),
        Vec4::new(0.0, 0.0, 0.5, 1.0),
    )
}

// ---------------------------------------------------------------------------------------------
// Analysis
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default)]
struct Counts {
    terrain: u32,
    holes: u32,
    skirt: u32,
    /// Skirt pixels thicker than 1 px (min of the horizontal and vertical run).
    skirt_thick: u32,
    skirt_max: u32,
    /// With skirts off: the thickest hole run (the raw crack width).
    hole_max: u32,
}

/// World ray direction of pixel (x, y).
fn ray(s: &Shot, x: u32, y: u32, w: u32, h: u32) -> DVec3 {
    let t = (FOV as f64 * 0.5).tan();
    let a = w as f64 / h as f64;
    let c = DVec3::new((2.0 * (x as f64 + 0.5) / w as f64 - 1.0) * t * a, (1.0 - 2.0 * (y as f64 + 0.5) / h as f64) * t, -1.0);
    (s.rot * c).normalize()
}

/// Does the ray hit the ellipsoid shrunk by `margin` metres?
fn hits_inner(world: &World, s: &Shot, d: DVec3, margin: f64) -> bool {
    let inv = world.pose.rotation.inverse();
    let (o, d) = (world.pose.to_body(s.eye), inv * d);
    let (a, b) = (world.body.equatorial_m - margin, world.body.polar_m - margin);
    let (o, d) = (DVec3::new(o.x / a, o.y / a, o.z / b), DVec3::new(d.x / a, d.y / a, d.z / b));
    let (qa, qb, qc) = (d.dot(d), 2.0 * o.dot(d), o.dot(o) - 1.0);
    let disc = qb * qb - 4.0 * qa * qc;
    disc >= 0.0 && (-qb - disc.sqrt()) / (2.0 * qa) > 0.0
}

/// Longest run of `mask` through each set pixel, horizontally and vertically: max of min(h, v).
fn runs(mask: &[bool], w: usize, h: usize) -> (u32, u32) {
    let mut hr = vec![0u32; mask.len()];
    let mut vr = vec![0u32; mask.len()];
    for y in 0..h {
        let mut x = 0;
        while x < w {
            if !mask[y * w + x] {
                x += 1;
                continue;
            }
            let s = x;
            while x < w && mask[y * w + x] {
                x += 1;
            }
            for k in s..x {
                hr[y * w + k] = (x - s) as u32;
            }
        }
    }
    for x in 0..w {
        let mut y = 0;
        while y < h {
            if !mask[y * w + x] {
                y += 1;
                continue;
            }
            let s = y;
            while y < h && mask[y * w + x] {
                y += 1;
            }
            for k in s..y {
                vr[k * w + x] = (y - s) as u32;
            }
        }
    }
    let (mut max, mut thick) = (0u32, 0u32);
    for i in 0..mask.len() {
        if mask[i] {
            let t = hr[i].min(vr[i]);
            max = max.max(t);
            if t > 1 {
                thick += 1;
            }
        }
    }
    (max, thick)
}

/// Shrink of the inner ellipsoid: lowest drawn height and the coarsest level's chord sag.
fn margin(world: &World, sel: &Selection) -> f64 {
    let mut lo = 0.0f64;
    let mut coarsest = NodeId::MAX_LEVEL;
    for d in &sel.draw {
        if let Some(p) = world.res.height_page(d.id) {
            lo = lo.min(p.min_m as f64);
        }
        coarsest = coarsest.min(d.id.level());
    }
    // Lattice cells are up to ~1.4x the nominal angle (tan warp); the diagonal is sqrt 2 longer.
    let cell = std::f64::consts::FRAC_PI_2 / (1u64 << coarsest) as f64 / 64.0 * 1.4 * std::f64::consts::SQRT_2;
    let sag = world.body.equatorial_m * cell * cell / 8.0;
    sag - lo + 2.0
}

fn analyse(world: &World, s: &Shot, sel: &Selection, vis: &[[u32; 2]], w: u32, h: u32) -> (Counts, Vec<bool>) {
    let m = margin(world, sel);
    let mut c = Counts::default();
    let mut skirt = vec![false; vis.len()];
    let mut hole = vec![false; vis.len()];
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let v = vis[i];
            if v[0] & TERRAIN_VIS_BIT != 0 {
                c.terrain += 1;
                if v[1] >= GRID_TRIS {
                    skirt[i] = true;
                    c.skirt += 1;
                }
            } else if hits_inner(world, s, ray(s, x, y, w, h), m) {
                hole[i] = true;
                c.holes += 1;
            }
        }
    }
    (c.skirt_max, c.skirt_thick) = runs(&skirt, w as usize, h as usize);
    c.hole_max = runs(&hole, w as usize, h as usize).0;
    (c, hole)
}

/// CPU shading of the visibility buffer: lambert of the triangle normal, colour by height,
/// skirt pixels red, holes magenta, sky dark blue.
fn shade(world: &World, tp: &TerrainProvider, vis: &[[u32; 2]], hole: &[bool], sun: Vec3, w: u32, h: u32) -> Vec<u8> {
    let draws = tp.draws();
    let rows: Vec<usize> = (0..h as usize).collect();
    let threads = std::thread::available_parallelism().map_or(8, |n| n.get()).min(16);
    let band = rows.len().div_ceil(threads);
    let out: Vec<Vec<u8>> = std::thread::scope(|sc| {
        let hs: Vec<_> = rows
            .chunks(band)
            .map(|rs| {
                sc.spawn(move || {
                    let mut cache: std::collections::HashMap<(u32, u32), [u8; 3]> = std::collections::HashMap::new();
                    let mut px = Vec::with_capacity(rs.len() * w as usize * 3);
                    for &y in rs {
                        for x in 0..w as usize {
                            let i = y * w as usize + x;
                            let v = vis[i];
                            let rgb = if v[0] & TERRAIN_VIS_BIT != 0 {
                                let (di, prim) = (v[0] & !TERRAIN_VIS_BIT, v[1]);
                                if prim >= GRID_TRIS {
                                    [230, 30, 30]
                                } else {
                                    *cache.entry((di, prim)).or_insert_with(|| {
                                        let d = &draws[di as usize];
                                        let Some(pg) = tp.draw_pages(d) else { return [0, 255, 255] };
                                        let t = triangle(prim);
                                        let p: Vec<Vec3> = t.iter().map(|&k| cpu_vertex(d, &pg, k)).collect();
                                        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero();
                                        let c = (p[0] + p[1] + p[2]) / 3.0;
                                        // Height of the triangle centre above the ellipsoid.
                                        let wc = world.pose.to_body(c.as_dvec3() + tp.eye);
                                        let alt = wc.length() - world.body.radius(wc.normalize());
                                        let alb = if alt < 1.0 {
                                            Vec3::new(0.10, 0.18, 0.32)
                                        } else if alt < 1800.0 {
                                            Vec3::new(0.25, 0.38, 0.16)
                                        } else if alt < 2700.0 {
                                            Vec3::new(0.42, 0.38, 0.30)
                                        } else {
                                            Vec3::new(0.92, 0.93, 0.95)
                                        };
                                        let l = 0.18 + 0.82 * n.dot(sun).max(0.0);
                                        let c = (alb * l).powf(1.0 / 2.2) * 255.0;
                                        [c.x as u8, c.y as u8, c.z as u8]
                                    })
                                }
                            } else if hole[i] {
                                [255, 0, 255]
                            } else {
                                [24, 40, 80]
                            };
                            px.extend_from_slice(&rgb);
                        }
                    }
                    px
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    out.concat()
}

fn save_png(dir: &std::path::Path, name: &str, rgb: &[u8], w: u32, h: u32) {
    let _ = std::fs::create_dir_all(dir);
    let file = std::fs::File::create(dir.join(format!("{name}.png"))).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().unwrap();
    wr.write_image_data(rgb).unwrap();
    wr.finish().unwrap();
}

fn cache_root() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("EARTHDROP_CACHE")?);
    root.join("mapterhorn").exists().then_some(root)
}

// ---------------------------------------------------------------------------------------------
// The test
// ---------------------------------------------------------------------------------------------

#[test]
#[ignore = "needs a GPU and EARTHDROP_CACHE with the Zermatt tiles; run with --ignored"]
fn ground_to_orbit_over_zermatt() {
    let Some(root) = cache_root() else {
        eprintln!("terrain harness: EARTHDROP_CACHE not set or has no mapterhorn tiles; skipped");
        return;
    };
    let Some(mut gpu) = Gpu::new() else {
        eprintln!("terrain harness: no GPU; skipped");
        return;
    };
    let shots = std::env::var_os("ED_TERRAIN_SHOTS").map(PathBuf::from);
    let mut world = World::new(root);
    let mut tp = TerrainProvider::new(&*gpu.dev, TerrainConfig { clear_targets: true, ..Default::default() }).unwrap();
    let sun = Vec3::new(0.45, 0.6, -0.66).normalize();
    let t0 = std::time::Instant::now();

    let mut total = Counts::default();
    let mut worst_crack_off = 0u32;
    let mut holes_off_total = 0u32;
    let mut atlas_seen = [0u32; 4];
    println!("terrain harness: {:>9} {:>5} {:>6} {:>4} {:>8} {:>6} {:>6} {:>5} {:>5} | skirts off: {:>6} {:>5}", "agl m", "drawn", "lvls", "stch", "terrain", "holes", "skirt", "thick", "max", "holes", "max");
    for k in 0..PATH_POINTS {
        let t = k as f64 / (PATH_POINTS - 1) as f64;
        let agl = 10f64.powf(6.0 * t);
        let (s, sel) = shot(&mut world, agl, t, W, H);
        let view = ed_view(&s, W, H);
        tp.config.skirts = true;
        let st = tp.update(&sel, &world.res, world.planet.bodies(), &[world.pose], s.eye, view.pixels_per_radian());
        let read_atlas = k % 6 == 0;
        gpu.frame(&mut tp, &s, sun, W, H, read_atlas);
        let vis = gpu.read_vis(W, H);
        if read_atlas {
            let a = gpu.read_atlas();
            for c in 0..4 {
                atlas_seen[c] = atlas_seen[c].max(a[c]);
            }
        }
        let (c, hole) = analyse(&world, &s, &sel, &vis, W, H);
        if let Some(dir) = &shots {
            let rgb = shade(&world, &tp, &vis, &hole, sun, W, H);
            save_png(dir, &format!("terrain_{k:02}_{:.0}m", agl), &rgb, W, H);
        }
        // Same view without skirts: the raw cracks.
        tp.config.skirts = false;
        tp.update(&sel, &world.res, world.planet.bodies(), &[world.pose], s.eye, view.pixels_per_radian());
        gpu.frame(&mut tp, &s, sun, W, H, false);
        let vis_off = gpu.read_vis(W, H);
        let (off, hole_off) = analyse(&world, &s, &sel, &vis_off, W, H);
        if let Some(dir) = &shots
            && off.holes > 0
        {
            let rgb = shade(&world, &tp, &vis_off, &hole_off, sun, W, H);
            save_png(dir, &format!("terrain_{k:02}_{:.0}m_noskirt", agl), &rgb, W, H);
        }
        let levels = sel.draw.iter().map(|d| d.id.level()).fold((u8::MAX, 0u8), |(a, b), l| (a.min(l), b.max(l)));
        println!(
            "terrain harness: {:>9.1} {:>5} {:>2}-{:<3} {:>4} {:>8} {:>6} {:>6} {:>5} {:>5} | skirts off: {:>6} {:>5}",
            agl, st.drawn, levels.0, levels.1, st.stitched, c.terrain, c.holes, c.skirt, c.skirt_thick, c.skirt_max, off.holes, off.hole_max
        );
        assert_eq!(st.dropped, 0, "nodes dropped at {agl} m");
        total.holes += c.holes;
        total.skirt_thick += c.skirt_thick;
        total.skirt_max = total.skirt_max.max(c.skirt_max);
        worst_crack_off = worst_crack_off.max(off.hole_max);
        holes_off_total += off.holes;
    }
    println!(
        "terrain harness: path done in {:.1} s: holes {}, skirt pixels thicker than 1 px {}, thickest skirt run {} px; skirts off: crack pixels {}, widest crack {} px; flat (no data) nodes {}; pages {}; shadow atlas texels per cascade (max) {:?}",
        t0.elapsed().as_secs_f32(),
        total.holes,
        total.skirt_thick,
        total.skirt_max,
        holes_off_total,
        worst_crack_off,
        world.src.flat_nodes.load(Ordering::Relaxed),
        world.res.pages.len(),
        atlas_seen
    );

    // GPU time of the terrain passes at 1440p.
    tp.config.skirts = true;
    for (k, agl) in [(0usize, 1.0f64), (8, 100.0), (16, 10_000.0), (23, 1_000_000.0)] {
        let t = k as f64 / (PATH_POINTS - 1) as f64;
        let (s, sel) = shot(&mut world, agl, t, 2560, 1440);
        let view = ed_view(&s, 2560, 1440);
        let st = tp.update(&sel, &world.res, world.planet.bodies(), &[world.pose], s.eye, view.pixels_per_radian());
        let (mut ms, mut sms) = (Vec::new(), Vec::new());
        for f in 0..12 {
            gpu.frame(&mut tp, &s, sun, 2560, 1440, false);
            if f >= 3
                && let Some(ft) = FrameTimings::read(&*gpu.dev, &mut gpu.scratch)
            {
                if let Some(x) = ft.pass_ms("terrain") {
                    ms.push(x);
                }
                if let Some(x) = ft.pass_ms("terrain_shadow") {
                    sms.push(x);
                }
            }
        }
        gpu.dev.wait_idle();
        let med = |v: &mut Vec<f32>| {
            v.sort_by(f32::total_cmp);
            v.get(v.len() / 2).copied().unwrap_or(f32::NAN)
        };
        println!(
            "terrain harness: 1440p at {agl:>9.0} m AGL: {} nodes ({} tris), terrain pass {:.3} ms, terrain_shadow {:.3} ms (cascade draws {:?}) (median of {})",
            st.drawn,
            st.drawn * TRIS,
            med(&mut ms),
            if tp.cascade_draws().iter().any(|&c| c > 0) { med(&mut sms) } else { 0.0 },
            tp.cascade_draws(),
            ms.len()
        );
    }

    Box::new(tp).destroy(&*gpu.dev);
    gpu.destroy();
    assert_eq!(total.holes, 0, "hole pixels along the path");
    assert_eq!(total.skirt_thick, 0, "cracks wider than 1 px (visible skirt runs) along the path");
}

#[test]
fn harness_heights_agree_across_levels() {
    // Pure-function heights: a lattice point shared by a node and its parent decodes the same.
    let Some(root) = cache_root() else { return };
    let src = Source::new(root);
    let child = NodeId::from_unit(BodyId(Body::earth().id.0), dir_of(ZERMATT.0, ZERMATT.1), 12);
    let parent = child.parent().unwrap();
    let (pc, pp) = (src.page(child), src.page(parent));
    let (qx, qy) = ((child.x() & 1) as usize * 128, (child.y() & 1) as usize * 128);
    for (i, j) in [(0usize, 0usize), (8, 8), (64, 128), (256, 256), (128, 0)] {
        assert_eq!(pc.height(i, j), pp.height(qx + i / 2, qy + j / 2), "({i},{j})");
    }
}
