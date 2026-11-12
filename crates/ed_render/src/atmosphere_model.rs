//! Physically based atmosphere, CPU side (Phase 2 W5): the per-body constants and an f64 mirror
//! of `shaders/atmos.slang` + `shaders/atmos_tables.slang` (same parameterisations, the same
//! table sizes and step counts), used for the sun colour in `FrameData`, the unit tests and the
//! calibration numbers. Engine-free and deterministic.
//!
//! Model: Hillaire 2020 ("A Scalable and Production Ready Sky and Atmosphere Rendering
//! Technique"): Rayleigh, Mie (Cornette-Shanks) and an ozone tent, a transmittance table in
//! Bruneton's (h, mu) parameterisation, a multiple-scattering table (isotropic second order,
//! summed as a geometric series: Psi = L2 / (1 - f_ms)), ray-marched sky radiance with analytic
//! per-segment integration, a Lambert ground lit by the sun and the multiple-scattered sky.
//! Lengths in km, coefficients in 1/km, radiance in renderer units (light.rs: 1 unit is about
//! 15.6 kcd/m2).

use glam::DVec3;
use std::f64::consts::PI;

/// Constants of one body's atmosphere (geometry, medium, sun). A body without an atmosphere is
/// the same shell with zero coefficients: transmittance 1, a black sky, the lit ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    /// Ground radius (km).
    pub radius_km: f64,
    /// Radius of the top of the atmosphere (km).
    pub top_km: f64,
    /// Rayleigh scattering at the ground (1/km) and its scale height (km).
    pub rayleigh: [f64; 3],
    pub rayleigh_h_km: f64,
    /// Mie scattering and absorption at the ground (1/km), scale height (km), phase asymmetry.
    pub mie_scattering: [f64; 3],
    pub mie_absorption: [f64; 3],
    pub mie_h_km: f64,
    pub mie_g: f64,
    /// Ozone absorption at its peak (1/km), a tent of `ozone_half_km` around `ozone_peak_km`.
    pub ozone: [f64; 3],
    pub ozone_peak_km: f64,
    pub ozone_half_km: f64,
    /// Sun irradiance at the top of the atmosphere (renderer units).
    pub sun_irradiance: [f64; 3],
    /// The sun disc's angular radius (rad).
    pub sun_radius: f64,
}

impl Body {
    /// Earth. Rayleigh and ozone are Hillaire 2020 / Bruneton 2017. Mie is a real clear day,
    /// not their near-aerosol-free default (AOD 0.005, which leaves skylight at 9 % of the sun
    /// and a saturated R : B 0.25): aerosol optical depth 0.05 at 550 nm with an Angstrom
    /// exponent of 1.3 (blue scatters 1.34x, red 0.76x), single-scattering albedo 0.92, scale
    /// height 1.2 km (meteorological visibility about 70 km, so ridges 30 km out still read).
    /// On open ground at the default sun that gives skylight : sun 0.15 (measured clear days:
    /// 0.12..0.18; M4e's hand-set 0.20 is a hazier day, AOD 0.084, which washed 30 km ridges
    /// out to the horizon sky) and skylight R : B 0.38 (M4e 0.50).
    /// The sun is white at the top of the atmosphere, 7.87 units: it reaches the ground at the
    /// luminance M4e calibrated (6.16 units for 100 klx at 47 degrees up), so exposure 0.7
    /// still meters it ([`crate::light::EXPOSURE`]); on that scale 7.87 units is 123 klx, 4 %
    /// under the photometric solar constant (128 klx).
    pub fn earth() -> Self {
        Self {
            radius_km: 6360.0,
            top_km: 6460.0,
            rayleigh: [5.802e-3, 13.558e-3, 33.1e-3],
            rayleigh_h_km: 8.0,
            mie_scattering: [0.0324, 0.042, 0.0564],
            mie_absorption: [0.0036; 3],
            mie_h_km: 1.2,
            mie_g: 0.8,
            ozone: [0.650e-3, 1.881e-3, 0.085e-3],
            ozone_peak_km: 25.0,
            ozone_half_km: 15.0,
            sun_irradiance: [7.87; 3],
            sun_radius: 0.004654,
        }
    }

    /// The Moon: no atmosphere (a vacuum shell), the same sun.
    pub fn moon() -> Self {
        Self {
            radius_km: 1737.4,
            top_km: 1837.4,
            rayleigh: [0.0; 3],
            mie_scattering: [0.0; 3],
            mie_absorption: [0.0; 3],
            ozone: [0.0; 3],
            ..Self::earth()
        }
    }

    pub fn has_atmosphere(&self) -> bool {
        let s: f64 = self.rayleigh.iter().chain(&self.mie_scattering).chain(&self.mie_absorption).chain(&self.ozone).sum();
        s > 0.0 && self.top_km > self.radius_km
    }

    pub fn medium(&self, h: f64) -> Medium {
        let h = h.max(0.0);
        let rd = (-h / self.rayleigh_h_km).exp();
        let md = (-h / self.mie_h_km).exp();
        let od = (1.0 - (h - self.ozone_peak_km).abs() / self.ozone_half_km).max(0.0);
        let v = |a: [f64; 3]| DVec3::from_array(a);
        let ray = v(self.rayleigh) * rd;
        let mie = v(self.mie_scattering) * md;
        let scat = ray + mie;
        Medium { ray, mie, scat, ext: scat + v(self.mie_absorption) * md + v(self.ozone) * od }
    }

    pub fn sun(&self) -> DVec3 {
        DVec3::from_array(self.sun_irradiance)
    }

    fn ht(&self) -> f64 {
        self.top_km - self.radius_km
    }
}

/// The medium at one altitude.
#[derive(Clone, Copy, Debug)]
pub struct Medium {
    pub ray: DVec3,
    pub mie: DVec3,
    pub scat: DVec3,
    pub ext: DVec3,
}

/// Altitude at distance t along a ray from altitude h with zenith cosine mu.
pub fn height_at(rb: f64, h: f64, mu: f64, t: f64) -> f64 {
    let r = rb + h;
    let q = t * t + 2.0 * r * mu * t + h * (2.0 * rb + h);
    q / ((rb * rb + q).max(0.0).sqrt() + rb)
}

pub fn hits_ground(rb: f64, h: f64, mu: f64) -> bool {
    let rm = (rb + h) * mu;
    mu < 0.0 && rm * rm >= h * (2.0 * rb + h)
}

pub fn dist_ground(rb: f64, h: f64, mu: f64) -> f64 {
    let a = -(rb + h) * mu;
    let c = h * (2.0 * rb + h);
    c / (a + (a * a - c).max(0.0).sqrt()).max(1e-12)
}

pub fn dist_top(rb: f64, rt: f64, h: f64, mu: f64) -> f64 {
    let r = rb + h;
    let d = (r * mu) * (r * mu) + ((rt - rb) - h) * (rt + r);
    (-r * mu + d.max(0.0).sqrt()).max(0.0)
}

pub fn cos_horizon(rb: f64, h: f64) -> f64 {
    -(h * (2.0 * rb + h)).max(0.0).sqrt() / (rb + h)
}

pub fn phase_rayleigh(nu: f64) -> f64 {
    3.0 / (16.0 * PI) * (1.0 + nu * nu)
}

pub fn phase_mie(g: f64, nu: f64) -> f64 {
    let k = 3.0 / (8.0 * PI) * (1.0 - g * g) / (2.0 + g * g);
    k * (1.0 + nu * nu) / (1.0 + g * g - 2.0 * g * nu).max(1e-4).powf(1.5)
}

/// Share of the sun's disc above the horizon at altitude h (atmos.slang `atm_earth_shadow`).
pub fn earth_shadow(rb: f64, h: f64, mu_s: f64) -> f64 {
    let ch = cos_horizon(rb, h);
    let s = 0.0047 * (1.0 - ch * ch).max(0.0).sqrt() + 1e-5;
    let x = ((mu_s - ch + s) / (2.0 * s)).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Optical depth from altitude h towards mu up to the top of the atmosphere (`steps` midpoint
/// samples, quadratically spaced as in `atmos_transmittance`).
pub fn optical_depth(b: &Body, h: f64, mu: f64, steps: u32) -> DVec3 {
    let d = dist_top(b.radius_km, b.top_km, h, mu);
    let (mut od, mut prev) = (DVec3::ZERO, 0.0);
    for i in 0..steps {
        let x = (i + 1) as f64 / steps as f64;
        let t1 = d * x * x;
        od += b.medium(height_at(b.radius_km, h, mu, 0.5 * (prev + t1))).ext * (t1 - prev);
        prev = t1;
    }
    od
}

/// Sun irradiance at altitude h with the sun at zenith cosine mu_s: through the atmosphere,
/// with the Earth's shadow. FrameData's `sun_color` (the sun at the camera).
pub fn sun_at(b: &Body, h: f64, mu_s: f64) -> DVec3 {
    let t = (-optical_depth(b, h.min(b.ht()), mu_s, 64)).exp();
    b.sun() * t * earth_shadow(b.radius_km, h, mu_s)
}

pub const TRANS_W: usize = 256;
pub const TRANS_H: usize = 64;
pub const MS_N: usize = 32;
const TRANS_STEPS: u32 = 40;
const MS_DIRS: usize = 8;
const MS_STEPS: u32 = 20;

/// The transmittance and multiple-scattering tables, built as the GPU builds them.
pub struct Tables {
    pub body: Body,
    pub albedo: DVec3,
    pub sunlit: f64,
    pub trans: Vec<DVec3>,
    pub ms: Vec<DVec3>,
    /// Per multiple-scattering texel: (L2, f_ms) before the series (tests).
    pub ms_terms: Vec<(DVec3, DVec3)>,
}

fn bilinear(buf: &[DVec3], w: usize, rows: usize, u: f64, v: f64) -> DVec3 {
    let px = u.clamp(0.0, 1.0) * (w - 1) as f64;
    let py = v.clamp(0.0, 1.0) * (rows - 1) as f64;
    let (ix, iy) = ((px as usize).min(w - 2), (py as usize).min(rows - 2));
    let (gx, gy) = (px - ix as f64, py - iy as f64);
    let b = iy * w + ix;
    let a = buf[b].lerp(buf[b + 1], gx);
    let c = buf[b + w].lerp(buf[b + w + 1], gx);
    a.lerp(c, gy)
}

impl Tables {
    pub fn new(body: &Body, albedo: DVec3, sunlit: f64) -> Self {
        let b = *body;
        let (rb, rt) = (b.radius_km, b.top_km);
        let ht = rt - rb;
        let big_h = (ht * (rt + rb)).sqrt();
        let mut trans = Vec::with_capacity(TRANS_W * TRANS_H);
        for y in 0..TRANS_H {
            for x in 0..TRANS_W {
                let (u, v) = (x as f64 / (TRANS_W - 1) as f64, y as f64 / (TRANS_H - 1) as f64);
                let rho = v * big_h;
                let h = rho * rho / ((rho * rho + rb * rb).sqrt() + rb);
                let r = rb + h;
                let (dmin, dmax) = (ht - h, rho + big_h);
                let d = dmin + u * (dmax - dmin);
                let mu = if d > 1e-6 { (((ht - h) * (rt + r) - d * d) / (2.0 * r * d)).clamp(-1.0, 1.0) } else { 1.0 };
                let (mut od, mut prev) = (DVec3::ZERO, 0.0);
                for i in 0..TRANS_STEPS {
                    let x = (i + 1) as f64 / TRANS_STEPS as f64;
                    let t1 = d * x * x;
                    od += b.medium(height_at(rb, h, mu, 0.5 * (prev + t1))).ext * (t1 - prev);
                    prev = t1;
                }
                trans.push((-od).exp());
            }
        }
        let mut t = Self { body: b, albedo, sunlit, trans, ms: Vec::new(), ms_terms: Vec::new() };
        let mut ms = Vec::with_capacity(MS_N * MS_N);
        let mut terms = Vec::with_capacity(MS_N * MS_N);
        for y in 0..MS_N {
            for x in 0..MS_N {
                let mu_s = x as f64 / (MS_N - 1) as f64 * 2.0 - 1.0;
                let h = (y as f64 / (MS_N - 1) as f64 * ht).max(1e-3);
                let (l2, fms) = t.ms_texel(h, mu_s);
                terms.push((l2, fms));
                ms.push(l2 / (DVec3::ONE - fms).max(DVec3::splat(1e-3)));
            }
        }
        t.ms = ms;
        t.ms_terms = terms;
        t
    }

    fn ms_texel(&self, h: f64, mu_s: f64) -> (DVec3, DVec3) {
        let b = &self.body;
        let rb = b.radius_km;
        let ss = (1.0 - mu_s * mu_s).max(0.0).sqrt();
        let (mut lsum, mut fsum) = (DVec3::ZERO, DVec3::ZERO);
        for i in 0..MS_DIRS {
            let mu = 1.0 - 2.0 * (i as f64 + 0.5) / MS_DIRS as f64;
            let sv = (1.0 - mu * mu).max(0.0).sqrt();
            let ground = hits_ground(rb, h, mu);
            let tmax = if ground { dist_ground(rb, h, mu) } else { dist_top(rb, b.top_km, h, mu) };
            for j in 0..MS_DIRS {
                let ph = 2.0 * PI * (j as f64 + 0.5) / MS_DIRS as f64;
                let nu = mu * mu_s + sv * ss * ph.cos();
                let (mut thr, mut l, mut fms, mut prev) = (DVec3::ONE, DVec3::ZERO, DVec3::ZERO, 0.0);
                for k in 0..MS_STEPS {
                    let x = (k + 1) as f64 / MS_STEPS as f64;
                    let t1 = tmax * x * x;
                    let (dt, tm) = (t1 - prev, 0.5 * (prev + t1));
                    prev = t1;
                    let hh = height_at(rb, h, mu, tm);
                    let mus = (((rb + h) * mu_s + tm * nu) / (rb + hh)).clamp(-1.0, 1.0);
                    let m = b.medium(hh);
                    let st = (-m.ext * dt).exp();
                    let e = m.ext.max(DVec3::splat(1e-9));
                    let s = self.sun_transmittance(hh, mus) * m.scat / (4.0 * PI);
                    l += thr * (s - s * st) / e;
                    fms += thr * (m.scat - m.scat * st) / e;
                    thr *= st;
                }
                if ground {
                    let mus = (((rb + h) * mu_s + tmax * nu) / rb).clamp(-1.0, 1.0);
                    l += thr * self.sun_transmittance(0.0, mus) * (mus.max(0.0) * self.sunlit) * self.albedo / PI;
                }
                lsum += l;
                fsum += fms;
            }
        }
        let n = (MS_DIRS * MS_DIRS) as f64;
        (lsum / n, fsum / n)
    }

    pub fn transmittance(&self, h: f64, mu: f64) -> DVec3 {
        let (rb, rt) = (self.body.radius_km, self.body.top_km);
        let ht = rt - rb;
        let h = h.clamp(0.0, ht);
        let big_h = (ht * (rt + rb)).sqrt();
        let rho = (h * (2.0 * rb + h)).max(0.0).sqrt();
        let d = dist_top(rb, rt, h, mu);
        let (dmin, dmax) = (ht - h, rho + big_h);
        bilinear(&self.trans, TRANS_W, TRANS_H, (d - dmin) / (dmax - dmin).max(1e-6), rho / big_h)
    }

    pub fn sun_transmittance(&self, h: f64, mu_s: f64) -> DVec3 {
        self.transmittance(h, mu_s) * earth_shadow(self.body.radius_km, h, mu_s)
    }

    pub fn multiscatter(&self, h: f64, mu_s: f64) -> DVec3 {
        bilinear(&self.ms, MS_N, MS_N, mu_s * 0.5 + 0.5, h / self.body.ht())
    }

    /// Radiance leaving the lit ground where a ray from (h0, mu_s, nu) lands after t.
    pub fn ground_radiance(&self, h0: f64, mu_s: f64, nu: f64, t: f64) -> DVec3 {
        let rb = self.body.radius_km;
        let mus = (((rb + h0) * mu_s + t * nu) / rb).clamp(-1.0, 1.0);
        let e = self.sun_transmittance(0.0, mus) * (mus.max(0.0) * self.sunlit) + self.multiscatter(0.0, mus) * PI;
        self.body.sun() * e * self.albedo / PI
    }

    /// Sky radiance and transmittance along a ray from altitude h (inside the atmosphere) up to
    /// distance `tmax` (or to the ground / top when `None`), `steps` quadratic segments; the lit
    /// ground is added when the ray reaches it.
    pub fn march(&self, h: f64, mu: f64, mu_s: f64, nu: f64, steps: u32, tmax: Option<f64>) -> (DVec3, DVec3) {
        let b = &self.body;
        let rb = b.radius_km;
        let ground = hits_ground(rb, h, mu);
        let tend = if ground { dist_ground(rb, h, mu) } else { dist_top(rb, b.top_km, h, mu) };
        let tm_ = tmax.map_or(tend, |t| t.min(tend));
        let (pr, pm) = (phase_rayleigh(nu), phase_mie(b.mie_g, nu));
        let (mut l, mut tr, mut prev) = (DVec3::ZERO, DVec3::ONE, 0.0);
        for i in 0..steps {
            let x = (i + 1) as f64 / steps as f64;
            let t1 = tm_ * x * x;
            let (dt, tm) = (t1 - prev, 0.5 * (prev + t1));
            prev = t1;
            let hh = height_at(rb, h, mu, tm);
            let mus = (((rb + h) * mu_s + tm * nu) / (rb + hh)).clamp(-1.0, 1.0);
            let m = b.medium(hh);
            let st = (-m.ext * dt).exp();
            let s = b.sun() * (self.sun_transmittance(hh, mus) * (m.ray * pr + m.mie * pm) + self.multiscatter(hh, mus) * m.scat);
            l += tr * (s - s * st) / m.ext.max(DVec3::splat(1e-9));
            tr *= st;
        }
        if ground && tmax.is_none_or(|t| t >= tend) {
            l += tr * self.ground_radiance(h, mu_s, nu, tend);
        }
        (l, tr)
    }

    /// Sky radiance towards (mu, nu) from altitude h; above the atmosphere the ray first moves
    /// to where it enters it (atmos.slang `atm_sky_ray`).
    pub fn sky(&self, h: f64, mu: f64, mu_s: f64, nu: f64, steps: u32) -> DVec3 {
        let b = &self.body;
        let (rb, rt) = (b.radius_km, b.top_km);
        let (mut h, mut mu, mut mu_s) = (h, mu, mu_s);
        if h > rt - rb {
            let r = rb + h;
            let d = (r * mu) * (r * mu) - (h - (rt - rb)) * (r + rt);
            if d < 0.0 {
                return DVec3::ZERO;
            }
            let te = -r * mu - d.sqrt();
            if te < 0.0 {
                return DVec3::ZERO;
            }
            mu = ((r * mu + te) / rt).clamp(-1.0, 1.0);
            mu_s = ((r * mu_s + te * nu) / rt).clamp(-1.0, 1.0);
            h = rt - rb;
        }
        self.march(h, mu, mu_s, nu, steps, None).0
    }

    /// Sky radiance in world direction `d` (y up) for a sun towards `sun` from altitude h.
    pub fn sky_dir(&self, h: f64, d: DVec3, sun: DVec3, steps: u32) -> DVec3 {
        self.sky(h, d.y.clamp(-1.0, 1.0), sun.y, d.dot(sun).clamp(-1.0, 1.0), steps)
    }
}

/// Irradiance on a plane facing `n` from the sky radiance `rad` (no sun disc), by the same
/// equal-area grid as light.rs `sh9_of`.
pub fn irradiance(n: DVec3, rad: impl Fn(DVec3) -> DVec3, nt: usize, np: usize) -> DVec3 {
    let mut e = DVec3::ZERO;
    for i in 0..nt {
        let ct = 1.0 - 2.0 * (i as f64 + 0.5) / nt as f64;
        let st = (1.0 - ct * ct).max(0.0).sqrt();
        for j in 0..np {
            let ph = (j as f64 + 0.5) / np as f64 * 2.0 * PI;
            let d = DVec3::new(st * ph.cos(), ct, st * ph.sin());
            let c = d.dot(n);
            if c > 0.0 {
                e += rad(d) * c;
            }
        }
    }
    e * (4.0 * PI / (nt * np) as f64)
}

pub fn luminance(c: DVec3) -> f64 {
    c.dot(DVec3::new(0.2126, 0.7152, 0.0722))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth_tables(albedo: f64) -> Tables {
        Tables::new(&Body::earth(), DVec3::splat(albedo), 1.0)
    }

    fn default_sun() -> DVec3 {
        DVec3::new(0.45, 0.62, 0.35).normalize()
    }

    fn sun_elev(deg: f64) -> DVec3 {
        let e = deg.to_radians();
        DVec3::new(e.cos(), e.sin(), 0.0)
    }

    #[test]
    fn transmittance_table_is_bounded_and_matches_direct_integration() {
        let t = earth_tables(0.0);
        assert!(t.trans.iter().all(|v| v.cmpgt(DVec3::ZERO).all() && v.cmple(DVec3::ONE).all()));
        for (h, mu) in [(0.0, 1.0), (0.0017, 0.7), (1.0, 0.2), (10.0, 0.05), (30.0, -0.05), (80.0, 0.5)] {
            let direct = (-optical_depth(&t.body, h, mu, 400)).exp();
            let lut = t.transmittance(h, mu);
            let e = (lut - direct).abs().max_element();
            assert!(e < 0.01, "h {h} mu {mu}: table {lut:?} direct {direct:?}");
        }
        // Zenith at sea level: blue loses most (Rayleigh), red least.
        let z = t.transmittance(0.0, 1.0);
        assert!(z.x > z.y && z.y > z.z && z.z > 0.6, "{z:?}");
        // Darker towards the horizon.
        assert!(t.transmittance(0.0, 0.05).z < 0.2 * z.z);
    }

    #[test]
    fn multiple_scattering_series_converges() {
        let t = earth_tables(0.3);
        for &(l2, f) in &t.ms_terms {
            assert!(f.cmpge(DVec3::ZERO).all() && f.cmplt(DVec3::splat(0.6)).all(), "f_ms {f:?}");
            assert!(l2.cmpge(DVec3::ZERO).all());
        }
        // Blue scatters most, so its series is the strongest.
        let (_, f0) = t.ms_terms[MS_N * MS_N / 2 + MS_N - 1];
        assert!(f0.z > f0.x, "{f0:?}");
    }

    #[test]
    fn energy_is_conserved() {
        // Non-absorbing medium, black ground: what reaches the ground plus what leaves the top
        // equals what came in (within the isotropic multiple-scattering approximation).
        let mut body = Body::earth();
        body.ozone = [0.0; 3];
        body.mie_absorption = [0.0; 3];
        let t = Tables::new(&body, DVec3::ZERO, 1.0);
        let sun = sun_elev(40.0);
        let e_in = body.sun() * sun.y;
        let direct = sun_at(&body, 0.0, sun.y) * sun.y;
        let down = irradiance(DVec3::Y, |d| t.sky_dir(0.0, d, sun, 24), 24, 48);
        let top = body.top_km - body.radius_km - 1e-3;
        let up = irradiance(DVec3::NEG_Y, |d| t.sky_dir(top, d, sun, 24), 24, 48);
        let sum = direct + down + up;
        for i in 0..3 {
            let r = sum[i] / e_in[i];
            assert!((0.9..1.03).contains(&r), "channel {i}: in {e_in:?} ground {direct:?} + {down:?}, space {up:?} (ratio {r})");
        }
        // With absorption (ozone, Mie) and a grey ground nothing is created.
        let t = earth_tables(0.3);
        let b = Body::earth();
        let direct = sun_at(&b, 0.0, sun.y) * sun.y;
        let down = irradiance(DVec3::Y, |d| t.sky_dir(0.0, d, sun, 24), 24, 48);
        assert!((direct + down).cmple(b.sun() * sun.y).all());
    }

    #[test]
    fn zenith_deep_blue_horizon_pale_sunset_red() {
        let t = earth_tables(0.2);
        let sun = default_sun();
        let az = DVec3::new(-sun.x, 0.0, -sun.z).normalize(); // away from the sun
        let zen = t.sky_dir(0.0017, DVec3::Y, sun, 30);
        let hor = t.sky_dir(0.0017, (az + DVec3::Y * 0.03).normalize(), sun, 30);
        let sat = |c: DVec3| 1.0 - c.min_element() / c.max_element();
        assert!(zen.z > zen.x * 2.5, "zenith {zen:?}");
        assert!(sat(hor) < sat(zen) && luminance(hor) > luminance(zen), "horizon {hor:?} zenith {zen:?}");
        // Sun 2 degrees up: the sun is red and the sky around it warm.
        let low = sun_elev(2.0);
        let s = sun_at(&t.body, 0.0017, low.y);
        assert!(s.x > 2.0 * s.z, "low sun {s:?}");
        let near = t.sky_dir(0.0017, (low + DVec3::Y * 0.02).normalize(), low, 30);
        assert!(near.x > near.z, "sky by the low sun {near:?}");
        // 400 km up: the sky away from the Earth is black.
        assert_eq!(t.sky_dir(400.0, DVec3::Y, sun, 30), DVec3::ZERO);
    }

    #[test]
    fn sun_keeps_the_m4e_calibration() {
        let b = Body::earth();
        let s = sun_at(&b, 0.0017, default_sun().y);
        let m4e = crate::light::Sky::for_sun(default_sun().as_vec3()).sun_color.as_dvec3();
        let r = luminance(s) / luminance(m4e);
        assert!((0.97..1.03).contains(&r), "sun {s:?} vs M4e {m4e:?}");
    }

    /// Skylight against the sun on open ground (the M4e calibration point: the analytic sky
    /// with gain 2.1 gave sky : sun 0.20 at the default sun; clear-sky measurements give
    /// 0.15-0.25). Prints the numbers with `--nocapture`.
    #[test]
    fn skylight_to_sun_ratio_is_physical() {
        let lighting = crate::light::LightingSettings::default();
        let a = lighting.bounce_albedo.as_dvec3();
        let t = Tables::new(&Body::earth(), a, lighting.bounce_sunlit as f64);
        let sun = default_sun();
        let h = 0.002;
        let direct = sun_at(&t.body, h, sun.y) * sun.y;
        let sky = irradiance(DVec3::Y, |d| t.sky_dir(h, d, sun, 20), 32, 64);
        let r = luminance(sky) / luminance(direct);
        let ana = crate::light::Sky::for_sun(sun.as_vec3());
        let sh = crate::light::ambient_sh9(&ana, &lighting);
        let m4e_sky = crate::light::sh_eval(&sh, glam::Vec3::Y).as_dvec3() * PI;
        let m4e_direct = ana.sun_color.as_dvec3() * sun.y;
        let below = irradiance(DVec3::NEG_Y, |d| t.sky_dir(h, d, sun, 20), 32, 64);
        let m4e_below = crate::light::sh_eval(&sh, glam::Vec3::NEG_Y).as_dvec3() * PI;
        eprintln!("sun at ground  atmos {direct:.3?} (lum {:.3})  M4e {m4e_direct:.3?} (lum {:.3})", luminance(direct), luminance(m4e_direct));
        eprintln!("sky irradiance atmos {sky:.3?} (lum {:.3}, R:B {:.2})  M4e {m4e_sky:.3?} (lum {:.3}, R:B {:.2})", luminance(sky), sky.x / sky.z, luminance(m4e_sky), m4e_sky.x / m4e_sky.z);
        eprintln!("sky : sun      atmos {r:.3}  M4e {:.3}", luminance(m4e_sky) / luminance(m4e_direct));
        eprintln!("ground bounce  atmos {below:.3?} (lum {:.3})  M4e {m4e_below:.3?} (lum {:.3})", luminance(below), luminance(m4e_below));
        assert!((0.1..0.35).contains(&r), "sky : sun {r}");
        assert!(sky.z > sky.x, "skylight is blue {sky:?}");
    }

    #[test]
    fn moon_has_no_sky() {
        let t = Tables::new(&Body::moon(), DVec3::splat(0.12), 1.0);
        assert!(!t.body.has_atmosphere() && Body::earth().has_atmosphere());
        let sun = default_sun();
        assert_eq!(t.transmittance(0.0, 0.1), DVec3::ONE);
        assert_eq!(t.sky_dir(0.0017, DVec3::Y, sun, 16), DVec3::ZERO);
        assert_eq!(sun_at(&t.body, 0.0, sun.y), t.body.sun());
    }
}
