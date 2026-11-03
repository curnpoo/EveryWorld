//! Clouds, engine-free half: what the sky should look like for a given
//! weather report, where the thunderstorms are, and the tileable noise
//! volumes the volumetric renderer (`ed_game::clouds`) raymarches.
//!
//! - [`CloudParams`] maps live weather (WMO code, cover %, wind) to a cloud
//!   climate: cumulus coverage and heights, a stratus sheet, storminess,
//!   rain shafts, wind drift.
//! - [`storm_cells`] scatters cumulonimbus cells on a geodetic grid that
//!   drifts with the wind, so a storm 60 km out is the same storm when you
//!   fly over to it, and it keeps coming toward you.
//! - [`shape_noise`] / [`detail_noise`] build Perlin-Worley and Worley fBm
//!   3D textures that tile, so the renderer can anchor them to ECEF modulo
//!   [`NOISE_PERIOD_M`] and stay seamless across floating-origin rebases.

use ed_data::weather::{HourlySample, Weather};

/// Every noise frequency the renderer uses divides this period (meters), so
/// `ecef mod NOISE_PERIOD_M` is an exact, precision-safe noise coordinate.
pub const NOISE_PERIOD_M: f64 = 262_144.0;
/// World size of one tile of the shape texture (base cloud blobs).
pub const SHAPE_PERIOD_M: f32 = 8_192.0;
/// World size of one tile of the detail texture (edge erosion).
pub const DETAIL_PERIOD_M: f32 = 1_024.0;
/// Large-scale coverage variation (where the cloud fields are).
pub const WEATHER_PERIOD_M: f32 = 65_536.0;

/// Planet-anchored noise coordinate of an ECEF point: the point minus the
/// wind drift, wrapped into one noise period per axis. The renderer adds
/// render-space offsets (rotated into ECEF axes) to the camera's coordinate,
/// so the field is the same whichever floating-origin frame drew it.
pub fn noise_coord(ecef: glam::DVec3, drift: glam::DVec3) -> glam::DVec3 {
    let n = ecef - drift;
    glam::DVec3::new(n.x.rem_euclid(NOISE_PERIOD_M), n.y.rem_euclid(NOISE_PERIOD_M), n.z.rem_euclid(NOISE_PERIOD_M))
}

/// Cloud climate for the renderer. Heights are meters above sea level.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CloudParams {
    /// 0..1 cumulus coverage (fraction of sky with convective cells).
    pub coverage: f32,
    pub cumulus_base_m: f32,
    /// Tops of the biggest cumulus (smaller ones stop lower).
    pub cumulus_top_m: f32,
    /// 0..1 amount of a flat stratus/nimbostratus sheet.
    pub stratus: f32,
    pub stratus_base_m: f32,
    pub stratus_top_m: f32,
    /// Extinction multiplier (thicker, wetter clouds are optically denser).
    pub density: f32,
    /// 0..1 extra darkening of the cloud bellies (rain-laden).
    pub darkness: f32,
    /// 0..1 how many cumulonimbus cells to spawn.
    pub storm: f32,
    /// 0..1 strength of rain shafts under raining clouds.
    pub precipitation: f32,
    /// Wind at cloud level (stronger than at 10 m).
    pub wind_speed_ms: f32,
    /// Meteorological direction the wind blows *from*, degrees.
    pub wind_from_deg: f32,
}

impl Default for CloudParams {
    fn default() -> Self {
        Self::from_conditions(&Conditions { weather_code: 2, cloud_cover_pct: 35.0, wind_speed_ms: 4.0, wind_direction_deg: 250.0, precipitation_mm: 0.0 })
    }
}

/// The handful of weather numbers clouds care about.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    pub weather_code: u16,
    pub cloud_cover_pct: f32,
    pub wind_speed_ms: f32,
    pub wind_direction_deg: f32,
    pub precipitation_mm: f32,
}

impl Conditions {
    pub fn current(w: &Weather) -> Self {
        Self {
            weather_code: w.weather_code,
            cloud_cover_pct: w.cloud_cover_pct,
            wind_speed_ms: w.wind_speed_ms,
            wind_direction_deg: w.wind_direction_deg,
            precipitation_mm: w.precipitation_mm,
        }
    }

    fn from_hourly(h: &HourlySample) -> Self {
        Self {
            weather_code: h.weather_code,
            cloud_cover_pct: h.cloud_cover_pct,
            wind_speed_ms: h.wind_speed_ms,
            wind_direction_deg: h.wind_direction_deg,
            precipitation_mm: h.precipitation_mm,
        }
    }

    /// Conditions at `unix_s`, interpolated from the hourly forecast when the
    /// clock is inside it (timelapse), otherwise the current report.
    pub fn at(w: &Weather, unix_s: f64) -> Self {
        let h = &w.hourly;
        let now = Self::current(w);
        if h.len() < 2 {
            return now;
        }
        let t = unix_s as i64;
        let Some(i) = h.windows(2).position(|p| p[0].unix_s <= t && t < p[1].unix_s) else { return now };
        let (a, b) = (&h[i], &h[i + 1]);
        let f = ((unix_s - a.unix_s as f64) / (b.unix_s - a.unix_s).max(1) as f64) as f32;
        let (ca, cb) = (Self::from_hourly(a), Self::from_hourly(b));
        // Within the current hour trust the live report over the forecast.
        let real_now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        if (t - real_now).abs() < 1800 {
            return now;
        }
        Self {
            weather_code: if f < 0.5 { ca.weather_code } else { cb.weather_code },
            cloud_cover_pct: lerp(ca.cloud_cover_pct, cb.cloud_cover_pct, f),
            wind_speed_ms: lerp(ca.wind_speed_ms, cb.wind_speed_ms, f),
            wind_direction_deg: lerp_angle_deg(ca.wind_direction_deg, cb.wind_direction_deg, f),
            precipitation_mm: lerp(ca.precipitation_mm, cb.precipitation_mm, f),
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_angle_deg(a: f32, b: f32, t: f32) -> f32 {
    let d = ((b - a + 540.0) % 360.0) - 180.0;
    (a + d * t + 360.0) % 360.0
}

impl CloudParams {
    /// Map a weather report to a cloud climate (WMO 4677 code families).
    pub fn from_conditions(c: &Conditions) -> Self {
        let cover = (c.cloud_cover_pct / 100.0).clamp(0.0, 1.0);
        let precip = (c.precipitation_mm / 4.0).clamp(0.0, 1.0);
        let mut p = Self {
            coverage: cover * 0.75,
            cumulus_base_m: 1_300.0,
            cumulus_top_m: 2_600.0 + 1_200.0 * cover,
            stratus: 0.0,
            stratus_base_m: 1_100.0,
            stratus_top_m: 1_700.0,
            density: 1.0,
            darkness: 0.0,
            storm: 0.0,
            precipitation: 0.0,
            // Wind at ~1.5 km is roughly 1.5-2x the 10 m wind.
            wind_speed_ms: (c.wind_speed_ms * 1.7).max(2.0),
            wind_from_deg: c.wind_direction_deg,
        };
        match c.weather_code {
            // Clear / mostly clear: a few small fair-weather cumulus.
            0 | 1 => {
                p.coverage = (cover * 0.6).clamp(0.08, 0.35);
                p.cumulus_top_m = 2_200.0 + 600.0 * cover;
            }
            // Partly cloudy: classic cumulus humilis/mediocris.
            2 => {
                p.coverage = (cover * 0.75).clamp(0.3, 0.6);
            }
            // Overcast: a thick stratocumulus/stratus deck.
            3 => {
                p.coverage = 0.35 + 0.3 * cover;
                p.stratus = 0.6 + 0.4 * cover;
                p.stratus_base_m = 900.0;
                p.stratus_top_m = 1_900.0;
                p.cumulus_base_m = 1_000.0;
                p.cumulus_top_m = 2_300.0;
                p.darkness = 0.25;
            }
            // Fog: the deck sits on the ground.
            45 | 48 => {
                p.coverage = 0.3;
                p.stratus = 1.0;
                p.stratus_base_m = 0.0;
                p.stratus_top_m = 450.0;
                p.cumulus_base_m = 500.0;
                p.cumulus_top_m = 1_200.0;
                p.darkness = 0.1;
            }
            // Drizzle / rain / snow: nimbostratus, low dark base, rain shafts.
            51..=67 | 71..=77 => {
                p.coverage = 0.6;
                p.stratus = 1.0;
                p.stratus_base_m = 700.0;
                p.stratus_top_m = 2_600.0;
                p.cumulus_base_m = 800.0;
                p.cumulus_top_m = 3_200.0;
                p.density = 1.4;
                p.darkness = 0.55;
                p.precipitation = 0.35 + 0.65 * precip.max(0.3);
            }
            // Showers: towering cumulus congestus, some raining.
            80..=82 | 85 | 86 => {
                p.coverage = (cover * 0.8).clamp(0.45, 0.75);
                p.cumulus_base_m = 1_100.0;
                p.cumulus_top_m = 5_500.0;
                p.density = 1.2;
                p.darkness = 0.35;
                p.storm = 0.25;
                p.precipitation = 0.5 + 0.5 * precip;
            }
            // Thunderstorm: cumulonimbus with anvils, heavy rain shafts.
            95..=99 => {
                p.coverage = (cover * 0.7).clamp(0.4, 0.7);
                p.cumulus_base_m = 1_100.0;
                p.cumulus_top_m = 4_500.0;
                p.density = 1.4;
                p.darkness = 0.5;
                p.storm = 1.0;
                p.precipitation = 1.0;
            }
            _ => {}
        }
        p
    }

    pub fn lerp(&self, o: &Self, t: f32) -> Self {
        Self {
            coverage: lerp(self.coverage, o.coverage, t),
            cumulus_base_m: lerp(self.cumulus_base_m, o.cumulus_base_m, t),
            cumulus_top_m: lerp(self.cumulus_top_m, o.cumulus_top_m, t),
            stratus: lerp(self.stratus, o.stratus, t),
            stratus_base_m: lerp(self.stratus_base_m, o.stratus_base_m, t),
            stratus_top_m: lerp(self.stratus_top_m, o.stratus_top_m, t),
            density: lerp(self.density, o.density, t),
            darkness: lerp(self.darkness, o.darkness, t),
            storm: lerp(self.storm, o.storm, t),
            precipitation: lerp(self.precipitation, o.precipitation, t),
            wind_speed_ms: lerp(self.wind_speed_ms, o.wind_speed_ms, t),
            wind_from_deg: lerp_angle_deg(self.wind_from_deg, o.wind_from_deg, t),
        }
    }

    /// Unit vector (east, north) the clouds drift *toward*.
    pub fn wind_dir_en(&self) -> [f32; 2] {
        let to = (self.wind_from_deg + 180.0).to_radians();
        [to.sin(), to.cos()]
    }

    /// How much direct sun reaches the ground through the stratus deck
    /// (0..1). Used to dim the sun under overcast so the ground lighting goes
    /// flat; cumulus and storms cast real (patchy) shadows instead.
    pub fn sun_fraction(&self) -> f32 {
        let deck = self.stratus * (0.8 + 0.12 * self.darkness);
        // Floor: the renderer's sky light doesn't know about the bright
        // cloud deck, so keep some sun as stand-in diffuse light.
        (1.0 - deck).clamp(0.25, 1.0)
    }

    /// Highest point any cloud reaches.
    pub fn ceiling_m(&self) -> f32 {
        let storm_top = if self.storm > 0.0 { STORM_TOP_MAX_M } else { 0.0 };
        self.cumulus_top_m.max(self.stratus_top_m).max(storm_top)
    }
}

/// A synthetic weather report for `--weather <name>` (tests, lookdev, and
/// when Open-Meteo rate-limits us).
pub fn weather_preset(name: &str) -> Option<Weather> {
    let (code, cover, wind, dir, precip, desc_humidity) = match name.to_ascii_lowercase().as_str() {
        "clear" => (0, 5.0, 3.0, 270.0, 0.0, 40.0),
        "fair" | "cumulus" => (2, 40.0, 4.0, 250.0, 0.0, 55.0),
        "cloudy" | "overcast" => (3, 100.0, 6.0, 220.0, 0.0, 80.0),
        "rain" => (63, 100.0, 7.0, 200.0, 3.0, 95.0),
        "showers" => (81, 70.0, 7.0, 240.0, 2.0, 80.0),
        "storm" | "thunderstorm" => (95, 75.0, 9.0, 240.0, 6.0, 85.0),
        "fog" => (45, 100.0, 1.0, 180.0, 0.0, 100.0),
        _ => return None,
    };
    Some(Weather {
        temperature_c: 18.0,
        humidity_pct: desc_humidity,
        precipitation_mm: precip,
        rain_mm: precip,
        cloud_cover_pct: cover,
        wind_speed_ms: wind,
        wind_direction_deg: dir,
        wind_gusts_ms: wind * 1.6,
        pressure_hpa: 1013.0,
        weather_code: code,
        ..Default::default()
    })
}

// ---------------------------------------------------------------------------
// Storm cells
// ---------------------------------------------------------------------------

pub const STORM_TOP_MAX_M: f32 = 12_500.0;
/// Storm grid spacing in degrees of latitude (~28 km).
const STORM_GRID_DEG: f64 = 0.25;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StormCell {
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Core radius at cloud base, meters.
    pub radius_m: f32,
    /// Top of the anvil, meters above sea level.
    pub top_m: f32,
    /// 0..1 maturity: young towers are narrow, mature cells grow anvils.
    pub strength: f32,
}

/// Cumulonimbus cells within `range_m` of (lat, lon), nearest first.
/// `drift_lat_deg`/`drift_lon_deg` is how far the wind has carried the storm
/// field since the epoch; cells are fixed in that drifting frame.
pub fn storm_cells(lat: f64, lon: f64, storm: f32, drift_lat_deg: f64, drift_lon_deg: f64, range_m: f64) -> Vec<StormCell> {
    if storm <= 0.0 {
        return Vec::new();
    }
    let m_per_deg_lat = 111_320.0;
    let m_per_deg_lon = |lat: f64| (m_per_deg_lat * lat.to_radians().cos()).max(1_000.0);
    // Position in the drifting frame.
    let (flat, flon) = (lat - drift_lat_deg, lon - drift_lon_deg);
    let gi = (flat / STORM_GRID_DEG).floor() as i64;
    let reach = (range_m / (STORM_GRID_DEG * m_per_deg_lat)).ceil() as i64 + 1;
    let mut out = Vec::new();
    for i in gi - reach..=gi + reach {
        // Each grid row has its own longitude step so cells stay ~square.
        let row_lat = (i as f64 + 0.5) * STORM_GRID_DEG;
        let lon_step = STORM_GRID_DEG * m_per_deg_lat / m_per_deg_lon(row_lat);
        let gj = (flon / lon_step).floor() as i64;
        let reach_j = (range_m / (lon_step * m_per_deg_lon(row_lat))).ceil() as i64 + 1;
        for j in gj - reach_j..=gj + reach_j {
            let key = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (j as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
            // Thunderstorm weather: about half the grid cells hold a storm.
            if crate::hash01(key, 1) > 0.5 * storm {
                continue;
            }
            let clat = (i as f64 + 0.15 + 0.7 * crate::hash01(key, 2) as f64) * STORM_GRID_DEG + drift_lat_deg;
            let clon = (j as f64 + 0.15 + 0.7 * crate::hash01(key, 3) as f64) * lon_step + drift_lon_deg;
            let (dn, de) = ((clat - lat) * m_per_deg_lat, (clon - lon) * m_per_deg_lon(lat));
            if (dn * dn + de * de).sqrt() > range_m {
                continue;
            }
            let strength = 0.45 + 0.55 * crate::hash01(key, 4);
            out.push(StormCell {
                lat_deg: clat,
                lon_deg: clon,
                radius_m: 3_000.0 + 3_500.0 * crate::hash01(key, 5),
                top_m: 8_500.0 + (STORM_TOP_MAX_M - 8_500.0) * strength,
                strength,
            });
        }
    }
    let d2 = |c: &StormCell| {
        let (dn, de) = ((c.lat_deg - lat) * m_per_deg_lat, (c.lon_deg - lon) * m_per_deg_lon(lat));
        dn * dn + de * de
    };
    out.sort_by(|a, b| d2(a).total_cmp(&d2(b)));
    out
}

/// Advance the storm-field drift by `dt_s` seconds of wind (degrees).
pub fn advect_deg(lat: f64, drift: [f64; 2], wind_en_ms: [f32; 2], dt_s: f64) -> [f64; 2] {
    let m_per_deg_lon = (111_320.0 * lat.to_radians().cos()).max(1_000.0);
    [drift[0] + wind_en_ms[1] as f64 * dt_s / 111_320.0, drift[1] + wind_en_ms[0] as f64 * dt_s / m_per_deg_lon]
}

// ---------------------------------------------------------------------------
// Tileable noise volumes
// ---------------------------------------------------------------------------

fn hash3(x: i32, y: i32, z: i32, seed: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f) ^ seed.wrapping_mul(0x9e37_79b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    h
}

fn h01(h: u32) -> f32 {
    (h & 0x00ff_ffff) as f32 / 16_777_216.0
}

/// Tileable Worley F1 on the unit cube with `freq` cells per side, returning
/// 1 - distance (bright cell centers, like billows), in 0..1.
pub fn worley(p: [f32; 3], freq: i32, seed: u32) -> f32 {
    let f = freq as f32;
    let q = [p[0] * f, p[1] * f, p[2] * f];
    let c = [q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32];
    let mut best = f32::MAX;
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (cx, cy, cz) = (c[0] + dx, c[1] + dy, c[2] + dz);
                let (wx, wy, wz) = (cx.rem_euclid(freq), cy.rem_euclid(freq), cz.rem_euclid(freq));
                let h = hash3(wx, wy, wz, seed);
                let fp = [
                    cx as f32 + h01(h),
                    cy as f32 + h01(h.rotate_left(11) ^ 0x5bd1_e995),
                    cz as f32 + h01(h.rotate_left(22) ^ 0x1b87_3593),
                ];
                let d = (fp[0] - q[0]).powi(2) + (fp[1] - q[1]).powi(2) + (fp[2] - q[2]).powi(2);
                best = best.min(d);
            }
        }
    }
    (1.0 - best.sqrt()).clamp(0.0, 1.0)
}

fn grad(h: u32, x: f32, y: f32, z: f32) -> f32 {
    // 12 cube-edge gradients.
    match h % 12 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        3 => -x - y,
        4 => x + z,
        5 => -x + z,
        6 => x - z,
        7 => -x - z,
        8 => y + z,
        9 => -y + z,
        10 => y - z,
        _ => -y - z,
    }
}

/// Tileable gradient (Perlin) noise, roughly -1..1.
pub fn perlin(p: [f32; 3], freq: i32, seed: u32) -> f32 {
    let f = freq as f32;
    let q = [p[0] * f, p[1] * f, p[2] * f];
    let i = [q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32];
    let t = [q[0] - i[0] as f32, q[1] - i[1] as f32, q[2] - i[2] as f32];
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let u = [fade(t[0]), fade(t[1]), fade(t[2])];
    let g = |dx: i32, dy: i32, dz: i32| {
        let h = hash3((i[0] + dx).rem_euclid(freq), (i[1] + dy).rem_euclid(freq), (i[2] + dz).rem_euclid(freq), seed);
        grad(h, t[0] - dx as f32, t[1] - dy as f32, t[2] - dz as f32)
    };
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = l(g(0, 0, 0), g(1, 0, 0), u[0]);
    let x10 = l(g(0, 1, 0), g(1, 1, 0), u[0]);
    let x01 = l(g(0, 0, 1), g(1, 0, 1), u[0]);
    let x11 = l(g(0, 1, 1), g(1, 1, 1), u[0]);
    l(l(x00, x10, u[1]), l(x01, x11, u[1]), u[2])
}

fn worley_fbm(p: [f32; 3], freq: i32, seed: u32) -> f32 {
    worley(p, freq, seed) * 0.625 + worley(p, freq * 2, seed + 1) * 0.25 + worley(p, freq * 4, seed + 2) * 0.125
}

fn perlin_fbm(p: [f32; 3], freq: i32, seed: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut f = freq;
    for o in 0..4 {
        sum += perlin(p, f, seed + o) * amp;
        amp *= 0.5;
        f *= 2;
    }
    (sum * 0.5 / 0.9375 + 0.5).clamp(0.0, 1.0)
}

fn remap(v: f32, lo: f32, hi: f32, nlo: f32, nhi: f32) -> f32 {
    nlo + (v - lo) / (hi - lo) * (nhi - nlo)
}

/// Fill a `size`^3 RGBA8 volume in parallel (x fastest, then y, then z).
fn volume(size: usize, f: impl Fn([f32; 3]) -> [f32; 4] + Sync) -> Vec<u8> {
    let mut data = vec![0u8; size * size * size * 4];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(size);
    let slab = size.div_ceil(threads) * size * size * 4;
    std::thread::scope(|s| {
        for (k, chunk) in data.chunks_mut(slab).enumerate() {
            let f = &f;
            s.spawn(move || {
                let z0 = k * slab / (size * size * 4);
                for (i, px) in chunk.chunks_exact_mut(4).enumerate() {
                    let idx = i + z0 * size * size;
                    let (x, y, z) = (idx % size, (idx / size) % size, idx / (size * size));
                    let p = [(x as f32 + 0.5) / size as f32, (y as f32 + 0.5) / size as f32, (z as f32 + 0.5) / size as f32];
                    let v = f(p);
                    for c in 0..4 {
                        px[c] = (v[c].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            });
        }
    });
    data
}

/// Base cloud shapes, `size`^3 RGBA8, tiling.
/// R: Perlin-Worley (billowy blobs), G/B/A: Worley fBm at 4/8/16 cells.
pub fn shape_noise(size: usize) -> Vec<u8> {
    volume(size, |p| {
        let pf = perlin_fbm(p, 4, 11);
        let w = worley_fbm(p, 4, 21);
        // Perlin-Worley: Perlin dilated by Worley, keeps the connected
        // structure of Perlin with the round billows of Worley.
        let pw = remap(pf, w - 1.0, 1.0, 0.0, 1.0);
        [pw, w, worley_fbm(p, 8, 31), worley_fbm(p, 16, 41)]
    })
}

/// Edge detail for erosion, `size`^3 RGBA8, tiling.
/// R/G/B: Worley fBm at 2/4/8 cells, A: Perlin fBm (wispy, for stratus).
pub fn detail_noise(size: usize) -> Vec<u8> {
    volume(size, |p| [worley_fbm(p, 2, 51), worley_fbm(p, 4, 61), worley_fbm(p, 8, 71), perlin_fbm(p, 4, 81)])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The floating origin re-anchors the render frame every 8 km. A cloud
    /// sample must land on the same noise coordinate (modulo the texture
    /// periods) from either frame, using the same f32 math as the shader.
    #[test]
    fn noise_coords_survive_rebase() {
        use ed_geo::{EnuFrame, Geodetic};
        use glam::{DVec3, Vec3};
        let drift = DVec3::new(1234.5, -987.0, 4321.0);
        let cam_geo = Geodetic::new(40.80, -73.95, 1500.0);
        let a = EnuFrame::new(Geodetic::new(40.77, -73.97, 0.0));
        let b = EnuFrame::new(Geodetic::new(40.84, -73.90, 0.0));
        // A cloud 30 km north-east of the camera, 1 km up.
        let target = Geodetic::new(40.99, -73.70, 2500.0);
        let sample = |f: &EnuFrame| {
            let cam_r = f.geodetic_to_render(cam_geo);
            let tgt_r = f.geodetic_to_render(target);
            let r2e = f.render_to_ecef();
            let cam_noise = noise_coord(r2e.transform_point3(cam_r), drift).as_vec3();
            // Shader side: f32 offset rotated into noise axes.
            let off = (tgt_r - cam_r).as_vec3();
            let m = r2e.matrix3.as_mat3();
            cam_noise + m * off
        };
        let (pa, pb) = (sample(&a), sample(&b));
        let wrap = |v: f32, p: f32| ((v % p) + p) % p;
        for period in [SHAPE_PERIOD_M, DETAIL_PERIOD_M, WEATHER_PERIOD_M] {
            for k in 0..3 {
                let (x, y) = (wrap(pa[k], period), wrap(pb[k], period));
                let d = (x - y).abs().min(period - (x - y).abs());
                assert!(d < 0.5, "axis {k} period {period}: {x} vs {y}");
            }
        }
        let _ = Vec3::ZERO;
    }

    #[test]
    fn noise_tiles_seamlessly() {
        for (a, b) in [([0.0, 0.3, 0.7], [1.0, 0.3, 0.7]), ([0.2, 0.0, 0.5], [0.2, 1.0, 0.5]), ([0.9, 0.4, 0.0], [0.9, 0.4, 1.0])] {
            assert!((worley(a, 4, 3) - worley(b, 4, 3)).abs() < 1e-4);
            assert!((perlin(a, 4, 3) - perlin(b, 4, 3)).abs() < 1e-4);
        }
    }

    #[test]
    fn noise_in_range_and_varied() {
        let mut lo = 1.0f32;
        let mut hi = 0.0f32;
        for i in 0..500 {
            let p = [crate::hash01(i, 1), crate::hash01(i, 2), crate::hash01(i, 3)];
            let w = worley(p, 4, 9);
            let n = perlin(p, 4, 9);
            assert!((0.0..=1.0).contains(&w));
            assert!((-1.2..=1.2).contains(&n));
            lo = lo.min(w);
            hi = hi.max(w);
        }
        assert!(hi - lo > 0.5, "worley should have contrast ({lo}..{hi})");
    }

    #[test]
    fn volumes_have_expected_size() {
        let v = shape_noise(8);
        assert_eq!(v.len(), 8 * 8 * 8 * 4);
        assert!(v.iter().any(|&b| b > 0) && v.iter().any(|&b| b < 255));
        assert_eq!(detail_noise(4).len(), 4 * 4 * 4 * 4);
    }

    #[test]
    fn periods_divide_noise_period() {
        for p in [SHAPE_PERIOD_M, DETAIL_PERIOD_M, WEATHER_PERIOD_M] {
            assert_eq!(NOISE_PERIOD_M % p as f64, 0.0);
        }
    }

    #[test]
    fn weather_maps_to_cloud_types() {
        let p = |name: &str| CloudParams::from_conditions(&Conditions::current(&weather_preset(name).unwrap()));
        let (clear, fair, overcast, storm, rain) = (p("clear"), p("fair"), p("overcast"), p("storm"), p("rain"));
        assert!(clear.coverage < fair.coverage);
        assert!(clear.storm == 0.0 && fair.storm == 0.0 && storm.storm == 1.0);
        assert!(overcast.stratus > 0.5 && fair.stratus == 0.0);
        assert!(rain.precipitation > 0.3 && fair.precipitation == 0.0);
        assert!(storm.ceiling_m() > 10_000.0);
        assert!(overcast.sun_fraction() < 0.4 && clear.sun_fraction() > 0.8);
        // Wind from the west drifts clouds east.
        let mut w = fair;
        w.wind_from_deg = 270.0;
        let [e, n] = w.wind_dir_en();
        assert!(e > 0.99 && n.abs() < 1e-3);
    }

    #[test]
    fn forecast_interpolates() {
        let mut w = weather_preset("clear").unwrap();
        w.hourly = vec![
            HourlySample { unix_s: 1_000_000, cloud_cover_pct: 0.0, weather_code: 0, wind_direction_deg: 350.0, ..Default::default() },
            HourlySample { unix_s: 1_003_600, cloud_cover_pct: 100.0, weather_code: 3, wind_direction_deg: 10.0, ..Default::default() },
        ];
        let c = Conditions::at(&w, 1_001_800.0);
        assert!((c.cloud_cover_pct - 50.0).abs() < 0.1);
        assert!((c.wind_direction_deg - 0.0).abs() < 0.1 || (c.wind_direction_deg - 360.0).abs() < 0.1);
        assert_eq!(Conditions::at(&w, 1_003_000.0).weather_code, 3);
        // Outside the forecast: current report.
        assert_eq!(Conditions::at(&w, 5.0).cloud_cover_pct, w.cloud_cover_pct);
    }

    #[test]
    fn storm_cells_are_stable_and_drift() {
        let a = storm_cells(40.7, -74.0, 1.0, 0.0, 0.0, 80_000.0);
        assert!(!a.is_empty(), "thunderstorm weather should produce cells");
        assert_eq!(a, storm_cells(40.7, -74.0, 1.0, 0.0, 0.0, 80_000.0));
        assert!(storm_cells(40.7, -74.0, 0.0, 0.0, 0.0, 80_000.0).is_empty());
        // Drifting the field 0.05 deg east moves the cells with it.
        let b = storm_cells(40.7, -74.0, 1.0, 0.0, 0.05, 200_000.0);
        let c0 = a[0];
        assert!(b.iter().any(|c| (c.lat_deg - c0.lat_deg).abs() < 1e-9 && (c.lon_deg - c0.lon_deg - 0.05).abs() < 1e-9));
        // Moving the observer does not move the cells.
        let far = storm_cells(40.9, -73.8, 1.0, 0.0, 0.0, 200_000.0);
        assert!(far.iter().any(|c| c == &c0));
        let d = advect_deg(40.7, [0.0, 0.0], [10.0, 0.0], 3600.0);
        assert!(d[0].abs() < 1e-12 && d[1] > 0.4 && d[1] < 0.5);
        for c in &a {
            assert!(c.top_m > 8_000.0 && c.top_m <= STORM_TOP_MAX_M && c.radius_m > 1_000.0);
        }
    }
}
