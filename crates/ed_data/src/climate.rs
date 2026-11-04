//! Static climate normals: WorldClim 2.1 (Fick & Hijmans 2017, CC BY 4.0) at
//! 10 arc-minutes, baked by `tools/bake_climate.py` and embedded in the binary,
//! so biomes come from real climate, work offline and are deterministic.
//!
//! Temperatures are stored reduced to sea level; [`Climate::temp_at`] puts the
//! terrain's real elevation back with a lapse rate. This is the long-term
//! *climate* (what grows here), not weather: the procedural weather sim
//! (`ed_climate`) plays on top of it.

use std::sync::OnceLock;

static BLOB: &[u8] = include_bytes!("../data/climate10.bin");

struct Grid {
    w: usize,
    h: usize,
    planes: [Vec<u8>; 5],
}

fn grid() -> &'static Grid {
    static G: OnceLock<Grid> = OnceLock::new();
    G.get_or_init(|| {
        use std::io::Read;
        let mut raw = Vec::new();
        flate2::read::ZlibDecoder::new(BLOB).read_to_end(&mut raw).expect("climate10.bin is corrupt");
        assert_eq!(&raw[..4], b"CLM1");
        let w = u16::from_le_bytes([raw[4], raw[5]]) as usize;
        let h = u16::from_le_bytes([raw[6], raw[7]]) as usize;
        let n = w * h;
        let plane = |i: usize| raw[8 + i * n..8 + (i + 1) * n].to_vec();
        Grid { w, h, planes: [plane(0), plane(1), plane(2), plane(3), plane(4)] }
    })
}

/// Climate normals at a point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Climate {
    /// Mean annual temperature reduced to sea level (°C).
    pub temp_sl: f32,
    /// Mean temperature of the coldest month, sea-level reduced (°C).
    pub tmin_sl: f32,
    /// Annual precipitation (mm).
    pub precip_mm: f32,
    /// Share of annual precipitation falling in the warmest quarter (0..1).
    pub warm_share: f32,
    /// Share falling in the coldest quarter (0..1).
    pub cold_share: f32,
}

const LAPSE: f32 = 0.0065;

impl Climate {
    /// Bilinear sample; `None` over open ocean far from any land.
    pub fn at(lat: f64, lon: f64) -> Option<Climate> {
        let g = grid();
        let fx = (lon + 180.0) / 360.0 * g.w as f64 - 0.5;
        let fy = (90.0 - lat) / 180.0 * g.h as f64 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = ((fx - x0) as f32, (fy - y0) as f32);
        let mut acc = [0.0f32; 5];
        let mut wsum = 0.0;
        for (dy, dx, wgt) in [(0, 0, (1.0 - tx) * (1.0 - ty)), (0, 1, tx * (1.0 - ty)), (1, 0, (1.0 - tx) * ty), (1, 1, tx * ty)] {
            let x = (x0 as i64 + dx).rem_euclid(g.w as i64) as usize;
            let y = (y0 as i64 + dy).clamp(0, g.h as i64 - 1) as usize;
            let i = y * g.w + x;
            if g.planes[0][i] == 255 || wgt <= 0.0 {
                continue;
            }
            for (a, p) in acc.iter_mut().zip(&g.planes) {
                *a += p[i] as f32 * wgt;
            }
            wsum += wgt;
        }
        if wsum <= 1e-4 {
            return None;
        }
        let v = acc.map(|a| a / wsum);
        let p = v[2] / 2.4;
        Some(Climate {
            temp_sl: v[0] / 3.0 - 40.0,
            tmin_sl: v[1] / 3.0 - 40.0,
            precip_mm: p * p,
            warm_share: v[3] / 255.0,
            cold_share: v[4] / 255.0,
        })
    }

    /// Mean annual temperature at `elev_m` above sea level.
    pub fn temp_at(&self, elev_m: f32) -> f32 {
        self.temp_sl - LAPSE * elev_m.max(0.0)
    }

    /// Coldest-month mean temperature at `elev_m`.
    pub fn tmin_at(&self, elev_m: f32) -> f32 {
        self.tmin_sl - LAPSE * elev_m.max(0.0)
    }

    /// Dry-summer (Mediterranean) signal: the warm quarter gets well under a
    /// third as much rain as the cold one, and the year is not desert-dry.
    pub fn dry_summer(&self) -> bool {
        self.warm_share < 0.6 * self.cold_share && self.warm_share < 0.14
    }

    /// UNEP-style aridity: precipitation over a Thornthwaite-ish potential
    /// evapotranspiration proxy (hotter = thirstier). < 0.2 desert, < 0.5 dry.
    pub fn aridity(&self, elev_m: f32) -> f32 {
        let t = self.temp_at(elev_m).max(0.0);
        let pet = (20.0 * t + 280.0).max(300.0) * if t > 5.0 { 1.0 } else { 0.7 };
        self.precip_mm / pet
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_places() {
        let sahara = Climate::at(23.4, 25.6).unwrap();
        assert!(sahara.precip_mm < 40.0, "{sahara:?}");
        assert!(sahara.aridity(300.0) < 0.1);
        let manaus = Climate::at(-3.1, -60.0).unwrap();
        assert!(manaus.precip_mm > 1800.0 && manaus.tmin_sl > 22.0, "{manaus:?}");
        let aix = Climate::at(43.5, 5.4).unwrap();
        assert!(aix.dry_summer(), "{aix:?}");
        let london = Climate::at(51.5, -0.1).unwrap();
        assert!(!london.dry_summer() && london.precip_mm > 500.0 && london.precip_mm < 900.0, "{london:?}");
        let siberia = Climate::at(62.0, 129.7).unwrap(); // Yakutsk
        assert!(siberia.temp_sl < -4.0 && siberia.tmin_sl < -30.0, "{siberia:?}");
        let cairo_delta = Climate::at(31.0, 31.0).unwrap();
        assert!(cairo_delta.precip_mm < 200.0);
    }

    #[test]
    fn open_ocean_is_none() {
        assert!(Climate::at(0.0, -140.0).is_none());
    }

    #[test]
    fn lapse_rate_cools() {
        let c = Climate::at(46.0, 7.7).unwrap();
        assert!(c.temp_at(3000.0) < c.temp_at(0.0) - 15.0);
    }
}
