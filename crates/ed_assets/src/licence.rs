//! Licence registry (`data/registry/*.toml`) and `ew-bake verify`: the licence gate over a
//! baked pack plus a full chunk-hash check.
//!
//! A pack fails verification when:
//! - any chunk hash does not match its bytes (`PackReader::verify_all`);
//! - it has no pack-info chunk ([`PackInfo`], written by every `ew-bake build`);
//! - a chunk has no licence id, or an id that is not in the registry;
//! - a chunk's licence is RED, YELLOW without review notes, or DEV in a game-packaged pack;
//! - `megascans` content, or content whose redistribution is "packaged game only", is in a
//!   pack that is not flagged game-packaged;
//! - a chunk lacks provenance (`src=`, `tool=`, `registry=`), its `registry=` is not one of
//!   its licences, or a Megascans chunk has no `fab-id=`;
//! - the pack-level licence list misses a chunk's licence.

use anyhow::{Context, Result, anyhow, bail};
use ed_geo::{BodyId, NodeId};
use ed_pack::kinds::Opaque;
use ed_pack::{Chunk, ChunkKey, ChunkKind, ChunkMeta, PackReader, Rung};
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The registry fields the gate reads (the full schema is `xtask/src/registry.rs`).
#[derive(Deserialize, Debug, Clone)]
pub struct Entry {
    pub id: String,
    pub kind: String,
    pub licence: String,
    #[serde(default)]
    pub redistribution: String,
    pub screening: String,
    #[serde(default)]
    pub review_notes: String,
}

#[derive(Debug, Clone, Default)]
pub struct Registry {
    pub entries: BTreeMap<String, Entry>,
}

/// Licence id of Megascans content.
pub const MEGASCANS: &str = "megascans";
/// Redistribution value that restricts content to the packaged game.
pub const PACKAGED_ONLY: &str = "packaged game only";

impl Registry {
    pub fn load(dir: &Path) -> Result<Registry> {
        let mut entries = BTreeMap::new();
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .with_context(|| format!("registry folder {}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        files.sort();
        for p in files {
            let e: Entry = toml::from_str(&std::fs::read_to_string(&p)?).with_context(|| p.display().to_string())?;
            if p.file_stem().and_then(|s| s.to_str()) != Some(e.id.as_str()) {
                bail!("{}: file name must equal id `{}`", p.display(), e.id);
            }
            entries.insert(e.id.clone(), e);
        }
        Ok(Registry { entries })
    }
    /// `data/registry` in `start` or its nearest ancestor that has one.
    pub fn find(start: &Path) -> Option<PathBuf> {
        let start = start.canonicalize().ok()?;
        start.ancestors().map(|a| a.join("data/registry")).find(|p| p.is_dir())
    }
    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }
    /// Problems with using `id` in a pack (`game_packaged` as flagged), empty when fine.
    pub fn check(&self, id: &str, game_packaged: bool) -> Vec<String> {
        let mut e = Vec::new();
        let Some(d) = self.get(id) else {
            return vec![format!("licence `{id}` is not in the registry")];
        };
        match d.screening.as_str() {
            "RED" => e.push(format!("licence `{id}` is RED ({})", d.licence)),
            "YELLOW" if d.review_notes.trim().is_empty() => e.push(format!("licence `{id}` is YELLOW without review notes")),
            "DEV" if game_packaged => e.push(format!("licence `{id}` is development-only and the pack is game-packaged")),
            "GREEN" | "YELLOW" | "DEV" => {}
            s => e.push(format!("licence `{id}` has unknown screening `{s}`")),
        }
        if !game_packaged && (id == MEGASCANS || d.redistribution.trim().eq_ignore_ascii_case(PACKAGED_ONLY)) {
            e.push(format!("licence `{id}` allows distribution only inside the packaged game; the pack is not flagged game_packaged"));
        }
        e
    }
}

/// Pack-level facts written by `ew-bake build` as an opaque `source` chunk at [`info_key`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct PackInfo {
    pub name: String,
    pub game_packaged: bool,
    pub tool: String,
}

/// Key of the pack-info chunk: the `source` kind at the level-0 node of face 0 of Earth.
pub fn info_key() -> ChunkKey {
    let node = NodeId::new(BodyId(3), 0, 0, 0, 0).expect("root node");
    ChunkKey::node(node, ChunkKind::Source, Rung::R0)
}

impl PackInfo {
    pub fn to_chunk(&self, meta: &ChunkMeta) -> Result<Chunk<'static>> {
        let text = toml::to_string(self)?;
        Ok(Chunk::encode(&Opaque { kind: ChunkKind::Source, data: Cow::Owned(text.into_bytes()) }, meta)?)
    }
    pub fn read<S: ed_pack::ReadAt>(r: &PackReader<S>) -> Result<PackInfo> {
        let c = r.read(&info_key()).map_err(|e| anyhow!("no pack-info chunk ({e}); was this pack built by ew-bake?"))?;
        let o: Opaque = c.decode()?;
        Ok(toml::from_str(std::str::from_utf8(&o.data)?)?)
    }
}

/// Verification result: `errors` empty means pass.
#[derive(Debug, Default)]
pub struct Verified {
    pub info: Option<PackInfo>,
    pub chunks: usize,
    pub keys: usize,
    pub licences: Vec<String>,
    pub errors: Vec<String>,
}

/// Verifies a pack file against the registry.
pub fn verify(path: &Path, reg: &Registry) -> Result<Verified> {
    let r = PackReader::open_file(path).with_context(|| format!("opening {}", path.display()))?;
    Ok(verify_reader(&r, reg))
}

pub fn verify_reader<S: ed_pack::ReadAt>(r: &PackReader<S>, reg: &Registry) -> Verified {
    let mut v = Verified { chunks: r.chunk_count(), keys: r.key_count(), licences: r.meta().licences.clone(), ..Default::default() };
    if let Err(e) = r.verify_all() {
        v.errors.push(format!("hash check: {e}"));
    }
    let info = match PackInfo::read(r) {
        Ok(i) => Some(i),
        Err(e) => {
            v.errors.push(format!("{e}"));
            None
        }
    };
    let gp = info.as_ref().is_some_and(|i| i.game_packaged);
    for id in &r.meta().licences {
        for e in reg.check(id, gp) {
            v.errors.push(format!("pack licence list: {e}"));
        }
    }
    let mut seen = BTreeMap::new();
    for e in r.entries() {
        if seen.insert(e.hash, ()).is_some() {
            continue;
        }
        let what = format!("{:?} {}", e.key.kind(), e.hash);
        let c = match r.read_hash(&e.hash) {
            Ok(c) => c,
            Err(err) => {
                v.errors.push(format!("{what}: {err}"));
                continue;
            }
        };
        if c.licences().is_empty() {
            v.errors.push(format!("{what}: no licence id"));
        }
        for id in c.licences() {
            for err in reg.check(id, gp) {
                v.errors.push(format!("{what}: {err}"));
            }
            if !r.meta().licences.contains(id) {
                v.errors.push(format!("{what}: licence `{id}` missing from the pack's licence list"));
            }
        }
        let prov = c.provenance();
        let has = |p: &str| prov.iter().any(|s| s.starts_with(p));
        for p in ["src=", "tool=", "registry="] {
            if !has(p) {
                v.errors.push(format!("{what}: provenance has no `{p}`"));
            }
        }
        for s in prov.iter().filter_map(|s| s.strip_prefix("registry=")) {
            if !c.licences().iter().any(|l| l == s) {
                v.errors.push(format!("{what}: provenance registry `{s}` is not one of its licences"));
            }
        }
        if c.licences().iter().any(|l| l == MEGASCANS) && !has("fab-id=") {
            v.errors.push(format!("{what}: Megascans content without a Fab asset id (`fab-id=`)"));
        }
    }
    v.info = info;
    v
}
