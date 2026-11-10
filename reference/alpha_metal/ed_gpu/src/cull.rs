//! GPU-driven frustum culling: the shared CPU/GPU contract.
//!
//! The Metal kernel in `shaders/cull.metal` uses the same struct layouts and the same
//! sphere-vs-plane test as this module. [`cull_cpu`] is the reference implementation.
//!
//! Conventions:
//! * Matrices are column-major: `m[col][row]`, and `view_proj * v` is clip space.
//! * Clip space is Metal's: `-w <= x <= w`, `-w <= y <= w`, `0 <= z <= w`.
//! * Planes are `[nx, ny, nz, d]`. A point `p` is on the inside when `dot(n, p) + d >= 0`.

/// One drawable instance. Layout must match `Instance` in `cull.metal` (32 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub center: [f32; 3],
    pub radius: f32,
    pub mesh: u32,
    pub first_index: u32,
    pub index_count: u32,
    pub base_vertex: i32,
}

/// Per-dispatch constants. Layout must match `CullParams` in `cull.metal` (112 bytes).
#[repr(C)]
pub struct CullParams {
    pub planes: [[f32; 4]; 6],
    pub instance_count: u32,
    pub max_draws: u32,
    pub _pad: [u32; 2],
}

/// Indirect draw arguments. Field order matches `MTLDrawIndexedPrimitivesIndirectArguments`
/// and the `DrawIndexedIndirect` struct in `cull.metal` (20 bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DrawIndexedIndirect {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

const _: () = assert!(std::mem::size_of::<Instance>() == 32);
const _: () = assert!(std::mem::size_of::<CullParams>() == 112);
const _: () = assert!(std::mem::size_of::<DrawIndexedIndirect>() == 20);

/// Extracts the six normalised frustum planes from a column-major `view_proj` matrix
/// (Metal clip space, z in `0..=w`). Planes point inward.
pub fn frustum_planes(view_proj: [[f32; 4]; 4]) -> [[f32; 4]; 6] {
    // Row i of the matrix, as used by the Gribb/Hartmann extraction.
    let row = |i: usize| -> [f32; 4] { [view_proj[0][i], view_proj[1][i], view_proj[2][i], view_proj[3][i]] };
    let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
    let add = |a: [f32; 4], b: [f32; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
    let sub = |a: [f32; 4], b: [f32; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];

    let raw = [
        add(r3, r0), // left:   x >= -w
        sub(r3, r0), // right:  x <=  w
        add(r3, r1), // bottom: y >= -w
        sub(r3, r1), // top:    y <=  w
        r2,          // near:   z >=  0 (Metal)
        sub(r3, r2), // far:    z <=  w
    ];

    let mut planes = [[0.0f32; 4]; 6];
    for (out, p) in planes.iter_mut().zip(raw) {
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        let inv = if len > 0.0 { 1.0 / len } else { 0.0 };
        *out = [p[0] * inv, p[1] * inv, p[2] * inv, p[3] * inv];
    }
    planes
}

/// Conservative sphere-vs-frustum test. Returns `false` only when the sphere is entirely
/// outside one of the planes. Matches the kernel test exactly.
pub fn sphere_visible(planes: &[[f32; 4]; 6], center: [f32; 3], radius: f32) -> bool {
    planes.iter().all(|p| {
        let dist = p[0] * center[0] + p[1] * center[1] + p[2] * center[2] + p[3];
        dist >= -radius
    })
}

/// CPU reference for the GPU kernel. Emits one draw per visible instance, in input order,
/// with `first_instance` set to the instance's index in `instances`. Stops at `max_draws`.
pub fn cull_cpu(planes: &[[f32; 4]; 6], instances: &[Instance], max_draws: u32) -> Vec<DrawIndexedIndirect> {
    instances
        .iter()
        .enumerate()
        .filter(|(_, inst)| sphere_visible(planes, inst.center, inst.radius))
        .take(max_draws as usize)
        .map(|(i, inst)| DrawIndexedIndirect {
            index_count: inst.index_count,
            instance_count: 1,
            first_index: inst.first_index,
            base_vertex: inst.base_vertex,
            first_instance: i as u32,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Metal-convention perspective (fovY 90 degrees, aspect 1, near 1, far 100), camera at
    /// the origin looking down -z, identity view. Column-major.
    fn test_view_proj() -> [[f32; 4]; 4] {
        let (near, far) = (1.0f32, 100.0f32);
        let a = far / (near - far);
        let b = near * far / (near - far);
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, a, -1.0],
            [0.0, 0.0, b, 0.0],
        ]
    }

    fn planes() -> [[f32; 4]; 6] {
        frustum_planes(test_view_proj())
    }

    fn inst(center: [f32; 3], radius: f32, first_index: u32) -> Instance {
        Instance { center, radius, mesh: 0, first_index, index_count: 36, base_vertex: 0 }
    }

    #[test]
    fn planes_are_normalised() {
        for p in planes() {
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "plane {p:?} not normalised");
        }
    }

    #[test]
    fn sphere_inside() {
        assert!(sphere_visible(&planes(), [0.0, 0.0, -10.0], 0.5));
    }

    #[test]
    fn sphere_outside_each_plane() {
        let p = planes();
        // Left (x >= z), right (x <= -z), bottom, top, near (depth >= 1), far (depth <= 100).
        assert!(!sphere_visible(&p, [-20.0, 0.0, -10.0], 0.5), "left");
        assert!(!sphere_visible(&p, [20.0, 0.0, -10.0], 0.5), "right");
        assert!(!sphere_visible(&p, [0.0, -20.0, -10.0], 0.5), "bottom");
        assert!(!sphere_visible(&p, [0.0, 20.0, -10.0], 0.5), "top");
        assert!(!sphere_visible(&p, [0.0, 0.0, -0.5], 0.0), "near");
        assert!(!sphere_visible(&p, [0.0, 0.0, -200.0], 0.5), "far");
    }

    #[test]
    fn sphere_straddling_each_plane() {
        let p = planes();
        // Centre is 0.141 outside the left plane (distance (x - z)/sqrt(2)).
        assert!(sphere_visible(&p, [-10.2, 0.0, -10.0], 0.2), "left straddle");
        assert!(!sphere_visible(&p, [-10.2, 0.0, -10.0], 0.1), "left just outside");
        assert!(sphere_visible(&p, [10.2, 0.0, -10.0], 0.2), "right straddle");
        assert!(sphere_visible(&p, [0.0, -10.2, -10.0], 0.2), "bottom straddle");
        assert!(sphere_visible(&p, [0.0, 10.2, -10.0], 0.2), "top straddle");
        // Near plane is at depth 1.
        assert!(sphere_visible(&p, [0.0, 0.0, -0.9], 0.2), "near straddle");
        assert!(!sphere_visible(&p, [0.0, 0.0, -0.9], 0.05), "near just outside");
        // Far plane is at depth 100.
        assert!(sphere_visible(&p, [0.0, 0.0, -100.1], 0.2), "far straddle");
    }

    #[test]
    fn cull_cpu_order_and_first_instance() {
        let instances = [
            inst([0.0, 0.0, -10.0], 0.5, 0),    // visible
            inst([50.0, 0.0, -10.0], 0.5, 1),   // culled
            inst([0.0, 0.0, -20.0], 0.5, 2),    // visible
            inst([0.0, 0.0, -300.0], 0.5, 3),   // culled
            inst([1.0, 1.0, -30.0], 0.5, 4),    // visible
        ];
        let draws = cull_cpu(&planes(), &instances, u32::MAX);
        let got: Vec<(u32, u32)> = draws.iter().map(|d| (d.first_instance, d.first_index)).collect();
        assert_eq!(got, vec![(0, 0), (2, 2), (4, 4)]);
        for d in &draws {
            assert_eq!(d.instance_count, 1);
            assert_eq!(d.index_count, 36);
        }
    }

    #[test]
    fn cull_cpu_respects_max_draws() {
        let instances: Vec<Instance> = (0..10).map(|i| inst([0.0, 0.0, -10.0], 0.5, i)).collect();
        let draws = cull_cpu(&planes(), &instances, 3);
        assert_eq!(draws.len(), 3);
        let firsts: Vec<u32> = draws.iter().map(|d| d.first_instance).collect();
        assert_eq!(firsts, vec![0, 1, 2]);

        assert!(cull_cpu(&planes(), &instances, 0).is_empty());
        assert_eq!(cull_cpu(&planes(), &instances, 100).len(), 10);
    }

    #[test]
    fn cull_cpu_is_deterministic() {
        let instances: Vec<Instance> = (0..50)
            .map(|i| inst([(i as f32 - 25.0) * 1.5, 0.0, -10.0 - i as f32], 0.5, i))
            .collect();
        let a = cull_cpu(&planes(), &instances, u32::MAX);
        let b = cull_cpu(&planes(), &instances, u32::MAX);
        assert_eq!(a, b);
        assert!(a.windows(2).all(|w| w[0].first_instance < w[1].first_instance));
    }
}
