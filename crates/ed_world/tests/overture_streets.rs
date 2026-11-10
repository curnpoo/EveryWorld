//! W4e spike: run the roads v2 network builder and height solver on Overture transportation
//! segments, next to the vector-tile input, for one city block of tiles.
//!
//! `python spikes/streets/extract.py 35.6595 139.7005 /tmp/shibuya.json`
//! `EW_OVERTURE_JSON=/tmp/shibuya.json cargo test -p ed_world --release --test overture_streets -- --nocapture`

use ed_data::cache::DiskCache;
use ed_data::mvt::{self, Feature, GeomType, Layer, Value};
use ed_geo::tile::TileId;
use ed_world::roads::{ground, net::RoadNet, solve};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

fn stats(name: &str, net: &RoadNet, gfn: &ground::GroundFn, wm: f64) {
    let t0 = std::time::Instant::now();
    let solved = solve::solve_with(net, &**gfn, None);
    let dt = t0.elapsed();
    let len: f64 = net.edges.iter().map(|e| e.pts.windows(2).map(|w| ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt()).sum::<f64>()).sum();
    // An endpoint is "dangling" if no other edge has a vertex within 0.5 m of it. Endpoints
    // outside the centre tile are ignored (the block's outer rim is cut by construction).
    let cell = |p: [f64; 2]| ((p[0] / 0.5).round() as i64, (p[1] / 0.5).round() as i64);
    let mut seen: HashMap<(i64, i64), HashSet<usize>> = HashMap::new();
    for (i, e) in net.edges.iter().enumerate() {
        for p in &e.pts {
            seen.entry(cell(*p)).or_default().insert(i);
        }
    }
    let (mut ends, mut dangling) = (0, 0);
    for (i, e) in net.edges.iter().enumerate() {
        for p in [e.pts[0], *e.pts.last().unwrap()] {
            if p[0].abs() > wm * 0.5 || p[1].abs() > wm * 0.5 {
                continue;
            }
            ends += 1;
            let c = cell(p);
            let joined = (-1..=1).any(|dx| (-1..=1).any(|dy| seen.get(&(c.0 + dx, c.1 + dy)).is_some_and(|s| s.iter().any(|j| *j != i))));
            dangling += !joined as usize;
        }
    }
    let st = &solved.stats;
    eprintln!(
        "{name:12} edges {:6}  length {:7.1} km  bridges {:4}  tunnels {:4}  crossings {:5}  clearance deficits {:4}  endpoints in centre tile {:5} dangling {:4}  solve {:?}",
        net.edges.len(), len / 1000.0, st.bridges, st.tunnels, st.crossings, st.deficits.len(), ends, dangling, dt
    );
}

#[test]
fn overture_through_roads_v2() {
    let Some(path) = std::env::var_os("EW_OVERTURE_JSON") else {
        eprintln!("skipped: set EW_OVERTURE_JSON (see the file header)");
        return;
    };
    ed_data::http::set_offline(true);
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let (z, tx, ty) = (v["tile"][0].as_u64().unwrap() as u8, v["tile"][1].as_u64().unwrap() as u32, v["tile"][2].as_u64().unwrap() as u32);
    let tile = TileId { z, x: tx, y: ty };
    let wm = tile.width_m();
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let cache = DiskCache::new(fixtures.join("cache"));
    let gfn = ground::load(&cache, tile, wm);

    // Vector-tile input: the recorded 3x3 block.
    let tpl = std::fs::read_to_string(fixtures.join("vector_template.txt")).unwrap();
    let mut vt = Vec::new();
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            if let Some(n) = tile.offset(dx as i64, dy as i64)
                && let Ok(t) = ed_data::vector::fetch(&cache, tpl.trim(), n)
            {
                vt.push((dx, dy, t));
            }
        }
    }
    let block: Vec<(i32, i32, &mvt::Tile)> = vt.iter().map(|(a, b, c)| (*a, *b, c)).collect();
    eprintln!("vector tiles found in fixtures: {}", block.len());
    stats("vector tiles", &RoadNet::from_tiles(tile, wm, &block), &gfn, wm);

    // Overture input: every segment goes, unclipped, into each tile of the block it touches,
    // in that tile's units. `from_tiles` clips and stitches exactly as it does for vector tiles.
    let n = (1u64 << z) as f64;
    let ext = 4096.0;
    let mut tiles: HashMap<(i32, i32), Vec<Feature>> = HashMap::new();
    for f in v["features"].as_array().unwrap() {
        let pts: Vec<(f64, f64)> = f["ll"].as_array().unwrap().iter().map(|p| {
            let (lon, lat) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
            ((lon + 180.0) / 360.0 * n, (1.0 - lat.to_radians().tan().asinh() / std::f64::consts::PI) / 2.0 * n)
        }).collect();
        let mut props = HashMap::new();
        props.insert("class".to_string(), Value::String(f["class"].as_str().unwrap_or("").to_string()));
        if f["bridge"].as_bool() == Some(true) {
            props.insert("brunnel".into(), Value::String("bridge".into()));
        } else if f["tunnel"].as_bool() == Some(true) {
            props.insert("brunnel".into(), Value::String("tunnel".into()));
        }
        if let Some(l) = f["level"].as_i64() {
            props.insert("layer".into(), Value::Int(l));
        }
        if f["link"].as_bool() == Some(true) {
            props.insert("ramp".into(), Value::Int(1));
        }
        if matches!(f["surface"].as_str(), Some("unpaved" | "gravel" | "dirt")) {
            props.insert("surface".into(), Value::String("unpaved".into()));
        }
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                let (ox, oy) = (tx as f64 + dx as f64, ty as f64 + dy as f64);
                if !pts.iter().any(|p| p.0 >= ox - 0.05 && p.0 <= ox + 1.05 && p.1 >= oy - 0.05 && p.1 <= oy + 1.05) {
                    continue;
                }
                let ring = pts.iter().map(|p| [((p.0 - ox) * ext) as f32, ((p.1 - oy) * ext) as f32]).collect();
                tiles.entry((dx, dy)).or_default().push(Feature { id: None, geom_type: GeomType::LineString, props: props.clone(), rings: vec![ring] });
            }
        }
    }
    let ov: Vec<(i32, i32, mvt::Tile)> = tiles.into_iter().map(|((dx, dy), features)| (dx, dy, mvt::Tile { layers: vec![Layer { name: "transportation".into(), extent: 4096, features }] })).collect();
    let block: Vec<(i32, i32, &mvt::Tile)> = ov.iter().map(|(a, b, c)| (*a, *b, c)).collect();
    stats("overture", &RoadNet::from_tiles(tile, wm, &block), &gfn, wm);
}
