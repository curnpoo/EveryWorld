//! Ground collision from the pages the renderer draws (P-slice S3).
//!
//! Static mesh bodies for the 3 x 3 nodes at [`COLLISION_LEVEL`] around the player, built with
//! [`ed_planet::collide::node_surface`] (the render grid rule, sim frame). A node whose page is
//! not resident is replaced by its nearest resident ancestor (coarser, so seams are possible
//! until the finer page arrives). Bodies are added and removed in sorted `NodeId` order, so the
//! entity numbering depends only on the player's path and the residency answers.

use ed_geo::NodeId;
use ed_planet::collide::node_surface;
use ed_planet::{Anchor, Residency};
use ed_sim::{BodyDesc, BodyKind, Entity, GeomId, Geometry, Shape, ShapeDesc, TriMesh, World};
use glam::DVec3;
use std::collections::BTreeMap;

/// Collision node level: a level-16 node on Earth is ~153 m wide, so the 64-cell render grid is
/// ~2.4 m, and a 3 x 3 block reaches at least ~150 m from the player in every direction.
pub const COLLISION_LEVEL: u8 = 16;
/// Render grid vertices used (1 = all of them, exactly what is drawn).
pub const STEP: u32 = 1;

#[derive(Default)]
pub struct TerrainCollision {
    /// Live static bodies by node.
    live: BTreeMap<NodeId, Entity>,
    /// Registered geometry by node (the sim never frees geometry, so each node is added once).
    geoms: BTreeMap<NodeId, (GeomId, DVec3)>,
}

impl TerrainCollision {
    pub fn new() -> TerrainCollision {
        TerrainCollision::default()
    }

    /// Nodes that currently have a body, sorted.
    pub fn nodes(&self) -> Vec<NodeId> {
        self.live.keys().copied().collect()
    }

    /// The nodes that should be collidable for a player at sim position `player`.
    pub fn wanted(anchor: &Anchor, res: &dyn Residency, player: DVec3) -> Vec<NodeId> {
        let dir = anchor.sim_to_body(player).normalize();
        let centre = NodeId::from_unit(anchor.body.id, dir, COLLISION_LEVEL);
        let mut out = Vec::new();
        for dy in -1..=1 {
            for dx in -1..=1 {
                let mut id = centre.neighbour(dx, dy);
                // Nearest resident ancestor (or the node itself).
                loop {
                    if res.height_page(id).is_some() {
                        out.push(id);
                        break;
                    }
                    match id.parent() {
                        Some(p) => id = p,
                        None => break,
                    }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Brings the bodies in line with the player's position. Returns the bodies added/removed.
    pub fn update(&mut self, world: &mut World, anchor: &Anchor, res: &dyn Residency, player: DVec3) -> (usize, usize) {
        let want = Self::wanted(anchor, res, player);
        let drop: Vec<NodeId> = self.live.keys().filter(|k| want.binary_search(k).is_err()).copied().collect();
        let mut removed = 0;
        for id in drop {
            if let Some(e) = self.live.remove(&id) {
                world.despawn(e);
                removed += 1;
            }
        }
        let mut added = 0;
        for id in want {
            if self.live.contains_key(&id) {
                continue;
            }
            let Some(page) = res.height_page(id) else { continue };
            let (g, origin) = match self.geoms.get(&id) {
                Some(&v) => v,
                None => {
                    let (origin, vertices, triangles) = node_surface(anchor, id, &page, STEP);
                    let Ok(g) = world.add_geometry(Geometry::Mesh(TriMesh { vertices, triangles })) else { continue };
                    self.geoms.insert(id, (g, origin));
                    (g, origin)
                }
            };
            let mut desc = BodyDesc::new(BodyKind::Static, origin);
            desc.shapes = vec![ShapeDesc::new(Shape::Mesh(g))];
            if let Ok(e) = world.create_body(desc) {
                self.live.insert(id, e);
                added += 1;
            }
        }
        (added, removed)
    }
}
