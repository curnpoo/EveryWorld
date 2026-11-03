//! Command line: where to drop in and how to run.
//!
//!   earthdrop --place "Zermatt"
//!   earthdrop --lat 43.53 --lon 5.45 --alt 3000      (skydive in)
//!   earthdrop --place "Shibuya" --hour 18.5 --screenshot shot.png

use bevy::prelude::*;
use ed_data::DiskCache;
use ed_geo::Geodetic;
use std::path::PathBuf;

#[derive(Resource, Debug, Clone)]
pub struct Config {
    pub start: Geodetic,
    pub place_name: String,
    /// Meters above ground to start at.
    pub start_agl: f64,
    /// Camera look direction at start (compass degrees) and pitch.
    pub heading_deg: f32,
    pub pitch_deg: f32,
    /// Override local solar hour (0-24). None = real current time.
    pub hour: Option<f64>,
    pub unix_override: Option<f64>,
    pub screenshot: Option<PathBuf>,
    /// Minimum frames to wait before a screenshot.
    pub screenshot_frames: u32,
    pub atmosphere: bool,
    pub width: u32,
    pub height: u32,
    pub cache: DiskCache,
    /// `--lookdev <dir>`: render art-review boards into this directory.
    pub lookdev: Option<PathBuf>,
    pub lookdev_only: Option<String>,
    /// Spectator fly camera instead of the robot.
    pub fly: bool,
}

impl Config {
    pub fn from_args() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let get = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
        let num = |k: &str| get(k).and_then(|v| v.parse::<f64>().ok());
        let cache = DiskCache::default_location();
        let (mut start, mut place_name) = (Geodetic::new(40.7812, -73.9665, 0.0), "Central Park, New York".to_string());
        if let Some(q) = get("--place") {
            match ed_data::geocode::search(&cache, &q) {
                Ok(results) if !results.is_empty() => {
                    let p = &results[0];
                    start = p.position;
                    place_name = format!("{}, {}", p.name, p.country);
                }
                Ok(_) => eprintln!("no place found for {q:?}, using default"),
                Err(e) => eprintln!("geocoding failed ({e}), using default"),
            }
        }
        if let (Some(lat), Some(lon)) = (num("--lat"), num("--lon")) {
            start = Geodetic::new(lat, lon, 0.0);
            place_name = format!("{lat:.4}, {lon:.4}");
        }
        let lookdev = get("--lookdev").map(PathBuf::from);
        // Ground height at the drop point so "--alt" means above ground.
        if lookdev.is_none() {
            let t = ed_geo::tile::TileId::containing(start.lat_deg, start.lon_deg, 12);
            if let Ok(h) = ed_data::elevation::fetch(&cache, t) {
                let (u, v) = t.geodetic_to_uv(start.lat_deg, start.lon_deg);
                start.alt_m = h.sample(u, v).max(0.0) as f64;
            }
        }
        if lookdev.is_some() && num("--lat").is_none() {
            start = Geodetic::new(40.0, -3.7, 0.0);
            place_name = "Lookdev stage".into();
        }
        let hour = num("--hour").or(lookdev.as_ref().map(|_| 14.5));
        let unix_override = hour.map(|h| {
            // Today's date (UTC), at the requested *local solar* hour.
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
            let midnight = (now / 86_400.0).floor() * 86_400.0;
            midnight + (h - start.lon_deg / 15.0) * 3600.0
        });
        Self {
            start,
            place_name,
            start_agl: num("--alt").unwrap_or(40.0),
            heading_deg: num("--heading").unwrap_or(0.0) as f32,
            pitch_deg: num("--pitch").unwrap_or(-12.0) as f32,
            hour,
            unix_override,
            screenshot: get("--screenshot").map(PathBuf::from),
            screenshot_frames: num("--frames").unwrap_or(30.0) as u32,
            atmosphere: !args.iter().any(|a| a == "--no-atmosphere"),
            width: num("--width").unwrap_or(1600.0) as u32,
            height: num("--height").unwrap_or(900.0) as u32,
            cache,
            lookdev,
            lookdev_only: get("--only"),
            fly: args.iter().any(|a| a == "--fly"),
        }
    }

    pub fn now_unix(&self) -> f64 {
        self.unix_override.unwrap_or_else(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()
        })
    }
}
