//! "Drop me in Zermatt": place name -> coordinates via Open-Meteo's geocoder.

use crate::DiskCache;
use ed_geo::Geodetic;

#[derive(Debug, Clone)]
pub struct Place {
    pub name: String,
    pub country: String,
    pub admin1: String,
    pub position: Geodetic,
    pub population: u64,
}

pub fn search(cache: &DiskCache, query: &str) -> anyhow::Result<Vec<Place>> {
    let q: String = query
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    let url = format!("https://geocoding-api.open-meteo.com/v1/search?name={q}&count=5&format=json");
    let key = format!("geocode/{}.json", q.to_lowercase());
    let bytes = cache.get_or_fetch(&key, None, || crate::http::get_bytes(&url))?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    let results = v["results"].as_array().cloned().unwrap_or_default();
    Ok(results
        .iter()
        .map(|r| Place {
            name: r["name"].as_str().unwrap_or_default().to_owned(),
            country: r["country"].as_str().unwrap_or_default().to_owned(),
            admin1: r["admin1"].as_str().unwrap_or_default().to_owned(),
            position: Geodetic::new(
                r["latitude"].as_f64().unwrap_or(0.0),
                r["longitude"].as_f64().unwrap_or(0.0),
                r["elevation"].as_f64().unwrap_or(0.0),
            ),
            population: r["population"].as_u64().unwrap_or(0),
        })
        .collect())
}
