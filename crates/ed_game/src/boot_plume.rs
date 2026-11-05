//! Rocket-boot exhaust plume: a port of IceRunner's booster VFX
//! (`ICERUNNER/client/src/vehicle/ExhaustVfx.ts`) to the two boot nozzles.
//!
//! Soft noisy sprites spawn at each nozzle, fly out along the jet, drag,
//! spin, grow and cool from pale gold through orange and ember red to grey
//! smoke; Shift (afterburner) blends in a hotter, longer, faster plume. Plus a
//! warm point light that pulses with the burn. Particles that reach the ground
//! splash out sideways along it (the visual half of the boot ground blast).
//!
//! The jet is anchored to the nozzle: particles leave it at the player's own
//! velocity plus the exhaust speed and only then drag back to the air's
//! frame, so at speed the trail stretches out behind the boot instead of the
//! player outrunning a cloud of sprites. Each nozzle also gets a bright core
//! streak (an axial billboard from the nozzle along the jet) whose length grows
//! with throttle and airspeed.
//!
//! Everything is one quad mesh rebuilt each frame (≤388 quads): sprites sorted
//! back to front, then the cores on top; alpha blended, unlit, HDR for bloom.

use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

use crate::origin::WorldOrigin;
use crate::player::{Player, RIDE_HEIGHT};

/// Particles per boot (IceRunner's `PARTICLE_CAPACITY`).
const CAP: usize = 192;
/// IceRunner's plume is sized for a hover-bike; a boot is smaller.
const SIZE_K: f32 = 0.5;
const SPEED_K: f32 = 0.6;

pub struct BootPlumePlugin;

impl Plugin for BootPlumePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BootNozzles>()
            .add_systems(Startup, spawn_plume)
            .add_systems(PostUpdate, update_plume.after(TransformSystems::Propagate));
    }
}

/// Written by `body::animate_body` each frame: where the nozzles are and which way they fire.
#[derive(Resource, Default)]
pub struct BootNozzles {
    pub pos: [Vec3; 2],
    pub dir: [Vec3; 2],
    pub fwd: [Vec3; 2],
    pub throttle: f32,
}

#[derive(Clone, Copy, Default)]
struct Particle {
    pos: Vec3,
    vel: Vec3,
    age: f32,
    life: f32,
    size0: f32,
    size1: f32,
    boost: f32,
    rot: f32,
    spin: f32,
    side: Vec3,
}

#[derive(Component)]
struct PlumeMesh;

#[derive(Component)]
struct PlumeLight;

#[derive(Default)]
struct State {
    parts: Vec<Particle>,
    acc: [f32; 2],
    boost: f32,
    generation: u64,
    rng: u32,
    t: f32,
}

impl State {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1u32 << 24) as f32
    }
}

/// IceRunner's alpha map: a soft radial blob broken up by a little noise.
fn particle_texture() -> Image {
    const N: u32 = 64;
    let mut rng = 0x1ce5_eedu32;
    let mut data = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let u = ((x as f32 + 0.5) / N as f32) * 2.0 - 1.0;
            let v = ((y as f32 + 0.5) / N as f32) * 2.0 - 1.0;
            let r = (u * u + v * v).sqrt().min(1.0);
            let radial = (1.0 - r).powf(1.8);
            rng ^= rng << 13;
            rng ^= rng >> 17;
            rng ^= rng << 5;
            let rnd = (rng >> 8) as f32 / (1u32 << 24) as f32;
            let noise = 0.5 + ((u + 0.3) * 8.5).sin() * 0.14 + ((v - 0.1) * 11.3).cos() * 0.12 + (rnd - 0.5) * 0.32;
            let a = (radial * (0.5 + noise * 0.55)).clamp(0.0, 1.0);
            data.extend_from_slice(&[255, 255, 255, (a * 255.0).round() as u8]);
        }
    }
    let mut img = Image::new(
        Extent3d { width: N, height: N, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    img
}

fn empty_mesh() -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, Vec::<[f32; 3]>::new())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, Vec::<[f32; 3]>::new())
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, Vec::<[f32; 2]>::new())
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, Vec::<[f32; 4]>::new())
        .with_inserted_indices(Indices::U32(Vec::new()))
}

fn spawn_plume(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let tex = images.add(particle_texture());
    let mat = mats.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(tex),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        fog_enabled: true,
        ..default()
    });
    commands.spawn((
        Mesh3d(meshes.add(empty_mesh())),
        MeshMaterial3d(mat),
        Transform::IDENTITY,
        NoFrustumCulling,
        bevy::light::NotShadowCaster,
        bevy::light::NotShadowReceiver,
        PlumeMesh,
        Name::new("boot plume"),
    ));
    commands.spawn((
        PointLight { color: Color::srgb_u8(0xff, 0x8a, 0x1f), intensity: 0.0, range: 14.0, shadow_maps_enabled: false, ..default() },
        Transform::IDENTITY,
        PlumeLight,
        Name::new("boot plume light"),
    ));
}

fn srgb(hex: u32) -> Vec3 {
    let c = Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8).to_linear();
    Vec3::new(c.red, c.green, c.blue)
}

#[allow(clippy::too_many_arguments)]
fn update_plume(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    noz: Res<BootNozzles>,
    players: Query<(&Transform, &avian3d::prelude::LinearVelocity, &Player)>,
    cams: Query<&GlobalTransform, (With<Camera3d>, Without<crate::globe::GlobeCamera>)>,
    mesh_q: Query<&Mesh3d, With<PlumeMesh>>,
    mut light_q: Query<(&mut PointLight, &mut Transform), (With<PlumeLight>, Without<Player>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut s: Local<State>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    if s.rng == 0 {
        s.rng = 0x2545_F491;
    }
    s.t += dt;
    if s.generation != origin.generation {
        s.generation = origin.generation;
        s.parts.clear();
    }
    let Ok(cam) = cams.single() else { return };
    let (world_vel, ground) = match players.single() {
        Ok((t, v, p)) => {
            let up = origin.up_at(t.translation);
            (v.0, (p.agl < 30.0).then(|| (t.translation - up * (p.agl + RIDE_HEIGHT), up)))
        }
        Err(_) => (Vec3::ZERO, None),
    };
    let up = origin.up_at(noz.pos[0]);

    // Space = 0.65 (gentle), Shift ramps 1.0 -> 3.5: map onto IceRunner's 0..1 intensity.
    let thr = noz.throttle;
    let firing = thr > 0.01;
    let i = ((thr - 0.2) / 1.6).clamp(0.0, 1.0);
    let boosting = thr > 0.9;
    let rate = if boosting { 9.0 } else { 6.0 };
    s.boost += ((boosting as u8 as f32) - s.boost) * (1.0 - (-rate * dt).exp());
    let b = s.boost;

    // ---- spawn (per boot)
    if firing && i > 0.0 {
        for k in 0..2 {
            let dir = noz.dir[k].normalize_or(-up);
            let fwd = noz.fwd[k].reject_from(dir).normalize_or(Vec3::X);
            let side = dir.cross(fwd).normalize_or(Vec3::Z);
            s.acc[k] += (72.0 + i * 104.0 + b * 54.0) * dt;
            while s.acc[k] >= 1.0 {
                s.acc[k] -= 1.0;
                if s.parts.len() >= CAP * 2 {
                    break;
                }
                // A tight jet: little sideways spread, small sprites at the nozzle
                // that only bloom into smoke once they're well behind it.
                let lateral = (s.rand() * 2.0 - 1.0) * (0.02 + i * 0.015 + b * 0.01);
                let vertical = (s.rand() * 2.0 - 1.0) * (0.015 + i * 0.01 + b * 0.008);
                let life = 0.6 + s.rand() * 0.35 + i * 0.3 + b * 0.2;
                let size0 = (0.2 + i * 0.1 + b * 0.05) * SIZE_K;
                let size1 = size0 * (1.8 + s.rand() * 1.2 + b * 0.8);
                let spin = (s.rand() - 0.5) * (1.8 + i * 0.8);
                let rot = s.rand() * std::f32::consts::TAU;
                // Spread the spawn point a little along the first frame's travel, so a fast burn has no gaps.
                let jitter = s.rand() * dt;
                let vel = dir * (14.0 + i * 16.0 + b * 12.0) * SPEED_K
                    + side * lateral * (10.0 + b * 4.0)
                    + fwd * vertical * (6.0 + b * 2.0)
                    // Exhaust leaves at the boot's own velocity; drag then hands it to the air.
                    + world_vel;
                s.parts.push(Particle {
                    pos: noz.pos[k] + vel * jitter,
                    vel,
                    age: 0.0,
                    life,
                    size0,
                    size1,
                    boost: b,
                    rot,
                    spin,
                    side,
                });
            }
        }
    } else {
        s.acc = [0.0; 2];
    }

    // ---- advance (IceRunner's per-frame drags, made frame-rate independent)
    let drag = 0.972f32.powf(dt * 60.0);
    let cool_drag = 0.992f32.powf(dt * 60.0);
    let t = s.t;
    s.parts.retain_mut(|p| {
        p.age += dt;
        if p.age >= p.life {
            return false;
        }
        let lt: f32 = p.age / p.life;
        p.pos += p.vel * dt;
        p.vel *= drag;
        p.vel += up * 0.08 * dt;
        p.vel += p.side * (t * 14.0 + p.rot).sin() * (0.13 + p.boost * 0.06) * dt;
        p.rot += p.spin * dt;
        p.boost = (p.boost - dt * 1.6).max(0.0);
        if lt > 0.45 {
            p.vel *= cool_drag;
        }
        // Hitting the ground: flatten onto it and splash outwards, a little rebound.
        if let Some((g, n)) = ground {
            let h = (p.pos - g).dot(n);
            if h < 0.08 {
                p.pos += n * (0.08 - h);
                let vn = p.vel.dot(n);
                if vn < 0.0 {
                    let tang = p.vel - n * vn;
                    let out = (p.pos - g).reject_from(n).normalize_or_zero();
                    p.vel = tang + out * (-vn) * 0.7 - n * vn * 0.12;
                    p.size1 *= 1.004;
                }
            }
        }
        true
    });

    // ---- light
    if let Ok((mut l, mut lt)) = light_q.single_mut() {
        let k = if firing { (i + b * 0.6).min(1.6) } else { 0.0 };
        let flick = 0.9 + 0.1 * (t * 47.0).sin() * (t * 31.0).cos();
        l.intensity = 2.5 * k * flick * 80_000.0;
        let c = srgb(0xff8a1f).lerp(srgb(0xff6611), i);
        l.color = Color::linear_rgb(c.x, c.y, c.z);
        lt.translation = (noz.pos[0] + noz.pos[1]) * 0.5 + (noz.dir[0] + noz.dir[1]) * 0.25;
    }

    // ---- render: back to front, camera-facing quads
    let Ok(m3) = mesh_q.single() else { return };
    let Some(mut mesh) = meshes.get_mut(&m3.0) else { return };
    let cp = cam.translation();
    let (cr, cu, cf) = (cam.right().as_vec3(), cam.up().as_vec3(), cam.forward().as_vec3());
    s.parts.sort_unstable_by(|a, b| b.pos.distance_squared(cp).total_cmp(&a.pos.distance_squared(cp)));
    let (c_start, c_hot, c_boost, c_ember, c_tail) =
        (srgb(0xffcc77), srgb(0xff6611), srgb(0xff9966), srgb(0xdd1100), srgb(0x333333));
    let opacity = 0.8 + 0.09 * (i + b * 0.25).clamp(0.0, 1.0);
    let n = s.parts.len();
    let mut pos = Vec::with_capacity(n * 4);
    let mut nor = Vec::with_capacity(n * 4);
    let mut uv = Vec::with_capacity(n * 4);
    let mut col = Vec::with_capacity(n * 4);
    let mut idx = Vec::with_capacity(n * 6);
    for p in &s.parts {
        let lt: f32 = p.age / p.life;
        let hot_t = (1.0 - lt * 1.35).clamp(0.0, 1.0);
        let smoke_t = ((lt - 0.18) / 0.82).clamp(0.0, 1.0);
        let size = p.size0 + (p.size1 - p.size0) * lt;
        let w = size * (0.9 + p.boost * 0.4) * 0.5;
        let h = size * (1.1 + p.boost * 0.55) * 0.5;
        let hot = c_hot.lerp(c_boost, p.boost * 0.35);
        let c = c_start.lerp(hot, 1.0 - hot_t).lerp(c_ember, lt.powf(0.6)).lerp(c_tail, smoke_t.powf(1.2));
        // Unlit + HDR: the young, hot core blooms; the smoke stays a plain grey.
        let glow = 1.0 + 5.0 * hot_t * hot_t * (1.0 - smoke_t);
        // Fade in over the first few frames and out at the very end so nothing pops.
        let alpha = opacity * (lt / 0.04).min(1.0) * ((1.0 - lt) / 0.15).min(1.0) * (1.0 - 0.35 * smoke_t);
        let (sn, cs) = p.rot.sin_cos();
        let ax = (cr * cs + cu * sn) * w;
        let ay = (cu * cs - cr * sn) * h;
        let base = pos.len() as u32;
        for (dx, dy, u, v) in [(-1.0, -1.0, 0.0, 1.0), (1.0, -1.0, 1.0, 1.0), (1.0, 1.0, 1.0, 0.0), (-1.0, 1.0, 0.0, 0.0)] {
            pos.push((p.pos + ax * dx + ay * dy).to_array());
            nor.push((-cf).to_array());
            uv.push([u, v]);
            col.push([c.x * glow, c.y * glow, c.z * glow, alpha]);
        }
        idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    // ---- core jets: an axial billboard per nozzle, starting at the nozzle.
    // It bends back into the airflow and stretches with airspeed, like the
    // particle trail it sits on.
    if firing && i > 0.0 {
        let speed = world_vel.length();
        let stretch = 1.0 + (speed / 35.0).min(2.5);
        for k in 0..2 {
            let dir = noz.dir[k].normalize_or(-up);
            let axis = (dir * (14.0 + 16.0 * i + 10.0 * b) - world_vel * 0.5).normalize_or(dir);
            let ph = t * 53.0 + k as f32 * 2.3;
            let flick = 0.9 + 0.1 * ph.sin() * (ph * 1.37).cos();
            let base_len = (0.28 + 0.4 * i + 0.35 * b) * stretch * flick;
            // (length, width, colour, HDR gain, alpha): orange sheath, then white-hot core.
            let layers = [
                (base_len * 1.5, 0.07 + 0.04 * i + 0.03 * b, c_hot.lerp(c_boost, b * 0.5), 3.0, 0.55),
                (base_len, 0.035 + 0.02 * i + 0.015 * b, srgb(0xfff2d0), 9.0, 0.9),
            ];
            for (len, wid, c, glow, alpha) in layers {
                // Starts just inside the nozzle so the blob's soft end hides in the boot.
                let a = noz.pos[k] - axis * len * 0.12;
                let z = noz.pos[k] + axis * len;
                let mid = (a + z) * 0.5;
                let side = axis.cross(cp - mid).normalize_or(cr) * wid;
                let base = pos.len() as u32;
                for (p, u, v) in [(a - side, 0.0, 0.0), (a + side, 1.0, 0.0), (z + side, 1.0, 1.0), (z - side, 0.0, 1.0)] {
                    pos.push(p.to_array());
                    nor.push((-cf).to_array());
                    uv.push([u, v]);
                    col.push([c.x * glow, c.y * glow, c.z * glow, alpha * (0.6 + 0.4 * i)]);
                }
                idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
    }
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, nor);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, col);
    mesh.insert_indices(Indices::U32(idx));
}
