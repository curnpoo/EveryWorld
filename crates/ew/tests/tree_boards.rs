//! Trees G6 boards and stats (ignored: they read the baked parts from the cache).
//!
//! `EARTHDROP_CACHE=$PWD/.earthdrop-cache cargo test --release -p ew --test tree_boards -- --ignored --nocapture`
//! `TREE_BOARD_OUT=dir` writes `board_stand100_{before,after}.png` there.

use ed_world::foliage::TreeKind;
use ed_world::stand::{STAND_SPECIES, StandTree};
use ed_world::trees::bank::{self, AUX_BARK, BankMesh, unpack_aux};
use ed_world::trees::parts::TreeParts;
use ed_world::trees::textures::{LEAF_BASE, Rgba8Image, leaf_atlas_with};
use glam::{Vec2, Vec3};
use std::path::PathBuf;

fn cache() -> PathBuf {
    std::env::var("EARTHDROP_CACHE").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.earthdrop-cache"))
}

fn parts() -> TreeParts {
    ew::trees::load_tree_parts(&cache()).expect("baked parts (tools/blender/trees/bake_tree_parts.sh)")
}

fn species(name: &str) -> usize {
    STAND_SPECIES.iter().position(|s| s.name == name).unwrap()
}

#[test]
#[ignore]
fn tree_g6_stats() {
    let p = parts();
    println!("images {:?}", p.images.iter().map(|i| (i.cell, i.size)).collect::<Vec<_>>());
    let a = leaf_atlas_with(Some(&p));
    let f = std::fs::File::create(std::env::temp_dir().join("g6_atlas.png")).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), a.width, a.height);
    e.set_color(png::ColorType::Rgba);
    e.write_header().unwrap().write_image_data(&a.data[..(a.width * a.height * 4) as usize]).unwrap();
    for part in &p.parts {
        let (mut lo, mut hi) = ([9f32; 2], [-9f32; 2]);
        for uv in &part.uvs {
            for k in 0..2 {
                lo[k] = lo[k].min(uv[k]);
                hi[k] = hi[k].max(uv[k]);
            }
        }
        println!("part {:<18} tris {:>3} cards {:>2} len {:.2} m uv {lo:?}..{hi:?} cells {:?}", part.name, part.triangles(), part.cards(), part.length, part.cells.first());
    }
    for name in ["Norway spruce", "European larch"] {
        let sp = species(name);
        for age in 0..3u8 {
            for (label, parts) in [("before", None), ("after", Some(&p))] {
                let nv = if parts.is_some() { bank::ASSEMBLED_VARIANTS } else { bank::CONIFER_VARIANTS };
                let (mut t, mut tmax) = ([0usize; 3], [0usize; 3]);
                let t0 = std::time::Instant::now();
                for v in 0..nv {
                    let m = bank::bank_mesh_with(sp, v, age, parts);
                    for k in 0..3 {
                        t[k] += m.triangles(k);
                        tmax[k] = tmax[k].max(m.triangles(k));
                    }
                }
                let m = bank::bank_mesh_with(sp, 0, age, parts);
                let bark_tris = m.lods[0].0.chunks_exact(3).filter(|t| m.words[t[0] as usize][1] & AUX_BARK != 0).count();
                println!("   v0 bark tris {bark_tris} of {}", m.triangles(0));
                let ms = t0.elapsed().as_secs_f64() * 1e3 / nv as f64;
                println!("{name:<15} age {age} {label:<6} mean tris {:?} max {:?} ({ms:.0} ms/mesh)", t.map(|x| x / nv as usize), tmax);
            }
        }
    }
}

/// Atlas sampler (sRGB mips -> linear).
struct Atlas {
    levels: Vec<(usize, usize, Vec<[f32; 4]>)>,
}

impl Atlas {
    fn new(img: &Rgba8Image) -> Atlas {
        let lin = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        let mut levels = Vec::new();
        let (mut w, mut h, mut o) = (img.width as usize, img.height as usize, 0usize);
        for _ in 0..img.mip_levels.max(1) {
            levels.push((w, h, img.data[o..o + w * h * 4].chunks_exact(4).map(|p| [lin(p[0]), lin(p[1]), lin(p[2]), p[3] as f32 / 255.0]).collect()));
            o += w * h * 4;
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        Atlas { levels }
    }
    fn sample(&self, uv: Vec2, lod: f32) -> [f32; 4] {
        let (w, h, px) = &self.levels[(lod.max(0.0) as usize).min(self.levels.len() - 1)];
        let x = ((uv.x * *w as f32) as isize).clamp(0, *w as isize - 1) as usize;
        let y = ((uv.y * *h as f32) as isize).clamp(0, *h as isize - 1) as usize;
        px[y * w + x]
    }
}

struct Canvas {
    w: usize,
    h: usize,
    rgb: Vec<[f32; 3]>,
    z: Vec<f32>,
}

/// Draws `m` (LOD `lod`) into `cv`: orthographic side view (camera towards -Z, pitched down
/// `pitch`), `mpp` metres per pixel, trunk base at pixel `base`; instance yaw, scale, widen,
/// lean and value as in the renderer.
#[allow(clippy::too_many_arguments)]
fn draw(cv: &mut Canvas, atlas: &Atlas, m: &BankMesh, lod: usize, ip: &bank::InstanceParams, mpp: f32, base: Vec2, pitch: f32) {
    let light = Vec3::new(-0.5, 0.75, 0.45).normalize();
    let leaf_base = LEAF_BASE.map(|c| c.powf(2.2));
    let (sy, cy) = ip.yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let lean_axis = Vec3::new(ip.lean_az.cos(), 0.0, ip.lean_az.sin()).cross(Vec3::Y).normalize_or(Vec3::X);
    let lean = glam::Quat::from_axis_angle(lean_axis, ip.lean);
    let inst_value = (ip.user & 255) as f32 / 128.0;
    let xf = |p: [f32; 3]| {
        let p = Vec3::new(p[0] * ip.widen, p[1], p[2] * ip.widen) * ip.scale;
        let p = lean * Vec3::new(cy * p.x + sy * p.z, p.y, -sy * p.x + cy * p.z);
        // Camera: x right, y up after pitching down; depth along the view.
        let y = p.y * cp + p.z * sp;
        let d = -p.z * cp + p.y * sp;
        (Vec2::new(base.x + p.x / mpp, base.y - y / mpp), d)
    };
    let nf = |n: [f32; 3]| {
        let n = lean * Vec3::new(cy * n[0] + sy * n[2], n[1], -sy * n[0] + cy * n[2]);
        n.normalize_or(Vec3::Y)
    };
    let idx = &m.lods[lod].0;
    for t in idx.chunks_exact(3) {
        let v: Vec<(Vec2, f32)> = t.iter().map(|&i| xf(m.positions[i as usize])).collect();
        let uv: Vec<Vec2> = t.iter().map(|&i| {
            let w = m.words[i as usize][0];
            Vec2::new((w & 0xffff) as f32 / 65535.0, (w >> 16) as f32 / 65535.0)
        })
        .collect();
        let (a, b, c) = (v[0].0, v[1].0, v[2].0);
        let area = (b - a).perp_dot(c - a);
        if area.abs() < 1e-6 {
            continue;
        }
        let ta = Vec2::new(atlas.levels[0].0 as f32, atlas.levels[0].1 as f32);
        let uarea = ((uv[1] - uv[0]) * ta).perp_dot((uv[2] - uv[0]) * ta).abs();
        let lod_t = 0.5 * (uarea / area.abs()).max(1.0).log2();
        let lo = a.min(b).min(c).max(Vec2::ZERO);
        let hi = a.max(b).max(c).min(Vec2::new(cv.w as f32 - 1.0, cv.h as f32 - 1.0));
        let bark = m.words[t[0] as usize][1] & AUX_BARK != 0;
        for y in lo.y as usize..=hi.y.max(0.0) as usize {
            for x in lo.x as usize..=hi.x.max(0.0) as usize {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (b - p).perp_dot(c - p) / area;
                let w1 = (c - p).perp_dot(a - p) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let d = v[0].1 * w0 + v[1].1 * w1 + v[2].1 * w2;
                let k = y * cv.w + x;
                if d <= cv.z[k] {
                    continue;
                }
                let n = (nf(m.normals[t[0] as usize]) * w0 + nf(m.normals[t[1] as usize]) * w1 + nf(m.normals[t[2] as usize]) * w2).normalize_or(Vec3::Y);
                let aux = m.words[t[0] as usize][1];
                let (value, _, _, _, _) = unpack_aux(aux);
                let shade = 0.3 + 0.7 * n.dot(light).abs().max(0.0) * if n.dot(light) > 0.0 { 1.0 } else { 0.45 };
                let col = if bark {
                    m.bark_albedo.map(|c| c * value * shade)
                } else {
                    let s = atlas.sample(uv[0] * w0 + uv[1] * w1 + uv[2] * w2, lod_t);
                    if s[3] < 0.5 {
                        continue;
                    }
                    [0, 1, 2].map(|j| (s[j] / leaf_base[j]).clamp(0.2, 2.5) * m.leaf_albedo[j] * value * inst_value * shade)
                };
                cv.z[k] = d;
                cv.rgb[k] = col;
            }
        }
    }
}

fn save(cv: &Canvas, ss: usize, path: &std::path::Path) {
    let (w, h) = (cv.w / ss, cv.h / ss);
    let enc = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0).round() as u8
    };
    let mut out = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let mut a = [0.0f32; 3];
            for j in 0..ss {
                for i in 0..ss {
                    let c = cv.rgb[(y * ss + j) * cv.w + x * ss + i];
                    for k in 0..3 {
                        a[k] += c[k];
                    }
                }
            }
            for k in 0..3 {
                out[(y * w + x) * 3 + k] = enc(a[k] / (ss * ss) as f32 * 2.2);
            }
        }
    }
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&out).unwrap();
}

/// 100 trees (50 spruce, 50 larch, mixed ages), one per cell, the way the stand picks them.
#[test]
#[ignore]
fn tree_g6_stand_board() {
    let out = PathBuf::from(std::env::var("TREE_BOARD_OUT").unwrap_or_else(|_| "/tmp".into()));
    let p = parts();
    let (spruce, larch) = (species("Norway spruce"), species("European larch"));
    let (cols, rows, cw, ch, ss) = (10usize, 10usize, 150usize, 260usize, 2usize);
    let max_h = bank::target_height(spruce, 2).max(bank::target_height(larch, 2)) * 1.25;
    let mpp = max_h / (ch as f32 * 0.94) / ss as f32;
    for (label, parts) in [("before", None), ("after", Some(&p))] {
        let atlas = Atlas::new(&leaf_atlas_with(parts));
        let mut cv = Canvas { w: cols * cw * ss, h: rows * ch * ss, rgb: Vec::new(), z: Vec::new() };
        cv.z = vec![f32::MIN; cv.w * cv.h];
        cv.rgb = (0..cv.w * cv.h).map(|k| {
            let y = (k / cv.w) as f32 / cv.h as f32;
            let yy = (k / cv.w) % (ch * ss);
            if yy as f32 > ch as f32 * ss as f32 * 0.965 { [0.08, 0.07, 0.05] } else { [0.32 - 0.1 * y, 0.42 - 0.1 * y, 0.55 - 0.08 * y] }
        })
        .collect();
        let mut cache = std::collections::BTreeMap::new();
        for i in 0..cols * rows {
            let sp = if i % 2 == 0 { spruce } else { larch };
            let seed = (i as u32).wrapping_mul(2_654_435_761) ^ 0x5EED;
            let age = [2u8, 2, 1, 2, 0, 1, 2, 2, 1, 2][(seed >> 7) as usize % 10];
            let hj = ed_world::hash01(seed as u64, 99);
            let height = bank::target_height(sp, age) * (0.82 + 0.36 * hj);
            let t = StandTree { dir: glam::DVec3::X, fx: 0.0, fy: 0.0, height_m: height, crown_r_m: 3.0, species: sp as u8, kind: TreeKind::Conifer, age_class: age, seed, id: ed_geo::ObjectId(0) };
            let mut ip = bank::instance_params(&t);
            if parts.is_none() {
                ip.variant = seed % bank::CONIFER_VARIANTS;
            }
            let m = cache.entry((sp, ip.variant, ip.age)).or_insert_with(|| bank::bank_mesh_with(sp, ip.variant, ip.age, parts));
            let (cx, cy) = (i % cols, i / cols);
            let base = Vec2::new((cx * cw + cw / 2) as f32 * ss as f32, ((cy + 1) * ch) as f32 * ss as f32 - ch as f32 * ss as f32 * 0.04);
            draw(&mut cv, &atlas, m, 0, &ip, mpp, base, 0.12);
        }
        save(&cv, ss, &out.join(format!("board_stand100_{label}.png")));
        println!("{label}: {} distinct bank meshes", cache.len());
    }
}
