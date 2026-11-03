//! Procedural robot body on top of the physics capsule.
//!
//! - Feet: each foot stays planted in the world until it drifts too far
//!   from where it *should* be (hip + velocity lead), then steps there along
//!   an arc, alternating left/right. Targets come from ground raycasts, so
//!   feet land on curbs, slopes and stairs.
//! - Legs and arms: analytic two-bone IK (hip-knee-ankle, shoulder-elbow-hand).
//! - Arms counter-swing with the legs; in the air they spread (skydive) or
//!   reach up (parachute).
//! - Head bobs with the gait; the first-person camera inherits it.
//!
//! Character design (for lookdev review): cream shell, dark visor face with
//! two glowing eyes, orange courier jacket, teal parcel backpack.

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

const THIGH: f32 = 0.46;
const SHIN: f32 = 0.46;
const UPPER_ARM: f32 = 0.30;
const FOREARM: f32 = 0.30;
const HIP_W: f32 = 0.13;
const SHOULDER_W: f32 = 0.24;
const STEP_TIME: f32 = 0.22;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Part {
    Pelvis,
    Torso,
    Backpack,
    Head,
    Visor,
    EyeL,
    EyeR,
    ThighL,
    ThighR,
    ShinL,
    ShinR,
    FootL,
    FootR,
    UpperArmL,
    UpperArmR,
    ForearmL,
    ForearmR,
    HandL,
    HandR,
    Canopy,
}

#[derive(Component)]
struct BodyPart(Part);

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
    let shell = mats.add(StandardMaterial { base_color: Color::srgb(0.93, 0.91, 0.86), perceptual_roughness: 0.45, ..default() });
    let joint = mats.add(StandardMaterial { base_color: Color::srgb(0.25, 0.26, 0.29), perceptual_roughness: 0.6, metallic: 0.3, ..default() });
    let jacket = mats.add(StandardMaterial { base_color: Color::srgb(0.98, 0.45, 0.1), perceptual_roughness: 0.85, ..default() });
    let pack = mats.add(StandardMaterial { base_color: Color::srgb(0.1, 0.55, 0.55), perceptual_roughness: 0.7, ..default() });
    let visor = mats.add(StandardMaterial { base_color: Color::srgb(0.03, 0.04, 0.06), perceptual_roughness: 0.08, reflectance: 0.8, ..default() });
    let eye = mats.add(StandardMaterial { base_color: Color::BLACK, emissive: LinearRgba::rgb(4.0, 14.0, 16.0) * 400.0, ..default() });
    let shoe = mats.add(StandardMaterial { base_color: Color::srgb(0.2, 0.21, 0.24), perceptual_roughness: 0.7, ..default() });
    let canopy = mats.add(StandardMaterial { base_color: Color::srgb(0.98, 0.45, 0.1), perceptual_roughness: 0.9, double_sided: true, cull_mode: None, ..default() });

    let capsule = meshes.add(Capsule3d::new(0.5, 1.0).mesh().rings(4).latitudes(8).longitudes(12));
    let sphere = meshes.add(Sphere::new(0.5).mesh().uv(16, 10));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let dome = meshes.add(Sphere::new(1.0).mesh().uv(24, 8));

    let mut spawn = |part: Part, mesh: &Handle<Mesh>, mat: &Handle<StandardMaterial>| {
        commands.spawn((BodyPart(part), Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()), Transform::default(), Visibility::Visible));
    };
    spawn(Part::Pelvis, &capsule, &joint);
    spawn(Part::Torso, &capsule, &jacket);
    spawn(Part::Backpack, &cube, &pack);
    spawn(Part::Head, &sphere, &shell);
    spawn(Part::Visor, &sphere, &visor);
    spawn(Part::EyeL, &sphere, &eye);
    spawn(Part::EyeR, &sphere, &eye);
    for p in [Part::ThighL, Part::ThighR, Part::UpperArmL, Part::UpperArmR] {
        spawn(p, &capsule, if matches!(p, Part::UpperArmL | Part::UpperArmR) { &jacket } else { &shell });
    }
    for p in [Part::ShinL, Part::ShinR, Part::ForearmL, Part::ForearmR] {
        spawn(p, &capsule, &shell);
    }
    spawn(Part::FootL, &cube, &shoe);
    spawn(Part::FootR, &cube, &shoe);
    spawn(Part::HandL, &sphere, &shell);
    spawn(Part::HandR, &sphere, &shell);
    spawn(Part::Canopy, &dome, &canopy);
    commands.insert_resource(Gait::default());
}

/// Two-bone IK: returns the middle joint given root, target, lengths and a
/// pole direction (which way the knee/elbow should point).
fn two_bone(root: Vec3, target: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - root;
    let d = to.length().clamp(0.01, l1 + l2 - 0.001);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let end = root + dir * d;
    // Law of cosines for the angle at the root.
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a = cos_a.acos();
    let bend = pole.reject_from_normalized(dir).normalize_or(Vec3::Z);
    let mid = root + (dir * a.cos() + bend * a.sin()) * l1;
    (mid, end)
}

fn limb(a: Vec3, b: Vec3, radius: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(0.001);
    // Capsule3d(0.5, 1.0) is 2.0 tall overall; scale so it spans a..b.
    Transform {
        translation: (a + b) * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::Y, d / len),
        scale: Vec3::new(radius * 2.0, len * 0.5, radius * 2.0),
    }
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

    // Lean into acceleration/velocity.
    let lean_target = if airborne { Vec3::ZERO } else { v_flat * 0.02 };
    gait.lean = gait.lean.lerp(lean_target, 1.0 - (-6.0 * dt).exp());
    let crouch = if p.crouch { 0.3 } else { 0.0 };
    let pelvis = pt.translation - up * (RIDE_HEIGHT - THIGH - SHIN + 0.08 + crouch) + up * 0.0;
    let hips = [pelvis - right * HIP_W, pelvis + right * HIP_W];

    // --- Feet ---
    let filter = SpatialQueryFilter::from_excluded_entities([pe]);
    let ground_at = |p: Vec3| -> Option<Vec3> {
        spatial.cast_ray(p + up * 1.0, Dir3::new(-up).ok()?, 3.0, true, &filter).map(|h| p + up * 1.0 - up * h.distance)
    };
    if !gait.init {
        for (i, h) in hips.iter().enumerate() {
            gait.feet[i].planted = *h - up * (THIGH + SHIN);
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
            // Dangle: feet hang under the hips, slightly apart.
            let target = hips[i] - up * (THIGH + SHIN) * 0.92 + fwd * 0.05 + right * (i as f32 * 2.0 - 1.0) * 0.08;
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
            let ideal = ground_at(ideal_xz).unwrap_or(ideal_xz - up * (THIGH + SHIN));
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
    let torso_base = pelvis + up * (0.1 + gait.bob) + gait.lean * 0.6;
    let chest = torso_base + up * 0.48 + gait.lean;
    let neck = chest + up * 0.16;
    let head = neck + up * 0.2;

    // --- Arms ---
    let shoulders = [chest - right * SHOULDER_W + up * 0.05, chest + right * SHOULDER_W + up * 0.05];
    let swing = (gait.phase).sin() * 0.35 * (speed / 6.0).min(1.0);
    let hands = match p.mode {
        MoveMode::Skydive => [shoulders[0] - right * 0.55 + up * 0.15, shoulders[1] + right * 0.55 + up * 0.15],
        MoveMode::Parachute => [shoulders[0] + up * 0.5 - right * 0.1, shoulders[1] + up * 0.5 + right * 0.1],
        MoveMode::RocketShoes => [shoulders[0] - up * 0.4 - right * 0.25 - fwd * 0.15, shoulders[1] - up * 0.4 + right * 0.25 - fwd * 0.15],
        _ => [
            shoulders[0] - up * 0.52 + fwd * swing - right * 0.06,
            shoulders[1] - up * 0.52 - fwd * swing + right * 0.06,
        ],
    };

    for (bp, mut t, mut vis) in &mut parts {
        *vis = Visibility::Visible;
        let side = |l: bool| if l { 0 } else { 1 };
        match bp.0 {
            Part::Pelvis => *t = Transform::from_translation(pelvis + up * 0.05).with_rotation(base).with_scale(Vec3::new(0.34, 0.12, 0.24)),
            Part::Torso => *t = limb(torso_base, chest + up * 0.08, 0.2).with_scale(Vec3::new(0.46, 0.36, 0.32)),
            Part::Backpack => *t = Transform::from_translation((torso_base + chest) * 0.5 + up * 0.05 - fwd * 0.24).with_rotation(base).with_scale(Vec3::new(0.38, 0.42, 0.22)),
            Part::Head => {
                *t = Transform::from_translation(head).with_rotation(base).with_scale(Vec3::new(0.36, 0.34, 0.34));
                if p.first_person {
                    *vis = Visibility::Hidden;
                }
            }
            Part::Visor => {
                *t = Transform::from_translation(head + fwd * 0.07 + up * 0.01).with_rotation(base).with_scale(Vec3::new(0.27, 0.2, 0.22));
                if p.first_person {
                    *vis = Visibility::Hidden;
                }
            }
            Part::EyeL | Part::EyeR => {
                let s = if bp.0 == Part::EyeL { -1.0 } else { 1.0 };
                *t = Transform::from_translation(head + fwd * 0.175 + up * 0.02 + right * s * 0.055).with_scale(Vec3::splat(0.045));
                if p.first_person {
                    *vis = Visibility::Hidden;
                }
            }
            Part::ThighL | Part::ThighR | Part::ShinL | Part::ShinR => {
                let i = side(matches!(bp.0, Part::ThighL | Part::ShinL));
                let (knee, ankle) = two_bone(hips[i], foot_pos[i] + up * 0.06, THIGH, SHIN, fwd + up * 0.1);
                *t = if matches!(bp.0, Part::ThighL | Part::ThighR) { limb(hips[i], knee, 0.075) } else { limb(knee, ankle, 0.065) };
            }
            Part::FootL | Part::FootR => {
                let i = side(bp.0 == Part::FootL);
                *t = Transform::from_translation(foot_pos[i] + up * 0.05 + fwd * 0.05).with_rotation(base).with_scale(Vec3::new(0.13, 0.09, 0.26));
            }
            Part::UpperArmL | Part::UpperArmR | Part::ForearmL | Part::ForearmR => {
                let i = side(matches!(bp.0, Part::UpperArmL | Part::ForearmL));
                let (elbow, hand) = two_bone(shoulders[i], hands[i], UPPER_ARM, FOREARM, -fwd - up * 0.2);
                *t = if matches!(bp.0, Part::UpperArmL | Part::UpperArmR) { limb(shoulders[i], elbow, 0.07) } else { limb(elbow, hand, 0.055) };
            }
            Part::HandL | Part::HandR => {
                let i = side(bp.0 == Part::HandL);
                let (_, hand) = two_bone(shoulders[i], hands[i], UPPER_ARM, FOREARM, -fwd);
                *t = Transform::from_translation(hand).with_scale(Vec3::splat(0.11));
            }
            Part::Canopy => {
                if p.mode == MoveMode::Parachute {
                    *t = Transform::from_translation(pt.translation + up * 6.5).with_rotation(base).with_scale(Vec3::new(4.5, 1.6, 2.6));
                } else {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}
