//! Real solar position (NOAA algorithm), so the sun is where it actually is
//! for any place and time on Earth.

use glam::DVec3;

/// Sun position as seen from a location.
#[derive(Debug, Clone, Copy)]
pub struct SunPosition {
    /// Degrees above the horizon (negative = night).
    pub elevation_deg: f64,
    /// Degrees clockwise from true north.
    pub azimuth_deg: f64,
}

impl SunPosition {
    /// Unit vector pointing *towards* the sun in ENU.
    pub fn direction_enu(&self) -> DVec3 {
        let el = self.elevation_deg.to_radians();
        let az = self.azimuth_deg.to_radians();
        DVec3::new(el.cos() * az.sin(), el.cos() * az.cos(), el.sin())
    }
}

/// Unix seconds -> Julian day.
pub fn julian_day(unix_s: f64) -> f64 {
    unix_s / 86_400.0 + 2_440_587.5
}

/// Solar position for `lat`/`lon` (degrees) at `unix_s` (UTC seconds).
pub fn sun_position(lat_deg: f64, lon_deg: f64, unix_s: f64) -> SunPosition {
    let jd = julian_day(unix_s);
    let t = (jd - 2_451_545.0) / 36_525.0; // Julian centuries since J2000

    let l0 = (280.466_46 + t * (36_000.769_83 + t * 0.000_303_2)).rem_euclid(360.0);
    let m = 357.529_11 + t * (35_999.050_29 - 0.000_153_7 * t);
    let e = 0.016_708_634 - t * (0.000_042_037 + 0.000_000_126_7 * t);
    let mr = m.to_radians();
    let c = mr.sin() * (1.914_602 - t * (0.004_817 + 0.000_014 * t))
        + (2.0 * mr).sin() * (0.019_993 - 0.000_101 * t)
        + (3.0 * mr).sin() * 0.000_289;
    let true_long = l0 + c;
    let omega = 125.04 - 1_934.136 * t;
    let app_long = true_long - 0.005_69 - 0.004_78 * omega.to_radians().sin();
    let eps0 = 23.0 + (26.0 + (21.448 - t * (46.815 + t * (0.000_59 - t * 0.001_813))) / 60.0) / 60.0;
    let eps = eps0 + 0.002_56 * omega.to_radians().cos();
    let decl = (eps.to_radians().sin() * app_long.to_radians().sin()).asin();

    let y = (eps.to_radians() / 2.0).tan().powi(2);
    let l0r = l0.to_radians();
    let eq_time = 4.0
        * (y * (2.0 * l0r).sin() - 2.0 * e * mr.sin() + 4.0 * e * y * mr.sin() * (2.0 * l0r).cos()
            - 0.5 * y * y * (4.0 * l0r).sin()
            - 1.25 * e * e * (2.0 * mr).sin())
        .to_degrees(); // minutes

    let minutes_utc = (unix_s.rem_euclid(86_400.0)) / 60.0;
    let true_solar = (minutes_utc + eq_time + 4.0 * lon_deg).rem_euclid(1440.0);
    let mut hour_angle = true_solar / 4.0 - 180.0;
    if hour_angle < -180.0 {
        hour_angle += 360.0;
    }

    let lat = lat_deg.to_radians();
    let ha = hour_angle.to_radians();
    let cos_zen = (lat.sin() * decl.sin() + lat.cos() * decl.cos() * ha.cos()).clamp(-1.0, 1.0);
    let zen = cos_zen.acos();
    let elevation = 90.0 - zen.to_degrees();

    let az_den = lat.cos() * zen.sin();
    let azimuth = if az_den.abs() > 1e-9 {
        let cos_az = ((lat.sin() * zen.cos() - decl.sin()) / az_den).clamp(-1.0, 1.0);
        let a = cos_az.acos().to_degrees();
        if hour_angle > 0.0 { (a + 180.0).rem_euclid(360.0) } else { (540.0 - a).rem_euclid(360.0) }
    } else if lat_deg > 0.0 {
        180.0
    } else {
        0.0
    };

    SunPosition { elevation_deg: elevation, azimuth_deg: azimuth }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solar_noon_equinox_equator() {
        // 2024-03-20 ~12:07 UTC is near solar noon at lon 0 on the equinox.
        let unix = 1_710_936_420.0;
        let s = sun_position(0.0, 0.0, unix);
        assert!(s.elevation_deg > 88.0, "{s:?}");
    }

    #[test]
    fn new_york_summer_afternoon() {
        // 2024-06-21 18:00 UTC = 14:00 EDT. Sun high in the SSW/SW.
        let unix = 1_718_992_800.0;
        let s = sun_position(40.7128, -74.006, unix);
        assert!((60.0..73.0).contains(&s.elevation_deg), "{s:?}");
        assert!((190.0..245.0).contains(&s.azimuth_deg), "{s:?}");
    }

    #[test]
    fn night_is_night() {
        // Midnight UTC in London in December.
        let unix = 1_734_739_200.0; // 2024-12-21 00:00 UTC
        let s = sun_position(51.5, -0.12, unix);
        assert!(s.elevation_deg < -40.0, "{s:?}");
    }

    #[test]
    fn morning_sun_is_east() {
        // 2024-06-21 10:00 UTC in Zermatt (12:00 CEST is ~solar 11:30).
        let unix = 1_718_964_000.0 - 3600.0 * 2.0; // 08:00 UTC
        let s = sun_position(46.02, 7.75, unix);
        assert!((60.0..140.0).contains(&s.azimuth_deg), "{s:?}");
    }
}
