//! Overture Maps buildings, baked offline (`tools/bake_overture.py`) into one JSON file per z14 tile and read
//! here as a vector-tile `building` layer, so the building generator needs no second code path.
//!
//! Overture's buildings are OSM's wherever OSM has them, plus machine-learned footprints and heights where OSM
//! has gaps. The release is pinned ([`RELEASE`]) so the world stays a pure function of (seed, data version).
//! No file for a tile (not baked, or `EW_OVERTURE=0`) means the OpenFreeMap buildings are used instead.

use crate::{DiskCache, mvt};
use ed_geo::tile::TileId;
use std::collections::HashMap;

pub const RELEASE: &str = "2026-10-21.0";

pub fn enabled() -> bool {
    std::env::var("EW_OVERTURE").map(|v| v != "0").unwrap_or(true)
}

/// A `building`-only vector tile for `tile` (z14), if one was baked.
pub fn building_tile(cache: &DiskCache, tile: TileId) -> Option<mvt::Tile> {
    if tile.z != 14 || !enabled() {
        return None;
    }
    let path = cache.root().join("overture").join(RELEASE).join(tile.z.to_string()).join(tile.x.to_string()).join(format!("{}.json", tile.y));
    let bytes = std::fs::read(path).ok()?;
    parse(&bytes).ok()
}

/// Facade colours arrive as "#rrggbb" or sometimes as names; only the hex form is used.
fn hex(s: &str) -> Option<String> {
    (s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())).then(|| s.to_ascii_lowercase())
}

pub fn parse(bytes: &[u8]) -> anyhow::Result<mvt::Tile> {
    let doc: serde_json::Value = serde_json::from_slice(bytes)?;
    let extent = doc["extent"].as_u64().unwrap_or(4096) as u32;
    let mut features = Vec::new();
    for b in doc["buildings"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let mut rings: Vec<Vec<[f32; 2]>> = Vec::new();
        for (i, r) in b["rings"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().enumerate() {
            let mut ring: Vec<[f32; 2]> = r
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
                .iter()
                .filter_map(|p| Some([p[0].as_f64()? as f32, p[1].as_f64()? as f32]))
                .collect();
            // GeoJSON repeats the first point at the end.
            if ring.len() > 1 && ring.first() == ring.last() {
                ring.pop();
            }
            if ring.len() < 3 {
                continue;
            }
            // Exterior rings positive, holes negative (the vector tile convention `mvt::polygons` expects).
            let area = mvt::ring_area(&ring);
            if (i == 0 && area < 0.0) || (i > 0 && area > 0.0) {
                ring.reverse();
            }
            rings.push(ring);
        }
        if rings.is_empty() {
            continue;
        }
        let mut props: HashMap<String, mvt::Value> = HashMap::new();
        let height = b["height"].as_f64().or_else(|| b["floors"].as_f64().map(|f| f * 3.2));
        if let Some(h) = height.filter(|h| *h > 0.0) {
            props.insert("render_height".into(), mvt::Value::Double(h));
        }
        props.insert("render_min_height".into(), mvt::Value::Double(b["min_height"].as_f64().unwrap_or(0.0)));
        if let Some(c) = b["facade_color"].as_str().and_then(hex) {
            props.insert("colour".into(), mvt::Value::String(c));
        }
        if b["has_parts"].as_bool().unwrap_or(false) {
            props.insert("hide_3d".into(), mvt::Value::Bool(true));
        }
        if let Some(r) = b["roof_shape"].as_str() {
            props.insert("roof:shape".into(), mvt::Value::String(r.into()));
        }
        if let Some(c) = b["class"].as_str() {
            props.insert("class".into(), mvt::Value::String(c.into()));
        }
        props.insert("ml".into(), mvt::Value::Bool(b["ml"].as_bool().unwrap_or(false)));
        features.push(mvt::Feature { id: None, geom_type: mvt::GeomType::Polygon, props, rings });
    }
    Ok(mvt::Tile { layers: vec![mvt::Layer { name: "building".into(), extent, features }] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_heights_parts_and_ring_orientation() {
        let json = br##"{"version":1,"release":"x","extent":4096,"buildings":[
            {"id":"a","rings":[[[0,0],[0,100],[100,100],[100,0],[0,0]],[[20,20],[40,20],[40,40],[20,40]]],"height":12.5,"min_height":null,"floors":null,"facade_color":"#AABBCC","ml":true,"part":false,"has_parts":false},
            {"id":"b","rings":[[[200,0],[300,0],[300,100],[200,100]]],"height":null,"floors":5,"min_height":3,"part":true,"has_parts":true,"roof_shape":"gabled"}]}"##;
        let t = parse(json).unwrap();
        let l = t.layer("building").unwrap();
        assert_eq!(l.features.len(), 2);
        let a = &l.features[0];
        assert_eq!(a.num("render_height"), Some(12.5));
        assert_eq!(a.str("colour"), Some("#aabbcc"));
        assert!(mvt::ring_area(&a.rings[0]) > 0.0 && mvt::ring_area(&a.rings[1]) < 0.0);
        assert_eq!(mvt::polygons(a).len(), 1);
        assert_eq!(mvt::polygons(a)[0].1.len(), 1, "the hole stays attached");
        let b = &l.features[1];
        assert!((b.num("render_height").unwrap() - 16.0).abs() < 1e-6, "5 floors");
        assert_eq!(b.num("render_min_height"), Some(3.0));
        assert!(matches!(b.props.get("hide_3d"), Some(mvt::Value::Bool(true))));
    }
}
