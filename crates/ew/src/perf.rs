//! Harness output: per-frame rows (`perf.csv`) and a summary (`summary.txt`).

use std::fmt::Write as _;

/// One frame. GPU numbers arrive a few frames late and are filled in when they do; a frame
/// whose results were overtaken by a later frame's (two frames retiring at once) keeps `None`.
#[derive(Clone, Debug, Default)]
pub struct FrameRow {
    /// CPU time to build and submit the frame, excluding the `begin_frame` pacing wait.
    pub cpu_ms: f64,
    /// Wall time since the previous frame started.
    pub wall_ms: f64,
    pub gpu_ms: Option<f32>,
    /// GPU ms per pass, in the order of [`PerfLog::passes`].
    pub pass_ms: Vec<Option<f32>>,
    /// Part of `cpu_ms` spent in `Renderer::flush_uploads`.
    pub flush_ms: f64,
    /// Part of `cpu_ms` spent in the provider's multi-draw loops.
    pub draw_loop_ms: f64,
    /// GPU culling counters (arrive late, like the timings).
    pub cull: Option<ed_render::provider::CullStats>,
}

pub struct PerfLog {
    pub passes: Vec<&'static str>,
    pub rows: Vec<FrameRow>,
}

/// Mean, median and 99th percentile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub n: usize,
    pub mean: f64,
    pub p50: f64,
    pub p99: f64,
    pub max: f64,
}

impl Stats {
    /// Nearest-rank percentiles.
    pub fn of(values: impl IntoIterator<Item = f64>) -> Option<Self> {
        let mut v: Vec<f64> = values.into_iter().filter(|x| x.is_finite()).collect();
        if v.is_empty() {
            return None;
        }
        v.sort_by(f64::total_cmp);
        let rank = |p: f64| v[((p * v.len() as f64).ceil() as usize).clamp(1, v.len()) - 1];
        Some(Self { n: v.len(), mean: v.iter().sum::<f64>() / v.len() as f64, p50: rank(0.50), p99: rank(0.99), max: v[v.len() - 1] })
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mean {:.3} ms, p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms (n={})", self.mean, self.p50, self.p99, self.max, self.n)
    }
}

impl PerfLog {
    pub fn new(passes: &[&'static str], frames: usize) -> Self {
        Self { passes: passes.to_vec(), rows: Vec::with_capacity(frames) }
    }

    /// Records GPU timings that belong to frame `frame`.
    pub fn set_gpu(&mut self, frame: usize, t: &ed_render::graph::FrameTimings) {
        if let Some(r) = self.rows.get_mut(frame) {
            r.gpu_ms = Some(t.frame_ms);
            r.pass_ms = self.passes.iter().map(|p| t.pass_ms(p)).collect();
        }
    }

    pub fn csv(&self) -> String {
        let mut s = String::from("frame,cpu_frame_ms,gpu_frame_ms");
        for p in &self.passes {
            let _ = write!(s, ",{p}_gpu_ms");
        }
        s.push_str(",wall_frame_ms,flush_ms,draw_loop_ms,instances,after_frustum,after_occlusion,drawn,triangles,fading\n");
        let opt = |v: Option<f32>| v.map(|x| format!("{x:.4}")).unwrap_or_default();
        for (i, r) in self.rows.iter().enumerate() {
            let _ = write!(s, "{i},{:.4},{}", r.cpu_ms, opt(r.gpu_ms));
            for k in 0..self.passes.len() {
                let _ = write!(s, ",{}", opt(r.pass_ms.get(k).copied().flatten()));
            }
            let _ = write!(s, ",{:.4},{:.4},{:.4}", r.wall_ms, r.flush_ms, r.draw_loop_ms);
            match &r.cull {
                Some(c) => {
                    let _ = writeln!(s, ",{},{},{},{},{},{}", c.total, c.after_frustum, c.after_occlusion, c.drawn(), c.triangles, c.fading);
                }
                None => s.push_str(",,,,,,\n"),
            }
        }
        s
    }

    /// Summary over frames `warmup..`.
    pub fn summary(&self, warmup: usize, header: &str, footer: &str) -> String {
        let rows = self.rows.get(warmup..).unwrap_or(&[]);
        let mut s = String::new();
        let _ = writeln!(s, "{header}");
        let _ = writeln!(s, "frames: {} total, {} after {warmup} warm-up frames", self.rows.len(), rows.len());
        let line = |s: &mut String, name: &str, st: Option<Stats>| {
            let _ = match st {
                Some(st) => writeln!(s, "{name}: {st}"),
                None => writeln!(s, "{name}: no samples"),
            };
        };
        line(&mut s, "gpu_frame", Stats::of(rows.iter().filter_map(|r| r.gpu_ms.map(f64::from))));
        line(&mut s, "cpu_frame", Stats::of(rows.iter().map(|r| r.cpu_ms)));
        line(&mut s, "wall_frame", Stats::of(rows.iter().map(|r| r.wall_ms)));
        let _ = writeln!(s, "per-pass GPU means:");
        for (k, p) in self.passes.iter().enumerate() {
            match Stats::of(rows.iter().filter_map(|r| r.pass_ms.get(k).copied().flatten().map(f64::from))) {
                Some(st) => {
                    let _ = writeln!(s, "  {p}: mean {:.3} ms, p99 {:.3} ms (n={})", st.mean, st.p99, st.n);
                }
                None => {
                    let _ = writeln!(s, "  {p}: no samples");
                }
            }
        }
        s.push_str(footer);
        s
    }
}

/// Pop detector result: per-tile change of mean display luminance between consecutive
/// frames, split into tiles that had pixels of an instance in a LOD transition (in either
/// frame) and all other tiles. Camera motion moves both; a LOD pop shows as a transition-tile
/// tail well above the other tiles'.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PopStats {
    pub pairs: usize,
    pub transition_tiles: u64,
    pub other_tiles: u64,
    pub transition_max: f32,
    pub transition_p99: f32,
    pub other_max: f32,
    pub other_p99: f32,
}

/// Histogram resolution of the p99 (|dL| in 0..1).
const POP_BINS: usize = 4096;

/// Pairs frames `f` and `f + 1` (both at or after `min_frame`). Max is exact, p99 from a
/// 4096-bin histogram (resolution 1/4096).
pub fn pop_stats(frames: &[ed_render::PopFrame], min_frame: u64) -> Option<PopStats> {
    let mut hist = [vec![0u64; POP_BINS], vec![0u64; POP_BINS]];
    let mut st = PopStats::default();
    for w in frames.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if a.frame < min_frame || b.frame != a.frame + 1 || a.luma.len() != b.luma.len() {
            continue;
        }
        st.pairs += 1;
        for i in 0..a.luma.len() {
            let d = (b.luma[i] - a.luma[i]).abs();
            if !d.is_finite() {
                continue;
            }
            let t = (a.transition_px[i] > 0.0 || b.transition_px[i] > 0.0) as usize;
            hist[t][((d * POP_BINS as f32) as usize).min(POP_BINS - 1)] += 1;
            if t == 1 {
                st.transition_tiles += 1;
                st.transition_max = st.transition_max.max(d);
            } else {
                st.other_tiles += 1;
                st.other_max = st.other_max.max(d);
            }
        }
    }
    if st.pairs == 0 {
        return None;
    }
    let p99 = |h: &[u64], n: u64, max: f32| {
        if n == 0 {
            return 0.0;
        }
        let rank = (0.99 * n as f64).ceil() as u64;
        let mut acc = 0;
        for (k, c) in h.iter().enumerate() {
            acc += c;
            if acc >= rank {
                return ((k + 1) as f32 / POP_BINS as f32).min(max);
            }
        }
        max
    };
    st.transition_p99 = p99(&hist[1], st.transition_tiles, st.transition_max);
    st.other_p99 = p99(&hist[0], st.other_tiles, st.other_max);
    Some(st)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_stats_split_transition_tiles() {
        let f = |frame, luma: Vec<f32>, tr: Vec<f32>| ed_render::PopFrame { frame, tiles_x: 2, tiles_y: 1, luma, transition_px: tr };
        let frames = vec![f(0, vec![0.5, 0.5], vec![0.0, 3.0]), f(1, vec![0.51, 0.7], vec![0.0, 0.0]), f(3, vec![0.0, 0.0], vec![0.0, 0.0])];
        let s = pop_stats(&frames, 0).unwrap();
        assert_eq!((s.pairs, s.transition_tiles, s.other_tiles), (1, 1, 1));
        assert!((s.transition_max - 0.2).abs() < 1e-6 && (s.other_max - 0.01).abs() < 1e-6);
        assert!(s.transition_p99 <= s.transition_max && s.transition_p99 > 0.19);
        assert!(pop_stats(&frames, 1).is_none(), "frames 1 and 3 are not consecutive");
    }

    #[test]
    fn percentiles_nearest_rank() {
        let s = Stats::of((1..=100).map(f64::from)).unwrap();
        assert_eq!((s.p50, s.p99, s.max), (50.0, 99.0, 100.0));
        assert!((s.mean - 50.5).abs() < 1e-12);
        assert_eq!(Stats::of([3.0]).unwrap().p99, 3.0);
        assert!(Stats::of([]).is_none());
    }

    #[test]
    fn csv_has_one_column_per_pass() {
        let mut log = PerfLog::new(&["sky", "scene"], 2);
        log.rows.push(FrameRow { cpu_ms: 1.0, wall_ms: 2.0, ..Default::default() });
        log.rows.push(FrameRow { cpu_ms: 1.0, wall_ms: 2.0, ..Default::default() });
        let t = ed_render::graph::FrameTimings {
            frame_ms: 0.5,
            passes: vec![ed_render::graph::PassTiming { name: "scene", ms: 0.25 }],
        };
        log.set_gpu(1, &t);
        let csv = log.csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "frame,cpu_frame_ms,gpu_frame_ms,sky_gpu_ms,scene_gpu_ms,wall_frame_ms,flush_ms,draw_loop_ms,instances,after_frustum,after_occlusion,drawn,triangles,fading"
        );
        assert_eq!(lines[1], "0,1.0000,,,,2.0000,0.0000,0.0000,,,,,,");
        assert_eq!(lines[2], "1,1.0000,0.5000,,0.2500,2.0000,0.0000,0.0000,,,,,,");
    }
}
