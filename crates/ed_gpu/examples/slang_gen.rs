//! Regenerates the checked-in Slang outputs (`<dir>/gen/msl/*.metal`, `<dir>/gen/spv/*.spv`)
//! for every engine shader directory. Run through `tools/shaders/build.sh` (or
//! `cargo run -p ed_gpu --example slang_gen [dir...]` from the workspace root). Needs slangc
//! (see `ed_gpu::slang::find_slangc`); validates SPIR-V with `spirv-val` when it is on PATH.

use ed_gpu::slang;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let dirs: Vec<PathBuf> = if args.is_empty() {
        slang::SHADER_DIRS.iter().map(|d| root.join(d)).collect()
    } else {
        args
    };
    let slangc = slang::find_slangc().unwrap_or_else(|e| fail(&e));
    let spirv_val = Command::new("spirv-val").arg("--version").output().is_ok();
    let mut n = 0;
    for dir in &dirs {
        if !dir.is_dir() {
            continue;
        }
        let files = slang::generate_dir(&slangc, dir).unwrap_or_else(|e| fail(&e));
        for sub in ["gen/msl", "gen/spv"] {
            let _ = std::fs::remove_dir_all(dir.join(sub));
            std::fs::create_dir_all(dir.join(sub)).unwrap();
        }
        for (rel, bytes) in &files {
            let path = dir.join(rel);
            std::fs::write(&path, bytes)
                .unwrap_or_else(|e| fail(&format!("{}: {e}", path.display())));
            if spirv_val && rel.extension().is_some_and(|e| e == "spv") {
                let o = Command::new("spirv-val")
                    .args(["--target-env", "vulkan1.3"])
                    .arg(&path)
                    .output()
                    .unwrap();
                if !o.status.success() {
                    fail(&format!(
                        "spirv-val {}: {}",
                        path.display(),
                        String::from_utf8_lossy(&o.stderr)
                    ));
                }
            }
            n += 1;
        }
    }
    println!(
        "slang_gen: {n} files ({}){}",
        slangc.display(),
        if spirv_val {
            ", spirv-val OK"
        } else {
            ", spirv-val not on PATH"
        }
    );
}

fn fail(msg: &str) -> ! {
    eprintln!("slang_gen: {msg}");
    std::process::exit(1)
}
