use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct Rules {
    allow: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    forbid: Vec<Forbid>,
    banned_external: Banned,
}
#[derive(Deserialize)]
struct Forbid {
    from: Vec<String>,
    to: Vec<String>,
    why: String,
}
#[derive(Deserialize)]
struct Banned {
    crates: Vec<String>,
    why: String,
}

pub fn check(root: &Path) -> Result<()> {
    let rules: Rules = toml::from_str(&std::fs::read_to_string(root.join("xtask/deps.toml"))?)
        .context("xtask/deps.toml")?;
    let out = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--locked"])
        .output()?;
    if !out.status.success() {
        bail!("cargo metadata failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let members: BTreeSet<&str> =
        meta["workspace_members"].as_array().unwrap().iter().filter_map(|v| v.as_str()).collect();

    // Direct edges between workspace crates (normal and build deps; dev deps are exempt).
    let mut direct: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut names = BTreeSet::new();
    for p in meta["packages"].as_array().unwrap() {
        let name = p["name"].as_str().unwrap();
        names.insert(name.to_string());
        if !members.contains(p["id"].as_str().unwrap()) {
            continue;
        }
        let e = direct.entry(name.to_string()).or_default();
        for d in p["dependencies"].as_array().unwrap() {
            if d["kind"].as_str() == Some("dev") {
                continue;
            }
            if d.get("path").is_some_and(|v| !v.is_null()) {
                e.insert(d["name"].as_str().unwrap().to_string());
            }
        }
    }

    let mut errors = Vec::new();
    for (krate, deps) in &direct {
        let Some(allowed) = rules.allow.get(krate) else {
            errors.push(format!("{krate}: not listed in xtask/deps.toml [allow]"));
            continue;
        };
        for d in deps {
            if !allowed.contains(d) {
                errors.push(format!("{krate} -> {d}: not in the allowed list for {krate}"));
            }
        }
    }
    // Transitive closure for the forbid rules.
    let reach = |from: &str| {
        let mut seen = BTreeSet::new();
        let mut stack = vec![from.to_string()];
        while let Some(n) = stack.pop() {
            for d in direct.get(&n).into_iter().flatten() {
                if seen.insert(d.clone()) {
                    stack.push(d.clone());
                }
            }
        }
        seen
    };
    for f in &rules.forbid {
        for from in &f.from {
            let r = reach(from);
            for to in &f.to {
                if r.contains(to) {
                    errors.push(format!("{from} reaches {to}: forbidden. {}", f.why));
                }
            }
        }
    }
    for b in &rules.banned_external.crates {
        if names.contains(b) {
            errors.push(format!("external crate `{b}` is in the graph. {}", rules.banned_external.why));
        }
    }
    if errors.is_empty() {
        println!("check-deps: {} workspace crates, all edges allowed", direct.len());
        Ok(())
    } else {
        for e in &errors {
            eprintln!("check-deps: {e}");
        }
        bail!("{} dependency rule violation(s)", errors.len())
    }
}
