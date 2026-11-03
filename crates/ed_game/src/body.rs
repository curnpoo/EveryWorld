//! Procedural robot body on top of the physics capsule.
//!
//! - Feet: each foot stays planted in the world until it drifts too far
//!   from where it *should* be (hip + velocity lead), then steps there along
//!   an arc, alternating left/right. Targets come from ground raycasts, so
//!   feet land on curbs, slopes and stairs.
//! - Legs and arms: analytic two-bone IK (hip-knee-ankle, shoulder-elbow-wrist).
//! - Arms counter-swing with the legs; in the air they spread (skydive) or
//!   reach up (parachute).
//! - Head bobs with the gait; the first-person camera inherits it.
//!
//! Construction (lookdev round 2): every limb is a cylinder spanning two
//! joints, every joint is a ball, so the body is always fully connected no
//! matter what the IK does. Rigid pieces (torso, head, hands, feet,
//! backpack) hang off per-bone frames. Head: a CRT TV with a bulged screen
//! face, glowing eyes that blink, antenna and dials.

use crate::origin::WorldOrigin;
use crate::player::{MoveMode, Player, RIDE_HEIGHT};
use avian3d::prelude::*;
use bevy::prelude::*;

pub struct BodyPlugin;

impl Plugin for BodyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_body).add_systems(PostUpdate, animate_body.before(TransformSystems::Propagate));
    }
}

const THIGH: f32 = 0.44;
const SHIN: f32 = 0.44;
const UPPER_ARM: f32 = 0.29;
const FOREARM: f32 = 0.27;
const HIP_W: f32 = 0.12;
const SHOULDER_W: f32 = 0.27;
const SPINE: f32 = 0.42;
const STEP_TIME: f32 = 0.22;

/// Joint ids.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum J {
    Pelvis,
    Chest,
    Neck,
    HipL,
    HipR,
    KneeL,
    KneeR,
    AnkleL,
    AnkleR,
    ShoulderL,
    ShoulderR,
    ElbowL,
    ElbowR,
    WristL,
    WristR,
}

/// Rigid frames (position + orientation).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum F {
    Pelvis,
    Chest,
    Head,
    FootL,
    FootR,
    HandL,
    HandR,
}

#[derive(Clone, Copy, Debug)]
enum Spec {
    /// Cylinder spanning two joints.
    Limb { a: J, b: J, r: f32 },
    /// Ball at a joint.
    Ball { j: J, r: f32 },
    /// A piece fixed to a frame: local offset, rotation, scale.
    Rigid { f: F, off: Vec3, rot: Quat, scale: Vec3 },
    /// Eyes: rigid on the head, blink by squashing.
    Eye { off: Vec3 },
    Canopy,
}

#[derive(Component)]
struct BodyPart(Spec);

#[derive(Default, Clone, Copy)]
struct Foot {
    planted: Vec3,
    from: Vec3,
    to: Vec3,
    t: f32,
    stepping: bool,
}

#[derive(Resource, Default)]
struct Gait {
    feet: [Foot; 2],
    init: bool,
    phase: f32,
    bob: f32,
    lean: Vec3,
}

fn spawn_body(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let mat = |m: &mut Assets<StandardMaterial>, rgb: (f32, f32, f32), rough: f32, metal: f32| {
        m.add(StandardMaterial { base_color: Color::srgb(rgb.0, rgb.1, rgb.2), perceptual_roughness: rough, metallic: metal, ..default() })
    };
    let shell = mat(&mut mats, (0.92, 0.89, 0.82), 0.4, 0.0);
    let shell_dark = mat(&mut mats, (0.78, 0.74, 0.66), 0.45, 0.0);
    let joint = mat(&mut mats, (0.22, 0.23, 0.26), 0.45, 0.6);
    let jacket = mat(&mut mats, (0.96, 0.42, 0.09), 0.85, 0.0);
    let jacket_dark = mat(&mut mats, (0.72, 0.28, 0.05), 0.85, 0.0);
    let pack = mat(&mut mats, (0.08, 0.52, 0.52), 0.7, 0.0);
    let strap = mat(&mut mats, (0.16, 0.17, 0.19), 0.8, 0.0);
    let shoe = mat(&mut mats, (0.24, 0.25, 0.28), 0.7, 0.0);
    let sole = mat(&mut mats, (0.95, 0.95, 0.93), 0.6, 0.0);
    let screen = mats.add(StandardMaterial { base_color: Color::srgb(0.03, 0.06, 0.07), perceptual_roughness: 0.06, reflectance: 0.9, ..default() });
    let eye = mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(0.6, 3.0, 3.2) * 2_000.0, ..default() });
    let brass = mat(&mut mats, (0.85, 0.66, 0.32), 0.3, 0.9);
    let canopy = mats.add(StandardMaterial { base_color: Color::srgb(0.96, 0.42, 0.09), perceptual_roughness: 0.9, double_sided: true, cull_mode: None, ..default() });

    let cyl = meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(20));
    let ball = meshes.add(Sphere::new(0.5).mesh().uv(20, 12));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let dome = meshes.add(Sphere::new(1.0).mesh().uv(24, 8));

    let q0 = Quat::IDENTITY;
    let qx = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2); // cylinder Y -> Z
    let qz = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2); // cylinder Y -> X
    let v = Vec3::new;
    use Spec::*;
    let parts: Vec<(Spec, &Handle<Mesh>, &Handle<StandardMaterial>)> = vec![
        // --- Legs: cylinder bones, ball joints ---
        (Limb { a: J::HipL, b: J::KneeL, r: 0.075 }, &cyl, &shell),
        (Limb { a: J::HipR, b: J::KneeR, r: 0.075 }, &cyl, &shell),
        (Limb { a: J::KneeL, b: J::AnkleL, r: 0.065 }, &cyl, &shell),
        (Limb { a: J::KneeR, b: J::AnkleR, r: 0.065 }, &cyl, &shell),
        (Ball { j: J::HipL, r: 0.1 }, &ball, &joint),
        (Ball { j: J::HipR, r: 0.1 }, &ball, &joint),
        (Ball { j: J::KneeL, r: 0.088 }, &ball, &joint),
        (Ball { j: J::KneeR, r: 0.088 }, &ball, &joint),
        (Ball { j: J::AnkleL, r: 0.07 }, &ball, &joint),
        (Ball { j: J::AnkleR, r: 0.07 }, &ball, &joint),
        // --- Feet: sneaker = upper + white sole + toe cap ---
        (Rigid { f: F::FootL, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, &cube, &shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, &cube, &shoe),
        (Rigid { f: F::FootL, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, &cube, &sole),
        (Rigid { f: F::FootR, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, &cube, &sole),
        (Rigid { f: F::FootL, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, &cyl, &shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, &cyl, &shoe),
        // --- Pelvis + waist + spine column ---
        (Rigid { f: F::Pelvis, off: v(0.0, 0.0, 0.0), rot: q0, scale: v(0.34, 0.16, 0.22) }, &cube, &joint),
        (Limb { a: J::Pelvis, b: J::Chest, r: 0.08 }, &cyl, &joint),
        // --- Torso: jacket block, hem, collar, zipper, pockets ---
        (Rigid { f: F::Chest, off: v(0.0, -0.2, 0.0), rot: q0, scale: v(0.46, 0.4, 0.28) }, &cube, &jacket),
        (Rigid { f: F::Chest, off: v(0.0, -0.41, 0.0), rot: q0, scale: v(0.44, 0.06, 0.26) }, &cube, &jacket_dark),
        (Rigid { f: F::Chest, off: v(0.0, 0.02, 0.0), rot: q0, scale: v(0.3, 0.07, 0.22) }, &cube, &jacket_dark),
        (Rigid { f: F::Chest, off: v(0.0, -0.2, -0.142), rot: q0, scale: v(0.025, 0.36, 0.01) }, &cube, &brass),
        (Rigid { f: F::Chest, off: v(-0.12, -0.3, -0.142), rot: q0, scale: v(0.12, 0.09, 0.01) }, &cube, &jacket_dark),
        (Rigid { f: F::Chest, off: v(0.12, -0.3, -0.142), rot: q0, scale: v(0.12, 0.09, 0.01) }, &cube, &jacket_dark),
        // --- Neck ---
        (Limb { a: J::Chest, b: J::Neck, r: 0.055 }, &cyl, &joint),
        // --- Arms: sleeve on the upper arm, shell forearm, ball joints ---
        (Limb { a: J::ShoulderL, b: J::ElbowL, r: 0.07 }, &cyl, &jacket),
        (Limb { a: J::ShoulderR, b: J::ElbowR, r: 0.07 }, &cyl, &jacket),
        (Limb { a: J::ElbowL, b: J::WristL, r: 0.05 }, &cyl, &shell),
        (Limb { a: J::ElbowR, b: J::WristR, r: 0.05 }, &cyl, &shell),
        (Ball { j: J::ShoulderL, r: 0.11 }, &ball, &jacket),
        (Ball { j: J::ShoulderR, r: 0.11 }, &ball, &jacket),
        (Ball { j: J::ElbowL, r: 0.07 }, &ball, &joint),
        (Ball { j: J::ElbowR, r: 0.07 }, &ball, &joint),
        (Ball { j: J::WristL, r: 0.05 }, &ball, &joint),
        (Ball { j: J::WristR, r: 0.05 }, &ball, &joint),
        // --- Hands: mitt + thumb ---
        (Rigid { f: F::HandL, off: v(0.0, -0.06, 0.0), rot: q0, scale: v(0.07, 0.11, 0.09) }, &cube, &shell),
        (Rigid { f: F::HandR, off: v(0.0, -0.06, 0.0), rot: q0, scale: v(0.07, 0.11, 0.09) }, &cube, &shell),
        (Rigid { f: F::HandL, off: v(0.03, -0.04, -0.05), rot: q0, scale: v(0.03, 0.06, 0.03) }, &cube, &shell_dark),
        (Rigid { f: F::HandR, off: v(-0.03, -0.04, -0.05), rot: q0, scale: v(0.03, 0.06, 0.03) }, &cube, &shell_dark),
        // --- Parcel backpack with straps and tape band ---
        (Rigid { f: F::Chest, off: v(0.0, -0.18, 0.27), rot: q0, scale: v(0.4, 0.42, 0.26) }, &cube, &pack),
        (Rigid { f: F::Chest, off: v(0.0, -0.18, 0.27), rot: q0, scale: v(0.41, 0.06, 0.27) }, &cube, &strap),
        (Rigid { f: F::Chest, off: v(-0.13, -0.1, -0.005), rot: q0, scale: v(0.05, 0.42, 0.29) }, &cube, &strap),
        (Rigid { f: F::Chest, off: v(0.13, -0.1, -0.005), rot: q0, scale: v(0.05, 0.42, 0.29) }, &cube, &strap),
        // --- CRT head: case, bezel, bulged screen, tube housing, dials, antenna ---
        (Rigid { f: F::Head, off: v(0.0, 0.0, 0.0), rot: q0, scale: v(0.42, 0.34, 0.3) }, &cube, &shell),
        (Rigid { f: F::Head, off: v(0.0, 0.0, -0.152), rot: q0, scale: v(0.38, 0.3, 0.01) }, &cube, &shell_dark),
        (Rigid { f: F::Head, off: v(-0.03, 0.005, -0.155), rot: q0, scale: v(0.29, 0.24, 0.05) }, &ball, &screen),
        (Rigid { f: F::Head, off: v(0.0, -0.01, 0.2), rot: q0, scale: v(0.3, 0.24, 0.14) }, &cube, &shell_dark),
        (Rigid { f: F::Head, off: v(0.155, 0.06, -0.155), rot: qx, scale: v(0.045, 0.03, 0.045) }, &cyl, &joint),
        (Rigid { f: F::Head, off: v(0.155, -0.04, -0.155), rot: qx, scale: v(0.04, 0.03, 0.04) }, &cyl, &brass),
        (Rigid { f: F::Head, off: v(0.155, -0.11, -0.153), rot: q0, scale: v(0.05, 0.02, 0.01) }, &cube, &joint),
        (Rigid { f: F::Head, off: v(0.09, 0.27, 0.0), rot: Quat::from_rotation_z(-0.35), scale: v(0.012, 0.22, 0.012) }, &cyl, &joint),
        (Rigid { f: F::Head, off: v(0.13, 0.38, 0.0), rot: q0, scale: v(0.035, 0.035, 0.035) }, &ball, &jacket),
        (Rigid { f: F::Head, off: v(0.0, -0.19, 0.0), rot: q0, scale: v(0.2, 0.04, 0.16) }, &cube, &joint),
        (Eye { off: v(-0.085, 0.02, -0.18) }, &ball, &eye),
        (Eye { off: v(0.035, 0.02, -0.18) }, &ball, &eye),
        (Canopy, &dome, &canopy),
    ];
    for (spec, mesh, m) in parts {
        commands.spawn((BodyPart(spec), Mesh3d(mesh.clone()), MeshMaterial3d(m.clone()), Transform::default(), Visibility::Visible));
    }
    commands.insert_resource(Gait::default());
}

/// Two-bone IK: returns (middle joint, end) given root, target, lengths and
/// a pole direction (which way the knee/elbow should point).
fn two_bone(root: Vec3, target: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - root;
    let d = to.length().clamp(0.01, l1 + l2 - 0.001);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let end = root + dir * d;
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a = cos_a.acos();
    let bend = pole.reject_from_normalized(dir).normalize_or(Vec3::Z);
    let mid = root + (dir * a.cos() + bend * a.sin()) * l1;
    (mid, end)
}

/// Unit cylinder (r 0.5, h 1, along Y) spanning a..b.
fn limb(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(0.001);
    Transform { translation: (a + b) * 0.5, rotation: Quat::from_rotation_arc(Vec3::Y, d / len), scale: Vec3::new(r * 2.0, len, r * 2.0) }
}

#[allow(clippy::type_complexity)]
fn animate_body(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    spatial: SpatialQuery,
    mut gait: ResMut<Gait>,
    players: Query<(Entity, &Transform, &LinearVelocity, &Player), Without<BodyPart>>,
    mut parts: Query<(&BodyPart, &mut Transform, &mut Visibility)>,
) {
    let Ok((pe, pt, vel, p)) = players.single() else { return };
    let dt = time.delta_secs().min(0.05);
    let up = origin.up_at(pt.translation);
    let base = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(p.yaw);
    let fwd = base * Vec3::NEG_Z;
    let right = base * Vec3::X;
    let v = vel.0;
    let v_flat = v - up * v.dot(up);
    let speed = v_flat.length();
    let airborne = p.mode != MoveMode::Walk;

    let lean_target = if airborne { Vec3::ZERO } else { v_flat * 0.02 };
    gait.lean = gait.lean.lerp(lean_target, 1.0 - (-6.0 * dt).exp());
    let crouch = if p.crouch { 0.3 } else { 0.0 };
    let pelvis = pt.translation - up * (RIDE_HEIGHT - THIGH - SHIN + 0.04 + crouch);
    let hips = [pelvis - right * HIP_W - up * 0.04, pelvis + right * HIP_W - up * 0.04];

    // --- Feet ---
    let filter = SpatialQueryFilter::from_excluded_entities([pe]);
    let ground_at = |p: Vec3| -> Option<Vec3> {
        spatial.cast_ray(p + up * 1.0, Dir3::new(-up).ok()?, 3.0, true, &filter).map(|h| p + up * 1.0 - up * h.distance)
    };
    if !gait.init {
        for (i, h) in hips.iter().enumerate() {
            gait.feet[i].planted = *h - up * (THIGH + SHIN - 0.04);
        }
        gait.init = true;
    }
    let stride = (0.35 + speed * 0.12).min(1.4);
    let mut foot_pos = [Vec3::ZERO; 2];
    for i in 0..2 {
        let other_stepping = gait.feet[1 - i].stepping;
        let f = &mut gait.feet[i];
        let ideal_xz = hips[i] + v_flat * (STEP_TIME * 1.1);
        if airborne {
            let target = hips[i] - up * (THIGH + SHIN) * 0.9 + fwd * 0.05 + right * (i as f32 * 2.0 - 1.0) * 0.06;
            f.planted = f.planted.lerp(target, 1.0 - (-14.0 * dt).exp());
            f.stepping = false;
            foot_pos[i] = f.planted;
            continue;
        }
        if f.stepping {
            f.t += dt / STEP_TIME;
            if f.t >= 1.0 {
                f.stepping = false;
                f.planted = f.to;
            }
        } else {
            let ideal = ground_at(ideal_xz).unwrap_or(ideal_xz - up * (THIGH + SHIN - 0.04));
            if (ideal - f.planted).reject_from_normalized(up).length() > stride * 0.5 && !other_stepping {
                f.stepping = true;
                f.t = 0.0;
                f.from = f.planted;
                f.to = ground_at(ideal_xz + v_flat * STEP_TIME * 0.5).unwrap_or(ideal);
            }
        }
        foot_pos[i] = if f.stepping {
            let t = f.t.clamp(0.0, 1.0);
            let s = t * t * (3.0 - 2.0 * t);
            f.from.lerp(f.to, s) + up * (t * std::f32::consts::PI).sin() * (0.08 + 0.05 * speed.min(6.0))
        } else {
            f.planted
        };
    }
    gait.phase += speed * dt * 2.2;
    let bob_target = if airborne { 0.0 } else { (gait.phase * 2.0).sin().abs() * 0.04 * speed.min(6.0) / 6.0 };
    gait.bob += (bob_target - gait.bob) * (1.0 - (-12.0 * dt).exp());

    // --- Spine, head ---
    let pelvis_c = pelvis + up * gait.bob;
    let chest = pelvis_c + up * SPINE + gait.lean;
    let chest_rot = Quat::from_rotation_arc(up, (chest - pelvis_c).normalize()) * base;
    let neck = chest + up * 0.1;
    let head_c = neck + up * 0.21;
    let t = time.elapsed_secs();
    let head_rot = base * Quat::from_rotation_x(-0.06 + (t * 0.7).sin() * 0.03) * Quat::from_rotation_z((t * 0.45).sin() * 0.04);

    // --- Legs ---
    let ankles = [foot_pos[0] + up * 0.11, foot_pos[1] + up * 0.11];
    let (knee_l, ankle_l) = two_bone(hips[0], ankles[0], THIGH, SHIN, fwd + up * 0.1);
    let (knee_r, ankle_r) = two_bone(hips[1], ankles[1], THIGH, SHIN, fwd + up * 0.1);

    // --- Arms ---
    let shoulders = [chest - right * SHOULDER_W - up * 0.06, chest + right * SHOULDER_W - up * 0.06];
    let swing = gait.phase.sin() * 0.35 * (speed / 6.0).min(1.0);
    let hands = match p.mode {
        MoveMode::Skydive => [shoulders[0] - right * 0.5 + up * 0.15, shoulders[1] + right * 0.5 + up * 0.15],
        MoveMode::Parachute => [shoulders[0] + up * 0.48 - right * 0.1, shoulders[1] + up * 0.48 + right * 0.1],
        MoveMode::RocketShoes => [shoulders[0] - up * 0.4 - right * 0.22 - fwd * 0.15, shoulders[1] - up * 0.4 + right * 0.22 - fwd * 0.15],
        _ => [shoulders[0] - up * 0.5 + fwd * swing - right * 0.07, shoulders[1] - up * 0.5 - fwd * swing + right * 0.07],
    };
    let (elbow_l, wrist_l) = two_bone(shoulders[0], hands[0], UPPER_ARM, FOREARM, -fwd - up * 0.2 - right * 0.3);
    let (elbow_r, wrist_r) = two_bone(shoulders[1], hands[1], UPPER_ARM, FOREARM, -fwd - up * 0.2 + right * 0.3);

    let joint = |j: J| -> Vec3 {
        match j {
            J::Pelvis => pelvis_c,
            J::Chest => chest,
            J::Neck => neck,
            J::HipL => hips[0],
            J::HipR => hips[1],
            J::KneeL => knee_l,
            J::KneeR => knee_r,
            J::AnkleL => ankle_l,
            J::AnkleR => ankle_r,
            J::ShoulderL => shoulders[0],
            J::ShoulderR => shoulders[1],
            J::ElbowL => elbow_l,
            J::ElbowR => elbow_r,
            J::WristL => wrist_l,
            J::WristR => wrist_r,
        }
    };
    // Hand frame: the body frame tipped so its -Y runs down the forearm.
    let hand_rot = |e: Vec3, w: Vec3| Quat::from_rotation_arc(-up, (w - e).normalize_or(-up)) * base;
    let frame = |f: F| -> (Vec3, Quat) {
        match f {
            F::Pelvis => (pelvis_c, base),
            F::Chest => (chest, chest_rot),
            F::Head => (head_c, head_rot),
            F::FootL => (ankle_l, base),
            F::FootR => (ankle_r, base),
            F::HandL => (wrist_l, hand_rot(elbow_l, wrist_l)),
            F::HandR => (wrist_r, hand_rot(elbow_r, wrist_r)),
        }
    };
    // Blink every few seconds.
    let blink = if (t % 3.7) < 0.12 { 0.15 } else { 1.0 };

    for (bp, mut tf, mut vis) in &mut parts {
        *vis = Visibility::Visible;
        match bp.0 {
            Spec::Limb { a, b, r } => *tf = limb(joint(a), joint(b), r),
            Spec::Ball { j, r } => *tf = Transform::from_translation(joint(j)).with_scale(Vec3::splat(r * 2.0)),
            Spec::Rigid { f, off, rot, scale } => {
                let (p0, q) = frame(f);
                *tf = Transform { translation: p0 + q * off, rotation: q * rot, scale };
                if f == F::Head && p.first_person {
                    *vis = Visibility::Hidden;
                }
            }
            Spec::Eye { off } => {
                let (p0, q) = frame(F::Head);
                *tf = Transform { translation: p0 + q * off, rotation: q, scale: Vec3::new(0.055, 0.075 * blink, 0.02) };
                if p.first_person {
                    *vis = Visibility::Hidden;
                }
            }
            Spec::Canopy => {
                if p.mode == MoveMode::Parachute {
                    *tf = Transform::from_translation(pt.translation + up * 6.5).with_rotation(base).with_scale(Vec3::new(4.5, 1.6, 2.6));
                } else {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}
