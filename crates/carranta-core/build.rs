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

/// Hot-path tables are padded to the full `u8` range; see `emit_2`.
const SLOTS: usize = 256;

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

    // Tables read on the hot path are padded to the full u8 range so that
    // `table[i as usize]` with `i: u8` is provably in bounds and the compiler
    // drops the check. Those reads are the inner loop of `longest_road`.
    emit_2(&mut s, "EDGE_ENDPOINTS", 2, &edge_endpoints, SLOTS);
    emit_2(&mut s, "VERTEX_EDGES", 3, &vertex_edges, verts.len());
    emit_2(
        &mut s,
        "VERTEX_NEIGHBORS",
        3,
        &vertex_neighbors,
        verts.len(),
    );
    emit_2(&mut s, "VERTEX_HEXES", 3, &vertex_hexes, verts.len());
    emit_2(&mut s, "HEX_VERTICES", 6, &hex_vertices, hexes.len());
    emit_2(&mut s, "HEX_EDGES", 6, &hex_edge_ids, hexes.len());

    // Edge-space adjacency, for the component flood: for each road, the mask
    // of roads sharing an intersection with it, and the mask of its two
    // intersections. Together these let the flood run one load per edge and
    // accumulate degree parity at the same time.
    let mut edge_adj_mask = vec![0u128; edges.len()];
    let mut edge_endpoint_mask = vec![0u64; edges.len()];
    for ei in 0..edges.len() {
        let [a, b] = edge_endpoints[ei];
        edge_endpoint_mask[ei] = (1u64 << a) | (1u64 << b);
        edge_adj_mask[ei] = vertex_edge_mask[a as usize] | vertex_edge_mask[b as usize];
    }
    edge_adj_mask.resize(SLOTS, 0);
    edge_endpoint_mask.resize(SLOTS, 0);
    writeln!(
        s,
        "pub const EDGE_ADJ_MASK: [u128; {SLOTS}] = {edge_adj_mask:?};"
    )
    .unwrap();
    writeln!(
        s,
        "pub const EDGE_ENDPOINT_MASK: [u64; {SLOTS}] = {edge_endpoint_mask:?};"
    )
    .unwrap();

    // Each intersection's neighbours as a mask. The Distance Rule bans every
    // intersection within one edge of a building, so the forbidden set is a
    // union of these rather than a walk over edges.
    let mut vertex_neighbor_mask = vec![0u64; verts.len()];
    for (vi, nb) in vertex_neighbors.iter().enumerate() {
        for &w in nb {
            if w != NONE {
                vertex_neighbor_mask[vi] |= 1u64 << w;
            }
        }
    }
    vertex_neighbor_mask.resize(SLOTS, 0);
    writeln!(
        s,
        "pub const VERTEX_NEIGHBOR_MASK: [u64; {SLOTS}] = {vertex_neighbor_mask:?};"
    )
    .unwrap();

    // Each hex's six corners as an intersection mask, so production for a
    // player on a hex is two popcounts.
    let mut hex_vertex_mask = vec![0u64; hexes.len()];
    for (hi, hv) in hex_vertices.iter().enumerate() {
        for &v in hv {
            hex_vertex_mask[hi] |= 1u64 << v;
        }
    }
    writeln!(
        s,
        "pub const HEX_VERTEX_MASK: [u64; {}] = {hex_vertex_mask:?};",
        hexes.len()
    )
    .unwrap();

    vertex_edge_mask.resize(SLOTS, 0);
    writeln!(
        s,
        "pub const VERTEX_EDGE_MASK: [u128; {SLOTS}] = {vertex_edge_mask:?};"
    )
    .unwrap();

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("topology_tables.rs");
    fs::write(out, s).unwrap();
}

fn emit_2<const N: usize>(s: &mut String, name: &str, n: usize, data: &[[u8; N]], pad_to: usize) {
    debug_assert_eq!(n, N);
    assert!(pad_to >= data.len());
    write!(s, "pub const {name}: [[u8; {N}]; {pad_to}] = [").unwrap();
    for row in data {
        write!(s, "{row:?},").unwrap();
    }
    for _ in data.len()..pad_to {
        write!(s, "[u8::MAX; {N}],").unwrap();
    }
    writeln!(s, "];").unwrap();
}
