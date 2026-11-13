//! G1 check dump (network): real canopy pages for a 3x3 block of level-12 nodes around Zermatt
//! and Blue Ridge, written as greyscale PNG mosaics (cover and height) for eyeballing.
//!
//! ```text
//! EARTHDROP_CACHE=$PWD/.earthdrop-cache EW_DUMP_DIR=/some/dir \
//!   cargo test -p ed_data --test canopy_dump --release -- --ignored --nocapture
//! ```

use ed_data::node::{node_canopy, node_heights, ElevationSource, RealCanopySource};
use ed_data::DiskCache;
use ed_geo::node::{BodyId, NodeId};
use ed_geo::{decode_height, PAGE_N};
use std::path::PathBuf;
use std::time::Instant;

fn node_at(lat: f64, lon: f64, level: u8) -> NodeId {
    let d = ed_geo::Geodetic::new(lat, lon, 0.0).to_ecef().normalize();
    NodeId::from_unit(BodyId(0), d, level)
}

fn write_png(path: &std::path::Path, w: usize, h: usize, px: &[u8]) {
    let f = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Grayscale);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(px).unwrap();
}

#[test]
#[ignore = "network"]
fn dump_canopy_mosaics() {
    let dir = PathBuf::from(std::env::var("EW_DUMP_DIR").unwrap_or_else(|_| {
        "/private/tmp/claude-501/-Users-curren-Documents-projects-EveryWorld-worktrees-foundation-refactor/030b2c30-1339-4d49-919d-c42d5b59d118/scratchpad/trees/g1/".into()
    }));
    std::fs::create_dir_all(&dir).unwrap();
    let cache = DiskCache::default_location();
    let (hsrc, csrc) = (ElevationSource { cache: &cache }, RealCanopySource { cache: &cache });
    for (name, lat, lon) in [("zermatt", 46.0200, 7.7560), ("blueridge", 37.95, -78.95)] {
        let centre = node_at(lat, lon, 12);
        let side = 3 * PAGE_N;
        let (mut cover, mut height) = (vec![0u8; side * side], vec![0u8; side * side]);
        for (gy, dy) in (-1..=1).enumerate() {
            for (gx, dx) in (-1..=1).enumerate() {
                let id = centre.neighbour(dx, dy);
                let t = Instant::now();
                let hp = node_heights(&hsrc, id);
                let t_h = t.elapsed();
                let t = Instant::now();
                let page = node_canopy(&csrc, id, &|i, j| hp.height(i, j));
                let ms = t.elapsed().as_secs_f64() * 1e3;
                let treed = page.samples.iter().filter(|s| s[0] > 128).count();
                let hmax = page.samples.iter().map(|s| decode_height(s[1])).fold(0.0f32, f32::max);
                println!("{name} {id:?} heights {:.0} ms, canopy {ms:.0} ms, {} bytes, treed {:.1}%, max height {hmax:.1} m", t_h.as_secs_f64() * 1e3, page.to_bytes().len(), 100.0 * treed as f64 / page.samples.len() as f64);
                for j in 0..PAGE_N {
                    for i in 0..PAGE_N {
                        let s = page.get(i, j);
                        let k = (gy * PAGE_N + j) * side + gx * PAGE_N + i;
                        cover[k] = s[0];
                        height[k] = (decode_height(s[1]) / 90.0 * 255.0).round().min(255.0) as u8;
                    }
                }
            }
        }
        write_png(&dir.join(format!("{name}_cover.png")), side, side, &cover);
        write_png(&dir.join(format!("{name}_height.png")), side, side, &height);
        println!("wrote {name}_cover.png and {name}_height.png in {}", dir.display());
    }
}
