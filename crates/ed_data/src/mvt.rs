//! Minimal Mapbox Vector Tile (MVT v2) decoder. MVT is just protobuf with a
//! tiny geometry command language, so we decode it by hand: no codegen, no
//! extra dependencies, and it's fast enough to run per tile on a worker.
//!
//! Spec: https://github.com/mapbox/vector-tile-spec/tree/master/2.1

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Float(f32),
    Double(f64),
    Int(i64),
    UInt(u64),
    Bool(bool),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match *self {
            Value::Float(v) => Some(v as f64),
            Value::Double(v) => Some(v),
            Value::Int(v) => Some(v as f64),
            Value::UInt(v) => Some(v as f64),
            Value::Bool(_) | Value::String(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeomType {
    Unknown,
    Point,
    LineString,
    Polygon,
}

#[derive(Debug, Clone)]
pub struct Feature {
    pub id: Option<u64>,
    pub geom_type: GeomType,
    pub props: HashMap<String, Value>,
    /// Decoded geometry: for points, each ring is one point; for lines, each
    /// ring is a linestring; for polygons, rings are exterior (clockwise in
    /// tile space, positive area) followed by their holes.
    pub rings: Vec<Vec<[f32; 2]>>,
}

impl Feature {
    pub fn str(&self, k: &str) -> Option<&str> {
        self.props.get(k).and_then(Value::as_str)
    }
    pub fn num(&self, k: &str) -> Option<f64> {
        self.props.get(k).and_then(Value::as_f64)
    }
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub extent: u32,
    pub features: Vec<Feature>,
}

#[derive(Debug, Clone, Default)]
pub struct Tile {
    pub layers: Vec<Layer>,
}

impl Tile {
    pub fn layer(&self, name: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.name == name)
    }

    /// Decode a tile. Transparently gunzips if the bytes are gzip-compressed.
    pub fn decode(bytes: &[u8]) -> anyhow::Result<Tile> {
        if bytes.starts_with(&[0x1f, 0x8b]) {
            use std::io::Read;
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(bytes).read_to_end(&mut out)?;
            return Self::decode_raw(&out);
        }
        Self::decode_raw(bytes)
    }

    fn decode_raw(bytes: &[u8]) -> anyhow::Result<Tile> {
        let mut r = Pb::new(bytes);
        let mut tile = Tile::default();
        while let Some((field, wt)) = r.key()? {
            match (field, wt) {
                (3, 2) => tile.layers.push(decode_layer(r.bytes()?)?),
                _ => r.skip(wt)?,
            }
        }
        Ok(tile)
    }
}

fn decode_layer(bytes: &[u8]) -> anyhow::Result<Layer> {
    let mut r = Pb::new(bytes);
    let mut name = String::new();
    let mut extent = 4096;
    let mut keys = Vec::new();
    let mut values = Vec::new();
    let mut raw_features = Vec::new();
    while let Some((field, wt)) = r.key()? {
        match (field, wt) {
            (1, 2) => name = String::from_utf8_lossy(r.bytes()?).into_owned(),
            (2, 2) => raw_features.push(r.bytes()?),
            (3, 2) => keys.push(String::from_utf8_lossy(r.bytes()?).into_owned()),
            (4, 2) => values.push(decode_value(r.bytes()?)?),
            (5, 0) => extent = r.varint()? as u32,
            _ => r.skip(wt)?,
        }
    }
    let features = raw_features
        .into_iter()
        .map(|b| decode_feature(b, &keys, &values))
        .collect::<anyhow::Result<_>>()?;
    Ok(Layer { name, extent, features })
}

fn decode_value(bytes: &[u8]) -> anyhow::Result<Value> {
    let mut r = Pb::new(bytes);
    let mut v = Value::Bool(false);
    while let Some((field, wt)) = r.key()? {
        v = match (field, wt) {
            (1, 2) => Value::String(String::from_utf8_lossy(r.bytes()?).into_owned()),
            (2, 5) => Value::Float(f32::from_le_bytes(r.fixed::<4>()?)),
            (3, 1) => Value::Double(f64::from_le_bytes(r.fixed::<8>()?)),
            (4, 0) => Value::Int(r.varint()? as i64),
            (5, 0) => Value::UInt(r.varint()?),
            (6, 0) => Value::Int(zigzag(r.varint()?)),
            (7, 0) => Value::Bool(r.varint()? != 0),
            _ => {
                r.skip(wt)?;
                continue;
            }
        };
    }
    Ok(v)
}

fn decode_feature(bytes: &[u8], keys: &[String], values: &[Value]) -> anyhow::Result<Feature> {
    let mut r = Pb::new(bytes);
    let mut f = Feature { id: None, geom_type: GeomType::Unknown, props: HashMap::new(), rings: Vec::new() };
    let mut geom: Vec<u32> = Vec::new();
    while let Some((field, wt)) = r.key()? {
        match (field, wt) {
            (1, 0) => f.id = Some(r.varint()?),
            (2, 2) => {
                let mut p = Pb::new(r.bytes()?);
                while !p.done() {
                    let k = p.varint()? as usize;
                    let v = p.varint()? as usize;
                    if let (Some(k), Some(v)) = (keys.get(k), values.get(v)) {
                        f.props.insert(k.clone(), v.clone());
                    }
                }
            }
            (3, 0) => {
                f.geom_type = match r.varint()? {
                    1 => GeomType::Point,
                    2 => GeomType::LineString,
                    3 => GeomType::Polygon,
                    _ => GeomType::Unknown,
                }
            }
            (4, 2) => {
                let mut p = Pb::new(r.bytes()?);
                while !p.done() {
                    geom.push(p.varint()? as u32);
                }
            }
            _ => r.skip(wt)?,
        }
    }
    f.rings = decode_geometry(&geom, f.geom_type);
    Ok(f)
}

fn decode_geometry(cmds: &[u32], ty: GeomType) -> Vec<Vec<[f32; 2]>> {
    let mut rings = Vec::new();
    let mut cur: Vec<[f32; 2]> = Vec::new();
    let (mut x, mut y) = (0i64, 0i64);
    let mut i = 0;
    while i < cmds.len() {
        let c = cmds[i];
        i += 1;
        let (id, count) = (c & 7, c >> 3);
        match id {
            1 | 2 => {
                for _ in 0..count {
                    if i + 1 >= cmds.len() {
                        break;
                    }
                    x += zigzag(cmds[i] as u64);
                    y += zigzag(cmds[i + 1] as u64);
                    i += 2;
                    if id == 1 {
                        if !cur.is_empty() {
                            rings.push(std::mem::take(&mut cur));
                        }
                        if ty == GeomType::Point {
                            rings.push(vec![[x as f32, y as f32]]);
                            continue;
                        }
                    }
                    cur.push([x as f32, y as f32]);
                }
            }
            7 => {
                if !cur.is_empty() {
                    rings.push(std::mem::take(&mut cur));
                }
            }
            _ => break,
        }
    }
    if !cur.is_empty() {
        rings.push(cur);
    }
    rings
}

/// Signed area in tile coordinates (y down). MVT exterior rings are
/// clockwise on screen, which is *positive* with this formula.
pub fn ring_area(ring: &[[f32; 2]]) -> f32 {
    let mut a = 0.0;
    for i in 0..ring.len() {
        let [x0, y0] = ring[i];
        let [x1, y1] = ring[(i + 1) % ring.len()];
        a += x0 * y1 - x1 * y0;
    }
    a * 0.5
}

/// Split polygon rings into (exterior, holes) groups.
pub fn polygons(f: &Feature) -> Vec<(Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>)> {
    let mut out: Vec<(Vec<[f32; 2]>, Vec<Vec<[f32; 2]>>)> = Vec::new();
    for ring in &f.rings {
        if ring.len() < 3 {
            continue;
        }
        if ring_area(ring) > 0.0 || out.is_empty() {
            out.push((ring.clone(), Vec::new()));
        } else if let Some(last) = out.last_mut() {
            last.1.push(ring.clone());
        }
    }
    out
}

fn zigzag(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

/// Tiny protobuf wire-format reader.
struct Pb<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Pb<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }
    fn varint(&mut self) -> anyhow::Result<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *self.buf.get(self.pos).ok_or_else(|| anyhow::anyhow!("truncated varint"))?;
            self.pos += 1;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        anyhow::bail!("varint too long")
    }
    fn key(&mut self) -> anyhow::Result<Option<(u32, u8)>> {
        if self.done() {
            return Ok(None);
        }
        let k = self.varint()?;
        Ok(Some(((k >> 3) as u32, (k & 7) as u8)))
    }
    fn bytes(&mut self) -> anyhow::Result<&'a [u8]> {
        let len = self.varint()? as usize;
        let end = self.pos.checked_add(len).filter(|&e| e <= self.buf.len()).ok_or_else(|| anyhow::anyhow!("truncated bytes"))?;
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn fixed<const N: usize>(&mut self) -> anyhow::Result<[u8; N]> {
        let s = self.buf.get(self.pos..self.pos + N).ok_or_else(|| anyhow::anyhow!("truncated fixed"))?;
        self.pos += N;
        Ok(s.try_into().unwrap())
    }
    fn skip(&mut self, wt: u8) -> anyhow::Result<()> {
        match wt {
            0 => {
                self.varint()?;
            }
            1 => self.pos += 8,
            2 => {
                self.bytes()?;
            }
            5 => self.pos += 4,
            _ => anyhow::bail!("unsupported wire type {wt}"),
        }
        anyhow::ensure!(self.pos <= self.buf.len(), "truncated");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn varint(mut v: u64, out: &mut Vec<u8>) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                out.push(b);
                break;
            }
            out.push(b | 0x80);
        }
    }
    fn field_bytes(field: u32, data: &[u8], out: &mut Vec<u8>) {
        varint(((field as u64) << 3) | 2, out);
        varint(data.len() as u64, out);
        out.extend_from_slice(data);
    }
    fn zz(v: i64) -> u64 {
        ((v << 1) ^ (v >> 63)) as u64
    }

    #[test]
    fn decodes_square_building() {
        // geometry: MoveTo(10,10) LineTo(+10,0)(0,+10)(-10,0) ClosePath
        let cmds: Vec<u64> = vec![9, zz(10), zz(10), (3 << 3) | 2, zz(10), 0, 0, zz(10), zz(-10), 0, 15];
        let mut geom = Vec::new();
        for c in cmds {
            varint(c, &mut geom);
        }
        let mut tags = Vec::new();
        varint(0, &mut tags);
        varint(0, &mut tags);

        let mut feat = Vec::new();
        varint((1 << 3) | 0, &mut feat);
        varint(42, &mut feat);
        field_bytes(2, &tags, &mut feat);
        varint((3 << 3) | 0, &mut feat);
        varint(3, &mut feat);
        field_bytes(4, &geom, &mut feat);

        let mut val = Vec::new();
        varint((5 << 3) | 0, &mut val);
        varint(25, &mut val);

        let mut layer = Vec::new();
        field_bytes(1, b"building", &mut layer);
        field_bytes(2, &feat, &mut layer);
        field_bytes(3, b"render_height", &mut layer);
        field_bytes(4, &val, &mut layer);
        varint((5 << 3) | 0, &mut layer);
        varint(4096, &mut layer);

        let mut tile = Vec::new();
        field_bytes(3, &layer, &mut tile);

        let t = Tile::decode(&tile).unwrap();
        let l = t.layer("building").unwrap();
        assert_eq!(l.extent, 4096);
        let f = &l.features[0];
        assert_eq!(f.id, Some(42));
        assert_eq!(f.geom_type, GeomType::Polygon);
        assert_eq!(f.num("render_height"), Some(25.0));
        assert_eq!(f.rings, vec![vec![[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]]]);
        assert!(ring_area(&f.rings[0]) > 0.0);
        assert_eq!(polygons(f).len(), 1);
    }
}
