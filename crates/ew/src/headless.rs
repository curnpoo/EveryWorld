//! Headless harness (P1-D15, W7): offscreen rendering with no window (works with the app
//! unfocused or occluded), along a deterministic scripted camera path. Writes `frame.png`
//! (last frame, 8-bit sRGB after tonemap), `perf.csv` and `summary.txt` into `--out`.
//!
//! Time: the camera path advances on virtual time, exactly one fixed step per frame, so the
//! images do not depend on how fast the machine is. Frame times are measured on the wall
//! clock, outside the step.

use crate::args::Args;
use crate::camera::ScriptedPath;
use crate::fixed_step::FixedStep;
use crate::perf::{FrameRow, PerfLog};
use ed_gpu::{Backend, BufferDesc, DeviceDesc, Format, ResourceState, TextureDesc, TextureKind, Usage};
use ed_render::{FrameParams, Renderer, SceneMode, View, ViewTarget, Viewport};
use std::time::Instant;

/// Output format of the offscreen target: the PNG bytes come straight from it.
pub const OUT_FORMAT: Format = Format::Rgba8Srgb;
/// Default frame count when `--frames` is not given.
pub const DEFAULT_FRAMES: u64 = 300;

/// What a run produced (for the caller's log line).
pub struct Report {
    pub summary: String,
    pub frames: u64,
}

pub fn run(args: &Args) -> Result<Report, String> {
    let frames = args.frames.unwrap_or(DEFAULT_FRAMES).max(1);
    let (w, h) = (args.width, args.height);
    let dev = ed_gpu::create_device(Backend::Metal, &DeviceDesc { validation: args.validation, frames_in_flight: args.frames_in_flight })
        .map_err(|e| format!("device: {e}"))?;
    let caps = crate::caps_line(dev.caps());
    let mut renderer = Renderer::new(&*dev).map_err(|e| format!("renderer: {e}"))?;
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
    let readback = dev
        .create_buffer(&BufferDesc {
            label: "headless readback",
            size: pitch as u64 * h as u64,
            usage: Usage { copy_dst: true, cpu_read: true, ..Default::default() },
        })
        .map_err(|e| format!("readback: {e}"))?;

    let mode = if args.empty { SceneMode::Empty } else { SceneMode::TestScene };
    let mut log = PerfLog::new(Renderer::pass_names(mode), frames as usize);
    let mut path = ScriptedPath::new(args.path);
    let mut fixed = FixedStep::default();
    let warmup = (frames as usize / 10).clamp(1, 30);
    let mut alloc_after_warmup = 0;
    let mut last_completed = 0u64;
    let mut prev_start: Option<Instant> = None;
    let mut validation_error = None;

    for i in 0..frames {
        let mut enc = dev.begin_frame().map_err(|e| format!("begin_frame: {e}"))?;
        let t0 = Instant::now();
        let wall_ms = prev_start.map(|p| (t0 - p).as_secs_f64() * 1e3).unwrap_or(0.0);
        prev_start = Some(t0);
        // GPU results of the frame that finished most recently.
        let c1 = dev.completed_frames();
        let t = renderer.timings(&*dev);
        if c1 > last_completed && dev.completed_frames() == c1 {
            if let Some(t) = t {
                log.set_gpu((c1 - 1) as usize, &t);
            }
            last_completed = c1;
        }

        let alpha = fixed.run(fixed.dt(), &mut path, |_| ());
        let view = View {
            camera: path.interpolated(alpha),
            viewport: Viewport { x: 0, y: 0, width: w, height: h },
            target: ViewTarget { texture: target, format: OUT_FORMAT, width: w, height: h, initial: ResourceState::Undefined, final_state: None },
        };
        let last = i + 1 == frames;
        let params = FrameParams { mode, readback: last.then_some(readback), ..Default::default() };
        renderer.render(&*dev, &mut *enc, &view, &params).map_err(|e| format!("render: {e}"))?;
        if let Err(e) = dev.end_frame(enc) {
            validation_error.get_or_insert(format!("frame {i}: {e}"));
        }
        log.rows.push(FrameRow { cpu_ms: t0.elapsed().as_secs_f64() * 1e3, wall_ms, ..Default::default() });
        if i + 1 == warmup as u64 {
            dev.wait_idle();
            alloc_after_warmup = dev.allocated_bytes();
        }
    }
    dev.wait_idle();
    if let Some(t) = renderer.timings(&*dev) {
        log.set_gpu(frames as usize - 1, &t);
    }
    let alloc_end = dev.allocated_bytes();

    // PNG of the last frame.
    let mut raw = Vec::new();
    dev.read_buffer(readback, &mut raw).map_err(|e| format!("read_buffer: {e}"))?;
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as usize {
        let o = y * pitch as usize;
        rgba.extend_from_slice(&raw[o..o + w as usize * 4]);
    }
    std::fs::create_dir_all(&args.out).map_err(|e| format!("{}: {e}", args.out.display()))?;
    write_png(&args.out.join("frame.png"), w, h, &rgba)?;
    std::fs::write(args.out.join("perf.csv"), log.csv()).map_err(|e| format!("perf.csv: {e}"))?;

    let header = format!(
        "ew --headless: {w}x{h}, {frames} frames, mode {}, path {:?}, frames in flight {}, validation {}",
        if args.empty { "empty (clear + tonemap + overlay)" } else { "test scene" },
        args.path,
        args.frames_in_flight,
        args.validation
    );
    let footer = format!(
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

    renderer.destroy(&*dev);
    dev.destroy_texture(target);
    dev.destroy_buffer(readback);
    dev.wait_idle();
    if let Some(e) = validation_error {
        return Err(format!("GPU validation error: {e}"));
    }
    Ok(Report { summary, frames })
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
