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
    /// Fixed UTC timestamp for the clock (default: 15:00 local solar time).
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
    /// Multiplayer server address (host:port) for ghost multiplayer.
    pub server: Option<String>,
    /// Player name shown to others.
    pub player_name: Option<String>,
    /// `--no-post`: drop bloom, TAA and SSAO.
    pub no_post: bool,
    /// `--no-hud`: hide the compass, readout and minimap (clean screenshots).
    pub no_hud: bool,
    /// `--scale <f>`: OS scale factor override (1.0 = half the pixels of a 2x Retina display).
    pub scale: Option<f64>,
    /// `--headless` (or `EW_HEADLESS=1`): no window, no Dock icon, no focus
    /// steal, no sound. The 3D camera renders into an offscreen image at
    /// `--width` x `--height`; `--screenshot` and `--perf` work as usual.
    pub headless: bool,
    /// `--cruise <m/s>`: fly camera moves forward on its own (scripted perf route).
    pub cruise: Option<f32>,
    /// `--spin <deg/s>`: fly camera yaws on its own (turn-around test).
    pub spin: Option<f32>,
    /// `--quit-after <s>`: exit after this many seconds (wall clock).
    pub quit_after: Option<f32>,
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
        // Daytime by default: 15:00 local solar time unless --hour overrides it.
        let hour = num("--hour").or(lookdev.as_ref().map(|_| 14.5)).or(Some(crate::menu::DEFAULT_SOLAR_HOUR));
        let unix_override = hour.map(|h| {
            // Today's date (UTC), at the requested *local solar* hour.
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
            let mut day = now;
            // `--date YYYY-MM-DD` pins the day (reference shots at a known season).
            if let Some(d) = get("--date") {
                let p: Vec<i64> = d.split('-').filter_map(|x| x.parse().ok()).collect();
                if let [y, m, dd] = p[..] {
                    // Days from civil (Howard Hinnant).
                    let y = if m <= 2 { y - 1 } else { y };
                    let era = y.div_euclid(400);
                    let yoe = y - era * 400;
                    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + dd - 1;
                    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
                    day = ((era * 146_097 + doe - 719_468) * 86_400) as f64 + 43_200.0;
                }
            }
            ed_geo::sun::unix_at_solar_hour(day, start.lon_deg, h)
        });
        Self {
            start,
            place_name,
            start_agl: num("--alt").unwrap_or(40.0),
            heading_deg: num("--heading").unwrap_or(0.0) as f32,
            pitch_deg: num("--pitch").unwrap_or(-12.0) as f32,
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
            server: get("--server"),
            player_name: get("--name"),
            no_post: args.iter().any(|a| a == "--no-post"),
            no_hud: args.iter().any(|a| a == "--no-hud"),
            scale: num("--scale"),
            headless: args.iter().any(|a| a == "--headless") || std::env::var_os("EW_HEADLESS").is_some_and(|v| v != "0"),
            cruise: num("--cruise").map(|v| v as f32),
            spin: num("--spin").map(|v| v as f32),
            quit_after: num("--quit-after").map(|v| v as f32),
        }
    }

    pub fn now_unix(&self) -> f64 {
        self.unix_override.unwrap_or_else(|| {
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64()
        })
    }
}
