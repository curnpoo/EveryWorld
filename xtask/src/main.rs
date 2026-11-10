//! Repository tasks. Run as `cargo xtask <command>`.
//!
//! - `check-deps`    enforce the crate dependency rules in `xtask/deps.toml`
//! - `licence-gate`  fail if a data version names a RED dataset or an unreviewed YELLOW one
//! - `credits`       regenerate `CREDITS.md` from the registry (`--check` to verify only)
//! - `golden`        run the determinism suite (`--bless` to rewrite the golden hashes)

mod deps;
mod registry;

use anyhow::{Result, bail};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    match args.first().map(String::as_str) {
        Some("check-deps") => deps::check(&root()),
        Some("licence-gate") => registry::gate(&root()),
        Some("credits") => registry::credits(&root(), flag("--check")),
        Some("golden") => {
            let mut c = Command::new("cargo");
            c.current_dir(root()).env("EW_GOLDEN", "1").args(["test", "-p", "ed_world", "--test", "golden", "--release", "--", "--nocapture"]);
            if flag("--bless") {
                c.env("EW_GOLDEN_BLESS", "1");
            }
            if !c.status()?.success() {
                bail!("golden suite failed");
            }
            Ok(())
        }
        _ => {
            eprintln!("usage: cargo xtask <check-deps | licence-gate | credits [--check] | golden [--bless]>");
            std::process::exit(2);
        }
    }
}
