//! Self-balancing physical robot body with capture-point stepping, spring-damper
//! joints, anatomical limits, and visible boot rocket jets.
//!
//! - **Capture-Point Stepping:** The feet don't animate on a fake walk timer.
//!   Instead, each foot computes its ideal ground balance point from the
//!   pelvis's actual horizontal position, lean, and velocity (`x_hip + v * T`).
//!   When you lean forward to walk or get nudged sideways, the Center of Mass
//!   drifts outside the planted foot's support radius, triggering the foot to
//!   step out to the capture point and catch the body!
//! - **Foot Force Feedback:** The world positions (`foot_world_pos`), sole
//!   directions (`foot_up_dir`), and planted states (`foot_planted`) of both
//!   feet are written directly back to `Player` every frame so `drive_player`
//!   applies ground reaction forces and boot rocket thrust at the actual shoes.
//! - **Boot Rocket Flames:** Each shoe has a physical thruster nozzle and a
//!   multi-layered emissive rocket plume (`Spec::JetFlame`) that ignites when
//!   `p.throttle > 0.0` and scales/brightens into an afterburner torch when
//!   `Shift` is held.

use crate::origin::WorldOrigin;
use crate::player::{MoveMode, Player};
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

pub struct BodyPlugin;

impl Plugin for BodyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cosmetics>()
            .add_systems(Startup, spawn_body)
            .add_systems(PostUpdate, animate_body.before(TransformSystems::Propagate));
    }
}

const THIGH: f32 = 0.44;
const SHIN: f32 = 0.44;
const UPPER_ARM: f32 = 0.29;
const FOREARM: f32 = 0.27;
const HIP_W: f32 = 0.13;
const SHOULDER_W: f32 = 0.27;
const SPINE: f32 = 0.42;
const BASE_STEP_TIME: f32 = 0.20;
/// Ankle joint height above the ground.
const ANKLE_H: f32 = 0.11;

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
    /// Rocket flame plume shooting out of the bottom of a shoe (`foot_idx`: 0 = L, 1 = R, `inner`: core vs outer plume).
    JetFlame { foot_idx: usize, inner: bool },
    Canopy,
}

#[derive(Component)]
struct BodyPart(Spec);

/// Deformable wingsuit membrane: 0 = left arm wing, 1 = right arm wing, 2 = leg wing (between the legs).
#[derive(Component)]
struct WingMesh(usize);

/// 7 outline points + 1 centroid, fan-triangulated; vertices are rewritten every frame in world space.
fn wing_mesh_blank() -> Mesh {
    let mut idx = Vec::new();
    for k in 0..7u32 {
        idx.extend_from_slice(&[7, k, (k + 1) % 7]);
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32; 3]; 8])
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 8])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0f32; 2]; 8])
        .with_inserted_indices(Indices::U32(idx))
}

/// Player customization. Accessories are optional pieces of the rig.
#[derive(Resource, Default, Clone)]
pub struct Cosmetics {
    pub backpack: bool,
}

/// Marks an optional accessory piece and which cosmetic toggles it.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Accessory {
    Backpack,
}

#[derive(Default, Clone, Copy)]
struct Foot {
    planted: Vec3,
    from: Vec3,
    to: Vec3,
    t: f32,
    step_dur: f32,
    stepping: bool,
}

/// A 3D spring-damper state in pelvis-local coordinates.
#[derive(Default, Clone, Copy)]
struct SpringVec3 {
    pos: Vec3,
    vel: Vec3,
}

impl SpringVec3 {
    fn step(&mut self, target: Vec3, external_accel: Vec3, kp: f32, kd: f32, dt: f32) {
        let force = (target - self.pos) * kp - self.vel * kd + external_accel;
        self.vel += force * dt;
        self.pos += self.vel * dt;
    }
}

#[derive(Resource, Default)]
struct Gait {
    feet: [Foot; 2],
    init: bool,
    /// Which foot stepped last (0 or 1), so strides naturally alternate Left -> Right -> Left.
    last_step_foot: usize,
    /// Seconds until another step may start (prevents rapid step-spam jitter).
    step_cooldown: f32,
    /// Low-pass filtered horizontal velocity used for footstep placement.
    vs: Vec3,
    /// Hysteretic 'walking vs standing' flag so footstep rules don't flip at a threshold.
    moving: bool,
    /// Whether the previous frame was airborne (to detect touchdown).
    was_airborne: bool,
    phase: f32,
    /// Spine pitch/roll deflection in pelvis-local space (x = lateral roll, z = forward/back pitch).
    spine_bend: SpringVec3,
    /// Torso twist: x = chest yaw relative to the pelvis, y = pelvis yaw counter-twist.
    spine_twist: SpringVec3,
    /// Head pitch/yaw/roll deflection relative to chest.
    head_angles: SpringVec3,
    /// Pelvis-local ankle positions driven by spring-damper physics + anatomical clamps.
    local_ankles: [SpringVec3; 2],
    /// Pelvis-local wrist positions driven by spring-damper physics + anatomical clamps.
    local_wrists: [SpringVec3; 2],
}

fn spawn_body(
    mut commands: Commands,
    cosmetics: Res<Cosmetics>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let mat = |m: &mut Assets<StandardMaterial>, rgb: (f32, f32, f32), rough: f32, metal: f32| {
        m.add(StandardMaterial {
            base_color: Color::srgb(rgb.0, rgb.1, rgb.2),
            perceptual_roughness: rough,
            metallic: metal,
            ..default()
        })
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
    let screen = mats.add(StandardMaterial {
        base_color: Color::srgb(0.03, 0.06, 0.07),
        perceptual_roughness: 0.06,
        reflectance: 0.9,
        ..default()
    });
    let eye = mats.add(StandardMaterial {
        base_color: Color::BLACK,
        emissive: LinearRgba::rgb(0.6, 3.0, 3.2) * 2_000.0,
        ..default()
    });
    // Rocket flame materials: bright orange-red outer plume + white-blue hot core!
    let flame_outer = mats.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.35, 0.05),
        emissive: LinearRgba::rgb(4.5, 1.4, 0.15) * 3_500.0,
        unlit: true,
        ..default()
    });
    let flame_core = mats.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.95, 1.0),
        emissive: LinearRgba::rgb(2.5, 4.0, 6.0) * 5_500.0,
        unlit: true,
        ..default()
    });
    let brass = mat(&mut mats, (0.85, 0.66, 0.32), 0.3, 0.9);
    let _wing = mat(&mut mats, (0.09, 0.13, 0.30), 0.55, 0.0);
    let canopy = mats.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.42, 0.09),
        perceptual_roughness: 0.9,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let cyl = meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(20));
    let ball = meshes.add(Sphere::new(0.5).mesh().uv(20, 12));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let dome = meshes.add(Sphere::new(1.0).mesh().uv(24, 8));
    let cone = meshes.add(Cone::new(0.5, 1.0).mesh().resolution(16));

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
        // --- Feet: sneaker = upper + white sole + toe cap + thruster nozzle ---
        (Rigid { f: F::FootL, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, &cube, &shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, &cube, &shoe),
        (Rigid { f: F::FootL, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, &cube, &sole),
        (Rigid { f: F::FootR, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, &cube, &sole),
        (Rigid { f: F::FootL, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, &cyl, &shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, &cyl, &shoe),
        (Rigid { f: F::FootL, off: v(0.0, -0.095, -0.03), rot: q0, scale: v(0.09, 0.025, 0.09) }, &cyl, &joint),
        (Rigid { f: F::FootR, off: v(0.0, -0.095, -0.03), rot: q0, scale: v(0.09, 0.025, 0.09) }, &cyl, &joint),
        // --- Boot Rocket Jet Flames (Left & Right, Outer & Core) ---
        (JetFlame { foot_idx: 0, inner: false }, &cone, &flame_outer),
        (JetFlame { foot_idx: 0, inner: true }, &cone, &flame_core),
        (JetFlame { foot_idx: 1, inner: false }, &cone, &flame_outer),
        (JetFlame { foot_idx: 1, inner: true }, &cone, &flame_core),
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
        commands.spawn((
            BodyPart(spec),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(m.clone()),
            Transform::default(),
            Visibility::Visible,
        ));
    }
    let backpack: [(Spec, &Handle<Mesh>, &Handle<StandardMaterial>); 4] = [
        (Rigid { f: F::Chest, off: v(0.0, -0.18, 0.27), rot: q0, scale: v(0.4, 0.42, 0.26) }, &cube, &pack),
        (Rigid { f: F::Chest, off: v(0.0, -0.18, 0.27), rot: q0, scale: v(0.41, 0.06, 0.27) }, &cube, &strap),
        (Rigid { f: F::Chest, off: v(-0.13, -0.1, -0.005), rot: q0, scale: v(0.05, 0.42, 0.29) }, &cube, &strap),
        (Rigid { f: F::Chest, off: v(0.13, -0.1, -0.005), rot: q0, scale: v(0.05, 0.42, 0.29) }, &cube, &strap),
    ];
    for (spec, mesh, m) in backpack {
        commands.spawn((
            BodyPart(spec),
            Accessory::Backpack,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(m.clone()),
            Transform::default(),
            if cosmetics.backpack { Visibility::Visible } else { Visibility::Hidden },
        ));
    }
    let wing_mat = mats.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.16, 0.34),
        perceptual_roughness: 0.6,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    for k in 0..3 {
        commands.spawn((
            WingMesh(k),
            Mesh3d(meshes.add(wing_mesh_blank())),
            MeshMaterial3d(wing_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            NoFrustumCulling,
        ));
    }
    commands.insert_resource(Gait::default());
}

/// Two-bone IK: returns (middle joint, end) given root, target, lengths and
/// a pole direction (which way the knee/elbow should point).
fn two_bone(root: Vec3, target: Vec3, l1: f32, l2: f32, pole: Vec3) -> (Vec3, Vec3) {
    let to = target - root;
    let d = to.length().clamp(0.12, l1 + l2 - 0.002);
    let dir = to.normalize_or(Vec3::NEG_Y);
    let end = root + dir * d;
    let cos_a = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
    let a = cos_a.acos();
    let bend = pole.reject_from_normalized(dir).normalize_or(Vec3::NEG_Z);
    let mid = root + (dir * a.cos() + bend * a.sin()) * l1;
    (mid, end)
}

/// Enforces hard anatomical joint limits on an ankle in pelvis-local space (`hip` = local hip position).
/// Prevents legs from ever folding up into the torso or crossing through the midline.
fn clamp_ankle_anatomical(hip: Vec3, mut ankle: Vec3, side: f32) -> Vec3 {
    let max_reach = THIGH + SHIN - 0.006;
    let min_reach = 0.26; // knee max bend ~135 deg
    // Ankle must stay below hip (`y <= hip.y - 0.16`) so legs never enter the torso!
    ankle.y = ankle.y.min(hip.y - 0.16);
    // Prevent crossing across the other leg's midline:
    if side < 0.0 {
        ankle.x = ankle.x.clamp(hip.x - 0.42, -0.03);
    } else {
        ankle.x = ankle.x.clamp(0.03, hip.x + 0.42);
    }
    // Forward/backward hip swing limit (-Z is forward, +Z is backward):
    ankle.z = ankle.z.clamp(-0.66, 0.62);

    let rel = ankle - hip;
    let len = rel.length();
    if len > max_reach {
        ankle = hip + rel * (max_reach / len);
    } else if len < min_reach {
        ankle = hip + rel.normalize_or(Vec3::NEG_Y) * min_reach;
    }
    ankle
}

/// Enforces hard anatomical joint limits on a wrist in pelvis-local space (`shoulder` = local shoulder position).
/// Prevents arms from clipping inside the chest box.
fn clamp_wrist_anatomical(shoulder: Vec3, mut wrist: Vec3, side: f32) -> Vec3 {
    let max_reach = UPPER_ARM + FOREARM - 0.008;
    let min_reach = 0.15;
    if wrist.y < shoulder.y + 0.05 && wrist.y > shoulder.y - 0.48 && wrist.z.abs() < 0.17 {
        if side < 0.0 {
            wrist.x = wrist.x.min(-0.22);
        } else {
            wrist.x = wrist.x.max(0.22);
        }
    }
    let rel = wrist - shoulder;
    let len = rel.length();
    if len > max_reach {
        wrist = shoulder + rel * (max_reach / len);
    } else if len < min_reach {
        wrist = shoulder + rel.normalize_or(Vec3::X * side) * min_reach;
    }
    wrist
}

/// Unit cylinder (r 0.5, h 1, along Y) spanning a..b.
fn limb(a: Vec3, b: Vec3, r: f32) -> Transform {
    let d = b - a;
    let len = d.length().max(0.001);
    Transform {
        translation: (a + b) * 0.5,
        rotation: Quat::from_rotation_arc(Vec3::Y, d / len),
        scale: Vec3::new(r * 2.0, len, r * 2.0),
    }
}

#[allow(clippy::type_complexity)]
fn animate_body(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    spatial: SpatialQuery,
    mut gait: ResMut<Gait>,
    mut players: Query<(Entity, &Transform, &LinearVelocity, &mut Player), Without<BodyPart>>,
    cosmetics: Res<Cosmetics>,
    mut parts: Query<(&BodyPart, Option<&Accessory>, &mut Transform, &mut Visibility)>,
    mut wings: Query<(&WingMesh, &Mesh3d, &mut Visibility), Without<BodyPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Ok((pe, pt, vel, mut p)) = players.single_mut() else { return };
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    let up = origin.up_at(pt.translation);

    // The pelvis orientation comes directly from the physical 6-DOF RigidBody transform!
    let base = if pt.rotation == Quat::IDENTITY && p.yaw != 0.0 {
        Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(p.yaw)
    } else {
        pt.rotation
    };
    let inv_base = base.inverse();
    let yaw_only = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(p.yaw);
    let right_yaw = yaw_only * Vec3::X;
    let fwd_yaw = yaw_only * Vec3::NEG_Z;

    let v = vel.0;
    let v_flat = v - up * v.dot(up);
    let speed = v_flat.length();
    let airborne = p.mode != MoveMode::Walk;

    let local_accel = inv_base * p.accel_smooth;
    let local_vel = inv_base * v;
    let local_wind = -local_vel;
    // How hard the body is trying to rotate onto its aimed posture, in body-local axes.
    let local_err = inv_base * p.rot_err;
    // Where the player wants to go, in body axes: limbs and spine bend toward it.
    let il = inv_base * p.intent_dir;
    let wing_amp = if matches!(p.mode, MoveMode::Skydive | MoveMode::Fall) { (v.length() / 70.0).clamp(0.0, 1.0) } else { 0.0 };
    let rot_effort = local_err.length().min(2.5);

    // The pelvis is directly at the RigidBody's physical translation (`pt.translation`),
    // because the leg forces in `drive_player` physically hold `pt.translation` up above the feet!
    let leg = THIGH + SHIN;
    let pelvis_c = pt.translation;
    // Pelvis counter-twist (spring state from the previous frame) swings the hips about the spine.
    let pelvis_twist = Quat::from_rotation_y(gait.spine_twist.pos.y);
    let hips_local = [pelvis_twist * Vec3::new(-HIP_W, -0.04, 0.0), pelvis_twist * Vec3::new(HIP_W, -0.04, 0.0)];

    if !gait.init {
        for i in 0..2 {
            let side = if i == 0 { -1.0 } else { 1.0 };
            let default_ankle = Vec3::new(side * HIP_W, -(leg * 0.94), 0.0);
            gait.local_ankles[i].pos = default_ankle;
            gait.feet[i].planted = pelvis_c + base * (default_ankle - Vec3::Y * ANKLE_H);
            gait.feet[i].step_dur = BASE_STEP_TIME;
            gait.local_wrists[i].pos = Vec3::new(side * (SHOULDER_W + 0.06), -0.12, 0.0);
        }
        gait.init = true;
    }

    // --- Capture-Point Ground Stepping & Mid-Air Leg Vectoring ---
    let filter = SpatialQueryFilter::from_excluded_entities([pe]);
    let ground_at = |world_pt: Vec3| -> Option<Vec3> {
        spatial
            .cast_ray(world_pt + up * 0.9, Dir3::new(-up).ok()?, 2.5, true, &filter)
            .map(|h| world_pt + up * (0.9 - h.distance))
    };

    // How far does the torso lean horizontally relative to `up`?
    let body_up = base * Vec3::Y;

    let mut target_ankles_local = [Vec3::ZERO; 2];

    if airborne {
        gait.was_airborne = true;
        gait.moving = false;
        for i in 0..2 {
            let side = if i == 0 { -1.0f32 } else { 1.0f32 };
            gait.feet[i].stepping = false;
            gait.feet[i].planted = pelvis_c + base * (gait.local_ankles[i].pos - Vec3::Y * ANKLE_H);
            p.foot_planted[i] = false;

            target_ankles_local[i] = match p.mode {
                MoveMode::RocketShoes => {
                    // Self-balancing gimbal legs in rocket mode:
                    // Because the body itself rotates in 3D to align its +Y axis with the camera-relative
                    // thrust direction, the legs extend straight along -Y (`-0.96 * leg`) so the soles
                    // push cleanly along the body's 3D thrust axis without fighting the knee bend!
                    let stance_w = if p.afterburner && p.fwd_in.abs() > 0.1 {
                        HIP_W * 0.80
                    } else {
                        HIP_W + 0.03
                    };
                    // While the torso swings around, the legs kick opposite the turn (reaction swing),
                    // scissor apart and knees tuck, so the whole body visibly works to re-aim.
                    let kick = (-local_err.cross(Vec3::NEG_Y * leg) * 0.30).clamp_length_max(0.55);
                    let tuck = rot_effort * 0.07;
                    // Hips and knees also bend toward the intended direction: feet trail behind it.
                    let trail = Vec3::new(-il.x, 0.0, -il.z).clamp_length_max(1.0) * 0.16;
                    let knee = Vec3::new(il.x, 0.0, il.z).length().min(1.0) * 0.08;
                    Vec3::new(side * (stance_w + rot_effort * 0.04), -(leg * 0.96) + tuck + knee, 0.02) + kick + trail
                }
                MoveMode::Skydive => {
                    let spread = 0.09 - 0.04 * p.fwd_in.max(0.0);
                    let knee_bend = if p.fwd_in < 0.0 { 0.15 * (-p.fwd_in) } else { 0.03 };
                    Vec3::new(side * (HIP_W + spread), -(leg * (0.94 - knee_bend)), 0.12 + knee_bend * 0.4)
                }
                MoveMode::Parachute => Vec3::new(side * (HIP_W + 0.03), -(leg * 0.90), 0.06),
                MoveMode::Fall => Vec3::new(side * (HIP_W + 0.05), -(leg * 0.84), 0.08),
                MoveMode::Walk => Vec3::new(side * HIP_W, -(leg * 0.92), 0.0),
            };
        }
    } else {
        // On the ground: Capture-Point Balance Stepping!
        // Where does each foot *need* to be on the ground so the robot stays balanced or propels itself?
        // Capture point = hip projection on ground + velocity lead + torso lean lead + wish step lead.
        // Smoothed horizontal velocity: physics velocity is noisy frame to frame and must not
        // drive footstep placement directly.
        let k_v = 1.0 - (-9.0 * dt).exp();
        gait.vs = gait.vs.lerp(v_flat, k_v);
        let vs = gait.vs;
        let vs_speed = vs.length();
        gait.moving = if gait.moving { vs_speed > 0.2 } else { vs_speed > 0.45 };
        let travelling = gait.moving;
        let dir = vs.normalize_or_zero();

        // Stride grows with speed (Raibert-style): half-stride the foot lands ahead of the hip.
        let s_half = (0.10 + 0.11 * vs_speed).clamp(0.10, 0.46);
        let step_dur = (0.27 - 0.04 * vs_speed).clamp(0.13, 0.24);
        // Right after landing the body is sliding on its knees: feet stay planted and slide with it.
        let sliding = p.landing_recovery > 0.1;
        gait.step_cooldown = (gait.step_cooldown - dt).max(0.0);

        let hip_ground = |side: f32| pelvis_c + right_yaw * (side * HIP_W);
        let hips_world_est = |i: usize| pelvis_c + base * hips_local[i];

        // Touchdown: the 'planted' positions were just trailing the body in the air. Put both feet
        // under their hips on the ground and calm the leg springs so nothing scrambles.
        if gait.was_airborne {
            for i in 0..2 {
                let side = if i == 0 { -1.0f32 } else { 1.0f32 };
                if let Some(g) = ground_at(hip_ground(side)) {
                    gait.feet[i].planted = g;
                }
                gait.feet[i].stepping = false;
                gait.local_ankles[i].vel = Vec3::ZERO;
            }
            gait.step_cooldown = 0.0;
            gait.was_airborne = false;
        }

        if sliding {
            for f in gait.feet.iter_mut() {
                f.stepping = false;
                f.planted += v_flat * dt;
            }
        }

        let any_stepping = gait.feet[0].stepping || gait.feet[1].stepping;
        if !any_stepping && !sliding && gait.step_cooldown <= 0.0 {
            // How urgently does each foot need to move?
            let mut need = [0.0f32; 2];
            for i in 0..2 {
                let side = if i == 0 { -1.0f32 } else { 1.0f32 };
                let rel = (gait.feet[i].planted - hip_ground(side)).reject_from_normalized(up);
                let rel_x = rel.dot(right_yaw);
                let rel_z = rel.dot(fwd_yaw);
                let inward = -side * rel_x;
                let comfort_lat = if inward > 0.0 { inward / 0.10 } else { -inward / 0.30 };
                let reach = (hips_world_est(i) - gait.feet[i].planted).length() / (leg * 0.97);
                let comfort = comfort_lat.max(reach).max(rel_z.abs() / if travelling { 0.55 } else { 0.30 });
                need[i] = comfort.max(if travelling {
                    // Only a foot that is *behind* the hip (along travel) is worth lifting, so a
                    // foot that just landed ahead can never step backwards. Lateral drift counts too.
                    let behind = -rel.dot(dir);
                    let lateral = (rel - dir * rel.dot(dir)).length();
                    (behind / s_half).max(lateral / 0.30)
                } else {
                    // Standing: only re-centre a foot that has wandered well away from its hip.
                    rel.length() / 0.34
                });
            }
            let pick = if need[0] >= need[1] { 0 } else { 1 };
            // Standing needs a clearly larger error than walking, so sway never triggers repeated taps.
            let thresh = if travelling { 1.0 } else { 1.25 };
            if need[pick] > thresh {
                let side = if pick == 0 { -1.0f32 } else { 1.0f32 };
                let hip_pred = hip_ground(side) + vs * (step_dur * 0.9);
                let target_xz = if travelling { hip_pred + dir * (s_half * 0.9) } else { hip_ground(side) };
                let landing_pt = ground_at(target_xz)
                    .or(p.foot_ground_hit[pick])
                    .unwrap_or(target_xz - up * (leg * 0.94 + ANKLE_H));
                let f = &mut gait.feet[pick];
                f.stepping = true;
                f.t = 0.0;
                f.step_dur = step_dur;
                f.from = f.planted;
                f.to = landing_pt;
                gait.last_step_foot = pick;
                gait.step_cooldown = if travelling { step_dur * 0.35 } else { 0.6 };
            }
        }

        for i in 0..2 {
            let f = &mut gait.feet[i];
            if f.stepping {
                f.t += dt / f.step_dur.max(0.08);
                if f.t >= 1.0 {
                    f.stepping = false;
                    f.planted = f.to;
                }
            }

            let foot_world = if f.stepping {
                p.foot_planted[i] = false;
                let s_t = f.t.clamp(0.0, 1.0);
                let s = s_t * s_t * (3.0 - 2.0 * s_t);
                // Lift foot in a clean arc over steps/curbs:
                let step_span = (f.to - f.from).length();
                let lift_h = (0.09 + step_span * 0.22).clamp(0.08, 0.28);
                f.from.lerp(f.to, s) + up * ((s_t * std::f32::consts::PI).sin() * lift_h)
            } else {
                p.foot_planted[i] = true;
                if let Some(g_pt) = ground_at(f.planted) {
                    f.planted = g_pt;
                }
                f.planted
            };

            let ankle_world = foot_world + up * ANKLE_H;
            target_ankles_local[i] = inv_base * (ankle_world - pelvis_c);
        }
    }

    gait.phase += speed * dt * 2.2;

    // --- Spine Physical Spring-Damper + Anatomical Flex Limits ---
    let spine_target = match p.mode {
        MoveMode::Walk => {
            let run_lean_fwd = -(speed / 7.5).min(1.0) * 0.22;
            let squat_lean = if p.jump_charge > 0.05 || p.crouch {
                -0.28 * p.jump_charge.max(if p.crouch { 0.8 } else { 0.0 })
            } else {
                0.0
            };
            let turn_roll = p.strafe * 0.14;
            Vec3::new(turn_roll, 0.0, run_lean_fwd + squat_lean)
        }
        MoveMode::RocketShoes => {
            // In rocket flight:
            // - W (`fwd_in > 0`): lean chest forward (-Z) into the boost!
            // - S (`fwd_in < 0`): arch chest back (+Z) to pull back and retro-boost!
            // - A/D (`strafe`): flex spine laterally into the turn!
            Vec3::new((il.x * 0.45).clamp(-0.34, 0.34), 0.0, (il.z * 0.45).clamp(-0.42, 0.38))
        }
        MoveMode::Skydive => {
            // Flared (S) arches the chest up; diving (W) tucks it; turns bend the spine into the bank.
            let arch = -p.fwd_in * 0.24 + 0.14;
            Vec3::new(p.strafe * 0.22, 0.0, arch)
        }
        _ => Vec3::new(p.strafe * 0.10, 0.0, 0.06),
    };
    let spine_inertial = Vec3::new(
        (-local_accel.x * 0.018).clamp(-0.45, 0.45) + (time.elapsed_secs() * 17.0).sin() * 0.05 * wing_amp,
        0.0,
        (-local_accel.z * 0.020).clamp(-0.45, 0.45) + (time.elapsed_secs() * 13.0).cos() * 0.05 * wing_amp,
    );
    gait.spine_bend.step(spine_target, spine_inertial * 20.0, 85.0, 13.0, dt);
    gait.spine_bend.pos.x = gait.spine_bend.pos.x.clamp(-0.34, 0.34);
    gait.spine_bend.pos.z = gait.spine_bend.pos.z.clamp(-0.42, 0.38);

    let spine_dir_local = Vec3::new(gait.spine_bend.pos.x, 1.0, gait.spine_bend.pos.z).normalize();
    let chest_local = spine_dir_local * SPINE;
    let chest = pelvis_c + base * chest_local;
    // Chest leads the turn, pelvis lags and counter-twists, so the torso visibly winds up.
    let twist_target = Vec3::new(
        (local_err.y * 0.55 + p.strafe * if airborne { 0.22 } else { 0.0 }).clamp(-0.75, 0.75),
        (-local_err.y * 0.28).clamp(-0.4, 0.4),
        0.0,
    );
    gait.spine_twist.step(twist_target, Vec3::ZERO, 90.0, 12.0, dt);
    gait.spine_twist.pos.x = gait.spine_twist.pos.x.clamp(-0.8, 0.8);
    gait.spine_twist.pos.y = gait.spine_twist.pos.y.clamp(-0.45, 0.45);
    let chest_twist = Quat::from_rotation_y(gait.spine_twist.pos.x);
    let chest_rot = base * Quat::from_rotation_arc(Vec3::Y, spine_dir_local) * chest_twist;
    let chest_up = chest_rot * Vec3::Y;

    // --- Head Physical Spring-Damper + Gaze ---
    let t_sec = time.elapsed_secs();
    let head_target = match p.mode {
        MoveMode::Skydive => Vec3::new(0.60 - p.fwd_in * 0.15, -p.strafe * 0.28, -p.strafe * 0.15),
        MoveMode::RocketShoes => {
            let pitch_look = if p.fwd_in > 0.1 {
                0.55
            } else if p.fwd_in < -0.1 {
                -0.25
            } else {
                0.15
            };
            Vec3::new(pitch_look, -p.strafe * 0.25, -p.strafe * 0.15)
        }
        MoveMode::Fall => Vec3::new(-0.42, 0.0, (t_sec * 1.2).sin() * 0.04),
        _ => Vec3::new(
            (p.cam_pitch * 0.35).clamp(-0.4, 0.4) + (t_sec * 0.7).sin() * 0.025,
            -p.strafe * 0.15,
            (t_sec * 0.45).sin() * 0.03,
        ),
    };
    let head_inertial = Vec3::new(
        (local_accel.z * 0.024).clamp(-0.55, 0.55),
        0.0,
        (-local_accel.x * 0.024).clamp(-0.55, 0.55),
    );
    gait.head_angles.step(head_target, head_inertial * 16.0, 95.0, 14.0, dt);
    gait.head_angles.pos.x = gait.head_angles.pos.x.clamp(-0.68, 0.85);
    gait.head_angles.pos.y = gait.head_angles.pos.y.clamp(-0.65, 0.65);
    gait.head_angles.pos.z = gait.head_angles.pos.z.clamp(-0.48, 0.48);

    let neck = chest + chest_up * 0.10;
    let head_c = neck + chest_up * 0.21;
    let head_rot = chest_rot
        * Quat::from_euler(
            EulerRot::YXZ,
            gait.head_angles.pos.y,
            gait.head_angles.pos.x,
            gait.head_angles.pos.z,
        );

    // --- Step Leg Springs, Enforce Hard Anatomical Limits & Write Foot State Back to Player ---
    let leg_wind_drag = (local_wind * 0.007).clamp_length_max(0.40);
    let leg_inertia = (-local_accel * 0.007).clamp_length_max(0.40);
    let mut ankles_world = [Vec3::ZERO; 2];
    let mut knees_world = [Vec3::ZERO; 2];
    let mut hips_world = [Vec3::ZERO; 2];

    for i in 0..2 {
        let side = if i == 0 { -1.0f32 } else { 1.0f32 };
        let (kp, kd) = if airborne { (125.0, 19.0) } else { (600.0, 40.0) };
        if airborne {
            gait.local_ankles[i].step(
                target_ankles_local[i],
                (leg_wind_drag + leg_inertia) * 14.0,
                kp,
                kd,
                dt,
            );
        } else {
            // On the ground the foot is held by contact, not by a bouncy spring: approach the
            // contact/swing target with a first-order filter (no overshoot, no ringing, any dt).
            let k = 1.0 - (-45.0 * dt).exp();
            let cur = gait.local_ankles[i].pos;
            gait.local_ankles[i].pos = cur.lerp(target_ankles_local[i], k);
            gait.local_ankles[i].vel = Vec3::ZERO;
        }
        gait.local_ankles[i].pos = clamp_ankle_anatomical(hips_local[i], gait.local_ankles[i].pos, side);

        let pole_local = Vec3::new(side * 0.08, 0.10, -1.0).normalize();
        let (knee_loc, ankle_loc) = two_bone(
            hips_local[i],
            gait.local_ankles[i].pos,
            THIGH,
            SHIN,
            pole_local,
        );
        hips_world[i] = pelvis_c + base * hips_local[i];
        knees_world[i] = pelvis_c + base * knee_loc;
        ankles_world[i] = pelvis_c + base * ankle_loc;

        // Write actual world foot position & sole thrust axis back to `Player`!
        let planted_g = if !airborne && !gait.feet[i].stepping {
            gait.feet[i].planted
        } else {
            ankles_world[i] - body_up * ANKLE_H
        };
        p.foot_world_pos[i] = planted_g;
        // Full leg axis from ankle through hip (`hips_world[i] - ankles_world[i]`) defines the shoe's +Y thrust axis
        // so normal knee bend doesn't deflect thrust away from the body's aimed 3D orientation!
        let leg_up = (hips_world[i] - ankles_world[i]).normalize_or(body_up);
        p.foot_up_dir[i] = leg_up;
    }

    // --- Step Arm Springs & Enforce Hard Anatomical Limits ---
    let shoulders_local = [
        chest_local + chest_twist * Vec3::new(-SHOULDER_W, -0.06, 0.0),
        chest_local + chest_twist * Vec3::new(SHOULDER_W, -0.06, 0.0),
    ];
    // Arm swing follows the actual forward/back offset of the opposite foot!
    let foot_z_diff = (gait.local_ankles[0].pos.z - gait.local_ankles[1].pos.z).clamp(-0.45, 0.45);
    let arm_wind_buffet = Vec3::new(
        (t_sec * 19.0).sin() * 0.04,
        (t_sec * 23.0).cos() * 0.04,
        (t_sec * 17.0).sin() * 0.04,
    ) * (local_wind.length() / 35.0).clamp(0.0, 1.5)
        * if matches!(p.mode, MoveMode::Skydive | MoveMode::Fall) { 2.5 } else { 1.0 };
    let arm_inertia = (-local_accel * 0.011 + local_wind * 0.005).clamp_length_max(0.55);

    let mut shoulders_world = [Vec3::ZERO; 2];
    let mut elbows_world = [Vec3::ZERO; 2];
    let mut wrists_world = [Vec3::ZERO; 2];

    for i in 0..2 {
        let side = if i == 0 { -1.0f32 } else { 1.0f32 };
        let sh = shoulders_local[i];
        let target_wrist = match p.mode {
            MoveMode::RocketShoes => {
                if p.fwd_in > 0.1 {
                    // Pressing W in rocket flight: reach arms forward (-Z) & slightly out to pitch the body forward!
                    let reach = if p.afterburner { 0.34 } else { 0.26 };
                    sh + Vec3::new(side * 0.16, -0.18, -reach)
                } else if p.fwd_in < -0.1 {
                    // Pressing S (retro-boost brake): throw arms back/out to counterbalance the feet swinging forward!
                    sh + Vec3::new(side * 0.36, -0.16, 0.24)
                } else if p.strafe.abs() > 0.1 {
                    // Pressing A/D: spread arms asymmetrically to roll/spin into the turn!
                    let asym = side * p.strafe * 0.14;
                    sh + Vec3::new(side * 0.34, -0.22 + asym, -0.06)
                } else if p.afterburner {
                    // Shift afterburner hover/climb: arms pinned tight and sleek to the sides!
                    sh + Vec3::new(side * 0.08, -0.48, 0.05)
                } else {
                    // Normal Space rocket hover: arms out ~30 deg from sides for balance stabilization.
                    sh + Vec3::new(side * 0.25, -0.36, 0.04)
                }
            }
            MoveMode::Skydive => {
                let sweep_back = p.fwd_in.max(0.0) * 0.24;
                let tuck_in = p.fwd_in.max(0.0) * 0.14;
                let flare_z = p.fwd_in.min(0.0) * 0.20;
                // Turn-side arm lifts into the wind (more drag/lift on that wing), the other drops.
                let bank_asym = (-side) * p.wing_turn * 0.16;
                sh + Vec3::new(
                    side * (0.50 - tuck_in),
                    -0.06 - sweep_back + bank_asym,
                    0.06 + sweep_back * 0.5 + flare_z,
                )
            }
            MoveMode::Fall => sh + Vec3::new(side * 0.44, -0.10 + (-side) * p.wing_turn * 0.16, -0.05),
            MoveMode::Parachute => sh + Vec3::new(side * 0.08, 0.46, -0.08),
            MoveMode::Walk => {
                if p.jump_charge > 0.05 || p.crouch {
                    // Swing arms back while charging a jump squat so they help throw weight upward on release!
                    let c = p.jump_charge.max(if p.crouch { 0.7 } else { 0.0 });
                    sh + Vec3::new(side * 0.12, -0.38, 0.28 * c)
                } else if p.jump_push_timer > 0.0 {
                    // Swing arms forward/up during the jump push!
                    sh + Vec3::new(side * 0.14, -0.15, -0.28)
                } else {
                    // Natural counter-swing driven by the actual opposite foot's position!
                    let swing_z = side * foot_z_diff * 0.95;
                    sh + Vec3::new(side * 0.08, -0.46 + swing_z.abs() * 0.08, swing_z)
                }
            }
        };

        // Arms fling opposite the turn while re-aiming, like counter-rotating for balance.
        let fling = if p.mode == MoveMode::RocketShoes {
            (-local_err.cross(Vec3::new(side * 0.45, -0.25, 0.0)) * 0.55).clamp_length_max(0.5)
        } else {
            Vec3::ZERO
        };
        let target_wrist = target_wrist + fling;
        gait.local_wrists[i].step(
            target_wrist,
            (arm_inertia + arm_wind_buffet) * 16.0,
            135.0,
            18.0,
            dt,
        );
        gait.local_wrists[i].pos = clamp_wrist_anatomical(sh, gait.local_wrists[i].pos, side);

        let elbow_pole = Vec3::new(side * 0.35, -0.20, 1.0).normalize();
        let (elbow_loc, wrist_loc) = two_bone(
            sh,
            gait.local_wrists[i].pos,
            UPPER_ARM,
            FOREARM,
            elbow_pole,
        );
        shoulders_world[i] = pelvis_c + base * sh;
        elbows_world[i] = pelvis_c + base * elbow_loc;
        wrists_world[i] = pelvis_c + base * wrist_loc;
    }

    let joint = |j: J| -> Vec3 {
        match j {
            J::Pelvis => pelvis_c,
            J::Chest => chest,
            J::Neck => neck,
            J::HipL => hips_world[0],
            J::HipR => hips_world[1],
            J::KneeL => knees_world[0],
            J::KneeR => knees_world[1],
            J::AnkleL => ankles_world[0],
            J::AnkleR => ankles_world[1],
            J::ShoulderL => shoulders_world[0],
            J::ShoulderR => shoulders_world[1],
            J::ElbowL => elbows_world[0],
            J::ElbowR => elbows_world[1],
            J::WristL => wrists_world[0],
            J::WristR => wrists_world[1],
        }
    };

    let hand_rot = |e: Vec3, w: Vec3| Quat::from_rotation_arc(-body_up, (w - e).normalize_or(-body_up)) * base;
    let foot_rot = |h: Vec3, a: Vec3, _idx: usize| {
        if !airborne {
            let n = p.ground_normal.normalize_or(up);
            let _ = fwd_yaw;
            Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(p.yaw)
        } else {
            Quat::from_rotation_arc(-body_up, (a - h).normalize_or(-body_up)) * base
        }
    };
    let frame = |f: F| -> (Vec3, Quat) {
        match f {
            F::Pelvis => (pelvis_c, base * pelvis_twist),
            F::Chest => (chest, chest_rot),
            F::Head => (head_c, head_rot),
            F::FootL => (ankles_world[0], foot_rot(hips_world[0], ankles_world[0], 0)),
            F::FootR => (ankles_world[1], foot_rot(hips_world[1], ankles_world[1], 1)),
            F::HandL => (wrists_world[0], hand_rot(elbows_world[0], wrists_world[0])),
            F::HandR => (wrists_world[1], hand_rot(elbows_world[1], wrists_world[1])),
        }
    };
    let blink = if (t_sec % 3.7) < 0.12 { 0.15 } else { 1.0 };

    for (bp, acc, mut tf, mut vis) in &mut parts {
        *vis = match acc {
            Some(Accessory::Backpack) if !cosmetics.backpack => Visibility::Hidden,
            _ => Visibility::Visible,
        };
        match bp.0 {
            Spec::Limb { a, b, r } => *tf = limb(joint(a), joint(b), r),
            Spec::Ball { j, r } => {
                *tf = Transform::from_translation(joint(j)).with_scale(Vec3::splat(r * 2.0))
            }
            Spec::Rigid { f, off, rot, scale } => {
                let (p0, q) = frame(f);
                *tf = Transform {
                    translation: p0 + q * off,
                    rotation: q * rot,
                    scale,
                };
                if f == F::Head && p.first_person {
                    *vis = Visibility::Hidden;
                }
            }
            Spec::Eye { off } => {
                let (p0, q) = frame(F::Head);
                *tf = Transform {
                    translation: p0 + q * off,
                    rotation: q,
                    scale: Vec3::new(0.055, 0.075 * blink, 0.02),
                };
                if p.first_person {
                    *vis = Visibility::Hidden;
                }
            }
            Spec::JetFlame { foot_idx, inner } => {
                if p.throttle <= 0.01 {
                    *vis = Visibility::Hidden;
                } else {
                    let f_frame = if foot_idx == 0 { F::FootL } else { F::FootR };
                    let (p0, q) = frame(f_frame);
                    // High-frequency turbulent flicker for the rocket exhaust plume:
                    let phase = t_sec * 62.0 + (foot_idx as f32) * 1.9;
                    let flicker = 0.88 + 0.24 * phase.sin() * (phase * 1.7).cos();
                    let len = (0.28 + p.throttle * 0.34) * flicker * if inner { 0.62 } else { 1.0 };
                    let width = (0.09 + p.throttle.min(2.5) * 0.035) * if inner { 0.58 } else { 1.0 };
                    // Nozzle is at local (0.0, -0.10, -0.03) on the shoe; flame shoots straight down (-Y of the foot)!
                    let nozzle_local = Vec3::new(0.0, -0.10 - len * 0.5, -0.03);
                    // Cone mesh tip points along +Y by default; rotate 180 deg around X so tip points down (-Y) into the exhaust trail!
                    let flip = Quat::from_rotation_x(std::f32::consts::PI);
                    *tf = Transform {
                        translation: p0 + q * nozzle_local,
                        rotation: q * flip,
                        scale: Vec3::new(width, len, width),
                    };
                }
            }
            Spec::Canopy => {
                if p.mode == MoveMode::Parachute {
                    *tf = Transform::from_translation(pelvis_c + body_up * 6.5)
                        .with_rotation(base)
                        .with_scale(Vec3::new(4.5, 1.6, 2.6));
                } else {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }

    // --- Wingsuit membranes: follow the actual arm and leg joints, billow with the wind ---
    let flying = p.wingsuit_on && !matches!(p.mode, MoveMode::Walk | MoveMode::Parachute) && !p.first_person;
    let back = base * Vec3::Z;
    let bulge = 0.05 + wing_amp * 0.03 + (0.012 + 0.02 * wing_amp) * (t_sec * 38.0).sin();
    for (w, m3, mut vis) in &mut wings {
        if !flying {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;
        let (poly, lift): ([Vec3; 7], Vec3) = match w.0 {
            0 | 1 => {
                let s = w.0;
                let wrist = wrists_world[s];
                let ankle = ankles_world[s];
                let trail = (wrist + ankle) * 0.5 + back * (bulge * 0.6);
                ([shoulders_world[s], elbows_world[s], wrist, trail, ankle, knees_world[s], hips_world[s]], back * bulge)
            }
            _ => {
                let crotch = (hips_world[0] + hips_world[1]) * 0.5 - body_up * 0.04;
                ([hips_world[0], knees_world[0], ankles_world[0], ankles_world[1], knees_world[1], hips_world[1], crotch], back * (bulge * 0.7))
            }
        };
        let centroid = poly.iter().copied().sum::<Vec3>() / 7.0 + lift;
        let n = (poly[2] - poly[0]).cross(poly[6] - poly[0]).normalize_or(back);
        let n = if n.dot(back) < 0.0 { -n } else { n };
        if let Some(mut mesh) = meshes.get_mut(&m3.0) {
            let mut pos: Vec<[f32; 3]> = poly.iter().map(|v| v.to_array()).collect();
            pos.push(centroid.to_array());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, pos);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![n.to_array(); 8]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ankle_clamp_never_enters_torso_or_crosses_midline() {
        let hip_l = Vec3::new(-HIP_W, -0.04, 0.0);
        let bad_ankle = Vec3::new(0.5, 1.2, -2.0);
        let clamped = clamp_ankle_anatomical(hip_l, bad_ankle, -1.0);
        assert!(clamped.y <= hip_l.y - 0.16, "ankle y={} entered torso", clamped.y);
        assert!(clamped.x <= -0.03, "left ankle x={} crossed midline", clamped.x);
        assert!((clamped - hip_l).length() <= THIGH + SHIN);
    }

    #[test]
    fn wrist_clamp_keeps_arms_out_of_chest() {
        let sh_r = Vec3::new(SHOULDER_W, 0.36, 0.0);
        let inside_chest = Vec3::new(0.0, 0.20, 0.0);
        let clamped = clamp_wrist_anatomical(sh_r, inside_chest, 1.0);
        assert!(clamped.x >= 0.22, "right wrist x={} clipped into chest", clamped.x);
        assert!((clamped - sh_r).length() <= UPPER_ARM + FOREARM);
    }
}
