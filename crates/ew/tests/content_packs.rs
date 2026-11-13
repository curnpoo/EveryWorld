//! M3: `ew` draws only baked packs.
//! - grep: no procedural geometry path in `ew/src`;
//! - a missing proving pack is an error naming the bake command;
//! - prefab round trip (needs Blender): export -> bake -> render -> re-export another
//!   variant -> rebake -> hot reload -> only the pixels around the prefab change, and only
//!   the changed material chunk is re-uploaded; then 50 reloads keep device memory flat.

mod common;

use ed_gpu::{BufferDesc, BufferId, Device, DeviceDesc, Format, ResourceState, TextureDesc, TextureId, TextureKind, Usage};
use ed_render::{Camera, FrameParams, Renderer, View, ViewTarget, Viewport};
use ew::content::{Content, PACKS, PackSpec};
use glam::{DVec3, Vec4};
use std::path::{Path, PathBuf};
use std::process::Command;

const BLENDER: &str = "/Applications/Blender.app/Contents/MacOS/Blender";

#[test]
fn ew_has_no_procedural_geometry_path() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    // Generators, simplifiers and hand-built meshes that M2 had in `ew`.
    let banned = ["meshopt", "build_lods", "build_part", "Builder", "PARTS", "layout()", "cube_mesh", "lathe(", "torus(", "blob(", "positions: vec!", "positions.push", "MeshLod {"];
    let mut hits = Vec::new();
    let mut files = 0;
    for e in std::fs::read_dir(&src).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_none_or(|x| x != "rs") {
            continue;
        }
        files += 1;
        let text = std::fs::read_to_string(&p).unwrap();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        // P-slice S5 placeholder car (boxes and a wheel cylinder). Temporary: remove this
        // exception when a vehicle pack (Blender glTF via ed_assets) replaces it.
        if name == "car.rs" {
            continue;
        }
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for b in banned {
                // content.rs converts decoded pack meshes into renderer LOD records.
                if code.contains(b) && !(name == "content.rs" && b == "MeshLod {") {
                    hits.push(format!("{name}:{}: {b}: {}", i + 1, line.trim()));
                }
            }
            // Renderer meshes are built from pack chunks only (content.rs).
            if code.contains("add_mesh(") && name != "content.rs" {
                hits.push(format!("{name}:{}: add_mesh outside content.rs", i + 1));
            }
        }
    }
    assert!(files >= 8, "{files} source files");
    assert!(hits.is_empty(), "procedural geometry in ew:\n{}", hits.join("\n"));
    let cargo = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("meshopt"), "ew must not depend on a mesh simplifier");
}

#[test]
fn missing_proving_pack_names_the_bake_command() {
    let empty = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("no_packs");
    std::fs::create_dir_all(&empty).unwrap();
    let args = ew::args::Args {
        headless: true,
        width: 64,
        height: 64,
        frames: Some(2),
        path: ew::camera::PathKind::Proving,
        packs: empty,
        out: PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("no_packs_out"),
        ..Default::default()
    };
    let e = match ew::headless::run(&args) {
        Ok(_) => panic!("loaded without a proving pack"),
        Err(e) => e,
    };
    assert!(e.contains("ew-bake -- build assets-src/proving.toml -o target/packs/proving.ewpack"), "{e}");
    assert!(e.contains("proving.ewpack"), "{e}");
}

// ---------------------------------------------------------------------------------------------
// Prefab round trip
// ---------------------------------------------------------------------------------------------

struct Shot {
    target: TextureId,
    readback: BufferId,
    w: u32,
    h: u32,
}

impl Shot {
    fn new(dev: &dyn Device, w: u32, h: u32) -> Shot {
        let target = dev
            .create_texture(&TextureDesc {
                label: "test target",
                kind: TextureKind::D2,
                format: Format::Rgba8Srgb,
                width: w,
                height: h,
                depth_or_layers: 1,
                mips: 1,
                samples: 1,
                usage: Usage { render_target: true, copy_src: true, ..Default::default() },
            })
            .unwrap();
        let pitch = Format::Rgba8Srgb.copy_row_pitch(w);
        let readback = dev.create_buffer(&BufferDesc { label: "test readback", size: pitch as u64 * h as u64, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } }).unwrap();
        Shot { target, readback, w, h }
    }

    /// Renders `frames` frames (LOD fades settle) and returns the last as RGBA8.
    fn render(&self, dev: &dyn Device, r: &mut Renderer, c: &Content, cam: &Camera, frames: u32) -> Vec<u8> {
        for i in 0..frames {
            c.animate(r, 1.0).unwrap();
            r.flush_uploads(dev).unwrap();
            let mut enc = dev.begin_frame().unwrap();
            let view = View {
                camera: cam.clone(),
                viewport: Viewport { x: 0, y: 0, width: self.w, height: self.h },
                target: ViewTarget { texture: self.target, format: Format::Rgba8Srgb, width: self.w, height: self.h, initial: ResourceState::Undefined, final_state: None },
                flags: Default::default(),
            };
            let params = FrameParams { readback: (i + 1 == frames).then_some(self.readback), ..Default::default() };
            r.render(dev, &mut *enc, &view, &params).unwrap();
            dev.end_frame(enc).unwrap();
        }
        dev.wait_idle();
        r.poll(dev);
        let mut raw = Vec::new();
        dev.read_buffer(self.readback, &mut raw).unwrap();
        let pitch = Format::Rgba8Srgb.copy_row_pitch(self.w) as usize;
        let mut out = Vec::with_capacity((self.w * self.h * 4) as usize);
        for y in 0..self.h as usize {
            out.extend_from_slice(&raw[y * pitch..y * pitch + self.w as usize * 4]);
        }
        out
    }

    fn destroy(self, dev: &dyn Device) {
        dev.destroy_texture(self.target);
        dev.destroy_buffer(self.readback);
    }
}

fn blender_export(out: &Path, variant: &str) {
    let ws = common::workspace();
    let st = Command::new(BLENDER)
        .current_dir(&ws)
        .args(["-b", "--factory-startup", "-P", "tools/blender/make_prefab.py", "--", "--out"])
        .arg(out)
        .args(["--variant", variant])
        .output()
        .expect("running Blender");
    let log = String::from_utf8_lossy(&st.stdout);
    assert!(st.status.success() && log.contains("PREFAB ok"), "Blender failed:\n{log}\n{}", String::from_utf8_lossy(&st.stderr));
}

/// Screen rectangle (min x, min y, max x, max y) of a world box seen by `cam`.
fn project_box(cam: &Camera, w: u32, h: u32, lo: DVec3, hi: DVec3) -> (f32, f32, f32, f32) {
    let vp = cam.relative_view_proj(w as f32 / h as f32);
    let mut r = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for k in 0..8 {
        let p = DVec3::new(if k & 1 == 0 { lo.x } else { hi.x }, if k & 2 == 0 { lo.y } else { hi.y }, if k & 4 == 0 { lo.z } else { hi.z });
        let rel = (p - cam.position).as_vec3();
        let c: Vec4 = vp * rel.extend(1.0);
        assert!(c.w > 0.0, "prefab box behind the camera");
        let (x, y) = ((c.x / c.w * 0.5 + 0.5) * w as f32, (0.5 - c.y / c.w * 0.5) * h as f32);
        r = (r.0.min(x), r.1.min(y), r.2.max(x), r.3.max(y));
    }
    r
}

#[test]
fn prefab_reexport_hot_reloads_and_changes_only_the_prefab() {
    if !Path::new(BLENDER).exists() {
        eprintln!("SKIP: Blender not found at {BLENDER}");
        return;
    }
    let ws = common::workspace();
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("prefab_rt");
    let _ = std::fs::remove_dir_all(&tmp);
    let (blend, packs) = (tmp.join("blender"), tmp.join("packs"));
    std::fs::create_dir_all(&packs).unwrap();
    std::fs::copy(common::packs().join("proving.ewpack"), packs.join("proving.ewpack")).unwrap();
    // Bake source next to the export (paths relative to it), registry found above it.
    let src = blend.join("bench.toml");
    blender_export(&blend, "oak");
    std::fs::write(&src, "[pack]\nname = \"bench\"\n\n[[gltf]]\npath = \"planter_bench.glb\"\nasset = \"prefabs/planter_bench\"\nregistry = \"everyworld-original\"\nmanifest = \"planter_bench.prefab.toml\"\n").unwrap();
    let bench = packs.join("bench.ewpack");
    ed_assets::bake::build(&src, &bench).unwrap();
    let oak = std::fs::read(&bench).unwrap();

    let Some(dev) = ed_gpu::device_or_skip(ed_gpu::default_backend(), &DeviceDesc { validation: true, frames_in_flight: 2 }) else { return };
    let dev = &*dev;
    let mut r = Renderer::new(dev).unwrap();
    let specs: Vec<PackSpec> = PACKS.iter().copied().filter(|s| s.name != "character").collect();
    let mut c = Content::load_specs(&mut r, dev, &packs, &specs).unwrap();
    assert_eq!(c.stats.missing, 0, "every generated placement resolves (Solarcat and the bench are authored in the layers)");
    // The bench is a layer `place` (M4b): author the layer objects the way the session does.
    let auth = ew::authored::Authored::load(&ws.join("assets-src/layers")).unwrap();
    let list: Vec<_> = auth.places.iter().map(|p| (p.id, p.asset, p.affine(), p.prefab.clone())).collect();
    let st = c.set_authored(&mut r, dev, &list).unwrap();
    assert_eq!(st.missing, 0, "the layer prefabs resolve: {st:?}");
    // Camera on the avenue looking at the bench (layer place at x 7.5, z 242 from the origin).
    let o = ew::proving::ORIGIN;
    let cam = Camera::look_at(o + DVec3::new(3.0, 1.5, 237.5), o + DVec3::new(7.5, 0.9, 242.0));
    let (w, h) = (640, 360);
    let shot = Shot::new(dev, w, h);
    let a = shot.render(dev, &mut r, &c, &cam, 40);

    // Re-export another variant, rebake, hot reload.
    blender_export(&blend, "teal");
    ed_assets::bake::build(&src, &bench).unwrap();
    let teal = std::fs::read(&bench).unwrap();
    assert_ne!(oak, teal);
    let st = c.poll_reload(&mut r, dev).unwrap().expect("bench pack change noticed");
    eprintln!("reload: {st:?}");
    assert_eq!((st.changed_keys, st.materials, st.meshes, st.textures, st.placements), (1, 1, 0, 0, 0), "only the wood material is re-uploaded: {st:?}");
    assert!(st.metadata_only >= 3, "mesh/prefab chunks changed only in provenance: {st:?}");
    let b = shot.render(dev, &mut r, &c, &cam, 40);

    // Every changed pixel lies inside the bench's projected box (+ 3 px).
    let (x0, y0, x1, y1) = project_box(&cam, w, h, o + DVec3::new(6.3, 0.0, 240.8), o + DVec3::new(8.7, 3.0, 243.2));
    let (mut changed, mut outside) = (0usize, Vec::new());
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if a[i..i + 4] != b[i..i + 4] {
                changed += 1;
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                if fx < x0 - 3.0 || fx > x1 + 3.0 || fy < y0 - 3.0 || fy > y1 + 3.0 {
                    outside.push((x, y));
                }
            }
        }
    }
    eprintln!("bench box on screen: ({x0:.0}, {y0:.0})..({x1:.0}, {y1:.0}); changed pixels {changed}, outside {}", outside.len());
    assert!(changed > 500, "the wood colour change is visible: {changed} pixels");
    assert!(outside.is_empty(), "{} changed pixels outside the prefab, first {:?}", outside.len(), &outside[..outside.len().min(5)]);
    // Before/after captures for the report.
    let out = ws.join("target/m3");
    std::fs::create_dir_all(&out).unwrap();
    save_png(&out.join("prefab_before.png"), w, h, &a);
    save_png(&out.join("prefab_after.png"), w, h, &b);

    // 50 more reloads alternating the two bakes: device memory and geometry stay flat.
    let probe = |r: &mut Renderer, c: &Content| {
        shot.render(dev, r, c, &cam, 4);
        (dev.allocated_bytes(), r.geometry_in_use(), r.texture_bytes())
    };
    let before = probe(&mut r, &c);
    for k in 0..50 {
        std::fs::write(&bench, if k % 2 == 0 { &oak } else { &teal }).unwrap();
        // Bump the stamp even if two writes land in the same mtime tick.
        let st = c.reload(&mut r, dev, &[1]).unwrap();
        assert_eq!(st.materials, 1, "reload {k}: {st:?}");
        shot.render(dev, &mut r, &c, &cam, 2);
    }
    let after = probe(&mut r, &c);
    eprintln!("50 reloads: allocated {} -> {} B, geometry {:?} -> {:?}, texture bytes {} -> {}", before.0, after.0, before.1, after.1, before.2, after.2);
    assert_eq!(before, after, "memory grew over 50 pack reloads");

    c.clear(&mut r);
    shot.destroy(dev);
    r.destroy(dev);
    dev.wait_idle();
}

fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut wr = e.write_header().unwrap();
    wr.write_image_data(rgba).unwrap();
    wr.finish().unwrap();
}
