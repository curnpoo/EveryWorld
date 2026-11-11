//! `ew-bake build` / `verify`: determinism (in process, across processes) and the licence gate.

use ed_assets::bake;
use ed_assets::fixtures;
use ed_assets::licence::{PackInfo, Registry, info_key, verify};
use ed_assets::texture;
use ed_pack::kinds::Opaque;
use ed_pack::{AssetId, Chunk, ChunkKey, ChunkKind, ChunkMeta, PackWriter};
use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch folder with its own `data/registry` (four ids covering every screening case).
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ew-bake-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("data/registry")).unwrap();
    let reg = [
        ("everyworld-original", "GREEN", "yes", ""),
        ("packonly", "YELLOW", "packaged game only", "owner asset"),
        ("megascans", "YELLOW", "conditions", "Fab Standard License"),
        ("redlic", "RED", "no", ""),
        ("devlic", "DEV", "no", "dev only"),
    ];
    for (id, scr, redis, notes) in reg {
        let t = format!("id = \"{id}\"\nkind = \"asset\"\nlicence = \"test\"\nredistribution = \"{redis}\"\nscreening = \"{scr}\"\nreview_notes = \"{notes}\"\n");
        std::fs::write(d.join(format!("data/registry/{id}.toml")), t).unwrap();
    }
    d
}

fn write_inputs(d: &Path) {
    std::fs::write(d.join("bar.glb"), fixtures::skinned_bar().unwrap()).unwrap();
    std::fs::write(d.join("brick.png"), texture::encode_png(&fixtures::pattern(64, 32, 9)).unwrap()).unwrap();
    std::fs::write(d.join("brick.material.toml"), "model = \"pbr\"\nbase_colour = [0.8, 0.4, 0.3]\nroughness = 0.9\n[textures]\nbase_colour = \"surfaces/brick/albedo\"\n").unwrap();
}

const SOURCE: &str = r#"
[pack]
name = "test"

[[gltf]]
path = "bar.glb"
asset = "props/bar"
registry = "everyworld-original"

[[texture]]
asset = "surfaces/brick/albedo"
path = "brick.png"
role = "colour"
registry = "everyworld-original"

[[material]]
asset = "surfaces/brick"
path = "brick.material.toml"
registry = "everyworld-original"
"#;

fn source(d: &Path, text: &str) -> PathBuf {
    let p = d.join("src.toml");
    std::fs::write(&p, text).unwrap();
    p
}

fn registry(d: &Path) -> Registry {
    Registry::load(&d.join("data/registry")).unwrap()
}

#[test]
fn bake_is_deterministic_in_process_and_across_processes() {
    let d = scratch("det");
    write_inputs(&d);
    let src = source(&d, SOURCE);
    let a = bake::build(&src, &d.join("a.ewpack")).unwrap();
    let b = bake::build(&src, &d.join("b.ewpack")).unwrap();
    assert_eq!(a.file_hash, b.file_hash);
    let exe = env!("CARGO_BIN_EXE_ew-bake");
    for out in ["c.ewpack", "e.ewpack"] {
        let st = Command::new(exe).args(["build", src.to_str().unwrap(), "-o", d.join(out).to_str().unwrap(), "-q"]).status().unwrap();
        assert!(st.success());
    }
    let bytes = |n: &str| std::fs::read(d.join(n)).unwrap();
    assert_eq!(bytes("a.ewpack"), bytes("c.ewpack"));
    assert_eq!(bytes("c.ewpack"), bytes("e.ewpack"));
    let h = Command::new(exe).args(["hash", d.join("c.ewpack").to_str().unwrap()]).output().unwrap();
    assert!(String::from_utf8_lossy(&h.stdout).starts_with(&a.file_hash.to_string()));
    let v = verify(&d.join("a.ewpack"), &registry(&d)).unwrap();
    assert!(v.errors.is_empty(), "{:?}", v.errors);
    // Rebuilding over an existing pack goes through a temp file and a rename.
    bake::build(&src, &d.join("a.ewpack")).unwrap();
    assert!(std::fs::read_dir(&d).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().contains(".tmp-")));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn every_chunk_has_provenance_and_no_private_path() {
    let d = scratch("prov");
    write_inputs(&d);
    let src = source(&d, SOURCE);
    bake::build(&src, &d.join("p.ewpack")).unwrap();
    let r = ed_pack::PackReader::open_file(&d.join("p.ewpack")).unwrap();
    let info = PackInfo::read(&r).unwrap();
    assert_eq!((info.name.as_str(), info.game_packaged), ("test", false));
    for e in r.entries() {
        let c = r.read_hash(&e.hash).unwrap();
        let p = c.provenance();
        for k in ["src=", "content=", "tool=ew-bake ", "registry="] {
            assert!(p.iter().any(|s| s.starts_with(k)), "{:?} lacks {k}: {p:?}", e.key.kind());
        }
        assert!(!c.licences().is_empty());
    }
    let raw = std::fs::read(d.join("p.ewpack")).unwrap();
    let dir = d.to_string_lossy().into_owned();
    assert!(!raw.windows(dir.len()).any(|w| w == dir.as_bytes()), "absolute source path leaked into the pack");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn env_paths_record_only_the_variable_and_file_name() {
    let d = scratch("env");
    write_inputs(&d);
    // SAFETY: test-local variable name; no other test reads it.
    unsafe { std::env::set_var("EW_TEST_BAR_GLB", d.join("bar.glb")) };
    let src = source(&d, "[pack]\nname = \"e\"\n[[gltf]]\npath_env = \"EW_TEST_BAR_GLB\"\nasset = \"a\"\nregistry = \"everyworld-original\"\n");
    bake::build(&src, &d.join("e.ewpack")).unwrap();
    let missing = source(&d, "[pack]\nname = \"e\"\n[[gltf]]\npath_env = \"EW_TEST_UNSET_VAR\"\nasset = \"a\"\nregistry = \"everyworld-original\"\n");
    let err = format!("{:#}", bake::build(&missing, &d.join("m.ewpack")).unwrap_err());
    assert!(err.contains("EW_TEST_UNSET_VAR"), "{err}");
    std::fs::remove_dir_all(&d).ok();
}

fn build_err(d: &Path, text: &str) -> String {
    let src = source(d, text);
    format!("{:#}", bake::build(&src, &d.join("x.ewpack")).expect_err("build must fail"))
}

#[test]
fn build_rejects_bad_licences() {
    let d = scratch("gate");
    write_inputs(&d);
    let g = |reg: &str, gp: bool| format!("[pack]\nname = \"g\"\ngame_packaged = {gp}\n[[gltf]]\npath = \"bar.glb\"\nasset = \"a\"\nregistry = \"{reg}\"\n");
    assert!(build_err(&d, &g("nope", false)).contains("not in the registry"));
    assert!(build_err(&d, &g("redlic", false)).contains("RED"));
    assert!(build_err(&d, &g("packonly", false)).contains("packaged game"));
    assert!(build_err(&d, &g("devlic", true)).contains("development-only"));
    // Allowed combinations.
    bake::build(&source(&d, &g("packonly", true)), &d.join("ok1.ewpack")).unwrap();
    bake::build(&source(&d, &g("devlic", false)), &d.join("ok2.ewpack")).unwrap();
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn megascans_rules() {
    let d = scratch("mega");
    let surf = d.join("outside/rock");
    std::fs::create_dir_all(&surf).unwrap();
    for (n, s) in [("Albedo", 1), ("Normal", 2), ("Roughness", 3)] {
        std::fs::write(surf.join(format!("rock_2K_{n}.png")), texture::encode_png(&fixtures::pattern(16, 16, s)).unwrap()).unwrap();
    }
    let m = |gp: bool, fab: &str| format!("[pack]\nname = \"m\"\ngame_packaged = {gp}\n[[megascans]]\ndir = \"outside/rock\"\nasset = \"surfaces/rock\"\nfab_id = \"{fab}\"\n");
    assert!(build_err(&d, &m(false, "abc123")).contains("game_packaged"));
    assert!(build_err(&d, &m(true, "")).contains("fab_id"));
    let rep = bake::build(&source(&d, &m(true, "abc123")), &d.join("m.ewpack")).unwrap();
    assert_eq!(rep.textures, 3);
    let r = ed_pack::PackReader::open_file(&d.join("m.ewpack")).unwrap();
    let c = r.read(&ChunkKey::asset(AssetId::from_path("surfaces/rock/albedo"), ChunkKind::Texture)).unwrap();
    assert!(c.provenance().iter().any(|p| p == "fab-id=abc123"));
    assert!(verify(&d.join("m.ewpack"), &registry(&d)).unwrap().errors.is_empty());
    // A source folder inside a repository is refused.
    std::fs::write(d.join(".git"), "gitdir: elsewhere\n").unwrap();
    assert!(build_err(&d, &m(true, "abc123")).contains("inside the repository"));
    std::fs::remove_dir_all(&d).ok();
}

/// Hand-built packs that `ew-bake build` would never write: `verify` must catch each.
#[test]
fn verify_fails_on_unknown_unlicensed_or_tampered_packs() {
    let d = scratch("verify");
    let reg = registry(&d);
    let info = |gp: bool| PackInfo { name: "v".into(), game_packaged: gp, tool: "test".into() };
    let full = |id: &str| ChunkMeta::new(1, 1).licence(id).provenance("src=x").provenance("tool=t").provenance(&format!("registry={id}"));
    let body = Opaque { kind: ChunkKind::Source, data: Cow::Borrowed(b"hello") };
    let key = ChunkKey::node(ed_assets::bake::proving_node(), ChunkKind::Source, ed_pack::Rung::R0);
    let pack = |name: &str, licences: &[&str], gp: bool, chunk: Chunk<'_>| {
        let mut w = PackWriter::new(1, 1);
        for l in licences {
            w.add_licence(l).unwrap();
        }
        w.add_licence("everyworld-original").unwrap();
        w.insert(info_key(), info(gp).to_chunk(&full("everyworld-original")).unwrap()).unwrap();
        w.insert(key, chunk).unwrap();
        let p = d.join(name);
        w.write_file(&p).unwrap();
        verify(&p, &reg).unwrap().errors
    };
    let errs = |n, l: &[&str], gp, m: ChunkMeta| pack(n, l, gp, Chunk::encode(&body, &m).unwrap());
    assert!(errs("ok.ewpack", &[], false, full("everyworld-original")).is_empty());
    assert!(errs("unknown.ewpack", &["mystery"], false, full("mystery")).iter().any(|e| e.contains("not in the registry")));
    assert!(errs("none.ewpack", &[], false, ChunkMeta::new(1, 1).provenance("src=x").provenance("tool=t")).iter().any(|e| e.contains("no licence id")));
    assert!(errs("red.ewpack", &["redlic"], false, full("redlic")).iter().any(|e| e.contains("RED")));
    assert!(errs("packonly.ewpack", &["packonly"], false, full("packonly")).iter().any(|e| e.contains("packaged game")));
    // (A chunk licence missing from the pack list cannot be produced: PackWriter unions them.)
    assert!(
        errs("noprov.ewpack", &[], false, ChunkMeta::new(1, 1).licence("everyworld-original"))
            .iter()
            .any(|e| e.contains("provenance has no `src=`"))
    );
    assert!(errs("nofab.ewpack", &["megascans"], true, full("megascans")).iter().any(|e| e.contains("fab-id")));
    // No pack-info chunk.
    let mut w = PackWriter::new(1, 1);
    w.add_licence("everyworld-original").unwrap();
    w.insert(key, Chunk::encode(&body, &full("everyworld-original")).unwrap()).unwrap();
    w.write_file(&d.join("noinfo.ewpack")).unwrap();
    assert!(verify(&d.join("noinfo.ewpack"), &reg).unwrap().errors.iter().any(|e| e.contains("pack-info")));
    // Tampered bytes: flip a byte inside the chunk payload ("hello").
    let mut raw = std::fs::read(d.join("ok.ewpack")).unwrap();
    let at = raw.windows(5).position(|w| w == b"hello").expect("payload stored uncompressed");
    raw[at] ^= 0x20;
    std::fs::write(d.join("tampered.ewpack"), &raw).unwrap();
    let e = verify(&d.join("tampered.ewpack"), &reg).map(|v| v.errors).unwrap_or_else(|e| vec![e.to_string()]);
    assert!(e.iter().any(|e| e.contains("hash")), "{e:?}");
    // The CLI exits non-zero on failure.
    let st = Command::new(env!("CARGO_BIN_EXE_ew-bake"))
        .args(["verify", d.join("unknown.ewpack").to_str().unwrap(), "--registry", d.join("data/registry").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!st.status.success());
    assert!(String::from_utf8_lossy(&st.stdout).contains("not in the registry"));
    std::fs::remove_dir_all(&d).ok();
}
