//! Cloud look reference: draws the cloud field of `ed_world::cloudscape` on the CPU with the
//! same density and lighting maths the GPU pass mirrors, under the renderer's own CPU
//! atmosphere (`atmosphere_model::Tables`) and tonemap (`tonemap::alpha_tonemap`), so a
//! preview is what the game should show.
//!
//! ```sh
//! cargo run --release -p ed_render --example cloud_preview -- OUT_DIR [preset..] \
//!     [--size 960x540] [--spp 4] [--view ground|base|above|all] [--time S] [--sun ELEV,AZ] [--cfg high|ref]
//! ```
//! Presets: clear fair scattered towering thunderstorm overcast frontal cirrus layered
//! (default: all but clear). `CLOUD_DEBUG=transmittance|sun|ambient` draws one term alone. `--spp` averages that many jittered marches per pixel (the GPU's
//! temporal accumulation converged); `--spp 1 --cfg high` is one raw GPU frame.

use ed_render::atmosphere_model::{Body, Tables};
use ed_render::{light::EXPOSURE, tonemap::alpha_tonemap};
use ed_world::cloudscape::light::{march, CloudLight, MarchCfg, Planet};
use ed_world::cloudscape::storm::storm_cells;
use ed_world::cloudscape::weather::{CloudWeather, Preset};
use ed_world::cloudscape::{field::Lod, CloudField, NoiseBank, EARTH_RADIUS_M};
use glam::{DVec2, DVec3, Vec3};
use std::sync::atomic::{AtomicUsize, Ordering};

struct View {
    name: &'static str,
    yaw_deg: f32,
    pitch_deg: f32,
    vfov_deg: f32,
}

/// Views; altitudes follow the weather (see [`view_alt`]).
const VIEWS: [View; 3] = [
    // On the ground, looking a little up across the sky.
    View { name: "ground", yaw_deg: 20.0, pitch_deg: 14.0, vfov_deg: 62.0 },
    // Flying just under the cloud base, level.
    View { name: "base", yaw_deg: 60.0, pitch_deg: 3.0, vfov_deg: 60.0 },
    // Above the low tops, looking out and a little down over the towers.
    View { name: "above", yaw_deg: 100.0, pitch_deg: -6.0, vfov_deg: 60.0 },
];

fn view_alt(v: &View, w: &CloudWeather) -> f64 {
    let low = w.layers()[0];
    match v.name {
        "ground" => w.ground_m as f64 + 2.0,
        "base" => (low.base_m - 250.0).max(w.ground_m + 50.0) as f64,
        _ => {
            // Above the low tops, and above a mid sheet when there is one.
            let mid = w.layers()[1];
            let a = (low.top_m + 600.0).max(3_000.0);
            if mid.coverage > 0.3 && a > mid.base_m - 300.0 { (mid.top_m + 600.0) as f64 } else { a as f64 }
        }
    }
}

/// Per-pixel, per-sample march jitter. Stratified over the samples of a pixel (sample `k` of
/// `n` lands in its own 1/n slice) with a hashed white-noise offset per pixel: structured
/// dither alone (IGN) beats against the smoothly varying step phase on flat cloud tops and
/// leaves moire contours that a handful of samples cannot average out.
fn jitter(x: u32, y: u32, k: u32, n: u32) -> f32 {
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841) ^ 0x9e37_79b9;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    let r = (h & 0xff_ffff) as f32 / 16_777_216.0;
    ((k as f32 + r) / n as f32).fract()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = None;
    let mut presets = Vec::new();
    let (mut w, mut h, mut spp, mut time) = (960u32, 540u32, 4u32, 600.0f64);
    let mut views: Vec<&View> = VIEWS.iter().collect();
    let mut sun_elev_az = (32.0f32, 145.0f32);
    let mut cfg = MarchCfg::reference();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--size" => {
                let s = it.next().expect("--size WxH");
                let (a, b) = s.split_once('x').expect("WxH");
                (w, h) = (a.parse().unwrap(), b.parse().unwrap());
            }
            "--spp" => spp = it.next().unwrap().parse().unwrap(),
            "--time" => time = it.next().unwrap().parse().unwrap(),
            "--sun" => {
                let s = it.next().unwrap();
                let (a, b) = s.split_once(',').expect("ELEV,AZ");
                sun_elev_az = (a.parse().unwrap(), b.parse().unwrap());
            }
            "--cfg" => cfg = if it.next().unwrap() == "high" { MarchCfg::high() } else { MarchCfg::reference() },
            "--view" => {
                let v = it.next().unwrap();
                views = if v == "all" { VIEWS.iter().collect() } else { VIEWS.iter().filter(|x| x.name == v).collect() };
            }
            s if out.is_none() => out = Some(std::path::PathBuf::from(s)),
            s => presets.push(Preset::from_name(s).unwrap_or_else(|| panic!("unknown preset {s}"))),
        }
    }
    let out = out.expect("usage: cloud_preview OUT_DIR [preset..]");
    std::fs::create_dir_all(&out).unwrap();
    if presets.is_empty() {
        presets = Preset::ALL[1..].to_vec();
    }
    let t0 = std::time::Instant::now();
    let bank = NoiseBank::bake(0xC10D);
    eprintln!("noise bank baked in {:.2} s", t0.elapsed().as_secs_f32());
    let body = Body::earth();
    let tables = Tables::new(&body, DVec3::new(0.3, 0.3, 0.3), 1.0);
    let (el, az) = (sun_elev_az.0.to_radians(), sun_elev_az.1.to_radians());
    // Render frame: x east, y up, z -north; azimuth clockwise from north.
    let sun = Vec3::new(az.sin() * el.cos(), el.sin(), -az.cos() * el.cos()).normalize();
    let planet = Planet { radius_m: EARTH_RADIUS_M };
    for p in &presets {
        let weather = CloudWeather::preset(*p);
        let layers = weather.layers();
        let storms = storm_cells(&weather, &layers[0], DVec2::ZERO, 120_000.0, time);
        let field = CloudField::new(&bank, layers, storms, time, weather.ground_m, weather.seed);
        for v in &views {
            let t1 = std::time::Instant::now();
            let px = render(&field, &tables, &body, &planet, v, view_alt(v, &weather), sun, w, h, spp, &cfg);
            let name = format!("{}_{}.png", p.name(), v.name);
            save(&out.join(&name), &px, w, h);
            eprintln!("{name}: {:.1} s", t1.elapsed().as_secs_f32());
        }
    }
}

/// Mean sky radiance over the upper hemisphere and the lit ground's radiance, at altitude
/// `h_km`, for the cloud ambient terms.
fn ambient(tables: &Tables, h_km: f64, sun: DVec3) -> (Vec3, Vec3) {
    let e = ed_render::atmosphere_model::irradiance(DVec3::Y, |d| if d.y > 0.0 { tables.sky_dir(h_km, d, sun, 12) } else { DVec3::ZERO }, 8, 16);
    let sky = e / std::f64::consts::PI;
    let ground = tables.ground_radiance(0.0, sun.y, 0.0, 0.0);
    (sky.as_vec3(), ground.as_vec3())
}

#[allow(clippy::too_many_arguments)]
fn render(field: &CloudField<'_>, tables: &Tables, body: &Body, planet: &Planet, v: &View, alt_m: f64, sun: Vec3, w: u32, h: u32, spp: u32, cfg: &MarchCfg) -> Vec<u8> {
    let (yaw, pitch) = (v.yaw_deg.to_radians(), v.pitch_deg.to_radians());
    let fwd = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), -yaw.cos() * pitch.cos());
    let right = fwd.cross(Vec3::Y).normalize();
    let up = right.cross(fwd);
    let tan = (v.vfov_deg.to_radians() * 0.5).tan();
    let aspect = w as f32 / h as f32;
    let sund = sun.as_dvec3();
    // Light at the clouds: the sun through the air down to ~2.5 km, the sky and ground there.
    let h_cloud_km = 2.5;
    let sun_rad = (body.sun() * tables.sun_transmittance(h_cloud_km, sund.y)).as_vec3();
    let (sky_amb, ground_amb) = ambient(tables, h_cloud_km, sund);
    let light = CloudLight { sun_dir: sun, sun: sun_rad, sky: sky_amb, ground: ground_amb };
    let cam = DVec3::new(0.0, alt_m, 0.0);
    let h_km = alt_m * 1e-3;
    let mut px = vec![0u8; (w * h * 4) as usize];
    let next = AtomicUsize::new(0);
    let rows: Vec<std::sync::Mutex<Vec<u8>>> = (0..h).map(|_| std::sync::Mutex::new(Vec::new())).collect();
    std::thread::scope(|s| {
        for _ in 0..std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4) {
            s.spawn(|| loop {
                let y = next.fetch_add(1, Ordering::Relaxed) as u32;
                if y >= h {
                    break;
                }
                let mut row = Vec::with_capacity(w as usize * 4);
                for x in 0..w {
                    let mut acc = Vec3::ZERO;
                    for k in 0..spp {
                        // Sub-pixel offsets and march jitter vary per sample (and per frame on the GPU).
                        let (ox, oy) = ((k as f32 * 0.618_034).fract(), (k as f32 * 0.754_877).fract());
                        let ndc_x = ((x as f32 + ox) / w as f32 * 2.0 - 1.0) * tan * aspect;
                        let ndc_y = (1.0 - (y as f32 + oy) / h as f32 * 2.0) * tan;
                        let d = (fwd + right * ndc_x + up * ndc_y).normalize();
                        acc += shade(field, tables, planet, cam, alt_m, h_km, d, sun, &light, cfg, jitter(x, y, k, spp));
                    }
                    let c = alpha_tonemap((acc / spp as f32).to_array(), EXPOSURE);
                    row.extend(c.map(|v| (v * 255.0 + 0.5) as u8));
                    row.push(255);
                }
                *rows[y as usize].lock().unwrap() = row;
            });
        }
    });
    for (y, r) in rows.into_iter().enumerate() {
        let r = r.into_inner().unwrap();
        px[y * w as usize * 4..(y + 1) * w as usize * 4].copy_from_slice(&r);
    }
    px
}

#[allow(clippy::too_many_arguments)]
fn shade(field: &CloudField<'_>, tables: &Tables, planet: &Planet, cam: DVec3, alt0: f64, h_km: f64, d: Vec3, sun: Vec3, light: &CloudLight, cfg: &MarchCfg, jitter: f32) -> Vec3 {
    let dd = d.as_dvec3();
    let sund = sun.as_dvec3();
    let mut bg = tables.sky_dir(h_km, dd, sund, 24);
    // Ground under the sky: cloud shadow on its sunlit part.
    let t_ground = planet.sphere(alt0, dd, field.ground_m as f64).map(|(n, _)| n).filter(|&n| n > 0.0);
    if let Some(tg) = t_ground {
        let gp = dd * tg;
        let shadow = cloud_shadow(field, planet, cam, alt0, gp, sun);
        // About 60% of a lit ground's light is direct sun at these sun heights.
        bg *= (0.4 + 0.6 * shadow) as f64;
    } else if d.dot(sun) > (0.004654f32 * 1.6).cos() {
        // The sun's disc (bloom-free), so silver linings have something to line.
        bg += tables.body.sun() * tables.sun_transmittance(h_km, sund.y) * 2.0e4;
    }
    let t_max = t_ground.unwrap_or(f64::INFINITY);
    // CLOUD_DEBUG=transmittance|sun|ambient: one term alone (look-dev and bug hunts).
    let dbg = std::env::var("CLOUD_DEBUG").unwrap_or_default();
    let light = &match dbg.as_str() {
        "sun" => CloudLight { sky: Vec3::ZERO, ground: Vec3::ZERO, ..*light },
        "ambient" => CloudLight { sun: Vec3::ZERO, ..*light },
        _ => *light,
    };
    let c = march(field, planet, cam, alt0, d, light, cfg, jitter, t_max);
    if dbg == "transmittance" {
        return Vec3::splat(c.transmittance * 2.0);
    }
    if c.transmittance >= 1.0 {
        return bg.as_vec3();
    }
    // Aerial perspective between the camera and the cloud: L = L_air + T_air * L_cloud +
    // T_cloud * (background - L_air).
    let mu = d.y as f64;
    let (l_air, t_air) = tables.march(h_km.min(99.0), mu, sund.y, dd.dot(sund), 16, Some(c.depth_m as f64 * 1e-3));
    let l = l_air + t_air * c.radiance.as_dvec3() + (bg - l_air).max(DVec3::ZERO) * c.transmittance as f64;
    l.as_vec3()
}

/// Transmittance of the clouds from ground point `gp` (camera-relative) towards the sun.
fn cloud_shadow(field: &CloudField<'_>, planet: &Planet, cam: DVec3, alt0: f64, gp: DVec3, sun: Vec3) -> f32 {
    let (lo, hi) = field.bounds();
    if hi <= lo {
        return 1.0;
    }
    let g_alt = planet.altitude(alt0, gp);
    let sd = sun.as_dvec3();
    let Some((t0, t1)) = planet.shell(g_alt, sd, lo as f64, hi as f64, 60_000.0) else { return 1.0 };
    let n = 24;
    let dt = (t1 - t0) / n as f64;
    let mut tau = 0.0f32;
    for i in 0..n {
        let q = gp + sd * (t0 + (i as f64 + 0.5) * dt);
        let hh = planet.altitude(alt0, q) as f32;
        let s = field.sample(cam.x + q.x, cam.z + q.z, hh, Lod::COARSE);
        // Rain shafts do not count for the sun.
        if s.albedo >= 1.0 {
            tau += s.sigma_t * dt as f32;
        }
    }
    (-tau).exp()
}

fn save(path: &std::path::Path, px: &[u8], w: u32, h: u32) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(px).unwrap();
}
