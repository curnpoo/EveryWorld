//! Builds the vendored Box3D (`vendor/box3d`, see its `VENDORED.md`) and our C shim
//! (`csrc/ed_box3d_shim.c`) with the `cc` crate. No CMake at build time.
//!
//! Settings mirror upstream's CMakeLists.txt at the pinned commit, plus double precision.
//! Kept conservative so the same script works with clang (macOS), MSVC and clang-cl
//! (Windows) and gcc or clang (Linux).

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let vendor = manifest.join("../../vendor/box3d");
    let src = vendor.join("src");
    let include = vendor.join("include");
    let shim = manifest.join("csrc/ed_box3d_shim.c");

    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed={}", include.display());
    println!("cargo:rerun-if-changed={}", shim.display());
    println!("cargo:rerun-if-changed=build.rs");

    // Sorted list of translation units: the archive content must not depend on readdir order.
    let mut units: Vec<PathBuf> = std::fs::read_dir(&src)
        .expect("vendor/box3d/src missing")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "c"))
        .collect();
    units.sort();
    assert!(units.len() > 40, "vendor/box3d/src looks incomplete ({} .c files)", units.len());

    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let debug_asserts = std::env::var("CARGO_CFG_DEBUG_ASSERTIONS").is_ok();

    let base = |b: &mut cc::Build| {
        b.include(&include).include(&src).define("BOX3D_DOUBLE_PRECISION", None).opt_level(3).warnings(false);
        if !debug_asserts {
            b.define("NDEBUG", None);
        }
        let c = b.get_compiler();
        if c.is_like_clang_cl() {
            b.flag("/std:c17");
            b.flag("/clang:-ffp-contract=off");
        } else if c.is_like_msvc() {
            b.flag("/std:c17");
            b.flag("/fp:precise");
            // MSVC 17.x keeps contractions off under /fp:precise unless /fp:contract is given.
        } else {
            b.flag("-std=gnu17");
            b.flag("-ffp-contract=off");
            b.flag("-fno-fast-math");
        }
    };

    let mut lib = cc::Build::new();
    base(&mut lib);
    let msvc_x86 = {
        let c = lib.get_compiler();
        c.is_like_msvc() && !c.is_like_clang_cl() && (target_arch == "x86_64" || target_arch == "x86")
    };
    for u in &units {
        if msvc_x86 && is_file(u, "wide8.c") {
            continue;
        }
        lib.file(u);
    }
    lib.file(&shim);
    lib.compile("ed_box3d");

    if msvc_x86 {
        // Upstream compiles the 8-wide SIMD unit with /arch:AVX2 on MSVC; the library picks it at
        // run time only when the CPU has AVX2.
        let mut wide8 = cc::Build::new();
        base(&mut wide8);
        wide8.flag("/arch:AVX2").file(src.join("wide8.c"));
        wide8.compile("ed_box3d_wide8");
    }

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "linux" || target_os == "android" || target_os == "freebsd" {
        println!("cargo:rustc-link-lib=m");
    }
}

fn is_file(p: &Path, name: &str) -> bool {
    p.file_name().is_some_and(|n| n == name)
}
