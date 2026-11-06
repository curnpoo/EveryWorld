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

/// One forward-geocoder hit, most relevant first. Nominatim's free-form search
/// understands anything the user can type: a country, a state or region, a
/// town, a street address, a POI.
#[derive(Debug, Clone)]
pub struct SearchHit {
    /// Short name ("Switzerland", "Zermatt").
    pub name: String,
    /// Region detail shown underneath ("Bavaria, Germany").
    pub region: String,
    pub position: Geodetic,
    /// Angular size of the result in degrees, from its bounding box. Bigger is
    /// further out; the map uses it to pick a zoom that frames the place.
    pub span_deg: f64,
}

/// Free-form forward geocode ("Switzerland", "Bavaria", "108 Ninth Ave, NYC")
/// via OSM Nominatim. Complements [`search`], which is city names only.
pub fn forward(cache: &DiskCache, query: &str) -> anyhow::Result<Vec<SearchHit>> {
    let q: String = query
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            b' ' => "+".into(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    let url = format!("https://nominatim.openstreetmap.org/search?q={q}&format=jsonv2&limit=5&addressdetails=1&accept-language=en");
    // Cache for a month: the geocoder's answer for a fixed query is stable.
    // v2 cache key: `accept-language=en` changed the answer quality.
    let key = format!("geocode/fwd2/{}.json", q.to_lowercase());
    let bytes = cache.get_or_fetch(&key, Some(std::time::Duration::from_secs(86_400 * 30)), || {
        crate::http::get_bytes(&url)
    })?;
    let v: serde_json::Value = serde_json::from_slice(&bytes)?;
    let empty = Vec::new();
    let results = v.as_array().unwrap_or(&empty);
    let hits = results
        .iter()
        .filter_map(|r| {
            let lat = r["lat"].as_str()?.parse::<f64>().ok()?;
            let lon = r["lon"].as_str()?.parse::<f64>().ok()?;
            let display = r["display_name"].as_str().unwrap_or_default().to_string();
            if display.is_empty() {
                return None;
            }
            let first_seg = display.split(", ").next().unwrap_or_default().to_owned();
            let name = r["name"].as_str().filter(|s| !s.is_empty() && !s.contains(" / ")).map(str::to_owned)
                .unwrap_or(first_seg);
            // Region detail: the two display segments after the name.
            let region: Vec<&str> = display.split(", ").skip(1).take(2).collect();
            // Bounds are [south, north, west, east], as strings in jsonv2.
            let bbox: Option<Vec<f64>> = r["boundingbox"].as_array().and_then(|a| {
                a.iter().take(4).map(|x| x.as_str()?.parse::<f64>().ok()).collect()
            });
            let span = bbox.map_or(0.15, |b| {
                let (s, n, w, e) = (b[0], b[1], b[2], b[3]);
                let lat_span = n - s;
                let lon_span = (e - w) * (s + n).mul_add(0.5, 90.0).to_radians().cos();
                (lat_span.max(lon_span.abs())).max(0.02)
            });
            Some(SearchHit {
                name: name.trim().to_string(),
                region: region.join(", "),
                position: Geodetic::new(lat, lon, 0.0),
                span_deg: span,
            })
        })
        .collect();
    Ok(hits)
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

    /// Hits Nominatim. `cargo test -p ed_data geocode::tests::reverse_names_zermatt -- --ignored`
    #[test]
    #[ignore]
    fn reverse_names_zermatt() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let p = reverse(&cache, Geodetic::new(46.0207, 7.7491, 0.0)).unwrap().expect("place");
        assert_eq!(p.name, "Zermatt");
        assert_eq!(p.country, "Switzerland");
    }

    /// Hits Nominatim. Also covers whatever the user may type: state, address.
    /// `cargo test -p ed_data geocode::tests::forward_finds_countries_and_addresses -- --ignored`
    #[test]
    #[ignore]
    fn forward_finds_countries_and_addresses() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let ch = forward(&cache, "Switzerland").unwrap();
        assert!(
            ch.first().is_some_and(|h| h.name.contains("Switzer")),
            "got {ch:?}"
        );
        assert!(ch[0].span_deg > 1.0, "a country should be big: {ch:?}");
        let addr = forward(&cache, "1600 Pennsylvania Avenue, Washington DC").unwrap();
        assert!(
            addr.first().is_some_and(|h| h.position.lat_deg > 38.0 && h.position.lat_deg < 39.5),
            "got {addr:?}"
        );
        assert!(addr[0].span_deg < 0.05, "an address should be small: {addr:?}");
    }
}
