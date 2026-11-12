//! The Megascans library importer on synthetic folders (no Megascans data in the repository):
//! kind detection, supplied LOD chains with measured error, FBX through headless Blender,
//! provenance, the repository check, two-process determinism, `scan`, and loading the
//! resulting prefabs back through the pack readers.
//!
//! Blender tests print SKIP when Blender is missing, unless `EW_REQUIRE_BLENDER=1`.

use ed_assets::bake;
use ed_assets::fixtures::{self, lod_chain_glb, pattern};
use ed_assets::geom;
use ed_assets::licence::{Registry, verify};
use ed_assets::megascans::{P_MEGASCANS_KIND, T_TRANSLUCENCY};
use ed_assets::megascans_lib::{self as ml, Kind, KindSource, Listing};
use ed_assets::texture::{self, Rgba8};
use ed_pack::kinds::{AlphaMode, Material, Mesh, Param, Prefab, Texture};
use ed_pack::{AssetId, ChunkKey, ChunkKind, PackReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn scratch(name: &str) -> PathBuf {
    // In-process tests never read EW_MEGASCANS_DIR from the developer's shell.
    // SAFETY: no test in this binary sets it in-process.
    unsafe { std::env::remove_var(ml::ENV_DIR) };
    let d = std::env::temp_dir().join(format!("ew-megascans-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("data/registry")).unwrap();
    for (id, scr, redis, notes) in [("everyworld-original", "GREEN", "yes", ""), ("megascans", "YELLOW", "conditions", "Fab Standard License")] {
        let t = format!("id = \"{id}\"\nkind = \"asset\"\nlicence = \"test\"\nredistribution = \"{redis}\"\nscreening = \"{scr}\"\nreview_notes = \"{notes}\"\n");
        std::fs::write(d.join(format!("data/registry/{id}.toml")), t).unwrap();
    }
    d
}

fn png(path: &Path, img: &Rgba8) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, texture::encode_png(img).unwrap()).unwrap();
}

fn maps(dir: &Path, id: &str, names: &[&str]) {
    for (i, n) in names.iter().enumerate() {
        png(&dir.join(format!("{id}_2K_{n}.png")), &pattern(32, 32, 10 + i as u32));
    }
}

/// An opacity map: left half transparent.
fn opacity(w: u32, h: u32) -> Rgba8 {
    let mut d = Vec::new();
    for _y in 0..h {
        for x in 0..w {
            let a = if x < w / 2 { 0 } else { 255 };
            d.extend_from_slice(&[a, a, a, 255]);
        }
    }
    Rgba8::new(w, h, d).unwrap()
}

const ROCK: &[(u32, u32)] = &[(48, 24), (24, 12), (12, 6)];

/// A library with one asset of every kind (glTF meshes; no Blender needed).
fn library(root: &Path) {
    // Surface, metadata variant.
    let s = root.join("srf001");
    maps(&s, "srf001", &["Albedo", "Normal", "Roughness", "AO", "Displacement"]);
    std::fs::write(s.join("srf001.json"), r#"{"id": "srf001", "semanticTags": {"asset_type": "surface"}, "categories": ["surface", "ground"]}"#).unwrap();
    // 3D asset, file names only: GLB with three LOD nodes, LOD-specific normal.
    let r = root.join("rck001");
    maps(&r, "rck001", &["Albedo", "Roughness"]);
    png(&r.join("rck001_2K_Normal_LOD0.png"), &pattern(32, 32, 3));
    std::fs::write(r.join("rck001_LODs.glb"), lod_chain_glb("rck001", ROCK, 1.0)).unwrap();
    // Plant, two variants in sub-folders, alpha-tested atlas, file names only.
    let p = root.join("plt001");
    maps(&p, "plt001", &["Albedo", "Normal", "Translucency"]);
    png(&p.join("plt001_2K_Opacity.png"), &opacity(32, 32));
    for v in [1, 2] {
        std::fs::create_dir_all(p.join(format!("Var{v}"))).unwrap();
        std::fs::write(p.join(format!("Var{v}/Var{v}_LODs.glb")), lod_chain_glb(&format!("Var{v}"), &[(40 + 8 * v, 20), (20, 10), (10, 5)], 1.0)).unwrap();
    }
    // Decal, metadata variant.
    let d = root.join("dcl001");
    maps(&d, "dcl001", &["Albedo", "Normal"]);
    png(&d.join("dcl001_2K_Opacity.png"), &opacity(32, 32));
    std::fs::write(d.join("dcl001.json"), r#"{"id": "dcl001", "type": "decal"}"#).unwrap();
    // Imperfection, file names only (gloss -> roughness).
    let i = root.join("imp001");
    maps(&i, "imp001", &["Gloss"]);
}

const MANIFEST: &str = r#"megascans_dir = "lib"
[pack]
name = "mega"
game_packaged = true

[[megascans]]
fab_id = "srf001"
alias = "forest_floor"

[[megascans]]
fab_id = "rck001"
alias = "boulder"

[[megascans]]
fab_id = "plt001"
alias = "fern"

[[megascans]]
fab_id = "dcl001"

[[megascans]]
fab_id = "imp001"
"#;

fn source(d: &Path, text: &str) -> PathBuf {
    let p = d.join("src.toml");
    std::fs::write(&p, text).unwrap();
    p
}

// ---------------------------------------------------------------------------------------------
// Blender

fn blender() -> Option<PathBuf> {
    let b = ml::blender_path();
    if b.is_none() {
        if std::env::var("EW_REQUIRE_BLENDER").is_ok_and(|v| v == "1") {
            panic!("EW_REQUIRE_BLENDER=1 but Blender was not found (set EW_BLENDER)");
        }
        eprintln!("SKIP: Blender not found (set EW_BLENDER or EW_REQUIRE_BLENDER=1)");
    }
    b
}

fn blender_script(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/blender/megascans").join(name)
}

/// The FBX fixture (made once per test process): `<dir>/fbx001_LODs.fbx` with three LODs in
/// centimetres, plus `<dir>/split/fbx001_LOD<k>.fbx`.
fn fbx_fixture() -> Option<&'static PathBuf> {
    static F: OnceLock<Option<PathBuf>> = OnceLock::new();
    F.get_or_init(|| {
        let b = blender()?;
        let d = std::env::temp_dir().join(format!("ew-megascans-fbx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let out = d.join("fbx001_LODs.fbx");
        let o = Command::new(&b)
            .args(["--background", "--factory-startup", "--python-exit-code", "1", "--python"])
            .arg(blender_script("make_fixture.py"))
            .arg("--")
            .arg("--out")
            .arg(&out)
            .arg("--split")
            .arg(d.join("split"))
            .output()
            .unwrap();
        assert!(o.status.success() && out.is_file(), "make_fixture.py failed: {}", String::from_utf8_lossy(&o.stderr));
        Some(d)
    })
    .as_ref()
}

fn fbx_asset(root: &Path) -> bool {
    let Some(fx) = fbx_fixture() else { return false };
    let a = root.join("fbx001");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::copy(fx.join("fbx001_LODs.fbx"), a.join("fbx001_LODs.fbx")).unwrap();
    maps(&a, "fbx001", &["Albedo", "Normal", "Roughness"]);
    std::fs::write(a.join("fbx001.json"), r#"{"id": "fbx001", "categories": ["3d", "rock"]}"#).unwrap();
    true
}

// ---------------------------------------------------------------------------------------------
// Tests

#[test]
fn kind_detection_from_metadata_and_file_names() {
    let d = scratch("kinds");
    let lib = d.join("lib");
    library(&lib);
    let kind = |id: &str| ml::detect(&Listing::read(&lib.join(id)).unwrap());
    assert_eq!(kind("srf001").0, Kind::Surface);
    assert!(matches!(kind("srf001").1, KindSource::Metadata(_)));
    assert_eq!(kind("rck001"), (Kind::Model3d, KindSource::FileNames));
    assert_eq!(kind("plt001"), (Kind::Plant, KindSource::FileNames));
    assert_eq!(kind("dcl001").0, Kind::Decal);
    assert!(matches!(kind("dcl001").1, KindSource::Metadata(_)));
    assert_eq!(kind("imp001"), (Kind::Imperfection, KindSource::FileNames));
    // The other variant of each: metadata wins over file names; names alone still work.
    let x = lib.join("x");
    for (files, json, want) in [
        (&["Albedo", "Opacity"][..], None, Kind::Decal),
        (&["Albedo", "Normal", "Displacement"][..], None, Kind::Surface),
        (&["Albedo", "Normal"][..], Some(r#"{"semanticTags": {"asset_type": "3D plant"}}"#), Kind::Plant),
        (&["Albedo"][..], Some(r#"{"categories": ["imperfection"]}"#), Kind::Imperfection),
        (&["Albedo"][..], Some(r#"{"categories": ["3d", "rock"]}"#), Kind::Model3d),
        (&["Albedo", "Opacity", "Displacement"][..], Some(r#"{"categories": ["surface"]}"#), Kind::Surface),
    ] {
        let _ = std::fs::remove_dir_all(&x);
        maps(&x, "x", files);
        if let Some(j) = json {
            std::fs::write(x.join("x.json"), j).unwrap();
        }
        assert_eq!(ml::detect(&Listing::read(&x).unwrap()).0, want, "{files:?} {json:?}");
    }
    let _ = std::fs::remove_dir_all(&x);
    std::fs::create_dir_all(&x).unwrap();
    std::fs::write(x.join("x_LOD0.fbx"), b"not really").unwrap();
    maps(&x, "x", &["Albedo"]);
    assert_eq!(ml::detect(&Listing::read(&x).unwrap()).0, Kind::Model3d);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn supplied_lods_have_measured_rising_error() {
    let d = scratch("lods");
    let lib = d.join("lib");
    library(&lib);
    let m = ml::import_model(&Listing::read(&lib.join("rck001")).unwrap(), &ml::ModelOptions::default()).unwrap();
    assert!(!m.via_blender);
    assert_eq!(m.variants.len(), 1);
    let v = &m.variants[0];
    assert_eq!(v.supplied, [0, 1, 2], "the supplied chain is used as is");
    assert_eq!(v.lods.len(), 3);
    for w in v.report.windows(2) {
        assert!(w[1].triangles < w[0].triangles && w[1].error > w[0].error && w[1].error > 0.0, "{:?}", v.report);
    }
    // Stored error is the measurement (no simplifier involved for supplied levels).
    let s = ed_assets::lod::surface_samples(&v.build.positions, &v.lods[0].indices);
    for (l, r) in v.lods.iter().zip(&v.report).skip(1) {
        let d = ed_assets::lod::surface_distance(&v.build.positions, &l.indices, &s);
        assert!((l.error - d).abs() < 1e-6 && r.measured == d, "{r:?} vs {d}");
    }
    // Only two supplied levels: a third, coarser one is generated and measured too.
    let two = d.join("two");
    std::fs::create_dir_all(&two).unwrap();
    std::fs::write(two.join("two.glb"), lod_chain_glb("two", &[(64, 32), (32, 16)], 1.0)).unwrap();
    let m = ml::import_model(&Listing::read(&two).unwrap(), &ml::ModelOptions::default()).unwrap();
    let v = &m.variants[0];
    assert_eq!(v.supplied, [0, 1]);
    assert!(v.lods.len() >= 3, "{:?}", v.report);
    assert!(v.report.windows(2).all(|w| w[1].error >= w[0].error && w[1].triangles < w[0].triangles), "{:?}", v.report);
    assert!(m.notes.iter().any(|n| n.contains("generated")), "{:?}", m.notes);
    // Seven supplied levels: four kept (first, last and two between).
    let seven = d.join("seven");
    std::fs::create_dir_all(&seven).unwrap();
    std::fs::write(seven.join("s.glb"), lod_chain_glb("s", &[(96, 48), (72, 36), (48, 24), (32, 16), (24, 12), (16, 8), (8, 4)], 1.0)).unwrap();
    let m = ml::import_model(&Listing::read(&seven).unwrap(), &ml::ModelOptions::default()).unwrap();
    let v = &m.variants[0];
    assert_eq!(v.found.len(), 7);
    assert_eq!(v.lods.len(), ml::MAX_LODS);
    assert_eq!((v.supplied[0], *v.supplied.last().unwrap()), (0, 6));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn fbx_through_blender_is_metres_y_up_and_byte_stable() {
    let Some(fx) = fbx_fixture() else { return };
    let b = ml::blender_path().unwrap();
    let d = scratch("fbx");
    // Two Blender processes, same input: identical GLB bytes.
    let inputs = [fx.join("fbx001_LODs.fbx"), fx.join("split/fbx001_LOD1.fbx")];
    let a = ml::convert_fbx(&b, &inputs, &d.join("a")).unwrap();
    let c = ml::convert_fbx(&b, &inputs, &d.join("c")).unwrap();
    for (x, y) in a.iter().zip(&c) {
        assert_eq!(std::fs::read(x).unwrap(), std::fs::read(y).unwrap(), "Blender glTF output differs between runs: {}", x.display());
    }
    eprintln!("Blender FBX->GLB: byte-identical across two processes ({} files)", a.len());
    // Import: three LOD nodes in one file, centimetres -> metres, up stays +Y.
    let lib = d.join("lib");
    assert!(fbx_asset(&lib));
    let m = ml::import_model(&Listing::read(&lib.join("fbx001")).unwrap(), &ml::ModelOptions::default()).unwrap();
    assert!(m.via_blender);
    let v = &m.variants[0];
    assert_eq!(v.supplied, [0, 1, 2]);
    assert!(v.report.windows(2).all(|w| w[1].error > w[0].error && w[1].triangles < w[0].triangles), "{:?}", v.report);
    let bb = v.build.bounds();
    let size = [0, 1, 2].map(|k| bb.max[k] - bb.min[k]);
    assert!(size.iter().all(|s| (0.4..1.5).contains(s)), "50 cm rock should be ~1 m across in metres: {size:?}");
    // The fixture sits 35 cm above its origin along Blender +Z = glTF/our +Y.
    assert!(bb.min[1] > -0.2 && bb.max[1] > 0.6, "up axis is +Y: {bb:?}");
    // One FBX per LOD (the other Megascans layout) gives the same chain.
    let split = lib.join("fbx002");
    std::fs::create_dir_all(&split).unwrap();
    for k in 0..3 {
        std::fs::copy(fx.join(format!("split/fbx001_LOD{k}.fbx")), split.join(format!("fbx002_LOD{k}.fbx"))).unwrap();
    }
    let m2 = ml::import_model(&Listing::read(&split).unwrap(), &ml::ModelOptions::default()).unwrap();
    assert_eq!(m2.variants[0].report, v.report);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn library_bake_provenance_prefabs_and_materials() {
    let d = scratch("bake");
    library(&d.join("lib"));
    let rep = bake::build(&source(&d, MANIFEST), &d.join("m.ewpack")).unwrap();
    for w in &rep.warnings {
        eprintln!("warning: {w}");
    }
    assert!(rep.warnings.iter().any(|w| w.contains("fern") && w.contains("triangles per LOD")), "{:?}", rep.warnings);
    let reg = Registry::load(&d.join("data/registry")).unwrap();
    let v = verify(&d.join("m.ewpack"), &reg).unwrap();
    assert!(v.errors.is_empty(), "{:?}", v.errors);
    let r = PackReader::open_file(&d.join("m.ewpack")).unwrap();
    // Every Megascans chunk: licence megascans, its Fab id, its kind.
    let mut mega = 0;
    for e in r.entries() {
        let c = r.read_hash(&e.hash).unwrap();
        if c.licences().iter().any(|l| l == "megascans") {
            mega += 1;
            let p = c.provenance();
            assert!(p.iter().any(|s| s.starts_with("fab-id=") && s.len() > 7), "{p:?}");
            assert!(p.iter().any(|s| s.starts_with("src=")) && p.iter().any(|s| s.starts_with("megascans-kind=")), "{p:?}");
        }
    }
    assert!(mega > 20, "{mega}");
    let prov = |key: ChunkKey| r.read(&key).unwrap().provenance().to_vec();
    assert!(prov(ChunkKey::asset(AssetId::from_path("megascans/boulder/mesh"), ChunkKind::Mesh)).iter().any(|s| s == "fab-id=rck001"));
    assert!(prov(ChunkKey::asset(AssetId::from_path("megascans/fern/var1/mesh"), ChunkKind::Mesh)).iter().any(|s| s == "fab-id=plt001"));
    assert!(prov(ChunkKey::asset(AssetId::from_path("megascans/dcl001"), ChunkKind::Material)).iter().any(|s| s == "megascans-kind=decal"));
    // Decal / imperfection: material + textures, tagged, no mesh or prefab.
    let mat = |a: &str| -> Material { r.read(&ChunkKey::asset(AssetId::from_path(a), ChunkKind::Material)).unwrap().decode().unwrap() };
    let dm = mat("megascans/dcl001");
    assert_eq!(dm.params[P_MEGASCANS_KIND], Param::Float(Kind::Decal.code()));
    assert_eq!(dm.alpha_mode, AlphaMode::Blend);
    let im = mat("megascans/imp001");
    assert_eq!(im.params[P_MEGASCANS_KIND], Param::Float(Kind::Imperfection.code()));
    assert!(im.textures.contains_key("orm") && !im.textures.contains_key("base_colour"));
    assert!(r.read(&ChunkKey::asset(AssetId::from_path("megascans/dcl001"), ChunkKind::Prefab)).is_err());
    // Surface: unchanged path (no tag).
    assert!(!mat("megascans/forest_floor").params.contains_key(P_MEGASCANS_KIND));
    // Plant material: alpha-tested, double-sided, opacity in the albedo's alpha.
    let pm = mat("megascans/fern/material");
    assert_eq!((pm.alpha_mode, pm.double_sided), (AlphaMode::Mask, true));
    assert!(pm.textures.contains_key(T_TRANSLUCENCY));
    let alb_c = r.read(&ChunkKey::asset(pm.textures["base_colour"], ChunkKind::Texture)).unwrap();
    let alb: Texture = alb_c.decode().unwrap();
    assert_eq!(alb.width, 32);
    // Prefabs load through the pack readers the runtime uses (ew::content: Prefab -> Mesh via
    // ed_assets::geom::decode, Material, Texture), with no new runtime code.
    for (prefab, collides) in [("megascans/boulder", true), ("megascans/fern", false), ("megascans/fern/var1", false), ("megascans/fern/var2", false)] {
        let p: Prefab = r.read(&ChunkKey::asset(AssetId::from_path(prefab), ChunkKind::Prefab)).unwrap().decode().unwrap();
        p.validate().unwrap();
        assert_eq!(p.nodes.len(), 1, "{prefab}");
        let n = &p.nodes[0];
        let mesh_c = r.read(&ChunkKey::asset(n.mesh.unwrap(), ChunkKind::Mesh)).unwrap();
        let mesh: Mesh = mesh_c.decode().unwrap();
        let dm = geom::decode(&mesh).unwrap();
        assert!(dm.lods.len() >= 3 && dm.lods.len() <= ml::MAX_LODS, "{prefab}");
        assert!(dm.lods.windows(2).all(|w| w[1].error >= w[0].error && w[1].indices.len() < w[0].indices.len()), "{prefab}");
        assert!(!dm.build.uvs.is_empty());
        let m: Material = r.read(&ChunkKey::asset(n.material.unwrap(), ChunkKind::Material)).unwrap().decode().unwrap();
        for t in m.textures.values() {
            let _: Texture = r.read(&ChunkKey::asset(*t, ChunkKind::Texture)).unwrap().decode().unwrap();
        }
        assert_eq!(r.read(&ChunkKey::asset(AssetId::from_path(prefab), ChunkKind::CollisionAsset)).is_ok(), collides, "{prefab}");
    }
    let pv1: Prefab = r.read(&ChunkKey::asset(AssetId::from_path("megascans/fern/var1"), ChunkKind::Prefab)).unwrap().decode().unwrap();
    let pv2: Prefab = r.read(&ChunkKey::asset(AssetId::from_path("megascans/fern/var2"), ChunkKind::Prefab)).unwrap().decode().unwrap();
    assert_ne!(pv1.nodes[0].mesh, pv2.nodes[0].mesh);
    // No private path in the pack.
    let raw = std::fs::read(d.join("m.ewpack")).unwrap();
    let dir = d.to_string_lossy().into_owned();
    assert!(!raw.windows(dir.len()).any(|w| w == dir.as_bytes()));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn library_inside_the_repository_is_refused() {
    let d = scratch("inrepo");
    library(&d.join("lib"));
    std::fs::write(d.join(".git"), "gitdir: elsewhere\n").unwrap();
    let e = format!("{:#}", bake::build(&source(&d, MANIFEST), &d.join("x.ewpack")).unwrap_err());
    assert!(e.contains("inside the repository"), "{e}");
    // Missing ids and missing library roots are explained.
    std::fs::remove_file(d.join(".git")).unwrap();
    let e = format!("{:#}", bake::build(&source(&d, &MANIFEST.replace("srf001", "nope42")), &d.join("x.ewpack")).unwrap_err());
    assert!(e.contains("no folder named nope42"), "{e}");
    let e = format!("{:#}", bake::build(&source(&d, MANIFEST.trim_start_matches("megascans_dir = \"lib\"\n")), &d.join("x.ewpack")).unwrap_err());
    assert!(e.contains(ml::ENV_DIR), "{e}");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn two_processes_bake_identical_packs() {
    let d = scratch("det");
    let lib = d.join("library");
    library(&lib);
    let with_fbx = fbx_asset(&lib);
    let mut text = MANIFEST.trim_start_matches("megascans_dir = \"lib\"\n").to_string();
    if with_fbx {
        text.push_str("\n[[megascans]]\nfab_id = \"fbx001\"\nalias = \"fbx_rock\"\n");
    }
    let src = source(&d, &text);
    let exe = env!("CARGO_BIN_EXE_ew-bake");
    for out in ["a.ewpack", "b.ewpack"] {
        let o = Command::new(exe).env(ml::ENV_DIR, &lib).args(["build", src.to_str().unwrap(), "-o", d.join(out).to_str().unwrap(), "-q"]).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
    let a = std::fs::read(d.join("a.ewpack")).unwrap();
    assert_eq!(a, std::fs::read(d.join("b.ewpack")).unwrap(), "two processes, different packs");
    let r = PackReader::open_file(&d.join("a.ewpack")).unwrap();
    if with_fbx {
        let p: Prefab = r.read(&ChunkKey::asset(AssetId::from_path("megascans/fbx_rock"), ChunkKind::Prefab)).unwrap().decode().unwrap();
        assert!(r.read(&ChunkKey::asset(p.nodes[0].mesh.unwrap(), ChunkKind::Mesh)).unwrap().provenance().iter().any(|s| s == "fab-id=fbx001"));
    } else {
        eprintln!("SKIP: FBX asset left out of the determinism bake (no Blender)");
    }
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn scan_lists_the_library_and_writes_nothing() {
    let d = scratch("scan");
    let lib = d.join("lib");
    library(&lib);
    std::fs::create_dir_all(lib.join("bad001")).unwrap();
    std::fs::write(lib.join("bad001/bad001_4K_Albedo.exr"), b"exr").unwrap();
    std::fs::write(lib.join("bad001/bad001.zip"), b"zip").unwrap();
    let before = ml::Listing::read(&lib).unwrap().files;
    let o = Command::new(env!("CARGO_BIN_EXE_ew-bake")).args(["megascans", "scan", lib.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let out = String::from_utf8_lossy(&o.stdout);
    eprintln!("{out}");
    for want in [
        "srf001  kind=surface (metadata srf001.json)",
        "rck001  kind=3d (file names)",
        "plt001  kind=3d-plant (file names)",
        "dcl001  kind=decal (metadata dcl001.json)",
        "imp001  kind=imperfection (file names)",
        "Albedo 2K png 32x32",
        "Normal 2K LOD0 png 32x32",
        "mesh glb: 2 file(s), variants Var1 Var2",
        "triangles: LOD0 2304, LOD1 576, LOD2 144",
        "problem: unextracted archive bad001.zip",
        "problem: Albedo only as EXR",
        "6 asset(s), 1 with problems",
    ] {
        assert!(out.contains(want), "missing {want:?} in\n{out}");
    }
    assert_eq!(ml::Listing::read(&lib).unwrap().files, before, "scan wrote files");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn surface_entries_by_dir_are_unchanged() {
    // The pre-library form (`dir = ...`) still bakes a surface the same way as `import_dir`.
    let d = scratch("legacy");
    let s = d.join("outside/rock");
    maps(&s, "rock", &["Albedo", "Normal", "Roughness"]);
    let text = "[pack]\nname = \"m\"\ngame_packaged = true\n[[megascans]]\ndir = \"outside/rock\"\nasset = \"surfaces/rock\"\nfab_id = \"abc123\"\n";
    bake::build(&source(&d, text), &d.join("m.ewpack")).unwrap();
    let r = PackReader::open_file(&d.join("m.ewpack")).unwrap();
    let direct = ed_assets::megascans::import_dir(&s, "surfaces/rock", false).unwrap();
    let m: Material = r.read(&ChunkKey::asset(AssetId::from_path("surfaces/rock"), ChunkKind::Material)).unwrap().decode().unwrap();
    assert_eq!(m, direct.material.1);
    for (a, t) in &direct.textures {
        let got_c = r.read(&ChunkKey::asset(AssetId::from_path(a), ChunkKind::Texture)).unwrap();
        let got: Texture = got_c.decode().unwrap();
        assert_eq!(&got, t, "{a}");
    }
    let _ = fixtures::pattern; // keep the import used when tests are filtered
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn heavy_plants_warn_but_import() {
    let d = scratch("heavy");
    let p = d.join("lib/hvy001");
    maps(&p, "hvy001", &["Albedo"]);
    png(&p.join("hvy001_2K_Opacity.png"), &opacity(32, 32));
    // 1000 x 260 x 2 = 520k triangles at LOD 0.
    std::fs::write(p.join("hvy001_LODs.glb"), lod_chain_glb("hvy001", &[(1000, 260), (100, 50), (25, 12)], 1.0)).unwrap();
    let text = "megascans_dir = \"lib\"\n[pack]\nname = \"h\"\ngame_packaged = true\n[[megascans]]\nfab_id = \"hvy001\"\nkind = \"3d-plant\"\n";
    let rep = bake::build(&source(&d, text), &d.join("h.ewpack")).unwrap();
    assert!(rep.warnings.iter().any(|w| w.contains("WARNING") && w.contains("520000 triangles")), "{:?}", rep.warnings);
    assert!(rep.warnings.iter().any(|w| w.contains("520000t 10000t 600t")), "{:?}", rep.warnings);
    std::fs::remove_dir_all(&d).ok();
}

// ---------------------------------------------------------------------------------------------
// Fab layouts: tier-folder 3D scans and plants (synthetic; mirrors the real download structure)

fn write(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// 3D scan: `high/ mid/ low/` each with `<Name>_<ms>_<Tier>.gltf` + `.bin` (every node named
/// LOD0, as Fab does) and `textures/<Name>_<ms>_<Tier>_<4K|2K|1K>_{albedo,normal,orm}.png`.
/// The ORM has R = 200 (occlusion), G = 100 (roughness), B = 30 (metalness).
fn tier_scan(root: &Path, with_mid: bool) {
    let a = root.join("fab-scan-uuid__stone");
    let tiers: &[(&str, &str, u32, u32, (u32, u32))] = &[("high", "High", 4, 32, (48, 24)), ("mid", "Mid", 2, 16, (24, 12)), ("low", "Low", 1, 8, (12, 6))];
    for &(dir, tier, k, px, level) in tiers {
        if dir == "mid" && !with_mid {
            continue;
        }
        let stem = format!("Stone_ms1_{tier}");
        let (j, b) = fixtures::external_gltf("stone_LOD0", &[level], &format!("{stem}.bin"), false);
        write(&a.join(format!("{dir}/{stem}.gltf")), &j);
        write(&a.join(format!("{dir}/{stem}.bin")), &b);
        let t = a.join(format!("{dir}/textures/{stem}_{k}K"));
        png(&PathBuf::from(format!("{}_albedo.png", t.display())), &pattern(px, px, 1));
        png(&PathBuf::from(format!("{}_normal.png", t.display())), &pattern(px, px, 2));
        let orm = Rgba8::new(px, px, (0..px * px).flat_map(|_| [200u8, 100, 30, 255]).collect()).unwrap();
        png(&PathBuf::from(format!("{}_orm.png", t.display())), &orm);
    }
    // The 8K Raw tier, if it was extracted, is never read.
    write(&a.join("raw/Stone_ms1_Raw.gltf"), b"not even json");
}

#[test]
fn tier_folder_scans_are_the_lod_chain_with_one_texture_set() {
    let d = scratch("tiers");
    let lib = d.join("lib");
    tier_scan(&lib, true);
    let l = Listing::read(&lib.join("fab-scan-uuid__stone")).unwrap();
    assert_eq!(ml::detect(&l).0, Kind::Model3d);
    assert!(l.meshes.iter().all(|m| !m.high && m.tier_lod.is_some()), "raw ignored, tiers are LODs: {:?}", l.meshes);
    let m = ml::import_model(&l, &ml::ModelOptions::default()).unwrap();
    let v = &m.variants[0];
    assert_eq!(v.supplied, [0, 1, 2], "High, Mid, Low = LOD0..2 although every node is named LOD0");
    assert_eq!(v.lods.len(), 3);
    assert!(v.report.windows(2).all(|w| w[1].triangles < w[0].triangles && w[1].error > w[0].error), "{:?}", v.report);
    // 2K asks for the Mid set; unspecified takes the largest (High); every map from one tier.
    let sel = |res: Option<u32>| ml::select_maps(&l, res).0;
    for (res, px) in [(Some(2048), "Mid_2K"), (None, "High_4K")] {
        let s = sel(res);
        for p in [s.albedo.as_ref().unwrap(), s.normal.as_ref().unwrap(), s.orm.as_ref().unwrap()] {
            assert!(p.to_string_lossy().contains(px), "{res:?}: {p:?}");
        }
    }
    // ORM is carried as is (R occlusion, G roughness, B metalness: the glTF convention).
    let maps = sel(Some(2048));
    let s = ml::import_textures(Kind::Model3d, &maps, "megascans/stone", false, Some("megascans/stone/material".into())).unwrap();
    let orm = &s.textures.iter().find(|(n, _)| n.ends_with("/orm")).unwrap().1;
    assert_eq!((orm.width, orm.height), (16, 16));
    let (_, mat) = &s.material;
    assert!(mat.textures.contains_key("orm") && mat.textures.contains_key("normal"));
    // A scan without a Mid tier: two supplied LODs, a third is generated.
    let lib2 = d.join("lib2");
    tier_scan(&lib2, false);
    let m = ml::import_model(&Listing::read(&lib2.join("fab-scan-uuid__stone")).unwrap(), &ml::ModelOptions::default()).unwrap();
    assert_eq!(m.variants[0].supplied, [0, 2]);
    assert!(m.variants[0].lods.len() >= 3 && m.notes.iter().any(|n| n.contains("generated")), "{:?}", m.notes);
    std::fs::remove_dir_all(&d).ok();
}

/// Plant: `<ms>_tier_N.gltf` + `.bin` per tier folder, `standard/<ms>_tier_N_nonUE.gltf` using
/// `../`, `Textures/T_<ms>_<4K|2K>_{B-O,B,N,ORT,Billboard_B-O,Billboard_N-T}.png`, `<ms>.json`.
/// The glTF holds one variant (VarA) with LOD0..LOD2 and a billboard LOD3. ORT = (200, 100, 60).
fn fab_plant(root: &Path, keep_top_level: bool) -> PathBuf {
    let a = root.join("fab-plant-uuid__bush");
    for (n, dir, k, px, lod0) in [(1, "high", 4, 32u32, (64u32, 32u32)), (2, "mid", 2, 16, (32, 16))] {
        let stem = format!("plantms_tier_{n}");
        let levels = [lod0, (16, 8), (8, 4), (1, 1)];
        let (j, b) = fixtures::external_gltf("SM_plantms_VarA", &levels, &format!("{stem}.bin"), true);
        if keep_top_level {
            write(&a.join(format!("{dir}/{stem}.gltf")), &j);
        }
        write(&a.join(format!("{dir}/{stem}.bin")), &b);
        let (js, _) = fixtures::external_gltf("SM_plantms_VarA", &levels, &format!("../{stem}.bin"), true);
        write(&a.join(format!("{dir}/standard/{stem}_nonUE.gltf")), &js);
        let t = |s: &str| a.join(format!("{dir}/Textures/T_plantms_{k}K_{s}.png"));
        let bo = Rgba8::new(px, px, (0..px * px).flat_map(|i| [10u8, 120, 30, if i % 2 == 0 { 0 } else { 255 }]).collect()).unwrap();
        png(&t("B-O"), &bo);
        png(&t("B"), &pattern(px, px, 4));
        png(&t("N"), &pattern(px, px, 5));
        let ort = Rgba8::new(px, px, (0..px * px).flat_map(|_| [200u8, 100, 60, 255]).collect()).unwrap();
        png(&t("ORT"), &ort);
        png(&t("Billboard_B-O"), &pattern(px, px, 6));
        png(&t("Billboard_N-T"), &pattern(px, px, 7));
        write(&a.join(format!("{dir}/plantms.json")), br#"{"id": "plantms", "semanticTags": {"asset_type": "3D plant"}}"#);
    }
    a
}

#[test]
fn fab_plants_use_packed_maps_a_tier_file_and_tag_the_billboard() {
    let d = scratch("plant");
    let a = fab_plant(&d.join("lib"), true);
    let l = Listing::read(&a).unwrap();
    assert_eq!(ml::detect(&l).0, Kind::Plant);
    let s = ml::select_maps(&l, Some(2048)).0;
    assert!(s.albedo_alpha && s.albedo.as_ref().unwrap().to_string_lossy().ends_with("mid/Textures/T_plantms_2K_B-O.png"), "{s:?}");
    assert!(s.ort.is_some() && s.billboard_albedo.is_some() && s.billboard_normal.is_some() && s.normal.is_some() && s.opacity.is_none());
    // Mesh: the mid tier's top-level file only (not the standard/ copy, not the other tier).
    let src = ml::model_sources(&l, Some(2048)).unwrap();
    assert_eq!(src.len(), 1);
    assert!(src[0].path.ends_with("mid/plantms_tier_2.gltf"), "{:?}", src[0].path);
    let m = ml::import_model(&l, &ml::ModelOptions { scale: 1.0, want_res: Some(2048) }).unwrap();
    let v = &m.variants[0];
    assert_eq!(v.report.len(), 3, "LOD0..2; the billboard is not in the chain: {:?}", v.report);
    assert_eq!(v.supplied, [0, 1, 2]);
    assert_eq!(v.billboard_tris, Some(2));
    assert_eq!(v.report[0].triangles, 32 * 16 * 2);
    // Without a wanted size the highest tier is used.
    let hi = ml::import_model(&l, &ml::ModelOptions::default()).unwrap();
    assert_eq!(hi.variants[0].report[0].triangles, 64 * 32 * 2);
    // Material: B-O is the albedo (alpha = opacity), ORT -> ORM with B cleared, translucency
    // from ORT's B, billboard pair kept, alpha-masked and double-sided.
    let maps = ml::select_maps(&l, Some(2048)).0;
    let sf = ml::import_textures(Kind::Plant, &maps, "megascans/bush", false, Some("megascans/bush/material".into())).unwrap();
    let (_, mat) = &sf.material;
    assert_eq!((mat.alpha_mode, mat.double_sided), (AlphaMode::Mask, true));
    for slot in ["base_colour", "normal", "orm", T_TRANSLUCENCY, "billboard_albedo", "billboard_normal"] {
        assert!(mat.textures.contains_key(slot), "{slot}");
    }
    assert_eq!(sf.textures.len(), 6);
    // standard/ only: `../x.bin` stays inside the asset folder, so it loads.
    let only_std = fab_plant(&d.join("lib_std"), false);
    let ls = Listing::read(&only_std).unwrap();
    let src = ml::model_sources(&ls, Some(2048)).unwrap();
    assert!(src[0].standard);
    let m = ml::import_model(&ls, &ml::ModelOptions { scale: 1.0, want_res: Some(2048) }).unwrap();
    assert_eq!(m.variants[0].report.len(), 3);
    assert!(m.sources.iter().any(|p| p.ends_with("mid/plantms_tier_2.bin")), "{:?}", m.sources);
    // A URI that leaves the asset folder is refused.
    let (bad, _) = fixtures::external_gltf("SM_plantms_VarA", &[(16, 8)], "../../../outside.bin", false);
    write(&only_std.join("mid/standard/plantms_tier_2_nonUE.gltf"), &bad);
    let err = ml::import_model(&Listing::read(&only_std).unwrap(), &ml::ModelOptions { scale: 1.0, want_res: Some(2048) }).err().expect("escape refused");
    assert!(format!("{err:#}").contains("leaves the asset folder"), "{err:#}");
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn fab_layouts_bake_with_provenance_and_scan_clean() {
    let d = scratch("fabbake");
    let lib = d.join("lib");
    tier_scan(&lib, true);
    fab_plant(&lib, true);
    let text = "megascans_dir = \"lib\"\n[pack]\nname = \"f\"\ngame_packaged = true\n[[megascans]]\nfab_id = \"fab-scan-uuid\"\nalias = \"stone\"\n[[megascans]]\nfab_id = \"fab-plant-uuid\"\nalias = \"bush\"\n";
    let rep = bake::build(&source(&d, text), &d.join("f.ewpack")).unwrap();
    assert!(rep.warnings.iter().any(|w| w.contains("bush") && w.contains("billboard")), "{:?}", rep.warnings);
    let r = PackReader::open_file(&d.join("f.ewpack")).unwrap();
    let prov = r.read(&ChunkKey::asset(AssetId::from_path("megascans/bush/material"), ChunkKind::Material)).unwrap().provenance().to_vec();
    assert!(prov.iter().any(|s| s == "megascans-id=plantms") && prov.iter().any(|s| s == "fab-id=fab-plant-uuid"), "{prov:?}");
    let mesh_c = r.read(&ChunkKey::asset(AssetId::from_path("megascans/stone/mesh"), ChunkKind::Mesh)).unwrap();
    let mesh: Mesh = mesh_c.decode().unwrap();
    assert_eq!(geom::decode(&mesh).unwrap().lods.len(), 3);
    // Scan: a Fab id folder whose metadata id differs is not a problem.
    let e = ml::scan_asset(&lib.join("fab-plant-uuid__bush")).unwrap();
    assert!(e.problems.is_empty(), "{:?}", e.problems);
    assert_eq!(e.megascans_id.as_deref(), Some("plantms"));
    assert!(e.triangles.iter().any(|t| t.contains("billboard")), "{:?}", e.triangles);
    std::fs::remove_dir_all(&d).ok();
}
