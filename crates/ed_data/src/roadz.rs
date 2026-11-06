//! Measured road heights: bridge decks, tunnel floors and ground roads with a surveyed Z.
//!
//! Two national road databases publish per-vertex heights, free:
//! * France, IGN BD TOPO `troncon_de_route` (Licence Ouverte 2.0) from the Geoplateforme WFS: Z per vertex
//!   (photogrammetry, 1.5-2.5 m), `position_par_rapport_au_sol` (> 0 bridge, < 0 tunnel), lane count and width.
//! * Norway, NVDB `veglenkesekvenser/segmentert` (NLOD 2.0): LINESTRING Z, 10-15 cm; no bridge flag, but the
//!   heights are the road's own.
//! Everything is fetched per z14 tile, normalised to [`Seg`] and cached as JSON under `roadz/v1/<source>/14/x/y.json`
//! (an empty answer is cached too). `EW_ROADZ=0` ignores the source. Elsewhere there is no source and the solver
//! infers heights as before.

use crate::{DiskCache, http};
use ed_geo::tile::TileId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Ign,
    Nvdb,
}

impl Source {
    fn dir(self) -> &'static str {
        match self {
            Source::Ign => "ign",
            Source::Nvdb => "nvdb",
        }
    }
    /// The source reports bridge / tunnel position, so matching can require it.
    pub fn flags_levels(self) -> bool {
        self == Source::Ign
    }
}

pub fn enabled() -> bool {
    std::env::var("EW_ROADZ").map(|v| v != "0").unwrap_or(true)
}

/// Which database covers this tile (by its centre; metropolitan France and mainland Norway).
pub fn source_for(tile: TileId) -> Option<Source> {
    if !enabled() || tile.z != 14 {
        return None;
    }
    let c = tile.center();
    let (lat, lon) = (c.lat_deg, c.lon_deg);
    if (41.3..=51.2).contains(&lat) && (-5.2..=9.7).contains(&lon) {
        Some(Source::Ign)
    } else if (57.9..=71.3).contains(&lat) && (4.0..=31.3).contains(&lon) {
        Some(Source::Nvdb)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Seg {
    /// > 0 bridge, < 0 tunnel, 0 ground (always 0 for NVDB, which does not say).
    pub level: i8,
    pub lanes: Option<u8>,
    pub width: Option<f32>,
    /// (lon, lat, z metres).
    pub pts: Vec<[f64; 3]>,
}

fn bounds(tile: TileId) -> (f64, f64, f64, f64) {
    let (nw, se) = (tile.uv_to_geodetic(0.0, 0.0), tile.uv_to_geodetic(1.0, 1.0));
    // A margin so a road starting just outside still arrives whole.
    let (dlat, dlon) = ((nw.lat_deg - se.lat_deg) * 0.02, (se.lon_deg - nw.lon_deg) * 0.02);
    (se.lat_deg - dlat, nw.lon_deg - dlon, nw.lat_deg + dlat, se.lon_deg + dlon)
}

/// Segments of one z14 tile, from the cache or the network.
pub fn fetch(cache: &DiskCache, source: Source, tile: TileId) -> anyhow::Result<Vec<Seg>> {
    let key = format!("roadz/v1/{}/14/{}/{}.json", source.dir(), tile.x, tile.y);
    let bytes = cache.get_or_fetch(&key, None, || {
        let segs = match source {
            Source::Ign => fetch_ign(tile)?,
            Source::Nvdb => fetch_nvdb(tile)?,
        };
        Ok(serde_json::to_vec(&segs)?)
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn prefetch(cache: &DiskCache, source: Source, tile: TileId) {
    let _ = fetch(cache, source, tile);
}

fn fetch_ign(tile: TileId) -> anyhow::Result<Vec<Seg>> {
    let (s, w, n, e) = bounds(tile);
    let mut out = Vec::new();
    for page in 0..30 {
        let url = format!(
            "https://data.geopf.fr/wfs/ows?service=WFS&version=2.0.0&request=GetFeature&typeNames=BDTOPO_V3:troncon_de_route&outputFormat=application/json&srsName=EPSG:4326&bbox={s},{w},{n},{e},urn:ogc:def:crs:EPSG::4326&propertyName=geometrie,position_par_rapport_au_sol,nombre_de_voies,largeur_de_chaussee&sortBy=cleabs&count=1000&startIndex={}",
            page * 1000
        );
        let bytes = http::get_bytes(&url)?;
        let got = parse_ign(&bytes)?;
        let n_feat = got.1;
        out.extend(got.0);
        if n_feat < 1000 {
            break;
        }
    }
    Ok(out)
}

/// (segments with a usable Z, number of features in the page).
pub fn parse_ign(bytes: &[u8]) -> anyhow::Result<(Vec<Seg>, usize)> {
    let doc: serde_json::Value = serde_json::from_slice(bytes)?;
    let feats = doc["features"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut out = Vec::new();
    for f in feats {
        let p = &f["properties"];
        let level = match &p["position_par_rapport_au_sol"] {
            serde_json::Value::String(s) => s.parse::<i32>().unwrap_or(0),
            v => v.as_i64().unwrap_or(0) as i32,
        }
        .clamp(-4, 4) as i8;
        let lines: Vec<&Vec<serde_json::Value>> = match f["geometry"]["type"].as_str() {
            Some("LineString") => f["geometry"]["coordinates"].as_array().into_iter().collect(),
            Some("MultiLineString") => f["geometry"]["coordinates"].as_array().into_iter().flatten().filter_map(|l| l.as_array()).collect(),
            _ => Vec::new(),
        };
        for line in lines {
            // -1000 is "no height": split the line there.
            let mut run: Vec<[f64; 3]> = Vec::new();
            let flush = |run: &mut Vec<[f64; 3]>, out: &mut Vec<Seg>| {
                if run.len() >= 2 {
                    out.push(Seg { level, lanes: p["nombre_de_voies"].as_u64().filter(|n| (1..=12).contains(n)).map(|n| n as u8), width: p["largeur_de_chaussee"].as_f64().filter(|w| *w >= 2.0 && *w <= 40.0).map(|w| w as f32), pts: std::mem::take(run) });
                } else {
                    run.clear();
                }
            };
            for c in line {
                let (lon, lat, z) = (c[0].as_f64(), c[1].as_f64(), c[2].as_f64());
                match (lon, lat, z) {
                    (Some(lon), Some(lat), Some(z)) if z > -999.0 => run.push([lon, lat, z]),
                    _ => flush(&mut run, &mut out),
                }
            }
            flush(&mut run, &mut out);
        }
    }
    Ok((out, feats.len()))
}

fn fetch_nvdb(tile: TileId) -> anyhow::Result<Vec<Seg>> {
    let (s, w, n, e) = bounds(tile);
    let mut url = Some(format!("https://nvdbapiles.atlas.vegvesen.no/vegnett/api/v4/veglenkesekvenser/segmentert?kartutsnitt={w},{s},{e},{n}&srid=wgs84&antall=1000"));
    let mut out = Vec::new();
    for _ in 0..40 {
        let Some(u) = url.take() else { break };
        let bytes = http::get_bytes_with(&u, &[("X-Client", "EveryWorld"), ("Accept", "application/json")])?;
        let (segs, next) = parse_nvdb(&bytes)?;
        out.extend(segs);
        url = next;
    }
    Ok(out)
}

/// NVDB's WKT is `LINESTRING Z (lat lon z, ...)` in WGS84.
pub fn parse_nvdb(bytes: &[u8]) -> anyhow::Result<(Vec<Seg>, Option<String>)> {
    let doc: serde_json::Value = serde_json::from_slice(bytes)?;
    let mut out = Vec::new();
    for o in doc["objekter"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let Some(wkt) = o["geometri"]["wkt"].as_str() else { continue };
        let Some(body) = wkt.split_once('(').map(|x| x.1.trim_end_matches(')')) else { continue };
        let pts: Vec<[f64; 3]> = body
            .split(',')
            .filter_map(|p| {
                let v: Vec<f64> = p.split_whitespace().filter_map(|x| x.parse().ok()).collect();
                (v.len() == 3).then(|| [v[1], v[0], v[2]])
            })
            .collect();
        if pts.len() >= 2 {
            out.push(Seg { level: 0, lanes: None, width: None, pts });
        }
    }
    let next = doc["metadata"]["neste"]["href"].as_str().map(String::from);
    Ok((out, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ign_heights_levels_lanes_and_gaps() {
        let json = br#"{"features":[
            {"geometry":{"type":"LineString","coordinates":[[2.35,48.85,36.7],[2.36,48.85,37.1],[2.37,48.85,-1000],[2.38,48.85,38.0],[2.39,48.85,38.2]]},
             "properties":{"position_par_rapport_au_sol":"1","nombre_de_voies":2,"largeur_de_chaussee":7.5}},
            {"geometry":{"type":"LineString","coordinates":[[2.35,48.86,-1000],[2.36,48.86,-1000]]},"properties":{"position_par_rapport_au_sol":"0"}}]}"#;
        let (segs, n) = parse_ign(json).unwrap();
        assert_eq!(n, 2);
        assert_eq!(segs.len(), 2, "the -1000 vertex splits the first line; the second has no heights at all");
        assert_eq!(segs[0].level, 1);
        assert_eq!((segs[0].lanes, segs[0].width), (Some(2), Some(7.5)));
        assert_eq!(segs[0].pts.len(), 2);
        assert_eq!(segs[1].pts[0], [2.38, 48.85, 38.0]);
    }

    #[test]
    fn nvdb_wkt_is_lat_lon_z() {
        let json = br#"{"objekter":[{"geometri":{"wkt":"LINESTRING Z (59.9066 10.7410 -14.87,59.9067 10.7409 -15.15)"}}],"metadata":{"neste":{"href":"https://x/next"}}}"#;
        let (segs, next) = parse_nvdb(json).unwrap();
        assert_eq!(segs[0].pts[0], [10.7410, 59.9066, -14.87]);
        assert_eq!(next.as_deref(), Some("https://x/next"));
    }

    /// Live: `cargo test -p ed_data roadz_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn roadz_live() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-roadz-live"));
        let _ = std::fs::remove_dir_all(cache.root());
        for (src, lat, lon) in [(Source::Nvdb, 59.9085, 10.7480), (Source::Ign, 48.8566, 2.3412)] {
            let tile = TileId::containing(lat, lon, 14);
            match fetch(&cache, src, tile) {
                Ok(v) => eprintln!("{src:?}: {} segments, {} bridges, {} tunnels", v.len(), v.iter().filter(|s| s.level > 0).count(), v.iter().filter(|s| s.level < 0).count()),
                Err(e) => eprintln!("{src:?}: ERROR {e:#}"),
            }
        }
    }

    #[test]
    fn sources_by_place() {
        assert_eq!(source_for(TileId::containing(48.86, 2.35, 14)), Some(Source::Ign));
        assert_eq!(source_for(TileId::containing(59.91, 10.75, 14)), Some(Source::Nvdb));
        assert_eq!(source_for(TileId::containing(40.7, -74.0, 14)), None);
        assert_eq!(source_for(TileId::containing(48.86, 2.35, 13)), None);
    }
}
