//! Shared test setup: the baked packs `ew` loads.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// `target/packs` of the workspace, with `proving.ewpack` baked in-process if it is missing
/// (tests only: the app itself never bakes, it names the bake command and exits).
pub fn packs() -> PathBuf {
    let ws = workspace();
    let dir = ws.join("target/packs");
    let pack = dir.join("proving.ewpack");
    if !pack.exists() {
        std::fs::create_dir_all(&dir).unwrap();
        ed_assets::bake::build(&ws.join("assets-src/proving.toml"), &pack).expect("baking the proving pack");
    }
    dir
}
