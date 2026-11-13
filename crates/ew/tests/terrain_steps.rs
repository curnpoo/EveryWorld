//! Measures stair-stepping in the node pages built from the cached Mapterhorn tiles at Zermatt.
//! Needs the disk cache (`EARTHDROP_CACHE`); run with `--ignored --nocapture`.

use ed_data::node::node_heights;
use ed_geo::NodeId;
use ew::terrain::{CacheSource, ZERMATT, geodetic_dir};

struct Stats {
    slope_deg: f64,
    plateau_frac: f64,
    rms2: f64,
    spikes: usize,
    n: usize,
}

fn row_stats(h: &[f64], spacing: f64) -> Stats {
    let n = h.len();
    let d: Vec<f64> = h.windows(2).map(|w| w[1] - w[0]).collect();
    let plateau = d.iter().filter(|x| x.abs() < 1e-9).count() as f64 / d.len() as f64;
    let d2: Vec<f64> = d.windows(2).map(|w| w[1] - w[0]).collect();
    let rms2 = (d2.iter().map(|x| x * x).sum::<f64>() / d2.len() as f64).sqrt();
    let mut a: Vec<f64> = d2.iter().map(|x| x.abs()).collect();
    a.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let med = a[a.len() / 2].max(1e-3);
    let spikes = d2.iter().filter(|x| x.abs() > 4.0 * med && x.abs() > 0.05).count();
    let mean = (h[n - 1] - h[0]) / (spacing * (n - 1) as f64);
    Stats { slope_deg: mean.atan().to_degrees(), plateau_frac: plateau, rms2, spikes, n }
}

#[test]
#[ignore = "needs the elevation disk cache (EARTHDROP_CACHE)"]
fn zermatt_profiles() {
    let root = ew::terrain::cache_root();
    let src = CacheSource::new(root);
    println!("zooms {:?}", src.zooms());
    let dir = geodetic_dir(ZERMATT.lat, ZERMATT.lon);
    for level in [16u8, 17, 18] {
        let c = NodeId::from_unit(ed_geo::BodyId(0), dir, level);
        let mut best: Option<(f64, Stats, NodeId)> = None;
        for dx in -6..=6 {
            for dy in -6..=6 {
                let id = c.neighbour(dx, dy);
                let p = node_heights(&src, id);
                let spacing = 2.4 * 2f64.powi(16 - level as i32);
                let h: Vec<f64> = (0..257).map(|i| p.height(i, 128)).collect();
                let s = row_stats(&h, spacing);
                let score = (s.slope_deg - 10.0).abs();
                if best.as_ref().map_or(true, |b| score < b.0) {
                    best = Some((score, s, id));
                }
            }
        }
        let (_, s, id) = best.unwrap();
        println!(
            "level {level} node {id:?}: slope {:.1} deg, plateau {:.1}%, rms d2 {:.4} m, spikes {} of {}",
            s.slope_deg, s.plateau_frac * 100.0, s.rms2, s.spikes, s.n
        );
    }
}

#[test]
#[ignore = "needs the elevation disk cache (EARTHDROP_CACHE)"]
fn source_roughness() {
    use ed_data::node::SourceView;
    let src = CacheSource::new(ew::terrain::cache_root());
    let t = ed_geo::tile::TileId::containing(ZERMATT.lat, ZERMATT.lon, 14);
    let r = src.elevation_tile(14, t.x, t.y).expect("z14 tile");
    // Median rms of the second difference over rows of 64 samples, raw vs B-spline-smoothed.
    let mut raw = vec![];
    let mut sm = vec![];
    for y in (8..r.size - 8).step_by(8) {
        let row: Vec<f64> = (0..r.size).map(|x| r.heights[y * r.size + x] as f64).collect();
        let slope = (row[r.size - 1] - row[0]).abs() / (6.6 * r.size as f64);
        if slope.atan().to_degrees() > 12.0 || slope.atan().to_degrees() < 4.0 { continue; }
        let d2 = |v: &[f64]| (v.windows(3).map(|w| (w[0] - 2.0 * w[1] + w[2]).powi(2)).sum::<f64>() / (v.len() - 2) as f64).sqrt();
        raw.push(d2(&row));
        let s: Vec<f64> = (1..row.len() - 1).map(|i| (row[i - 1] + 4.0 * row[i] + row[i + 1]) / 6.0).collect();
        sm.push(d2(&s));
    }
    raw.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sm.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("source z14 rows {}: median rms d2 raw {:.3} m, B-spline {:.3} m", raw.len(), raw[raw.len() / 2], sm[sm.len() / 2]);
}
