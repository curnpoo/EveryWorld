//! Skeletons, retargeting, clips, cloth.
//!
//! Allowed workspace dependencies: `ed_geo`, `ed_pack`. The list is enforced by
//! `cargo xtask check-deps` (rules in `xtask/deps.toml`).
//!
//! M3: skinning palettes from a pack [`Skeleton`] and a deterministic procedural pose
//! ([`sine_pose`]) used to prove the skinned path. The pose is a pure function of the skeleton
//! and the simulation time: every joint below the root sways about a joint-index-dependent
//! axis with a joint-index-dependent phase. It makes no assumption about joint names or body
//! layout (no neck, head or limb lookups), so any rig animates.

use ed_pack::kinds::{Skeleton, Transform};
use glam::{Mat4, Quat, Vec3};

/// A local transform as glam values.
pub fn local_matrix(t: &Transform) -> Mat4 {
    Mat4::from_scale_rotation_translation(Vec3::from(t.scale), Quat::from_array(t.rotation).normalize(), Vec3::from(t.translation))
}

/// Global (skeleton-space) transforms from local ones. Joints are stored parents first.
pub fn globals(sk: &Skeleton, locals: &[Mat4]) -> Vec<Mat4> {
    let mut g: Vec<Mat4> = Vec::with_capacity(sk.joints.len());
    for (i, j) in sk.joints.iter().enumerate() {
        let m = match j.parent {
            Some(p) => g[p as usize] * locals[i],
            None => locals[i],
        };
        g.push(m);
    }
    g
}

/// Skinning palette: `global * inverse_bind` per joint (mesh space), the input of
/// `ed_render::Renderer::set_skin_pose`.
pub fn palette(sk: &Skeleton, locals: &[Mat4]) -> Vec<Mat4> {
    globals(sk, locals).iter().zip(&sk.joints).map(|(g, j)| *g * Mat4::from_cols_array(&j.inverse_bind)).collect()
}

/// The rest pose's local matrices.
pub fn rest_locals(sk: &Skeleton) -> Vec<Mat4> {
    sk.joints.iter().map(|j| local_matrix(&j.rest)).collect()
}

/// Parameters of the procedural sway.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sway {
    /// Peak rotation per joint, radians.
    pub amplitude: f32,
    /// Hz.
    pub frequency: f32,
}

impl Default for Sway {
    fn default() -> Self {
        Self { amplitude: 0.22, frequency: 0.5 }
    }
}

/// Joint `i`'s sway axis (local space): cycles through X, Z and a diagonal so neighbouring
/// joints bend differently.
fn axis(i: usize) -> Vec3 {
    match i % 3 {
        0 => Vec3::X,
        1 => Vec3::Z,
        _ => Vec3::new(1.0, 0.0, 1.0).normalize(),
    }
}

/// Local matrices of the procedural pose at simulation time `t` seconds: rest * sway. Roots
/// keep their rest transform (the character stays where it was placed). Deterministic: the
/// same skeleton and `t` give bit-identical matrices.
pub fn sine_pose(sk: &Skeleton, t: f64, s: Sway) -> Vec<Mat4> {
    sk.joints
        .iter()
        .enumerate()
        .map(|(i, j)| {
            let rest = local_matrix(&j.rest);
            if j.parent.is_none() {
                return rest;
            }
            // Phase in f64 so long sessions do not lose precision.
            let phase = (std::f64::consts::TAU * s.frequency as f64 * t + i as f64 * 0.9).rem_euclid(std::f64::consts::TAU);
            let a = s.amplitude * (phase as f32).sin();
            rest * Mat4::from_quat(Quat::from_axis_angle(axis(i), a))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed_pack::AssetId;
    use ed_pack::kinds::Joint;

    /// A three-joint chain up +Y, one metre apart, inverse binds of the rest pose.
    fn chain() -> Skeleton {
        let mut joints = Vec::new();
        for i in 0..3u32 {
            let rest = Transform { translation: [0.0, if i == 0 { 0.0 } else { 1.0 }, 0.0], ..Default::default() };
            let inv = Mat4::from_translation(Vec3::new(0.0, -(i as f32), 0.0)).to_cols_array();
            joints.push(Joint { parent: i.checked_sub(1), name: format!("j{i}"), rest, inverse_bind: inv });
        }
        Skeleton { mesh: AssetId::from_path("m"), joints }
    }

    #[test]
    fn rest_pose_palette_is_identity() {
        let sk = chain();
        for m in palette(&sk, &rest_locals(&sk)) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-6), "{m:?}");
        }
    }

    #[test]
    fn sine_pose_is_deterministic_and_moves() {
        let sk = chain();
        let a = sine_pose(&sk, 1.25, Sway::default());
        let b = sine_pose(&sk, 1.25, Sway::default());
        assert_eq!(a.iter().map(|m| m.to_cols_array().map(f32::to_bits)).collect::<Vec<_>>(), b.iter().map(|m| m.to_cols_array().map(f32::to_bits)).collect::<Vec<_>>());
        // The root stays at rest; the chain's tip moves; two times give two poses.
        let rest = rest_locals(&sk);
        assert_eq!(a[0], rest[0]);
        let tip = |p: &[Mat4]| globals(&sk, p)[2].w_axis.truncate();
        assert!(tip(&a).distance(tip(&rest)) > 0.05);
        let c = sine_pose(&sk, 2.9, Sway::default());
        assert!(tip(&a).distance(tip(&c)) > 0.05);
        // Bone lengths are kept (rotations only).
        let g = globals(&sk, &a);
        assert!((g[2].w_axis.truncate().distance(g[1].w_axis.truncate()) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn long_running_time_stays_precise() {
        let sk = chain();
        let day = 86_400.0 * 30.0;
        let a = sine_pose(&sk, day + 0.25, Sway::default());
        let b = sine_pose(&sk, 0.25, Sway::default());
        for (x, y) in a.iter().zip(&b) {
            assert!(x.abs_diff_eq(*y, 1e-3));
        }
    }
}
