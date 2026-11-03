//! The solar system, where it really is right now.
//!
//! Planets: JPL "Approximate Positions of the Planets" Keplerian elements
//! (valid 1800-2050, error ~arcminutes, plenty for a game sky).
//! Moon: the Astronomical Almanac low-precision series (~0.3°, ~0.2% range).
//! Output is in Earth-centered Earth-fixed meters, the same frame `ed_geo`
//! uses, so "look up and fly to the Moon" is just vector math.
//!
//! Coordinates beyond Earth: bodies are tracked in f64 meters relative to
//! Earth's center. That's ~0.5 mm precision at the Moon and ~1 m at Neptune;
//! good enough because the renderer only ever draws relative to the camera.

use ed_geo::Geodetic;
use glam::DVec3;

pub const AU: f64 = 1.495_978_707e11;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyId {
    Sun,
    Mercury,
    Venus,
    Earth,
    Moon,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
}

#[derive(Debug, Clone, Copy)]
pub struct BodyInfo {
    pub id: BodyId,
    pub name: &'static str,
    pub radius_m: f64,
    /// GM (m^3/s^2).
    pub mu: f64,
    /// Stylized base color (sRGB hex) for distant rendering and HUD.
    pub color: u32,
    /// Has an atmosphere worth rendering.
    pub atmosphere: bool,
    /// Ring inner/outer radius (m), if any.
    pub rings: Option<(f64, f64)>,
    /// Sidereal rotation period (hours, negative = retrograde).
    pub rotation_h: f64,
}

pub const BODIES: &[BodyInfo] = &[
    BodyInfo { id: BodyId::Sun, name: "Sun", radius_m: 6.9634e8, mu: 1.327_124_4e20, color: 0xfff4d6, atmosphere: false, rings: None, rotation_h: 609.12 },
    BodyInfo { id: BodyId::Mercury, name: "Mercury", radius_m: 2.4397e6, mu: 2.2032e13, color: 0x9e9a94, atmosphere: false, rings: None, rotation_h: 1407.6 },
    BodyInfo { id: BodyId::Venus, name: "Venus", radius_m: 6.0518e6, mu: 3.248_59e14, color: 0xe8d3a1, atmosphere: true, rings: None, rotation_h: -5832.5 },
    BodyInfo { id: BodyId::Earth, name: "Earth", radius_m: 6.371e6, mu: ed_geo::EARTH_MU, color: 0x4f86c6, atmosphere: true, rings: None, rotation_h: 23.9345 },
    BodyInfo { id: BodyId::Moon, name: "Moon", radius_m: 1.7374e6, mu: 4.904_869_5e12, color: 0xc9c6bf, atmosphere: false, rings: None, rotation_h: 655.72 },
    BodyInfo { id: BodyId::Mars, name: "Mars", radius_m: 3.3895e6, mu: 4.282_837e13, color: 0xc8643b, atmosphere: true, rings: None, rotation_h: 24.6229 },
    BodyInfo { id: BodyId::Jupiter, name: "Jupiter", radius_m: 6.9911e7, mu: 1.266_865_34e17, color: 0xd8b48a, atmosphere: true, rings: None, rotation_h: 9.925 },
    BodyInfo { id: BodyId::Saturn, name: "Saturn", radius_m: 5.8232e7, mu: 3.793_118_7e16, color: 0xe3cf9a, atmosphere: true, rings: Some((7.45e7, 1.4022e8)), rotation_h: 10.656 },
    BodyInfo { id: BodyId::Uranus, name: "Uranus", radius_m: 2.5362e7, mu: 5.793_939e15, color: 0xa7d8de, atmosphere: true, rings: Some((3.8e7, 5.1e7)), rotation_h: -17.24 },
    BodyInfo { id: BodyId::Neptune, name: "Neptune", radius_m: 2.4622e7, mu: 6.836_529e15, color: 0x4f6fd6, atmosphere: true, rings: None, rotation_h: 16.11 },
];

pub fn info(id: BodyId) -> &'static BodyInfo {
    BODIES.iter().find(|b| b.id == id).expect("all bodies listed")
}

/// JPL Table 1 elements at J2000 and per-century rates:
/// a (AU), e, I, L, long. perihelion, long. ascending node (degrees).
type Elements = [f64; 6];
const KEPLER: &[(BodyId, Elements, Elements)] = &[
    (BodyId::Mercury, [0.387_099_27, 0.205_635_93, 7.004_979_02, 252.250_323_50, 77.457_796_28, 48.330_765_93],
        [0.000_000_37, 0.000_019_06, -0.005_947_49, 149_472.674_111_75, 0.160_476_89, -0.125_340_81]),
    (BodyId::Venus, [0.723_335_66, 0.006_776_72, 3.394_676_05, 181.979_099_50, 131.602_467_18, 76.679_842_55],
        [0.000_003_90, -0.000_041_07, -0.000_788_90, 58_517.815_387_29, 0.002_683_29, -0.277_694_18]),
    (BodyId::Earth, [1.000_002_61, 0.016_711_23, -0.000_015_31, 100.464_571_66, 102.937_681_93, 0.0],
        [0.000_005_62, -0.000_043_92, -0.012_946_68, 35_999.372_449_81, 0.323_273_64, 0.0]),
    (BodyId::Mars, [1.523_710_34, 0.093_394_10, 1.849_691_42, -4.553_432_05, -23.943_629_59, 49.559_538_91],
        [0.000_018_47, 0.000_078_82, -0.008_131_31, 19_140.302_684_99, 0.444_410_88, -0.292_573_43]),
    (BodyId::Jupiter, [5.202_887_00, 0.048_386_24, 1.304_396_95, 34.396_440_51, 14.728_479_83, 100.473_909_09],
        [-0.000_116_07, -0.000_132_53, -0.001_837_14, 3_034.746_127_75, 0.212_526_68, 0.204_691_06]),
    (BodyId::Saturn, [9.536_675_94, 0.053_861_79, 2.485_991_87, 49.954_244_23, 92.598_878_31, 113.662_424_48],
        [-0.001_250_60, -0.000_509_91, 0.001_936_09, 1_222.493_622_01, -0.418_972_16, -0.288_677_94]),
    (BodyId::Uranus, [19.189_164_64, 0.047_257_44, 0.772_637_83, 313.238_104_51, 170.954_276_30, 74.016_925_03],
        [-0.001_961_76, -0.000_043_97, -0.002_429_39, 428.482_027_85, 0.408_052_81, 0.042_405_89]),
    (BodyId::Neptune, [30.069_922_76, 0.008_590_48, 1.770_043_47, -55.120_029_69, 44.964_762_27, 131.784_225_74],
        [0.000_262_91, 0.000_051_05, 0.000_353_72, 218.459_453_25, -0.322_414_64, -0.005_086_64]),
];

fn julian_centuries(unix_s: f64) -> f64 {
    (unix_s / 86_400.0 + 2_440_587.5 - 2_451_545.0) / 36_525.0
}

/// Heliocentric ecliptic J2000 position in AU.
fn heliocentric_ecliptic(id: BodyId, t: f64) -> DVec3 {
    let Some((_, e0, rate)) = KEPLER.iter().find(|k| k.0 == id) else { return DVec3::ZERO };
    let el: Vec<f64> = (0..6).map(|i| e0[i] + rate[i] * t).collect();
    let (a, e, inc, l, varpi, node) = (el[0], el[1], el[2].to_radians(), el[3], el[4], el[5]);
    let omega = (varpi - node).to_radians();
    let node = node.to_radians();
    let m = ((l - varpi).rem_euclid(360.0)).to_radians();
    // Kepler's equation by Newton iteration.
    let mut ea = m + e * m.sin();
    for _ in 0..12 {
        let d = (ea - e * ea.sin() - m) / (1.0 - e * ea.cos());
        ea -= d;
        if d.abs() < 1e-12 {
            break;
        }
    }
    let xp = a * (ea.cos() - e);
    let yp = a * (1.0 - e * e).sqrt() * ea.sin();
    let (co, so) = (omega.cos(), omega.sin());
    let (cn, sn) = (node.cos(), node.sin());
    let (ci, si) = (inc.cos(), inc.sin());
    DVec3::new(
        (co * cn - so * sn * ci) * xp + (-so * cn - co * sn * ci) * yp,
        (co * sn + so * cn * ci) * xp + (-so * sn + co * cn * ci) * yp,
        (so * si) * xp + (co * si) * yp,
    )
}

/// Geocentric ecliptic position of the Moon, meters.
fn moon_geocentric_ecliptic(t: f64) -> DVec3 {
    let s = |a: f64, b: f64| (a + b * t).to_radians().sin();
    let c = |a: f64, b: f64| (a + b * t).to_radians().cos();
    let lambda = 218.32 + 481_267.881 * t + 6.29 * s(135.0, 477_198.87) - 1.27 * s(259.3, -413_335.36)
        + 0.66 * s(235.7, 890_534.22)
        + 0.21 * s(269.9, 954_397.74)
        - 0.19 * s(357.5, 35_999.05)
        - 0.11 * s(186.5, 966_404.03);
    let beta = 5.13 * s(93.3, 483_202.02) + 0.28 * s(228.2, 960_400.89) - 0.28 * s(318.3, 6_003.15) - 0.17 * s(217.6, -407_332.21);
    let parallax = 0.9508 + 0.0518 * c(135.0, 477_198.87) + 0.0095 * c(259.3, -413_335.36) + 0.0078 * c(235.7, 890_534.22)
        + 0.0028 * c(269.9, 954_397.74);
    let r = 6_378_140.0 / parallax.to_radians().sin();
    let (l, b) = (lambda.to_radians(), beta.to_radians());
    DVec3::new(r * b.cos() * l.cos(), r * b.cos() * l.sin(), r * b.sin())
}

/// Ecliptic -> equatorial (rotate about X by the obliquity).
fn ecliptic_to_equatorial(v: DVec3, t: f64) -> DVec3 {
    let eps = (23.439_291 - 0.013_004_2 * t).to_radians();
    let (c, s) = (eps.cos(), eps.sin());
    DVec3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z)
}

/// Greenwich mean sidereal time (radians).
pub fn gmst(unix_s: f64) -> f64 {
    let d = unix_s / 86_400.0 + 2_440_587.5 - 2_451_545.0;
    (280.460_618_37 + 360.985_647_366_29 * d).rem_euclid(360.0).to_radians()
}

/// Inertial (equatorial) -> ECEF.
fn eci_to_ecef(v: DVec3, unix_s: f64) -> DVec3 {
    let th = gmst(unix_s);
    let (c, s) = (th.cos(), th.sin());
    DVec3::new(c * v.x + s * v.y, -s * v.x + c * v.y, v.z)
}

/// Where every body is, at one instant, in ECEF meters (Earth center = 0).
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub unix_s: f64,
    pub positions: Vec<(BodyId, DVec3)>,
}

impl Snapshot {
    pub fn at(unix_s: f64) -> Snapshot {
        let t = julian_centuries(unix_s);
        let earth = heliocentric_ecliptic(BodyId::Earth, t);
        let mut positions = Vec::with_capacity(BODIES.len());
        for b in BODIES {
            let ecl_m = match b.id {
                BodyId::Earth => DVec3::ZERO,
                BodyId::Moon => moon_geocentric_ecliptic(t),
                BodyId::Sun => -earth * AU,
                id => (heliocentric_ecliptic(id, t) - earth) * AU,
            };
            positions.push((b.id, eci_to_ecef(ecliptic_to_equatorial(ecl_m, t), unix_s)));
        }
        Snapshot { unix_s, positions }
    }

    pub fn ecef(&self, id: BodyId) -> DVec3 {
        self.positions.iter().find(|p| p.0 == id).map(|p| p.1).unwrap_or_default()
    }

    /// What an observer sees: unit direction (ECEF), distance to the center,
    /// and angular radius (radians).
    pub fn sky(&self, observer_ecef: DVec3, id: BodyId) -> (DVec3, f64, f64) {
        let d = self.ecef(id) - observer_ecef;
        let dist = d.length();
        let r = info(id).radius_m;
        (d / dist, dist, (r / dist).min(1.0).asin())
    }
}

/// Convenience: the Sun's direction in ECEF from an observer.
pub fn sun_dir_ecef(observer: Geodetic, unix_s: f64) -> DVec3 {
    Snapshot::at(unix_s).sky(observer.to_ecef(), BodyId::Sun).0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unix(y: i32, mo: u32, d: u32, h: f64) -> f64 {
        // Days from civil (Howard Hinnant), good for any Gregorian date.
        let y = if mo <= 2 { y - 1 } else { y } as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (mo as i64 + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        (era * 146_097 + doe - 719_468) as f64 * 86_400.0 + h * 3600.0
    }

    #[test]
    fn date_helper() {
        assert_eq!(unix(1970, 1, 1, 0.0), 0.0);
        assert_eq!(unix(2024, 4, 8, 0.0), 1_712_534_400.0);
    }

    #[test]
    fn distances_are_sane() {
        let s = Snapshot::at(unix(2026, 11, 3, 12.0));
        let moon = s.ecef(BodyId::Moon).length();
        assert!((356_000e3..407_000e3).contains(&moon), "moon {moon}");
        let sun = s.ecef(BodyId::Sun).length() / AU;
        assert!((0.983..1.017).contains(&sun), "sun {sun} AU");
    }

    #[test]
    fn mars_close_approach_jan_2025() {
        let s = Snapshot::at(unix(2025, 1, 12, 0.0));
        let d = s.ecef(BodyId::Mars).length() / AU;
        assert!((0.62..0.67).contains(&d), "mars {d} AU");
    }

    #[test]
    fn great_american_eclipse_2024() {
        // Totality near Torreón, Mexico around 18:18 UTC on 2024-04-08.
        let obs = Geodetic::new(25.29, -104.14, 1_500.0).to_ecef();
        let s = Snapshot::at(unix(2024, 4, 8, 18.0 + 18.0 / 60.0));
        let (sun, _, _) = s.sky(obs, BodyId::Sun);
        let (moon, _, _) = s.sky(obs, BodyId::Moon);
        let sep = sun.angle_between(moon).to_degrees();
        assert!(sep < 0.8, "sun-moon separation {sep}°");
        // And the sun was high in the sky there.
        let up = obs.normalize();
        assert!(sun.dot(up) > 0.8);
    }
}
