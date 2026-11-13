//! Private binding to the C shim (`csrc/ed_box3d_shim.c`) over vendored Box3D.
//!
//! Hand-written declarations for exactly the shim functions we call. All structs are `repr(C)`
//! mirrors of the shim's boundary structs; `layout_guard` (test below) compares every size,
//! alignment and field offset with the values the C compiler reports, so an upstream bump or a
//! shim edit cannot silently corrupt memory. Box3D's own definition structs never cross.

#![allow(dead_code)]

use std::ffi::{c_int, c_void};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct B3Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct B3Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub s: f32,
}

impl Default for B3Quat {
    fn default() -> Self {
        B3Quat { x: 0.0, y: 0.0, z: 0.0, s: 1.0 }
    }
}

/// Box3D world position (`b3Pos`, double-precision build).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct B3Pos {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct B3Capsule {
    pub center1: B3Vec3,
    pub center2: B3Vec3,
    pub radius: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EdWorldDesc {
    pub gravity: B3Vec3,
    pub enable_sleep: u8,
    pub enable_continuous: u8,
    pub pad: [u8; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EdBodyDesc {
    pub position: B3Pos,
    pub rotation: B3Quat,
    pub linear_velocity: B3Vec3,
    pub angular_velocity: B3Vec3,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity_scale: f32,
    pub kind: i32,
    pub is_bullet: u8,
    pub enable_sleep: u8,
    pub lock_rotation: u8,
    pub pad: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EdShapeMat {
    pub category_bits: u64,
    pub mask_bits: u64,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EdBodyState {
    pub p: B3Pos,
    pub q: B3Quat,
    pub v: B3Vec3,
    pub w: B3Vec3,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EdHit {
    pub point: B3Pos,
    pub normal: B3Vec3,
    pub fraction: f32,
    pub body_key: u64,
    pub shape_key: u64,
    pub hit: i32,
    pub pad: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EdPlane {
    pub normal: B3Vec3,
    pub offset: f32,
    pub point: B3Vec3,
    pub pad: f32,
}

unsafe extern "C" {
    pub fn ed_b3_is_double_precision() -> c_int;
    pub fn ed_b3_layout(out: *mut u64, capacity: c_int) -> c_int;

    pub fn ed_world_create(desc: *const EdWorldDesc) -> u32;
    pub fn ed_world_destroy(w: u32);
    pub fn ed_world_step(w: u32, dt: f32, substeps: c_int);
    pub fn ed_world_awake_body_count(w: u32) -> c_int;

    pub fn ed_body_create(w: u32, desc: *const EdBodyDesc) -> u64;
    pub fn ed_body_destroy(w: u32, key: u64);
    pub fn ed_bodies_read(w: u32, keys: *const u64, count: c_int, out: *mut EdBodyState);
    pub fn ed_body_is_awake(w: u32, key: u64) -> c_int;
    pub fn ed_body_set_transform(w: u32, key: u64, p: *const B3Pos, q: *const B3Quat);
    pub fn ed_body_set_velocity(w: u32, key: u64, v: *const B3Vec3, av: *const B3Vec3);
    pub fn ed_body_set_target(w: u32, key: u64, p: *const B3Pos, q: *const B3Quat, dt: f32);
    pub fn ed_body_apply_impulse(w: u32, key: u64, impulse: *const B3Vec3);
    pub fn ed_body_apply_force_at(w: u32, key: u64, force: *const B3Vec3, point: *const B3Pos);
    pub fn ed_body_apply_torque(w: u32, key: u64, torque: *const B3Vec3);
    pub fn ed_body_apply_impulse_at(w: u32, key: u64, impulse: *const B3Vec3, point: *const B3Pos);
    pub fn ed_body_set_mass(w: u32, key: u64, mass: f32, center: *const B3Vec3, inertia_diag: *const B3Vec3);
    pub fn ed_body_mass(w: u32, key: u64, center: *mut B3Vec3) -> f32;

    pub fn ed_shape_box(w: u32, body: u64, m: *const EdShapeMat, half: *const B3Vec3, p: *const B3Vec3, q: *const B3Quat) -> u64;
    pub fn ed_shape_sphere(w: u32, body: u64, m: *const EdShapeMat, center: *const B3Vec3, radius: f32) -> u64;
    pub fn ed_shape_capsule(w: u32, body: u64, m: *const EdShapeMat, c: *const B3Capsule) -> u64;
    pub fn ed_shape_hull(w: u32, body: u64, m: *const EdShapeMat, hull: *const c_void, p: *const B3Vec3, q: *const B3Quat) -> u64;
    pub fn ed_shape_mesh(w: u32, body: u64, m: *const EdShapeMat, mesh: *const c_void) -> u64;
    pub fn ed_shape_height_field(w: u32, body: u64, m: *const EdShapeMat, hf: *const c_void) -> u64;

    pub fn ed_hull_create(points: *const B3Vec3, count: c_int) -> *mut c_void;
    pub fn ed_hull_destroy(hull: *mut c_void);
    pub fn ed_hull_edges(hull: *const c_void, out: *mut f32, edge_capacity: c_int) -> c_int;
    pub fn ed_mesh_create(vertices: *const B3Vec3, vertex_count: c_int, indices: *const i32, triangle_count: c_int) -> *mut c_void;
    pub fn ed_mesh_destroy(mesh: *mut c_void);
    pub fn ed_height_field_create(heights: *const f32, count_x: c_int, count_z: c_int, scale: *const B3Vec3, min_h: f32, max_h: f32) -> *mut c_void;
    pub fn ed_height_field_destroy(hf: *mut c_void);

    pub fn ed_cast_ray(w: u32, origin: *const B3Pos, translation: *const B3Vec3, category: u64, mask: u64, out: *mut EdHit) -> c_int;
    pub fn ed_cast_shape(
        w: u32,
        origin: *const B3Pos,
        points: *const B3Vec3,
        count: c_int,
        radius: f32,
        translation: *const B3Vec3,
        category: u64,
        mask: u64,
        out: *mut EdHit,
    ) -> c_int;
    pub fn ed_collide_mover(w: u32, origin: *const B3Pos, mover: *const B3Capsule, category: u64, mask: u64, out: *mut EdPlane, capacity: c_int) -> c_int;
    pub fn ed_cast_mover(w: u32, origin: *const B3Pos, mover: *const B3Capsule, translation: *const B3Vec3, category: u64, mask: u64) -> f32;

    pub fn ed_world_snapshot(w: u32, out_size: *mut c_int) -> *mut u8;
    pub fn ed_blob_free(blob: *mut u8, size: c_int);
    pub fn ed_world_restore(w: u32, blob: *const u8, size: c_int) -> c_int;
}

/// The Rust side of the layout table, in the order `ed_b3_layout` writes it.
pub(crate) fn rust_layout() -> Vec<u64> {
    use std::mem::{align_of, offset_of, size_of};
    macro_rules! l {
        ($v:ident, $t:ty) => {
            $v.push(size_of::<$t>() as u64);
            $v.push(align_of::<$t>() as u64);
        };
    }
    macro_rules! o {
        ($v:ident, $t:ty, $f:ident) => {
            $v.push(offset_of!($t, $f) as u64);
        };
    }
    let mut v = Vec::new();
    l!(v, B3Vec3);
    l!(v, B3Quat);
    l!(v, B3Pos);
    l!(v, B3Capsule);
    o!(v, B3Capsule, center2);
    o!(v, B3Capsule, radius);
    l!(v, EdWorldDesc);
    o!(v, EdWorldDesc, enable_sleep);
    o!(v, EdWorldDesc, enable_continuous);
    l!(v, EdBodyDesc);
    o!(v, EdBodyDesc, rotation);
    o!(v, EdBodyDesc, linear_velocity);
    o!(v, EdBodyDesc, angular_velocity);
    o!(v, EdBodyDesc, linear_damping);
    o!(v, EdBodyDesc, gravity_scale);
    o!(v, EdBodyDesc, kind);
    o!(v, EdBodyDesc, is_bullet);
    o!(v, EdBodyDesc, lock_rotation);
    l!(v, EdShapeMat);
    o!(v, EdShapeMat, mask_bits);
    o!(v, EdShapeMat, density);
    o!(v, EdShapeMat, restitution);
    l!(v, EdBodyState);
    o!(v, EdBodyState, q);
    o!(v, EdBodyState, v);
    o!(v, EdBodyState, w);
    l!(v, EdHit);
    o!(v, EdHit, normal);
    o!(v, EdHit, fraction);
    o!(v, EdHit, body_key);
    o!(v, EdHit, shape_key);
    o!(v, EdHit, hit);
    l!(v, EdPlane);
    o!(v, EdPlane, offset);
    o!(v, EdPlane, point);
    v
}

/// The C side of the layout table.
pub(crate) fn c_layout() -> Vec<u64> {
    let mut buf = [0u64; 128];
    // SAFETY: the buffer holds 128 entries and we pass that capacity; the shim writes at most
    // that many and returns the count written.
    let n = unsafe { ed_b3_layout(buf.as_mut_ptr(), buf.len() as c_int) };
    assert!(n >= 0, "layout buffer too small");
    buf[..n as usize].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_guard() {
        let c = c_layout();
        let r = rust_layout();
        assert_eq!(c.len(), r.len(), "layout tables differ in length: shim and ffi.rs out of sync");
        for (i, (a, b)) in c.iter().zip(r.iter()).enumerate() {
            assert_eq!(a, b, "FFI layout mismatch at entry {i}: C {a} vs Rust {b}");
        }
    }

    #[test]
    fn double_precision_build() {
        // SAFETY: plain query with no arguments.
        assert_eq!(unsafe { ed_b3_is_double_precision() }, 1);
    }
}
