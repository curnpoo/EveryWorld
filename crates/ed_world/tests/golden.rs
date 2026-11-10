//! Determinism suite (Phase 0, W3). `cargo xtask golden`, or `cargo xtask golden --bless`.
//!
//! Builds fixed tiles from recorded source data with the network refused, and hashes each
//! subsystem's output. Hashes are compared (a) between two builds in this process and
//! (b) against `tests/golden/hashes.txt`, which is shared by macOS and Windows CI.
//!
//! Positions are quantised to 1 mm before hashing so that float noise is separated from real
//! divergence. A subsystem listed in `tests/golden/quarantine.txt` is reported, not failed.
//!
//! Recording fixtures (needs network): `EW_GOLDEN_RECORD=1 cargo test -p ed_world --test golden --release`.

use ed_data::cache::DiskCache;
use ed_geo::tile::TileId;
use ed_geo::{EnuFrame, Geodetic};
use ed_world::mesh::MeshData;
use ed_world::tilegen::{self, TileBuild, TileJob};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The six canonical places (one per world type) and two synthetic inputs that need no data.
const PLACES: &[(&str, f64, f64, u8)] = &[
    ("city_shibuya", 35.6595, 139.7005, 14),
    ("alpine_zermatt", 46.0207, 7.7491, 14),
    ("desert_wadi_rum", 29.5765, 35.4200, 14),
    ("forest_schwarzwald", 48.5300, 8.2200, 14),
    ("coast_amalfi", 40.6340, 14.6020, 14),
    ("farmland_beauce", 48.2500, 1.6500, 14),
    // No fixture data exists for these: every source misses and the generator's fallbacks run.
    ("synthetic_ocean", 0.0, -30.0, 14),
    ("synthetic_coarse", 10.0, -140.0, 9),
];

struct H(u64);
impl H {
    fn new() -> Self {
        H(0xcbf29ce484222325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 = (self.0 ^ x as u64).wrapping_mul(0x100000001b3);
        }
    }
    fn u(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    /// Quantise: `q` units per 1.0 (1000 = millimetres for positions in metres).
    fn f(&mut self, v: f32, q: f32) {
        self.bytes(&((v * q).round() as i64).to_le_bytes());
    }
    fn fs<const N: usize>(&mut self, v: &[[f32; N]], q: f32) {
        self.u(v.len() as u64);
        for a in v {
            for &x in a {
                self.f(x, q);
            }
        }
    }
    fn mesh(&mut self, m: &MeshData) {
        self.fs(&m.positions, 1000.0);
        self.fs(&m.normals, 100.0);
        self.fs(&m.uvs, 10_000.0);
        self.fs(&m.colors, 255.0);
        self.u(m.indices.len() as u64);
        for &i in &m.indices {
            self.u(i as u64);
        }
    }
    fn debug(&mut self, v: &impl std::fmt::Debug) {
        self.bytes(format!("{v:?}").as_bytes());
    }
}

fn subsystems(b: &TileBuild) -> BTreeMap<&'static str, u64> {
    let mut out = BTreeMap::new();
    let mut put = |k, f: &dyn Fn(&mut H)| {
        let mut h = H::new();
        f(&mut h);
        out.insert(k, h.0);
    };
    put("terrain", &|h| {
        h.fs(&b.terrain.positions, 1000.0);
        h.u(b.terrain.indices.len() as u64);
        h.f(b.min_max_height.0, 1000.0);
        h.f(b.min_max_height.1, 1000.0);
    });
    put("surface", &|h| {
        for s in &b.terrain.splat {
            h.bytes(s);
        }
        for s in &b.terrain.sat {
            h.bytes(s);
        }
        if let Some((n, px)) = &b.sat_tex {
            h.u(*n as u64);
            h.bytes(px);
        }
        if let Some((n, px)) = &b.detail_tex {
            h.u(*n as u64);
            h.bytes(px);
        }
        h.f(b.sea_fraction, 10_000.0);
    });
    put("water", &|h| h.mesh(&b.water));
    put("roads", &|h| {
        h.mesh(&b.roads);
        if let Some((v, t)) = &b.road_collider {
            h.u(v.len() as u64);
            h.u(t.len() as u64);
        }
        if let Some(n) = &b.nav {
            h.debug(n);
        }
    });
    fn mats(h: &mut H, m: &ed_world::buildings::BuildingMeshes) {
        let mut mats: Vec<_> = m.out.meshes.iter().map(|(k, v)| (format!("{k:?}"), v)).collect();
        mats.sort_by(|a, b| a.0.cmp(&b.0));
        for (k, v) in mats {
            h.bytes(k.as_bytes());
            h.mesh(v);
        }
    }
    put("buildings_far", &|h| h.mesh(&b.building_far));
    put("buildings_lod", &|h| {
        let mut lod: Vec<_> = b.building_lod.iter().collect();
        lod.sort_by_key(|(c, _)| *c);
        for (c, m) in lod {
            h.u(c.0 as u64 * 256 + c.1 as u64);
            mats(h, m);
        }
    });
    put("buildings_detail", &|h| {
        if let Some(d) = &b.building_detail {
            let mut chunks: Vec<_> = d.chunks.keys().copied().collect();
            chunks.sort();
            for c in chunks {
                h.u(c.0 as u64 * 256 + c.1 as u64);
                mats(h, &d.build_chunk(c, false));
            }
        }
    });
    put("foliage", &|h| {
        h.fs(&b.trees.impostors.positions, 1000.0);
        h.fs(&b.trees.impostors.params, 1000.0);
        h.u(b.trees.chunks.len() as u64);
        for c in &b.trees.chunks {
            h.u(c.instances.len() as u64);
            h.debug(&c.instances);
        }
    });
    put("props", &|h| {
        h.mesh(&b.street_props);
        h.mesh(&b.rocks);
        h.u(b.placements.len() as u64);
        for p in &b.placements {
            h.debug(&p.asset);
            h.fs(&[p.pos.to_array()], 1000.0);
            h.f(p.scale, 1000.0);
        }
    });
    out
}

fn dir(sub: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join(sub)
}

fn build(name: &str, lat: f64, lon: f64, z: u8, cache: &DiskCache, tpl: &Option<String>) -> TileBuild {
    let job = TileJob {
        tile: TileId::containing(lat, lon, z),
        frame: EnuFrame::new(Geodetic::new(lat, lon, 0.0)),
        cache: cache.clone(),
        vector_template: if name.starts_with("synthetic") { None } else { tpl.clone() },
        far: false,
    };
    if std::env::var_os("EW_GOLDEN_RECORD").is_some() && !name.starts_with("synthetic") {
        tilegen::prefetch(&job);
    }
    tilegen::build(&job)
}

#[test]
fn golden() {
    let record = std::env::var_os("EW_GOLDEN_RECORD").is_some();
    let bless = record || std::env::var_os("EW_GOLDEN_BLESS").is_some();
    // Slow (about a minute in release). Runs only through `cargo xtask golden`.
    if !bless && std::env::var_os("EW_GOLDEN").is_none() {
        eprintln!("golden: skipped (run `cargo xtask golden`)");
        return;
    }
    let fixtures = dir("fixtures");
    let cache = DiskCache::new(fixtures.join("cache"));
    let tpl_path = fixtures.join("vector_template.txt");
    let tpl = if record {
        let t = ed_data::vector::tile_template(&cache).expect("tilejson");
        std::fs::write(&tpl_path, &t).unwrap();
        Some(t)
    } else {
        ed_data::http::set_offline(true);
        Some(std::fs::read_to_string(&tpl_path).expect("fixtures missing: record them first").trim().to_string())
    };

    let mut lines = Vec::new();
    let mut unstable = Vec::new();
    for &(name, lat, lon, z) in PLACES {
        let t0 = std::time::Instant::now();
        let a = subsystems(&build(name, lat, lon, z, &cache, &tpl));
        let b = subsystems(&build(name, lat, lon, z, &cache, &tpl));
        for (k, v) in &a {
            if b[k] != *v {
                unstable.push(format!("{name}/{k}"));
            }
            lines.push(format!("{name}/{k} {v:016x}"));
        }
        eprintln!("golden: {name} built twice in {:?}", t0.elapsed());
    }
    let got = lines.join("\n") + "\n";
    let golden_path = dir("golden").join("hashes.txt");
    let quarantine: Vec<String> = std::fs::read_to_string(dir("golden").join("quarantine.txt"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split('#').next().map(str::trim).filter(|l| !l.is_empty()).map(String::from))
        .collect();
    let quarantined = |k: &str| quarantine.iter().any(|q| k == q || k.ends_with(&format!("/{q}")));

    if bless {
        std::fs::write(&golden_path, &got).unwrap();
        eprintln!("golden: wrote {}", golden_path.display());
    }
    let want = std::fs::read_to_string(&golden_path).expect("no golden file: run `cargo xtask golden --bless`");
    let want: BTreeMap<&str, &str> = want.lines().filter_map(|l| l.split_once(' ')).collect();
    let mut failed = Vec::new();
    for l in &lines {
        let (k, v) = l.split_once(' ').unwrap();
        let same_run = !unstable.iter().any(|u| u == k);
        let same_golden = want.get(k) == Some(&v);
        if same_run && same_golden {
            continue;
        }
        let why = if !same_run { "differs between two builds in one process" } else { "differs from the golden file" };
        if quarantined(k) {
            eprintln!("golden: QUARANTINED {k}: {why}");
        } else {
            failed.push(format!("{k}: {why} (got {v}, want {})", want.get(k).unwrap_or(&"none")));
        }
    }
    if !record {
        let n = ed_data::http::REQUESTS.load(std::sync::atomic::Ordering::Relaxed);
        eprintln!("golden: {n} network request(s) attempted and refused");
    }
    assert!(failed.is_empty(), "determinism failures:\n{}", failed.join("\n"));
}
