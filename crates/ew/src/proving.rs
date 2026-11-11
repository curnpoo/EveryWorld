//! The proving ground's camera path and shots. Its content (27 parts with measured LOD
//! chains, 50 materials, the ground slab, the 100k-instance layout, the planter bench and
//! Solarcat) is baked into packs by `ew-bake` ([`ed_assets::proving`], `assets-src/*.toml`) and
//! loaded by [`crate::content`]; `ew` itself generates no geometry.

pub use ed_assets::proving::{MAX_BUILDING_H, ORIGIN};
use ed_render::Camera;
use glam::DVec3;

/// A camera at a fixed spot: street level on the central avenue, looking north.
pub fn street_camera() -> Camera {
    Camera::look_at(ORIGIN + DVec3::new(2.0, 1.7, 300.0), ORIGIN + DVec3::new(2.0, 4.0, 0.0))
}

/// Length of the proving path at speed 1, seconds (then it repeats).
pub const PATH_SECONDS: f64 = 10.0;

/// Proving path keyframes: (time s, position rel. to [`ORIGIN`], yaw, pitch). Yaw 0 looks
/// north (-Z), +pi/2 west (-X).
/// - 0-4 s street level up the central avenue (x = 0), looking north: 1.6 km sight line;
/// - 4-5.5 s climb; 5.5-8 s rooftop pass at 130 m (tallest roof ~123 m) turning west;
/// - 8-10 s down into the cross street (z = 0), looking west along it: long sight line.
pub const PATH_KEYS: [(f64, [f64; 3], f32, f32); 6] = [
    (0.0, [3.0, 1.7, 330.0], 0.0, 0.02),
    (4.0, [3.0, 1.7, 170.0], 0.0, 0.02),
    (5.5, [3.0, 130.0, 90.0], 0.0, -0.35),
    (8.0, [-20.0, 130.0, -60.0], 0.9, -0.3),
    (9.0, [-60.0, 2.0, 2.0], std::f32::consts::FRAC_PI_2, 0.0),
    (10.0, [-90.0, 2.0, 2.0], std::f32::consts::FRAC_PI_2, 0.01),
];

/// Times of the street and rooftop screenshots on the path, seconds.
pub const STREET_SHOT_S: f64 = 2.0;
pub const ROOFTOP_SHOT_S: f64 = 6.5;
pub const SIGHTLINE_SHOT_S: f64 = 9.5;

/// Camera pose at path time `t` seconds (piecewise linear between [`PATH_KEYS`], wraps).
pub fn path_pose(t: f64) -> crate::camera::Pose {
    let t = t.rem_euclid(PATH_SECONDS);
    let k = PATH_KEYS.windows(2).position(|w| t < w[1].0).unwrap_or(PATH_KEYS.len() - 2);
    let (a, b) = (PATH_KEYS[k], PATH_KEYS[k + 1]);
    let f = ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0);
    let p = DVec3::from(a.1).lerp(DVec3::from(b.1), f);
    crate::camera::Pose { position: ORIGIN + p, yaw: a.2 + (b.2 - a.2) * f as f32, pitch: a.3 + (b.3 - a.3) * f as f32 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_is_continuous_and_stays_above_roofs_on_the_pass() {
        let mut prev = path_pose(0.0);
        for i in 1..600 {
            let p = path_pose(i as f64 / 60.0);
            assert!(p.position.distance(prev.position) < 3.0, "step {i}");
            prev = p;
        }
        for i in 330..480 {
            assert!(path_pose(i as f64 / 60.0).position.y - ORIGIN.y >= 125.0);
        }
    }
}
