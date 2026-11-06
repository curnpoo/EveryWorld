//! Dumps a tile's building LOD geometry (what the renderer actually gets for a city block) to
//! a flat binary file, so the LOD decimation can be inspected and rendered offline.
//!
//! `cargo run --release -p ed_world --example dump_tile_lod -- [lat] [lon] [out.bin]`
//!
//! Format (little endian): "EWL1", u32 vertex count, u32 index count, then positions (3 f32),
//! normals (3 f32), colours (4 f32) per vertex, then indices (u32).

use ed_data::DiskCache;
use ed_geo::{EnuFrame, Geodetic, tile::TileId};
use ed_world::tilegen::{self, TileJob};
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lat: f64 = args.get(1).map(|s| s.parse().unwrap()).unwrap_or(40.7690);
    let lon: f64 = args.get(2).map(|s| s.parse().unwrap()).unwrap_or(-73.9750);
    let out = args.get(3).cloned().unwrap_or_else(|| "tile_lod.bin".into());

    let cache = DiskCache::default_location();
    let template = ed_data::vector::tile_template(&cache).expect("vector tile template (cache offline?)");
    let tile = TileId::containing(lat, lon, 14);
    let job = TileJob { tile, frame: EnuFrame::new(Geodetic::new(lat, lon, 0.0)), cache, vector_template: Some(template) };
    let built = tilegen::build(&job);

    // One material at a time: this is how the renderer sees a tile (a merged mesh per material),
    // and the per-material mesh has a different cluster grid than the all-material merge.
    let only = std::env::var("EPL_MAT").ok();
    let (mut pos, mut nrm, mut col, mut idx) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut chunks = 0;
    for (_, b) in &built.building_lod {
        chunks += 1;
        for (mat, m) in b.out.meshes.iter() {
            if let Some(want) = &only {
                if format!("{mat:?}") != *want {
                    continue;
                }
            }
            let base = pos.len() as u32;
            pos.extend_from_slice(&m.positions);
            nrm.extend_from_slice(&m.normals);
            col.extend_from_slice(&m.colors);
            idx.extend(m.indices.iter().map(|i| i + base));
        }
    }
    assert_eq!(nrm.len(), pos.len());
    assert_eq!(col.len(), pos.len());
    println!("tile {:?}: {} chunks, {} verts, {} tris", tile, chunks, pos.len(), idx.len() / 3);

    let mut f = std::io::BufWriter::new(std::fs::File::create(&out).expect("create output"));
    f.write_all(b"EWL1").unwrap();
    f.write_all(&(pos.len() as u32).to_le_bytes()).unwrap();
    f.write_all(&(idx.len() as u32).to_le_bytes()).unwrap();
    for p in &pos {
        for k in 0..3 {
            f.write_all(&p[k].to_le_bytes()).unwrap();
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
    println!("wrote {out}");
}
