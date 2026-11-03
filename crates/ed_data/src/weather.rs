//! Live weather from Open-Meteo (free, no key). We pull "current" conditions
//! plus a 48 h hourly forecast so the sky can timelapse forward and storms
//! can be seen rolling in.

use crate::DiskCache;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Weather {
    pub temperature_c: f32,
    pub humidity_pct: f32,
    /// mm in the last hour.
    pub precipitation_mm: f32,
    pub rain_mm: f32,
    pub snowfall_cm: f32,
    /// 0..100
    pub cloud_cover_pct: f32,
    pub wind_speed_ms: f32,
    /// Meteorological: direction the wind blows *from*, degrees.
    pub wind_direction_deg: f32,
    pub wind_gusts_ms: f32,
    pub pressure_hpa: f32,
    /// WMO weather code (0 clear .. 99 thunderstorm with hail).
    pub weather_code: u16,
    pub hourly: Vec<HourlySample>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HourlySample {
    pub unix_s: i64,
    pub temperature_c: f32,
    pub precipitation_mm: f32,
    pub cloud_cover_pct: f32,
    pub wind_speed_ms: f32,
    pub wind_direction_deg: f32,
    pub weather_code: u16,
}

impl Weather {
    pub fn is_thunderstorm(&self) -> bool {
        self.weather_code >= 95
    }
    pub fn is_snowing(&self) -> bool {
        self.snowfall_cm > 0.0 || matches!(self.weather_code, 71..=77 | 85 | 86)
    }
    pub fn is_raining(&self) -> bool {
        !self.is_snowing() && (self.rain_mm > 0.0 || matches!(self.weather_code, 51..=67 | 80..=82 | 95..=99))
    }
    /// Rough visibility in meters derived from conditions (fog codes 45/48).
    pub fn visibility_m(&self) -> f32 {
        match self.weather_code {
            45 | 48 => 400.0,
            _ if self.is_snowing() => 2_000.0,
            _ if self.precipitation_mm > 4.0 => 3_000.0,
            _ if self.is_raining() => 8_000.0,
            _ => 40_000.0 - self.humidity_pct * 200.0,
        }
    }
    pub fn describe(&self) -> &'static str {
        match self.weather_code {
            0 => "Clear",
            1 => "Mostly clear",
            2 => "Partly cloudy",
            3 => "Overcast",
            45 | 48 => "Fog",
            51..=57 => "Drizzle",
            61..=67 => "Rain",
            71..=77 => "Snow",
            80..=82 => "Showers",
            85 | 86 => "Snow showers",
            95..=99 => "Thunderstorm",
            _ => "Unknown",
        }
    }
}

/// Weather for a location. Cached for 15 minutes, keyed to ~0.1° cells.
pub fn fetch(cache: &DiskCache, lat: f64, lon: f64) -> anyhow::Result<Weather> {
    let (la, lo) = ((lat * 10.0).round() / 10.0, (lon * 10.0).round() / 10.0);
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={la}&longitude={lo}\
         &current=temperature_2m,relative_humidity_2m,precipitation,rain,snowfall,cloud_cover,\
         wind_speed_10m,wind_direction_10m,wind_gusts_10m,surface_pressure,weather_code\
         &hourly=temperature_2m,precipitation,cloud_cover,wind_speed_10m,wind_direction_10m,weather_code\
         &wind_speed_unit=ms&timeformat=unixtime&forecast_days=2"
    );
    let key = format!("weather/{la:.1}_{lo:.1}.json");
    let bytes = cache.get_or_fetch(&key, Some(Duration::from_secs(900)), || crate::http::get_bytes(&url))?;
    parse(&bytes)
}

pub fn parse(bytes: &[u8]) -> anyhow::Result<Weather> {
    let v: serde_json::Value = serde_json::from_slice(bytes)?;
    let c = &v["current"];
    let f = |k: &str| c[k].as_f64().unwrap_or(0.0) as f32;
    let h = &v["hourly"];
    let arr = |k: &str, i: usize| h[k][i].as_f64().unwrap_or(0.0) as f32;
    let n = h["time"].as_array().map_or(0, Vec::len);
    let hourly = (0..n)
        .map(|i| HourlySample {
            unix_s: h["time"][i].as_i64().unwrap_or(0),
            temperature_c: arr("temperature_2m", i),
            precipitation_mm: arr("precipitation", i),
            cloud_cover_pct: arr("cloud_cover", i),
            wind_speed_ms: arr("wind_speed_10m", i),
            wind_direction_deg: arr("wind_direction_10m", i),
            weather_code: arr("weather_code", i) as u16,
        })
        .collect();
    Ok(Weather {
        temperature_c: f("temperature_2m"),
        humidity_pct: f("relative_humidity_2m"),
        precipitation_mm: f("precipitation"),
        rain_mm: f("rain"),
        snowfall_cm: f("snowfall"),
        cloud_cover_pct: f("cloud_cover"),
        wind_speed_ms: f("wind_speed_10m"),
        wind_direction_deg: f("wind_direction_10m"),
        wind_gusts_ms: f("wind_gusts_10m"),
        pressure_hpa: f("surface_pressure"),
        weather_code: f("weather_code") as u16,
        hourly,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sample() {
        let json = br#"{"current":{"temperature_2m":4.5,"relative_humidity_2m":80,"precipitation":0.4,
            "rain":0,"snowfall":0.3,"cloud_cover":100,"wind_speed_10m":6.1,"wind_direction_10m":270,
            "wind_gusts_10m":11,"surface_pressure":845,"weather_code":73},
            "hourly":{"time":[1730000000,1730003600],"temperature_2m":[4,3],"precipitation":[0,1],
            "cloud_cover":[90,100],"wind_speed_10m":[5,6],"wind_direction_10m":[260,270],"weather_code":[3,73]}}"#;
        let w = parse(json).unwrap();
        assert!(w.is_snowing() && !w.is_raining());
        assert_eq!(w.describe(), "Snow");
        assert_eq!(w.hourly.len(), 2);
        assert_eq!(w.hourly[1].weather_code, 73);
    }

    #[test]
    #[ignore]
    fn live() {
        let cache = DiskCache::new(std::env::temp_dir().join("ed-net-test"));
        let w = fetch(&cache, 46.02, 7.75).unwrap();
        eprintln!("{w:?}");
        assert!(w.hourly.len() >= 24);
    }
}
