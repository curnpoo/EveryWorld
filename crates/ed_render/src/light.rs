//! Baseline lighting, CPU side (M4d): the switches and tunables ([`LightingSettings`]), the sky
//! model's parameters for a sun direction ([`Sky`], the alpha's analytic sky), its diffuse
//! light as SH9 ([`sky_sh9`], baked only when the sun moves), and the cascaded shadow fit
//! ([`plan_cascades`]). [`light_words`] packs it all into `FrameData.light` (common.slang
//! `LightData`, [`crate::gpu::FD_LIGHT`]).
//!
//! Cascades (shape of docs/alpha/plans/shadows.md stage 1, built in ed_render): the view depth
//! up to the shadow distance is split by the practical scheme (log/linear blend `lambda`, or
//! explicit split distances); each slice gets a bounding sphere (rotation-invariant size), the
//! sphere's centre is snapped to whole texels in a light frame anchored to the f64 world (no
//! shimmer when the camera moves or turns, and no snap jump when the origin re-anchors), and
//! the orthographic box is the sphere's light-space square plus `caster_extend` towards the
//! sun. The radius is inflated so the slice never reaches the filter margin or the blend band;
//! the band is where the next cascade blends in, and the last cascade fades out.

use crate::Camera;
use glam::{DVec3, Mat4, Vec3, Vec4};

/// Most cascades a frame can have (the atlas holds 2 x 2 tiles).
pub const MAX_CASCADES: usize = 4;
/// `LightData` words (common.slang).
pub const LIGHT_WORDS: usize = 144;

/// Scene exposure into the tone map (the `FrameParams` default), set photographically: the
/// "sunny 16" rule (EV100 15) for a clear day. The sun is 6.4 units for about 100 klx, so a
/// radiance of 1 is about 15.6 kcd/m2; EV100 15 meters 4096 cd/m2 (0.26 units) as mid-grey,
/// which the film curve expects at 0.18: 0.18 / 0.26 = 0.7. Sunlit concrete (albedo 0.3) then
/// lands about 1.3 stops over mid-grey and white paint stays below clipping. The alpha ran 1.7
/// ([`crate::tonemap::ALPHA_EXPOSURE`]), 1.3 stops over, which pushed every sunlit surface into
/// the curve's shoulder (pastel, with shadow and AO contrast compressed).
pub const EXPOSURE: f32 = 0.7;

/// Lighting switches and tunables. Every feature has an off switch (P1 rule): `--no-shadows`,
/// `--no-ao`, `--no-sky-ambient` in `ew`, or `EW_NO_SHADOWS=1`, `EW_NO_AO=1`,
/// `EW_NO_SKY_AMBIENT=1`, `EW_NO_SKYVIS=1` ([`LightingSettings::from_env`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightingSettings {
    /// Cascaded sun shadows (cost: shadow_cull + shadow_raster passes, PCF in resolve).
    pub shadows: bool,
    /// GTAO (cost: the "ao" pass, upsample in resolve).
    pub ao: bool,
    /// Sky ambient as SH9 and the sky specular term; off falls back to a flat hemisphere.
    pub sky_ambient: bool,
    /// Cascade count, 1..=4.
    pub cascades: u32,
    /// Shadow distance (view depth, m).
    pub shadow_distance: f32,
    /// Practical split scheme: 0 = linear, 1 = logarithmic.
    pub split_lambda: f32,
    /// Shadow filter (penumbra) radius in metres, the same in every cascade where the texel
    /// size allows (`filter_texels`).
    pub shadow_filter_m: f32,
    /// Largest filter radius in texels (tap budget: the 5x5 grid is spread over it).
    pub shadow_filter_max_texels: f32,
    /// Explicit cascade far distances (m), overriding `split_lambda` (the first `cascades` used).
    pub splits: Option<[f32; MAX_CASCADES]>,
    /// Texels per cascade tile side.
    pub shadow_resolution: u32,
    /// Blend band at a cascade's edge, fraction of its half-width.
    pub cascade_blend: f32,
    /// The last cascade fades to lit from this fraction of the shadow distance.
    pub fade_start: f32,
    /// Extra depth towards the sun for casters outside the view (m); casters beyond it are
    /// clamped onto the box ("pancaking").
    pub caster_extend: f32,
    /// Receiver offset along the geometric normal, texels.
    pub normal_bias: f32,
    /// Receiver depth bias, texels.
    pub depth_bias: f32,
    /// GTAO world radius (m).
    pub ao_radius: f32,
    /// GTAO strength (exponent on visibility).
    pub ao_intensity: f32,
    /// Large-scale sky visibility (M4f): the GTAO slices are searched a second time out to
    /// [`Self::sky_vis_radius`], and the share of the sky that street canyons hide is lit by
    /// bounce light instead of skylight. Needs `ao`. Cost: about 0.1 ms in the "ao" pass at
    /// 1280x720 (four more taps per slice side), nothing extra in resolve.
    pub sky_vis: bool,
    /// Sky-visibility search radius (m): the canyon scale (the proving ground's streets are
    /// 20 m wide between buildings up to 120 m). Screen-space, so occluders off screen do not
    /// count, and the search stops at 0.75 of the screen height.
    pub sky_vis_radius: f32,
    /// Radiance of the occluded sky, as a fraction of the ground-bounce radiance (the lower
    /// hemisphere of the ambient): building faces are lit about like the street, a bit less
    /// (half of them face away from the sun).
    pub sky_vis_fill: f32,
    /// Diffuse skylight strength relative to the visible sky model. The analytic sky is a clear
    /// gradient with no haze or multiple scattering, so its integral is about half of real
    /// skylight against the sun; 2.1 puts sky : sun on open ground at 0.20 for the default
    /// sun (48 degrees up): about 100 klx direct, 15-25 klx diffuse on a clear day.
    pub sky_ambient_gain: f32,
    /// Saturation of the diffuse skylight (1 = the sky model's colours, 0 = grey). The visible
    /// sky is a saturated blue (R : B 0.13 from above); real clear-sky irradiance is about
    /// 10000-15000 K, R : B near 0.5, which 0.3 gives.
    pub sky_ambient_saturation: f32,
    /// Albedo of the surroundings below the horizon for the bounce term: the lower hemisphere
    /// of the ambient light is this times the light reaching the ground (sun and sky). A city
    /// mix of asphalt (0.1) and concrete or stone (0.3), slightly warm.
    pub bounce_albedo: Vec3,
    /// Fraction of the surrounding ground taken as sunlit for the bounce term (streets are
    /// partly in shadow).
    pub bounce_sunlit: f32,
    /// AO multi-bounce (Jimenez 2016): occlusion is weaker on bright albedo, where light
    /// bouncing inside the crease fills it back in. 0 off, 1 full.
    pub ao_multibounce: f32,
    /// Debug view in resolve (`EW_LIGHT_DEBUG`): 0 off, 1 sun visibility tinted by cascade
    /// (red, green, blue, yellow; grey past the last), 2 ambient occlusion, 3 receiver minus
    /// stored light depth (red: receiver nearer the sun, green: behind an occluder), 4 sky
    /// visibility.
    pub debug: u32,
}

impl Default for LightingSettings {
    fn default() -> Self {
        Self {
            shadows: true,
            ao: true,
            sky_ambient: true,
            cascades: 4,
            shadow_distance: 1000.0,
            split_lambda: 0.95,
            shadow_filter_m: 0.4,
            shadow_filter_max_texels: 8.0,
            splits: None,
            shadow_resolution: 2048,
            cascade_blend: 0.15,
            fade_start: 0.45,
            caster_extend: 150.0,
            normal_bias: 1.0,
            depth_bias: 1.0,
            ao_radius: 2.5,
            ao_intensity: 2.0,
            sky_vis: true,
            sky_vis_radius: 60.0,
            sky_vis_fill: 0.8,
            sky_ambient_gain: 2.1,
            sky_ambient_saturation: 0.3,
            bounce_albedo: Vec3::new(0.25, 0.22, 0.18),
            bounce_sunlit: 0.5,
            ao_multibounce: 1.0,
            debug: 0,
        }
    }
}

impl LightingSettings {
    /// Defaults with the `EW_NO_SHADOWS`, `EW_NO_AO`, `EW_NO_SKY_AMBIENT` switches and
    /// `EW_LIGHT_DEBUG=shadow|ao|depth|skyvis` applied.
    pub fn from_env() -> Self {
        let on = |v: &str| std::env::var(v).is_ok_and(|s| s != "0" && !s.is_empty());
        let debug = match std::env::var("EW_LIGHT_DEBUG").as_deref() {
            Ok("shadow") | Ok("1") => 1,
            Ok("ao") | Ok("2") => 2,
            Ok("depth") | Ok("3") => 3,
            Ok("skyvis") | Ok("4") => 4,
            _ => 0,
        };
        Self { shadows: !on("EW_NO_SHADOWS"), ao: !on("EW_NO_AO"), sky_ambient: !on("EW_NO_SKY_AMBIENT"), sky_vis: !on("EW_NO_SKYVIS"), debug, ..Self::default() }
    }
    /// Cascades actually drawn (0 with shadows off).
    pub fn cascade_count(&self) -> u32 {
        if self.shadows { self.cascades.clamp(1, MAX_CASCADES as u32) } else { 0 }
    }
    /// Shadow atlas size in texels (2 tiles wide).
    pub fn atlas_size(&self) -> (u32, u32) {
        let n = self.cascade_count().max(1);
        let r = self.shadow_resolution.max(64);
        (if n > 1 { 2 * r } else { r }, n.div_ceil(2) * r)
    }
}

/// The sky model's inputs for one sun direction (the alpha's constants).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    /// Unit vector towards the sun (render axes: x east, y up, z south).
    pub sun_dir: Vec3,
    /// Daylight factor: clamp(sun.y * 6, 0.03, 1).
    pub day: f32,
    /// Sun irradiance (linear).
    pub sun_color: Vec3,
    pub zenith: Vec3,
    pub horizon: Vec3,
    pub ground: Vec3,
}

impl Sky {
    pub fn for_sun(dir: Vec3) -> Self {
        let s = dir.normalize_or(Vec3::Y);
        let day = (s.y * 6.0).clamp(0.03, 1.0);
        let warm = Vec3::new(1.0, 0.62, 0.34).lerp(Vec3::new(1.0, 0.96, 0.88), (s.y * 2.5).clamp(0.0, 1.0));
        // Below the horizon the sun stops lighting (the alpha never went there).
        let horizon_gate = (1.0 + s.y * 20.0).clamp(0.0, 1.0);
        Self {
            sun_dir: s,
            day,
            sun_color: warm * 6.4 * day * horizon_gate,
            zenith: Vec3::new(0.025, 0.09, 0.40),
            horizon: Vec3::new(0.36, 0.55, 0.95),
            ground: Vec3::new(0.30, 0.31, 0.32),
        }
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Sky radiance towards `d` (common.slang `sky_rad`, line for line).
pub fn sky_rad(sky: &Sky, d: Vec3, disc_k: f32) -> Vec3 {
    let sun = sky.sun_dir;
    let h = d.y;
    let t = (1.0 - h.max(0.0)).clamp(0.0, 1.0).powf(6.5);
    let mut c = sky.zenith.lerp(sky.horizon, t);
    let mu = d.dot(sun);
    let warm = (1.0 - sun.y * 2.5).clamp(0.0, 1.0);
    c += Vec3::new(1.0, 0.42, 0.14) * (mu * 0.5 + 0.5).clamp(0.0, 1.0).powf(5.0) * warm * 0.85 * (0.4 + t);
    c = c.lerp(c * Vec3::new(1.0, 0.7, 0.55), warm * 0.4);
    c += Vec3::new(1.0, 0.9, 0.7) * mu.clamp(0.0, 1.0).powf(28.0) * 0.35;
    c = c.lerp(sky.ground, (-h * 6.0).clamp(0.0, 1.0));
    let disk = smoothstep(0.99965, 0.99985, mu) * if h >= 0.0 { 1.0 } else { 0.0 };
    c * sky.day + Vec3::new(1.0, 0.95, 0.85) * disk * 60.0 * sky.day * disc_k
}

/// The shader's SH9 polynomial terms (lighting.slang `sh_irradiance`).
fn sh_poly(n: Vec3) -> [f32; 9] {
    [1.0, n.y, n.z, n.x, n.x * n.y, n.y * n.z, 3.0 * n.z * n.z - 1.0, n.x * n.z, n.x * n.x - n.y * n.y]
}

/// Real SH basis constants matching [`sh_poly`].
const SH_K: [f32; 9] = [0.282095, 0.488603, 0.488603, 0.488603, 1.092548, 1.092548, 0.315392, 1.092548, 0.546274];
/// Cosine-lobe convolution per band divided by pi: (pi, 2pi/3, pi/4) / pi.
const SH_A: [f32; 9] = [1.0, 2.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0, 0.25, 0.25, 0.25, 0.25, 0.25];

/// Diffuse sky light as SH9 for the shader: coefficient i times `sh_poly(n)[i]`, summed, is the
/// irradiance at normal `n` divided by pi (no sun disc: the direct term has it). Numerical
/// integration over 64 x 128 directions; deterministic (fixed order, f64 sums).
pub fn sky_sh9(sky: &Sky) -> [[f32; 4]; 9] {
    sh9_of(|d| sky_rad(sky, d, 0.0))
}

/// SH9 (shader form, see [`sky_sh9`]) of a radiance function over the sphere.
fn sh9_of(rad: impl Fn(Vec3) -> Vec3) -> [[f32; 4]; 9] {
    const NT: usize = 64;
    const NP: usize = 128;
    let mut acc = [[0.0f64; 3]; 9];
    for i in 0..NT {
        let ct = 1.0 - 2.0 * (i as f64 + 0.5) / NT as f64; // uniform in cos(theta): equal-area bands
        let st = (1.0 - ct * ct).max(0.0).sqrt();
        for j in 0..NP {
            let ph = (j as f64 + 0.5) / NP as f64 * std::f64::consts::TAU;
            let d = Vec3::new((st * ph.cos()) as f32, (st * ph.sin()) as f32, ct as f32);
            let l = rad(d);
            let y = sh_poly(d);
            for k in 0..9 {
                let w = (y[k] * SH_K[k]) as f64;
                acc[k][0] += l.x as f64 * w;
                acc[k][1] += l.y as f64 * w;
                acc[k][2] += l.z as f64 * w;
            }
        }
    }
    let d_omega = 4.0 * std::f64::consts::PI / (NT * NP) as f64;
    let mut out = [[0.0f32; 4]; 9];
    for k in 0..9 {
        let s = d_omega * (SH_A[k] * SH_K[k]) as f64;
        out[k] = [(acc[k][0] * s) as f32, (acc[k][1] * s) as f32, (acc[k][2] * s) as f32, 0.0];
    }
    out
}

/// Ambient light as SH9 for the shader: the sky ([`sky_sh9`] without its flat ground colour)
/// scaled and desaturated to real skylight (`sky_ambient_gain`, `sky_ambient_saturation`), plus
/// the bounce off the surroundings in the lower hemisphere: `bounce_albedo` times the light on
/// open ground (the sun over `bounce_sunlit` of it, and the skylight). Baked when the sun or
/// the settings change.
pub fn ambient_sh9(sky: &Sky, s: &LightingSettings) -> [[f32; 4]; 9] {
    let open = Sky { ground: Vec3::ZERO, ..*sky };
    let mut sh = sh9_of(|d| sky_rad(&open, d, 0.0));
    for c in &mut sh {
        let v = Vec3::new(c[0], c[1], c[2]);
        let l = Vec3::splat(v.dot(Vec3::new(0.2126, 0.7152, 0.0722)));
        let v = l.lerp(v, s.sky_ambient_saturation) * s.sky_ambient_gain;
        *c = [v.x, v.y, v.z, 0.0];
    }
    // Radiance leaving lit ground (Lambert): albedo * irradiance / pi.
    let sky_up = sh_eval(&sh, Vec3::Y);
    let sun_up = sky.sun_color * sky.sun_dir.y.max(0.0) / std::f32::consts::PI * s.bounce_sunlit.clamp(0.0, 1.0);
    let bounce = s.bounce_albedo * (sun_up + sky_up);
    // The same horizon blend as the sky model's ground.
    let below = sh9_of(|d| bounce * (-d.y * 6.0).clamp(0.0, 1.0));
    for k in 0..9 {
        for i in 0..3 {
            sh[k][i] += below[k][i];
        }
    }
    sh
}

/// Evaluates [`sky_sh9`] coefficients at `n` (the shader's `sh_irradiance`).
pub fn sh_eval(sh: &[[f32; 4]; 9], n: Vec3) -> Vec3 {
    let y = sh_poly(n);
    let mut c = Vec3::ZERO;
    for k in 0..9 {
        c += Vec3::new(sh[k][0], sh[k][1], sh[k][2]) * y[k];
    }
    c.max(Vec3::ZERO)
}

/// Cascade far distances (view depth, m).
pub fn cascade_splits(s: &LightingSettings, near: f32) -> Vec<f32> {
    let n = s.cascade_count().max(1) as usize;
    if let Some(sp) = s.splits {
        return sp[..n].to_vec();
    }
    let (lo, hi) = (near.max(1.0), s.shadow_distance.max(near.max(1.0) + 1.0));
    (1..=n)
        .map(|i| {
            let f = i as f32 / n as f32;
            let log = lo * (hi / lo).powf(f);
            let lin = lo + (hi - lo) * f;
            s.split_lambda * log + (1.0 - s.split_lambda) * lin
        })
        .collect()
}

/// Smallest PCF radius in texels (mirrors `PCF_KMIN` in lighting.slang).
pub const PCF_KMIN: f32 = 1.0;

/// PCF radius of a cascade with `texel` metres per texel, in texels (mirrors
/// `pcf_radius_texels` in lighting.slang): the world filter radius, clamped to
/// [`PCF_KMIN`, `shadow_filter_max_texels`].
pub fn filter_texels(s: &LightingSettings, texel: f32) -> f32 {
    (s.shadow_filter_m / texel).clamp(PCF_KMIN, s.shadow_filter_max_texels.max(PCF_KMIN))
}

/// One frame's cascades.
#[derive(Clone, Debug, PartialEq)]
pub struct CascadePlan {
    pub count: u32,
    pub tile: u32,
    /// Camera-relative world -> cascade clip.
    pub matrices: [Mat4; MAX_CASCADES],
    /// Per cascade: texel size (m), depth range (m), tile origin x, y (px).
    pub cascade: [[f32; 4]; MAX_CASCADES],
    pub splits: [f32; MAX_CASCADES],
    /// Absolute (f64) snapped centre of each cascade, for tests.
    pub centres: [DVec3; MAX_CASCADES],
}

/// Filter margin kept free at a tile's edge (texels): the 4x4 PCF footprint plus normal offset.
pub const EDGE_MARGIN_TEXELS: f32 = 3.0;

/// Light frame: z towards the sun; x, y fixed for a given sun.
pub fn light_basis(sun: Vec3) -> (DVec3, DVec3, DVec3) {
    let lz = sun.normalize_or(Vec3::Y).as_dvec3();
    let r = if lz.y.abs() > 0.99 { DVec3::Z } else { DVec3::Y };
    let lx = r.cross(lz).normalize();
    let ly = lz.cross(lx);
    (lx, ly, lz)
}

/// Fits the cascades for `cam` (aspect = width / height) and `sun`.
pub fn plan_cascades(s: &LightingSettings, cam: &Camera, aspect: f32, sun: Vec3) -> CascadePlan {
    let n = s.cascade_count() as usize;
    let tile = s.shadow_resolution.max(64);
    let mut plan = CascadePlan { count: n as u32, tile, matrices: [Mat4::IDENTITY; MAX_CASCADES], cascade: [[0.0; 4]; MAX_CASCADES], splits: [0.0; MAX_CASCADES], centres: [DVec3::ZERO; MAX_CASCADES] };
    if n == 0 {
        return plan;
    }
    let splits = cascade_splits(s, cam.near);
    let ty = (0.5 * cam.fov_y).tan() as f64;
    let tx = ty * aspect as f64;
    let t2 = tx * tx + ty * ty;
    let (lx, ly, lz) = light_basis(sun);
    let fwd = cam.forward().as_dvec3();
    // Slice points must stay inside the margin and out of the blend band.
    let shrink = (1.0 - EDGE_MARGIN_TEXELS as f64 * 2.0 / tile as f64) * (1.0 - s.cascade_blend.clamp(0.0, 0.5) as f64);
    let mut near = cam.near as f64;
    for c in 0..n {
        let far = splits[c] as f64;
        // Bounding sphere of the slice [near, far]: centre on the view axis.
        let zc = (0.5 * (far + near) * (1.0 + t2)).min(far);
        let r_fit = ((far - zc).powi(2) + far * far * t2).sqrt().max(((zc - near).powi(2) + near * near * t2).sqrt());
        // Quantised so the size (and texel) never drifts with float noise.
        let r = ((r_fit / shrink) * 64.0).ceil() / 64.0;
        let texel = 2.0 * r / tile as f64;
        let centre = cam.position + fwd * zc;
        let (cx, cy) = (centre.dot(lx), centre.dot(ly));
        let snapped = centre + lx * ((cx / texel).round() * texel - cx) + ly * ((cy / texel).round() * texel - cy);
        let rel = (snapped - cam.position).as_vec3();
        let depth = 2.0 * r + s.caster_extend.max(0.0) as f64;
        let (lxf, lyf, lzf) = (lx.as_vec3(), ly.as_vec3(), lz.as_vec3());
        let (rf, df) = (r as f32, depth as f32);
        // Rows: x = (p - c).lx / r, y = (p - c).ly / r, z = ((p - c).lz + r) / depth.
        let rows = [
            Vec4::new(lxf.x / rf, lxf.y / rf, lxf.z / rf, -rel.dot(lxf) / rf),
            Vec4::new(lyf.x / rf, lyf.y / rf, lyf.z / rf, -rel.dot(lyf) / rf),
            Vec4::new(lzf.x / df, lzf.y / df, lzf.z / df, (rf - rel.dot(lzf)) / df),
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        ];
        plan.matrices[c] = Mat4::from_cols(rows[0], rows[1], rows[2], rows[3]).transpose();
        plan.cascade[c] = [texel as f32, df, ((c % 2) as u32 * tile) as f32, ((c / 2) as u32 * tile) as f32];
        plan.splits[c] = far as f32;
        plan.centres[c] = snapped;
        near = far;
    }
    plan
}

/// `LightData` words.
pub fn light_words(s: &LightingSettings, plan: &CascadePlan, sh: &[[f32; 4]; 9], ao_half: (u32, u32), sun_disc: f32) -> [u32; LIGHT_WORDS] {
    fn put(w: &mut [u32], at: usize, v: &[f32]) {
        for (i, x) in v.iter().enumerate() {
            w[at + i] = x.to_bits();
        }
    }
    let mut w = [0u32; LIGHT_WORDS];
    for c in 0..MAX_CASCADES {
        put(&mut w, c * 16, &plan.matrices[c].to_cols_array());
        put(&mut w, 64 + c * 4, &plan.cascade[c]);
    }
    put(&mut w, 80, &plan.splits);
    put(&mut w, 84, &[plan.count as f32, plan.tile as f32, s.cascade_blend.clamp(0.0, 0.5), EDGE_MARGIN_TEXELS]);
    let last = plan.splits[(plan.count.max(1) - 1) as usize];
    put(&mut w, 88, &[last * s.fade_start, last, s.normal_bias, s.depth_bias]);
    w[92..96].copy_from_slice(&[(plan.count > 0) as u32, s.ao as u32, s.sky_ambient as u32, s.debug]);
    put(&mut w, 96, &[s.ao_intensity, ao_half.0 as f32, ao_half.1 as f32, s.ao_radius]);
    put(&mut w, 100, &[sun_disc, s.ao_multibounce.clamp(0.0, 1.0), s.shadow_filter_m, s.shadow_filter_max_texels]);
    for (k, c) in sh.iter().enumerate() {
        put(&mut w, 104 + k * 4, c);
    }
    put(&mut w, 140, &[(s.ao && s.sky_vis) as u32 as f32, s.sky_vis_fill.max(0.0), s.sky_vis_radius, 0.0]);
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sh9_reproduces_the_sky_irradiance() {
        let sky = Sky::for_sun(Vec3::new(0.4, 0.6, -0.3));
        let sh = sky_sh9(&sky);
        // Brute-force cosine-weighted average of the sky for a few normals.
        for n in [Vec3::Y, Vec3::NEG_Y, Vec3::X, Vec3::new(0.3, 0.5, -0.8).normalize()] {
            let mut acc = Vec3::ZERO;
            let k = 256;
            for i in 0..k {
                for j in 0..k {
                    let ct = 1.0 - 2.0 * (i as f32 + 0.5) / k as f32;
                    let st = (1.0 - ct * ct).sqrt();
                    let ph = (j as f32 + 0.5) / k as f32 * std::f32::consts::TAU;
                    let d = Vec3::new(st * ph.cos(), st * ph.sin(), ct);
                    acc += sky_rad(&sky, d, 0.0) * d.dot(n).max(0.0);
                }
            }
            let e = acc * (4.0 * std::f32::consts::PI / (k * k) as f32) / std::f32::consts::PI;
            let s = sh_eval(&sh, n);
            assert!((e - s).abs().max_element() < 0.1 * e.max_element() + 0.01, "n {n}: brute {e} sh {s}");
        }
    }

    #[test]
    fn ambient_is_paler_and_brighter_than_the_sky_and_bounces_warm() {
        let sky = Sky::for_sun(Vec3::new(0.45, 0.62, 0.35).normalize());
        let s = LightingSettings::default();
        let (raw, amb) = (sh_eval(&sky_sh9(&sky), Vec3::Y), sh_eval(&ambient_sh9(&sky, &s), Vec3::Y));
        let lum = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        assert!(lum(amb) > lum(raw) && amb.z / amb.x < 0.6 * raw.z / raw.x, "raw {raw} ambient {amb}");
        // Open ground: sky to sun 0.15-0.25 (clear day, 100 klx direct, 15-25 klx diffuse).
        let sun = lum(sky.sun_color) * sky.sun_dir.y / std::f32::consts::PI;
        assert!((4.0..6.7).contains(&(sun / lum(amb))), "sun {sun} sky {}", lum(amb));
        // Facing down: the bounce, warmer than the sky and dimmer than the sunlit ground.
        let down = sh_eval(&ambient_sh9(&sky, &s), Vec3::NEG_Y);
        assert!(down.z / down.x < 0.75 * amb.z / amb.x && lum(down) < sun * 0.5, "down {down} up {amb}");
        // Unit settings and no bounce give the sky above the horizon back.
        let plain = LightingSettings { sky_ambient_gain: 1.0, sky_ambient_saturation: 1.0, bounce_albedo: Vec3::ZERO, ..s };
        let up = sh_eval(&ambient_sh9(&sky, &plain), Vec3::Y);
        assert!((up - raw).abs().max_element() < 0.02, "plain {up} raw {raw}");
    }

    #[test]
    fn cascades_cover_their_slices_and_snap_to_texels() {
        let s = LightingSettings::default();
        let cam = Camera::look_at(DVec3::new(1000.25, 1.7, -300.6), DVec3::new(1010.0, 3.0, -800.0));
        let sun = Vec3::new(0.3, 0.7, 0.4).normalize();
        let p = plan_cascades(&s, &cam, 16.0 / 9.0, sun);
        assert_eq!(p.count, 4);
        let (lx, ly, _) = light_basis(sun);
        for c in 0..4 {
            let texel = p.cascade[c][0] as f64;
            for v in [p.centres[c].dot(lx) / texel, p.centres[c].dot(ly) / texel] {
                assert!((v - v.round()).abs() < 1e-3, "cascade {c} centre off the texel grid: {v}");
            }
            // The far corners of the slice are inside the tile, out of the blend band.
            let far = p.splits[c];
            let ty = (0.5 * cam.fov_y).tan();
            let tx = ty * 16.0 / 9.0;
            let rot = cam.rotation();
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner = rot * Vec3::new(sx * tx * far, sy * ty * far, -far);
                let q = p.matrices[c] * corner.extend(1.0);
                assert!(q.x.abs().max(q.y.abs()) <= 1.0 - s.cascade_blend + 1e-3, "cascade {c} corner {q}");
                assert!((0.0..=1.0).contains(&q.z));
            }
        }
        assert!(p.splits.windows(2).all(|w| w[0] < w[1]) && (p.splits[3] - s.shadow_distance).abs() < 1e-3);
    }

    #[test]
    fn filter_radius_is_constant_in_world_units_across_cascades() {
        let cam = Camera::look_at(DVec3::new(1000.25, 1.7, -300.6), DVec3::new(1010.0, 3.0, -800.0));
        let sun = Vec3::new(0.3, 0.7, 0.4).normalize();
        // A distance whose texels all fit the clamp range: the world radius is equal (+-10%).
        let s = LightingSettings { shadow_distance: 120.0, split_lambda: 0.5, shadow_filter_m: 0.2, ..LightingSettings::default() };
        let p = plan_cascades(&s, &cam, 16.0 / 9.0, sun);
        for c in 0..p.count as usize {
            let t = p.cascade[c][0];
            let r = filter_texels(&s, t) * t;
            assert!((r / s.shadow_filter_m - 1.0).abs() <= 0.10, "cascade {c}: radius {r} m vs {}", s.shadow_filter_m);
        }
        // Defaults: radius is the world value wherever the clamps allow; where texels are too
        // coarse (far) the filter is the 1-texel minimum, below one pixel at the cascade's range.
        let s = LightingSettings::default();
        let p = plan_cascades(&s, &cam, 16.0 / 9.0, sun);
        for c in 0..p.count as usize {
            let t = p.cascade[c][0];
            let k = filter_texels(&s, t);
            let r = k * t;
            if k > PCF_KMIN && k < s.shadow_filter_max_texels {
                assert!((r / s.shadow_filter_m - 1.0).abs() <= 0.10, "cascade {c}: {r}");
            } else if k <= PCF_KMIN {
                let px = 2.0 * p.splits[c] * (0.5 * cam.fov_y).tan() / 720.0;
                assert!(r <= 2.5 * px, "cascade {c}: min filter {r} m vs pixel {px} m");
            }
        }
    }
}
