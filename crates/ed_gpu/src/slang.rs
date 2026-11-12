//! Slang shader compilation (P1-D1): one `.slang` source per module, compiled by `slangc` to
//! MSL (one text for every entry point of the module) and SPIR-V (one module per entry point,
//! so each carries only the capabilities of its own stage). Std only: no Slang library is
//! linked. Used by the checked-in shader generator (`tools/shaders/build.sh`), the freshness
//! tests, and runtime hot reload; release builds use the checked-in outputs and need no slangc.
//!
//! Sources include the binding ABI with `#include "shader_abi.slang"` (see [`ABI_SOURCE`]).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

/// The pinned Slang release. Other versions may emit different code, so the generator and
/// the freshness tests refuse them.
pub const SLANG_VERSION: &str = "2026.20";

/// `shader_abi.slang`: the bindless ABI for Slang sources (tables, constants, accessors).
pub const ABI_SOURCE: &str = include_str!("shader_abi.slang");

/// Every directory holding engine `.slang` modules, relative to the workspace root. Each has
/// its generated outputs under `gen/` (see [`generate_dir`]).
pub const SHADER_DIRS: &[&str] = &[
    "crates/ed_gpu/tests/shaders/slang",
    "crates/ed_render/shaders",
    "crates/ed_editor/shaders",
];

/// Flags shared by both targets: column-major matrices as in the Rust structs, no `#line`
/// noise in the checked-in output, and quiet about the D3D-only `register` annotations.
const COMMON_FLAGS: &[&str] = &[
    "-matrix-layout-column-major",
    "-line-directive-mode",
    "none",
    "-Wno-39001",
    "-Wno-39029",
];
const MSL_FLAGS: &[&str] = &["-target", "metal", "-DED_METAL"];
const SPIRV_FLAGS: &[&str] = &[
    "-target",
    "spirv",
    "-DED_SPIRV",
    "-fvk-use-entrypoint-name",
    "-emit-spirv-directly",
];

/// Pipeline stage of a Slang entry point (`[shader("...")]`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlangStage {
    Vertex,
    Fragment,
    Compute,
}

/// One compiled module: the MSL text for every entry point and one SPIR-V binary per entry.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct SlangOutput {
    pub msl: String,
    /// `(entry name, SPIR-V bytes)` in source order.
    pub spirv: Vec<(String, Vec<u8>)>,
}

/// Finds a `slangc` of the pinned version: `$SLANGC`, then `slangc` on PATH, then
/// `~/.local/opt/slang-<SLANG_VERSION>/bin/slangc`. `Err` says why none qualified.
pub fn find_slangc() -> Result<PathBuf, String> {
    let mut tried = Vec::new();
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Some(p) = std::env::var_os("SLANGC") {
        cands.push(PathBuf::from(p));
    }
    if let Some(path) = std::env::var_os("PATH") {
        cands.extend(std::env::split_paths(&path).map(|d| d.join("slangc")));
    }
    if let Some(home) = std::env::var_os("HOME") {
        cands
            .push(PathBuf::from(home).join(format!(".local/opt/slang-{SLANG_VERSION}/bin/slangc")));
    }
    for c in cands {
        if !c.is_file() {
            continue;
        }
        match Command::new(&c).arg("-v").output() {
            Ok(o) => {
                let v = String::from_utf8_lossy(&o.stdout).to_string()
                    + &String::from_utf8_lossy(&o.stderr);
                if v.trim() == SLANG_VERSION {
                    return Ok(c);
                }
                tried.push(format!("{} is version {}", c.display(), v.trim()));
            }
            Err(e) => tried.push(format!("{}: {e}", c.display())),
        }
    }
    Err(format!(
        "no slangc {SLANG_VERSION} (set SLANGC=<path>){}",
        if tried.is_empty() {
            String::new()
        } else {
            format!("; {}", tried.join("; "))
        }
    ))
}

/// Entry points declared in `src` with `[shader("vertex" | "fragment" | "compute")]`, in
/// source order (the function name is the last identifier before the parameter list).
pub fn entry_points(src: &str) -> Vec<(String, SlangStage)> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("[shader(\"") {
        rest = &rest[i + 9..];
        let Some(q) = rest.find('"') else { break };
        let stage = match &rest[..q] {
            "vertex" => SlangStage::Vertex,
            "fragment" | "pixel" => SlangStage::Fragment,
            "compute" => SlangStage::Compute,
            _ => continue,
        };
        // The declaration header after the attribute: attributes, return type, name, params.
        let end = rest.find(['{', ';']).unwrap_or(rest.len());
        let mut depth = 0u32;
        let head: String = rest[q + 1..end]
            .chars()
            .filter(|&c| match c {
                '[' => {
                    depth += 1;
                    false
                }
                ']' => {
                    depth = depth.saturating_sub(1);
                    false
                }
                _ => depth == 0,
            })
            .collect();
        let Some(open) = head.find('(') else { continue };
        let name: String = head[..open]
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        if !name.is_empty() {
            out.push((name, stage));
        }
    }
    out
}

/// Compiles module `name` (its text is `files[0]`; further `(file name, text)` pairs are
/// includable, and `shader_abi.slang` is always available). The files are written to a
/// private temporary directory, so this works for edited in-memory sources as well.
pub fn compile(slangc: &Path, files: &[(&str, &str)]) -> Result<SlangOutput, String> {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let (main, main_src) = *files.first().ok_or("no source")?;
    let dir = std::env::temp_dir().join(format!(
        "ed_slang_{}_{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let result = (|| {
        std::fs::write(dir.join("shader_abi.slang"), ABI_SOURCE).map_err(|e| e.to_string())?;
        for (f, text) in files {
            std::fs::write(dir.join(f), text).map_err(|e| e.to_string())?;
        }
        let run = |flags: &[&str], extra: &[&str], out: &str| -> Result<Vec<u8>, String> {
            let o = Command::new(slangc)
                .arg(main)
                .arg("-I")
                .arg(".")
                .args(COMMON_FLAGS)
                .args(flags)
                .args(extra)
                .arg("-o")
                .arg(out)
                .current_dir(&dir)
                .output()
                .map_err(|e| format!("{}: {e}", slangc.display()))?;
            let log = String::from_utf8_lossy(&o.stderr).to_string()
                + &String::from_utf8_lossy(&o.stdout);
            if !o.status.success() {
                return Err(format!("slangc {main} ({}): {}", flags[1], log.trim()));
            }
            std::fs::read(dir.join(out))
                .map_err(|e| format!("slangc {main}: no output ({e}); {}", log.trim()))
        };
        let entries = entry_points(main_src);
        if entries.is_empty() {
            return Err(format!("{main}: no [shader(\"...\")] entry points"));
        }
        let msl =
            String::from_utf8(run(MSL_FLAGS, &[], "out.metal")?).map_err(|e| e.to_string())?;
        let mut spirv = Vec::new();
        for (entry, stage) in &entries {
            let mut bytes = run(SPIRV_FLAGS, &["-entry", entry], &format!("{entry}.spv"))?;
            if *stage == SlangStage::Fragment {
                fragment_primitive_id_without_geometry(&mut bytes);
            }
            spirv.push((entry.clone(), bytes));
        }
        Ok(SlangOutput { msl, spirv })
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Slang declares `OpCapability Geometry` for `SV_PrimitiveID` in a fragment shader. SPIR-V
/// also accepts `Tessellation` for that built-in, and MoltenVK (no geometry shaders) supports
/// tessellation, so fragment modules swap one for the other (ed_gpu's Vulkan device enables
/// `tessellationShader`). Leaves any other use of the capability alone.
pub fn fragment_primitive_id_without_geometry(spv: &mut Vec<u8>) {
    const OP_CAPABILITY: u32 = (2 << 16) | 17;
    const GEOMETRY: u32 = 2;
    const TESSELLATION: u32 = 3;
    const OP_ENTRY_POINT: u32 = 15;
    if spv.len() < 20 || !spv.len().is_multiple_of(4) {
        return;
    }
    let mut w: Vec<u32> = spv
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect();
    let mut i = 5;
    let (mut geom_at, mut has_tess) = (None, false);
    while i < w.len() {
        let (count, op) = ((w[i] >> 16) as usize, w[i] & 0xffff);
        if count == 0 {
            return;
        }
        if w[i] == OP_CAPABILITY && i + 1 < w.len() {
            match w[i + 1] {
                GEOMETRY => geom_at = Some(i),
                TESSELLATION => has_tess = true,
                _ => {}
            }
        }
        // A geometry-stage entry point really needs the capability.
        if op == OP_ENTRY_POINT && w.get(i + 1) == Some(&3) {
            return;
        }
        i += count;
    }
    let Some(g) = geom_at else { return };
    if has_tess {
        w.drain(g..g + 2);
    } else {
        w[g + 1] = TESSELLATION;
    }
    *spv = w.iter().flat_map(|x| x.to_le_bytes()).collect();
}

/// Generated outputs of the Slang modules in `dir` (every `*.slang` with an entry point):
/// `(path relative to dir, bytes)`, `gen/msl/<stem>.metal` and `gen/spv/<stem>.<entry>.spv`.
/// Modules may include any other `.slang` file in `dir`.
pub fn generate_dir(slangc: &Path, dir: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>, String> {
    let mut srcs: Vec<(String, String)> = Vec::new();
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.extension().and_then(|x| x.to_str()) == Some("slang") {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            srcs.push((
                name,
                std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?,
            ));
        }
    }
    srcs.sort();
    let mut out = Vec::new();
    for (i, (name, text)) in srcs.iter().enumerate() {
        if entry_points(text).is_empty() {
            continue;
        }
        let mut files = vec![(name.as_str(), text.as_str())];
        files.extend(
            srcs.iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, (n, t))| (n.as_str(), t.as_str())),
        );
        let o = compile(slangc, &files)?;
        let stem = name.trim_end_matches(".slang");
        out.push((
            PathBuf::from(format!("gen/msl/{stem}.metal")),
            o.msl.into_bytes(),
        ));
        for (entry, bytes) in o.spirv {
            out.push((PathBuf::from(format!("gen/spv/{stem}.{entry}.spv")), bytes));
        }
    }
    Ok(out)
}

/// Compares [`generate_dir`] with the checked-in files under `dir`; returns the stale or
/// missing paths (also checked-in `gen/` files no module produces any more).
pub fn stale_outputs(slangc: &Path, dir: &Path) -> Result<Vec<String>, String> {
    let fresh = generate_dir(slangc, dir)?;
    let mut stale: Vec<String> = fresh
        .iter()
        .filter(|(p, b)| std::fs::read(dir.join(p)).ok().as_ref() != Some(b))
        .map(|(p, _)| p.display().to_string())
        .collect();
    for sub in ["gen/msl", "gen/spv"] {
        if let Ok(rd) = std::fs::read_dir(dir.join(sub)) {
            for e in rd.flatten() {
                let rel = PathBuf::from(sub).join(e.file_name());
                if !fresh.iter().any(|(p, _)| *p == rel) {
                    stale.push(format!("{} (orphan)", rel.display()));
                }
            }
        }
    }
    stale.sort();
    Ok(stale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_entry_points_with_attributes() {
        let src = r#"
            [shader("compute")]
            [numthreads(8, 8, 1)]
            void cs_main(uint3 id: SV_DispatchThreadID) {}
            [shader("vertex")] VsOut vs_a(uint v: SV_VulkanVertexID) { return o; }
            [shader("fragment")]
            float4 fs_b(VsOut i) : SV_Target0 { return 0; }
        "#;
        assert_eq!(
            entry_points(src),
            vec![
                ("cs_main".into(), SlangStage::Compute),
                ("vs_a".into(), SlangStage::Vertex),
                ("fs_b".into(), SlangStage::Fragment)
            ]
        );
    }

    #[test]
    fn geometry_capability_becomes_tessellation() {
        let words: [u32; 9] = [
            0x0723_0203,
            0x0001_0600,
            0,
            10,
            0,
            (2 << 16) | 17,
            2,
            (2 << 16) | 17,
            1,
        ];
        let mut b: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        fragment_primitive_id_without_geometry(&mut b);
        assert_eq!(&b[24..28], &3u32.to_le_bytes());
    }
}
