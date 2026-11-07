//! OpenMapTiles-schema vector tiles from OpenFreeMap (free, no API key).
//!
//! One z14 tile (~2.4 km at the equator) carries everything we need to
//! build the world: `building` (with render_height/min_height/colour),
//! `transportation` (roads, rail, paths with class/surface/bridge flags),
//! `water`, `waterway`, `landcover` (wood/grass/farmland/ice/rock/sand/wetland),
//! `landuse` (residential/industrial/commercial/...), `park`, `aerodrome_label`,
//! `poi` and `place`. Schema: https://openmaptiles.org/schema/

use crate::{DiskCache, mvt};
use ed_geo::tile::TileId;
use std::time::Duration;

/// OpenMapTiles' highest native zoom. Higher zooms overzoom from here.
pub const MAX_ZOOM: u8 = 14;
const TILEJSON: &str = "https://tiles.openfreemap.org/planet";

/// Resolve the current tile URL template from the TileJSON (it embeds the
/// planet build date, so tiles under one template are immutable).
pub fn tile_template(cache: &DiskCache) -> anyhow::Result<String> {
    let bytes = cache.get_or_fetch("openfreemap/tilejson.json", Some(Duration::from_secs(86_400 * 7)), || {
        crate::http::get_bytes(TILEJSON)
    })?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    v["tiles"][0]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("tilejson has no tiles[]"))
}

/// Fetch and decode a vector tile. `id.z` must be <= [`MAX_ZOOM`].
pub fn fetch(cache: &DiskCache, template: &str, id: TileId) -> anyhow::Result<mvt::Tile> {
    let bytes = fetch_bytes(cache, template, id)?;
    if bytes.is_empty() {
        return Ok(mvt::Tile::default());
    }
    mvt::Tile::decode(&bytes)
}

/// Download `id` into the cache without decoding it (for IO threads).
pub fn prefetch(cache: &DiskCache, template: &str, id: TileId) -> anyhow::Result<()> {
    fetch_bytes(cache, template, id).map(|_| ())
}

fn fetch_bytes(cache: &DiskCache, template: &str, id: TileId) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(id.z <= MAX_ZOOM, "vector tiles stop at z{MAX_ZOOM}");
    // Key by template so a new planet build doesn't serve stale tiles.
    let build = template.split('/').rev().nth(3).unwrap_or("planet");
    let key = format!("openfreemap/{}/{}/{}.pbf", crate::DATA_VERSION, build, id.path());
    let url = template
        .replace("{z}", &id.z.to_string())
        .replace("{x}", &id.x.to_string())
        .replace("{y}", &id.y.to_string());
    cache.get_or_fetch(&key, None, || Ok(crate::http::get_bytes_opt(&url)?.unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits the network. `cargo test -p ed_data -- --ignored`
    #[test]
    #[ignore]
    fn manhattan_has_tall_buildings() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let tpl = tile_template(&cache).unwrap();
        let id = TileId::containing(40.7484, -73.9857, 14); // Empire State
        let t = fetch(&cache, &tpl, id).unwrap();
        let names: Vec<_> = t.layers.iter().map(|l| l.name.as_str()).collect();
        eprintln!("layers: {names:?}");
        let b = t.layer("building").expect("building layer");
        let tallest = b.features.iter().filter_map(|f| f.num("render_height")).fold(0.0, f64::max);
        eprintln!("{} buildings, tallest {tallest} m", b.features.len());
        assert!(tallest > 200.0);
    }
}
