//! G2 check dump (network): the simulated stand over the real Zermatt level-12 canopy pages,
//! written as top-down PNGs (crown discs coloured by species, tallest drawn last).
//!
//! ```text
//! EARTHDROP_CACHE=$PWD/.earthdrop-cache EW_DUMP_DIR=/some/dir \
//!   cargo test -p ed_world --test stand_dump --release -- --ignored --nocapture
//! ```

use ed_data::node::{node_canopy, node_heights, ElevationSource, RealCanopySource};
use ed_data::DiskCache;
use ed_geo::node::{BodyId, NodeId};
use ed_geo::{CanopyPage, HeightPage};
use ed_world::stand::{cell_metres, stand_for_node, CanopySample, StandTree, FACE_LEVEL};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

fn node_at(lat: f64, lon: f64, level: u8) -> NodeId {
    let d = ed_geo::Geodetic::new(lat, lon, 0.0).to_ecef().normalize();
    NodeId::from_unit(BodyId(0), d, level)
}

fn write_rgb(path: &std::path::Path, w: usize, h: usize, px: &[u8]) {
    let f = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(px).unwrap();
}

/// Crown discs of `trees` into an RGB image covering `[x0, x0+w*mpp) x [y0, y0+h*mpp)` metres
/// from the node's lower-left cell corner.
fn render(id: NodeId, trees: &[StandTree], x0: f64, y0: f64, w: usize, h: usize, mpp: f64) -> Vec<u8> {
    let side = 1i64 << (FACE_LEVEL - id.level() as u32);
    let (ci0, cj0) = (id.x() as i64 * side, id.y() as i64 * side);
    let mut img = vec![[28u8, 30, 26]; w * h];
    let mut order: Vec<&StandTree> = trees.iter().collect();
    order.sort_by(|a, b| a.height_m.total_cmp(&b.height_m).then(a.id.cmp(&b.id)));
    for t in order {
        let (mx, my) = cell_metres(t.fx, t.fy);
        let (px, py) = ((t.fx - ci0 as f64) * mx as f64, (t.fy - cj0 as f64) * my as f64);
        let (cx, cy, r) = ((px - x0) / mpp, (py - y0) / mpp, (t.crown_r_m as f64 / mpp).max(0.6));
        if cx < -r || cy < -r || cx > w as f64 + r || cy > h as f64 + r {
            continue;
        }
        let leaf = t.species_def().leaf;
        let shade = 0.75 + 0.25 * (t.height_m / 40.0).min(1.0);
        let col = [
            (((leaf >> 16) & 255) as f32 * shade) as u8,
            (((leaf >> 8) & 255) as f32 * shade) as u8,
            ((leaf & 255) as f32 * shade) as u8,
        ];
        for y in (cy - r).floor().max(0.0) as usize..=((cy + r).ceil().max(0.0) as usize).min(h - 1) {
            for x in (cx - r).floor().max(0.0) as usize..=((cx + r).ceil().max(0.0) as usize).min(w - 1) {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                if dx * dx + dy * dy <= r * r {
                    img[y * w + x] = col;
                }
            }
        }
    }
    img.into_iter().flatten().collect()
}

#[test]
#[ignore = "network"]
fn dump_zermatt_stand() {
    let dir = PathBuf::from(std::env::var("EW_DUMP_DIR").unwrap_or_else(|_| {
        "/private/tmp/claude-501/-Users-curren-Documents-projects-EveryWorld-worktrees-foundation-refactor/030b2c30-1339-4d49-919d-c42d5b59d118/scratchpad/trees/g2/".into()
    }));
    std::fs::create_dir_all(&dir).unwrap();
    let cache = DiskCache::default_location();
    let (hsrc, csrc) = (ElevationSource { cache: &cache }, RealCanopySource { cache: &cache });
    let centre = node_at(46.0200, 7.7560, 12);
    let mut pages: HashMap<NodeId, (HeightPage, CanopyPage)> = HashMap::new();
    for dy in -1..=1 {
        for dx in -1..=1 {
            let id = centre.neighbour(dx, dy);
            let hp = node_heights(&hsrc, id);
            let cp = node_canopy(&csrc, id, &|i, j| hp.height(i, j));
            pages.insert(id, (hp, cp));
        }
    }
    let sample = |d: glam::DVec3| -> Option<CanopySample> {
        let id = NodeId::from_unit(BodyId(0), d, 12);
        let (hp, cp) = pages.get(&id)?;
        let (u, v) = id.local_uv(d);
        Some(CanopySample::from_page(cp, u, v, hp.sample(u, v) as f32, 0.3))
    };
    let t = Instant::now();
    let trees = stand_for_node(centre, &sample, 0xE7E2);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    let side = 1i64 << (FACE_LEVEL - 12);
    let (mx, my) = cell_metres((centre.x() as i64 * side + side / 2) as f64, (centre.y() as i64 * side + side / 2) as f64);
    let (wm, hm) = (side as f64 * mx as f64, side as f64 * my as f64);
    let area_ha = wm * hm / 1e4;
    let mut by_species: std::collections::BTreeMap<&str, usize> = Default::default();
    for t in &trees {
        *by_species.entry(t.species_def().name).or_default() += 1;
    }
    println!("zermatt {centre:?}: {} trees on {:.0} x {:.0} m ({:.0}/ha), stand {ms:.0} ms", trees.len(), wm, hm, trees.len() as f64 / area_ha);
    println!("species: {by_species:?}");
    let (w, h) = ((wm / 2.0) as usize, (hm / 2.0) as usize);
    write_rgb(&dir.join("zermatt_node.png"), w, h, &render(centre, &trees, 0.0, 0.0, w, h, 2.0));
    let (x0, y0) = ((wm - 1000.0) / 2.0, (hm - 1000.0) / 2.0);
    write_rgb(&dir.join("zermatt_1km.png"), 500, 500, &render(centre, &trees, x0, y0, 500, 500, 2.0));
    let (x1, y1) = (x0 + 375.0, y0 + 375.0);
    write_rgb(&dir.join("zermatt_250m.png"), 500, 500, &render(centre, &trees, x1, y1, 500, 500, 0.5));
    println!("wrote PNGs in {}", dir.display());
}
