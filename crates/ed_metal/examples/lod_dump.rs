//! Runs the native renderer's mesh preparation on a dumped tile and writes the LOD levels as
//! OBJ files, so the decimation can be looked at instead of guessed at.
//!
//! `cargo run --release -p ed_metal --example lod_dump -- tile_lod.bin out_dir`
//!
//! Input format (little endian): "EWB1"/"EWL1", u32 vertex count, u32 index count, then
//! positions (3 f32), normals (3 f32), colours (4 f32) per vertex, then indices (u32).

use ed_metal::world::prep;
use std::io::Write;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = args.get(1).cloned().unwrap_or_else(|| "tile_lod.bin".into());
    let out_dir = std::path::PathBuf::from(args.get(2).cloned().unwrap_or_else(|| "lod_out".into()));
    std::fs::create_dir_all(&out_dir).expect("out dir");

    let raw = std::fs::read(&input).expect("read input");
    let mut o = 4usize; // magic (4) then vertex count, index count
    let rd = |o: &mut usize| -> u32 {
        let v = u32::from_le_bytes(raw[*o..*o + 4].try_into().unwrap());
        *o += 4;
        v
    };
    let nv = rd(&mut o) as usize;
    let ni = rd(&mut o) as usize;
    o = 12; // data starts after the header
    let mut pos = Vec::with_capacity(nv);
    let mut nrm = Vec::with_capacity(nv);
    let mut col = Vec::with_capacity(nv);
    for _ in 0..nv {
        let mut p = [0f32; 3];
        for k in 0..3 {
            p[k] = f32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
            o += 4;
        }
        pos.push(p);
    }
    for _ in 0..nv {
        let mut n = [0f32; 3];
        for k in 0..3 {
            n[k] = f32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
            o += 4;
        }
        nrm.push(n);
    }
    for _ in 0..nv {
        let mut c = [0f32; 4];
        for k in 0..4 {
            c[k] = f32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
            o += 4;
        }
        col.push(c);
    }
    let mut idx = Vec::with_capacity(ni);
    for _ in 0..ni {
        idx.push(rd(&mut o));
    }
    println!("input: {nv} verts, {ni} indices ({} tris)", ni / 3);

    let p = prep(&pos, &nrm, &vec![[0.0; 2]; nv], &col, &idx);
    println!("clusters: {}, proxy indices: {}", p.clusters.len(), p.pidx.len());

    write_obj(&out_dir.join("full.obj"), &pos, &idx, None);

    // Longest edges in the ORIGINAL geometry: if the source mesh already has huge triangles
    // (a landmark roof, a ground slab), the LOD cannot be blamed for them.
    {
        let mut worst: Vec<(f32, [f32; 3], [f32; 3])> = Vec::new();
        for t in idx.chunks_exact(3) {
            let (a, b) = (pos[t[0] as usize], pos[t[1] as usize]);
            let e = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
            worst.push((e, a, b));
        }
        worst.sort_by(|x, y| y.0.total_cmp(&x.0));
        println!("original: longest edges");
        for (e, a, b) in worst.iter().take(5) {
            println!("  {e:.1} m  {a:?} -> {b:?}");
        }
    }

    for lvl in 0..3 {
        let mut vidx: Vec<u32> = Vec::new();
        let mut tris = 0usize;
        let mut max_edge = 0f32;
        for c in &p.clusters {
            let (f, n) = c.proxy[lvl];
            if n == 0 {
                continue;
            }
            for t in p.pidx[f..f + n].chunks_exact(3) {
                vidx.extend_from_slice(t);
                tris += 1;
                let (a, b, c2) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
                let e = |p: [f32; 3], q: [f32; 3]| ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
                max_edge = max_edge.max(e(a, b)).max(e(b, c2)).max(e(a, c2));
            }
        }
        println!("level {lvl}: {tris} tris ({:.1}% of full), longest edge {max_edge:.2} m", 100.0 * tris as f32 / (ni / 3) as f32);
        write_obj(&out_dir.join(format!("proxy{lvl}.obj")), &pos, &vidx, Some(ed_metal::world::LOD_ERRORS[lvl]));
    }
}

fn write_obj(path: &std::path::Path, pos: &[[f32; 3]], idx: &[u32], error: Option<f32>) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).expect("create obj"));
    if let Some(e) = error {
        writeln!(f, "# lod error {e} m").unwrap();
    }
    for p in pos {
        writeln!(f, "v {} {} {}", p[0], p[1], p[2]).unwrap();
    }
    for t in idx.chunks_exact(3) {
        writeln!(f, "f {} {} {}", t[0] + 1, t[1] + 1, t[2] + 1).unwrap();
    }
    println!("wrote {}", path.display());
}
