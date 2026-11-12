//! The checked-in SPIR-V in `tests/shaders/spv/` must match its GLSL source. Recompiles every
//! shader with `glslangValidator` (skipped when it is not on PATH) and compares the bytes.
//! On a mismatch run `tests/shaders/build.sh`.

use std::path::Path;
use std::process::Command;

#[test]
fn checked_in_spirv_matches_glsl_sources() {
    if Command::new("glslangValidator").arg("--version").output().is_err() {
        eprintln!("SKIP: glslangValidator not on PATH");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("tests/shaders");
    let out_dir = std::env::temp_dir().join(format!("ed_gpu_spv_{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let mut sources: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("vert" | "frag" | "comp")))
        .collect();
    sources.sort();
    assert!(!sources.is_empty(), "no shaders in {}", dir.display());
    let mut stale = Vec::new();
    for src in &sources {
        let name = src.file_name().unwrap().to_str().unwrap();
        let fresh = out_dir.join(format!("{name}.spv"));
        let st = Command::new("glslangValidator")
            .args(["-V", "--target-env", "vulkan1.3", "-I../../src", name, "-o"])
            .arg(&fresh)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(st.status.success(), "{name}: {}", String::from_utf8_lossy(&st.stdout));
        let checked_in = std::fs::read(dir.join("spv").join(format!("{name}.spv"))).unwrap_or_default();
        if std::fs::read(&fresh).unwrap() != checked_in {
            stale.push(name.to_string());
        }
    }
    let _ = std::fs::remove_dir_all(&out_dir);
    assert!(stale.is_empty(), "stale SPIR-V for {stale:?}; run crates/ed_gpu/tests/shaders/build.sh");
}
