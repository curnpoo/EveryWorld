//! The checked-in Slang outputs (`gen/msl`, `gen/spv` in every `slang::SHADER_DIRS` entry)
//! must match their sources. Recompiles them with slangc 2026.20 (skipped when none is
//! found) and compares the bytes. On a mismatch run `tools/shaders/build.sh`.

use ed_gpu::slang;
use std::path::Path;

#[test]
fn checked_in_slang_outputs_match_sources() {
    let slangc = match slang::find_slangc() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("SKIP: {e}");
            return;
        }
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut stale = Vec::new();
    for dir in slang::SHADER_DIRS {
        let d = root.join(dir);
        if d.is_dir() {
            stale.extend(
                slang::stale_outputs(&slangc, &d)
                    .unwrap_or_else(|e| panic!("{dir}: {e}"))
                    .into_iter()
                    .map(|p| format!("{dir}/{p}")),
            );
        }
    }
    assert!(
        stale.is_empty(),
        "stale generated shaders {stale:#?}; run tools/shaders/build.sh"
    );
}
