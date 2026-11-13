//! Contract test G4: shared borders are bit-identical (decoded), for neighbours and for
//! parent pages, across all six faces and both poles.

mod node_common;
use ed_data::node::{HeightPage, node_heights, parent_heights};
use ed_geo::node::{BodyId, NodeId};
use node_common::*;

/// Border samples as (lattice point bits, decoded height bits).
fn border(id: NodeId, p: &HeightPage) -> Vec<([u64; 3], u32)> {
    let mut v = Vec::new();
    for j in 0..257u32 {
        for i in 0..257u32 {
            if i == 0 || j == 0 || i == 256 || j == 256 {
                let q = id.grid_point(i, j, 256);
                v.push(([q.x.to_bits(), q.y.to_bits(), q.z.to_bits()], (p.height(i as usize, j as usize) as f32).to_bits()));
            }
        }
    }
    v
}

/// Number of shared lattice points, asserting every shared point decodes to identical bits.
fn shared_matches(a: NodeId, pa: &HeightPage, b: NodeId, pb: &HeightPage) -> usize {
    let (ba, bb) = (border(a, pa), border(b, pb));
    let mut n = 0;
    for (ka, ha) in &ba {
        for (kb, hb) in &bb {
            if ka == kb {
                assert_eq!(ha, hb, "shared point differs between {a} and {b}");
                n += 1;
            }
        }
    }
    n
}

fn pole_nodes(rng: &mut Rng) -> Vec<NodeId> {
    let mut v = Vec::new();
    for face in [4u8, 5] {
        for level in 0..=10u8 {
            let n = 1u32 << level;
            // Nodes touching the face centre (the pole) at this level, plus a random one.
            for (x, y) in [(n / 2, n / 2), (n.saturating_sub(1) / 2, n.saturating_sub(1) / 2)] {
                v.push(NodeId::new(BodyId(0), face, level, x, y).unwrap());
            }
            let _ = rng.next();
        }
    }
    v
}

#[test]
fn g4_neighbour_borders_bit_identical() {
    let src = Synth::new();
    let mut rng = Rng(0xE0E0_2024);
    let mut nodes: Vec<NodeId> = pole_nodes(&mut rng);
    // Every level-0 and level-1 node, then random ones up to 1000 in total.
    for face in 0..6u8 {
        nodes.push(NodeId::new(BodyId(0), face, 0, 0, 0).unwrap());
    }
    while nodes.len() < 1000 {
        nodes.push(random_node(&mut rng, 10));
    }
    assert_eq!(nodes.len(), 1000);
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut faces_seen = [false; 6];
    let (mut crossings, mut checked) = (0, 0);
    for id in nodes {
        faces_seen[id.face() as usize] = true;
        let pa = node_heights(&src, id);
        assert_eq!(pa.samples.len(), 257 * 257);
        let (dx, dy) = dirs[rng.below(4) as usize];
        let nb = id.neighbour(dx, dy);
        if nb == id {
            continue; // level 0 against itself on a clamped edge is not a neighbour
        }
        let pb = node_heights(&src, nb);
        let m = shared_matches(id, &pa, nb, &pb);
        assert_eq!(m, 257, "{id} and {nb} must share a full edge");
        checked += 1;
        if nb.face() != id.face() {
            crossings += 1;
        }
    }
    assert!(faces_seen.iter().all(|&f| f));
    assert!(crossings > 20, "few face crossings: {crossings}");
    eprintln!("G4 neighbours: {checked} pairs checked, {crossings} across faces");
}

#[test]
fn g4_parent_borders_bit_identical() {
    let src = Synth::new();
    let mut rng = Rng(0xFACE);
    let build = |id: NodeId| {
        let ch = id.children().unwrap();
        let pages: Vec<HeightPage> = ch.iter().map(|&c| node_heights(&src, c)).collect();
        let parent = parent_heights([&pages[0], &pages[1], &pages[2], &pages[3]]);
        (ch, pages, parent)
    };
    let mut list: Vec<NodeId> = pole_nodes(&mut rng).into_iter().filter(|n| n.level() <= 6).collect();
    while list.len() < 60 {
        let n = random_node(&mut rng, 7);
        list.push(n);
    }
    for id in list {
        let (ch, pages, parent) = build(id);
        // Direct check through the lattice: every parent border point equals the child sample
        // at the same lattice point.
        let mut matched = 0;
        for j in 0..257u32 {
            for i in 0..257u32 {
                if !(i == 0 || j == 0 || i == 256 || j == 256) {
                    continue;
                }
                let p = id.grid_point(i, j, 256);
                for (c, page) in ch.iter().zip(&pages) {
                    let (ox, oy) = ((c.x() & 1) * 256, (c.y() & 1) * 256);
                    // child index of the parent lattice point (2i, 2j) relative to this child
                    let (li, lj) = (2 * i as i64 - ox as i64, 2 * j as i64 - oy as i64);
                    if (0..=256).contains(&li) && (0..=256).contains(&lj) {
                        let cp = c.grid_point(li as u32, lj as u32, 256);
                        assert_eq!(cp, p, "lattice nesting broke for {id}");
                        assert_eq!(page.height(li as usize, lj as usize).to_bits(), parent.height(i as usize, j as usize).to_bits());
                        matched += 1;
                    }
                }
            }
        }
        assert!(matched >= 4 * 256, "{matched}");
        // Neighbouring parents (built from their own children) agree on the shared edge.
        let nb = id.neighbour(1, 0);
        if nb != id {
            let (_, _, pn) = build(nb);
            assert_eq!(shared_matches(id, &parent, nb, &pn), 257);
        }
    }
}

/// Fine levels: pages there are de-terraced over a window of source pixels around the node
/// (`ed_data::node::DEQUANT_FROM_LEVEL`); the window differs per node, the shared edges must not.
#[test]
fn g4_fine_level_borders_bit_identical() {
    let src = Synth::new();
    let mut rng = Rng(0xF1_4E_2026);
    let dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let mut checked = 0;
    while checked < 60 {
        let level = ed_data::node::DEQUANT_FROM_LEVEL + rng.below(6) as u8;
        let n = 1u64 << level;
        let id = NodeId::new(BodyId(0), rng.below(4) as u8, level, rng.below(n) as u32, rng.below(n) as u32).unwrap();
        let (dx, dy) = dirs[rng.below(4) as usize];
        let nb = id.neighbour(dx, dy);
        if nb == id {
            continue;
        }
        let (pa, pb) = (node_heights(&src, id), node_heights(&src, nb));
        assert_eq!(shared_matches(id, &pa, nb, &pb), 257, "{id} and {nb} must share a full edge");
        checked += 1;
    }
}
