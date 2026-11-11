//! Override layers (`docs/contracts/layers.md`): the TOML reader and the canonical writer.
//!
//! One file per L10 node per layer directory (`layers/<layer>/<body>/<face>/10/<x>/<y>.toml`),
//! and `layers/layers.toml` for order and default state. `place`, `replace`, `suppress` and
//! `marker` are fully typed; `mission`, `road_patch` and `terrain_patch` are parsed and kept as
//! TOML values until their applications land.
//!
//! **Canonical form** (what [`LayerFile::to_toml`] writes, and what round-trips byte for byte):
//! - file-level keys other than `op` first, sorted, then a blank line;
//! - operations sorted by `id`, each as `[[op]]` followed by `id`, `op`, then the operation's
//!   fields in the contract's table order (unknown-op fields sorted by name), one blank line
//!   between operations, `\n` line endings, a final newline;
//! - floats in Rust's shortest round-trip form with at least one decimal (`1.0`, `0.3827`),
//!   nested tables and arrays inline (`{ a = 1, b = 2 }`, keys sorted), basic strings.
//!
//! Every parse error carries the 1-based line it refers to.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use ed_geo::{BodyId, NodeId, ObjectId};
use serde::Deserialize;
use toml::{Spanned, Value};

/// Override layer files group operations by L10 node.
pub const LAYER_NODE_LEVEL: u8 = 10;

pub type Vec3 = [f64; 3];
/// Quaternion `x, y, z, w`, relative to local east, north, up at the position.
pub type Quat = [f64; 4];

#[derive(Clone, Debug, PartialEq)]
pub enum Scale {
    Uniform(f64),
    Axes(Vec3),
}

/// `volume`: a box, a sphere or a polygon prism. Written as an inline table with `shape` first.
#[derive(Clone, Debug, PartialEq)]
pub enum Volume {
    /// `{ shape = "box", centre, half_extents, rotation? }`
    Box { centre: Vec3, half_extents: Vec3, rotation: Option<Quat> },
    /// `{ shape = "sphere", centre, radius }`
    Sphere { centre: Vec3, radius: f64 },
    /// `{ shape = "prism", points, height }`: base polygon (body-fixed points) extruded `height`
    /// metres along local up.
    Prism { points: Vec<Vec3>, height: f64 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub prefab: String,
    pub position: Vec3,
    pub rotation: Option<Quat>,
    pub scale: Option<Scale>,
    /// Footprint margin in metres; `None` = the prefab's default.
    pub clear: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ReplaceParams {
    pub style: Option<String>,
    pub height: Option<f64>,
    pub colour: Option<String>,
    pub roof: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReplaceWith {
    Prefab(String),
    Params(ReplaceParams),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Replace {
    pub target: ObjectId,
    pub with: ReplaceWith,
}

/// At least one of `targets` and `volume`.
#[derive(Clone, Debug, PartialEq)]
pub struct Suppress {
    pub targets: Option<Vec<ObjectId>>,
    pub volume: Option<Volume>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub name: String,
    /// `spawn`, `landing_pad`, `poi`, `trigger`, ... (open set).
    pub kind: String,
    pub position: Vec3,
    pub rotation: Option<Quat>,
    pub volume: Option<Volume>,
    pub tags: Option<Vec<String>>,
}

/// Operations that are parsed and preserved but not yet typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum OtherOp {
    Mission,
    RoadPatch,
    TerrainPatch,
}

impl OtherOp {
    pub fn name(self) -> &'static str {
        match self {
            OtherOp::Mission => "mission",
            OtherOp::RoadPatch => "road_patch",
            OtherOp::TerrainPatch => "terrain_patch",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpBody {
    Place(Place),
    Replace(Replace),
    Suppress(Suppress),
    Marker(Marker),
    Other { op: OtherOp, fields: BTreeMap<String, Value> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    /// Authored `ObjectId` (top bit set).
    pub id: ObjectId,
    pub body: OpBody,
}

impl Op {
    pub fn name(&self) -> &'static str {
        match &self.body {
            OpBody::Place(_) => "place",
            OpBody::Replace(_) => "replace",
            OpBody::Suppress(_) => "suppress",
            OpBody::Marker(_) => "marker",
            OpBody::Other { op, .. } => op.name(),
        }
    }
    /// The body-fixed anchor position, for operations that have one. The file an operation
    /// lives in is the L10 node containing this point.
    pub fn anchor(&self) -> Option<Vec3> {
        match &self.body {
            OpBody::Place(p) => Some(p.position),
            OpBody::Marker(m) => Some(m.position),
            OpBody::Suppress(Suppress { volume: Some(v), .. }) => Some(match v {
                Volume::Box { centre, .. } | Volume::Sphere { centre, .. } => *centre,
                Volume::Prism { points, .. } => points.first().copied()?,
            }),
            _ => None,
        }
    }
}

/// One layer file (one L10 node of one layer).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LayerFile {
    /// File-level keys other than `op`, preserved as-is.
    pub header: BTreeMap<String, Value>,
    pub ops: Vec<Op>,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
#[error("line {line}: {kind}")]
pub struct LayerError {
    /// 1-based line in the source text (0 when unknown).
    pub line: usize,
    pub kind: LayerErrorKind,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum LayerErrorKind {
    #[error("TOML syntax: {0}")]
    Syntax(String),
    #[error("operation `{op}` is missing field `{field}`")]
    MissingField { op: String, field: &'static str },
    #[error("operation `{op}` has unknown field `{field}`")]
    UnknownField { op: String, field: String },
    #[error("field `{field}`: expected {expected}")]
    WrongType { field: String, expected: &'static str },
    #[error("unknown operation `{0}`")]
    UnknownOp(String),
    #[error("`{0}` is not a 32-digit hex ObjectId")]
    InvalidId(String),
    #[error("operation id {0} is not an authored id (top bit clear)")]
    NotAuthored(ObjectId),
    #[error("duplicate operation id {0}")]
    DuplicateId(ObjectId),
    #[error("{0}")]
    Invalid(String),
}

fn line_at(src: &str, byte: usize) -> usize {
    let end = byte.min(src.len());
    src.as_bytes()[..end].iter().filter(|b| **b == b'\n').count() + 1
}

fn syntax(src: &str, e: toml::de::Error) -> LayerError {
    LayerError {
        line: e.span().map_or(0, |s| line_at(src, s.start)),
        kind: LayerErrorKind::Syntax(e.message().to_string()),
    }
}

type RawOp = Spanned<BTreeMap<String, Spanned<Value>>>;

#[derive(Deserialize)]
struct RawFile {
    #[serde(default)]
    op: Vec<RawOp>,
}

/// Field access for one operation, tracking lines and leftovers.
struct Fields<'s> {
    src: &'s str,
    line: usize,
    op: String,
    map: BTreeMap<String, Spanned<Value>>,
}

type LResult<T> = Result<T, LayerError>;

impl Fields<'_> {
    fn err(&self, line: usize, kind: LayerErrorKind) -> LayerError {
        LayerError { line, kind }
    }
    fn line_of(&self, v: &Spanned<Value>) -> usize {
        line_at(self.src, v.span().start)
    }
    fn take(&mut self, k: &str) -> Option<(usize, Value)> {
        self.map.remove(k).map(|v| (line_at(self.src, v.span().start), v.into_inner()))
    }
    fn missing(&self, field: &'static str) -> LayerError {
        self.err(self.line, LayerErrorKind::MissingField { op: self.op.clone(), field })
    }
    fn req(&mut self, k: &'static str) -> LResult<(usize, Value)> {
        self.take(k).ok_or_else(|| self.missing(k))
    }
    fn opt<T>(&mut self, k: &'static str, f: impl Fn(usize, &str, Value) -> LResult<T>) -> LResult<Option<T>> {
        self.take(k).map(|(l, v)| f(l, k, v)).transpose()
    }
    fn get<T>(&mut self, k: &'static str, f: impl Fn(usize, &str, Value) -> LResult<T>) -> LResult<T> {
        let (l, v) = self.req(k)?;
        f(l, k, v)
    }
    /// Any key not consumed is an error, reported at the earliest such line.
    fn finish(self) -> LResult<()> {
        let first = self.map.iter().min_by_key(|(_, v)| v.span().start);
        match first {
            Some((k, v)) => Err(self.err(
                self.line_of(v),
                LayerErrorKind::UnknownField { op: self.op.clone(), field: k.clone() },
            )),
            None => Ok(()),
        }
    }
}

fn wrong(line: usize, field: &str, expected: &'static str) -> LayerError {
    LayerError { line, kind: LayerErrorKind::WrongType { field: field.to_string(), expected } }
}
fn invalid(line: usize, msg: String) -> LayerError {
    LayerError { line, kind: LayerErrorKind::Invalid(msg) }
}

fn as_f64(line: usize, field: &str, v: &Value) -> LResult<f64> {
    let x = match v {
        Value::Float(f) => *f,
        Value::Integer(i) => *i as f64,
        _ => return Err(wrong(line, field, "a number")),
    };
    if x.is_finite() { Ok(x) } else { Err(wrong(line, field, "a finite number")) }
}
fn as_string(line: usize, field: &str, v: Value) -> LResult<String> {
    match v {
        Value::String(s) if !s.is_empty() => Ok(s),
        _ => Err(wrong(line, field, "a non-empty string")),
    }
}
fn as_array<const N: usize>(line: usize, field: &str, v: &Value, expected: &'static str) -> LResult<[f64; N]> {
    match v {
        Value::Array(a) if a.len() == N => {
            let mut out = [0f64; N];
            for (o, x) in out.iter_mut().zip(a) {
                *o = as_f64(line, field, x).map_err(|_| wrong(line, field, expected))?;
            }
            Ok(out)
        }
        _ => Err(wrong(line, field, expected)),
    }
}
fn as_vec3(line: usize, field: &str, v: Value) -> LResult<Vec3> {
    as_array::<3>(line, field, &v, "an array of 3 numbers")
}
fn as_quat(line: usize, field: &str, v: Value) -> LResult<Quat> {
    let q = as_array::<4>(line, field, &v, "a quaternion [x, y, z, w]")?;
    let n = q.iter().map(|x| x * x).sum::<f64>().sqrt();
    if (n - 1.0).abs() > 1e-3 {
        return Err(invalid(line, format!("`{field}` is not a unit quaternion (length {n})")));
    }
    Ok(q)
}
fn as_positive(line: usize, field: &str, v: Value) -> LResult<f64> {
    let x = as_f64(line, field, &v)?;
    if x < 0.0 { Err(wrong(line, field, "a non-negative number")) } else { Ok(x) }
}
fn as_scale(line: usize, field: &str, v: Value) -> LResult<Scale> {
    match v {
        Value::Array(_) => Ok(Scale::Axes(as_array::<3>(line, field, &v, "a number or an array of 3 numbers")?)),
        _ => Ok(Scale::Uniform(as_f64(line, field, &v).map_err(|_| wrong(line, field, "a number or an array of 3 numbers"))?)),
    }
}
fn as_id(line: usize, _field: &str, v: Value) -> LResult<ObjectId> {
    match v {
        Value::String(s) => s.parse().map_err(|_| LayerError { line, kind: LayerErrorKind::InvalidId(s) }),
        _ => Err(wrong(line, "id", "a 32-digit hex string")),
    }
}
fn as_id_list(line: usize, field: &str, v: Value) -> LResult<Vec<ObjectId>> {
    match v {
        Value::Array(a) if !a.is_empty() => a.into_iter().map(|x| as_id(line, field, x)).collect(),
        _ => Err(wrong(line, field, "a non-empty array of ObjectId strings")),
    }
}
fn as_str_list(line: usize, field: &str, v: Value) -> LResult<Vec<String>> {
    match v {
        Value::Array(a) => a.into_iter().map(|x| as_string(line, field, x)).collect(),
        _ => Err(wrong(line, field, "an array of strings")),
    }
}
fn as_volume(line: usize, field: &str, v: Value) -> LResult<Volume> {
    let Value::Table(mut t) = v else {
        return Err(wrong(line, field, "a table with `shape`"));
    };
    let mut take = |k: &'static str| t.remove(k).ok_or_else(|| invalid(line, format!("`{field}` is missing `{k}`")));
    let shape = as_string(line, "shape", take("shape")?)?;
    let vol = match shape.as_str() {
        "box" => {
            let centre = as_vec3(line, "centre", take("centre")?)?;
            let half_extents = as_vec3(line, "half_extents", take("half_extents")?)?;
            let rotation = t.remove("rotation").map(|r| as_quat(line, "rotation", r)).transpose()?;
            Volume::Box { centre, half_extents, rotation }
        }
        "sphere" => Volume::Sphere {
            centre: as_vec3(line, "centre", take("centre")?)?,
            radius: as_positive(line, "radius", take("radius")?)?,
        },
        "prism" => {
            let points = match take("points")? {
                Value::Array(a) if a.len() >= 3 => {
                    a.into_iter().map(|p| as_vec3(line, "points", p)).collect::<LResult<Vec<_>>>()?
                }
                _ => return Err(wrong(line, "points", "an array of at least 3 points")),
            };
            Volume::Prism { points, height: as_positive(line, "height", take("height")?)? }
        }
        s => return Err(invalid(line, format!("unknown volume shape `{s}` (box, sphere, prism)"))),
    };
    if let Some(k) = t.keys().next() {
        return Err(invalid(line, format!("`{field}` has unknown key `{k}`")));
    }
    Ok(vol)
}
fn as_params(line: usize, field: &str, v: Value) -> LResult<ReplaceParams> {
    let Value::Table(mut t) = v else {
        return Err(wrong(line, field, "a table"));
    };
    let mut s = |k: &str| t.remove(k).map(|v| as_string(line, k, v)).transpose();
    let p = ReplaceParams { style: s("style")?, colour: s("colour")?, roof: s("roof")?, height: None };
    let p = ReplaceParams { height: t.remove("height").map(|v| as_positive(line, "height", v)).transpose()?, ..p };
    if let Some(k) = t.keys().next() {
        return Err(invalid(line, format!("`params` has unknown key `{k}` (style, height, colour, roof)")));
    }
    if p == ReplaceParams::default() {
        return Err(invalid(line, "`params` is empty".into()));
    }
    Ok(p)
}

fn parse_op(src: &str, raw: RawOp) -> LResult<(usize, Op)> {
    let line = line_at(src, raw.span().start);
    let mut f = Fields { src, line, op: String::new(), map: raw.into_inner() };
    let (op_line, op_v) = f.req("op")?;
    let op_name = as_string(op_line, "op", op_v)?;
    f.op = op_name.clone();
    let (id_line, id_v) = f.req("id")?;
    let id = as_id(id_line, "id", id_v)?;
    if !id.is_authored() {
        return Err(LayerError { line: id_line, kind: LayerErrorKind::NotAuthored(id) });
    }
    let body = match op_name.as_str() {
        "place" => OpBody::Place(Place {
            prefab: f.get("prefab", as_string)?,
            position: f.get("position", as_vec3)?,
            rotation: f.opt("rotation", as_quat)?,
            scale: f.opt("scale", as_scale)?,
            clear: f.opt("clear", as_positive)?,
        }),
        "replace" => {
            let target = f.get("target", as_id)?;
            let prefab = f.opt("prefab", as_string)?;
            let params = f.opt("params", as_params)?;
            let with = match (prefab, params) {
                (Some(p), None) => ReplaceWith::Prefab(p),
                (None, Some(p)) => ReplaceWith::Params(p),
                (None, None) => return Err(f.missing("prefab")),
                (Some(_), Some(_)) => {
                    return Err(invalid(line, "`replace` takes either `prefab` or `params`, not both".into()));
                }
            };
            OpBody::Replace(Replace { target, with })
        }
        "suppress" => {
            let targets = f.opt("targets", as_id_list)?;
            let volume = f.opt("volume", as_volume)?;
            if targets.is_none() && volume.is_none() {
                return Err(f.missing("targets"));
            }
            OpBody::Suppress(Suppress { targets, volume })
        }
        "marker" => OpBody::Marker(Marker {
            name: f.get("name", as_string)?,
            kind: f.get("kind", as_string)?,
            position: f.get("position", as_vec3)?,
            rotation: f.opt("rotation", as_quat)?,
            volume: f.opt("volume", as_volume)?,
            tags: f.opt("tags", as_str_list)?,
        }),
        "mission" | "road_patch" | "terrain_patch" => {
            let op = match op_name.as_str() {
                "mission" => OtherOp::Mission,
                "road_patch" => OtherOp::RoadPatch,
                _ => OtherOp::TerrainPatch,
            };
            let fields = std::mem::take(&mut f.map).into_iter().map(|(k, v)| (k, v.into_inner())).collect();
            OpBody::Other { op, fields }
        }
        other => return Err(LayerError { line: op_line, kind: LayerErrorKind::UnknownOp(other.to_string()) }),
    };
    f.finish()?;
    Ok((line, Op { id, body }))
}

impl LayerFile {
    /// Parse a layer file. Rejects malformed operations with the line they are on.
    pub fn parse(src: &str) -> Result<LayerFile, LayerError> {
        let mut table: toml::Table = toml::from_str(src).map_err(|e| syntax(src, e))?;
        let raw: RawFile = toml::from_str(src).map_err(|e| syntax(src, e))?;
        table.remove("op");
        let header = table.into_iter().collect();
        let mut seen = BTreeSet::new();
        let mut ops = Vec::with_capacity(raw.op.len());
        for r in raw.op {
            let (line, op) = parse_op(src, r)?;
            if !seen.insert(op.id) {
                return Err(LayerError { line, kind: LayerErrorKind::DuplicateId(op.id) });
            }
            ops.push(op);
        }
        // Operations apply sorted by id (layers.md section 4), so the parsed value does not
        // depend on file order.
        ops.sort_by_key(|o| o.id);
        Ok(LayerFile { header, ops })
    }

    /// The canonical text (see the module docs).
    pub fn to_toml(&self) -> String {
        let mut s = String::new();
        for (k, v) in &self.header {
            kv(&mut s, k, &fmt_value(v));
        }
        let mut ops: Vec<&Op> = self.ops.iter().collect();
        ops.sort_by_key(|o| o.id);
        for (i, op) in ops.iter().enumerate() {
            if i > 0 || !self.header.is_empty() {
                s.push('\n');
            }
            write_op(&mut s, op);
        }
        s
    }
}

fn kv(s: &mut String, k: &str, v: &str) {
    let _ = writeln!(s, "{} = {v}", fmt_key(k));
}

fn write_op(s: &mut String, op: &Op) {
    s.push_str("[[op]]\n");
    kv(s, "id", &fmt_str(&op.id.to_string()));
    kv(s, "op", &fmt_str(op.name()));
    match &op.body {
        OpBody::Place(p) => {
            kv(s, "prefab", &fmt_str(&p.prefab));
            kv(s, "position", &fmt_floats(&p.position));
            if let Some(r) = &p.rotation {
                kv(s, "rotation", &fmt_floats(r));
            }
            if let Some(sc) = &p.scale {
                kv(s, "scale", &match sc {
                    Scale::Uniform(x) => fmt_f64(*x),
                    Scale::Axes(a) => fmt_floats(a),
                });
            }
            if let Some(c) = p.clear {
                kv(s, "clear", &fmt_f64(c));
            }
        }
        OpBody::Replace(r) => {
            kv(s, "target", &fmt_str(&r.target.to_string()));
            match &r.with {
                ReplaceWith::Prefab(p) => kv(s, "prefab", &fmt_str(p)),
                ReplaceWith::Params(p) => {
                    let mut parts = Vec::new();
                    if let Some(v) = &p.colour {
                        parts.push(format!("colour = {}", fmt_str(v)));
                    }
                    if let Some(v) = p.height {
                        parts.push(format!("height = {}", fmt_f64(v)));
                    }
                    if let Some(v) = &p.roof {
                        parts.push(format!("roof = {}", fmt_str(v)));
                    }
                    if let Some(v) = &p.style {
                        parts.push(format!("style = {}", fmt_str(v)));
                    }
                    kv(s, "params", &format!("{{ {} }}", parts.join(", ")));
                }
            }
        }
        OpBody::Suppress(sp) => {
            if let Some(t) = &sp.targets {
                let ids: Vec<String> = t.iter().map(|i| fmt_str(&i.to_string())).collect();
                kv(s, "targets", &format!("[{}]", ids.join(", ")));
            }
            if let Some(v) = &sp.volume {
                kv(s, "volume", &fmt_volume(v));
            }
        }
        OpBody::Marker(m) => {
            kv(s, "name", &fmt_str(&m.name));
            kv(s, "kind", &fmt_str(&m.kind));
            kv(s, "position", &fmt_floats(&m.position));
            if let Some(r) = &m.rotation {
                kv(s, "rotation", &fmt_floats(r));
            }
            if let Some(v) = &m.volume {
                kv(s, "volume", &fmt_volume(v));
            }
            if let Some(t) = &m.tags {
                let tags: Vec<String> = t.iter().map(|x| fmt_str(x)).collect();
                kv(s, "tags", &format!("[{}]", tags.join(", ")));
            }
        }
        OpBody::Other { fields, .. } => {
            for (k, v) in fields {
                kv(s, k, &fmt_value(v));
            }
        }
    }
}

fn fmt_volume(v: &Volume) -> String {
    match v {
        Volume::Box { centre, half_extents, rotation } => {
            let mut s = format!(
                "{{ shape = \"box\", centre = {}, half_extents = {}",
                fmt_floats(centre),
                fmt_floats(half_extents)
            );
            if let Some(r) = rotation {
                let _ = write!(s, ", rotation = {}", fmt_floats(r));
            }
            s + " }"
        }
        Volume::Sphere { centre, radius } => {
            format!("{{ shape = \"sphere\", centre = {}, radius = {} }}", fmt_floats(centre), fmt_f64(*radius))
        }
        Volume::Prism { points, height } => {
            let pts: Vec<String> = points.iter().map(|p| fmt_floats(p)).collect();
            format!("{{ shape = \"prism\", points = [{}], height = {} }}", pts.join(", "), fmt_f64(*height))
        }
    }
}

/// Shortest round-trip decimal with at least one fractional digit.
pub fn fmt_f64(v: f64) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let s = format!("{v}");
    if s.contains(['.', 'e', 'E']) { s } else { s + ".0" }
}
fn fmt_floats(v: &[f64]) -> String {
    let parts: Vec<String> = v.iter().map(|x| fmt_f64(*x)).collect();
    format!("[{}]", parts.join(", "))
}
fn fmt_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\t' => o.push_str("\\t"),
            '\r' => o.push_str("\\r"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                let _ = write!(o, "\\u{:04X}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}
fn fmt_key(k: &str) -> String {
    if !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        k.to_string()
    } else {
        fmt_str(k)
    }
}
fn fmt_value(v: &Value) -> String {
    match v {
        Value::String(s) => fmt_str(s),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => fmt_f64(*f),
        Value::Boolean(b) => b.to_string(),
        Value::Datetime(d) => d.to_string(),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(fmt_value).collect();
            format!("[{}]", parts.join(", "))
        }
        Value::Table(t) => {
            if t.is_empty() {
                return "{}".into();
            }
            let mut keys: Vec<&String> = t.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys.iter().map(|k| format!("{} = {}", fmt_key(k), fmt_value(&t[*k]))).collect();
            format!("{{ {} }}", parts.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------------------------
// layers.toml

/// One row of `layers/layers.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerEntry {
    /// Directory name under `layers/`.
    pub name: String,
    /// On by default.
    pub enabled: bool,
}

/// `layers/layers.toml`: application order (first applies first) and default state.
///
/// ```toml
/// [[layer]]
/// name = "base_hubs"
/// enabled = true
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct LayerIndex {
    pub layers: Vec<LayerEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    name: Spanned<String>,
    enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIndex {
    #[serde(default)]
    layer: Vec<RawEntry>,
}

/// A layer directory name: `[a-z0-9_-]+`.
pub fn valid_layer_name(n: &str) -> bool {
    !n.is_empty() && n.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

impl LayerIndex {
    pub fn parse(src: &str) -> Result<LayerIndex, LayerError> {
        let raw: RawIndex = toml::from_str(src).map_err(|e| syntax(src, e))?;
        let mut seen = BTreeSet::new();
        let mut layers = Vec::with_capacity(raw.layer.len());
        for e in raw.layer {
            let line = line_at(src, e.name.span().start);
            let name = e.name.into_inner();
            if !valid_layer_name(&name) {
                return Err(invalid(line, format!("layer name `{name}` must match [a-z0-9_-]+")));
            }
            if !seen.insert(name.clone()) {
                return Err(invalid(line, format!("layer `{name}` listed twice")));
            }
            layers.push(LayerEntry { name, enabled: e.enabled });
        }
        Ok(LayerIndex { layers })
    }
    pub fn to_toml(&self) -> String {
        let mut s = String::new();
        for (i, l) in self.layers.iter().enumerate() {
            if i > 0 {
                s.push('\n');
            }
            s.push_str("[[layer]]\n");
            kv(&mut s, "name", &fmt_str(&l.name));
            kv(&mut s, "enabled", &l.enabled.to_string());
        }
        s
    }
}

// ---------------------------------------------------------------------------------------------
// Directories

/// `<body>/<face>/<level>/<x>/<y>.toml`, relative to a layer directory. `body_name` is the
/// body catalog name (`earth`); the caller maps names to `BodyId`s (`ed_space`).
pub fn node_file_path(body_name: &str, node: NodeId) -> PathBuf {
    let mut p = PathBuf::from(body_name);
    p.push(node.face().to_string());
    p.push(node.level().to_string());
    p.push(node.x().to_string());
    p.push(format!("{}.toml", node.y()));
    p
}

/// Inverse of [`node_file_path`]. `None` unless the path is exactly that shape for an L10
/// node.
pub fn node_from_file_path(rel: &Path, body_of: &dyn Fn(&str) -> Option<BodyId>) -> Option<NodeId> {
    let parts: Vec<&str> = rel.iter().map(|c| c.to_str()).collect::<Option<_>>()?;
    let [body, face, level, x, y] = parts.as_slice() else {
        return None;
    };
    let y = y.strip_suffix(".toml")?;
    let num = |s: &str| -> Option<u32> {
        // Canonical decimal only: no sign, no leading zeros.
        (s == "0" || (!s.starts_with('0') && !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())))
            .then(|| s.parse().ok())
            .flatten()
    };
    let level = num(level)?;
    if level != LAYER_NODE_LEVEL as u32 {
        return None;
    }
    let face = num(face)?;
    NodeId::new(body_of(body)?, u8::try_from(face).ok()?, level as u8, num(x)?, num(y)?)
}

#[derive(Debug, thiserror::Error)]
pub enum LayerDirError {
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("{0}: not a <body>/<face>/10/<x>/<y>.toml layer file")]
    BadPath(PathBuf),
    #[error("{path}: {error}")]
    Parse { path: PathBuf, error: LayerError },
}

/// Read every node file of one layer directory, keyed (and so ordered) by node.
pub fn read_layer_dir(
    dir: &Path,
    body_of: &dyn Fn(&str) -> Option<BodyId>,
) -> Result<BTreeMap<NodeId, LayerFile>, LayerDirError> {
    let io = |path: &Path| {
        let path = path.to_path_buf();
        move |source| LayerDirError::Io { path, source }
    };
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(io(&d))? {
            let p = e.map_err(io(&d))?.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut out = BTreeMap::new();
    for p in files {
        let rel = p.strip_prefix(dir).unwrap_or(&p);
        let node = node_from_file_path(rel, body_of).ok_or_else(|| LayerDirError::BadPath(p.clone()))?;
        let text = std::fs::read_to_string(&p).map_err(io(&p))?;
        let file = LayerFile::parse(&text).map_err(|error| LayerDirError::Parse { path: p.clone(), error })?;
        out.insert(node, file);
    }
    Ok(out)
}

/// Write one node file in canonical form, creating directories. Returns the path written.
pub fn write_layer_file(dir: &Path, body_name: &str, node: NodeId, file: &LayerFile) -> std::io::Result<PathBuf> {
    if node.level() != LAYER_NODE_LEVEL {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "layer files are per L10 node"));
    }
    let p = dir.join(node_file_path(body_name, node));
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&p, file.to_toml())?;
    Ok(p)
}
