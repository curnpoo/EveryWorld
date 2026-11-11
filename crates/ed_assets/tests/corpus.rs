//! ~100 MB glTF corpus baked twice in separate processes (opt in: it writes ~100 MB under
//! `target/m3/corpus` and takes a while).
//!
//! `cargo test --release -p ed_assets --test corpus -- --ignored --nocapture`

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

#[test]
#[ignore = "writes ~100 MB under target/m3/corpus"]
fn hundred_mb_corpus_bakes_identically_in_two_processes() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/m3/corpus");
    std::fs::create_dir_all(&root).unwrap();
    let mut toml = String::from("[pack]\nname = \"corpus\"\n");
    let mut total = 0u64;
    let mut i = 0u32;
    while total < 100 * 1024 * 1024 {
        let name = format!("item{i:02}.glb");
        let p = root.join(&name);
        if !p.exists() {
            std::fs::write(&p, ed_assets::fixtures::corpus_item(i, 320, 1024).unwrap()).unwrap();
        }
        total += std::fs::metadata(&p).unwrap().len();
        toml += &format!("[[gltf]]\npath = \"{name}\"\nasset = \"corpus/item{i:02}\"\nregistry = \"everyworld-original\"\n");
        i += 1;
    }
    let src = root.join("corpus.toml");
    std::fs::write(&src, toml).unwrap();
    eprintln!("corpus: {i} files, {:.1} MB", total as f64 / 1048576.0);
    let exe = env!("CARGO_BIN_EXE_ew-bake");
    let time = std::path::Path::new("/usr/bin/time");
    let mut hashes = Vec::new();
    for run in 0..2 {
        let out = root.join(format!("corpus{run}.ewpack"));
        let t0 = Instant::now();
        let mut cmd = if time.exists() { Command::new(time) } else { Command::new(exe) };
        if time.exists() {
            cmd.arg("-l").arg(exe);
        }
        let o = cmd.args(["build", src.to_str().unwrap(), "-o", out.to_str().unwrap(), "-q"]).output().unwrap();
        let secs = t0.elapsed().as_secs_f64();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let err = String::from_utf8_lossy(&o.stderr);
        let rss = err.lines().find(|l| l.contains("maximum resident set size")).and_then(|l| l.split_whitespace().next()?.parse::<u64>().ok());
        let bytes = std::fs::read(&out).unwrap();
        let h = blake3::hash(&bytes).to_hex().to_string();
        eprintln!(
            "run {run}: {secs:.2} s, peak RSS {} MB, pack {:.1} MB, hash {h}",
            rss.map_or("?".into(), |r| format!("{:.0}", r as f64 / 1048576.0)),
            bytes.len() as f64 / 1048576.0
        );
        hashes.push(h);
    }
    assert_eq!(hashes[0], hashes[1]);
}
