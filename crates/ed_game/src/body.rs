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
//!   `throttle > 0.0` and scales/brightens into an afterburner torch when
//!   `Shift` is held.

use crate::net::{PoseMode, RemoteProxy};
use crate::origin::WorldOrigin;
use crate::player::{MoveMode, Player};
use avian3d::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use std::collections::HashMap;

pub struct BodyPlugin;

impl Plugin for BodyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cosmetics>()
            .init_resource::<GaitStore>()
            .add_systems(PreStartup, init_body_assets)
            .add_systems(PostUpdate, animate_body.before(TransformSystems::Propagate));
    }
}

/// Links a body piece (part, wing, flame) to the actor entity it belongs to.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyOwner(pub Entity);

/// Deterministic index of a body part (used for stable addressing in pose replication).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PartIndex(pub u16);

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
pub enum J {
    Pelvis = 0,
    Chest = 1,
    Neck = 2,
    HipL = 3,
    HipR = 4,
    KneeL = 5,
    KneeR = 6,
    AnkleL = 7,
    AnkleR = 8,
    ShoulderL = 9,
    ShoulderR = 10,
    ElbowL = 11,
    ElbowR = 12,
    WristL = 13,
    WristR = 14,
}

/// Rigid frames (position + orientation).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum F {
    Pelvis = 0,
    Chest = 1,
    Head = 2,
    FootL = 3,
    FootR = 4,
    HandL = 5,
    HandR = 6,
}

#[derive(Clone, Copy, Debug)]
pub enum Spec {
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
pub struct BodyPart(pub Spec);

/// Deformable wingsuit membrane: 0 = left arm wing, 1 = right arm wing, 2 = leg wing (between the legs).
#[derive(Component)]
pub(crate) struct WingMesh(pub usize);

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
pub enum Accessory {
    Backpack,
}

#[derive(Default, Clone, Copy)]
pub struct Foot {
    pub planted: Vec3,
    pub from: Vec3,
    pub to: Vec3,
    pub t: f32,
    pub step_dur: f32,
    pub stepping: bool,
}

/// A 3D spring-damper state in pelvis-local coordinates.
#[derive(Default, Clone, Copy)]
pub struct SpringVec3 {
    pub pos: Vec3,
    pub vel: Vec3,
}

impl SpringVec3 {
    pub fn step(&mut self, target: Vec3, external_accel: Vec3, kp: f32, kd: f32, dt: f32) {
        let force = (target - self.pos) * kp - self.vel * kd + external_accel;
        self.vel += force * dt;
        self.pos += self.vel * dt;
    }
}

#[derive(Default, Clone, Copy)]
pub struct Gait {
    pub feet: [Foot; 2],
    pub init: bool,
    /// Which foot stepped last (0 or 1), so strides naturally alternate Left -> Right -> Left.
    pub last_step_foot: usize,
    /// Seconds until another step may start (prevents rapid step-spam jitter).
    pub step_cooldown: f32,
    /// Low-pass filtered horizontal velocity used for footstep placement.
    pub vs: Vec3,
    /// Hysteretic 'walking vs standing' flag so footstep rules don't flip at a threshold.
    pub moving: bool,
    /// Whether the previous frame was airborne (to detect touchdown).
    pub was_airborne: bool,
    pub phase: f32,
    /// Spine pitch/roll deflection in pelvis-local space (x = lateral roll, z = forward/back pitch).
    pub spine_bend: SpringVec3,
    /// Torso twist: x = chest yaw relative to the pelvis, y = pelvis yaw counter-twist.
    pub spine_twist: SpringVec3,
    /// Head pitch/yaw/roll deflection relative to chest.
    pub head_angles: SpringVec3,
    /// Pelvis-local ankle positions driven by spring-damper physics + anatomical clamps.
    pub local_ankles: [SpringVec3; 2],
    /// Pelvis-local wrist positions driven by spring-damper physics + anatomical clamps.
    pub local_wrists: [SpringVec3; 2],
}

#[derive(Resource, Default)]
pub struct GaitStore(pub HashMap<Entity, Gait>);

/// Shared mesh and material assets instantiated once for all character rigs.
#[derive(Resource, Clone)]
pub struct BodyAssets {
    pub cyl: Handle<Mesh>,
    pub ball: Handle<Mesh>,
    pub cube: Handle<Mesh>,
    pub dome: Handle<Mesh>,
    pub cone: Handle<Mesh>,

    pub shell: Handle<StandardMaterial>,
    pub shell_dark: Handle<StandardMaterial>,
    pub joint: Handle<StandardMaterial>,
    pub jacket: Handle<StandardMaterial>,
    pub jacket_dark: Handle<StandardMaterial>,
    pub pack: Handle<StandardMaterial>,
    pub strap: Handle<StandardMaterial>,
    pub shoe: Handle<StandardMaterial>,
    pub sole: Handle<StandardMaterial>,
    pub screen: Handle<StandardMaterial>,
    pub eye: Handle<StandardMaterial>,
    pub flame_outer: Handle<StandardMaterial>,
    pub flame_core: Handle<StandardMaterial>,
    pub brass: Handle<StandardMaterial>,
    pub canopy: Handle<StandardMaterial>,
    pub wing_mat: Handle<StandardMaterial>,
}

pub fn init_body_assets(
    mut commands: Commands,
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
    let canopy = mats.add(StandardMaterial {
        base_color: Color::srgb(0.96, 0.42, 0.09),
        perceptual_roughness: 0.9,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let wing_mat = mats.add(StandardMaterial {
        base_color: Color::srgb(0.10, 0.16, 0.34),
        perceptual_roughness: 0.6,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    let cyl = meshes.add(Cylinder::new(0.5, 1.0).mesh().resolution(20));
    let ball = meshes.add(Sphere::new(0.5).mesh().uv(20, 12));
    let cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let dome = meshes.add(Sphere::new(1.0).mesh().uv(24, 8));
    let cone = meshes.add(Cone::new(0.5, 1.0).mesh().resolution(16));

    commands.insert_resource(BodyAssets {
        cyl,
        ball,
        cube,
        dome,
        cone,
        shell,
        shell_dark,
        joint,
        jacket,
        jacket_dark,
        pack,
        strap,
        shoe,
        sole,
        screen,
        eye,
        flame_outer,
        flame_core,
        brass,
        canopy,
        wing_mat,
    });
}

/// Spawns a full set of owned body parts and wings for an actor entity.
pub fn spawn_body_parts(
    commands: &mut Commands,
    assets: &BodyAssets,
    meshes: &mut Assets<Mesh>,
    owner: Entity,
    cosmetics: &Cosmetics,
) {
    let q0 = Quat::IDENTITY;
    let qx = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2); // cylinder Y -> Z
    let qz = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2); // cylinder Y -> X
    let v = Vec3::new;
    use Spec::*;

    let parts: Vec<(Spec, &Handle<Mesh>, &Handle<StandardMaterial>)> = vec![
        // --- Legs: cylinder bones, ball joints ---
        (Limb { a: J::HipL, b: J::KneeL, r: 0.075 }, &assets.cyl, &assets.shell),
        (Limb { a: J::HipR, b: J::KneeR, r: 0.075 }, &assets.cyl, &assets.shell),
        (Limb { a: J::KneeL, b: J::AnkleL, r: 0.065 }, &assets.cyl, &assets.shell),
        (Limb { a: J::KneeR, b: J::AnkleR, r: 0.065 }, &assets.cyl, &assets.shell),
        (Ball { j: J::HipL, r: 0.1 }, &assets.ball, &assets.joint),
        (Ball { j: J::HipR, r: 0.1 }, &assets.ball, &assets.joint),
        (Ball { j: J::KneeL, r: 0.088 }, &assets.ball, &assets.joint),
        (Ball { j: J::KneeR, r: 0.088 }, &assets.ball, &assets.joint),
        (Ball { j: J::AnkleL, r: 0.07 }, &assets.ball, &assets.joint),
        (Ball { j: J::AnkleR, r: 0.07 }, &assets.ball, &assets.joint),
        // --- Feet: sneaker = upper + white sole + toe cap + thruster nozzle ---
        (Rigid { f: F::FootL, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, &assets.cube, &assets.shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, &assets.cube, &assets.shoe),
        (Rigid { f: F::FootL, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, &assets.cube, &assets.sole),
        (Rigid { f: F::FootR, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, &assets.cube, &assets.sole),
        (Rigid { f: F::FootL, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, &assets.cyl, &assets.shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, &assets.cyl, &assets.shoe),
        (Rigid { f: F::FootL, off: v(0.0, -0.095, -0.03), rot: q0, scale: v(0.09, 0.025, 0.09) }, &assets.cyl, &assets.joint),
        (Rigid { f: F::FootR, off: v(0.0, -0.095, -0.03), rot: q0, scale: v(0.09, 0.025, 0.09) }, &assets.cyl, &assets.joint),
        // --- Boot Rocket Jet Flames (Left & Right, Outer & Core) ---
        (JetFlame { foot_idx: 0, inner: false }, &assets.cone, &assets.flame_outer),
        (JetFlame { foot_idx: 0, inner: true }, &assets.cone, &assets.flame_core),
        (JetFlame { foot_idx: 1, inner: false }, &assets.cone, &assets.flame_outer),
        (JetFlame { foot_idx: 1, inner: true }, &assets.cone, &assets.flame_core),
        // --- Pelvis + waist + spine column ---
        (Rigid { f: F::Pelvis, off: v(0.0, 0.0, 0.0), rot: q0, scale: v(0.34, 0.16, 0.22) }, &assets.cube, &assets.joint),
        (Limb { a: J::Pelvis, b: J::Chest, r: 0.08 }, &assets.cyl, &assets.joint),
        // --- Torso: jacket block, hem, collar, zipper, pockets ---
        (Rigid { f: F::Chest, off: v(0.0, -0.2, 0.0), rot: q0, scale: v(0.46, 0.4, 0.28) }, &assets.cube, &assets.jacket),
        (Rigid { f: F::Chest, off: v(0.0, -0.41, 0.0), rot: q0, scale: v(0.44, 0.06, 0.26) }, &assets.cube, &assets.jacket_dark),
        (Rigid { f: F::Chest, off: v(0.0, 0.02, 0.0), rot: q0, scale: v(0.3, 0.07, 0.22) }, &assets.cube, &assets.jacket_dark),
        (Rigid { f: F::Chest, off: v(0.0, -0.2, -0.142), rot: q0, scale: v(0.025, 0.36, 0.01) }, &assets.cube, &assets.brass),
        (Rigid { f: F::Chest, off: v(-0.12, -0.3, -0.142), rot: q0, scale: v(0.12, 0.09, 0.01) }, &assets.cube, &assets.jacket_dark),
        (Rigid { f: F::Chest, off: v(0.12, -0.3, -0.142), rot: q0, scale: v(0.12, 0.09, 0.01) }, &assets.cube, &assets.jacket_dark),
        // --- Neck ---
        (Limb { a: J::Chest, b: J::Neck, r: 0.055 }, &assets.cyl, &assets.joint),
        // --- Arms: sleeve on the upper arm, shell forearm, ball joints ---
        (Limb { a: J::ShoulderL, b: J::ElbowL, r: 0.07 }, &assets.cyl, &assets.jacket),
        (Limb { a: J::ShoulderR, b: J::ElbowR, r: 0.07 }, &assets.cyl, &assets.jacket),
        (Limb { a: J::ElbowL, b: J::WristL, r: 0.05 }, &assets.cyl, &assets.shell),
        (Limb { a: J::ElbowR, b: J::WristR, r: 0.05 }, &assets.cyl, &assets.shell),
        (Ball { j: J::ShoulderL, r: 0.11 }, &assets.ball, &assets.jacket),
        (Ball { j: J::ShoulderR, r: 0.11 }, &assets.ball, &assets.jacket),
        (Ball { j: J::ElbowL, r: 0.07 }, &assets.ball, &assets.joint),
        (Ball { j: J::ElbowR, r: 0.07 }, &assets.ball, &assets.joint),
        (Ball { j: J::WristL, r: 0.05 }, &assets.ball, &assets.joint),
        (Ball { j: J::WristR, r: 0.05 }, &assets.ball, &assets.joint),
        // --- Hands: mitt + thumb ---
        (Rigid { f: F::HandL, off: v(0.0, -0.06, 0.0), rot: q0, scale: v(0.07, 0.11, 0.09) }, &assets.cube, &assets.shell),
        (Rigid { f: F::HandR, off: v(0.0, -0.06, 0.0), rot: q0, scale: v(0.07, 0.11, 0.09) }, &assets.cube, &assets.shell),
        (Rigid { f: F::HandL, off: v(0.03, -0.04, -0.05), rot: q0, scale: v(0.03, 0.06, 0.03) }, &assets.cube, &assets.shell_dark),
        (Rigid { f: F::HandR, off: v(-0.03, -0.04, -0.05), rot: q0, scale: v(0.03, 0.06, 0.03) }, &assets.cube, &assets.shell_dark),
        // --- CRT head: case, bezel, bulged screen, tube housing, dials, antenna ---
        (Rigid { f: F::Head, off: v(0.0, 0.0, 0.0), rot: q0, scale: v(0.42, 0.34, 0.3) }, &assets.cube, &assets.shell),
        (Rigid { f: F::Head, off: v(0.0, 0.0, -0.152), rot: q0, scale: v(0.38, 0.3, 0.01) }, &assets.cube, &assets.shell_dark),
        (Rigid { f: F::Head, off: v(-0.03, 0.005, -0.155), rot: q0, scale: v(0.29, 0.24, 0.05) }, &assets.ball, &assets.screen),
        (Rigid { f: F::Head, off: v(0.0, -0.01, 0.2), rot: q0, scale: v(0.3, 0.24, 0.14) }, &assets.cube, &assets.shell_dark),
        (Rigid { f: F::Head, off: v(0.155, 0.06, -0.155), rot: qx, scale: v(0.045, 0.03, 0.045) }, &assets.cyl, &assets.joint),
        (Rigid { f: F::Head, off: v(0.155, -0.04, -0.155), rot: qx, scale: v(0.04, 0.03, 0.04) }, &assets.cyl, &assets.brass),
        (Rigid { f: F::Head, off: v(0.155, -0.11, -0.153), rot: q0, scale: v(0.05, 0.02, 0.01) }, &assets.cube, &assets.joint),
        (Rigid { f: F::Head, off: v(0.09, 0.27, 0.0), rot: Quat::from_rotation_z(-0.35), scale: v(0.012, 0.22, 0.012) }, &assets.cyl, &assets.joint),
        (Rigid { f: F::Head, off: v(0.13, 0.38, 0.0), rot: q0, scale: v(0.035, 0.035, 0.035) }, &assets.ball, &assets.jacket),
        (Rigid { f: F::Head, off: v(0.0, -0.19, 0.0), rot: q0, scale: v(0.2, 0.04, 0.16) }, &assets.cube, &assets.joint),
        (Eye { off: v(-0.085, 0.02, -0.18) }, &assets.ball, &assets.eye),
        (Eye { off: v(0.035, 0.02, -0.18) }, &assets.ball, &assets.eye),
        (Canopy, &assets.dome, &assets.canopy),
    ];

    let mut part_index = 0u16;
    for (spec, mesh, m) in parts {
        commands.spawn((
            BodyPart(spec),
            BodyOwner(owner),
            PartIndex(part_index),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(m.clone()),
            Transform::default(),
            Visibility::Visible,
        ));
        part_index += 1;
    }

    let backpack: [(Spec, &Handle<Mesh>, &Handle<StandardMaterial>); 4] = [
        (Rigid { f: F::Chest, off: v(0.0, -0.18, 0.27), rot: q0, scale: v(0.4, 0.42, 0.26) }, &assets.cube, &assets.pack),
        (Rigid { f: F::Chest, off: v(0.0, -0.18, 0.27), rot: q0, scale: v(0.41, 0.06, 0.27) }, &assets.cube, &assets.strap),
        (Rigid { f: F::Chest, off: v(-0.13, -0.1, -0.005), rot: q0, scale: v(0.05, 0.42, 0.29) }, &assets.cube, &assets.strap),
        (Rigid { f: F::Chest, off: v(0.13, -0.1, -0.005), rot: q0, scale: v(0.05, 0.42, 0.29) }, &assets.cube, &assets.strap),
    ];
    for (spec, mesh, m) in backpack {
        commands.spawn((
            BodyPart(spec),
            BodyOwner(owner),
            PartIndex(part_index),
            Accessory::Backpack,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(m.clone()),
            Transform::default(),
            if cosmetics.backpack { Visibility::Visible } else { Visibility::Hidden },
        ));
        part_index += 1;
    }

    for k in 0..3 {
        commands.spawn((
            WingMesh(k),
            BodyOwner(owner),
            Mesh3d(meshes.add(wing_mesh_blank())),
            MeshMaterial3d(assets.wing_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            NoFrustumCulling,
        ));
    }
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

/// Comprehensive rig state input set. Both local player and remote proxies produce this.
#[derive(Clone, Copy, Debug)]
pub struct RigState {
    pub mode: MoveMode,
    pub yaw: f32,
    pub cam_pitch: f32,
    pub throttle: f32,
    pub wingsuit_on: bool,
    pub afterburner: bool,
    pub fwd_in: f32,
    pub strafe: f32,
    pub wing_arch: f32,
    pub wing_turn: f32,
    pub fall_spread: f32,
    pub fall_tuck: f32,
    pub jump_charge: f32,
    pub jump_push_timer: f32,
    pub crouch: bool,
    pub landing_recovery: f32,
    pub leg_steer: Vec3,
    pub intent_dir: Vec3,
    pub rot_err: Vec3,
    pub accel_smooth: Vec3,
    pub ground_normal: Vec3,
    pub foot_ground_hit: [Option<Vec3>; 2],
    pub first_person: bool,
    pub vel: Vec3,
}

impl Default for RigState {
    fn default() -> Self {
        Self {
            mode: MoveMode::Walk,
            yaw: 0.0,
            cam_pitch: 0.0,
            throttle: 0.0,
            wingsuit_on: false,
            afterburner: false,
            fwd_in: 0.0,
            strafe: 0.0,
            wing_arch: 0.0,
            wing_turn: 0.0,
            fall_spread: 0.0,
            fall_tuck: 0.0,
            jump_charge: 0.0,
            jump_push_timer: 0.0,
            crouch: false,
            landing_recovery: 0.0,
            leg_steer: Vec3::ZERO,
            intent_dir: Vec3::ZERO,
            rot_err: Vec3::ZERO,
            accel_smooth: Vec3::ZERO,
            ground_normal: Vec3::Y,
            foot_ground_hit: [None; 2],
            first_person: false,
            vel: Vec3::ZERO,
        }
    }
}

impl RigState {
    pub fn from_player(p: &Player, vel: Vec3) -> Self {
        Self {
            mode: p.mode,
            yaw: p.yaw,
            cam_pitch: p.cam_pitch,
            throttle: p.throttle,
            wingsuit_on: p.wingsuit_on,
            afterburner: p.afterburner,
            fwd_in: p.fwd_in,
            strafe: p.strafe,
            wing_arch: p.wing_arch,
            wing_turn: p.wing_turn,
            fall_spread: p.fall_spread,
            fall_tuck: p.fall_tuck,
            jump_charge: p.jump_charge,
            jump_push_timer: p.jump_push_timer,
            crouch: p.crouch,
            landing_recovery: p.landing_recovery,
            leg_steer: p.leg_steer,
            intent_dir: p.intent_dir,
            rot_err: p.rot_err,
            accel_smooth: p.accel_smooth,
            ground_normal: p.ground_normal,
            foot_ground_hit: p.foot_ground_hit,
            first_person: p.first_person,
            vel,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlameMode {
    ParticlePlume,
    Cone,
}

/// The output of solving one actor's procedural pose.
pub struct Pose {
    pub pelvis_c: Vec3,
    pub base: Quat,
    pub body_up: Vec3,
    pub blink: f32,
    pub flying: bool,
    pub joints: [Vec3; 15],
    pub frames: [(Vec3, Quat); 7],
    pub wing_polys: [([Vec3; 7], Vec3); 3],

    // Foot feedback state
    pub foot_world_pos: [Vec3; 2],
    pub foot_up_dir: [Vec3; 2],
    pub foot_planted: [bool; 2],

    // Boot nozzle state
    pub nozzle_pos: [Vec3; 2],
    pub nozzle_dir: [Vec3; 2],
    pub nozzle_fwd: [Vec3; 2],
    pub throttle: f32,
}

impl Pose {
    pub fn joint(&self, j: J) -> Vec3 {
        self.joints[j as usize]
    }

    pub fn frame(&self, f: F) -> (Vec3, Quat) {
        self.frames[f as usize]
    }
}

/// Solves procedural IK and joint kinematics for an actor given its RigState.
pub fn solve_rig(
    state: &RigState,
    gait: &mut Gait,
    pt_translation: Vec3,
    pt_rotation: Quat,
    up: Vec3,
    spatial: &SpatialQuery,
    filter_entity: Option<Entity>,
    dt: f32,
    t_sec: f32,
) -> Pose {
    let base = if pt_rotation == Quat::IDENTITY && state.yaw != 0.0 {
        Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(state.yaw)
    } else {
        pt_rotation
    };
    let inv_base = base.inverse();
    let yaw_only = Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(state.yaw);
    let right_yaw = yaw_only * Vec3::X;
    let fwd_yaw = yaw_only * Vec3::NEG_Z;

    let v = state.vel;
    let v_flat = v - up * v.dot(up);
    let speed = v_flat.length();
    let airborne = state.mode != MoveMode::Walk;

    let local_accel = inv_base * state.accel_smooth;
    let local_vel = inv_base * v;
    let local_wind = -local_vel;
    let local_err = inv_base * state.rot_err;
    let il = inv_base * state.intent_dir;
    let wing_amp = if matches!(state.mode, MoveMode::Skydive | MoveMode::Fall) {
        (v.length() / 70.0).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let rot_effort = local_err.length().min(2.5);

    let leg = THIGH + SHIN;
    let pelvis_c = pt_translation;
    let pelvis_twist = Quat::from_rotation_y(gait.spine_twist.pos.y);
    let hips_local = [
        pelvis_twist * Vec3::new(-HIP_W, -0.04, 0.0),
        pelvis_twist * Vec3::new(HIP_W, -0.04, 0.0),
    ];

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

    let ground_at = |world_pt: Vec3| -> Option<Vec3> {
        let filter = match filter_entity {
            Some(e) => SpatialQueryFilter::from_excluded_entities([e]),
            None => SpatialQueryFilter::default(),
        };
        spatial
            .cast_ray(world_pt + up * 0.9, Dir3::new(-up).ok()?, 2.5, true, &filter)
            .map(|h| world_pt + up * (0.9 - h.distance))
    };

    let body_up = base * Vec3::Y;
    let mut target_ankles_local = [Vec3::ZERO; 2];
    let mut foot_planted = [false; 2];

    if airborne {
        gait.was_airborne = true;
        gait.moving = false;
        for i in 0..2 {
            let side = if i == 0 { -1.0f32 } else { 1.0f32 };
            gait.feet[i].stepping = false;
            gait.feet[i].planted = pelvis_c + base * (gait.local_ankles[i].pos - Vec3::Y * ANKLE_H);
            foot_planted[i] = false;

            target_ankles_local[i] = match state.mode {
                MoveMode::RocketShoes => {
                    let stance_w = if state.afterburner && state.fwd_in.abs() > 0.1 {
                        HIP_W * 0.80
                    } else {
                        HIP_W + 0.03
                    };
                    let tuck = rot_effort * 0.07;
                    let knee = Vec3::new(il.x, 0.0, il.z).length().min(1.0) * 0.08;
                    Vec3::new(side * (stance_w + rot_effort * 0.04), -(leg * 0.96) + tuck + knee, 0.02)
                        + state.leg_steer
                }
                MoveMode::Skydive => {
                    let spread = 0.09 - 0.04 * (-state.wing_arch).max(0.0);
                    let knee_bend = 0.03 + 0.12 * state.wing_arch.max(0.0);
                    let suit = Vec3::new(
                        side * (HIP_W + spread),
                        -(leg * (0.94 - knee_bend)),
                        0.12 + knee_bend * 0.4,
                    );
                    let eagle = Vec3::new(side * (HIP_W + 0.30), -(leg * 0.82), 0.16);
                    let ball = Vec3::new(side * (HIP_W + 0.02), -(leg * 0.42), -leg * 0.38);
                    suit.lerp(eagle, state.fall_spread).lerp(ball, state.fall_tuck)
                }
                MoveMode::Parachute => Vec3::new(side * (HIP_W + 0.03), -(leg * 0.90), 0.06),
                MoveMode::Fall => Vec3::new(side * (HIP_W + 0.05), -(leg * 0.84), 0.08),
                MoveMode::Walk => Vec3::new(side * HIP_W, -(leg * 0.92), 0.0),
            };
        }
    } else {
        let k_v = 1.0 - (-9.0 * dt).exp();
        gait.vs = gait.vs.lerp(v_flat, k_v);
        let vs = gait.vs;
        let vs_speed = vs.length();
        gait.moving = if gait.moving { vs_speed > 0.2 } else { vs_speed > 0.45 };
        let travelling = gait.moving;
        let dir = vs.normalize_or_zero();

        let s_half = (0.10 + 0.11 * vs_speed).clamp(0.10, 0.46);
        let step_dur = (0.27 - 0.04 * vs_speed).clamp(0.13, 0.24);
        let sliding = state.landing_recovery > 0.1;
        gait.step_cooldown = (gait.step_cooldown - dt).max(0.0);

        let hip_ground = |side: f32| pelvis_c + right_yaw * (side * HIP_W);
        let hips_world_est = |i: usize| pelvis_c + base * hips_local[i];

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
            let mut need = [0.0f32; 2];
            for i in 0..2 {
                let side = if i == 0 { -1.0f32 } else { 1.0f32 };
                let rel = (gait.feet[i].planted - hip_ground(side)).reject_from_normalized(up);
                let rel_x = rel.dot(right_yaw);
                let rel_z = rel.dot(fwd_yaw);
                let inward = -side * rel_x;
                let comfort_lat = if inward > 0.0 { inward / 0.10 } else { -inward / 0.30 };
                let reach = (hips_world_est(i) - gait.feet[i].planted).length() / (leg * 0.97);
                let comfort = comfort_lat
                    .max(reach)
                    .max(rel_z.abs() / if travelling { 0.55 } else { 0.30 });
                need[i] = comfort.max(if travelling {
                    let behind = -rel.dot(dir);
                    let lateral = (rel - dir * rel.dot(dir)).length();
                    (behind / s_half).max(lateral / 0.30)
                } else {
                    rel.length() / 0.34
                });
            }
            let pick = if need[0] >= need[1] { 0 } else { 1 };
            let thresh = if travelling { 1.0 } else { 1.25 };
            if need[pick] > thresh {
                let side = if pick == 0 { -1.0f32 } else { 1.0f32 };
                let hip_pred = hip_ground(side) + vs * (step_dur * 0.9);
                let target_xz = if travelling {
                    hip_pred + dir * (s_half * 0.9)
                } else {
                    hip_ground(side)
                };
                let landing_pt = ground_at(target_xz)
                    .or(state.foot_ground_hit[pick])
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
                foot_planted[i] = false;
                let s_t = f.t.clamp(0.0, 1.0);
                let s = s_t * s_t * (3.0 - 2.0 * s_t);
                let step_span = (f.to - f.from).length();
                let lift_h = (0.09 + step_span * 0.22).clamp(0.08, 0.28);
                f.from.lerp(f.to, s) + up * ((s_t * std::f32::consts::PI).sin() * lift_h)
            } else {
                foot_planted[i] = true;
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

    let spine_target = match state.mode {
        MoveMode::Walk => {
            let run_lean_fwd = -(speed / 7.5).min(1.0) * 0.22;
            let squat_lean = if state.jump_charge > 0.05 || state.crouch {
                -0.28 * state.jump_charge.max(if state.crouch { 0.8 } else { 0.0 })
            } else {
                0.0
            };
            let turn_roll = state.strafe * 0.14;
            Vec3::new(turn_roll, 0.0, run_lean_fwd + squat_lean)
        }
        MoveMode::RocketShoes => {
            Vec3::new((il.x * 0.45).clamp(-0.34, 0.34), 0.0, (il.z * 0.45).clamp(-0.42, 0.38))
        }
        MoveMode::Skydive => {
            let arch = state.wing_arch * 0.24 + 0.14;
            let suit = Vec3::new(-state.wing_turn * 0.22, 0.0, arch);
            suit.lerp(Vec3::new(0.0, 0.0, 0.30), state.fall_spread)
                .lerp(Vec3::new(0.0, 0.0, -0.35), state.fall_tuck)
        }
        _ => Vec3::new(state.strafe * 0.10, 0.0, 0.06),
    };
    let spine_inertial = Vec3::new(
        (-local_accel.x * 0.018).clamp(-0.45, 0.45) + (t_sec * 17.0).sin() * 0.05 * wing_amp,
        0.0,
        (-local_accel.z * 0.020).clamp(-0.45, 0.45) + (t_sec * 13.0).cos() * 0.05 * wing_amp,
    );
    gait.spine_bend.step(spine_target, spine_inertial * 20.0, 85.0, 13.0, dt);
    gait.spine_bend.pos.x = gait.spine_bend.pos.x.clamp(-0.34, 0.34);
    gait.spine_bend.pos.z = gait.spine_bend.pos.z.clamp(-0.42, 0.38);

    let spine_dir_local = Vec3::new(gait.spine_bend.pos.x, 1.0, gait.spine_bend.pos.z).normalize();
    let chest_local = spine_dir_local * SPINE;
    let chest = pelvis_c + base * chest_local;

    let twist_target = Vec3::new(
        (local_err.y * 0.55 + state.strafe * if airborne { 0.22 } else { 0.0 }).clamp(-0.75, 0.75),
        (-local_err.y * 0.28).clamp(-0.4, 0.4),
        0.0,
    );
    gait.spine_twist.step(twist_target, Vec3::ZERO, 90.0, 12.0, dt);
    gait.spine_twist.pos.x = gait.spine_twist.pos.x.clamp(-0.8, 0.8);
    gait.spine_twist.pos.y = gait.spine_twist.pos.y.clamp(-0.45, 0.45);
    let chest_twist = Quat::from_rotation_y(gait.spine_twist.pos.x);
    let chest_rot = base * Quat::from_rotation_arc(Vec3::Y, spine_dir_local) * chest_twist;
    let chest_up = chest_rot * Vec3::Y;

    let head_target = match state.mode {
        MoveMode::Skydive => Vec3::new(
            0.60 + state.wing_arch * 0.15,
            state.wing_turn * 0.28,
            state.wing_turn * 0.15,
        ),
        MoveMode::RocketShoes => {
            let pitch_look = if state.fwd_in > 0.1 {
                0.55
            } else if state.fwd_in < -0.1 {
                -0.25
            } else {
                0.15
            };
            Vec3::new(pitch_look, -state.strafe * 0.25, -state.strafe * 0.15)
        }
        MoveMode::Fall => Vec3::new(-0.42, 0.0, (t_sec * 1.2).sin() * 0.04),
        _ => Vec3::new(
            (state.cam_pitch * 0.35).clamp(-0.4, 0.4) + (t_sec * 0.7).sin() * 0.025,
            -state.strafe * 0.15,
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

    let leg_wind_drag = (local_wind * 0.007).clamp_length_max(0.40);
    let leg_inertia = (-local_accel * 0.007).clamp_length_max(0.40);
    let mut ankles_world = [Vec3::ZERO; 2];
    let mut knees_world = [Vec3::ZERO; 2];
    let mut hips_world = [Vec3::ZERO; 2];
    let mut foot_world_pos = [Vec3::ZERO; 2];
    let mut foot_up_dir = [Vec3::Y; 2];

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

        let planted_g = if !airborne && !gait.feet[i].stepping {
            gait.feet[i].planted
        } else {
            ankles_world[i] - body_up * ANKLE_H
        };
        foot_world_pos[i] = planted_g;
        let leg_up = (hips_world[i] - ankles_world[i]).normalize_or(body_up);
        foot_up_dir[i] = leg_up;
    }

    let shoulders_local = [
        chest_local + chest_twist * Vec3::new(-SHOULDER_W, -0.06, 0.0),
        chest_local + chest_twist * Vec3::new(SHOULDER_W, -0.06, 0.0),
    ];
    let foot_z_diff = (gait.local_ankles[0].pos.z - gait.local_ankles[1].pos.z).clamp(-0.45, 0.45);
    let arm_wind_buffet = Vec3::new(
        (t_sec * 19.0).sin() * 0.04,
        (t_sec * 23.0).cos() * 0.04,
        (t_sec * 17.0).sin() * 0.04,
    ) * (local_wind.length() / 35.0).clamp(0.0, 1.5)
        * if matches!(state.mode, MoveMode::Skydive | MoveMode::Fall) { 2.5 } else { 1.0 };
    let arm_inertia = (-local_accel * 0.011 + local_wind * 0.005).clamp_length_max(0.55);

    let mut shoulders_world = [Vec3::ZERO; 2];
    let mut elbows_world = [Vec3::ZERO; 2];
    let mut wrists_world = [Vec3::ZERO; 2];

    for i in 0..2 {
        let side = if i == 0 { -1.0f32 } else { 1.0f32 };
        let sh = shoulders_local[i];
        let target_wrist = match state.mode {
            MoveMode::RocketShoes => {
                if state.fwd_in > 0.1 {
                    let reach = if state.afterburner { 0.34 } else { 0.26 };
                    sh + Vec3::new(side * 0.16, -0.18, -reach)
                } else if state.fwd_in < -0.1 {
                    sh + Vec3::new(side * 0.36, -0.16, 0.24)
                } else if state.strafe.abs() > 0.1 {
                    let asym = side * state.strafe * 0.14;
                    sh + Vec3::new(side * 0.34, -0.22 + asym, -0.06)
                } else if state.afterburner {
                    sh + Vec3::new(side * 0.08, -0.48, 0.05)
                } else {
                    sh + Vec3::new(side * 0.25, -0.36, 0.04)
                }
            }
            MoveMode::Skydive => {
                let dive = (-state.wing_arch).max(0.0);
                let sweep_back = dive * 0.24;
                let tuck_in = dive * 0.14;
                let flare_z = -state.wing_arch.max(0.0) * 0.20;
                let bank_asym = (-side) * state.wing_turn * 0.16;
                let suit = Vec3::new(
                    side * (0.50 - tuck_in),
                    -0.06 - sweep_back + bank_asym,
                    0.06 + sweep_back * 0.5 + flare_z,
                );
                let eagle = Vec3::new(side * 0.58, 0.18, 0.08);
                let ball = Vec3::new(side * 0.16, -0.42, -0.30);
                sh + suit.lerp(eagle, state.fall_spread).lerp(ball, state.fall_tuck)
            }
            MoveMode::Fall => sh + Vec3::new(side * 0.44, -0.10 + (-side) * state.wing_turn * 0.16, -0.05),
            MoveMode::Parachute => sh + Vec3::new(side * 0.08, 0.46, -0.08),
            MoveMode::Walk => {
                if state.jump_charge > 0.05 || state.crouch {
                    let c = state.jump_charge.max(if state.crouch { 0.7 } else { 0.0 });
                    sh + Vec3::new(side * 0.12, -0.38, 0.28 * c)
                } else if state.jump_push_timer > 0.0 {
                    sh + Vec3::new(side * 0.14, -0.15, -0.28)
                } else {
                    let swing_z = side * foot_z_diff * 0.95;
                    sh + Vec3::new(side * 0.08, -0.46 + swing_z.abs() * 0.08, swing_z)
                }
            }
        };

        let fling = if state.mode == MoveMode::RocketShoes {
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

    let joints = [
        pelvis_c,           // J::Pelvis = 0
        chest,              // J::Chest = 1
        neck,               // J::Neck = 2
        hips_world[0],      // J::HipL = 3
        hips_world[1],      // J::HipR = 4
        knees_world[0],     // J::KneeL = 5
        knees_world[1],     // J::KneeR = 6
        ankles_world[0],    // J::AnkleL = 7
        ankles_world[1],    // J::AnkleR = 8
        shoulders_world[0], // J::ShoulderL = 9
        shoulders_world[1], // J::ShoulderR = 10
        elbows_world[0],    // J::ElbowL = 11
        elbows_world[1],    // J::ElbowR = 12
        wrists_world[0],    // J::WristL = 13
        wrists_world[1],    // J::WristR = 14
    ];

    let hand_rot = |e: Vec3, w: Vec3| {
        Quat::from_rotation_arc(-body_up, (w - e).normalize_or(-body_up)) * base
    };
    let foot_rot = |h: Vec3, a: Vec3| {
        if !airborne {
            let n = state.ground_normal.normalize_or(up);
            Quat::from_rotation_arc(Vec3::Y, n) * Quat::from_rotation_y(state.yaw)
        } else {
            Quat::from_rotation_arc(-body_up, (a - h).normalize_or(-body_up)) * base
        }
    };

    let frames = [
        (pelvis_c, base * pelvis_twist),                          // F::Pelvis = 0
        (chest, chest_rot),                                        // F::Chest = 1
        (head_c, head_rot),                                        // F::Head = 2
        (ankles_world[0], foot_rot(hips_world[0], ankles_world[0])), // F::FootL = 3
        (ankles_world[1], foot_rot(hips_world[1], ankles_world[1])), // F::FootR = 4
        (wrists_world[0], hand_rot(elbows_world[0], wrists_world[0])), // F::HandL = 5
        (wrists_world[1], hand_rot(elbows_world[1], wrists_world[1])), // F::HandR = 6
    ];

    let blink = if (t_sec % 3.7) < 0.12 { 0.15 } else { 1.0 };
    let flying = state.wingsuit_on
        && !matches!(state.mode, MoveMode::Walk | MoveMode::Parachute)
        && !state.first_person;

    let back = base * Vec3::Z;
    let bulge = 0.05 + wing_amp * 0.03 + (0.012 + 0.02 * wing_amp) * (t_sec * 38.0).sin();
    let wing_polys = [
        {
            let wrist = wrists_world[0];
            let ankle = ankles_world[0];
            let trail = (wrist + ankle) * 0.5 + back * (bulge * 0.6);
            (
                [shoulders_world[0], elbows_world[0], wrist, trail, ankle, knees_world[0], hips_world[0]],
                back * bulge,
            )
        },
        {
            let wrist = wrists_world[1];
            let ankle = ankles_world[1];
            let trail = (wrist + ankle) * 0.5 + back * (bulge * 0.6);
            (
                [shoulders_world[1], elbows_world[1], wrist, trail, ankle, knees_world[1], hips_world[1]],
                back * bulge,
            )
        },
        {
            let crotch = (hips_world[0] + hips_world[1]) * 0.5 - body_up * 0.04;
            (
                [
                    hips_world[0],
                    knees_world[0],
                    ankles_world[0],
                    ankles_world[1],
                    knees_world[1],
                    hips_world[1],
                    crotch,
                ],
                back * (bulge * 0.7),
            )
        },
    ];

    let (p0_fl, q_fl) = frames[F::FootL as usize];
    let (p0_fr, q_fr) = frames[F::FootR as usize];
    let nozzle_pos = [
        p0_fl + q_fl * Vec3::new(0.0, -0.10, -0.03),
        p0_fr + q_fr * Vec3::new(0.0, -0.10, -0.03),
    ];
    let nozzle_dir = [q_fl * Vec3::NEG_Y, q_fr * Vec3::NEG_Y];
    let nozzle_fwd = [q_fl * Vec3::NEG_Z, q_fr * Vec3::NEG_Z];

    Pose {
        pelvis_c,
        base,
        body_up,
        blink,
        flying,
        joints,
        frames,
        wing_polys,
        foot_world_pos,
        foot_up_dir,
        foot_planted,
        nozzle_pos,
        nozzle_dir,
        nozzle_fwd,
        throttle: state.throttle,
    }
}

pub fn apply_pose_to_parts(
    _owner: Entity,
    pose: &Pose,
    parts: &mut [(&BodyPart, &PartIndex, Option<&Accessory>, &mut Transform, &mut Visibility)],
    flame_mode: FlameMode,
    mode: MoveMode,
    first_person: bool,
    cosmetics: &Cosmetics,
    t_sec: f32,
    replicated_parts: Option<&HashMap<u16, (Vec3, Quat)>>,
) {
    for (bp, p_idx, acc, tf, vis) in parts.iter_mut() {
        **vis = match acc {
            Some(Accessory::Backpack) if !cosmetics.backpack => Visibility::Hidden,
            _ => Visibility::Visible,
        };

        if let Some(rep) = replicated_parts {
            if let Some(&(p_local, q_local)) = rep.get(&p_idx.0) {
                tf.translation = pose.pelvis_c + pose.base * p_local;
                tf.rotation = pose.base * q_local;
                continue;
            }
        }

        match bp.0 {
            Spec::Limb { a, b, r } => **tf = limb(pose.joint(a), pose.joint(b), r),
            Spec::Ball { j, r } => {
                **tf = Transform::from_translation(pose.joint(j)).with_scale(Vec3::splat(r * 2.0));
            }
            Spec::Rigid { f, off, rot, scale } => {
                let (p0, q) = pose.frame(f);
                **tf = Transform {
                    translation: p0 + q * off,
                    rotation: q * rot,
                    scale,
                };
                if f == F::Head && first_person {
                    **vis = Visibility::Hidden;
                }
            }
            Spec::Eye { off } => {
                let (p0, q) = pose.frame(F::Head);
                **tf = Transform {
                    translation: p0 + q * off,
                    rotation: q,
                    scale: Vec3::new(0.055, 0.075 * pose.blink, 0.02),
                };
                if first_person {
                    **vis = Visibility::Hidden;
                }
            }
            Spec::JetFlame { foot_idx, inner } => {
                let f_frame = if foot_idx == 0 { F::FootL } else { F::FootR };
                match flame_mode {
                    FlameMode::ParticlePlume => {
                        **vis = Visibility::Hidden;
                    }
                    FlameMode::Cone => {
                        if pose.throttle <= 0.01 {
                            **vis = Visibility::Hidden;
                        } else {
                            let (p0, q) = pose.frame(f_frame);
                            let phase = t_sec * 62.0 + (foot_idx as f32) * 1.9;
                            let flicker = 0.88 + 0.24 * phase.sin() * (phase * 1.7).cos();
                            let len = (0.28 + pose.throttle * 0.34) * flicker * if inner { 0.62 } else { 1.0 };
                            let width = (0.09 + pose.throttle.min(2.5) * 0.035) * if inner { 0.58 } else { 1.0 };
                            let nozzle_local = Vec3::new(0.0, -0.10 - len * 0.5, -0.03);
                            let flip = Quat::from_rotation_x(std::f32::consts::PI);
                            **tf = Transform {
                                translation: p0 + q * nozzle_local,
                                rotation: q * flip,
                                scale: Vec3::new(width, len, width),
                            };
                        }
                    }
                }
            }
            Spec::Canopy => {
                if mode == MoveMode::Parachute {
                    **tf = Transform::from_translation(pose.pelvis_c + pose.body_up * 6.5)
                        .with_rotation(pose.base)
                        .with_scale(Vec3::new(4.5, 1.6, 2.6));
                } else {
                    **vis = Visibility::Hidden;
                }
            }
        }
    }
}

pub fn apply_pose_to_wings(
    _owner: Entity,
    pose: &Pose,
    wings: &mut [(&WingMesh, &Mesh3d, &mut Visibility)],
    meshes: &mut Assets<Mesh>,
) {
    let back = pose.base * Vec3::Z;
    for (w, m3, vis) in wings.iter_mut() {
        if !pose.flying {
            **vis = Visibility::Hidden;
            continue;
        }
        **vis = Visibility::Visible;
        let (poly, lift) = pose.wing_polys[w.0.min(2)];
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

#[allow(clippy::type_complexity)]
fn animate_body(
    time: Res<Time>,
    origin: Res<WorldOrigin>,
    spatial: SpatialQuery,
    mut gait_store: ResMut<GaitStore>,
    mut players: Query<(Entity, &Transform, &LinearVelocity, &mut Player), Without<BodyPart>>,
    remote_proxies: Query<(Entity, &Transform, &RemoteProxy), (Without<Player>, Without<BodyPart>)>,
    cosmetics: Res<Cosmetics>,
    mut parts: Query<(
        &BodyPart,
        &BodyOwner,
        &PartIndex,
        Option<&Accessory>,
        &mut Transform,
        &mut Visibility,
    )>,
    mut wings: Query<(&WingMesh, &BodyOwner, &Mesh3d, &mut Visibility), Without<BodyPart>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut nozzles: Option<ResMut<crate::boot_plume::BootNozzles>>,
) {
    let dt = time.delta_secs().clamp(1e-4, 0.05);
    let t_sec = time.elapsed_secs();
    if let Some(n) = nozzles.as_deref_mut() {
        n.remote.clear();
    }

    // Group part queries by owner
    let mut parts_by_owner: HashMap<
        Entity,
        Vec<(&BodyPart, &PartIndex, Option<&Accessory>, &mut Transform, &mut Visibility)>,
    > = HashMap::new();
    for (bp, owner, idx, acc, tf, vis) in &mut parts {
        parts_by_owner
            .entry(owner.0)
            .or_default()
            .push((bp, idx, acc, tf.into_inner(), vis.into_inner()));
    }

    let mut wings_by_owner: HashMap<Entity, Vec<(&WingMesh, &Mesh3d, &mut Visibility)>> =
        HashMap::new();
    for (w, owner, m3, vis) in &mut wings {
        wings_by_owner
            .entry(owner.0)
            .or_default()
            .push((w, m3, vis.into_inner()));
    }

    // 1. Local players (usually exactly 1)
    for (pe, pt, vel, mut p) in &mut players {
        let up = origin.up_at(pt.translation);
        let gait = gait_store.0.entry(pe).or_default();
        let state = RigState::from_player(&p, vel.0);
        let pose = solve_rig(
            &state,
            gait,
            pt.translation,
            pt.rotation,
            up,
            &spatial,
            Some(pe),
            dt,
            t_sec,
        );

        p.foot_world_pos = pose.foot_world_pos;
        p.foot_up_dir = pose.foot_up_dir;
        p.foot_planted = pose.foot_planted;

        if let Some(n) = nozzles.as_deref_mut() {
            n.pos = pose.nozzle_pos;
            n.dir = pose.nozzle_dir;
            n.fwd = pose.nozzle_fwd;
            n.throttle = pose.throttle;
        }

        let flame_mode = if nozzles.is_some() {
            FlameMode::ParticlePlume
        } else {
            FlameMode::Cone
        };

        if let Some(my_parts) = parts_by_owner.get_mut(&pe) {
            apply_pose_to_parts(
                pe,
                &pose,
                my_parts.as_mut_slice(),
                flame_mode,
                p.mode,
                p.first_person,
                &cosmetics,
                t_sec,
                None,
            );

            if std::env::var_os("EW_BODY_DUMP").is_some() {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("/tmp/body_dump.txt")
                {
                    for (_bp, p_idx, _acc, tf, vis) in my_parts.iter() {
                        let _ = writeln!(
                            f,
                            "{}: t={:?} r={:?} s={:?} v={:?}",
                            p_idx.0, tf.translation, tf.rotation, tf.scale, vis
                        );
                    }
                }
            }
        }

        if let Some(my_wings) = wings_by_owner.get_mut(&pe) {
            apply_pose_to_wings(pe, &pose, my_wings.as_mut_slice(), &mut meshes);
        }
    }

    // 2. Remote player proxies
    for (re, rt, proxy) in &remote_proxies {
        let up = origin.up_at(rt.translation);
        let gait = gait_store.0.entry(re).or_default();
        let rep_opt = if proxy.pose_mode == PoseMode::Replicated {
            Some(&proxy.replicated_parts)
        } else {
            None
        };
        let pose = solve_rig(
            &proxy.state,
            gait,
            rt.translation,
            proxy.quat,
            up,
            &spatial,
            None,
            dt,
            t_sec,
        );

        if let Some(n) = nozzles.as_deref_mut() {
            n.remote.push(crate::boot_plume::RemoteNozzles {
                id: proxy.id,
                pos: pose.nozzle_pos,
                dir: pose.nozzle_dir,
                fwd: pose.nozzle_fwd,
                throttle: pose.throttle,
                velocity: proxy.state.vel,
            });
        }

        let remote_cosmetics = Cosmetics { backpack: false };
        if let Some(my_parts) = parts_by_owner.get_mut(&re) {
            apply_pose_to_parts(
                re,
                &pose,
                my_parts.as_mut_slice(),
                FlameMode::Cone,
                proxy.state.mode,
                false,
                &remote_cosmetics,
                t_sec,
                rep_opt,
            );
        }

        if let Some(my_wings) = wings_by_owner.get_mut(&re) {
            apply_pose_to_wings(re, &pose, my_wings.as_mut_slice(), &mut meshes);
        }
    }
    if let Some(n) = nozzles.as_deref_mut() {
        n.remote.sort_by_key(|r| r.id);
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

    #[test]
    fn part_index_stability_and_count() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default());
        let owner = app.world_mut().spawn_empty().id();

        // Create dummy assets
        let mut meshes = Assets::<Mesh>::default();
        let mut materials = Assets::<StandardMaterial>::default();
        let dummy_mesh = meshes.add(Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default()));
        let dummy_mat = materials.add(StandardMaterial::default());
        let assets = BodyAssets {
            cyl: dummy_mesh.clone(),
            ball: dummy_mesh.clone(),
            cube: dummy_mesh.clone(),
            dome: dummy_mesh.clone(),
            cone: dummy_mesh.clone(),
            shell: dummy_mat.clone(),
            shell_dark: dummy_mat.clone(),
            joint: dummy_mat.clone(),
            jacket: dummy_mat.clone(),
            jacket_dark: dummy_mat.clone(),
            pack: dummy_mat.clone(),
            strap: dummy_mat.clone(),
            shoe: dummy_mat.clone(),
            sole: dummy_mat.clone(),
            screen: dummy_mat.clone(),
            eye: dummy_mat.clone(),
            flame_outer: dummy_mat.clone(),
            flame_core: dummy_mat.clone(),
            brass: dummy_mat.clone(),
            canopy: dummy_mat.clone(),
            wing_mat: dummy_mat.clone(),
        };

        let mut commands = app.world_mut().commands();
        spawn_body_parts(&mut commands, &assets, &mut meshes, owner, &Cosmetics { backpack: true });
        app.update();

        let mut part_indices: Vec<u16> = Vec::new();
        let mut part_owners: Vec<Entity> = Vec::new();
        let mut wing_count = 0;

        for (owner_comp, part_idx) in app.world_mut().query::<(&BodyOwner, &PartIndex)>().iter(app.world()) {
            part_owners.push(owner_comp.0);
            part_indices.push(part_idx.0);
        }
        for (owner_comp, _wing) in app.world_mut().query::<(&BodyOwner, &WingMesh)>().iter(app.world()) {
            assert_eq!(owner_comp.0, owner);
            wing_count += 1;
        }

        // 58 standard parts + 4 backpack parts = 62 indexed parts
        assert_eq!(part_indices.len(), 62, "must spawn exactly 62 indexed parts");
        assert_eq!(wing_count, 3, "must spawn exactly 3 wing meshes");
        assert!(part_owners.iter().all(|&o| o == owner), "all parts must belong to owner");

        // Part indices must be strictly 0..62 in sequential order
        part_indices.sort_unstable();
        let expected: Vec<u16> = (0..62).collect();
        assert_eq!(part_indices, expected, "part indices must be stable 0..62");
    }

    #[test]
    fn rig_state_from_player_matches() {
        let mut p = Player::idle(1.7, MoveMode::Walk);
        p.yaw = 1.23;
        p.cam_pitch = -0.45;
        p.throttle = 0.8;
        p.wingsuit_on = true;
        p.afterburner = true;
        p.fwd_in = 1.0;
        p.strafe = -0.5;
        p.wing_arch = 0.2;
        p.wing_turn = -0.1;
        p.fall_spread = 0.4;
        p.fall_tuck = 0.1;
        p.jump_charge = 0.6;
        p.jump_push_timer = 0.05;
        p.crouch = true;
        p.landing_recovery = 0.3;
        p.leg_steer = Vec3::new(0.1, 0.2, 0.3);
        p.intent_dir = Vec3::new(0.4, 0.5, 0.6);
        p.rot_err = Vec3::new(0.7, 0.8, 0.9);
        p.accel_smooth = Vec3::new(1.0, 1.1, 1.2);
        p.first_person = false;

        let vel = Vec3::new(2.0, 3.0, 4.0);
        let st = RigState::from_player(&p, vel);

        assert_eq!(st.mode, MoveMode::Walk);
        assert_eq!(st.yaw, 1.23);
        assert_eq!(st.cam_pitch, -0.45);
        assert_eq!(st.throttle, 0.8);
        assert!(st.wingsuit_on);
        assert!(st.afterburner);
        assert_eq!(st.fwd_in, 1.0);
        assert_eq!(st.strafe, -0.5);
        assert_eq!(st.wing_arch, 0.2);
        assert_eq!(st.wing_turn, -0.1);
        assert_eq!(st.fall_spread, 0.4);
        assert_eq!(st.fall_tuck, 0.1);
        assert_eq!(st.jump_charge, 0.6);
        assert_eq!(st.jump_push_timer, 0.05);
        assert!(st.crouch);
        assert_eq!(st.landing_recovery, 0.3);
        assert_eq!(st.leg_steer, Vec3::new(0.1, 0.2, 0.3));
        assert_eq!(st.intent_dir, Vec3::new(0.4, 0.5, 0.6));
        assert_eq!(st.rot_err, Vec3::new(0.7, 0.8, 0.9));
        assert_eq!(st.accel_smooth, Vec3::new(1.0, 1.1, 1.2));
        assert_eq!(st.vel, vel);
    }

    #[test]
    fn remote_proxy_never_carries_player() {
        let mut app = App::new();
        let remote_ent = app.world_mut().spawn((
            RemoteProxy {
                id: 42,
                state: RigState {
                    mode: MoveMode::Walk,
                    yaw: 0.0,
                    cam_pitch: 0.0,
                    throttle: 0.0,
                    wingsuit_on: false,
                    afterburner: false,
                    fwd_in: 0.0,
                    strafe: 0.0,
                    wing_arch: 0.0,
                    wing_turn: 0.0,
                    fall_spread: 0.0,
                    fall_tuck: 0.0,
                    jump_charge: 0.0,
                    jump_push_timer: 0.0,
                    crouch: false,
                    landing_recovery: 0.0,
                    leg_steer: Vec3::ZERO,
                    intent_dir: Vec3::ZERO,
                    rot_err: Vec3::ZERO,
                    accel_smooth: Vec3::ZERO,
                    ground_normal: Vec3::Y,
                    foot_ground_hit: [None, None],
                    first_person: false,
                    vel: Vec3::ZERO,
                },
                quat: Quat::IDENTITY,
                pose_mode: PoseMode::Procedural,
                replicated_parts: HashMap::new(),
            },
        )).id();

        assert!(
            app.world().get::<Player>(remote_ent).is_none(),
            "RemoteProxy must NEVER carry a Player component (would break 15 .single() queries)"
        );
    }
}
