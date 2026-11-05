//! What the game tells the soundscape. `Params` is continuous state, sent
//! every frame and smoothed on the audio thread; `Event` is a one-shot.
//! Weight arrays (vegetation, fauna, rain surfaces, blast kinds, tyre
//! surfaces) let everything crossfade smoothly instead of switching.

/// Vegetation sound classes (`docs/plans/sound.md` §5 table).
pub mod veg {
    pub const CONIFER: usize = 0;
    pub const CYPRESS: usize = 1;
    pub const BROADLEAF: usize = 2;
    pub const BIRCH: usize = 3;
    pub const OLIVE: usize = 4;
    pub const PALM: usize = 5;
    pub const BANANA: usize = 6;
    pub const SPARSE: usize = 7;
    pub const CACTUS: usize = 8;
    pub const SHRUB: usize = 9;
    pub const GRASS: usize = 10;
    pub const REEDS: usize = 11;
    pub const N: usize = 12;
}

/// Living-thing layers. Weights are already biome × time × temperature.
pub mod fauna {
    pub const DRONE: usize = 0; // tropical insect wall
    pub const BIRDS: usize = 1; // songbirds
    pub const CICADAS: usize = 2;
    pub const CRICKETS: usize = 3;
    pub const FROGS: usize = 4;
    pub const OWLS: usize = 5;
    pub const GULLS: usize = 6;
    pub const COWBELLS: usize = 7;
    pub const ICE: usize = 8; // glacier creaks
    pub const LOONS: usize = 9;
    pub const N: usize = 10;
}

/// What rain lands on around the listener.
pub mod rain_on {
    pub const LEAVES: usize = 0;
    pub const PAVING: usize = 1;
    pub const WATER: usize = 2;
    pub const ROOF: usize = 3;
    pub const N: usize = 4;
}

/// What the rocket exhaust is blasting.
pub mod blast {
    pub const DUST: usize = 0;
    pub const GRASS: usize = 1;
    pub const SPRAY: usize = 2;
    pub const SNOW: usize = 3;
    pub const PAVING: usize = 4;
    pub const N: usize = 5;
}

/// Tyre contact surfaces.
pub mod tyre {
    pub const ASPHALT: usize = 0;
    pub const GRAVEL: usize = 1;
    pub const DIRT: usize = 2;
    pub const N: usize = 3;
}

/// Footstep / landing surface. The first 12 mirror `ed_world::surface::Ground`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Surface {
    #[default]
    Grass,
    DryGrass,
    Moss,
    ForestFloor,
    Dirt,
    Farmland,
    Sand,
    Gravel,
    Rock,
    Snow,
    Paving,
    Mud,
    Water,
    Wood,
    Metal,
    Moon,
}

impl Surface {
    pub fn from_ground(g: ed_world::surface::Ground) -> Surface {
        use ed_world::surface::Ground as G;
        match g {
            G::Grass => Surface::Grass,
            G::DryGrass => Surface::DryGrass,
            G::Moss => Surface::Moss,
            G::ForestFloor => Surface::ForestFloor,
            G::Dirt => Surface::Dirt,
            G::Farmland => Surface::Farmland,
            G::Sand => Surface::Sand,
            G::Gravel => Surface::Gravel,
            G::Rock => Surface::Rock,
            G::Snow => Surface::Snow,
            G::Paving => Surface::Paving,
            G::Mud => Surface::Mud,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Params {
    // ---- air and body ----
    /// Air density at the listener, kg/m³ (1.225 at sea level, 0 in space).
    pub density: f32,
    /// Dynamic pressure of the player's own motion through the air, Pa.
    pub q: f32,
    pub mach: f32,
    /// Sideways airflow in the head frame, -1 (from left) .. 1 (from right).
    pub flow_side: f32,
    /// Freefall posture, 0..1 each.
    pub tuck: f32,
    pub spread: f32,
    /// Body roll rate, rad/s: sweeps the roar between the ears.
    pub roll_rate: f32,
    /// 0..1 crossfades for the airborne modes.
    pub freefall: f32,
    pub wingsuit: f32,
    /// Wingsuit pull / turn load, 0..1.
    pub wing_load: f32,
    pub canopy: f32,
    /// Per-boot throttle (0 off, ~1 cruise, up to 4 afterburner).
    pub boots: [f32; 2],
    /// Afterburner spool, 0..1.
    pub afterburner: f32,
    /// Leg swing, -1..1: wobbles the two boot pitches against each other.
    pub leg_steer: f32,
    /// Exhaust-on-ground strength 0..1 and what it hits (weights).
    pub blast: f32,
    pub blast_kind: [f32; blast::N],
    pub jump_charge: f32,
    /// Re-entry heating 0..1 (∝ ρv³).
    pub reentry: f32,
    /// Limb motion 0..1 (servo whir; the only sound in space).
    pub servo: f32,

    // ---- world ----
    pub wind: f32,
    pub gust: f32,
    pub veg: [f32; veg::N],
    /// Ocean surf proximity 0..1 and how rocky the coast is 0..1.
    pub surf: f32,
    pub surf_rocky: f32,
    /// River proximity 0..1 and steepness 0..1 (0 = wide slow, 1 = rapids).
    pub river: f32,
    pub river_steep: f32,
    pub waterfall: f32,
    /// Lake shore lapping 0..1.
    pub lake: f32,
    /// Rain, mm/h. Drizzle 0..1 is a fine hiss with no drops. Hail 0..1.
    pub rain: f32,
    pub drizzle: f32,
    pub hail: f32,
    pub rain_on: [f32; rain_on::N],
    /// The robot is out in the rain (not under a roof), 0..1.
    pub exposed: f32,
    /// Snow cover / snowfall muffle 0..1, fog damping 0..1.
    pub snow: f32,
    pub fog: f32,
    pub temp_c: f32,
    pub fauna: [f32; fauna::N],
    /// Traffic: motorway/trunk roar 0..1, primary hum 0..1, minor-road passes 0..1.
    pub traffic_far: f32,
    pub traffic_near: f32,
    pub traffic_minor: f32,
    /// Commercial crowd murmur, industrial drone.
    pub crowd: f32,
    pub industry: f32,
    /// Mains frequency (50/60 Hz) and the transformer/sodium hum levels.
    pub grid_hz: f32,
    pub hum: f32,
    pub sodium: f32,
    /// Rail proximity 0..1: trains pass now and then.
    pub rail: f32,
    /// Reflection: wet 0..1 and delay (street canyon slap-back or valley echo).
    pub echo: f32,
    pub echo_delay_s: f32,

    // ---- vehicles ----
    pub tyre_speed: f32,
    pub tyre_on: [f32; tyre::N],
    pub skid: f32,
    pub stall: f32,
    pub cabin_wind: f32,

    // ---- UI ----
    /// Altimeter beeps per second (0 = off, >= 12 = continuous tone).
    pub altimeter: f32,
    /// Ground-proximity warning 0..1.
    pub gpw: f32,
    pub master: f32,
    /// Per-bus user gains 0..1, indexed by [`bus`]: body, air, ambience, UI.
    pub mix: [f32; 4],
}

/// Indices into [`Params::mix`].
pub mod bus {
    pub const BODY: usize = 0;
    pub const AIR: usize = 1;
    pub const AMB: usize = 2;
    pub const UI: usize = 3;
}

impl Default for Params {
    fn default() -> Self {
        Params {
            density: 1.225,
            q: 0.0,
            mach: 0.0,
            flow_side: 0.0,
            tuck: 0.0,
            spread: 0.0,
            roll_rate: 0.0,
            freefall: 0.0,
            wingsuit: 0.0,
            wing_load: 0.0,
            canopy: 0.0,
            boots: [0.0; 2],
            afterburner: 0.0,
            leg_steer: 0.0,
            blast: 0.0,
            blast_kind: [0.0; blast::N],
            jump_charge: 0.0,
            reentry: 0.0,
            servo: 0.0,
            wind: 0.0,
            gust: 0.0,
            veg: [0.0; veg::N],
            surf: 0.0,
            surf_rocky: 0.0,
            river: 0.0,
            river_steep: 0.0,
            waterfall: 0.0,
            lake: 0.0,
            rain: 0.0,
            drizzle: 0.0,
            hail: 0.0,
            rain_on: [0.0; rain_on::N],
            exposed: 1.0,
            snow: 0.0,
            fog: 0.0,
            temp_c: 15.0,
            fauna: [0.0; fauna::N],
            traffic_far: 0.0,
            traffic_near: 0.0,
            traffic_minor: 0.0,
            crowd: 0.0,
            industry: 0.0,
            grid_hz: 50.0,
            hum: 0.0,
            sodium: 0.0,
            rail: 0.0,
            echo: 0.0,
            echo_delay_s: 0.05,
            tyre_speed: 0.0,
            tyre_on: [1.0, 0.0, 0.0],
            skid: 0.0,
            stall: 0.0,
            cabin_wind: 0.0,
            altimeter: 0.0,
            gpw: 0.0,
            master: 1.0,
            mix: [1.0; 4],
        }
    }
}

macro_rules! smooth {
    ($s:ident, $t:ident, $k:ident; $($f:ident),* ; arrays $($a:ident),*) => {
        $( $s.$f += ($t.$f - $s.$f) * $k; )*
        $( for (x, y) in $s.$a.iter_mut().zip($t.$a.iter()) { *x += (*y - *x) * $k; } )*
    };
}

impl Params {
    /// Moves every field a fraction `k` toward `t` (called once per block).
    /// Discrete-ish fields (grid frequency, echo delay) snap.
    pub fn smooth_toward(&mut self, t: &Params, k: f32) {
        smooth!(self, t, k;
            density, q, mach, flow_side, tuck, spread, roll_rate, freefall, wingsuit, wing_load, canopy,
            afterburner, leg_steer, blast, jump_charge, reentry, servo, wind, gust, surf, surf_rocky,
            river, river_steep, waterfall, lake, rain, drizzle, hail, exposed, snow, fog, temp_c,
            traffic_far, traffic_near, traffic_minor, crowd, industry, hum, sodium, rail, echo,
            tyre_speed, skid, stall, cabin_wind, gpw, master;
            arrays boots, blast_kind, veg, rain_on, fauna, tyre_on, mix);
        self.grid_hz = t.grid_hz;
        self.echo_delay_s += (t.echo_delay_s - self.echo_delay_s) * (k * 0.25);
        self.altimeter = t.altimeter;
    }

    /// Airspeed recovered from q and density.
    pub fn airspeed(&self) -> f32 {
        if self.density > 1e-4 { (2.0 * self.q / self.density).sqrt() } else { 0.0 }
    }

    /// 1 at sea-level density, 0 by about 30 km: gates every sound carried by air.
    pub fn air_gate(&self) -> f32 {
        crate::dsp::smoothstep(0.004, 0.06, self.density / 1.225)
    }

    /// Ambient world beds fade earlier (above ~15 km only the wind is left).
    pub fn world_gate(&self) -> f32 {
        crate::dsp::smoothstep(0.12, 0.45, self.density / 1.225)
    }

    /// "At speed, wind is king": world beds duck up to -12 dB above ~30 m/s.
    pub fn duck(&self) -> f32 {
        1.0 - 0.75 * crate::dsp::smoothstep(350.0, 1_800.0, self.q)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    /// `force` 0..1 (walk ~0.5, sprint 0.8, crouch 0.2). `pan` -1..1 (left/right foot).
    Footstep { surface: Surface, force: f32, pan: f32, sprint: bool },
    /// Impact speed into the ground, m/s.
    Land { surface: Surface, impact: f32 },
    Launch { surface: Surface, charge: f32 },
    ChargeFull,
    Ignite { boot: u8 },
    BoostKick { stage: u8 },
    /// `burn_s` = how long the boots ran (long burns tink while cooling).
    Cutoff { burn_s: f32 },
    ChuteOpen,
    SuitOpen,
    SuitClose,
    /// Something big passed close at `speed` m/s; `pan` side, `size` 0..1 (bush .. tall tree/building).
    Whoosh { speed: f32, pan: f32, size: f32 },
    /// Lightning at `distance_m`; the audio thread delays it at 343 m/s.
    Thunder { distance_m: f32 },
    /// Suspension bottoming / pothole. `strength` 0..1.
    Thump { strength: f32, pan: f32 },
    /// A short tone marking a movement-mode switch (0 walk, 1 rocket, 2 skydive, 3 parachute, 4 wingsuit).
    ModeChirp { mode: u8 },
    Splash { force: f32 },
    /// Bumped into something solid: 0 wood, 1 concrete, 2 rock, 3 metal.
    Bump { what: u8, force: f32 },
}

/// Speed of sound used for delays (the sound plan's 343 m/s).
pub const SOUND_SPEED: f32 = 343.0;

/// Dolbear's law (1897) in Celsius: snowy tree cricket chirps per minute,
/// N = 7.2 T - 32 (T_F = 50 + (N - 40)/4 rearranged). Silent below ~5 °C.
pub fn dolbear_cpm(temp_c: f32) -> f32 {
    (7.2 * temp_c - 32.0).max(0.0)
}

/// q = ½ ρ v².
pub fn dynamic_pressure(density: f32, speed: f32) -> f32 {
    0.5 * density * speed * speed
}

/// Mains frequency by location. The Americas are mostly 60 Hz (except
/// most of South America's south cone and a few others, which use 50), Japan splits at the
/// Fuji river (east 50 Hz, west 60 Hz), South Korea, Taiwan, the Philippines and
/// Saudi Arabia are 60, everything else 50.
pub fn grid_hz(lat: f64, lon: f64) -> f32 {
    // Japan: east of roughly 138.5°E (the Fuji river / Itoigawa line) is 50 Hz.
    if (24.0..46.0).contains(&lat) && (128.0..146.0).contains(&lon) {
        return if lon > 138.5 && lat > 34.8 { 50.0 } else { 60.0 };
    }
    // South Korea, Taiwan, Philippines.
    if (33.0..39.0).contains(&lat) && (124.0..130.0).contains(&lon) {
        return 60.0;
    }
    if (21.5..25.5).contains(&lat) && (119.5..122.5).contains(&lon) {
        return 60.0;
    }
    if (4.5..21.0).contains(&lat) && (116.0..127.0).contains(&lon) {
        return 60.0;
    }
    // Saudi Arabia.
    // (box trimmed so Israel, Jordan, Iraq, Kuwait, Bahrain, Qatar and the UAE stay 50 Hz)
    let gulf = (lat > 24.0 && lon > 51.0) || (lat > 25.8 && lon > 50.2) || (lat > 29.0 && lon > 44.0);
    if (16.0..31.0).contains(&lat) && (36.0..56.0).contains(&lon) && !gulf {
        return 60.0;
    }
    if (-170.0..-30.0).contains(&lon) {
        // Southern cone (Argentina, Chile, Uruguay, Paraguay, Bolivia) is 50 Hz;
        // Brazil, Colombia, Venezuela, Peru and North/Central America 60 Hz.
        let southern_cone = lat < -17.0 && lon < -53.0 || lat < -30.0;
        // French Guiana and Suriname are 50/60 mixed; treat as 60.
        return if southern_cone { 50.0 } else { 60.0 };
    }
    50.0
}

/// Beeps per second for the audible altimeter: silent above 1500 m AGL,
/// then faster as you descend, a continuous tone at deployment height (~800 m) while falling fast.
pub fn altimeter_rate(agl_m: f32, vz_down_ms: f32) -> f32 {
    if vz_down_ms < 8.0 || agl_m > 1_500.0 || agl_m < 0.0 {
        return 0.0;
    }
    if agl_m < 800.0 {
        return 20.0; // pull!
    }
    1.0 + 9.0 * (1.0 - (agl_m - 800.0) / 700.0)
}

/// Ground-proximity warning: 0 when time-to-impact > 4 s, 1 at 1 s.
/// Only for falls that would hurt (> 20 m/s down): hopping off a ledge or
/// settling onto the ground after a spawn drop never sounds it.
pub fn gpw_level(agl_m: f32, vz_down_ms: f32) -> f32 {
    if vz_down_ms <= 20.0 {
        return 0.0;
    }
    let tti = agl_m / vz_down_ms;
    crate::dsp::smoothstep(4.0, 1.0, tti)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dolbear_hot_nights_chirp_faster() {
        assert_eq!(dolbear_cpm(0.0), 0.0);
        assert!((dolbear_cpm(20.0) - 112.0).abs() < 0.01);
        assert!(dolbear_cpm(28.0) > dolbear_cpm(20.0));
    }

    #[test]
    fn grid_frequency_by_country() {
        assert_eq!(grid_hz(40.7, -74.0), 60.0); // New York
        assert_eq!(grid_hz(48.85, 2.35), 50.0); // Paris
        assert_eq!(grid_hz(35.68, 139.7), 50.0); // Tokyo (east)
        assert_eq!(grid_hz(34.69, 135.5), 60.0); // Osaka (west)
        assert_eq!(grid_hz(-34.6, -58.4), 50.0); // Buenos Aires
        assert_eq!(grid_hz(-23.5, -46.6), 60.0); // São Paulo
        assert_eq!(grid_hz(37.5, 127.0), 60.0); // Seoul
        assert_eq!(grid_hz(31.6, -8.0), 50.0); // Marrakech
        assert_eq!(grid_hz(-33.9, 151.2), 50.0); // Sydney
        assert_eq!(grid_hz(24.7, 46.7), 60.0); // Riyadh
        assert_eq!(grid_hz(31.8, 35.2), 50.0); // Jerusalem
        assert_eq!(grid_hz(25.2, 55.3), 50.0); // Dubai
    }

    #[test]
    fn wind_follows_q_not_speed() {
        let sea = dynamic_pressure(1.225, 60.0);
        let high = dynamic_pressure(0.04, 60.0); // ~25 km
        assert!(sea > 20.0 * high);
    }

    #[test]
    fn altimeter_and_gpw() {
        assert_eq!(altimeter_rate(3_000.0, 50.0), 0.0);
        assert!(altimeter_rate(1_000.0, 50.0) > altimeter_rate(1_400.0, 50.0));
        assert_eq!(altimeter_rate(700.0, 50.0), 20.0);
        assert_eq!(altimeter_rate(700.0, 2.0), 0.0); // under canopy, slow
        assert_eq!(gpw_level(500.0, 50.0), 0.0); // 10 s out
        assert!(gpw_level(60.0, 50.0) > 0.9);
        assert_eq!(gpw_level(5.0, 6.0), 0.0); // spawn drop / small hop
    }

    #[test]
    fn smoothing_converges() {
        let mut a = Params::default();
        let mut t = Params::default();
        t.q = 1000.0;
        t.veg[3] = 1.0;
        for _ in 0..200 {
            a.smooth_toward(&t, 0.1);
        }
        assert!((a.q - 1000.0).abs() < 0.01);
        assert!((a.veg[3] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn air_gates() {
        let mut p = Params::default();
        assert!(p.air_gate() > 0.99 && p.world_gate() > 0.99);
        p.density = 0.19; // ~15 km
        assert!(p.world_gate() < 0.3 && p.air_gate() > 0.9);
        p.density = 0.0;
        assert_eq!(p.air_gate(), 0.0);
    }
}
