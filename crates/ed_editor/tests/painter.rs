//! Headless painter test (Metal): render the editor panels offscreen through `Painter`, read
//! the image back, write `target/editor/panel.png`, check the content is non-trivial and
//! deterministic, and measure the painter's GPU time.
#![cfg(target_os = "macos")]

use ed_editor::editor::{EditorState, FrameStats, Inspected, LogLevel, ObjectInfo, PassTiming};
use ed_editor::painter::{PaintJob, PaintTarget, Painter};
use ed_editor::transform::Transform;
use ed_editor::{egui, panels};
use ed_geo::ObjectId;
use ed_gpu::*;
use glam::{DQuat, DVec3};

const CLEAR: [f32; 4] = [0.05, 0.08, 0.12, 1.0];

fn device() -> Box<dyn Device> {
    create_device(
        Backend::Metal,
        &DeviceDesc {
            validation: true,
            frames_in_flight: 2,
        },
    )
    .expect("Metal device")
}

fn stats() -> FrameStats {
    let p = |label: &str, depth, gpu_ms| PassTiming {
        label: label.into(),
        depth,
        gpu_ms,
    };
    FrameStats {
        frame: 1234,
        frame_ms: 16.4,
        cpu_ms: 5.1,
        gpu_ms: 11.7,
        passes: vec![
            p("frame", 0, 11.7),
            p("cull", 1, 0.42),
            p("visibility", 1, 3.9),
            p("resolve", 1, 5.2),
            p("egui", 1, 0.08),
        ],
        instances: 48_213,
        visible_instances: 9_377,
        gpu_memory_bytes: 1_734_000_000,
    }
}

fn state() -> EditorState {
    let mut s = EditorState::default();
    let id = ObjectId::authored(7);
    let t = Transform {
        position: DVec3::new(6_378_137.25, 12.5, -1_250_000.75),
        rotation: DQuat::from_rotation_y(0.4),
        scale: DVec3::ONE,
    };
    s.inspected.push(Inspected {
        object: id,
        info: Some(ObjectInfo {
            object: id,
            node: None,
            node_origin: Some(DVec3::new(6_378_000.0, 0.0, -1_250_000.0)),
            prefab: Some("kit/bench".into()),
            part: Some(0),
            mesh: Some("bench_seat".into()),
            material: Some("wood_oak".into()),
            lod: Some(0),
            transform: t,
        }),
        transform: Some(t),
        enu_rotation: Some(DQuat::from_rotation_z(0.4)),
        layer: Some("props".into()),
        layer_node: None,
        authored: None,
        movable: false,
    });
    s.time = 1.0;
    s.log(LogLevel::Info, "hot reload: shaders/egui.metal (0.8 ms)");
    s.log(LogLevel::Warn, "material wood_oak: missing roughness map");
    s.log(LogLevel::Error, "prefab kit/lamp: parse error at line 3");
    s
}

/// Run the panels for a few passes (windows settle their sizes over the first passes) and
/// return the last pass's paint job.
fn panel_job(size: [u32; 2], ppp: f32) -> PaintJob {
    let ctx = egui::Context::default();
    let mut st = state();
    let s = stats();
    let mut textures = egui::TexturesDelta::default();
    let mut last = None;
    for i in 0..4 {
        let mut raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32 / ppp, size[1] as f32 / ppp),
            )),
            time: Some(10.0 + i as f64),
            focused: true,
            ..Default::default()
        };
        raw.viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(ppp);
        let full = ctx.run_pass(raw, |ctx| panels::ui(&mut st, ctx, &s));
        // Keep every texture delta: the painter has not seen the earlier passes.
        textures.append(full.textures_delta);
        last = Some((full.shapes, full.pixels_per_point));
    }
    let (shapes, ppp) = last.unwrap();
    PaintJob {
        primitives: ctx.tessellate(shapes, ppp),
        textures,
        pixels_per_point: ppp,
    }
}

struct Target {
    tex: TextureId,
    rb: BufferId,
    w: u32,
    h: u32,
}

fn target(dev: &dyn Device, w: u32, h: u32) -> Target {
    let tex = dev
        .create_texture(&TextureDesc {
            label: "panel",
            kind: TextureKind::D2,
            format: Format::Rgba8Unorm,
            width: w,
            height: h,
            depth_or_layers: 1,
            mips: 1,
            samples: 1,
            usage: Usage {
                render_target: true,
                copy_src: true,
                ..Default::default()
            },
        })
        .unwrap();
    let rb = dev
        .create_buffer(&BufferDesc {
            label: "panel readback",
            size: (Format::Rgba8Unorm.copy_row_pitch(w) * h) as u64,
            usage: Usage {
                copy_dst: true,
                cpu_read: true,
                ..Default::default()
            },
        })
        .unwrap();
    Target { tex, rb, w, h }
}

/// Paint one frame (optionally copying it out). Returns nothing; read with `read`.
fn frame(dev: &dyn Device, painter: &mut Painter, t: &Target, copy: bool) {
    let mut e = dev.begin_frame().unwrap();
    e.transition(&[Transition::Texture {
        texture: t.tex,
        sub: None,
        from: ResourceState::Undefined,
        to: ResourceState::RenderTarget,
    }]);
    let pt = PaintTarget {
        texture: t.tex,
        format: Format::Rgba8Unorm,
        width: t.w,
        height: t.h,
        load: LoadOp::Clear(CLEAR),
    };
    painter.paint(dev, &mut *e, &pt).unwrap();
    if copy {
        e.transition(&[
            Transition::Texture {
                texture: t.tex,
                sub: None,
                from: ResourceState::RenderTarget,
                to: ResourceState::CopySrc,
            },
            Transition::Buffer {
                buffer: t.rb,
                from: ResourceState::Undefined,
                to: ResourceState::CopyDst,
            },
        ]);
        e.copy_texture_to_buffer(
            t.tex,
            Subresource { mip: 0, layer: 0 },
            t.rb,
            0,
            Format::Rgba8Unorm.copy_row_pitch(t.w),
        );
    }
    dev.end_frame(e).unwrap();
}

fn read(dev: &dyn Device, t: &Target) -> Vec<u8> {
    dev.wait_idle();
    let mut raw = Vec::new();
    dev.read_buffer(t.rb, &mut raw).unwrap();
    let pitch = Format::Rgba8Unorm.copy_row_pitch(t.w) as usize;
    let mut out = Vec::with_capacity((t.w * t.h * 4) as usize);
    for y in 0..t.h as usize {
        out.extend_from_slice(&raw[y * pitch..y * pitch + t.w as usize * 4]);
    }
    out
}

/// Render the panels once with a fresh painter and return RGBA8 pixels.
fn render(dev: &dyn Device, w: u32, h: u32) -> Vec<u8> {
    let mut painter = Painter::new(dev).unwrap();
    let mut job = panel_job([w, h], 1.0);
    painter.prepare(dev, &mut job).unwrap();
    assert!(
        painter.stats.draws > 0 && painter.stats.texture_uploads >= 1,
        "{:?}",
        painter.stats
    );
    let t = target(dev, w, h);
    frame(dev, &mut painter, &t, true);
    let px = read(dev, &t);
    painter.destroy(dev);
    dev.destroy_texture(t.tex);
    dev.destroy_buffer(t.rb);
    px
}

#[test]
fn panels_render_offscreen_deterministically() {
    let dev = device();
    let (w, h) = (960u32, 640u32);
    let a = render(&*dev, w, h);
    let b = render(&*dev, w, h);
    assert!(a == b, "two renders of the same UI differ");

    let path =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/editor/panel.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let f = std::fs::File::create(&path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&a).unwrap();
    eprintln!("wrote {}", path.display());

    // Content checks: the clear colour shows where no window is, windows cover a good part
    // of the image, and there is bright anti-aliased text (many grey levels).
    let clear: [u8; 4] = [13, 20, 31, 255];
    let pixel =
        |x: u32, y: u32| -> [u8; 4] { a[((y * w + x) * 4) as usize..][..4].try_into().unwrap() };
    let near = |p: [u8; 4], q: [u8; 4]| {
        p.iter()
            .zip(q)
            .all(|(a, b)| (*a as i32 - b as i32).abs() <= 1)
    };
    assert!(
        near(pixel(w / 2, h / 2), clear),
        "the centre is the clear colour: {:?}",
        pixel(w / 2, h / 2)
    );
    let n = (w * h) as usize;
    let covered = (0..n)
        .filter(|&i| !near(a[i * 4..i * 4 + 4].try_into().unwrap(), clear))
        .count();
    let bright = (0..n)
        .filter(|&i| a[i * 4] > 150 && a[i * 4 + 1] > 150 && a[i * 4 + 2] > 150)
        .count();
    let mut levels = std::collections::BTreeSet::new();
    for i in 0..n {
        levels.insert(a[i * 4 + 1]);
    }
    eprintln!(
        "covered {covered} of {n}, bright {bright}, green levels {}",
        levels.len()
    );
    assert!(covered > n / 8, "windows cover {covered} of {n} pixels");
    assert!(bright > 500, "text pixels: {bright}");
    assert!(
        levels.len() > 64,
        "anti-aliased text gives many levels: {}",
        levels.len()
    );
    // Coloured log lines (amber warning, red error) made it through the sRGB path.
    let amber = (0..n)
        .filter(|&i| {
            a[i * 4] > 180 && a[i * 4 + 1] > 130 && a[i * 4 + 1] < 215 && a[i * 4 + 2] < 110
        })
        .count();
    let red = (0..n)
        .filter(|&i| a[i * 4] > 180 && a[i * 4 + 1] < 110 && a[i * 4 + 2] < 110)
        .count();
    assert!(amber > 20 && red > 20, "amber {amber}, red {red}");
}

#[test]
fn painter_gpu_time() {
    let dev = device();
    let (w, h) = (2560u32, 1440u32);
    let mut painter = Painter::new(&*dev).unwrap();
    let mut job = if std::env::var_os("EW_EGUI_EMPTY").is_some() {
        PaintJob {
            primitives: vec![],
            textures: Default::default(),
            pixels_per_point: 2.0,
        }
    } else {
        panel_job([w, h], 2.0)
    };
    painter.prepare(&*dev, &mut job).unwrap();
    let t = target(&*dev, w, h);
    // Repaint the same geometry back to back (the ring is refilled and every draw recorded
    // each frame, as an idle editor does). The GPU clocks down when it idles between tiny
    // frames, which inflates timer results several times over, so the first half is warm-up
    // and only the second half is measured.
    let frames: usize = std::env::var("EW_EGUI_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(240);
    let mut ms = Vec::new();
    let mut timings = Vec::new();
    let mut seen = 0;
    for i in 0..frames {
        frame(&*dev, &mut painter, &t, false);
        if dev.completed_frames() > seen {
            seen = dev.completed_frames();
            dev.timings(&mut timings);
            if i >= frames / 2
                && let Some(tm) = timings.iter().find(|t| t.label == "egui")
            {
                ms.push(tm.gpu_ms);
            }
        }
    }
    dev.wait_idle();
    assert!(!ms.is_empty(), "no egui timer results: {timings:?}");
    ms.sort_by(|a, b| a.total_cmp(b));
    let median = ms[ms.len() / 2];
    eprintln!(
        "egui painter GPU at {w}x{h} (ppp 2): median {median:.4} ms, min {:.4}, max {:.4} over {} frames; {:?}, {} geometry bytes",
        ms[0],
        ms[ms.len() - 1],
        ms.len(),
        painter.stats,
        painter.geometry_bytes()
    );
    assert!(
        median < 0.3,
        "painter GPU time {median} ms over the 0.3 ms budget"
    );
    painter.destroy(&*dev);
    dev.destroy_texture(t.tex);
    dev.destroy_buffer(t.rb);
}
