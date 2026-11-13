//! Draws the robot from primitives (cube, cylinder, sphere) placed on the solved rig.
//!
//! The alpha's character was built this way too: a part list (limbs between joints, balls at
//! joints, rigid pieces on frames) in the alpha's colours. The rig ([`super::rig`]) knows
//! nothing of this file, so another character can be driven by the same [`Pose`] later.
//!
//! This is the one place in `ew` that builds meshes from code (like `car.rs`); the
//! content-pack lint names it as an exception.

use super::rig::{F, J, Pose};
use ed_render::Renderer;
use ed_render::scene::{InstanceDesc, InstanceId, Material, MeshAsset, MeshId, MeshLod, NodeSlot};
use glam::{Affine3A, DVec3, Quat, Vec3};

/// What a part is attached to.
#[derive(Clone, Copy, Debug)]
enum Spec {
    /// Cylinder spanning two joints.
    Limb { a: J, b: J, r: f32 },
    /// Ball at a joint.
    Ball { j: J, r: f32 },
    /// A piece fixed to a frame: local offset, rotation, size.
    Rigid { f: F, off: Vec3, rot: Quat, scale: Vec3 },
    /// Eye: rigid on the head, blinks by squashing.
    Eye { off: Vec3 },
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    Cube,
    Cyl,
    Ball,
}

#[derive(Clone, Copy, Debug)]
enum Mat {
    Shell,
    ShellDark,
    Joint,
    Jacket,
    JacketDark,
    Shoe,
    Sole,
    Screen,
    Glow,
    Brass,
}

const MATS: [Mat; 10] = [Mat::Shell, Mat::ShellDark, Mat::Joint, Mat::Jacket, Mat::JacketDark, Mat::Shoe, Mat::Sole, Mat::Screen, Mat::Glow, Mat::Brass];

/// The alpha's material table: sRGB base colour, roughness, metallic.
fn material(m: Mat) -> Material {
    let (c, rough, metal): ([f32; 3], f32, f32) = match m {
        Mat::Shell => ([0.92, 0.89, 0.82], 0.4, 0.0),
        Mat::ShellDark => ([0.78, 0.74, 0.66], 0.45, 0.0),
        Mat::Joint => ([0.22, 0.23, 0.26], 0.45, 0.6),
        Mat::Jacket => ([0.96, 0.42, 0.09], 0.85, 0.0),
        Mat::JacketDark => ([0.72, 0.28, 0.05], 0.85, 0.0),
        Mat::Shoe => ([0.24, 0.25, 0.28], 0.7, 0.0),
        Mat::Sole => ([0.95, 0.95, 0.93], 0.6, 0.0),
        Mat::Screen => ([0.03, 0.06, 0.07], 0.06, 0.0),
        // The alpha's eyes were emissive cyan; this material model has no emission, so a bright
        // cyan stands in.
        Mat::Glow => ([0.35, 0.95, 1.0], 0.3, 0.0),
        Mat::Brass => ([0.85, 0.66, 0.32], 0.3, 0.9),
    };
    let lin = |s: f32| s.powf(2.2);
    Material::pbr(Vec3::new(lin(c[0]), lin(c[1]), lin(c[2])), metal, rough)
}

/// The alpha's part list, from `body.rs` `spawn_body_parts` (walking parts; the flames,
/// canopy and optional backpack are not used here).
fn parts() -> Vec<(Spec, Shape, Mat)> {
    use Mat::*;
    use Shape::{Ball as B, Cube as C, Cyl as Y};
    use Spec::*;
    let q0 = Quat::IDENTITY;
    let qx = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let qz = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
    let v = Vec3::new;
    vec![
        (Limb { a: J::HipL, b: J::KneeL, r: 0.075 }, Y, Shell),
        (Limb { a: J::HipR, b: J::KneeR, r: 0.075 }, Y, Shell),
        (Limb { a: J::KneeL, b: J::AnkleL, r: 0.065 }, Y, Shell),
        (Limb { a: J::KneeR, b: J::AnkleR, r: 0.065 }, Y, Shell),
        (Ball { j: J::HipL, r: 0.1 }, B, Joint),
        (Ball { j: J::HipR, r: 0.1 }, B, Joint),
        (Ball { j: J::KneeL, r: 0.088 }, B, Joint),
        (Ball { j: J::KneeR, r: 0.088 }, B, Joint),
        (Ball { j: J::AnkleL, r: 0.07 }, B, Joint),
        (Ball { j: J::AnkleR, r: 0.07 }, B, Joint),
        // Sneakers: upper, white sole, toe cap, nozzle.
        (Rigid { f: F::FootL, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, C, Shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.025, -0.05), rot: q0, scale: v(0.13, 0.08, 0.27) }, C, Shoe),
        (Rigid { f: F::FootL, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, C, Sole),
        (Rigid { f: F::FootR, off: v(0.0, -0.075, -0.05), rot: q0, scale: v(0.14, 0.03, 0.29) }, C, Sole),
        (Rigid { f: F::FootL, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, Y, Shoe),
        (Rigid { f: F::FootR, off: v(0.0, -0.035, -0.17), rot: qz, scale: v(0.08, 0.13, 0.08) }, Y, Shoe),
        (Rigid { f: F::FootL, off: v(0.0, -0.095, -0.03), rot: q0, scale: v(0.09, 0.025, 0.09) }, Y, Joint),
        (Rigid { f: F::FootR, off: v(0.0, -0.095, -0.03), rot: q0, scale: v(0.09, 0.025, 0.09) }, Y, Joint),
        // Pelvis, waist and spine.
        (Rigid { f: F::Pelvis, off: v(0.0, 0.0, 0.0), rot: q0, scale: v(0.34, 0.16, 0.22) }, C, Joint),
        (Limb { a: J::Pelvis, b: J::Chest, r: 0.08 }, Y, Joint),
        // Torso: jacket, hem, collar, zipper, pockets.
        (Rigid { f: F::Chest, off: v(0.0, -0.2, 0.0), rot: q0, scale: v(0.46, 0.4, 0.28) }, C, Jacket),
        (Rigid { f: F::Chest, off: v(0.0, -0.41, 0.0), rot: q0, scale: v(0.44, 0.06, 0.26) }, C, JacketDark),
        (Rigid { f: F::Chest, off: v(0.0, 0.02, 0.0), rot: q0, scale: v(0.3, 0.07, 0.22) }, C, JacketDark),
        (Rigid { f: F::Chest, off: v(0.0, -0.2, -0.142), rot: q0, scale: v(0.025, 0.36, 0.01) }, C, Brass),
        (Rigid { f: F::Chest, off: v(-0.12, -0.3, -0.142), rot: q0, scale: v(0.12, 0.09, 0.01) }, C, JacketDark),
        (Rigid { f: F::Chest, off: v(0.12, -0.3, -0.142), rot: q0, scale: v(0.12, 0.09, 0.01) }, C, JacketDark),
        (Limb { a: J::Chest, b: J::Neck, r: 0.055 }, Y, Joint),
        // Arms.
        (Limb { a: J::ShoulderL, b: J::ElbowL, r: 0.07 }, Y, Jacket),
        (Limb { a: J::ShoulderR, b: J::ElbowR, r: 0.07 }, Y, Jacket),
        (Limb { a: J::ElbowL, b: J::WristL, r: 0.05 }, Y, Shell),
        (Limb { a: J::ElbowR, b: J::WristR, r: 0.05 }, Y, Shell),
        (Ball { j: J::ShoulderL, r: 0.11 }, B, Jacket),
        (Ball { j: J::ShoulderR, r: 0.11 }, B, Jacket),
        (Ball { j: J::ElbowL, r: 0.07 }, B, Joint),
        (Ball { j: J::ElbowR, r: 0.07 }, B, Joint),
        (Ball { j: J::WristL, r: 0.05 }, B, Joint),
        (Ball { j: J::WristR, r: 0.05 }, B, Joint),
        // Hands: mitt and thumb.
        (Rigid { f: F::HandL, off: v(0.0, -0.06, 0.0), rot: q0, scale: v(0.07, 0.11, 0.09) }, C, Shell),
        (Rigid { f: F::HandR, off: v(0.0, -0.06, 0.0), rot: q0, scale: v(0.07, 0.11, 0.09) }, C, Shell),
        (Rigid { f: F::HandL, off: v(0.03, -0.04, -0.05), rot: q0, scale: v(0.03, 0.06, 0.03) }, C, ShellDark),
        (Rigid { f: F::HandR, off: v(-0.03, -0.04, -0.05), rot: q0, scale: v(0.03, 0.06, 0.03) }, C, ShellDark),
        // CRT head: case, bezel, bulged screen, tube housing, dials, antenna, chin.
        (Rigid { f: F::Head, off: v(0.0, 0.0, 0.0), rot: q0, scale: v(0.42, 0.34, 0.3) }, C, Shell),
        (Rigid { f: F::Head, off: v(0.0, 0.0, -0.152), rot: q0, scale: v(0.38, 0.3, 0.01) }, C, ShellDark),
        (Rigid { f: F::Head, off: v(-0.03, 0.005, -0.155), rot: q0, scale: v(0.29, 0.24, 0.05) }, B, Screen),
        (Rigid { f: F::Head, off: v(0.0, -0.01, 0.2), rot: q0, scale: v(0.3, 0.24, 0.14) }, C, ShellDark),
        (Rigid { f: F::Head, off: v(0.155, 0.06, -0.155), rot: qx, scale: v(0.045, 0.03, 0.045) }, Y, Joint),
        (Rigid { f: F::Head, off: v(0.155, -0.04, -0.155), rot: qx, scale: v(0.04, 0.03, 0.04) }, Y, Brass),
        (Rigid { f: F::Head, off: v(0.155, -0.11, -0.153), rot: q0, scale: v(0.05, 0.02, 0.01) }, C, Joint),
        (Rigid { f: F::Head, off: v(0.09, 0.27, 0.0), rot: Quat::from_rotation_z(-0.35), scale: v(0.012, 0.22, 0.012) }, Y, Joint),
        (Rigid { f: F::Head, off: v(0.13, 0.38, 0.0), rot: q0, scale: v(0.035, 0.035, 0.035) }, B, Jacket),
        (Rigid { f: F::Head, off: v(0.0, -0.19, 0.0), rot: q0, scale: v(0.2, 0.04, 0.16) }, C, Joint),
        (Eye { off: v(-0.085, 0.02, -0.18) }, B, Glow),
        (Eye { off: v(0.035, 0.02, -0.18) }, B, Glow),
    ]
}

/// A surface of revolution about y from a profile of `(radius, y, normal_r, normal_y)` points
/// listed bottom to top. Repeat a point with a different normal for a hard edge.
fn revolve(profile: &[(f32, f32, f32, f32)], sides: u32) -> MeshAsset {
    let mut m = MeshAsset::default();
    let mut idx = Vec::new();
    for i in 0..=sides {
        let a = i as f32 / sides as f32 * std::f32::consts::TAU;
        let (c, s) = (a.cos(), a.sin());
        for &(r, y, nr, ny) in profile {
            m.positions.push([r * c, y, r * s]);
            m.normals.push([nr * c, ny, nr * s]);
        }
    }
    let n = profile.len() as u32;
    for i in 0..sides {
        for j in 0..n - 1 {
            let (p00, p01) = (i * n + j, i * n + j + 1);
            let (p10, p11) = ((i + 1) * n + j, (i + 1) * n + j + 1);
            let (a, b) = (profile[j as usize], profile[j as usize + 1]);
            if a.0 == b.0 && a.1 == b.1 {
                continue; // a hard-edge duplicate: no surface between
            }
            if b.0 > 0.0 {
                idx.extend_from_slice(&[p00, p01, p11]);
            }
            if a.0 > 0.0 {
                idx.extend_from_slice(&[p00, p11, p10]);
            }
        }
    }
    m.lods.push(MeshLod { indices: idx, error: 0.0 });
    m
}

/// Unit cylinder (radius 0.5, height 1) along y, with caps.
fn cylinder_mesh() -> MeshAsset {
    revolve(&[(0.0, -0.5, 0.0, -1.0), (0.5, -0.5, 0.0, -1.0), (0.5, -0.5, 1.0, 0.0), (0.5, 0.5, 1.0, 0.0), (0.5, 0.5, 0.0, 1.0), (0.0, 0.5, 0.0, 1.0)], 20)
}

/// Unit sphere (diameter 1), smooth.
fn ball_mesh() -> MeshAsset {
    let rings = 12;
    let profile: Vec<_> = (0..=rings)
        .map(|k| {
            let t = k as f32 / rings as f32 * std::f32::consts::PI;
            let (r, y) = (t.sin() * 0.5, -t.cos() * 0.5);
            (r, y, t.sin(), -t.cos())
        })
        .collect();
    revolve(&profile, 20)
}

pub struct RobotView {
    node: NodeSlot,
    parts: Vec<(InstanceId, Spec)>,
    shown: bool,
}

impl RobotView {
    pub fn new(r: &mut Renderer, dev: &dyn ed_gpu::Device) -> Result<RobotView, String> {
        let meshes: [MeshId; 3] = [
            r.add_mesh(dev, &ed_render::cube_mesh()).map_err(|e| format!("robot mesh: {e}"))?,
            r.add_mesh(dev, &cylinder_mesh()).map_err(|e| format!("robot mesh: {e}"))?,
            r.add_mesh(dev, &ball_mesh()).map_err(|e| format!("robot mesh: {e}"))?,
        ];
        let node = r.instances().add_node(DVec3::ZERO).ok_or("robot: node table full")?;
        let mut mats = Vec::new();
        for m in MATS {
            mats.push(r.add_material(material(m)).ok_or("robot: material table full")?);
        }
        let mut out = Vec::new();
        for (k, (spec, shape, mat)) in parts().into_iter().enumerate() {
            let mesh = meshes[shape as usize];
            let id = r
                .instances()
                .add(&InstanceDesc { node, transform: Affine3A::from_scale(Vec3::ZERO), mesh, material: mats[mat as usize], object: ed_geo::ObjectId(0x20B0_0000 + k as u128), flags: 0 })
                .ok_or("robot: instance pool full")?;
            out.push((id, spec));
        }
        Ok(RobotView { node, parts: out, shown: false })
    }

    /// Places the robot: `pose` is relative to `origin` (render frame, f64).
    pub fn draw(&mut self, r: &mut Renderer, pose: &Pose, origin: DVec3) {
        self.shown = true;
        r.instances().set_node(self.node, origin);
        for (id, spec) in &self.parts {
            let t = match *spec {
                Spec::Limb { a, b, r: rad } => {
                    let (pa, pb) = (pose.joint(a), pose.joint(b));
                    let d = pb - pa;
                    let len = d.length().max(0.001);
                    Affine3A::from_scale_rotation_translation(Vec3::new(rad * 2.0, len, rad * 2.0), Quat::from_rotation_arc(Vec3::Y, d / len), (pa + pb) * 0.5)
                }
                Spec::Ball { j, r: rad } => Affine3A::from_scale_rotation_translation(Vec3::splat(rad * 2.0), Quat::IDENTITY, pose.joint(j)),
                Spec::Rigid { f, off, rot, scale } => {
                    let (p0, q) = pose.frame(f);
                    Affine3A::from_scale_rotation_translation(scale, q * rot, p0 + q * off)
                }
                Spec::Eye { off } => {
                    let (p0, q) = pose.frame(F::Head);
                    Affine3A::from_scale_rotation_translation(Vec3::new(0.055, 0.075 * pose.blink, 0.02), q, p0 + q * off)
                }
            };
            r.instances().set_transform(*id, &t);
        }
    }

    /// Hides every part (the robot is in the car).
    pub fn hide(&mut self, r: &mut Renderer) {
        if !self.shown {
            return;
        }
        self.shown = false;
        for (id, _) in &self.parts {
            r.instances().set_transform(*id, &Affine3A::from_scale(Vec3::ZERO));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every triangle faces the way its vertex normals point.
    fn assert_outward(m: &MeshAsset) {
        let p = |i: u32| Vec3::from(m.positions[i as usize]);
        let n = |i: u32| Vec3::from(m.normals[i as usize]);
        let idx = &m.lods[0].indices;
        assert!(!idx.is_empty());
        for t in idx.chunks(3) {
            let face = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            assert!(face.length() > 1e-9, "degenerate triangle");
            let avg = n(t[0]) + n(t[1]) + n(t[2]);
            assert!(face.dot(avg) > 0.0, "triangle {t:?} faces inward");
        }
    }

    #[test]
    fn revolved_meshes_face_outward() {
        assert_outward(&cylinder_mesh());
        assert_outward(&ball_mesh());
    }

    #[test]
    fn every_part_names_a_valid_material_and_shape() {
        let p = parts();
        assert!(p.len() > 50);
        for (_, shape, mat) in p {
            assert!((shape as usize) < 3 && (mat as usize) < MATS.len());
        }
    }
}
