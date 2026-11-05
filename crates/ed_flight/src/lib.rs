//! Flight physics: atmosphere, drag, Mach, gravity, and the "superpower"
//! rocket model.
//!
//! Philosophy: the *forces* are real (gravity falls off with altitude, air
//! thins out, drag scales with density and v², Mach comes from real speed of
//! sound), but the player has absurd thrust and inertial dampers. That's what
//! makes breaking the sound barrier a few seconds after lift-off feel right:
//! the world reacts physically to something impossible.

pub mod flyer;

use glam::DVec3;

pub const G0: f64 = 9.806_65;

/// International Standard Atmosphere (0-86 km), exponential above.
#[derive(Debug, Clone, Copy)]
pub struct Air {
    pub density: f64,
    pub temperature_k: f64,
    pub pressure_pa: f64,
    pub speed_of_sound: f64,
}

pub fn atmosphere(alt_m: f64) -> Air {
    const R: f64 = 287.052_87;
    const GAMMA: f64 = 1.4;
    // (base altitude, base temperature, lapse rate, base pressure)
    const LAYERS: [(f64, f64, f64, f64); 7] = [
        (0.0, 288.15, -0.0065, 101_325.0),
        (11_000.0, 216.65, 0.0, 22_632.1),
        (20_000.0, 216.65, 0.001, 5_474.89),
        (32_000.0, 228.65, 0.0028, 868.019),
        (47_000.0, 270.65, 0.0, 110.906),
        (51_000.0, 270.65, -0.0028, 66.9389),
        (71_000.0, 214.65, -0.002, 3.956_42),
    ];
    let h = alt_m.max(-500.0);
    if h > 86_000.0 {
        let rho86 = 6.958e-6;
        let density = rho86 * (-(h - 86_000.0) / 6_500.0).exp();
        let t = 186.87;
        return Air { density, temperature_k: t, pressure_pa: density * R * t, speed_of_sound: (GAMMA * R * t).sqrt() };
    }
    let &(hb, tb, lb, pb) = LAYERS.iter().rev().find(|l| h >= l.0).unwrap_or(&LAYERS[0]);
    let dh = h - hb;
    let t = tb + lb * dh;
    let p = if lb.abs() < 1e-12 {
        pb * (-G0 * dh / (R * tb)).exp()
    } else {
        pb * (t / tb).powf(-G0 / (lb * R))
    };
    Air { density: p / (R * t), temperature_k: t, pressure_pa: p, speed_of_sound: (GAMMA * R * t).sqrt() }
}

/// Gravity at a geocentric position (point-mass, `mu` = GM).
pub fn gravity(pos_from_center: DVec3, mu: f64) -> DVec3 {
    let r2 = pos_from_center.length_squared();
    if r2 < 1.0 {
        return DVec3::ZERO;
    }
    -pos_from_center.normalize() * (mu / r2)
}

/// Quadratic drag force.
pub fn drag(vel_rel_air: DVec3, density: f64, cd_area: f64) -> DVec3 {
    let v = vel_rel_air.length();
    if v < 1e-6 {
        return DVec3::ZERO;
    }
    -vel_rel_air * (0.5 * density * v * cd_area)
}

/// Tracks Mach crossings so the game can fire sonic booms, vapor cones, etc.
#[derive(Debug, Default, Clone, Copy)]
pub struct MachTracker {
    pub mach: f64,
    supersonic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachEvent {
    /// Crossed Mach 1 going up: boom + vapor cone.
    BrokeBarrier,
    /// Dropped below Mach 1.
    Subsonic,
}

impl MachTracker {
    pub fn update(&mut self, speed: f64, alt_m: f64) -> Option<MachEvent> {
        self.mach = speed / atmosphere(alt_m).speed_of_sound;
        // Hysteresis so hovering at Mach 1.0 doesn't machine-gun booms.
        match (self.supersonic, self.mach) {
            (false, m) if m >= 1.0 => {
                self.supersonic = true;
                Some(MachEvent::BrokeBarrier)
            }
            (true, m) if m < 0.95 => {
                self.supersonic = false;
                Some(MachEvent::Subsonic)
            }
            _ => None,
        }
    }
}

/// The player's rocket (rocket shoes, jetpack, or an actual rocket ship all
/// use this, with different tuning).
#[derive(Debug, Clone, Copy)]
pub struct SuperRocket {
    /// Thrust as a multiple of local gravity at full throttle, before boost.
    pub thrust_g: f64,
    /// Boost multiplier per stage (hold boost to climb stages).
    pub boost_stages: [f64; 4],
    /// Drag coefficient * area / mass (m²/kg). Small = slippery.
    pub cd_area_per_kg: f64,
    /// 0..1: how strongly velocity is steered toward where you're pointing
    /// ("inertial dampers"). 0 = pure Newtonian.
    pub assist: f64,
}

impl SuperRocket {
    pub const ROCKET_SHOES: Self = Self { thrust_g: 2.2, boost_stages: [1.0, 3.0, 10.0, 40.0], cd_area_per_kg: 0.004, assist: 0.6 };
    pub const ROCKET_SHIP: Self = Self { thrust_g: 4.0, boost_stages: [1.0, 10.0, 100.0, 2_000.0], cd_area_per_kg: 0.0004, assist: 0.85 };

    /// Acceleration (m/s², excluding gravity) for the current controls.
    /// `facing` is a unit vector; `throttle` 0..1; `stage` 0..3.
    pub fn accel(&self, vel_rel_air: DVec3, facing: DVec3, throttle: f64, stage: usize, alt_m: f64, local_g: f64) -> DVec3 {
        let boost = self.boost_stages[stage.min(3)];
        let thrust = facing * (self.thrust_g * local_g * boost * throttle.clamp(0.0, 1.0));
        let air = atmosphere(alt_m);
        let d = drag(vel_rel_air, air.density, self.cd_area_per_kg);
        // Assist: bleed sideways velocity into the facing direction.
        let v = vel_rel_air;
        let along = facing * v.dot(facing);
        let side = v - along;
        let steer = -side * self.assist * 1.5;
        thrust + d + steer
    }
}

/// Simple integrator state for anything flying in the world frame.
#[derive(Debug, Clone, Copy)]
pub struct Body {
    pub pos: DVec3,
    pub vel: DVec3,
}

impl Body {
    /// Semi-implicit Euler: stable enough at 60-120 Hz for gameplay.
    pub fn step(&mut self, accel: DVec3, dt: f64) {
        self.vel += accel * dt;
        self.pos += self.vel * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isa_values() {
        let sl = atmosphere(0.0);
        assert!((sl.density - 1.225).abs() < 0.001);
        assert!((sl.speed_of_sound - 340.3).abs() < 0.5);
        let cruise = atmosphere(11_000.0);
        assert!((cruise.density - 0.364).abs() < 0.01);
        assert!((cruise.speed_of_sound - 295.1).abs() < 0.5);
        assert!(atmosphere(100_000.0).density < 1e-6);
    }

    #[test]
    fn mach_events_have_hysteresis() {
        let mut m = MachTracker::default();
        assert_eq!(m.update(300.0, 0.0), None);
        assert_eq!(m.update(345.0, 0.0), Some(MachEvent::BrokeBarrier));
        assert_eq!(m.update(335.0, 0.0), None);
        assert_eq!(m.update(310.0, 0.0), Some(MachEvent::Subsonic));
    }

    #[test]
    fn rocket_shoes_break_sound_barrier_fast() {
        // Point straight up at full boost from sea level; how long to Mach 1?
        let r = SuperRocket::ROCKET_SHOES;
        let mut b = Body { pos: DVec3::new(0.0, 0.0, 0.0), vel: DVec3::ZERO };
        let mut mach = MachTracker::default();
        let dt = 1.0 / 120.0;
        let mut t = 0.0;
        let mut broke = None;
        while t < 30.0 {
            let a = r.accel(b.vel, DVec3::Z, 1.0, 3, b.pos.z, G0) - DVec3::Z * G0;
            b.step(a, dt);
            t += dt;
            if mach.update(b.vel.length(), b.pos.z) == Some(MachEvent::BrokeBarrier) {
                broke = Some(t);
                break;
            }
        }
        let t = broke.expect("never went supersonic");
        assert!(t < 2.0, "took {t:.1}s to break the barrier");
    }

    #[test]
    fn ship_reaches_space_quickly() {
        let r = SuperRocket::ROCKET_SHIP;
        let mut b = Body { pos: DVec3::ZERO, vel: DVec3::ZERO };
        let dt = 1.0 / 60.0;
        let mut t = 0.0;
        while b.pos.z < 100_000.0 && t < 120.0 {
            let a = r.accel(b.vel, DVec3::Z, 1.0, 2, b.pos.z, G0) - DVec3::Z * G0;
            b.step(a, dt);
            t += dt;
        }
        assert!(t < 30.0, "Karman line took {t:.0}s");
    }
}
