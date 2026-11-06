//! Dumps one real building (full detail, from the tile cache, offline) to a flat binary file
//! for the `ed_metal` cluster A/B probe. The building is centred on x/z = 0 with its base at y = 0.
//!
//! `cargo run --release -p ed_world --example dump_building -- [lat] [lon] [out.bin]`
//!
//! Format (little endian): "EWB1", u32 vertex count, u32 index count, then positions (3 f32),
//! normals (3 f32), colors (4 f32) per vertex, then indices (u32).

use ed_data::DiskCache;
use ed_geo::{EnuFrame, Geodetic, tile::TileId};
use ed_world::{buildings, tilegen::{self, TileJob}};
use glam::Vec3;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lat: f64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(46.0207);
    let lon: f64 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(7.7491);
    let out = args.get(3).cloned().unwrap_or_else(|| "building.bin".into());

    let cache = DiskCache::default_location();
    let template = ed_data::vector::tile_template(&cache).expect("vector tile template (cache offline?)");
    let tile = TileId::containing(lat, lon, 14);
    let job = TileJob { tile, frame: EnuFrame::new(Geodetic::new(lat, lon, 0.0)), cache, vector_template: Some(template) };
    let built = tilegen::build(&job);
    let Some(ds) = built.building_detail.clone() else {
        panic!("no building detail for tile {:?}: errors {:?}", tile, built.errors);
    };

    let mut chunk_keys: Vec<_> = ds.chunks.keys().copied().collect();
    chunk_keys.sort();
    let key = *chunk_keys.iter().max_by_key(|k| ds.chunks[k].len()).expect("no buildings in tile");
    let specs = &ds.chunks[&key];
    let mut sized: Vec<(usize, usize)> = specs
        .iter()
        .enumerate()
        .map(|(i, s)| (buildings::build_detail(std::iter::once(s), &ds.ctx).triangle_count(), i))
        .collect();
    sized.sort();
    let pick_mode = args.get(4).cloned().unwrap_or_default();
    let (tris, pick) = if pick_mode == "largest" {
        *sized.iter().max().unwrap()
    } else if pick_mode == "smallest" {
        *sized.iter().min().unwrap()
    } else {
        sized[sized.len() / 2]
    };
    println!("tile {:?}: {} buildings in chunk {:?}; picked spec {} (median, {} tris)", tile, specs.len(), key, pick, tris);

    let meshes = buildings::build_detail(std::iter::once(&specs[pick]), &ds.ctx);
    let (mut pos, mut nrm, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for m in meshes.out.meshes.values() {
        let base = pos.len() as u32;
        pos.extend_from_slice(&m.positions);
        nrm.extend_from_slice(&m.normals);
        col.extend_from_slice(&m.colors);
        idx.extend(m.indices.iter().map(|i| i + base));
    }
    assert!(nrm.len() == pos.len() && col.len() == pos.len(), "attribute length mismatch");

    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in &pos {
        lo = lo.min(Vec3::from(*p));
        hi = hi.max(Vec3::from(*p));
    }
    let shift = Vec3::new(-(lo.x + hi.x) * 0.5, -lo.y, -(lo.z + hi.z) * 0.5);
    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("create output"));
    f.write_all(b"EWB1").unwrap();
    f.write_all(&(pos.len() as u32).to_le_bytes()).unwrap();
    f.write_all(&(idx.len() as u32).to_le_bytes()).unwrap();
    for p in &pos {
        for k in 0..3 {
            f.write_all(&(p[k] + shift[k]).to_le_bytes()).unwrap();
        }
    }
    for n in &nrm {
        for k in 0..3 {
            f.write_all(&n[k].to_le_bytes()).unwrap();
        }
    }
    for c in &col {
        for k in 0..4 {
            f.write_all(&c[k].to_le_bytes()).unwrap();
        }
    }
    for i in &idx {
        f.write_all(&i.to_le_bytes()).unwrap();
    }
    f.flush().unwrap();
    println!("wrote {out}: {} verts, {} tris, size {:?} m", pos.len(), idx.len() / 3, hi - lo);
}
