//! Real terrain in `ew` (P-slice S1): height pages from the `.earthdrop-cache` disk cache,
//! `ed_planet` selection each frame and the renderer's terrain provider, all in one local frame
//! ([`Anchor`]: render x = east, y = up, z = -north at a point on the ellipsoid).
//!
//! * Source: [`CacheSource`] serves Mapterhorn tiles from the cache; the app's
//!   [`CacheSource::for_app`] also fetches missing ones (Mapterhorn only) unless `EW_OFFLINE=1`. A zoom that is not on disk is resampled from what is: finer zooms from the
//!   finest cached ancestor (bilinear upsampling), coarser zooms from the coarsest cached zoom
//!   (point-sampled). No data at all is 0 m. A pure function of the cache contents.
//! * Pages: [`ed_data::node::node_heights`] (the production lattice rule, `d * r(d)`), built
//!   synchronously on scoped worker threads, at most `max_loads` per update (headless: all of
//!   them, re-selecting until the selection is complete).
//! * [`TerrainWorld`] is the [`Residency`] other modules read (S3 collision: `height_page`).
//!
//! Places for the headless paths: [`ZERMATT`] and [`BLUE_RIDGE`] ([`path_pose`]).

use crate::camera::Pose;
use ed_data::DiskCache;
use ed_data::node::{ElevationSource, Raster, SourceView};
use ed_geo::NodeId;
use ed_planet::{Anchor, Body, BodyPose, ChunkMask, HeightPage, Planet, Residency, SelectConfig, Selection, View};
use ed_render::Camera;
use ed_geo::CanopyPage;
use ed_render::terrain::{TerrainConfig, TerrainProvider, TerrainStats, TreesFar};
use glam::{DQuat, DVec3};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// A place a terrain path starts from: anchor (lat, lon degrees) and the path's camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
    /// Ground height near the anchor (m above the ellipsoid, from the data; the path's AGL base).
    pub ground_m: f64,
    /// Start height above `ground_m` (m) and height gain by the end of the path (m).
    pub agl_m: f64,
    pub rise_m: f64,
    /// What the camera looks at: (lat, lon, height m).
    pub look: (f64, f64, f64),
}

/// Zermatt village, looking up-valley (south-west) to the Matterhorn.
pub const ZERMATT: Place = Place { name: "zermatt", lat: 46.0207, lon: 7.7491, ground_m: 1610.0, agl_m: 300.0, rise_m: 5000.0, look: (45.9763, 7.6586, 3600.0) };
/// The Black Mountains (Blue Ridge, North Carolina) from the south-west, looking north-east
/// across the ridges toward Mount Mitchell (the cached Mapterhorn z14 block).
pub const BLUE_RIDGE: Place = Place { name: "blueridge", lat: 35.660, lon: -82.360, ground_m: 814.0, agl_m: 450.0, rise_m: 4000.0, look: (35.7650, -82.2652, 1500.0) };

/// A synthetic rolling-hills place on the Swiss plateau for judging grass (no real data: heights
/// come from [`hills_height`], see [`CacheSource::hills`]). Looks about 400 m north at ground height.
pub const GRASSLAB: Place = Place { name: "grasslab", lat: 47.05, lon: 7.60, ground_m: 520.0, agl_m: 30.0, rise_m: 300.0, look: (47.05 + 400.0 / 110_574.0, 7.60, 520.0) };

/// A catalogue place whose ground height is read from the elevation data when it is first used
/// (see [`resolve_place`]). `look` is (lat, lon, metres above the ground there).
pub struct PlaceDef {
    pub name: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub agl_m: f64,
    pub rise_m: f64,
    pub look: (f64, f64, f64),
}

/// Real places for judging grass, trees and ground in other biomes (`--play NAME`, `--path NAME`).
/// Heights come from the elevation data at load time; fetched into the cache unless `EW_OFFLINE=1`.
pub const PLACES: &[PlaceDef] = &[
    // Erg Chebbi (Morocco): sand desert, dunes to the north-west.
    PlaceDef { name: "dunes", lat: 31.15, lon: -3.99, agl_m: 2.0, rise_m: 300.0, look: (31.17, -4.01, 60.0) },
    // Tucson Mountains (Arizona): saguaro desert scrub, ridge to the west.
    PlaceDef { name: "sonoran", lat: 32.25, lon: -111.17, agl_m: 2.0, rise_m: 300.0, look: (32.25, -111.215, 150.0) },
    // Arenal (Costa Rica): rainforest hills, volcano to the north-west.
    PlaceDef { name: "jungle", lat: 10.43, lon: -84.74, agl_m: 2.0, rise_m: 300.0, look: (10.463, -84.703, 50.0) },
    // Everglades (Florida): sawgrass wetland, flat.
    PlaceDef { name: "everglades", lat: 25.75, lon: -80.85, agl_m: 2.0, rise_m: 300.0, look: (25.76, -80.85, 2.0) },
    // Toolik Lake (Alaska): tundra, the Brooks Range to the south.
    PlaceDef { name: "tundra", lat: 68.63, lon: -149.60, agl_m: 2.0, rise_m: 300.0, look: (68.58, -149.60, 600.0) },
    // Alpe di Siusi (Dolomites): alpine meadow, Sciliar to the north-west.
    PlaceDef { name: "siusi", lat: 46.54, lon: 11.62, agl_m: 2.0, rise_m: 300.0, look: (46.53, 11.60, 200.0) },
    // Flint Hills (Kansas): tallgrass prairie, rolling.
    PlaceDef { name: "prairie", lat: 38.60, lon: -96.50, agl_m: 2.0, rise_m: 300.0, look: (38.61, -96.50, 5.0) },
    // Central Iowa: cropland, flat.
    PlaceDef { name: "iowa", lat: 41.95, lon: -93.65, agl_m: 2.0, rise_m: 300.0, look: (42.01, -93.60, 3.0) },
    // Cades Cove (Great Smoky Mountains): meadow, forested ridges around.
    PlaceDef { name: "smokies", lat: 35.60, lon: -83.80, agl_m: 2.0, rise_m: 300.0, look: (35.59, -83.78, 150.0) },
];

/// What `--play` and `--path` take besides the fixed paths: a [`PLACES`] name or `LAT,LON`.
/// Pure parsing (no I/O); returns the index into the request table ([`requested`]).
pub fn parse_place(spec: &str) -> Result<u16, String> {
    let def = if let Some(d) = PLACES.iter().find(|d| d.name == spec) {
        (spec.to_string(), d.lat, d.lon, d.agl_m, d.rise_m, Some(d.look))
    } else {
        let v: Vec<f64> = spec.split(',').map(|t| t.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| unknown_place(spec))?;
        match v[..] {
            [lat, lon] if (-85.0..=85.0).contains(&lat) && (-180.0..=180.0).contains(&lon) => (format!("{lat:.4},{lon:.4}"), lat, lon, 2.0, 300.0, None),
            _ => return Err(unknown_place(spec)),
        }
    };
    let mut r = REQUESTS.lock().unwrap();
    if let Some(i) = r.iter().position(|e| e.0 == def.0) {
        return Ok(i as u16);
    }
    r.push(def);
    Ok((r.len() - 1) as u16)
}

fn unknown_place(spec: &str) -> String {
    let names: Vec<&str> = PLACES.iter().map(|d| d.name).collect();
    format!("unknown place {spec:?}: expected zermatt|blueridge|grasslab|{}|LAT,LON", names.join("|"))
}

type Request = (String, f64, f64, f64, f64, Option<(f64, f64, f64)>);
static REQUESTS: Mutex<Vec<Request>> = Mutex::new(Vec::new());
static RESOLVED: Mutex<BTreeMap<u16, &'static Place>> = Mutex::new(BTreeMap::new());

/// The [`Place`] of a parsed request, its ground height read from the elevation data (fetched
/// into the cache unless offline) on first use. A point with no elevation data is at 0 m.
/// A `LAT,LON` place looks 600 m north at ground height.
pub fn requested(index: u16) -> &'static Place {
    if let Some(p) = RESOLVED.lock().unwrap().get(&index) {
        return p;
    }
    let (name, lat, lon, agl_m, rise_m, look) = REQUESTS.lock().unwrap()[index as usize].clone();
    let src = CacheSource::for_app(cache_root());
    let ground_m = src.height(lat, lon).unwrap_or(0.0);
    let look = match look {
        Some((la, lo, up)) => (la, lo, src.height(la, lo).unwrap_or(ground_m) + up),
        None => {
            let la = lat + 600.0 / 110_574.0;
            (la, lon, src.height(la, lon).unwrap_or(ground_m))
        }
    };
    let name: &'static str = Box::leak(name.into_boxed_str());
    let place: &'static Place = Box::leak(Box::new(Place { name, lat, lon, ground_m, agl_m, rise_m, look }));
    eprintln!("ew: place {name}: ground {ground_m:.1} m, look height {:.1} m", look.2);
    *RESOLVED.lock().unwrap().entry(index).or_insert(place)
}

/// Height (m) of the grass-lab hills at a lat/lon: 520 m plus 0 to ~50 m of smooth rolling relief
/// (four octaves of seeded egg-crate sines, wavelengths 85 to 800 m), damped to flat within ~60 m
/// of the anchor and easing in by ~260 m. A pure function of the point, so every zoom agrees.
pub fn hills_height(lat: f64, lon: f64) -> f64 {
    let x = (lon - GRASSLAB.lon) * GRASSLAB.lat.to_radians().cos() * 111_320.0;
    let y = (lat - GRASSLAB.lat) * 110_574.0;
    // (wavelength m, amplitude m, heading rad, phase a, phase b): fixed constants, no RNG.
    const OCT: [(f64, f64, f64, f64, f64); 4] = [(800.0, 14.0, 0.35, 0.7, 2.1), (330.0, 7.0, 1.2, 4.0, 0.3), (140.0, 2.5, 2.4, 1.9, 5.2), (85.0, 1.0, 0.8, 3.3, 1.1)];
    let mut sum = 0.0;
    let mut amp = 0.0;
    for (wl, a, th, pa, pb) in OCT {
        let k = std::f64::consts::TAU / wl;
        let (s, c) = th.sin_cos();
        let (u, v) = (x * c + y * s, -x * s + y * c);
        sum += a * (k * u + pa).sin() * (k * 1.3 * v + pb).sin();
        amp += a;
    }
    let t = (((x * x + y * y).sqrt() - 60.0) / 200.0).clamp(0.0, 1.0);
    let mask = t * t * (3.0 - 2.0 * t);
    GRASSLAB.ground_m + mask * (amp + sum)
}

/// Length of a terrain path at speed 1 (s): 2 s hold, then the climb.
pub const PATH_SECONDS: f64 = 10.0;
/// Screenshot times on a terrain path (s).
pub const LOW_SHOT_S: f64 = 1.0;
pub const MID_SHOT_S: f64 = 6.0;

/// Camera pose at path time `t` (s) for `place`, render frame of the place's anchor. Holds
/// for 2 s, then rises (ease-in) by `rise_m` by the end, keeping the look target.
pub fn path_pose(place: &Place, t: f64) -> Pose {
    let anchor = place_anchor(place);
    let t = t.clamp(0.0, PATH_SECONDS);
    let k = ((t - 2.0) / (PATH_SECONDS - 2.0)).clamp(0.0, 1.0);
    let h = place.ground_m + place.agl_m + place.rise_m * k * k;
    let eye = anchor.body_to_render(anchor.body.point(anchor.dir, h));
    let target = anchor.body_to_render(anchor.body.point(geodetic_dir(place.look.0, place.look.1), place.look.2));
    Pose::from_camera(&Camera::look_at(eye, target))
}

/// A fixed near-ground eye (`--ground`): `h` metres above the ellipsoid at (lat, lon), looking
/// along `heading` (degrees from north, clockwise) and `pitch` (degrees, + up) in the local
/// east/north/up frame of that point. Render frame of `place`'s anchor.
pub fn ground_pose(place: &Place, lat: f64, lon: f64, heading: f64, pitch: f64, h: f64) -> Pose {
    let anchor = place_anchor(place);
    let d = geodetic_dir(lat, lon);
    let eye_b = anchor.body.point(d, h);
    let up = d;
    let east = DVec3::Z.cross(up).normalize();
    let north = up.cross(east);
    let (hd, pt) = (heading.to_radians(), pitch.to_radians());
    let look = (east * hd.sin() + north * hd.cos()) * pt.cos() + up * pt.sin();
    let eye = anchor.body_to_render(eye_b);
    let target = anchor.body_to_render(eye_b + look * 100.0);
    Pose::from_camera(&Camera::look_at(eye, target))
}

/// Body direction of the ellipsoid point at (lat, lon): `ed_data`'s lattice rule inverted.
pub fn geodetic_dir(lat: f64, lon: f64) -> DVec3 {
    let e = ed_geo::Geodetic::new(lat, lon, 0.0).to_ecef();
    DVec3::new(e.x, e.y, e.z).normalize()
}

pub fn place_anchor(place: &Place) -> Anchor {
    Anchor::new(Body::earth(), geodetic_dir(place.lat, place.lon))
}

/// The terrain cache root: `EARTHDROP_CACHE`, else `.earthdrop-cache` in the working directory.
pub fn cache_root() -> PathBuf {
    std::env::var_os("EARTHDROP_CACHE").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".earthdrop-cache"))
}

// ---------------------------------------------------------------------------------------------
// Source: Mapterhorn tiles from the disk cache only
// ---------------------------------------------------------------------------------------------

type TileKey = (u8, u32, u32);
/// A map of once-computed tiles: the map lock is held only to find the cell, the (slow) decode
/// or resample runs in the cell's `OnceLock`, so two threads never build the same tile.
type TileMap = Mutex<BTreeMap<TileKey, Arc<OnceLock<Option<Arc<Raster>>>>>>;
/// A sampler's own few recently used tiles (no lock per tap).
type Local = Vec<(TileKey, Option<Arc<Raster>>)>;

fn cell(map: &TileMap, key: TileKey) -> Arc<OnceLock<Option<Arc<Raster>>>> {
    map.lock().unwrap().entry(key).or_default().clone()
}

/// Mapterhorn elevation tiles from the disk cache (see the module docs).
pub struct CacheSource {
    root: PathBuf,
    cache: DiskCache,
    tiles: TileMap,
    /// Tiles as on disk (decoded once even when many page builders ask at the same time).
    on_disk: TileMap,
    /// Cached zoom levels (ascending) and, for the coarsest, every tile on disk.
    zooms: Vec<u8>,
    coarse: BTreeSet<(u32, u32)>,
    /// Synthetic grass-lab heights ([`hills_height`]) instead of the disk cache.
    hills: bool,
    /// Missing Mapterhorn tiles are fetched into the cache ([`CacheSource::for_app`]).
    online: bool,
}

impl CacheSource {
    /// The synthetic rolling-hills source: heights are a pure function of each pixel's lat/lon.
    pub fn hills(root: impl Into<PathBuf>) -> CacheSource {
        ed_data::http::set_offline(true);
        let root = root.into();
        CacheSource { cache: DiskCache::new(root.clone()), root, tiles: Mutex::new(BTreeMap::new()), on_disk: Mutex::new(BTreeMap::new()), zooms: vec![14], coarse: BTreeSet::new(), hills: true, online: false }
    }

    /// A hills tile: every pixel centre through [`hills_height`].
    fn hills_tile(z: u8, x: u32, y: u32) -> Raster {
        let size = 512;
        let n = (1u64 << z) as f64;
        let mut heights = Vec::with_capacity(size * size);
        for py in 0..size {
            let my = std::f64::consts::PI * (1.0 - 2.0 * (y as f64 + (py as f64 + 0.5) / size as f64) / n);
            let lat = my.sinh().atan().to_degrees();
            for px in 0..size {
                let lon = (x as f64 + (px as f64 + 0.5) / size as f64) / n * 360.0 - 180.0;
                heights.push(hills_height(lat, lon) as f32);
            }
        }
        Raster { size, heights }
    }

    /// The app's source: fetches missing Mapterhorn tiles into the cache unless `EW_OFFLINE=1`
    /// (then the same as [`CacheSource::new`]). Tests and harnesses use `new` (never online).
    pub fn for_app(root: impl Into<PathBuf>) -> CacheSource {
        let root = root.into();
        if ed_data::http::offline() {
            return CacheSource::new(root);
        }
        CacheSource { cache: DiskCache::new(root.clone()), root, tiles: Mutex::new(BTreeMap::new()), on_disk: Mutex::new(BTreeMap::new()), zooms: vec![], coarse: BTreeSet::new(), hills: false, online: true }
    }

    pub fn is_online(&self) -> bool {
        self.online
    }

    pub fn new(root: impl Into<PathBuf>) -> CacheSource {
        ed_data::http::set_offline(true);
        let root = root.into();
        let dir = root.join("mapterhorn").join(ed_data::DATA_VERSION);
        let mut zooms: Vec<u8> = std::fs::read_dir(&dir).map(|r| r.flatten().filter_map(|e| e.file_name().to_str()?.parse().ok()).collect()).unwrap_or_default();
        zooms.retain(|&z| z <= ed_data::elevation::MAX_ZOOM);
        zooms.sort_unstable();
        let mut coarse = BTreeSet::new();
        if let Some(&z) = zooms.first() {
            for x in std::fs::read_dir(dir.join(z.to_string())).into_iter().flatten().flatten() {
                let Some(xi) = x.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else { continue };
                for y in std::fs::read_dir(x.path()).into_iter().flatten().flatten() {
                    if let Some(yi) = y.file_name().to_str().and_then(|s| s.split('.').next()?.parse::<u32>().ok()) {
                        coarse.insert((xi, yi));
                    }
                }
            }
        }
        CacheSource { cache: DiskCache::new(root.clone()), root, tiles: Mutex::new(BTreeMap::new()), on_disk: Mutex::new(BTreeMap::new()), zooms, coarse, hills: false, online: false }
    }

    /// Cached zooms (ascending); empty when the cache has no Mapterhorn tiles.
    pub fn zooms(&self) -> &[u8] {
        &self.zooms
    }

    /// A tile exactly as on disk.
    fn disk(&self, z: u8, x: u32, y: u32) -> Option<Arc<Raster>> {
        cell(&self.on_disk, (z, x, y))
            .get_or_init(|| {
                if self.online {
                    // Mapterhorn only (no unregistered Terrarium fallback); cache first.
                    return ed_data::elevation::fetch_mapterhorn(&self.cache, ed_geo::tile::TileId::new(z, x, y))
                        .map_err(|e| eprintln!("ew: mapterhorn {z}/{x}/{y}: {e}"))
                        .ok()
                        .flatten()
                        .map(|t| Arc::new(Raster { size: t.size, heights: t.heights }));
                }
                let path = self.root.join(format!("mapterhorn/{}/{z}/{x}/{y}.webp", ed_data::DATA_VERSION));
                if path.exists() { ElevationSource { cache: &self.cache }.elevation_tile(z, x, y) } else { None }
            })
            .clone()
    }

    /// `disk` through the sampler's local tiles.
    fn local_tile(&self, key: TileKey, local: &mut Local) -> Option<Arc<Raster>> {
        if let Some((_, t)) = local.iter().find(|(k, _)| *k == key) {
            return t.clone();
        }
        let t = self.disk(key.0, key.1, key.2);
        if local.len() >= 8 {
            local.clear();
        }
        local.push((key, t.clone()));
        t
    }

    /// Height at global pixel coordinates (pixel centres at +0.5) of zoom `zs`, Catmull-Rom
    /// bicubic (C1: bilinear left a crease on every source cell); `None` when a needed tile is
    /// missing. Outer taps in a missing neighbour tile fall back to the nearest inner tap.
    fn sample(&self, zs: u8, gx: f64, gy: f64, size: usize, last: &mut Local) -> Option<f32> {
        let n = 1i64 << zs;
        let w = n * size as i64;
        let (fx, fy) = (gx - 0.5, gy - 0.5);
        let (x0, y0) = (fx.floor() as i64, fy.floor() as i64);
        let (wx, wy) = (ed_data::node::cubic_weights(fx - x0 as f64), ed_data::node::cubic_weights(fy - y0 as f64));
        let mut px = [[0.0f64; 4]; 4];
        let mut have = [[false; 4]; 4];
        for m in 0..4usize {
            for k in 0..4usize {
                let x = (x0 - 1 + k as i64).rem_euclid(w);
                let y = (y0 - 1 + m as i64).clamp(0, w - 1);
                let key = (zs, (x / size as i64) as u32, (y / size as i64) as u32);
                match self.local_tile(key, last) {
                    Some(t) if t.size == size => {
                        px[m][k] = t.heights[(y % size as i64) as usize * size + (x % size as i64) as usize] as f64;
                        have[m][k] = true;
                    }
                    _ => {}
                }
            }
        }
        for m in 0..4usize {
            for k in 0..4usize {
                if !have[m][k] {
                    let (mm, kk) = (m.clamp(1, 2), k.clamp(1, 2));
                    if !have[mm][kk] {
                        return None;
                    }
                    px[m][k] = px[mm][kk];
                }
            }
        }
        let mut h = 0.0;
        for m in 0..4 {
            h += wy[m] * (0..4).map(|k| wx[k] * px[m][k]).sum::<f64>();
        }
        Some(h as f32)
    }

    /// A tile not on disk, resampled from a cached zoom `zs`.
    fn resample(&self, z: u8, x: u32, y: u32, zs: u8, size: usize) -> Raster {
        let scale = 2f64.powi(zs as i32 - z as i32);
        let mut heights = Vec::with_capacity(size * size);
        let mut last = Local::new();
        for py in 0..size {
            for px in 0..size {
                // Pixel centre in zoom-z global pixels, then in zoom-zs global pixels.
                let gx = ((x as usize * size + px) as f64 + 0.5) * scale;
                let gy = ((y as usize * size + py) as f64 + 0.5) * scale;
                heights.push(self.sample(zs, gx, gy, size, &mut last).unwrap_or(0.0));
            }
        }
        Raster { size, heights }
    }

    /// Height (m) at lat/lon from the finest cached zoom that has it.
    pub fn height(&self, lat: f64, lon: f64) -> Option<f64> {
        if self.hills {
            return Some(hills_height(lat, lon));
        }
        let zs: Vec<u8> = if self.online { vec![14, 12, 10, 8, 6] } else { self.zooms.iter().rev().copied().collect() };
        for z in zs {
            let n = (1u64 << z) as f64;
            let latr = lat.clamp(-ed_geo::tile::MAX_LAT, ed_geo::tile::MAX_LAT).to_radians();
            let gx = (lon + 180.0) / 360.0 * n;
            let gy = (1.0 - (latr.tan() + 1.0 / latr.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
            let Some(t) = self.disk(z, gx.floor() as u32, gy.floor() as u32) else { continue };
            let mut last = Local::new();
            if let Some(h) = self.sample(z, gx * t.size as f64, gy * t.size as f64, t.size, &mut last) {
                return Some(h as f64);
            }
        }
        None
    }
}

impl SourceView for CacheSource {
    fn elevation_tile(&self, z: u8, x: u32, y: u32) -> Option<Arc<Raster>> {
        cell(&self.tiles, (z, x, y)).get_or_init(|| self.build_tile(z, x, y)).clone()
    }
}

impl CacheSource {
    fn build_tile(&self, z: u8, x: u32, y: u32) -> Option<Arc<Raster>> {
        if self.hills {
            return Some(Arc::new(Self::hills_tile(z, x, y)));
        }
        if self.online {
            // Exact tile when Mapterhorn has it, else upsampled from the nearest coarser one.
            if z <= ed_data::elevation::MAX_ZOOM {
                if let Some(t) = self.disk(z, x, y) {
                    return Some(t);
                }
            }
            let top = z.min(ed_data::elevation::MAX_ZOOM + 1).saturating_sub(1);
            return (0..=top).rev().find_map(|zs| {
                let sh = z - zs;
                let a = self.disk(zs, x >> sh, y >> sh)?;
                Some(Arc::new(self.resample(z, x, y, zs, a.size)))
            });
        }
        let on_disk = if self.zooms.contains(&z) { self.disk(z, x, y) } else { None };
        on_disk.or_else(|| {
            let &zmin = self.zooms.first()?;
            if z < zmin {
                // From the coarsest cached zoom, when any of its tiles under this one exists.
                let sh = zmin - z;
                let (x0, y0, n) = (x << sh, y << sh, 1u32 << sh);
                let any = self.coarse.range((x0, 0)..(x0 + n, 0)).find(|(_, cy)| *cy >= y0 && *cy < y0 + n);
                any.and_then(|&(cx, cy)| self.disk(zmin, cx, cy)).map(|t| Arc::new(self.resample(z, x, y, zmin, t.size)))
            } else {
                // Upsampled from the finest cached ancestor.
                self.zooms.iter().rev().filter(|&&zs| zs < z).find_map(|&zs| {
                    let sh = z - zs;
                    let a = self.disk(zs, x >> sh, y >> sh)?;
                    Some(Arc::new(self.resample(z, x, y, zs, a.size)))
                })
            }
        })
    }
}

// ---------------------------------------------------------------------------------------------
// Residency and the per-frame driver
// ---------------------------------------------------------------------------------------------

/// Resident height pages by node (the loader's table), and canopy pages (trees G3) when the
/// far forest rung is on: `None` = built, no canopy anywhere on the node.
#[derive(Default)]
pub struct PageStore {
    pages: BTreeMap<NodeId, Arc<HeightPage>>,
    canopy: BTreeMap<NodeId, Option<Arc<CanopyPage>>>,
    /// Ground pages (grass plan G2), built with the canopy page; not uploaded yet (G3/G4).
    ground: BTreeMap<NodeId, Option<Arc<ed_data::node::GroundPage>>>,
    /// Canopy pages are part of a node's bytes.
    trees: bool,
}

impl PageStore {
    pub fn len(&self) -> usize {
        self.pages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }
}

impl Residency for PageStore {
    fn bytes(&self, _id: NodeId) -> u64 {
        (ed_planet::PAGE_N * ed_planet::PAGE_N * if self.trees { 6 } else { 2 }) as u64
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
    fn canopy_page(&self, id: NodeId) -> Option<Arc<CanopyPage>> {
        self.canopy.get(&id).cloned().flatten()
    }
}

/// The canopy page of `id` (trees G3) from the real canopy source through the disk cache, on
/// the node's own heights. `None` = no page, drawn with the old procedural forest: no source data
/// was read (offline, uncached), or the node is all sea. A land node with real data but no trees
/// still gets a page (zero cover, data share 3): "no forest here" is authoritative.
///
/// On the synthetic grass-lab hills the source is "data present, no cover" (grass, height 0), so
/// the shader's procedural forest fallback never fires there.
pub fn build_canopy(heights_src: &CacheSource, cache: &ed_data::DiskCache, id: NodeId, heights: &HeightPage) -> Option<Arc<CanopyPage>> {
    build_canopy_ground(heights_src, cache, id, heights).0
}

/// The ground page of `id` (grass plan G2: land-cover shares), alone. `None` = no source data
/// was read or the node is all sea. Prefer [`build_canopy_ground`] when the canopy page is
/// wanted too: it reads the land cover once.
pub fn build_ground(heights_src: &CacheSource, cache: &ed_data::DiskCache, id: NodeId, heights: &HeightPage) -> Option<Arc<ed_data::node::GroundPage>> {
    let page = if heights_src.hills {
        ed_data::node::node_ground(&hills_source(), id)
    } else {
        ed_data::node::node_ground(&ed_data::node::RealCanopySource { cache }, id)
    };
    ground_if_land(page, heights)
}

fn hills_source() -> ed_data::node::SyntheticCanopy {
    ed_data::node::SyntheticCanopy { cover: Box::new(|_, _| (ed_data::worldcover::GRASS, 0, 0)), height: Box::new(|_, _| Some(0.0)) }
}

fn ground_if_land(page: ed_data::node::GroundPage, heights: &HeightPage) -> Option<Arc<ed_data::node::GroundPage>> {
    (page.samples.iter().any(|s| s.iter().any(|&v| v > 0)) && node_has_land(heights)).then(|| Arc::new(page))
}

fn node_has_land(heights: &HeightPage) -> bool {
    (0..ed_planet::PAGE_N * ed_planet::PAGE_N).any(|k| heights.height(k % ed_planet::PAGE_N, k / ed_planet::PAGE_N) > 0.5)
}

/// The canopy and ground pages of `id` from one read of the land cover.
pub fn build_canopy_ground(heights_src: &CacheSource, cache: &ed_data::DiskCache, id: NodeId, heights: &HeightPage) -> (Option<Arc<CanopyPage>>, Option<Arc<ed_data::node::GroundPage>>) {
    let alt = |i: usize, j: usize| heights.height(i, j) as f64;
    let (page, ground) = if heights_src.hills {
        ed_data::node::node_canopy_ground(&hills_source(), id, &alt)
    } else {
        ed_data::node::node_canopy_ground(&ed_data::node::RealCanopySource { cache }, id, &alt)
    };
    let data = page.samples.iter().any(|s| s[3] >> 6 > 0);
    ((data && node_has_land(heights)).then(|| Arc::new(page)), ground_if_land(ground, heights))
}

/// Loader results.
enum Built {
    Height(NodeId, Arc<HeightPage>),
    Canopy(NodeId, Option<Arc<CanopyPage>>, Option<Arc<ed_data::node::GroundPage>>, f64),
}

/// Background page builders: a shared job queue, results back on a channel. Workers exit
/// when the [`Loader`] (its job sender) is dropped.
struct Loader {
    jobs: std::sync::mpsc::Sender<NodeId>,
    done: std::sync::mpsc::Receiver<Built>,
    inflight: BTreeSet<NodeId>,
}

impl Loader {
    /// `canopy`: also build each node's canopy page after its heights (sent separately, so a
    /// slow canopy read never holds the terrain back).
    fn new(src: Arc<CacheSource>, threads: usize, canopy: Option<Arc<ed_data::DiskCache>>) -> Loader {
        let (jobs, rx) = std::sync::mpsc::channel::<NodeId>();
        let (tx, done) = std::sync::mpsc::channel();
        let rx = Arc::new(Mutex::new(rx));
        for _ in 0..threads {
            let (rx, tx, src, canopy) = (rx.clone(), tx.clone(), src.clone(), canopy.clone());
            std::thread::spawn(move || {
                loop {
                    let job = rx.lock().map(|r| r.recv());
                    let Ok(Ok(id)) = job else { break };
                    let page = Arc::new(ed_data::node::node_heights(&*src, id));
                    if tx.send(Built::Height(id, page.clone())).is_err() {
                        break;
                    }
                    if let Some(cache) = &canopy {
                        let t = std::time::Instant::now();
                        let (c, g) = build_canopy_ground(&src, cache, id, &page);
                        let ms = t.elapsed().as_secs_f64() * 1e3;
                        if tx.send(Built::Canopy(id, c, g, ms)).is_err() {
                            break;
                        }
                    }
                }
            });
        }
        Loader { jobs, done, inflight: BTreeSet::new() }
    }
}

/// Counters of the latest [`TerrainWorld::update`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WorldStats {
    pub drawn: u32,
    pub wanted: u32,
    pub holes: u32,
    pub loaded: u32,
    pub resident: u32,
    pub evicted: u32,
}

/// Real terrain around one anchor: source, residency, selection (see the module docs).
pub struct TerrainWorld {
    pub anchor: Anchor,
    pub planet: Planet,
    pub config: SelectConfig,
    store: PageStore,
    src: Arc<CacheSource>,
    sel: Selection,
    /// Background page builder (windowed play): `None` builds on the calling thread only.
    loader: Option<Loader>,
    eye: DVec3,
    ppr: f64,
    /// Pages kept in CPU memory (bytes); least recently wanted ones beyond it are dropped.
    pub cache_bytes: u64,
    /// The far forest rung (`EW_TREES_FAR`, trees G3).
    pub trees_far: TreesFar,
    /// Canopy source cache (trees on).
    canopy_cache: Option<Arc<ed_data::DiskCache>>,
    /// Canopy page builds: count and total ms (headless report).
    pub canopy_built: (u32, f64),
    /// Ground pages built (with the canopy page, one shared land-cover read): count and total ms
    /// of that combined build.
    pub ground_built: (u32, f64),
    /// The grass lab's synthetic place (grass style A paints its type map there).
    pub grass_lab: bool,
}

impl TerrainWorld {
    pub fn new(cache: &Path, place: &Place) -> TerrainWorld {
        let anchor = place_anchor(place);
        let trees_far = TreesFar::from_env();
        TerrainWorld {
            planet: Planet::new(vec![anchor.body.clone()]),
            anchor,
            config: SelectConfig::default(),
            store: PageStore { trees: trees_far.on(), ..Default::default() },
            trees_far,
            canopy_cache: trees_far.on().then(|| Arc::new(ed_data::DiskCache::new(cache))),
            canopy_built: (0, 0.0),
            ground_built: (0, 0.0),
            grass_lab: place.name == GRASSLAB.name,
            src: Arc::new(if place.name == GRASSLAB.name { CacheSource::hills(cache) } else { CacheSource::for_app(cache) }),
            sel: Selection::default(),
            loader: None,
            eye: DVec3::ZERO,
            ppr: 1.0,
            cache_bytes: 1 << 30,
        }
    }

    /// The body-to-render pose of the anchor (what the renderer and the selector are given).
    pub fn pose(&self) -> BodyPose {
        self.anchor.render_pose()
    }

    pub fn source(&self) -> &CacheSource {
        &self.src
    }

    /// The shared source (background workers).
    pub fn source_arc(&self) -> Arc<CacheSource> {
        self.src.clone()
    }

    /// The residency (resident pages); `ed_planet::Residency` for other modules.
    pub fn residency(&self) -> &PageStore {
        &self.store
    }

    /// The resident page of `id`.
    pub fn page(&self, id: NodeId) -> Option<Arc<HeightPage>> {
        self.store.height_page(id)
    }

    pub fn selection(&self) -> &Selection {
        &self.sel
    }

    /// Builds the missing pages of `ids` now (scoped threads, results in sorted order); at most
    /// `max` of them. Returns how many were built.
    pub fn load(&mut self, ids: &[NodeId], max: usize) -> usize {
        let mut missing: Vec<NodeId> = ids.iter().copied().filter(|id| !self.store.pages.contains_key(id)).collect();
        missing.sort_unstable();
        missing.dedup();
        missing.truncate(max);
        if missing.is_empty() {
            return 0;
        }
        let threads = std::thread::available_parallelism().map_or(8, |n| n.get()).min(16);
        let src = &*self.src;
        // Contiguous runs of sorted ids per worker: neighbouring nodes share source tiles, so
        // each worker stays on a few tiles (a shared counter was ~3x more CPU). Results land by
        // index, so the store fills in sorted order whatever the thread timing.
        type Slot = OnceLock<(HeightPage, Option<(Option<Arc<CanopyPage>>, Option<Arc<ed_data::node::GroundPage>>)>, f64)>;
        let slots: Vec<Slot> = (0..missing.len()).map(|_| OnceLock::new()).collect();
        let chunk = missing.len().div_ceil(threads);
        let canopy = self.canopy_cache.as_deref();
        std::thread::scope(|s| {
            for (c, ids) in missing.chunks(chunk).enumerate() {
                let slots = &slots;
                s.spawn(move || {
                    for (k, &id) in ids.iter().enumerate() {
                        let page = ed_data::node::node_heights(src, id);
                        let t = std::time::Instant::now();
                        let cp = canopy.map(|cache| build_canopy_ground(src, cache, id, &page));
                        let _ = slots[c * chunk + k].set((page, cp, t.elapsed().as_secs_f64() * 1e3));
                    }
                });
            }
        });
        for (id, slot) in missing.iter().zip(slots) {
            let (p, cp, ms) = slot.into_inner().expect("page worker");
            self.store.pages.insert(*id, Arc::new(p));
            if let Some((cp, gp)) = cp {
                self.store.canopy.insert(*id, cp);
                self.store.ground.insert(*id, gp);
                self.canopy_built.0 += 1;
                self.canopy_built.1 += ms;
                self.ground_built.0 += 1;
                self.ground_built.1 += ms;
            }
        }
        missing.len()
    }

    /// Selects for `camera` and loads pages: up to `max_loads` per round; with `settle`,
    /// re-selects and loads until nothing is missing (or 12 rounds).
    pub fn update(&mut self, camera: &Camera, viewport: [u32; 2], max_loads: usize, settle: bool) -> WorldStats {
        let view = View {
            position: camera.position,
            rotation: DQuat::from_rotation_y(camera.yaw as f64) * DQuat::from_rotation_x(camera.pitch as f64),
            velocity: DVec3::ZERO,
            fov_y: camera.fov_y as f64,
            viewport,
            look_ahead_s: 0.0,
        };
        let poses = [self.pose()];
        let mut loaded = 0;
        let mut sel = self.planet.select(&view, &poses, &self.store, &self.config);
        for _ in 0..if settle { 12 } else { 1 } {
            let n = self.load(&sel.wanted, max_loads);
            loaded += n;
            if n == 0 {
                break;
            }
            sel = self.planet.select(&view, &poses, &self.store, &self.config);
            if sel.holes.is_empty() && sel.wanted.iter().all(|id| self.store.pages.contains_key(id)) {
                break;
            }
        }
        let evicted = self.planet.evict(self.cache_bytes);
        for id in &evicted {
            self.store.pages.remove(id);
            self.store.canopy.remove(id);
            self.store.ground.remove(id);
        }
        self.eye = camera.position;
        self.ppr = view.pixels_per_radian();
        let st = WorldStats { drawn: sel.draw.len() as u32, wanted: sel.wanted.len() as u32, holes: sel.holes.len() as u32, loaded: loaded as u32, resident: self.store.len() as u32, evicted: evicted.len() as u32 };
        self.sel = sel;
        st
    }

    /// Gives the latest selection to the renderer's terrain provider (call before
    /// `flush_uploads`). `None` when no provider is registered.
    pub fn feed(&self, r: &mut ed_render::Renderer) -> Option<TerrainStats> {
        let pose = self.pose();
        let t = r.terrain_mut()?;
        Some(t.update(&self.sel, &self.store, self.planet.bodies(), &[pose], self.eye, self.ppr))
    }

    /// The provider to register on the renderer (`Renderer::set_terrain`).
    pub fn provider(&self, dev: &dyn ed_gpu::Device) -> Result<TerrainProvider, ed_gpu::Error> {
        // `EW_TERRAIN_POOL=<slots>`: GPU page pool size (396 KB per slot with canopy pages). The
        // 1024 default saturated at 1440p near the ground (drawn nodes + their parents came to
        // about 1160), which dropped pages and left holes.
        let pool_slots = std::env::var("EW_TERRAIN_POOL").ok().and_then(|v| v.parse().ok()).unwrap_or(1536);
        let mut t = TerrainProvider::new(dev, TerrainConfig { select: self.config, trees_far: self.trees_far, pool_slots, ..Default::default() })?;
        // Scanned surface layers (P-slice S7): only when a local library is present.
        if let Some(m) = crate::terrain_mat::load() {
            t.set_materials(dev, &m)?;
        }
        Ok(t)
    }

    /// Starts `threads` background page builders (windowed play). Pages still arrive only
    /// through [`TerrainWorld::poll`], and a page is a pure function of its `NodeId`, so the
    /// order they arrive in changes what is drawn for a few frames, never what a page holds.
    pub fn start_loader(&mut self, threads: usize) {
        if self.loader.is_none() {
            self.loader = Some(Loader::new(self.src.clone(), threads.max(1), self.canopy_cache.clone()));
        }
    }

    /// Queues the missing pages of `ids` (in the given priority order) on the background
    /// loader, keeping at most `max_inflight` queued or building. Returns how many were queued.
    pub fn request(&mut self, ids: &[NodeId], max_inflight: usize) -> usize {
        let Some(l) = &mut self.loader else { return 0 };
        let mut n = 0;
        for &id in ids {
            if l.inflight.len() >= max_inflight {
                break;
            }
            if self.store.pages.contains_key(&id) || l.inflight.contains(&id) {
                continue;
            }
            if l.jobs.send(id).is_ok() {
                l.inflight.insert(id);
                n += 1;
            }
        }
        n
    }

    /// Takes the pages the background loader finished. Returns how many arrived.
    pub fn poll(&mut self) -> usize {
        let Some(l) = &mut self.loader else { return 0 };
        let mut n = 0;
        while let Ok(b) = l.done.try_recv() {
            match b {
                Built::Height(id, page) => {
                    // A node stays in flight until its canopy page is in too (trees on).
                    if self.canopy_cache.is_none() {
                        l.inflight.remove(&id);
                    }
                    self.store.pages.entry(id).or_insert(page);
                    n += 1;
                }
                Built::Canopy(id, page, ground, ms) => {
                    l.inflight.remove(&id);
                    if self.store.pages.contains_key(&id) {
                        self.store.canopy.insert(id, page);
                        self.store.ground.insert(id, ground);
                        self.canopy_built.0 += 1;
                        self.canopy_built.1 += ms;
                        self.ground_built.0 += 1;
                        self.ground_built.1 += ms;
                    }
                }
            }
        }
        n
    }

    /// Pages queued or building on the background loader.
    pub fn inflight(&self) -> usize {
        self.loader.as_ref().map_or(0, |l| l.inflight.len())
    }

    /// The non-blocking per-frame driver: takes finished pages, selects for `camera`, and
    /// queues the wanted pages that are missing, coarse levels first and then nearest the
    /// camera (a parent draws until its children arrive). Needs [`TerrainWorld::start_loader`].
    pub fn update_async(&mut self, camera: &Camera, viewport: [u32; 2], max_inflight: usize) -> WorldStats {
        let loaded = self.poll();
        let view = Self::view_of(camera, viewport);
        let poses = [self.pose()];
        let sel = self.planet.select(&view, &poses, &self.store, &self.config);
        let eye = self.anchor.render_to_body(camera.position).normalize();
        let mut missing: Vec<(u8, u64, NodeId)> = sel
            .wanted
            .iter()
            .filter(|id| !self.store.pages.contains_key(id))
            .map(|&id| (id.level(), (id.centre().angle_between(eye) * 1e9) as u64, id))
            .collect();
        missing.sort_unstable();
        let order: Vec<NodeId> = missing.into_iter().map(|m| m.2).collect();
        self.request(&order, max_inflight);
        let evicted = self.planet.evict(self.cache_bytes);
        for id in &evicted {
            self.store.pages.remove(id);
            self.store.canopy.remove(id);
            self.store.ground.remove(id);
        }
        self.eye = camera.position;
        self.ppr = view.pixels_per_radian();
        let st = WorldStats { drawn: sel.draw.len() as u32, wanted: sel.wanted.len() as u32, holes: sel.holes.len() as u32, loaded: loaded as u32, resident: self.store.len() as u32, evicted: evicted.len() as u32 };
        self.sel = sel;
        st
    }

    fn view_of(camera: &Camera, viewport: [u32; 2]) -> View {
        View {
            position: camera.position,
            rotation: DQuat::from_rotation_y(camera.yaw as f64) * DQuat::from_rotation_x(camera.pitch as f64),
            velocity: DVec3::ZERO,
            fov_y: camera.fov_y as f64,
            viewport,
            look_ahead_s: 0.0,
        }
    }

    /// The terrain point under render position `r` (render frame), from resident pages: the
    /// body point at the terrain height along `r`'s body direction.
    pub fn ground_point(&self, r: DVec3) -> Option<DVec3> {
        let b = self.anchor.render_to_body(r);
        let h = self.planet.height(&self.store, self.anchor.body.id, b)?.0;
        Some(self.anchor.body_to_render(self.anchor.body.point(b.normalize(), h)))
    }

    /// Terrain height (m above the ellipsoid) under render position `r`, from resident pages.
    pub fn ground(&self, r: DVec3) -> Option<f64> {
        let b = self.anchor.render_to_body(r);
        self.planet.height(&self.store, self.anchor.body.id, b).map(|h| h.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_start_at_the_anchor_and_rise() {
        for place in [ZERMATT, BLUE_RIDGE] {
            let p0 = path_pose(&place, 0.0);
            // Heights go along the body direction, not the ellipsoid normal: a few metres off.
            assert!(p0.position.x.abs() < 20.0 && p0.position.z.abs() < 20.0, "{p0:?}");
            assert!((p0.position.y - place.ground_m - place.agl_m).abs() < 1.0);
            let p1 = path_pose(&place, PATH_SECONDS);
            assert!(p1.position.y > p0.position.y + place.rise_m * 0.9);
        }
    }

    #[test]
    fn grasslab_hills_are_deterministic_continuous_and_bounded() {
        let a = CacheSource::hills(".");
        let z = 16u8;
        let ti = ed_geo::tile::TileId::containing(GRASSLAB.lat + 500.0 / 110_574.0, GRASSLAB.lon + 300.0 / 76_000.0, z);
        let (x, y) = (ti.x, ti.y);
        let t0 = CacheSource::hills_tile(z, x, y);
        assert_eq!(t0.heights.iter().flat_map(|h| h.to_bits().to_le_bytes()).collect::<Vec<_>>(), CacheSource::hills_tile(z, x, y).heights.iter().flat_map(|h| h.to_bits().to_le_bytes()).collect::<Vec<_>>());
        // Neighbouring tiles: last column of one next to the first column of the next (rows align).
        let t1 = CacheSource::hills_tile(z, x + 1, y);
        let n = t0.size;
        for py in (0..n).step_by(7) {
            let d = (t0.heights[py * n + n - 1] - t1.heights[py * n]).abs();
            assert!(d < 0.5, "x edge row {py}: {d}");
        }
        let t2 = CacheSource::hills_tile(z, x, y + 1);
        for px in (0..n).step_by(7) {
            let d = (t0.heights[(n - 1) * n + px] - t2.heights[px]).abs();
            assert!(d < 0.5, "y edge col {px}: {d}");
        }
        // Relief 0..80 m over the base, around the anchor; flat at the anchor; zooms agree.
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for i in 0..80 {
            for j in 0..80 {
                let h = hills_height(GRASSLAB.lat + (i as f64 - 40.0) * 5e-5 * 4.0, GRASSLAB.lon + (j as f64 - 40.0) * 7e-5 * 4.0) - GRASSLAB.ground_m;
                lo = lo.min(h);
                hi = hi.max(h);
            }
        }
        assert!(lo >= -1e-9 && hi <= 80.0 && hi > 5.0, "{lo} {hi}");
        assert!((hills_height(GRASSLAB.lat, GRASSLAB.lon) - GRASSLAB.ground_m).abs() < 1e-9);
        assert_eq!(a.height(GRASSLAB.lat, GRASSLAB.lon), Some(hills_height(GRASSLAB.lat, GRASSLAB.lon)));
    }

    #[test]
    fn zermatt_looks_south_west() {
        let p = path_pose(&ZERMATT, 0.0).camera();
        let f = p.forward();
        assert!(f.x < 0.0 && f.z > 0.0, "{f:?}");
    }

    /// Ground heights near the places, from the cache (offline; prints them).
    #[test]
    #[ignore]
    fn probe_heights() {
        let src = CacheSource::new(cache_root());
        for (lat, lon) in [(ZERMATT.lat, ZERMATT.lon), (ZERMATT.look.0, ZERMATT.look.1), (BLUE_RIDGE.lat, BLUE_RIDGE.lon), (BLUE_RIDGE.look.0, BLUE_RIDGE.look.1)] {
            eprintln!("{lat:.4} {lon:.4}: {:?}", src.height(lat, lon));
        }
    }

    /// FNV-1a over a page's bytes.
    fn page_hash(p: &HeightPage) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        let mut eat = |b: &[u8]| {
            for &x in b {
                h = (h ^ x as u64).wrapping_mul(0x100000001b3);
            }
        };
        for s in &p.samples {
            eat(&s.to_le_bytes());
        }
        eat(&p.scale.to_le_bytes());
        eat(&p.offset.to_le_bytes());
        eat(&p.min_m.to_le_bytes());
        eat(&p.max_m.to_le_bytes());
        h
    }

    /// Pages around Zermatt are bit-identical to the ones the pre-S6 sampler built (golden
    /// hashes taken from it), whatever the thread timing. Needs the Zermatt tiles in the cache.
    #[test]
    #[ignore = "needs EARTHDROP_CACHE with the Zermatt tiles"]
    fn zermatt_pages_are_byte_identical() {
        let src = CacheSource::new(cache_root());
        let lat = ZERMATT.lat.to_radians();
        let lon = ZERMATT.lon.to_radians();
        let dir = DVec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin());
        let body = Body::earth().id;
        let ids: Vec<NodeId> = [6u8, 10, 13, 15, 16].iter().map(|&l| NodeId::from_unit(body, dir, l)).collect();
        let got: Vec<u64> = std::thread::scope(|s| {
            let hs: Vec<_> = ids.iter().map(|&id| { let src = &src; s.spawn(move || page_hash(&ed_data::node::node_heights(src, id))) }).collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        eprintln!("PAGE HASHES {got:?}");
        let golden: [u64; 5] = [18410746731241230945, 10255135414199695523, 9571166664950946143, 10870807238342481626, 1803437067188059674];
        assert_eq!(got, golden);
    }
}
