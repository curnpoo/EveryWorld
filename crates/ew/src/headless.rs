//! Headless harness (P1-D15, W7): offscreen rendering with no window (works with the app
//! unfocused or occluded), along a deterministic scripted camera path. Writes `frame.png`
//! (last frame, 8-bit sRGB after tonemap), `perf.csv` and `summary.txt` into `--out`; on the
//! proving path also `street.png`, `rooftop.png` and `sightline.png`.
//!
//! Time: the camera path advances on virtual time, exactly one fixed step per frame, so the
//! images do not depend on how fast the machine is. Frame times are measured on the wall
//! clock, outside the step.
//!
//! Frame order: `flush_uploads` (instance/material/provider uploads, lands at the next
//! `begin_frame`) -> `begin_frame` (pacing wait, not counted) -> record + submit. "CPU frame
//! build" is flush + record + submit.

use crate::args::{Args, SceneKind};
use crate::camera::{PathKind, ScriptedPath};
use crate::fixed_step::FixedStep;
use crate::perf::{FrameRow, PerfLog, Stats};
use ed_gpu::{Backend, BufferDesc, BufferId, DeviceDesc, Format, ResourceState, TextureDesc, TextureKind, Usage};
use ed_render::{FrameParams, PickResult, PopFrame, Renderer, SceneMode, TestScene, View, ViewTarget, Viewport};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

/// Output format of the offscreen target: the PNG bytes come straight from it.
pub const OUT_FORMAT: Format = Format::Rgba8Srgb;
/// Default frame count when `--frames` is not given.
pub const DEFAULT_FRAMES: u64 = 300;
/// Pick the centre pixel every this many frames (shows picking in the summary).
const PICK_EVERY: u64 = 60;

/// What a run produced.
pub struct Report {
    pub summary: String,
    pub frames: u64,
    pub alloc_after_warmup: u64,
    pub alloc_end: u64,
    /// The last frame's PNG.
    pub frame_png: PathBuf,
    /// Extra screenshots (name, path).
    pub shots: Vec<(&'static str, PathBuf)>,
    /// Renderer instances the scene created.
    pub instances: usize,
}

pub fn run(args: &Args) -> Result<Report, String> {
    let frames = args.frames.unwrap_or(DEFAULT_FRAMES).max(1);
    let (w, h) = (args.width, args.height);
    let dev = ed_gpu::create_device(Backend::Metal, &DeviceDesc { validation: args.validation, frames_in_flight: args.frames_in_flight })
        .map_err(|e| format!("device: {e}"))?;
    let caps = crate::caps_line(dev.caps());
    let mut renderer = Renderer::new(&*dev).map_err(|e| format!("renderer: {e}"))?;
    args.apply_lod(renderer.lod_config());
    let scene = args.scene();
    let mut scene_lines = String::new();
    let mut content: Option<crate::content::Content> = None;
    let mut instance_count = 0usize;
    if !args.empty {
        match scene {
            SceneKind::Test => {
                instance_count = TestScene::load(&mut renderer, &*dev).map_err(|e| format!("test scene: {e}"))?.len();
                scene_lines.push_str("scene: test (6 instances)\n");
            }
            SceneKind::Proving => {
                let c = crate::content::Content::load(&mut renderer, &*dev, &args.packs)?;
                let _ = writeln!(
                    scene_lines,
                    "scene: proving ground from packs, {} placements -> {} instances ({} skinned), {} meshes, {} materials, {} textures ({:.1} MiB), {} missing; loaded in {:.0} ms",
                    c.placement_count(),
                    c.instance_count(),
                    c.skinned_parts(),
                    c.mesh_count(),
                    c.material_count(),
                    c.texture_count(),
                    c.stats.texture_bytes as f64 / (1 << 20) as f64,
                    c.stats.missing,
                    c.load_ms
                );
                scene_lines.push_str(&c.describe());
                for n in &c.notes {
                    eprintln!("ew: {n}");
                }
                instance_count = c.instance_count();
                content = Some(c);
            }
        }
    }
    let target = dev
        .create_texture(&TextureDesc {
            label: "headless output",
            kind: TextureKind::D2,
            format: OUT_FORMAT,
            width: w,
            height: h,
            depth_or_layers: 1,
            mips: 1,
            samples: 1,
            usage: Usage { render_target: true, copy_src: true, ..Default::default() },
        })
        .map_err(|e| format!("target: {e}"))?;
    let pitch = OUT_FORMAT.copy_row_pitch(w);
    let readback_buf = |label| {
        dev.create_buffer(&BufferDesc { label, size: pitch as u64 * h as u64, usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() } })
            .map_err(|e| format!("readback: {e}"))
    };
    let readback = readback_buf("headless readback")?;
    // Screenshots along the proving path: (name, frame, buffer).
    let mut shots: Vec<(&'static str, u64, BufferId)> = Vec::new();
    if args.path == PathKind::Proving && !args.empty {
        for (name, t) in [("street", crate::proving::STREET_SHOT_S), ("rooftop", crate::proving::ROOFTOP_SHOT_S), ("sightline", crate::proving::SIGHTLINE_SHOT_S)] {
            let f = (t * 60.0 / args.speed.max(1e-6)).round() as u64;
            if f + 1 < frames {
                shots.push((name, f, readback_buf(name)?));
            }
        }
    }

    let mode = if args.empty { SceneMode::Empty } else { SceneMode::Scene };
    let all_passes = renderer.pass_names(&FrameParams { mode, pop_stats: args.pop, pick: Some((0, 0)), ..Default::default() });
    let mut log = PerfLog::new(&all_passes, frames as usize);
    let mut path = ScriptedPath::with_speed(args.path, args.speed);
    let mut fixed = FixedStep::default();
    let warmup = (frames as usize / 10).clamp(1, 30);
    let mut alloc_after_warmup = 0;
    let mut last_completed = 0u64;
    let mut prev_start: Option<Instant> = None;
    let mut validation_error = None;
    let mut first_frame_index = None;
    let mut pops: Vec<PopFrame> = Vec::new();
    let mut last_pick: Option<PickResult> = None;
    let mut upload_bytes = 0u64;

    for i in 0..frames {
        if let Some(c) = &content {
            // Simulation time of this frame's pose: the fixed step count (deterministic).
            c.animate(&mut renderer, (path.step + 1) as f64 / 60.0)?;
        }
        let tf = Instant::now();
        let up = renderer.flush_uploads(&*dev).map_err(|e| format!("flush_uploads: {e}"))?;
        upload_bytes += up.instance_bytes + up.material_bytes;
        let flush_ms = tf.elapsed().as_secs_f64() * 1e3;
        let mut enc = dev.begin_frame().map_err(|e| format!("begin_frame: {e}"))?;
        let t0 = Instant::now();
        let first = *first_frame_index.get_or_insert(enc.frame_index());
        let wall_ms = prev_start.map(|p| (t0 - p).as_secs_f64() * 1e3).unwrap_or(0.0);
        prev_start = Some(t0);
        // GPU results of the frame that finished most recently.
        let c1 = dev.completed_frames();
        let t = renderer.timings(&*dev);
        if c1 > last_completed && dev.completed_frames() == c1 {
            if let Some(t) = t {
                log.set_gpu(c1.saturating_sub(1 + first) as usize, &t);
            }
            last_completed = c1;
        }

        let alpha = fixed.run(fixed.dt(), &mut path, |_| ());
        let camera = match args.camera {
            Some(p) => crate::camera::Pose { position: crate::proving::ORIGIN + glam::DVec3::new(p[0], p[1], p[2]), yaw: p[3] as f32, pitch: p[4] as f32 }.camera(),
            None => path.interpolated(alpha),
        };
        let view = View {
            camera,
            viewport: Viewport { x: 0, y: 0, width: w, height: h },
            target: ViewTarget { texture: target, format: OUT_FORMAT, width: w, height: h, initial: ResourceState::Undefined, final_state: None },
        };
        let shot = shots.iter().find(|s| s.1 == i).map(|s| s.2);
        let last = i + 1 == frames;
        let params = FrameParams {
            mode,
            readback: if last { Some(readback) } else { shot },
            pick: (!args.empty && i % PICK_EVERY == PICK_EVERY - 1).then_some((w / 2, h / 2)),
            pop_stats: args.pop && !args.empty,
            ..Default::default()
        };
        renderer.render(&*dev, &mut *enc, &view, &params).map_err(|e| format!("render: {e}"))?;
        if let Err(e) = dev.end_frame(enc) {
            validation_error.get_or_insert(format!("frame {i}: {e}"));
        }
        log.rows.push(FrameRow {
            cpu_ms: flush_ms + t0.elapsed().as_secs_f64() * 1e3,
            wall_ms,
            flush_ms,
            draw_loop_ms: renderer.draw_loop_ms(),
            ..Default::default()
        });
        collect(&mut renderer, &mut log, first, &mut pops, &mut last_pick);
        if i + 1 == warmup as u64 {
            dev.wait_idle();
            alloc_after_warmup = dev.allocated_bytes();
        }
    }
    dev.wait_idle();
    if let Some(t) = renderer.timings(&*dev) {
        log.set_gpu(frames as usize - 1, &t);
    }
    renderer.poll(&*dev);
    collect(&mut renderer, &mut log, first_frame_index.unwrap_or(0), &mut pops, &mut last_pick);
    let alloc_end = dev.allocated_bytes();

    // PNGs.
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    let save = |buf: BufferId, name: &str| -> Result<PathBuf, String> {
        let mut raw = Vec::new();
        dev.read_buffer(buf, &mut raw).map_err(|e| format!("read_buffer: {e}"))?;
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h as usize {
            let o = y * pitch as usize;
            rgba.extend_from_slice(&raw[o..o + w as usize * 4]);
        }
        let p = args.out.join(name);
        write_png(&p, w, h, &rgba)?;
        Ok(p)
    };
    let frame_png = save(readback, "frame.png")?;
    let mut shot_paths = Vec::new();
    for (name, _, buf) in &shots {
        shot_paths.push((*name, save(*buf, &format!("{name}.png"))?));
    }
    std::fs::write(args.out.join("perf.csv"), log.csv()).map_err(|e| format!("perf.csv: {e}"))?;

    let header = format!(
        "ew --headless: {w}x{h}, {frames} frames, mode {}, path {:?} at speed {}, frames in flight {}, validation {}",
        if args.empty {
            "empty (clear + tonemap + overlay)"
        } else if scene == SceneKind::Proving {
            "proving ground"
        } else {
            "test scene"
        },
        args.path,
        args.speed,
        args.frames_in_flight,
        args.validation
    );
    let mut footer = String::new();
    if !args.empty {
        let first = first_frame_index.unwrap_or(0);
        footer.push_str(&m2_lines(&M2Inputs { args, r: &renderer, log: &log, warmup, pops: &pops, first, pick: last_pick, upload_bytes, scene: &scene_lines }));
    }
    let _ = write!(
        footer,
        "allocated_bytes: {alloc_after_warmup} after warm-up, {alloc_end} at end ({:+} B)\ntransient textures created: {}\n{caps}\n{}\n",
        alloc_end as i64 - alloc_after_warmup as i64,
        renderer.transient_textures_created(),
        match &validation_error {
            Some(e) => format!("validation: FIRST ERROR {e}"),
            None => "validation: no errors".to_string(),
        }
    );
    let summary = log.summary(warmup, &header, &footer);
    std::fs::write(args.out.join("summary.txt"), &summary).map_err(|e| format!("summary.txt: {e}"))?;

    if let Some(mut c) = content {
        c.clear(&mut renderer);
    }
    renderer.destroy(&*dev);
    dev.destroy_texture(target);
    dev.destroy_buffer(readback);
    for (_, _, b) in shots {
        dev.destroy_buffer(b);
    }
    dev.wait_idle();
    if let Some(e) = validation_error {
        return Err(format!("GPU validation error: {e}"));
    }
    Ok(Report { summary, frames, alloc_after_warmup, alloc_end, frame_png, shots: shot_paths, instances: instance_count })
}

/// Moves finished frames' counters, pop tiles and pick answers out of the renderer.
fn collect(r: &mut Renderer, log: &mut PerfLog, first: u64, pops: &mut Vec<PopFrame>, pick: &mut Option<PickResult>) {
    for c in r.take_cull_history() {
        if let Some(row) = c.frame.checked_sub(first).and_then(|k| log.rows.get_mut(k as usize)) {
            row.cull = Some(c);
        }
    }
    pops.extend(r.take_pop_frames());
    if let Some(p) = r.pick_result() {
        *pick = Some(p);
    }
}

struct M2Inputs<'a> {
    args: &'a Args,
    r: &'a Renderer,
    log: &'a PerfLog,
    warmup: usize,
    pops: &'a [PopFrame],
    /// Device frame index of the run's first frame.
    first: u64,
    pick: Option<PickResult>,
    upload_bytes: u64,
    scene: &'a str,
}

/// The M2 summary lines: scene, instance counts, draw calls, CPU build, culling cost,
/// budgets, picking, pop detector.
fn m2_lines(m: &M2Inputs<'_>) -> String {
    let (log, r) = (m.log, m.r);
    let rows = log.rows.get(m.warmup..).unwrap_or(&[]);
    let mut s = String::from(m.scene);
    let lod = r.lod_provider();
    let mut cfg = ed_render::provider::LodConfig::default();
    m.args.apply_lod(&mut cfg);
    let _ = writeln!(
        s,
        "lod: threshold {} px projected error, hysteresis x{}, cross-fade {} frames, occlusion {}",
        cfg.lod_px,
        cfg.hysteresis,
        cfg.fade_frames,
        if cfg.occlusion { "two-phase HZB" } else { "off" }
    );
    let culls: Vec<_> = rows.iter().filter_map(|r| r.cull).collect();
    let mm = |f: &dyn Fn(&ed_render::provider::CullStats) -> u32| {
        let v: Vec<u32> = culls.iter().map(f).collect();
        let mean = v.iter().map(|&x| x as f64).sum::<f64>() / v.len().max(1) as f64;
        format!("mean {:.0}, min {}, max {}", mean, v.iter().min().copied().unwrap_or(0), v.iter().max().copied().unwrap_or(0))
    };
    let _ = writeln!(s, "instances per frame (GPU counters, {} frames after warm-up):", culls.len());
    let _ = writeln!(s, "  total:           {}", mm(&|c| c.total));
    let _ = writeln!(s, "  after frustum:   {}", mm(&|c| c.after_frustum));
    let _ = writeln!(s, "  after occlusion: {}", mm(&|c| c.after_occlusion));
    let _ = writeln!(s, "  drawn:           {}", mm(&|c| c.drawn()));
    let _ = writeln!(s, "    phase early:   {}", mm(&|c| c.drawn_early));
    let _ = writeln!(s, "    phase late:    {}", mm(&|c| c.drawn_late));
    let _ = writeln!(s, "  in cross-fade:   {}", mm(&|c| c.fading));
    let _ = writeln!(s, "  LOD changes:     {}", mm(&|c| c.changed));
    let _ = writeln!(s, "  triangles:       {}", mm(&|c| c.triangles));
    let _ = writeln!(s, "  non-empty bins:  {}", mm(&|c| c.nonempty_bins));
    let _ = writeln!(
        s,
        "draw calls per frame: {} ({} indirect = {} mesh x LOD bins x solid/dither x early/late; + resolve + tonemap)",
        r.draw_calls(),
        lod.draw_calls(),
        lod.bins()
    );
    let line = |s: &mut String, name: &str, st: Option<Stats>| {
        let _ = match st {
            Some(st) => writeln!(s, "{name}: {st}"),
            None => writeln!(s, "{name}: no samples"),
        };
    };
    line(&mut s, "cpu frame build (flush + record + submit)", Stats::of(rows.iter().map(|r| r.cpu_ms)));
    line(&mut s, "  flush_uploads", Stats::of(rows.iter().map(|r| r.flush_ms)));
    line(&mut s, "  multi-draw CPU loops", Stats::of(rows.iter().map(|r| r.draw_loop_ms)));
    let _ = writeln!(s, "uploads: {} B instance + material data over the run (initial load included)", m.upload_bytes);
    // Culling GPU cost: the cull compute passes plus the depth pyramid.
    let cull_passes: Vec<usize> = ["cull_early", "hzb", "cull_late"].iter().filter_map(|n| log.passes.iter().position(|p| p == n)).collect();
    let cull_ms = Stats::of(rows.iter().filter_map(|row| {
        let v: Vec<f32> = cull_passes.iter().filter_map(|&k| row.pass_ms.get(k).copied().flatten()).collect();
        (v.len() == cull_passes.len()).then(|| v.iter().map(|&x| x as f64).sum())
    }));
    line(&mut s, "culling GPU (cull_early + hzb + cull_late)", cull_ms);
    let gpu = Stats::of(rows.iter().filter_map(|r| r.gpu_ms.map(f64::from)));
    let _ = writeln!(
        s,
        "frames over 25 ms after warm-up: gpu {}, wall {}",
        rows.iter().filter_map(|r| r.gpu_ms).filter(|&x| x > 25.0).count(),
        rows.iter().filter(|r| r.wall_ms > 25.0).count()
    );
    let verdict = |ok: bool| if ok { "met" } else { "MISSED" };
    if let Some(g) = gpu {
        let _ = writeln!(s, "budget 60 fps (gpu p99 <= 16.7 ms): {} ({:.2} ms)", verdict(g.p99 <= 16.7), g.p99);
    }
    if let Some(c) = cull_ms {
        let _ = writeln!(s, "budget culling < 1 ms: {} (mean {:.3} ms, p99 {:.3} ms)", verdict(c.p99 < 1.0), c.mean, c.p99);
    }
    if let Some(c) = Stats::of(rows.iter().map(|r| r.cpu_ms)) {
        let _ = writeln!(s, "budget cpu build < 1 ms: {} (mean {:.3} ms, p99 {:.3} ms)", verdict(c.p99 < 1.0), c.mean, c.p99);
    }
    if let Some(p) = m.pick {
        let _ = writeln!(
            s,
            "pick (centre pixel, frame {}): instance {:?}, object {}, lod {}",
            p.frame,
            p.instance,
            p.object.map(|o| format!("{:#x}", o.0)).unwrap_or_else(|| "none (sky)".into()),
            p.lod
        );
    }
    let _ = writeln!(s, "lost readbacks: {}", lod.lost_readbacks());
    if m.args.pop {
        match crate::perf::pop_stats(m.pops, m.first + m.warmup as u64) {
            Some(p) => {
                let _ = writeln!(
                    s,
                    "pop detector ({} frame pairs, {px}x{px} px tiles, |dL| of mean display luminance 0..1):\n  transition tiles: {} samples, max {:.4}, p99 {:.4}\n  other tiles:      {} samples, max {:.4}, p99 {:.4}",
                    p.pairs,
                    p.transition_tiles,
                    p.transition_max,
                    p.transition_p99,
                    p.other_tiles,
                    p.other_max,
                    p.other_p99,
                    px = ed_render::POP_TILE
                );
            }
            None => s.push_str("pop detector: no consecutive frame pairs read back\n"),
        }
    }
    s
}

fn write_png(path: &std::path::Path, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
    let mut wr = enc.write_header().map_err(|e| format!("png: {e}"))?;
    wr.write_image_data(rgba).map_err(|e| format!("png: {e}"))?;
    wr.finish().map_err(|e| format!("png: {e}"))
}
