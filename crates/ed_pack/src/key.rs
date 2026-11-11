//! Chunk kinds, rungs and manifest keys.
//!
//! Key bytes are a fixed 40-byte, **big-endian** encoding so that byte order equals the
//! logical order `(NodeId, kind, rung, layer)` and all chunks of one node are contiguous in the
//! sorted manifest. (Every other integer in the format is little-endian.)

use crate::error::{Error, Result};
use crate::hash::AssetId;
use ed_geo::NodeId;

/// Chunk kind ids (`pack.md` section 4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u16)]
pub enum ChunkKind {
    Terrain = 1,
    Layers = 2,
    Proxy = 3,
    Shells = 4,
    Instances = 5,
    Collision = 6,
    Nav = 7,
    Ground = 8,
    Source = 9,
    Mesh = 16,
    Clusters = 17,
    Texture = 18,
    Material = 19,
    Skeleton = 20,
    Clip = 21,
    Prefab = 22,
    CollisionAsset = 23,
    Layer = 32,
}

impl ChunkKind {
    pub const ALL: [ChunkKind; 18] = [
        ChunkKind::Terrain,
        ChunkKind::Layers,
        ChunkKind::Proxy,
        ChunkKind::Shells,
        ChunkKind::Instances,
        ChunkKind::Collision,
        ChunkKind::Nav,
        ChunkKind::Ground,
        ChunkKind::Source,
        ChunkKind::Mesh,
        ChunkKind::Clusters,
        ChunkKind::Texture,
        ChunkKind::Material,
        ChunkKind::Skeleton,
        ChunkKind::Clip,
        ChunkKind::Prefab,
        ChunkKind::CollisionAsset,
        ChunkKind::Layer,
    ];

    pub fn from_u16(v: u16) -> Option<ChunkKind> {
        ChunkKind::ALL.iter().copied().find(|k| *k as u16 == v)
    }
    pub fn id(self) -> u16 {
        self as u16
    }
    /// Keyed by `(NodeId, kind, rung)`.
    pub fn is_node_kind(self) -> bool {
        (1..=9).contains(&(self as u16))
    }
    /// Keyed by `(AssetId, kind)`.
    pub fn is_asset_kind(self) -> bool {
        (16..=23).contains(&(self as u16))
    }
    /// The schema version this build reads and writes. Reserved kinds (no typed body yet)
    /// are schema 0 with an opaque payload ([`crate::kinds::Opaque`]).
    pub fn schema(self) -> u16 {
        use ChunkKind::*;
        match self {
            Instances | Collision | Mesh | Clusters | Texture | Material | Prefab | CollisionAsset | Layer => 1,
            Terrain | Layers | Proxy | Shells | Nav | Ground | Source | Skeleton | Clip => 0,
        }
    }
    pub fn is_reserved(self) -> bool {
        self.schema() == 0
    }
}

/// Representation rung (`generation.md` section 3).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(u8)]
pub enum Rung {
    R0 = 0,
    R1 = 1,
    R2 = 2,
    R3 = 3,
    R4 = 4,
}

impl Rung {
    pub const ALL: [Rung; 5] = [Rung::R0, Rung::R1, Rung::R2, Rung::R3, Rung::R4];
    pub fn from_u8(v: u8) -> Option<Rung> {
        Rung::ALL.get(v as usize).copied()
    }
}

/// What a manifest entry is filed under.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ChunkKey {
    /// Generated node content: kinds 1 to 9.
    Node { node: NodeId, kind: ChunkKind, rung: Rung },
    /// A compiled override layer (kind 32) for an L10 node; `layer` is [`layer_name_id`].
    Layer { node: NodeId, layer: u64 },
    /// Asset content: kinds 16 to 23.
    Asset { asset: AssetId, kind: ChunkKind },
}

pub const KEY_LEN: usize = 40;
const TAG_NODE: u8 = 0;
const TAG_ASSET: u8 = 1;
const RUNG_NONE: u8 = 0xFF;

/// First 8 bytes (big-endian) of BLAKE3 of the layer directory name.
pub fn layer_name_id(name: &str) -> u64 {
    let h = blake3::hash(name.as_bytes());
    let mut b = [0u8; 8];
    b.copy_from_slice(&h.as_bytes()[..8]);
    u64::from_be_bytes(b)
}

impl ChunkKey {
    pub fn node(node: NodeId, kind: ChunkKind, rung: Rung) -> ChunkKey {
        ChunkKey::Node { node, kind, rung }
    }
    pub fn layer(node: NodeId, layer_name: &str) -> ChunkKey {
        ChunkKey::Layer { node, layer: layer_name_id(layer_name) }
    }
    pub fn asset(asset: AssetId, kind: ChunkKind) -> ChunkKey {
        ChunkKey::Asset { asset, kind }
    }
    pub fn kind(&self) -> ChunkKind {
        match *self {
            ChunkKey::Node { kind, .. } | ChunkKey::Asset { kind, .. } => kind,
            ChunkKey::Layer { .. } => ChunkKind::Layer,
        }
    }
    pub fn node_id(&self) -> Option<NodeId> {
        match *self {
            ChunkKey::Node { node, .. } | ChunkKey::Layer { node, .. } => Some(node),
            ChunkKey::Asset { .. } => None,
        }
    }
    /// Node keys must name a node kind, asset keys an asset kind.
    pub fn validate(&self) -> Result<()> {
        match *self {
            ChunkKey::Node { kind, .. } if !kind.is_node_kind() => {
                Err(Error::Invalid(format!("{kind:?} is not a node chunk kind")))
            }
            ChunkKey::Asset { kind, .. } if !kind.is_asset_kind() => {
                Err(Error::Invalid(format!("{kind:?} is not an asset chunk kind")))
            }
            _ => Ok(()),
        }
    }

    /// Node and layer keys: `0, node u64, kind u16, rung u8 (0xFF for layers), layer u64, 0...`.
    /// Asset keys: `1, asset [32], kind u16, 0...`. Big-endian, zero-filled to 40 bytes.
    pub fn to_bytes(&self) -> [u8; KEY_LEN] {
        let mut b = [0u8; KEY_LEN];
        match *self {
            ChunkKey::Node { node, kind, rung } => {
                b[0] = TAG_NODE;
                b[1..9].copy_from_slice(&node.bits().to_be_bytes());
                b[9..11].copy_from_slice(&kind.id().to_be_bytes());
                b[11] = rung as u8;
            }
            ChunkKey::Layer { node, layer } => {
                b[0] = TAG_NODE;
                b[1..9].copy_from_slice(&node.bits().to_be_bytes());
                b[9..11].copy_from_slice(&ChunkKind::Layer.id().to_be_bytes());
                b[11] = RUNG_NONE;
                b[12..20].copy_from_slice(&layer.to_be_bytes());
            }
            ChunkKey::Asset { asset, kind } => {
                b[0] = TAG_ASSET;
                b[1..33].copy_from_slice(&asset.0);
                b[33..35].copy_from_slice(&kind.id().to_be_bytes());
            }
        }
        b
    }

    pub fn from_bytes(b: &[u8; KEY_LEN]) -> Result<ChunkKey> {
        let bad = |why: &str| Error::Corrupt(format!("manifest key: {why}"));
        let kind_at = |i: usize| {
            let v = u16::from_be_bytes([b[i], b[i + 1]]);
            ChunkKind::from_u16(v).ok_or(Error::UnknownKind(v))
        };
        let key = match b[0] {
            TAG_NODE => {
                let node = NodeId::from_bits(u64::from_be_bytes(b[1..9].try_into().unwrap_or_default()))
                    .ok_or_else(|| bad("invalid NodeId"))?;
                let kind = kind_at(9)?;
                if b[20..].iter().any(|x| *x != 0) {
                    return Err(bad("non-zero tail"));
                }
                if kind == ChunkKind::Layer {
                    if b[11] != RUNG_NONE {
                        return Err(bad("layer key with a rung"));
                    }
                    ChunkKey::Layer { node, layer: u64::from_be_bytes(b[12..20].try_into().unwrap_or_default()) }
                } else {
                    let rung = Rung::from_u8(b[11]).ok_or_else(|| bad("invalid rung"))?;
                    if b[12..20].iter().any(|x| *x != 0) {
                        return Err(bad("non-zero layer field in a node key"));
                    }
                    ChunkKey::Node { node, kind, rung }
                }
            }
            TAG_ASSET => {
                let mut a = [0u8; 32];
                a.copy_from_slice(&b[1..33]);
                let kind = kind_at(33)?;
                if b[35..].iter().any(|x| *x != 0) {
                    return Err(bad("non-zero tail"));
                }
                ChunkKey::Asset { asset: AssetId(a), kind }
            }
            t => return Err(bad(&format!("unknown tag {t}"))),
        };
        key.validate().map_err(|e| bad(&e.to_string()))?;
        Ok(key)
    }
}

/// The 9-byte prefix shared by every key of `node`.
pub(crate) fn node_prefix(node: NodeId) -> [u8; 9] {
    let mut p = [0u8; 9];
    p[0] = TAG_NODE;
    p[1..9].copy_from_slice(&node.bits().to_be_bytes());
    p
}
