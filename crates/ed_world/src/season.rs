//! Leaf season (trees G5, O3-9): when a tree leafs out, turns and drops its leaves, as a pure
//! function of the date, where it stands and its species' phenology row.
//!
//! * **Phenology** is a species column ([`crate::stand::Pheno`]): day of year of leaf-out, of
//!   the autumn colour peak and of leaf drop at the reference place (latitude 46 deg, 500 m).
//! * **Place** moves those days by Hopkins' bioclimatic law: spring events come about
//!   [`HOPKINS_DAYS`] later per degree of latitude poleward and per [`HOPKINS_M`] m higher
//!   (earlier going the other way). Autumn events move the opposite way, but only by
//!   [`AUTUMN_GAIN`] of the spring shift: the observed autumn gradient with altitude is much
//!   flatter than the spring one (larches at 2000 m still turn in early October). The shift is
//!   clamped to [`SHIFT_MIN`]..[`SHIFT_MAX`] days (the law is a mid-latitude one).
//! * **Southern hemisphere** trees see the date half a year on.
//! * **Rate** scales the year about midsummer: `phase_day = REF_DAY + (day - REF_DAY) * rate`.
//!   Rate 1 is real time; below 1 the seasons creep (the date moves slowly away from
//!   midsummer), above 1 they rush; 0 holds midsummer. Wrapped into the year.
//!
//! The GPU runs the same ramps (`common.slang` `season_leaf`); the table it reads is
//! [`gpu_rows`], the per-tree place word is [`season_word`].

use crate::stand::{Pheno, STAND_SPECIES};

/// Reference latitude (deg) and altitude (m) of the phenology rows.
pub const REF_LAT: f64 = 46.0;
pub const REF_ALT_M: f64 = 500.0;
/// Hopkins: days later per degree of latitude and per `HOPKINS_M` metres of altitude.
pub const HOPKINS_DAYS: f64 = 4.0;
pub const HOPKINS_M: f64 = 122.0;
/// Share of the spring shift that autumn events take, with the opposite sign.
pub const AUTUMN_GAIN: f64 = 0.35;
pub const SHIFT_MIN: f64 = -60.0;
pub const SHIFT_MAX: f64 = 75.0;
/// Midsummer, the day the rate scalar scales about (day of year, 15 July).
pub const REF_DAY: f64 = 196.0;
pub const YEAR: f64 = 365.25;

/// Ramp half-widths (days): leaf-out is complete ~8 days after its day, colour peaks ~14
/// days after its day, drop ends ~10 days after its day. Each starts a little before.
const OUT_RAMP: (f64, f64) = (-6.0, 8.0);
const COLOUR_RAMP: (f64, f64) = (-8.0, 14.0);
const DROP_RAMP: (f64, f64) = (-10.0, 10.0);
/// Evergreen winter dulling: comes on over `WINTER_RAMP` after its day, leaves with leaf-out.
const WINTER_RAMP: (f64, f64) = (-10.0, 30.0);
const SPRING_RELEASE: (f64, f64) = (-10.0, 20.0);

/// A tree's leaf state: `green` 0..1 (leaf-out: 0 bare, 1 full), `colour` 0..1 (how far the
/// leaves have turned toward the species' autumn colour; for evergreens, how far toward the
/// winter colour), `drop` 0..1 (leaves gone). Leaves present = `green * (1 - drop)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeafState {
    pub green: f32,
    pub colour: f32,
    pub drop: f32,
}

impl LeafState {
    /// Share of the summer leaf mass still on the tree.
    pub fn leaves(&self) -> f32 {
        self.green * (1.0 - self.drop)
    }
}

fn smooth(lo: f64, hi: f64, x: f64) -> f32 {
    let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
    (t * t * (3.0 - 2.0 * t)) as f32
}

/// Day after the rate scalar, wrapped into `0..YEAR`.
pub fn phase_day(day: f64, rate: f64) -> f64 {
    (REF_DAY + (day - REF_DAY) * rate).rem_euclid(YEAR)
}

/// Spring shift (days) of a place relative to the reference: positive = spring later, autumn
/// earlier (by [`AUTUMN_GAIN`] of it).
pub fn shift_days(lat: f64, alt_m: f64) -> f64 {
    (HOPKINS_DAYS * (lat.abs() - REF_LAT) + HOPKINS_DAYS * (alt_m - REF_ALT_M) / HOPKINS_M).clamp(SHIFT_MIN, SHIFT_MAX)
}

/// Leaf state on `phase_day` (already through [`phase_day`]) at `lat` (deg), `alt_m`.
pub fn leaf_state_phase(phase: f64, lat: f64, alt_m: f64, p: &Pheno) -> LeafState {
    let d = if lat < 0.0 { (phase + YEAR * 0.5).rem_euclid(YEAR) } else { phase };
    let s = shift_days(lat, alt_m);
    let ds = d - s;
    let da = d + AUTUMN_GAIN * s;
    let (lo, co, dr) = (p.leaf_out as f64, p.colour as f64, p.drop as f64);
    if p.evergreen {
        // Dull from `colour` (autumn) until the spring release.
        let a = smooth(co + WINTER_RAMP.0, co + WINTER_RAMP.1, da);
        let r = 1.0 - smooth(lo + SPRING_RELEASE.0, lo + SPRING_RELEASE.1, ds);
        return LeafState { green: 1.0, colour: p.winter * a.max(r), drop: 0.0 };
    }
    LeafState {
        green: smooth(lo + OUT_RAMP.0, lo + OUT_RAMP.1, ds),
        colour: smooth(co + COLOUR_RAMP.0, co + COLOUR_RAMP.1, da),
        drop: smooth(dr + DROP_RAMP.0, dr + DROP_RAMP.1, da),
    }
}

/// Leaf state on calendar `day` (day of year, fractional) at `lat`, `alt_m` with the global
/// `rate` scalar (1 = real time).
pub fn leaf_state(day: f64, lat: f64, alt_m: f64, p: &Pheno, rate: f64) -> LeafState {
    leaf_state_phase(phase_day(day, rate), lat, alt_m, p)
}

/// Day of year (1 = 1 January, fractional 0 at midnight) of a Gregorian date.
pub fn day_of_year(y: i32, m: u32, d: u32) -> f64 {
    const CUM: [u32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let mut n = CUM[(m.clamp(1, 12) - 1) as usize] + d;
    if leap && m > 2 {
        n += 1;
    }
    n as f64
}

/// The per-tree place word the GPU reads: bits 0..8 species row, bit 8 southern hemisphere,
/// bits 16..32 the spring shift in 1/16 day (signed). `common.slang` `season_leaf`.
pub fn season_word(species: usize, lat: f64, alt_m: f64) -> u32 {
    let s = (shift_days(lat, alt_m) * 16.0).round() as i32 as i16 as u16 as u32;
    (species as u32 & 0xFF) | ((lat < 0.0) as u32) << 8 | s << 16
}

fn lin(c: u32) -> f32 {
    let x = c as f32 / 255.0;
    if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
}

/// One species' GPU rows (two float4s): `[leaf_out, colour, drop, evergreen]`, then
/// `[target albedo rgb (linear), winter share (evergreen)]`.
pub fn gpu_row(p: &Pheno) -> [f32; 8] {
    let h = p.colour_hex;
    [p.leaf_out, p.colour, p.drop, if p.evergreen { 1.0 } else { 0.0 }, lin(h >> 16 & 255), lin(h >> 8 & 255), lin(h & 255), p.winter]
}

/// Rows the GPU season table holds.
pub const GPU_ROWS: usize = 32;

/// The whole table, one row per [`STAND_SPECIES`] entry.
pub fn gpu_rows() -> Vec<[f32; 8]> {
    assert!(STAND_SPECIES.len() <= GPU_ROWS, "season table: {} species", STAND_SPECIES.len());
    STAND_SPECIES.iter().map(|s| gpu_row(&s.pheno)).collect()
}

/// Rows the far rung (no per-tree data) uses for its broadleaf and conifer canopy.
pub fn far_rows() -> [u32; 2] {
    let find = |n: &str| STAND_SPECIES.iter().position(|s| s.name == n).expect("generic row") as u32;
    [find("Broadleaf (generic)"), find("Spruce (generic)")]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(name: &str) -> Pheno {
        STAND_SPECIES.iter().find(|s| s.name == name).unwrap().pheno
    }

    #[test]
    fn monotone_through_the_year() {
        let beech = row("European beech");
        let mut prev = leaf_state(1.0, 46.0, 500.0, &beech, 1.0);
        assert_eq!((prev.green, prev.drop), (0.0, 0.0));
        let mut peak_green = 0.0f32;
        for d in 2..=365 {
            let s = leaf_state(d as f64, 46.0, 500.0, &beech, 1.0);
            if d < 190 {
                assert!(s.green >= prev.green - 1e-6, "green {d}");
                peak_green = peak_green.max(s.green);
            } else {
                assert!(s.colour >= prev.colour - 1e-6 && s.drop >= prev.drop - 1e-6, "autumn {d}");
            }
            prev = s;
        }
        assert!(peak_green > 0.99);
        assert!(prev.drop > 0.99 && prev.colour > 0.99, "{prev:?}");
        // Midsummer is full green, unturned.
        let s = leaf_state(196.0, 46.0, 500.0, &beech, 1.0);
        assert_eq!((s.green, s.colour, s.drop), (1.0, 0.0, 0.0));
    }

    #[test]
    fn hopkins_shifts() {
        let b = row("European beech");
        // Day on which leaf-out reaches one half.
        let half = |lat: f64, alt: f64| (1..250).map(|d| d as f64).find(|&d| leaf_state(d, lat, alt, &b, 1.0).green >= 0.5).unwrap();
        let d0 = half(46.0, 500.0);
        let d1 = half(47.0, 500.0);
        assert!((d1 - d0 - 4.0).abs() <= 1.0, "{d0} {d1}");
        let d2 = half(46.0, 622.0);
        assert!((d2 - d0 - 4.0).abs() <= 1.0, "{d0} {d2}");
        // Autumn is earlier (not later) poleward, by AUTUMN_GAIN of the spring shift.
        let half_drop = |lat: f64| (200..366).map(|d| d as f64).find(|&d| leaf_state(d, lat, 500.0, &b, 1.0).drop >= 0.5).unwrap();
        let (a0, a1) = (half_drop(46.0), half_drop(56.0));
        assert!(a1 < a0 && ((a0 - a1) - 10.0 * 4.0 * AUTUMN_GAIN).abs() <= 1.5, "{a0} {a1}");
    }

    #[test]
    fn southern_hemisphere_is_half_a_year_out() {
        let b = row("European beech");
        let n = leaf_state(196.0, 46.0, 500.0, &b, 1.0);
        let s = leaf_state(196.0, -46.0, 500.0, &b, 1.0);
        assert_eq!(n.green, 1.0);
        assert_eq!(s.green, 0.0, "southern July is winter");
        let s2 = leaf_state(196.0 + YEAR * 0.5, -46.0, 500.0, &b, 1.0);
        assert!(s2.green > 0.99);
    }

    #[test]
    fn rate_scales_the_year_about_midsummer() {
        let b = row("European beech");
        assert_eq!(phase_day(120.0, 1.0), 120.0);
        // Rate 0 holds midsummer; rate 0.5 puts 20 October (day 293) in early September.
        assert!((phase_day(293.0, 0.0) - REF_DAY).abs() < 1e-9);
        assert!((phase_day(293.0, 0.5) - 244.5).abs() < 1e-9);
        let s = leaf_state(293.0, 46.0, 500.0, &b, 0.0);
        assert_eq!((s.green, s.colour), (1.0, 0.0));
        // Real time at day 305: turned.
        assert!(leaf_state(305.0, 46.0, 500.0, &b, 1.0).colour > 0.9);
    }

    #[test]
    fn evergreens_stay_and_larch_drops() {
        let spruce = row("Norway spruce");
        for d in (1..=365).step_by(7) {
            let s = leaf_state(d as f64, 46.0, 500.0, &spruce, 1.0);
            assert_eq!((s.green, s.drop), (1.0, 0.0));
            assert!(s.colour <= 0.6 + 1e-6);
        }
        assert_eq!(leaf_state(196.0, 46.0, 500.0, &spruce, 1.0).colour, 0.0);
        assert!(leaf_state(350.0, 46.0, 500.0, &spruce, 1.0).colour > 0.5);
        let larch = row("European larch");
        let (oct, dec) = (leaf_state(290.0, 46.0, 1800.0, &larch, 1.0), leaf_state(355.0, 46.0, 1800.0, &larch, 1.0));
        assert!(oct.colour > 0.2 && oct.drop < 0.5, "{oct:?}");
        assert!(dec.drop > 0.95, "{dec:?}");
        let tropic = row("Tropical hardwood");
        for d in (1..=365).step_by(30) {
            assert_eq!(leaf_state(d as f64, 0.0, 100.0, &tropic, 1.0).colour, 0.0);
        }
    }

    #[test]
    fn dates_and_words() {
        assert_eq!(day_of_year(2026, 1, 1), 1.0);
        assert_eq!(day_of_year(2026, 10, 20), 293.0);
        assert_eq!(day_of_year(2024, 3, 1), 61.0);
        let w = season_word(5, 46.0, 500.0 + 122.0 * 2.0);
        assert_eq!(w & 255, 5);
        assert_eq!(((w >> 16) as u16 as i16) as f64 / 16.0, 8.0);
        let s = season_word(1, -30.0, 0.0);
        assert_eq!((s >> 8) & 1, 1);
        assert!(((s >> 16) as u16 as i16) < 0);
        assert_eq!(gpu_rows().len(), STAND_SPECIES.len());
    }
}
