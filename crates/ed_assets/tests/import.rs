//! glTF import: skins, WebP, KHR_texture_transform, required extensions, Solarcat (opt-in).

use ed_assets::fixtures::{self, BAR_UV_OFFSET, BAR_UV_SCALE};
use ed_assets::gltf_import::{self, UvXf};
use ed_assets::material::{P_UV, T_BASE, T_ORM};
use ed_pack::AssetId;
use ed_pack::kinds::{TextureFormat, ColourRole};

#[test]
fn skinned_bar_sorts_joints_remaps_and_bakes_uv_transform() {
    let glb = fixtures::skinned_bar().unwrap();
    let imp = gltf_import::import(&glb, None, "test/bar").unwrap();

    // Skeleton: the skin lists [child, root]; we store parents first.
    assert_eq!(imp.skeletons.len(), 1);
    let (sk_name, sk) = &imp.skeletons[0];
    assert_eq!(sk_name, "test/bar/mesh0_0/skeleton");
    assert_eq!(sk.mesh, AssetId::from_path("test/bar/mesh0_0"));
    let names: Vec<&str> = sk.joints.iter().map(|j| j.name.as_str()).collect();
    assert_eq!(names, ["root", "child"]);
    assert_eq!(sk.joints[0].parent, None);
    assert_eq!(sk.joints[1].parent, Some(0));
    assert_eq!(sk.joints[1].rest.translation, [0.0, 1.0, 0.0]);
    // Inverse binds travel with their joints: the child's is translate(0, -1, 0).
    assert_eq!(sk.joints[1].inverse_bind[13], -1.0);
    assert_eq!(sk.joints[0].inverse_bind[13], 0.0);

    // Mesh: JOINTS remapped (source joint 1 = root -> 0, source 0 = child -> 1), weights kept.
    assert_eq!(imp.meshes.len(), 1);
    let b = &imp.meshes[0].build;
    assert!(b.is_skinned());
    for (v, p) in b.positions.iter().enumerate() {
        assert_eq!(b.joints[v][..2], [0, 1], "vertex {v}");
        let t = p[1] / 2.0;
        assert!((b.weights[v][0] - (1.0 - t)).abs() < 1e-6 && (b.weights[v][1] - t).abs() < 1e-6);
    }

    // The texture transform is the same on every texture of the material: baked into UVs.
    assert_eq!(imp.baked_uv_transforms, [0]);
    let xf = UvXf { offset: BAR_UV_OFFSET, scale: BAR_UV_SCALE, ..Default::default() };
    for (v, p) in b.positions.iter().enumerate() {
        // Source UV: u = corner / 4 (corner from x/z), v = ring / 3.
        let corner = match (p[0] > 0.0, p[2] > 0.0) {
            (false, false) => 0.0,
            (true, false) => 1.0,
            (true, true) => 2.0,
            (false, true) => 3.0,
        };
        let want = xf.apply([corner / 4.0, p[1] / 2.0]);
        assert!((b.uvs[v][0] - want[0]).abs() < 1e-5 && (b.uvs[v][1] - want[1]).abs() < 1e-5, "uv {v}: {:?} vs {want:?}", b.uvs[v]);
    }
    let (_, mat) = imp.materials.iter().find(|(a, _)| a == "test/bar/material0").expect("material0");
    assert!(!mat.params.contains_key(P_UV), "baked transform must not also be in the material");

    // WebP base colour -> BC7 sRGB with mips; metal-rough PNG -> ORM BC7 linear.
    let base = mat.textures.get(T_BASE).expect("base colour slot");
    let orm = mat.textures.get(T_ORM).expect("orm slot");
    let tex = |id: &AssetId| &imp.textures.iter().find(|(a, _)| AssetId::from_path(a) == *id).unwrap().1;
    assert_eq!((tex(base).format, tex(base).role, tex(base).width, tex(base).mips.len()), (TextureFormat::Bc7, ColourRole::Srgb, 16, 5));
    assert_eq!((tex(orm).format, tex(orm).role), (TextureFormat::Bc7, ColourRole::Linear));

    // Prefab: one node for the primitive, identity (glTF ignores the skinned node's transform).
    let (pname, prefab) = &imp.prefab;
    assert_eq!(pname, "test/bar");
    let n = prefab.nodes.iter().find(|n| n.mesh.is_some()).unwrap();
    assert_eq!(n.transform.translation, [0.0; 3]);
}

#[test]
fn non_uniform_transforms_stay_in_the_material() {
    // Different transforms on base colour and metal-rough: cannot bake, so carried as params.
    let glb = fixtures::skinned_bar().unwrap();
    let mut doc = fixtures::split_glb(&glb).unwrap();
    doc["materials"][0]["pbrMetallicRoughness"]["metallicRoughnessTexture"]["extensions"]["KHR_texture_transform"]["scale"] = serde_json::json!([3.0, 3.0]);
    let glb2 = fixtures::rejoin_glb(&glb, &doc);
    let imp = gltf_import::import(&glb2, None, "test/bar2").unwrap();
    assert!(imp.baked_uv_transforms.is_empty());
    let (_, mat) = imp.materials.iter().find(|(a, _)| a == "test/bar2/material0").unwrap();
    assert!(mat.params.contains_key(P_UV));
}

#[test]
fn unsupported_required_extension_is_rejected_by_name() {
    let err = gltf_import::import(&fixtures::requires("KHR_draco_mesh_compression"), None, "x").err().expect("must fail");
    let msg = format!("{err:#}");
    assert!(msg.contains("KHR_draco_mesh_compression"), "{msg}");
    // Supported ones load.
    gltf_import::import(&fixtures::requires("KHR_texture_transform"), None, "x").unwrap();
}

#[test]
fn import_is_deterministic() {
    let glb = fixtures::skinned_bar().unwrap();
    let a = gltf_import::import(&glb, None, "d").unwrap();
    let b = gltf_import::import(&glb, None, "d").unwrap();
    assert_eq!(a.meshes[0].build, b.meshes[0].build);
    assert_eq!(a.textures, b.textures);
}

/// The owner's private character. Opt in: `EW_SOLARCAT=/path/Solarcat.glb cargo test -p
/// ed_assets -- --ignored solarcat`.
#[test]
#[ignore = "needs EW_SOLARCAT (owner's private asset)"]
fn solarcat_imports_skinned_and_textured() {
    let Some(p) = std::env::var_os("EW_SOLARCAT") else {
        eprintln!("EW_SOLARCAT not set; skipping");
        return;
    };
    let imp = gltf_import::import_file(std::path::Path::new(&p), "characters/solarcat").unwrap();
    assert!(!imp.meshes.is_empty());
    assert!(imp.meshes.iter().all(|m| m.build.is_skinned()), "Solarcat is skinned");
    assert!(!imp.skeletons.is_empty());
    let sk = &imp.skeletons[0].1;
    for (i, j) in sk.joints.iter().enumerate() {
        if let Some(pa) = j.parent {
            assert!((pa as usize) < i, "parents first");
        }
    }
    assert!(imp.textures.iter().any(|(_, t)| t.format == TextureFormat::Bc7 && t.role == ColourRole::Srgb));
    let rep = &imp.meshes[0].report;
    assert!(rep.windows(2).all(|w| w[1].measured >= w[0].measured));
    eprintln!(
        "solarcat: {} meshes, {} joints, {} textures, lod0 {} tris",
        imp.meshes.len(),
        sk.joints.len(),
        imp.textures.len(),
        imp.meshes[0].build.indices.len() / 3
    );
}
