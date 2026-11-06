//! OpenStreetMap road tags (lanes, width, sidewalks, cycle lanes, speed limits), baked per z14 tile by
//! `tools/bake_osm_tags.py` into `<cache>/osmtags/v1/14/<x>/<y>.json`. The vector tiles the game draws roads from
//! do not carry these. A tile with no file just has no tags (`EW_OSMTAGS=0` also ignores them).

use crate::DiskCache;
use ed_geo::tile::TileId;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct OsmWay {
    pub id: i64,
    pub highway: String,
    pub tags: HashMap<String, String>,
    /// Polylines in tile units (0..extent, y down), not clipped to the tile.
    pub lines: Vec<Vec<[f32; 2]>>,
}

#[derive(Debug, Clone, Default)]
pub struct OsmTile {
    pub extent: f32,
    pub baked: String,
    pub ways: Vec<OsmWay>,
}

pub fn enabled() -> bool {
    std::env::var("EW_OSMTAGS").map(|v| v != "0").unwrap_or(true)
}

pub fn load(cache: &DiskCache, tile: TileId) -> Option<OsmTile> {
    if !enabled() || tile.z != 14 {
        return None;
    }
    let path = cache.root().join("osmtags").join("v1").join("14").join(tile.x.to_string()).join(format!("{}.json", tile.y));
    parse(&std::fs::read(path).ok()?).ok()
}

pub fn parse(bytes: &[u8]) -> anyhow::Result<OsmTile> {
    let doc: serde_json::Value = serde_json::from_slice(bytes)?;
    let mut ways = Vec::new();
    for w in doc["ways"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let lines: Vec<Vec<[f32; 2]>> = w["lines"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(|l| l.as_array().map(Vec::as_slice).unwrap_or(&[]).iter().filter_map(|p| Some([p[0].as_f64()? as f32, p[1].as_f64()? as f32])).collect::<Vec<_>>())
            .filter(|l| l.len() >= 2)
            .collect();
        if lines.is_empty() {
            continue;
        }
        let tags = w["tags"].as_object().map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect()).unwrap_or_default();
        ways.push(OsmWay { id: w["id"].as_i64().unwrap_or(0), highway: w["highway"].as_str().unwrap_or("").to_string(), tags, lines });
    }
    Ok(OsmTile { extent: doc["extent"].as_f64().unwrap_or(4096.0) as f32, baked: doc["baked"].as_str().unwrap_or("").to_string(), ways })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ways_tags_and_lines() {
        let t = parse(br#"{"version":1,"baked":"2026-11-06","extent":4096,"ways":[
            {"id":5,"highway":"residential","tags":{"lanes":"2","sidewalk":"both"},"lines":[[[0,0],[10,5]]]},
            {"id":6,"highway":"service","tags":{},"lines":[]}]}"#)
        .unwrap();
        assert_eq!(t.ways.len(), 1, "a way with no geometry is dropped");
        assert_eq!(t.ways[0].tags["lanes"], "2");
        assert_eq!(t.ways[0].lines[0][1], [10.0, 5.0]);
        assert_eq!(t.baked, "2026-11-06");
    }
}
