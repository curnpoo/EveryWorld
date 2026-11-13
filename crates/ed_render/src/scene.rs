//! Scene inputs shared by every geometry provider (renderer.md §3): meshes with LOD chains,
//! materials, and the GPU instance pools with stable ids and dirty-range uploads.

use crate::gpu::{self, INSTANCE_ALIVE, INSTANCE_WORDS};
use ed_geo::ObjectId;
use glam::{Affine3A, DVec3, Vec3};

/// Most LOD levels a mesh may have.
pub const MAX_LODS: usize = 4;

/// One LOD level: triangle list into the mesh's vertices, and the geometric error of the level
/// against LOD 0 in mesh units (metres), as reported by the simplifier.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshLod {
    pub indices: Vec<u32>,
    pub error: f32,
}

/// A mesh with up to [`MAX_LODS`] levels sharing one vertex array. Triangles are CCW seen from
/// outside. `uvs` is empty (all zero) or one per vertex.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshAsset {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub lods: Vec<MeshLod>,
}

/// A skinned mesh: the bind-pose mesh plus up to four joint influences per vertex. Joint
/// indices refer to the palette given to [`crate::Renderer::set_skin_pose`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinnedMeshAsset {
    pub mesh: MeshAsset,
    pub joints: Vec<[u16; 4]>,
    pub weights: Vec<[f32; 4]>,
    pub joint_count: u32,
    /// Bounding-sphere radius multiplier covering every pose (culling uses the bind pose's
    /// sphere scaled by this; 1.5 is safe for limbs that stay within half the bind extent).
    pub pose_margin: f32,
}

/// Handle to a skinned mesh's per-instance output (its own vertex range, drawn as `mesh`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SkinId(pub u32);

/// Texel data of one texture: block-compressed or RGBA8, every mip level from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureAsset {
    pub format: ed_gpu::Format,
    pub width: u32,
    pub height: u32,
    pub mips: Vec<Vec<u8>>,
}

/// Handle to a texture added with [`crate::Renderer::add_texture`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextureHandle(pub u32);

/// Textures a material samples (none: the factors alone). Base colour is sRGB, normal is a
/// BC5 tangent-space XY map, ORM is linear R occlusion / G roughness / B metal (glTF layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaterialTextures {
    pub base: Option<TextureHandle>,
    pub normal: Option<TextureHandle>,
    pub orm: Option<TextureHandle>,
}

/// Texture-coordinate transform (KHR_texture_transform order: scale, rotate, offset).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvTransform {
    pub offset: glam::Vec2,
    pub scale: glam::Vec2,
    pub rotation: f32,
}

impl Default for UvTransform {
    fn default() -> Self {
        Self { offset: glam::Vec2::ZERO, scale: glam::Vec2::ONE, rotation: 0.0 }
    }
}

impl MeshAsset {
    /// Bounding sphere (centre of the box, farthest vertex).
    pub fn bounding_sphere(&self) -> (Vec3, f32) {
        if self.positions.is_empty() {
            return (Vec3::ZERO, 0.0);
        }
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in &self.positions {
            lo = lo.min(Vec3::from(*p));
            hi = hi.max(Vec3::from(*p));
        }
        let c = (lo + hi) * 0.5;
        let r = self.positions.iter().map(|p| Vec3::from(*p).distance(c)).fold(0.0f32, f32::max);
        (c, r)
    }
}

/// Handle to a mesh added with [`crate::Renderer::add_mesh`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MeshId(pub u32);

/// Handle to a material added with [`crate::Renderer::add_material`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MaterialId(pub u32);

/// Shading model. Both read the same lighting (one sun plus sky ambient).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialModel {
    /// GGX / Smith / Schlick, metallic-roughness.
    Pbr,
    /// Banded diffuse with a shadow colour and rim light.
    Toon,
}

/// Materials v0 (a row in the bindless material buffer).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub model: MaterialModel,
    /// Linear base colour.
    pub base_color: Vec3,
    pub metallic: f32,
    pub roughness: f32,
    /// Toon: shadow colour multiplier.
    pub shade_color: Vec3,
    /// Toon: number of light bands.
    pub bands: f32,
    /// Toon: rim strength.
    pub rim: f32,
    /// Node-space 2 m checker modulation.
    pub checker: bool,
    pub textures: MaterialTextures,
    pub uv: UvTransform,
}

impl Material {
    pub fn pbr(base_color: Vec3, metallic: f32, roughness: f32) -> Self {
        Self { model: MaterialModel::Pbr, base_color, metallic, roughness, shade_color: Vec3::splat(0.45), bands: 3.0, rim: 0.0, checker: false, textures: MaterialTextures::default(), uv: UvTransform::default() }
    }
    pub fn toon(base_color: Vec3, shade_color: Vec3, bands: f32, rim: f32) -> Self {
        Self { model: MaterialModel::Toon, base_color, metallic: 0.0, roughness: 1.0, shade_color, bands, rim, checker: false, textures: MaterialTextures::default(), uv: UvTransform::default() }
    }
    pub fn with_checker(mut self) -> Self {
        self.checker = true;
        self
    }
    pub fn with_textures(mut self, t: MaterialTextures) -> Self {
        self.textures = t;
        self
    }
    /// GPU row; `tex` maps a handle to its bindless index (`NO_TEXTURE` when absent).
    pub(crate) fn words(&self, tex: impl Fn(Option<TextureHandle>) -> u32) -> [u32; gpu::MATERIAL_WORDS] {
        let f = |v: f32| v.to_bits();
        let model = match self.model {
            MaterialModel::Pbr => 0.0,
            MaterialModel::Toon => 1.0,
        };
        [
            f(self.base_color.x),
            f(self.base_color.y),
            f(self.base_color.z),
            f(model),
            f(self.metallic),
            f(self.roughness),
            f(self.bands),
            f(if self.checker { 1.0 } else { 0.0 }),
            f(self.shade_color.x),
            f(self.shade_color.y),
            f(self.shade_color.z),
            f(self.rim),
            tex(self.textures.base),
            tex(self.textures.normal),
            tex(self.textures.orm),
            f(self.uv.rotation),
            f(self.uv.offset.x),
            f(self.uv.offset.y),
            f(self.uv.scale.x),
            f(self.uv.scale.y),
        ]
    }
}

/// A node origin (f64, CPU only). Instances store f32 transforms relative to their node; the
/// renderer uploads one camera-relative f32 origin per node and frame (P1-D7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeSlot(pub u16);

/// Stable handle to an instance. The slot index is also its GPU index; the generation guards
/// against stale handles after removal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InstanceId {
    pub index: u32,
    pub generation: u32,
}

/// What [`InstancePools::add`] takes.
#[derive(Clone, Copy, Debug)]
pub struct InstanceDesc {
    pub node: NodeSlot,
    /// Transform relative to the node origin.
    pub transform: Affine3A,
    pub mesh: MeshId,
    pub material: MaterialId,
    /// Owning object (picking returns it).
    pub object: ObjectId,
    /// Free flag bits (15 bits, stored above `ALIVE`).
    pub flags: u16,
}

/// The GPU instance pools: a CPU shadow of the instance buffer plus dirty tracking. Only slots
/// that changed are uploaded (sorted, merged into ranges); nothing loops over all instances per
/// frame. A moved instance keeps its old transform as "previous" for one frame, then settles.
pub struct InstancePools {
    nodes: Vec<DVec3>,
    words: Vec<u32>,
    objects: Vec<ObjectId>,
    generations: Vec<u32>,
    free: Vec<u32>,
    high_water: u32,
    alive: u32,
    capacity: u32,
    max_nodes: u32,
    dirty_bits: Vec<u64>,
    dirty: Vec<u32>,
    moved: Vec<u32>,
    settle: Vec<u32>,
    mesh_alive: Vec<u32>,
}

fn affine_rows(t: &Affine3A) -> [u32; 12] {
    let m = t.matrix3;
    let p = t.translation;
    let (c0, c1, c2) = (m.x_axis, m.y_axis, m.z_axis);
    [c0.x, c1.x, c2.x, p.x, c0.y, c1.y, c2.y, p.y, c0.z, c1.z, c2.z, p.z].map(f32::to_bits)
}

impl InstancePools {
    pub(crate) fn new(capacity: u32, max_nodes: u32) -> Self {
        assert!(capacity <= 1 << gpu::ENTRY_INSTANCE_BITS, "instance index must fit the visible-list entry");
        Self {
            nodes: Vec::new(),
            words: Vec::new(),
            objects: Vec::new(),
            generations: Vec::new(),
            free: Vec::new(),
            high_water: 0,
            alive: 0,
            capacity,
            max_nodes,
            dirty_bits: Vec::new(),
            dirty: Vec::new(),
            moved: Vec::new(),
            settle: Vec::new(),
            mesh_alive: Vec::new(),
        }
    }

    /// Adds a node origin. `None` when the node table is full.
    pub fn add_node(&mut self, origin: DVec3) -> Option<NodeSlot> {
        if self.nodes.len() as u32 >= self.max_nodes {
            return None;
        }
        self.nodes.push(origin);
        Some(NodeSlot(self.nodes.len() as u16 - 1))
    }
    /// Moves a node (all its instances move with it; no instance upload).
    pub fn set_node(&mut self, node: NodeSlot, origin: DVec3) {
        self.nodes[node.0 as usize] = origin;
    }
    pub fn nodes(&self) -> &[DVec3] {
        &self.nodes
    }

    fn mark(&mut self, i: u32) {
        let (w, b) = ((i / 64) as usize, i % 64);
        if self.dirty_bits.len() <= w {
            self.dirty_bits.resize(w + 1, 0);
        }
        if self.dirty_bits[w] & (1 << b) == 0 {
            self.dirty_bits[w] |= 1 << b;
            self.dirty.push(i);
        }
    }

    fn slot(&self, id: InstanceId) -> Option<usize> {
        let i = id.index as usize;
        (i < self.generations.len() && self.generations[i] == id.generation && self.words[i * INSTANCE_WORDS + gpu::INST_NODE_FLAGS] & INSTANCE_ALIVE != 0)
            .then_some(i)
    }

    /// Adds an instance. `None` when the pool is full.
    pub fn add(&mut self, d: &InstanceDesc) -> Option<InstanceId> {
        let i = match self.free.pop() {
            Some(i) => i,
            None => {
                if self.high_water >= self.capacity {
                    return None;
                }
                self.high_water += 1;
                self.words.resize(self.high_water as usize * INSTANCE_WORDS, 0);
                self.objects.push(ObjectId(0));
                self.generations.push(0);
                self.high_water - 1
            }
        };
        let w = &mut self.words[i as usize * INSTANCE_WORDS..(i as usize + 1) * INSTANCE_WORDS];
        let rows = affine_rows(&d.transform);
        w[..12].copy_from_slice(&rows);
        w[gpu::INST_PREV..gpu::INST_PREV + 12].copy_from_slice(&rows);
        w[gpu::INST_MESH] = d.mesh.0;
        w[gpu::INST_MATERIAL] = d.material.0;
        w[gpu::INST_OBJECT_LO] = d.object.0 as u32;
        w[gpu::INST_NODE_FLAGS] = d.node.0 as u32 | INSTANCE_ALIVE | (((d.flags & 0x7FFF) as u32) << 17);
        w[gpu::INST_USER] = 0;
        self.objects[i as usize] = d.object;
        self.generations[i as usize] = self.generations[i as usize].wrapping_add(1);
        self.alive += 1;
        let m = d.mesh.0 as usize;
        if self.mesh_alive.len() <= m {
            self.mesh_alive.resize(m + 1, 0);
        }
        self.mesh_alive[m] += 1;
        self.mark(i);
        Some(InstanceId { index: i, generation: self.generations[i as usize] })
    }

    /// New transform relative to the node. The previous transform is kept for one frame.
    pub fn set_transform(&mut self, id: InstanceId, t: &Affine3A) -> bool {
        let Some(i) = self.slot(id) else { return false };
        let rows = affine_rows(t);
        let w = &mut self.words[i * INSTANCE_WORDS..(i + 1) * INSTANCE_WORDS];
        let (cur, prev) = w.split_at_mut(gpu::INST_PREV);
        prev[..12].copy_from_slice(&cur[..12]);
        cur[..12].copy_from_slice(&rows);
        self.mark(i as u32);
        self.moved.push(i as u32);
        true
    }

    /// Sets the instance's free user word (`GpuInstance.pad[0]`; trees: bank.rs
    /// `InstanceParams::user`).
    pub fn set_user(&mut self, id: InstanceId, user: u32) -> bool {
        let Some(i) = self.slot(id) else { return false };
        self.words[i * INSTANCE_WORDS + gpu::INST_USER] = user;
        self.mark(i as u32);
        true
    }
    pub fn set_material(&mut self, id: InstanceId, material: MaterialId) -> bool {
        let Some(i) = self.slot(id) else { return false };
        self.words[i * INSTANCE_WORDS + gpu::INST_MATERIAL] = material.0;
        self.mark(i as u32);
        true
    }

    pub fn remove(&mut self, id: InstanceId) -> bool {
        let Some(i) = self.slot(id) else { return false };
        let w = &mut self.words[i * INSTANCE_WORDS..(i + 1) * INSTANCE_WORDS];
        let mesh = w[gpu::INST_MESH] as usize;
        w[gpu::INST_NODE_FLAGS] = 0;
        self.objects[i] = ObjectId(0);
        self.alive -= 1;
        self.mesh_alive[mesh] -= 1;
        self.free.push(i as u32);
        self.mark(i as u32);
        true
    }

    /// Live instances.
    pub fn alive(&self) -> u32 {
        self.alive
    }
    /// Slots in use or freed (the range the culling dispatch covers).
    pub fn slots(&self) -> u32 {
        self.high_water
    }
    pub fn capacity(&self) -> u32 {
        self.capacity
    }
    /// The object of the live instance in GPU slot `index`.
    pub fn object(&self, index: u32) -> Option<ObjectId> {
        let i = index as usize;
        (i < self.objects.len() && self.words[i * INSTANCE_WORDS + gpu::INST_NODE_FLAGS] & INSTANCE_ALIVE != 0).then(|| self.objects[i])
    }
    pub(crate) fn mesh_alive(&self, mesh: usize) -> u32 {
        self.mesh_alive.get(mesh).copied().unwrap_or(0)
    }
    /// Slots waiting for upload.
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }

    /// Settles last frame's moves (previous = current) and returns the dirty slots as sorted,
    /// merged ranges. Clears the dirty set. Cost is proportional to the changes only.
    pub(crate) fn take_dirty_ranges(&mut self) -> Vec<std::ops::Range<u32>> {
        let settle = std::mem::take(&mut self.settle);
        for i in settle {
            let i = i as usize;
            if self.words[i * INSTANCE_WORDS + gpu::INST_NODE_FLAGS] & INSTANCE_ALIVE != 0 {
                let w = &mut self.words[i * INSTANCE_WORDS..(i + 1) * INSTANCE_WORDS];
                let (cur, prev) = w.split_at_mut(gpu::INST_PREV);
                prev[..12].copy_from_slice(&cur[..12]);
                self.mark(i as u32);
            }
        }
        self.settle = std::mem::take(&mut self.moved);
        let mut d = std::mem::take(&mut self.dirty);
        for &i in &d {
            self.dirty_bits[(i / 64) as usize] &= !(1u64 << (i % 64));
        }
        d.sort_unstable();
        let mut out: Vec<std::ops::Range<u32>> = Vec::new();
        for i in d {
            match out.last_mut() {
                // Merge across small gaps too: one bigger copy beats many tiny ones.
                Some(r) if i <= r.end + 4 => r.end = i + 1,
                _ => out.push(i..i + 1),
            }
        }
        out
    }

    pub(crate) fn range_words(&self, r: &std::ops::Range<u32>) -> &[u32] {
        &self.words[r.start as usize * INSTANCE_WORDS..r.end as usize * INSTANCE_WORDS]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(mesh: u32) -> InstanceDesc {
        InstanceDesc {
            node: NodeSlot(0),
            transform: Affine3A::from_translation(Vec3::new(1.0, 2.0, 3.0)),
            mesh: MeshId(mesh),
            material: MaterialId(0),
            object: ObjectId(0xABCD_0000_0000_0000_0000_0000_1234_5678),
            flags: 0,
        }
    }

    #[test]
    fn ids_are_stable_and_generational() {
        let mut p = InstancePools::new(16, 4);
        let a = p.add(&desc(0)).unwrap();
        let b = p.add(&desc(1)).unwrap();
        assert!(p.remove(a));
        assert!(!p.remove(a), "stale handle");
        let c = p.add(&desc(0)).unwrap();
        assert_eq!(c.index, a.index, "slot reused");
        assert_ne!(c.generation, a.generation);
        assert!(!p.set_transform(a, &Affine3A::IDENTITY));
        assert!(p.set_transform(b, &Affine3A::IDENTITY));
        assert_eq!(p.alive(), 2);
        assert_eq!(p.object(b.index), Some(desc(1).object));
    }

    #[test]
    fn dirty_ranges_merge_and_settle() {
        let mut p = InstancePools::new(1024, 4);
        let ids: Vec<_> = (0..100).map(|_| p.add(&desc(0)).unwrap()).collect();
        assert_eq!(p.take_dirty_ranges(), vec![0..100]);
        assert!(p.take_dirty_ranges().is_empty(), "nothing changed");
        p.set_transform(ids[10], &Affine3A::IDENTITY);
        p.set_transform(ids[12], &Affine3A::IDENTITY);
        p.set_transform(ids[70], &Affine3A::IDENTITY);
        assert_eq!(p.take_dirty_ranges(), vec![10..13, 70..71]);
        // Next frame: previous = current for the moved ones.
        assert_eq!(p.take_dirty_ranges(), vec![10..13, 70..71]);
        let w = p.range_words(&(10..11));
        assert_eq!(&w[..12], &w[gpu::INST_PREV..gpu::INST_PREV + 12]);
        assert!(p.take_dirty_ranges().is_empty());
    }
}
