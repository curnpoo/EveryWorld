//! Floating origin. The render world is a local East-North-Up frame around
//! the player; when they wander more than a few km away, we re-anchor the
//! frame under them and move everything, so `f32` never runs out of
//! precision anywhere on Earth (or on the way to the Moon).
//!
//! - Static world content carries [`GeoAnchored`]: its true placement in ECEF
//!   (f64). Its `Transform` is derived from that whenever the origin moves.
//! - Moving things (player, vehicles, camera) carry [`Rebased`] and are
//!   transformed rigidly from the old frame into the new one.

use avian3d::prelude::*;
use bevy::prelude::*;
use ed_geo::{EnuFrame, Geodetic};
use glam::{DAffine3, DMat4, DVec3};

pub struct OriginPlugin;

impl Plugin for OriginPlugin {
    fn build(&self, app: &mut App) {
        let cfg = app.world().resource::<crate::config::Config>().clone();
        let start = Geodetic { alt_m: 0.0, ..cfg.start };
        app.insert_resource(WorldOrigin { frame: EnuFrame::new(start), generation: 0 })
            .add_systems(PostUpdate, (rebase_if_far, apply_anchors).chain().before(TransformSystems::Propagate));
    }
}

#[derive(Resource)]
pub struct WorldOrigin {
    pub frame: EnuFrame,
    pub generation: u64,
}

impl WorldOrigin {
    pub fn to_geodetic(&self, p: Vec3) -> Geodetic {
        self.frame.render_to_geodetic(p.as_dvec3())
    }
    pub fn to_render(&self, g: Geodetic) -> Vec3 {
        self.frame.geodetic_to_render(g).as_vec3()
    }
    pub fn ecef_to_render(&self, p: DVec3) -> DVec3 {
        self.frame.ecef_to_render().transform_point3(p)
    }
    /// Local up at a render-space point (tilts with Earth's curvature).
    pub fn up_at(&self, p: Vec3) -> Vec3 {
        self.frame.up_at_render(p.as_dvec3()).as_vec3()
    }
}

/// Placement of a static entity: local space -> ECEF.
#[derive(Component, Clone, Copy)]
pub struct GeoAnchored(pub DAffine3);

impl GeoAnchored {
    /// An entity built in `frame`'s render space, offset by `anchor`.
    pub fn from_render(frame: &EnuFrame, anchor: DVec3) -> Self {
        Self(frame.render_to_ecef() * DAffine3::from_translation(anchor))
    }
}

/// Moving entity that must be carried across rebases.
#[derive(Component)]
pub struct Rebased;

/// The entity whose position drives streaming and rebasing.
#[derive(Component)]
pub struct Focus;

pub const REBASE_DISTANCE_M: f32 = 16_000.0;

fn affine_to_transform(a: DAffine3) -> Transform {
    Transform::from_matrix(DMat4::from(a).as_mat4())
}

fn apply_anchors(origin: Res<WorldOrigin>, mut q: Query<(Ref<GeoAnchored>, &mut Transform)>) {
    let to_render = origin.frame.ecef_to_render();
    let all = origin.is_changed();
    let t0 = std::time::Instant::now();
    let mut moved = 0usize;
    for (anchor, mut t) in &mut q {
        if all || anchor.is_changed() {
            *t = affine_to_transform(to_render * anchor.0);
            moved += 1;
        }
    }
    // A teleport rewrites every static transform; Avian then re-syncs their colliders.
    if all && moved > 0 && t0.elapsed().as_millis() > 30 {
        warn!("apply_anchors moved {moved} statics in {} ms", t0.elapsed().as_millis());
    }
}

#[allow(clippy::type_complexity)]
fn rebase_if_far(
    mut origin: ResMut<WorldOrigin>,
    focus_static: Query<&Transform, (With<Focus>, Without<Rebased>)>,
    mut movers: Query<
        (&mut Transform, Has<Focus>, Option<&mut Position>, Option<&mut Rotation>, Option<&mut LinearVelocity>, Option<&mut AngularVelocity>),
        (With<Rebased>, Without<GeoAnchored>),
    >,
) {
    let focus = movers.iter().find(|m| m.1).map(|m| m.0.translation).or_else(|| focus_static.iter().next().map(|t| t.translation));
    let Some(p) = focus else { return };
    let horiz = Vec2::new(p.x, p.z).length();
    if horiz < REBASE_DISTANCE_M {
        return;
    }
    let g = origin.to_geodetic(p);
    let new_frame = EnuFrame::new(Geodetic { alt_m: 0.0, ..g });
    // old render -> ECEF -> new render
    let m: DAffine3 = new_frame.ecef_to_render() * origin.frame.render_to_ecef();
    let rot = Quat::from_mat3(&m.matrix3.as_mat3());
    for (mut t, _, pos, r, lv, av) in &mut movers {
        t.translation = m.transform_point3(t.translation.as_dvec3()).as_vec3();
        t.rotation = (rot * t.rotation).normalize();
        if let Some(mut pos) = pos {
            pos.0 = t.translation;
        }
        if let Some(mut r) = r {
            r.0 = t.rotation;
        }
        if let Some(mut v) = lv {
            v.0 = rot * v.0;
        }
        if let Some(mut w) = av {
            w.0 = rot * w.0;
        }
    }
    info!("rebased origin to {:.5}, {:.5}", g.lat_deg, g.lon_deg);
    origin.frame = new_frame;
    origin.generation += 1;
}
