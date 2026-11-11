//! The BIO_FOR Blender kit (tools/blender/kit) as an import test: sockets become prefab
//! connection points, KHR_materials_transmission maps to plain glass, LOD error stays
//! monotonic. Also reports whether the 3 cm battens of the timber wall survive LOD 1.
//! Skips when the kit has not been built:
//! `Blender --background --factory-startup --python tools/blender/kit/build_kit.py -- --out target/kit`.

use ed_assets::gltf_import;
use ed_assets::material::P_TRANSMISSION;
use ed_pack::kinds::PointKind;
use std::path::PathBuf;

fn kit_dir() -> Option<PathBuf> {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/kit/glb");
    d.is_dir().then_some(d)
}

fn socket_nodes(glb: &[u8]) -> usize {
    let doc = ed_assets::fixtures::split_glb(glb).unwrap();
    doc["nodes"].as_array().map_or(0, |n| n.iter().filter(|n| n["name"].as_str().is_some_and(|s| s.starts_with("SOCKET_"))).count())
}

/// Area of triangles lying in the plane z = `z` (the batten faces, outer face is +Z in glTF).
fn area_at_z(p: &[[f32; 3]], idx: &[u32], z: f32) -> f32 {
    idx.chunks(3)
        .filter(|t| t.iter().all(|&i| (p[i as usize][2] - z).abs() < 1e-3))
        .map(|t| {
            let [a, b, c] = [p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]];
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
        })
        .sum()
}

#[test]
fn kit_parts_import_with_sockets_glass_and_monotonic_lods() {
    let Some(dir) = kit_dir() else {
        eprintln!("target/kit/glb missing (build the kit first); skipping");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "glb")).collect();
    files.sort();
    assert_eq!(files.len(), 19, "kit has 19 parts");
    let (mut glass, mut sockets) = (0, 0);
    for f in &files {
        let name = f.file_stem().unwrap().to_str().unwrap();
        let bytes = std::fs::read(f).unwrap();
        let imp = gltf_import::import(&bytes, None, &format!("kit/{name}")).unwrap();
        let prefab = &imp.prefab.1;
        prefab.validate().unwrap();
        let want = socket_nodes(&bytes);
        assert_eq!(prefab.points.len(), want, "{name}: one connection point per SOCKET_ node");
        assert!(prefab.points.iter().all(|p| p.kind == PointKind::Connection && p.name.starts_with("socket:")));
        assert!(prefab.nodes.iter().all(|n| !n.name.starts_with("SOCKET_")), "{name}: sockets are points, not meshes");
        sockets += want;
        glass += imp.materials.iter().filter(|(_, m)| m.params.contains_key(P_TRANSMISSION)).count();
        for m in &imp.meshes {
            assert!(m.lods.windows(2).all(|w| w[1].error >= w[0].error && w[1].indices.len() < w[0].indices.len()), "{}", m.asset);
        }
        if name == "BIO_FOR_WALL_TIMBER_A" {
            // mesh0_0 is the board-and-batten cladding.
            let m = &imp.meshes[0];
            let p = &m.build.positions;
            let zmax = m.lods[0].indices.iter().map(|&i| p[i as usize][2]).fold(f32::MIN, f32::max);
            let a0 = area_at_z(p, &m.lods[0].indices, zmax);
            let tris: Vec<String> = m.lods.iter().map(|l| format!("{}t/{:.3}m", l.indices.len() / 3, l.error)).collect();
            eprintln!("WALL_TIMBER_A cladding LODs: {}", tris.join(" "));
            for (k, l) in m.lods.iter().enumerate().skip(1) {
                let a = area_at_z(p, &l.indices, zmax);
                eprintln!("  LOD{k}: batten face area {a:.3} m2 of {a0:.3} m2 at LOD0 ({:.0}%)", 100.0 * a / a0);
            }
        }
    }
    assert!(glass >= 3, "the three glazed parts carry a transmission (glass) material: {glass}");
    eprintln!("kit: {} parts, {sockets} sockets, {glass} glass materials", files.len());
}
