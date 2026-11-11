//! `ew-bake`: source TOML -> `.ewpack`.
//!
//! ```text
//! ew-bake build <source.toml> -o <out.ewpack>    bake (atomic write), prints hash + stats
//! ew-bake watch <source.toml> -o <out.ewpack>    rebake whenever an input changes
//! ew-bake verify <pack> [--registry DIR]         licence gate + chunk hash check
//! ew-bake hash <pack>                            BLAKE3 of the file
//! ```

use anyhow::{Context, Result, bail};
use ed_assets::{bake, licence};
use std::path::{Path, PathBuf};
use std::time::Duration;

const USAGE: &str = "usage:
  ew-bake build <source.toml> -o <out.ewpack> [-q]
  ew-bake watch <source.toml> -o <out.ewpack> [--interval-ms N] [--once-after N]
  ew-bake verify <pack.ewpack> [--registry DIR]
  ew-bake hash <pack.ewpack>";

fn main() {
    if let Err(e) = run() {
        eprintln!("ew-bake: error: {e:#}");
        std::process::exit(1);
    }
}

struct Args {
    pos: Vec<String>,
    out: Option<PathBuf>,
    registry: Option<PathBuf>,
    interval_ms: u64,
    once_after: Option<usize>,
    quiet: bool,
}

fn parse() -> Result<Args> {
    let mut a = Args { pos: vec![], out: None, registry: None, interval_ms: 250, once_after: None, quiet: false };
    let mut it = std::env::args().skip(1);
    while let Some(s) = it.next() {
        match s.as_str() {
            "-o" | "--out" => a.out = Some(it.next().context("-o needs a path")?.into()),
            "--registry" => a.registry = Some(it.next().context("--registry needs a folder")?.into()),
            "--interval-ms" => a.interval_ms = it.next().context("--interval-ms needs a number")?.parse()?,
            "--once-after" => a.once_after = Some(it.next().context("--once-after needs a number")?.parse()?),
            "-q" | "--quiet" => a.quiet = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            _ if s.starts_with('-') => bail!("unknown option {s}\n{USAGE}"),
            _ => a.pos.push(s),
        }
    }
    Ok(a)
}

fn print_build(rep: &bake::BakeReport, out: &Path, quiet: bool) {
    println!(
        "built {} -> {}  hash {}  {} bytes  {} keys  {} chunks  {} meshes  {} textures  {:.2} s",
        rep.name,
        out.display(),
        rep.file_hash,
        rep.bytes,
        rep.keys,
        rep.chunks,
        rep.meshes.len(),
        rep.textures,
        rep.seconds
    );
    if !quiet {
        // Per mesh: triangles / stored error (metres, non-decreasing) per LOD.
        for (name, lods) in rep.meshes.iter().take(64) {
            let s: Vec<String> = lods.iter().map(|l| format!("{}t/{:.4}m", l.triangles, l.error)).collect();
            if !s.is_empty() {
                println!("  {name}: {}", s.join(" "));
            }
        }
    }
}

fn run() -> Result<()> {
    let a = parse()?;
    let cmd = a.pos.first().map(String::as_str).unwrap_or("");
    match cmd {
        "build" => {
            let (src, out) = src_out(&a)?;
            let rep = bake::build(&src, &out)?;
            print_build(&rep, &out, a.quiet);
        }
        "watch" => {
            let (src, out) = src_out(&a)?;
            let mut builds = 0usize;
            let mut inputs: Vec<PathBuf> = vec![src.clone()];
            let mut last = None;
            println!("watching {} -> {} (Ctrl-C to stop)", src.display(), out.display());
            loop {
                let now = bake::stamps(&inputs);
                if last.as_ref() != Some(&now) {
                    match bake::build(&src, &out) {
                        Ok(rep) => {
                            print_build(&rep, &out, true);
                            inputs = rep.inputs.clone();
                            builds += 1;
                        }
                        Err(e) => eprintln!("ew-bake: build failed (keeping the previous pack): {e:#}"),
                    }
                    last = Some(bake::stamps(&inputs));
                    if a.once_after.is_some_and(|n| builds >= n) {
                        return Ok(());
                    }
                }
                std::thread::sleep(Duration::from_millis(a.interval_ms));
            }
        }
        "verify" => {
            let pack = PathBuf::from(a.pos.get(1).context(USAGE)?);
            let reg_dir = match &a.registry {
                Some(d) => d.clone(),
                None => licence::Registry::find(pack.parent().unwrap_or(Path::new(".")))
                    .or_else(|| licence::Registry::find(&std::env::current_dir().ok()?))
                    .context("no data/registry folder found above the pack or the current folder; pass --registry DIR")?,
            };
            let reg = licence::Registry::load(&reg_dir)?;
            let v = licence::verify(&pack, &reg)?;
            let gp = v.info.as_ref().map(|i| i.game_packaged);
            println!(
                "{}: {} keys, {} chunks, licences [{}], game_packaged {}",
                pack.display(),
                v.keys,
                v.chunks,
                v.licences.join(", "),
                gp.map_or("?".into(), |g| g.to_string())
            );
            if v.errors.is_empty() {
                println!("verify: OK");
            } else {
                for e in v.errors.iter().take(50) {
                    println!("  {e}");
                }
                if v.errors.len() > 50 {
                    println!("  ... {} more", v.errors.len() - 50);
                }
                bail!("verify FAILED: {} problem(s)", v.errors.len());
            }
        }
        "hash" => {
            let pack = PathBuf::from(a.pos.get(1).context(USAGE)?);
            println!("{}  {}", bake::file_hash(&pack)?, pack.display());
        }
        _ => bail!("{USAGE}"),
    }
    Ok(())
}

fn src_out(a: &Args) -> Result<(PathBuf, PathBuf)> {
    let src = PathBuf::from(a.pos.get(1).context(USAGE)?);
    let out = a.out.clone().context("missing -o <out.ewpack>")?;
    Ok((src, out))
}
