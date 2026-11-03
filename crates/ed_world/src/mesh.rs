//! Plain mesh buffers. The game converts these into `bevy::Mesh`; keeping
//! them engine-free means generation is unit-testable and runs on any thread.

#[derive(Debug, Clone, Default)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn vertex(&mut self, p: [f32; 3], n: [f32; 3], uv: [f32; 2], c: [f32; 4]) -> u32 {
        let i = self.positions.len() as u32;
        self.positions.push(p);
        self.normals.push(n);
        self.uvs.push(uv);
        self.colors.push(c);
        i
    }

    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    /// Flat-shaded quad from four corners in counter-clockwise order (as seen
    /// from the front), with per-corner UVs.
    pub fn quad(&mut self, p: [glam::Vec3; 4], uv: [[f32; 2]; 4], c: [f32; 4]) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero().to_array();
        let i: Vec<u32> = (0..4).map(|k| self.vertex(p[k].to_array(), n, uv[k], c)).collect();
        self.tri(i[0], i[1], i[2]);
        self.tri(i[0], i[2], i[3]);
    }

    /// Flat-shaded triangle (counter-clockwise front).
    pub fn triangle(&mut self, p: [glam::Vec3; 3], uv: [[f32; 2]; 3], c: [f32; 4]) {
        let n = (p[1] - p[0]).cross(p[2] - p[0]).normalize_or_zero().to_array();
        let a = self.vertex(p[0].to_array(), n, uv[0], c);
        let b = self.vertex(p[1].to_array(), n, uv[1], c);
        let d = self.vertex(p[2].to_array(), n, uv[2], c);
        self.tri(a, b, d);
    }

    pub fn append(&mut self, other: &MeshData) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.uvs.extend_from_slice(&other.uvs);
        self.colors.extend_from_slice(&other.colors);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// sRGB hex -> linear RGBA (vertex colors are linear).
pub fn rgb(hex: u32) -> [f32; 4] {
    let c = |s: u32| {
        let v = ((hex >> s) & 0xff) as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    [c(16), c(8), c(0), 1.0]
}

pub fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t, a[3] + (b[3] - a[3]) * t]
}

pub fn scale_color(a: [f32; 4], k: f32) -> [f32; 4] {
    [a[0] * k, a[1] * k, a[2] * k, a[3]]
}
