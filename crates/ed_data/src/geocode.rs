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

/// Reverse geocode a point with OpenStreetMap Nominatim. Returns the nearest
/// settlement plus country, so a map click can name where it landed.
pub fn reverse(cache: &DiskCache, g: Geodetic) -> anyhow::Result<Option<Place>> {
    let lat = g.lat_deg;
    let lon = g.lon_deg;
    let url = format!(
        "https://nominatim.openstreetmap.org/reverse?lat={lat:.6}&lon={lon:.6}&format=jsonv2&addressdetails=1&zoom=14&accept-language=en"
    );
    let key = format!("geocode/reverse/{lat:.4}_{lon:.4}.json");
    let bytes = cache.get_or_fetch(&key, Some(std::time::Duration::from_secs(86_400 * 30)), || {
        crate::http::get_bytes(&url)
    })?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    let addr = &v["address"];
    let pick = |k: &str| addr[k].as_str().filter(|s| !s.is_empty());
    let name = pick("city")
        .or_else(|| pick("town"))
        .or_else(|| pick("village"))
        .or_else(|| pick("municipality"))
        .or_else(|| pick("hamlet"))
        .or_else(|| pick("suburb"))
        .or_else(|| pick("county"))
        .or_else(|| v["name"].as_str())
        .or_else(|| v["display_name"].as_str().and_then(|s| s.split(',').next()))
        .unwrap_or_default()
        .trim()
        .to_string();
    let country = pick("country").unwrap_or_default().trim().to_string();
    if name.is_empty() && country.is_empty() {
        return Ok(None);
    }
    Ok(Some(Place {
        name,
        country,
        admin1: pick("state").unwrap_or_default().to_string(),
        position: g,
        population: 0,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hits Nominatim. `cargo test -p ed_data geocode::tests::reverse -- --ignored`
    #[test]
    #[ignore]
    fn reverse_names_zermatt() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let p = reverse(&cache, Geodetic::new(46.0207, 7.7491, 0.0)).unwrap().expect("place");
        assert_eq!(p.name, "Zermatt");
        assert_eq!(p.country, "Switzerland");
    }
}
