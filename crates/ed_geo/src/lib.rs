//! Earth math for EveryWorld.
//!
//! Everything here is `f64` and engine-agnostic. The game renders in `f32`
//! relative to a floating origin (see [`EnuFrame`]); this crate is the source
//! of truth for "where on Earth is this".
//!
//! Frames used:
//! - **Geodetic**: WGS84 latitude/longitude (degrees) + height above ellipsoid (m).
//! - **ECEF**: Earth-centered, Earth-fixed Cartesian (m). Z through north pole.
//! - **ENU**: local East/North/Up tangent plane anchored at an origin.
//! - **Render (Y-up)**: ENU remapped to Bevy's convention: x=East, y=Up, z=-North.

pub mod node;
pub mod sun;
pub mod tile;

pub use node::{BodyId, NodeId, ObjectId};

use glam::{DMat3, DVec3};
use serde::{Deserialize, Serialize};

/// WGS84 semi-major axis (m).
pub const WGS84_A: f64 = 6_378_137.0;
/// WGS84 flattening.
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;
/// WGS84 semi-minor axis (m).
pub const WGS84_B: f64 = WGS84_A * (1.0 - WGS84_F);
/// First eccentricity squared.
pub const WGS84_E2: f64 = WGS84_F * (2.0 - WGS84_F);
/// Mean Earth radius (m), for great-circle math.
pub const EARTH_MEAN_RADIUS: f64 = 6_371_008.8;
/// Earth's standard gravitational parameter (m^3/s^2).
pub const EARTH_MU: f64 = 3.986_004_418e14;
/// Earth's rotation rate (rad/s).
pub const EARTH_OMEGA: f64 = 7.292_115e-5;

/// A point on (or above) the Earth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Geodetic {
    pub lat_deg: f64,
    pub lon_deg: f64,
    /// Height above the WGS84 ellipsoid, meters.
    pub alt_m: f64,
}

impl Geodetic {
    pub const fn new(lat_deg: f64, lon_deg: f64, alt_m: f64) -> Self {
        Self { lat_deg, lon_deg, alt_m }
    }

    pub fn to_ecef(self) -> DVec3 {
        let (lat, lon) = (self.lat_deg.to_radians(), self.lon_deg.to_radians());
        let (slat, clat) = lat.sin_cos();
        let (slon, clon) = lon.sin_cos();
        let n = WGS84_A / (1.0 - WGS84_E2 * slat * slat).sqrt();
        DVec3::new(
            (n + self.alt_m) * clat * clon,
            (n + self.alt_m) * clat * slon,
            (n * (1.0 - WGS84_E2) + self.alt_m) * slat,
        )
    }

    /// ECEF -> geodetic (Bowring's method, sub-millimeter for terrestrial heights,
    /// still accurate to well under a meter at orbital altitude).
    pub fn from_ecef(p: DVec3) -> Self {
        let lon = p.y.atan2(p.x);
        let r = (p.x * p.x + p.y * p.y).sqrt();
        if r < 1e-6 {
            // On the polar axis.
            let lat = if p.z >= 0.0 { 90.0 } else { -90.0 };
            return Self::new(lat, 0.0, p.z.abs() - WGS84_B);
        }
        let ep2 = (WGS84_A * WGS84_A - WGS84_B * WGS84_B) / (WGS84_B * WGS84_B);
        let theta = (p.z * WGS84_A).atan2(r * WGS84_B);
        let (st, ct) = theta.sin_cos();
        let lat = (p.z + ep2 * WGS84_B * st * st * st)
            .atan2(r - WGS84_E2 * WGS84_A * ct * ct * ct);
        let slat = lat.sin();
        let n = WGS84_A / (1.0 - WGS84_E2 * slat * slat).sqrt();
        let alt = if lat.cos().abs() > 1e-9 {
            r / lat.cos() - n
        } else {
            p.z.abs() / slat.abs() - n * (1.0 - WGS84_E2)
        };
        Self::new(lat.to_degrees(), lon.to_degrees(), alt)
    }

    /// Great-circle distance to another point (meters, mean-radius sphere).
    pub fn distance_to(self, other: Geodetic) -> f64 {
        let (p1, p2) = (self.lat_deg.to_radians(), other.lat_deg.to_radians());
        let dp = p2 - p1;
        let dl = (other.lon_deg - self.lon_deg).to_radians();
        let a = (dp / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
        2.0 * EARTH_MEAN_RADIUS * a.sqrt().atan2((1.0 - a).sqrt())
    }

    /// Initial great-circle bearing to `other`, degrees clockwise from north.
    pub fn bearing_to(self, other: Geodetic) -> f64 {
        let (p1, p2) = (self.lat_deg.to_radians(), other.lat_deg.to_radians());
        let dl = (other.lon_deg - self.lon_deg).to_radians();
        let y = dl.sin() * p2.cos();
        let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
        (y.atan2(x).to_degrees() + 360.0) % 360.0
    }

    /// Point reached travelling `dist_m` along a great circle at `bearing_deg`.
    pub fn destination(self, bearing_deg: f64, dist_m: f64) -> Geodetic {
        let d = dist_m / EARTH_MEAN_RADIUS;
        let b = bearing_deg.to_radians();
        let p1 = self.lat_deg.to_radians();
        let l1 = self.lon_deg.to_radians();
        let p2 = (p1.sin() * d.cos() + p1.cos() * d.sin() * b.cos()).asin();
        let l2 = l1 + (b.sin() * d.sin() * p1.cos()).atan2(d.cos() - p1.sin() * p2.sin());
        Geodetic::new(p2.to_degrees(), wrap_lon(l2.to_degrees()), self.alt_m)
    }
}

/// Wrap a longitude into [-180, 180).
pub fn wrap_lon(lon: f64) -> f64 {
    let mut l = (lon + 180.0) % 360.0;
    if l < 0.0 {
        l += 360.0;
    }
    l - 180.0
}

/// A local East-North-Up tangent frame. This is the floating origin: the game
/// world is rendered in this frame, and the frame is re-anchored ("rebased")
/// when the player strays too far so `f32` precision never runs out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnuFrame {
    pub origin: Geodetic,
    pub origin_ecef: DVec3,
    /// Rows are the E, N, U unit vectors expressed in ECEF.
    ecef_to_enu: DMat3,
}

impl EnuFrame {
    pub fn new(origin: Geodetic) -> Self {
        let (lat, lon) = (origin.lat_deg.to_radians(), origin.lon_deg.to_radians());
        let (slat, clat) = lat.sin_cos();
        let (slon, clon) = lon.sin_cos();
        let east = DVec3::new(-slon, clon, 0.0);
        let north = DVec3::new(-slat * clon, -slat * slon, clat);
        let up = DVec3::new(clat * clon, clat * slon, slat);
        // DMat3 is column-major; build with E/N/U as columns then transpose so
        // they become rows (ecef -> enu projects onto each axis).
        let enu_to_ecef = DMat3::from_cols(east, north, up);
        Self { origin, origin_ecef: origin.to_ecef(), ecef_to_enu: enu_to_ecef.transpose() }
    }

    pub fn ecef_to_enu(&self, p: DVec3) -> DVec3 {
        self.ecef_to_enu * (p - self.origin_ecef)
    }

    pub fn enu_to_ecef(&self, enu: DVec3) -> DVec3 {
        self.ecef_to_enu.transpose() * enu + self.origin_ecef
    }

    /// Rotate a direction (not a point) from ECEF into ENU.
    pub fn dir_ecef_to_enu(&self, d: DVec3) -> DVec3 {
        self.ecef_to_enu * d
    }

    pub fn dir_enu_to_ecef(&self, d: DVec3) -> DVec3 {
        self.ecef_to_enu.transpose() * d
    }

    pub fn geodetic_to_render(&self, g: Geodetic) -> DVec3 {
        enu_to_render(self.ecef_to_enu(g.to_ecef()))
    }

    pub fn render_to_geodetic(&self, r: DVec3) -> Geodetic {
        Geodetic::from_ecef(self.enu_to_ecef(render_to_enu(r)))
    }

    /// Rigid transform taking render-space points to ECEF.
    pub fn render_to_ecef(&self) -> glam::DAffine3 {
        // render (x=E, y=U, z=-N) -> ENU columns, then ENU -> ECEF.
        let render_to_enu = DMat3::from_cols(DVec3::X, DVec3::new(0.0, 0.0, 1.0), DVec3::new(0.0, -1.0, 0.0));
        let m = self.ecef_to_enu.transpose() * render_to_enu;
        glam::DAffine3::from_mat3_translation(m, self.origin_ecef)
    }

    /// Rigid transform taking ECEF points to render space.
    pub fn ecef_to_render(&self) -> glam::DAffine3 {
        self.render_to_ecef().inverse()
    }

    /// Rotate a direction vector (not a point) from render space (x=E, y=U, z=-N) into ECEF.
    pub fn dir_render_to_ecef(&self, d: DVec3) -> DVec3 {
        let render_to_enu = DMat3::from_cols(DVec3::X, DVec3::new(0.0, 0.0, 1.0), DVec3::new(0.0, -1.0, 0.0));
        let m = self.ecef_to_enu.transpose() * render_to_enu;
        m * d
    }

    /// Rotate a direction vector (not a point) from ECEF into render space (x=E, y=U, z=-N).
    pub fn dir_ecef_to_render(&self, d: DVec3) -> DVec3 {
        let render_to_enu = DMat3::from_cols(DVec3::X, DVec3::new(0.0, 0.0, 1.0), DVec3::new(0.0, -1.0, 0.0));
        let m = self.ecef_to_enu.transpose() * render_to_enu;
        m.transpose() * d
    }

    /// Convert an orientation quaternion from render space into ECEF.
    pub fn quat_render_to_ecef(&self, q: glam::Quat) -> glam::Quat {
        let (_, frame_rot, _) = self.render_to_ecef().to_scale_rotation_translation();
        (frame_rot.as_quat() * q).normalize()
    }

    /// Convert an orientation quaternion from ECEF into render space.
    pub fn quat_ecef_to_render(&self, q: glam::Quat) -> glam::Quat {
        let (_, frame_rot, _) = self.render_to_ecef().to_scale_rotation_translation();
        (frame_rot.as_quat().inverse() * q).normalize()
    }

    /// Local "up" (away from Earth's center along the ellipsoid normal) at a
    /// render-space point. Far from the origin this tilts: that's the curvature.
    pub fn up_at_render(&self, r: DVec3) -> DVec3 {
        let g = self.render_to_geodetic(r);
        let n = EnuFrame::new(g).dir_enu_to_ecef(DVec3::Z);
        enu_to_render(self.dir_ecef_to_enu(n))
    }
}

/// ENU (x=E, y=N, z=U) -> render (x=E, y=U, z=-N).
pub fn enu_to_render(v: DVec3) -> DVec3 {
    DVec3::new(v.x, v.z, -v.y)
}

/// Render (x=E, y=U, z=-N) -> ENU.
pub fn render_to_enu(v: DVec3) -> DVec3 {
    DVec3::new(v.x, -v.z, v.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn ecef_roundtrip() {
        for g in [
            Geodetic::new(46.02, 7.75, 1620.0),   // Zermatt
            Geodetic::new(40.758, -73.9855, 10.0), // Times Square
            Geodetic::new(-33.86, 151.21, 0.0),   // Sydney
            Geodetic::new(89.9, 12.0, 100.0),
            Geodetic::new(-45.0, -179.9, 400_000.0), // LEO altitude
        ] {
            let back = Geodetic::from_ecef(g.to_ecef());
            assert!(close(back.lat_deg, g.lat_deg, 1e-7), "{g:?} -> {back:?}");
            assert!(close(back.lon_deg, g.lon_deg, 1e-7), "{g:?} -> {back:?}");
            assert!(close(back.alt_m, g.alt_m, 1e-2), "{g:?} -> {back:?}");
        }
    }

    #[test]
    fn enu_axes_make_sense() {
        let origin = Geodetic::new(48.8584, 2.2945, 35.0); // Eiffel tower
        let f = EnuFrame::new(origin);
        // 100 m up is +U.
        let up = f.ecef_to_enu(Geodetic { alt_m: 135.0, ..origin }.to_ecef());
        assert!(close(up.z, 100.0, 1e-3) && up.x.abs() < 1e-3 && up.y.abs() < 1e-3);
        // A point slightly north is +N.
        let n = f.ecef_to_enu(Geodetic::new(48.8594, 2.2945, 35.0).to_ecef());
        assert!(n.y > 100.0 && n.x.abs() < 0.01);
        // Render frame: north is -Z, up is +Y.
        let r = enu_to_render(n);
        assert!(r.z < -100.0);
    }

    #[test]
    fn render_affine_matches_pointwise() {
        let f = EnuFrame::new(Geodetic::new(35.0, 139.0, 20.0));
        let g = Geodetic::new(35.01, 139.02, 300.0);
        let r = f.geodetic_to_render(g);
        let e = f.render_to_ecef().transform_point3(r);
        assert!((e - g.to_ecef()).length() < 1e-4);
        let back = f.ecef_to_render().transform_point3(e);
        assert!((back - r).length() < 1e-4);
    }

    #[test]
    fn curvature_drop() {
        // 10 km away the ground has dropped ~7.8 m below the tangent plane.
        let origin = Geodetic::new(0.0, 0.0, 0.0);
        let f = EnuFrame::new(origin);
        let far = origin.destination(90.0, 10_000.0);
        let enu = f.ecef_to_enu(far.to_ecef());
        assert!(close(enu.z, -7.85, 0.2), "drop was {}", enu.z);
    }

    #[test]
    fn great_circle() {
        let nyc = Geodetic::new(40.7128, -74.0060, 0.0);
        let paris = Geodetic::new(48.8566, 2.3522, 0.0);
        let d = nyc.distance_to(paris);
        assert!(close(d, 5_837_000.0, 15_000.0), "{d}");
        let b = nyc.bearing_to(paris);
        assert!(close(b, 53.7, 1.0), "{b}");
        let there = nyc.destination(b, d);
        assert!(there.distance_to(paris) < 1000.0);
    }

    #[test]
    fn wraps() {
        assert!(close(wrap_lon(190.0), -170.0, 1e-9));
        assert!(close(wrap_lon(-190.0), 170.0, 1e-9));
    }

    #[test]
    fn quat_and_dir_frame_roundtrip() {
        let f = EnuFrame::new(Geodetic::new(43.53, 5.45, 300.0));
        let q = glam::Quat::from_euler(glam::EulerRot::XYZ, 0.4, -0.6, 1.2);
        let q_ecef = f.quat_render_to_ecef(q);
        let q_back = f.quat_ecef_to_render(q_ecef);
        assert!((q.dot(q_back).abs() - 1.0).abs() < 1e-5);

        let d = DVec3::new(1.0, 2.5, -3.2).normalize();
        let d_ecef = f.dir_render_to_ecef(d);
        let d_back = f.dir_ecef_to_render(d_ecef);
        assert!((d - d_back).length() < 1e-6);
    }
}
