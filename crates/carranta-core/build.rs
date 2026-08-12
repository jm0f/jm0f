//! Generates the board topology as `const` tables at compile time.
//!
//! The board is 19 hexes in a radius-2 hexagonal arrangement. Rather than
//! hand-transcribing 54 intersections and 72 edges (error-prone, and wrong in
//! a way that unit tests struggle to catch), we derive them from the lattice:
//!
//! - every **intersection** is the unique meeting point of exactly 3 lattice
//!   hexes, so a sorted triple of hex coordinates is its canonical identity;
//! - every **edge** is shared by exactly 2 lattice hexes, so a sorted pair is
//!   its canonical identity.
//!
//! Some of those lattice hexes lie off the board (they are sea). That is the
//! point: it is what gives coastal intersections and coastal roads without
//! any special-casing.
//!
//! Emitting `const` arrays rather than building at runtime keeps every lookup
//! a plain indexed read with no atomic, no lazy init, and no indirection.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// Axial neighbour directions, in cyclic order around a hex.
const DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

/// Board radius: 2 gives the classic 19-hex island.
const RADIUS: i32 = 2;

type Hex = (i32, i32);

fn add(a: Hex, d: (i32, i32)) -> Hex {
    (a.0 + d.0, a.1 + d.1)
}

/// Hexes within `RADIUS` of the origin, in a stable order.
fn board_hexes() -> Vec<Hex> {
    let mut v = Vec::new();
    for q in -RADIUS..=RADIUS {
        for r in -RADIUS..=RADIUS {
            let s = -q - r;
            if q.abs().max(r.abs()).max(s.abs()) <= RADIUS {
                v.push((q, r));
            }
        }
    }
    v.sort();
    v
}

/// The 6 corners of a hex, each as a canonical sorted triple.
fn corners(h: Hex) -> [[Hex; 3]; 6] {
    let mut out = [[(0, 0); 3]; 6];
    for i in 0..6 {
        let a = add(h, DIRS[i]);
        let b = add(h, DIRS[(i + 1) % 6]);
        let mut t = [h, a, b];
        t.sort();
        out[i] = t;
    }
    out
}

/// The 6 edges of a hex, each as a canonical sorted pair.
fn hex_edges(h: Hex) -> [[Hex; 2]; 6] {
    let mut out = [[(0, 0); 2]; 6];
    for i in 0..6 {
        let mut p = [h, add(h, DIRS[i])];
        p.sort();
        out[i] = p;
    }
    out
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let hexes = board_hexes();
    assert_eq!(hexes.len(), 19, "radius-2 board must have 19 hexes");
    let hex_id: BTreeMap<Hex, usize> = hexes.iter().copied().zip(0..).collect();

    // Canonical, sorted identities give stable IDs across builds.
    let mut vert_set: BTreeSet<[Hex; 3]> = BTreeSet::new();
    let mut edge_set: BTreeSet<[Hex; 2]> = BTreeSet::new();
    for &h in &hexes {
        vert_set.extend(corners(h));
        edge_set.extend(hex_edges(h));
    }
    let verts: Vec<[Hex; 3]> = vert_set.into_iter().collect();
    let edges: Vec<[Hex; 2]> = edge_set.into_iter().collect();

    assert_eq!(verts.len(), 54, "expected 54 intersections");
    assert_eq!(edges.len(), 72, "expected 72 edges");
    // Euler's formula for a connected planar graph, counting the outer face.
    assert_eq!(
        verts.len() + hexes.len() + 1,
        edges.len() + 2,
        "Euler check"
    );

    let vert_id: BTreeMap<[Hex; 3], usize> = verts.iter().copied().zip(0..).collect();
    let edge_id: BTreeMap<[Hex; 2], usize> = edges.iter().copied().zip(0..).collect();

    // edge -> its two endpoints: the intersections whose triple contains both
    // of the edge's hexes.
    let mut edge_endpoints = vec![[0u8; 2]; edges.len()];
    for (ei, e) in edges.iter().enumerate() {
        let mut found = Vec::new();
        for (vi, v) in verts.iter().enumerate() {
            if e.iter().all(|h| v.contains(h)) {
                found.push(vi as u8);
            }
        }
        assert_eq!(found.len(), 2, "edge {ei:?} must have exactly 2 endpoints");
        edge_endpoints[ei] = [found[0], found[1]];
    }

    // vertex -> incident edges / neighbouring vertices, padded with NONE.
    const NONE: u8 = u8::MAX;
    let mut vertex_edges = vec![[NONE; 3]; verts.len()];
    let mut vertex_neighbors = vec![[NONE; 3]; verts.len()];
    let mut vertex_hexes = vec![[NONE; 3]; verts.len()];
    let mut vertex_edge_mask = vec![0u128; verts.len()];

    for (vi, v) in verts.iter().enumerate() {
        let mut slot = 0;
        // The three hex pairs drawn from this intersection's triple are its
        // three candidate edges; a pair of two off-board hexes is open sea and
        // simply is not in the edge set.
        for i in 0..3 {
            for j in (i + 1)..3 {
                let mut p = [v[i], v[j]];
                p.sort();
                if let Some(&ei) = edge_id.get(&p) {
                    vertex_edges[vi][slot] = ei as u8;
                    vertex_edge_mask[vi] |= 1u128 << ei;
                    let [a, b] = edge_endpoints[ei];
                    vertex_neighbors[vi][slot] = if a as usize == vi { b } else { a };
                    slot += 1;
                }
            }
        }
        assert!(
            (2..=3).contains(&slot),
            "intersection {vi} has degree {slot}, expected 2 or 3"
        );

        let mut hslot = 0;
        for h in v.iter() {
            if let Some(&hi) = hex_id.get(h) {
                vertex_hexes[vi][hslot] = hi as u8;
                hslot += 1;
            }
        }
        assert!((1..=3).contains(&hslot), "intersection touches 1..=3 hexes");
    }

    // hex -> its 6 corners and 6 edges.
    let mut hex_vertices = vec![[0u8; 6]; hexes.len()];
    let mut hex_edge_ids = vec![[0u8; 6]; hexes.len()];
    for (hi, &h) in hexes.iter().enumerate() {
        for (i, c) in corners(h).iter().enumerate() {
            hex_vertices[hi][i] = vert_id[c] as u8;
        }
        for (i, e) in hex_edges(h).iter().enumerate() {
            hex_edge_ids[hi][i] = edge_id[e] as u8;
        }
    }

    let mut s = String::new();
    writeln!(s, "// @generated by build.rs — do not edit.").unwrap();
    writeln!(s, "pub const HEX_COUNT: usize = {};", hexes.len()).unwrap();
    writeln!(s, "pub const VERTEX_COUNT: usize = {};", verts.len()).unwrap();
    writeln!(s, "pub const EDGE_COUNT: usize = {};", edges.len()).unwrap();
    writeln!(s, "pub const NONE: u8 = u8::MAX;").unwrap();

    emit_2(&mut s, "EDGE_ENDPOINTS", "u8", 2, &edge_endpoints);
    emit_2(&mut s, "VERTEX_EDGES", "u8", 3, &vertex_edges);
    emit_2(&mut s, "VERTEX_NEIGHBORS", "u8", 3, &vertex_neighbors);
    emit_2(&mut s, "VERTEX_HEXES", "u8", 3, &vertex_hexes);
    emit_2(&mut s, "HEX_VERTICES", "u8", 6, &hex_vertices);
    emit_2(&mut s, "HEX_EDGES", "u8", 6, &hex_edge_ids);

    writeln!(
        s,
        "pub const VERTEX_EDGE_MASK: [u128; {}] = {:?};",
        verts.len(),
        vertex_edge_mask
    )
    .unwrap();

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("topology_tables.rs");
    fs::write(out, s).unwrap();
}

fn emit_2<const N: usize>(s: &mut String, name: &str, ty: &str, n: usize, data: &[[u8; N]]) {
    debug_assert_eq!(n, N);
    write!(s, "pub const {name}: [[{ty}; {N}]; {}] = [", data.len()).unwrap();
    for row in data {
        write!(s, "{row:?},").unwrap();
    }
    writeln!(s, "];").unwrap();
}
