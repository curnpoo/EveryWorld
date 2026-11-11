//! Transform gizmo: picking rays, drag maths, draw lists and screen-space hit tests.
//!
//! Maths is f64 in the world frame. Drawing data is camera-relative f32 (positions minus the
//! camera position), which is what the renderer's overlay pass takes, so the gizmo stays
//! exact far from the origin. Pixel coordinates are physical pixels, origin top-left.

use ed_render::Camera;
use glam::{DQuat, DVec2, DVec3};

use crate::model::Snap;
use crate::transform::Transform;

/// Camera maths in f64 for one viewport.
#[derive(Clone, Copy, Debug)]
pub struct ViewMath {
    pub eye: DVec3,
    pub rotation: DQuat,
    pub tan_half_y: f64,
    pub aspect: f64,
    pub near: f64,
    pub size: DVec2,
}

impl ViewMath {
    pub fn new(cam: &Camera, size_px: [u32; 2]) -> Self {
        let size = DVec2::new(size_px[0].max(1) as f64, size_px[1].max(1) as f64);
        ViewMath {
            eye: cam.position,
            rotation: cam.rotation().as_dquat(),
            tan_half_y: (cam.fov_y as f64 * 0.5).tan(),
            aspect: size.x / size.y,
            near: cam.near as f64,
            size,
        }
    }
    pub fn forward(&self) -> DVec3 {
        self.rotation * DVec3::NEG_Z
    }
    /// World-space ray through a pixel (pixel centres at +0.5).
    pub fn ray(&self, px: DVec2) -> Ray {
        let ndc = DVec2::new(
            2.0 * px.x / self.size.x - 1.0,
            1.0 - 2.0 * px.y / self.size.y,
        );
        let v = DVec3::new(
            ndc.x * self.tan_half_y * self.aspect,
            ndc.y * self.tan_half_y,
            -1.0,
        );
        Ray {
            origin: self.eye,
            dir: (self.rotation * v).normalize(),
        }
    }
    /// Pixel position of a world point, or `None` behind the near plane.
    pub fn project(&self, p: DVec3) -> Option<DVec2> {
        let v = self.rotation.inverse() * (p - self.eye);
        if -v.z < self.near {
            return None;
        }
        let x = v.x / -v.z / (self.tan_half_y * self.aspect);
        let y = v.y / -v.z / self.tan_half_y;
        Some(DVec2::new(
            (x + 1.0) * 0.5 * self.size.x,
            (1.0 - y) * 0.5 * self.size.y,
        ))
    }
    /// World size of `px` pixels at distance of `p`.
    pub fn pixel_world(&self, p: DVec3, px: f64) -> f64 {
        let depth = (p - self.eye).dot(self.forward()).max(self.near);
        depth * 2.0 * self.tan_half_y * px / self.size.y
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: DVec3,
    pub dir: DVec3,
}

impl Ray {
    pub fn at(&self, s: f64) -> DVec3 {
        self.origin + self.dir * s
    }
    /// Intersection with the plane through `p` with normal `n`, in front of the origin.
    pub fn plane(&self, p: DVec3, n: DVec3) -> Option<DVec3> {
        let d = n.dot(self.dir);
        if d.abs() < 1e-9 {
            return None;
        }
        let s = n.dot(p - self.origin) / d;
        (s > 0.0).then(|| self.at(s))
    }
    /// Parameter `t` of the point on the line `p + t a` closest to this ray (`a` unit).
    pub fn closest_on_line(&self, p: DVec3, a: DVec3) -> Option<f64> {
        let r = self.origin - p;
        let b = self.dir.dot(a);
        let denom = 1.0 - b * b;
        if denom < 1e-6 {
            return None; // looking along the axis
        }
        Some((a.dot(r) - b * self.dir.dot(r)) / denom)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

/// Which axes the gizmo uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Space {
    /// Local east, north, up at the object.
    #[default]
    Enu,
    /// The object's own axes.
    Object,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// Move along axis 0, 1, 2.
    Axis(u8),
    /// Move in the plane normal to axis 0, 1, 2.
    Plane(u8),
    /// Rotate about axis 0, 1, 2.
    Ring(u8),
    /// Uniform scale.
    Scale,
}

/// Length of the gizmo axes on screen, pixels.
pub const GIZMO_PX: f64 = 110.0;
/// Hit tolerance, pixels.
pub const HIT_PX: f64 = 8.0;
const RING_SEGMENTS: usize = 64;

/// A gizmo placed at an object for one frame.
#[derive(Clone, Copy, Debug)]
pub struct Gizmo {
    pub mode: Mode,
    pub origin: DVec3,
    /// Unit world axes (columns of the gizmo frame).
    pub axes: [DVec3; 3],
    /// World length of an axis.
    pub length: f64,
}

pub const AXIS_COLORS: [[u8; 4]; 3] = [[230, 60, 60, 255], [80, 200, 80, 255], [70, 120, 240, 255]];
pub const HOVER_COLOR: [u8; 4] = [255, 220, 40, 255];
pub const SCALE_COLOR: [u8; 4] = [235, 235, 235, 255];

impl Gizmo {
    pub fn new(mode: Mode, origin: DVec3, frame: DQuat, view: &ViewMath) -> Self {
        Gizmo {
            mode,
            origin,
            axes: [frame * DVec3::X, frame * DVec3::Y, frame * DVec3::Z],
            length: view.pixel_world(origin, GIZMO_PX),
        }
    }

    fn plane_corners(&self, i: usize) -> [DVec3; 4] {
        let (a, b) = (self.axes[(i + 1) % 3], self.axes[(i + 2) % 3]);
        let (lo, hi) = (0.25 * self.length, 0.45 * self.length);
        let o = self.origin;
        [
            o + a * lo + b * lo,
            o + a * hi + b * lo,
            o + a * hi + b * hi,
            o + a * lo + b * hi,
        ]
    }

    fn ring_points(&self, i: usize) -> Vec<DVec3> {
        let (a, b) = (self.axes[(i + 1) % 3], self.axes[(i + 2) % 3]);
        (0..=RING_SEGMENTS)
            .map(|k| {
                let t = k as f64 / RING_SEGMENTS as f64 * std::f64::consts::TAU;
                self.origin + (a * t.cos() + b * t.sin()) * self.length
            })
            .collect()
    }

    /// The handle under a pixel, nearest first. Axes beat planes when both are within reach.
    pub fn hit(&self, view: &ViewMath, px: DVec2) -> Option<Handle> {
        let mut best: Option<(f64, Handle)> = None;
        let mut offer = |d: f64, h: Handle| {
            if d <= HIT_PX && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, h));
            }
        };
        let o = view.project(self.origin)?;
        match self.mode {
            Mode::Translate => {
                for i in 0..3 {
                    if let Some(e) = view.project(self.origin + self.axes[i] * self.length) {
                        offer(seg_dist(px, o.lerp(e, 0.15), e), Handle::Axis(i as u8));
                    }
                    let q: Option<Vec<DVec2>> = self
                        .plane_corners(i)
                        .iter()
                        .map(|c| view.project(*c))
                        .collect();
                    if let Some(q) = q
                        && in_quad(px, &q)
                    {
                        offer(HIT_PX * 0.5, Handle::Plane(i as u8));
                    }
                }
            }
            Mode::Rotate => {
                let to_eye = view.eye - self.origin;
                for i in 0..3 {
                    let pts = self.ring_points(i);
                    for w in pts.windows(2) {
                        // Only the half of the ring facing the camera.
                        if (w[0] - self.origin).dot(to_eye) < -0.1 * self.length * to_eye.length() {
                            continue;
                        }
                        if let (Some(a), Some(b)) = (view.project(w[0]), view.project(w[1])) {
                            offer(seg_dist(px, a, b), Handle::Ring(i as u8));
                        }
                    }
                }
            }
            Mode::Scale => {
                offer((px - o).length() - 6.0, Handle::Scale);
                for i in 0..3 {
                    if let Some(e) = view.project(self.origin + self.axes[i] * self.length) {
                        offer(seg_dist(px, o, e), Handle::Scale);
                    }
                }
            }
        }
        best.map(|(_, h)| h)
    }

    /// Lines and triangles for the overlay pass, camera-relative f32.
    #[allow(clippy::needless_range_loop)] // `i` indexes axes, colours and handles together
    pub fn draw(&self, view: &ViewMath, hover: Option<Handle>, out: &mut DrawList) {
        let rel = |p: DVec3| (p - view.eye).as_vec3().to_array();
        let col = |h: Handle, base: [u8; 4]| if hover == Some(h) { HOVER_COLOR } else { base };
        match self.mode {
            Mode::Translate => {
                for i in 0..3 {
                    let c = col(Handle::Axis(i as u8), AXIS_COLORS[i]);
                    let a = self.axes[i];
                    let tip = self.origin + a * self.length;
                    out.line(rel(self.origin + a * 0.15 * self.length), rel(tip), c);
                    // Cone: 8 triangles around the tip.
                    let (u, v) = (self.axes[(i + 1) % 3], self.axes[(i + 2) % 3]);
                    let base = tip - a * 0.18 * self.length;
                    let r = 0.05 * self.length;
                    let ring: Vec<DVec3> = (0..=8)
                        .map(|k| {
                            let t = k as f64 / 8.0 * std::f64::consts::TAU;
                            base + (u * t.cos() + v * t.sin()) * r
                        })
                        .collect();
                    let tip_end = tip + a * 0.02 * self.length;
                    for w in ring.windows(2) {
                        out.tri(rel(w[0]), rel(w[1]), rel(tip_end), c);
                    }
                    let mut pc = col(Handle::Plane(i as u8), AXIS_COLORS[i]);
                    pc[3] = if hover == Some(Handle::Plane(i as u8)) {
                        200
                    } else {
                        110
                    };
                    let q = self.plane_corners(i);
                    out.tri(rel(q[0]), rel(q[1]), rel(q[2]), pc);
                    out.tri(rel(q[0]), rel(q[2]), rel(q[3]), pc);
                }
            }
            Mode::Rotate => {
                for i in 0..3 {
                    let c = col(Handle::Ring(i as u8), AXIS_COLORS[i]);
                    let pts = self.ring_points(i);
                    for w in pts.windows(2) {
                        out.line(rel(w[0]), rel(w[1]), c);
                    }
                }
            }
            Mode::Scale => {
                let c = col(Handle::Scale, SCALE_COLOR);
                for i in 0..3 {
                    let end = self.origin + self.axes[i] * self.length;
                    out.line(
                        rel(self.origin),
                        rel(end),
                        if hover == Some(Handle::Scale) {
                            HOVER_COLOR
                        } else {
                            AXIS_COLORS[i]
                        },
                    );
                    cube(
                        out,
                        end,
                        &self.axes,
                        0.05 * self.length,
                        AXIS_COLORS[i],
                        &rel,
                    );
                }
                cube(out, self.origin, &self.axes, 0.07 * self.length, c, &rel);
            }
        }
    }
}

fn cube(
    out: &mut DrawList,
    c: DVec3,
    axes: &[DVec3; 3],
    h: f64,
    col: [u8; 4],
    rel: &dyn Fn(DVec3) -> [f32; 3],
) {
    for i in 0..3 {
        let (n, u, v) = (axes[i], axes[(i + 1) % 3], axes[(i + 2) % 3]);
        for s in [-1.0, 1.0] {
            let f = c + n * h * s;
            let q = [
                f - u * h - v * h,
                f + u * h - v * h,
                f + u * h + v * h,
                f - u * h + v * h,
            ];
            out.tri(rel(q[0]), rel(q[1]), rel(q[2]), col);
            out.tri(rel(q[0]), rel(q[2]), rel(q[3]), col);
        }
    }
}

fn seg_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() > 0.0 {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a + ab * t - p).length()
}

fn in_quad(p: DVec2, q: &[DVec2]) -> bool {
    // Convex polygon, either winding.
    let mut sign = 0.0;
    for i in 0..q.len() {
        let (a, b) = (q[i], q[(i + 1) % q.len()]);
        let c = (b - a).perp_dot(p - a);
        if c != 0.0 {
            if sign != 0.0 && c.signum() != sign {
                return false;
            }
            sign = c.signum();
        }
    }
    true
}

/// One vertex of the gizmo overlay: camera-relative position, sRGB colour with alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawVertex {
    pub pos: [f32; 3],
    pub color: [u8; 4],
}

/// Line list (pairs) and triangle list (triples) for a debug/overlay pass, drawn without a
/// depth test so the gizmo is always visible.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawList {
    pub lines: Vec<DrawVertex>,
    pub triangles: Vec<DrawVertex>,
}

impl DrawList {
    pub fn clear(&mut self) {
        self.lines.clear();
        self.triangles.clear();
    }
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty() && self.triangles.is_empty()
    }
    pub fn line(&mut self, a: [f32; 3], b: [f32; 3], color: [u8; 4]) {
        self.lines
            .extend([DrawVertex { pos: a, color }, DrawVertex { pos: b, color }]);
    }
    pub fn tri(&mut self, a: [f32; 3], b: [f32; 3], c: [f32; 3], color: [u8; 4]) {
        self.triangles.extend([
            DrawVertex { pos: a, color },
            DrawVertex { pos: b, color },
            DrawVertex { pos: c, color },
        ]);
    }
}

/// A drag in progress: what was grabbed and where.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub handle: Handle,
    pub gizmo: Gizmo,
    /// Where the grab ray met the handle's constraint (axis parameter, plane point, ...).
    start: DragStart,
}

#[derive(Clone, Copy, Debug)]
enum DragStart {
    Axis(f64),
    Plane(DVec3),
    Ring(DVec3),
    Scale(f64),
}

/// Result of a drag update, applied to each selected object's start transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Delta {
    Translate(DVec3),
    /// Axis (unit, world) and angle in radians.
    Rotate(DVec3, f64),
    Scale(f64),
}

impl Drag {
    /// Start a drag on `handle`; `None` when the ray cannot meet the handle's constraint.
    pub fn begin(gizmo: Gizmo, handle: Handle, view: &ViewMath, ray: &Ray) -> Option<Drag> {
        let start = match handle {
            Handle::Axis(i) => {
                DragStart::Axis(ray.closest_on_line(gizmo.origin, gizmo.axes[i as usize])?)
            }
            Handle::Plane(i) => DragStart::Plane(ray.plane(gizmo.origin, gizmo.axes[i as usize])?),
            Handle::Ring(i) => {
                DragStart::Ring(ray.plane(gizmo.origin, gizmo.axes[i as usize])? - gizmo.origin)
            }
            Handle::Scale => {
                let d = (ray.plane(gizmo.origin, view.forward())? - gizmo.origin).length();
                DragStart::Scale(d.max(1e-9))
            }
        };
        Some(Drag {
            handle,
            gizmo,
            start,
        })
    }

    /// The delta from the drag start for the current ray, snapped. `None` keeps the last
    /// delta (the ray missed the constraint this frame).
    pub fn update(&self, view: &ViewMath, ray: &Ray, snap: &Snap) -> Option<Delta> {
        let g = &self.gizmo;
        match (self.handle, self.start) {
            (Handle::Axis(i), DragStart::Axis(t0)) => {
                let a = g.axes[i as usize];
                let t = ray.closest_on_line(g.origin, a)?;
                Some(Delta::Translate(a * snap.distance(t - t0)))
            }
            (Handle::Plane(i), DragStart::Plane(p0)) => {
                let p = ray.plane(g.origin, g.axes[i as usize])?;
                let d = p - p0;
                let (u, v) = (g.axes[(i as usize + 1) % 3], g.axes[(i as usize + 2) % 3]);
                Some(Delta::Translate(
                    u * snap.distance(d.dot(u)) + v * snap.distance(d.dot(v)),
                ))
            }
            (Handle::Ring(i), DragStart::Ring(v0)) => {
                let a = g.axes[i as usize];
                let v = ray.plane(g.origin, a)? - g.origin;
                let ang = a.dot(v0.cross(v)).atan2(v0.dot(v));
                Some(Delta::Rotate(a, snap.angle(ang)))
            }
            (Handle::Scale, DragStart::Scale(d0)) => {
                let d = (ray.plane(g.origin, view.forward())? - g.origin).length();
                Some(Delta::Scale(snap.scale(d / d0).max(1e-3)))
            }
            _ => None,
        }
    }
}

impl Delta {
    /// Apply to an object's transform at drag start. Rotation and scale act about the
    /// object's own position. `keep_upright` re-expresses the rotation at the new position so
    /// a translated object keeps its rotation relative to local east, north, up.
    pub fn apply(
        &self,
        start: &Transform,
        keep_upright: &dyn Fn(DVec3, DVec3, DQuat) -> DQuat,
    ) -> Transform {
        match *self {
            Delta::Translate(d) => {
                let position = start.position + d;
                Transform {
                    position,
                    rotation: keep_upright(start.position, position, start.rotation),
                    scale: start.scale,
                }
            }
            Delta::Rotate(axis, ang) => Transform {
                rotation: (DQuat::from_axis_angle(axis, ang) * start.rotation).normalize(),
                ..*start
            },
            Delta::Scale(s) => Transform {
                scale: start.scale * s,
                ..*start
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ANGLE_STEP;

    fn cam() -> Camera {
        Camera::look_at(
            DVec3::new(6_378_137.0, 10.0, -1_250_000.0 + 20.0),
            DVec3::new(6_378_137.0, 0.0, -1_250_000.0),
        )
    }

    #[test]
    fn project_matches_renderer_projection() {
        let c = cam();
        let v = ViewMath::new(&c, [1600, 900]);
        let p = DVec3::new(6_378_139.0, 1.5, -1_250_003.0);
        let px = v.project(p).unwrap();
        let clip = c.relative_view_proj(1600.0 / 900.0) * (p - c.position).as_vec3().extend(1.0);
        let ndc = clip.truncate() / clip.w;
        let expect = DVec2::new((ndc.x as f64 + 1.0) * 800.0, (1.0 - ndc.y as f64) * 450.0);
        assert!((px - expect).length() < 0.05, "{px} vs {expect}");
        // The ray through that pixel passes through the point.
        let r = v.ray(px);
        let s = (p - r.origin).dot(r.dir);
        assert!((r.at(s) - p).length() < 1e-3);
    }

    fn setup() -> (ViewMath, Gizmo) {
        let v = ViewMath::new(&cam(), [1600, 900]);
        let o = DVec3::new(6_378_137.0, 0.0, -1_250_000.0);
        (v, Gizmo::new(Mode::Translate, o, DQuat::IDENTITY, &v))
    }

    #[test]
    fn hit_axes_and_planes() {
        let (v, g) = setup();
        let x_mid = v.project(g.origin + g.axes[0] * g.length * 0.7).unwrap();
        assert_eq!(g.hit(&v, x_mid), Some(Handle::Axis(0)));
        let y_mid = v.project(g.origin + g.axes[1] * g.length * 0.7).unwrap();
        assert_eq!(
            g.hit(&v, y_mid + DVec2::new(3.0, 0.0)),
            Some(Handle::Axis(1))
        );
        let plane_c = v
            .project(g.origin + (g.axes[0] + g.axes[2]) * g.length * 0.35)
            .unwrap();
        assert_eq!(g.hit(&v, plane_c), Some(Handle::Plane(1)));
        assert_eq!(g.hit(&v, DVec2::new(5.0, 5.0)), None);
        // Length is constant on screen: ~GIZMO_PX pixels for an axis perpendicular to view.
        let len_px = (v.project(g.origin + g.axes[0] * g.length).unwrap()
            - v.project(g.origin).unwrap())
        .length();
        assert!((len_px - GIZMO_PX).abs() < 15.0, "{len_px}");
    }

    #[test]
    fn axis_drag_follows_the_cursor() {
        let (v, g) = setup();
        let p0 = v.project(g.origin + g.axes[0] * g.length * 0.6).unwrap();
        let d = Drag::begin(g, Handle::Axis(0), &v, &v.ray(p0)).unwrap();
        // Move the cursor to where the point 2.3 m further along X projects.
        let target = g.origin + g.axes[0] * (g.length * 0.6 + 2.3);
        let p1 = v.project(target).unwrap();
        let Delta::Translate(t) = d.update(&v, &v.ray(p1), &Snap::default()).unwrap() else {
            panic!()
        };
        assert!((t - DVec3::X * 2.3).length() < 1e-6, "{t}");
        let Delta::Translate(t) = d
            .update(
                &v,
                &v.ray(p1),
                &Snap {
                    enabled: true,
                    grid: 1,
                },
            )
            .unwrap()
        else {
            panic!()
        };
        assert!((t - DVec3::X * 2.5).length() < 1e-9);
        // Moving the cursor off-axis does not move the object off the axis.
        let Delta::Translate(t) = d
            .update(&v, &v.ray(p1 + DVec2::new(0.0, 40.0)), &Snap::default())
            .unwrap()
        else {
            panic!()
        };
        assert!(t.y.abs() < 1e-9 && t.z.abs() < 1e-9);
    }

    #[test]
    fn plane_drag_stays_in_plane() {
        let (v, g) = setup();
        let p0 = v
            .project(g.origin + (g.axes[0] + g.axes[2]) * g.length * 0.35)
            .unwrap();
        let d = Drag::begin(g, Handle::Plane(1), &v, &v.ray(p0)).unwrap();
        let Delta::Translate(t) = d
            .update(&v, &v.ray(p0 + DVec2::new(30.0, 20.0)), &Snap::default())
            .unwrap()
        else {
            panic!()
        };
        assert!(t.y.abs() < 1e-9 && t.length() > 0.1);
    }

    #[test]
    fn ring_drag_rotates_and_snaps() {
        let (v, mut g) = setup();
        g.mode = Mode::Rotate;
        // Grab the Y ring (rotation about up) at +X, drag to +(-Z) direction = 90 degrees about +Y.
        let a = v.project(g.origin + g.axes[0] * g.length).unwrap();
        assert!(matches!(g.hit(&v, a), Some(Handle::Ring(_))));
        let d = Drag::begin(g, Handle::Ring(1), &v, &v.ray(a)).unwrap();
        let ang = 0.5f64;
        let p = g.origin + (g.axes[0] * ang.cos() - g.axes[2] * ang.sin()) * g.length;
        let Delta::Rotate(axis, got) = d
            .update(&v, &v.ray(v.project(p).unwrap()), &Snap::default())
            .unwrap()
        else {
            panic!()
        };
        assert!((axis - DVec3::Y).length() < 1e-12);
        assert!((got - ang).abs() < 1e-6, "{got}");
        let Delta::Rotate(_, s) = d
            .update(
                &v,
                &v.ray(v.project(p).unwrap()),
                &Snap {
                    enabled: true,
                    grid: 0,
                },
            )
            .unwrap()
        else {
            panic!()
        };
        assert!((s - 2.0 * ANGLE_STEP).abs() < 1e-12);
    }

    #[test]
    fn scale_drag_and_apply() {
        let (v, mut g) = setup();
        g.mode = Mode::Scale;
        let o = v.project(g.origin).unwrap();
        assert_eq!(g.hit(&v, o), Some(Handle::Scale));
        let start_px = o + DVec2::new(50.0, 0.0);
        let d = Drag::begin(g, Handle::Scale, &v, &v.ray(start_px)).unwrap();
        let Delta::Scale(s) = d
            .update(&v, &v.ray(o + DVec2::new(100.0, 0.0)), &Snap::default())
            .unwrap()
        else {
            panic!()
        };
        assert!((s - 2.0).abs() < 0.02, "{s}");
        let t = Delta::Scale(2.0).apply(&Transform::IDENTITY, &|_, _, q| q);
        assert_eq!(t.scale, DVec3::splat(2.0));
    }

    #[test]
    fn draw_lists_are_camera_relative() {
        let (v, g) = setup();
        let mut dl = DrawList::default();
        g.draw(&v, Some(Handle::Axis(0)), &mut dl);
        assert!(!dl.lines.is_empty() && !dl.triangles.is_empty());
        assert_eq!(dl.lines.len() % 2, 0);
        assert_eq!(dl.triangles.len() % 3, 0);
        // Everything is within a few gizmo lengths of the camera, not at 6e6 m.
        let far = dl
            .lines
            .iter()
            .chain(&dl.triangles)
            .map(|p| glam::Vec3::from(p.pos).length())
            .fold(0.0f32, f32::max);
        assert!(far < 100.0, "{far}");
        assert!(dl.lines.iter().any(|p| p.color == HOVER_COLOR));
        for m in [Mode::Rotate, Mode::Scale] {
            let mut dl = DrawList::default();
            Gizmo { mode: m, ..g }.draw(&v, None, &mut dl);
            assert!(!dl.is_empty());
        }
    }
}
