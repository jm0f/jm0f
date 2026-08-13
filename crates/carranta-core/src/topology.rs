//! Static board topology: 19 hexes, 54 intersections, 72 edges.
//!
//! All tables are generated at compile time (see `build.rs`) so every lookup
//! here is an indexed read from a `const` array — no lazy init, no atomics,
//! no bounds surprises. Everything is `#[inline]` and takes plain `u8` ids.

include!(concat!(env!("OUT_DIR"), "/topology_tables.rs"));

/// Bitset over the 72 edges. One `u128` holds the whole board.
pub type EdgeSet = u128;
/// Bitset over the 54 intersections. One `u64` holds the whole board.
pub type VertexSet = u64;

/// Every edge on the board.
pub const ALL_EDGES: EdgeSet = (1u128 << EDGE_COUNT) - 1;
/// Every intersection on the board.
pub const ALL_VERTICES: VertexSet = (1u64 << VERTEX_COUNT) - 1;

#[inline(always)]
pub const fn edge_bit(e: u8) -> EdgeSet {
    1u128 << e
}

#[inline(always)]
pub const fn vertex_bit(v: u8) -> VertexSet {
    1u64 << v
}

/// The two intersections an edge connects.
#[inline(always)]
pub fn edge_endpoints(e: u8) -> [u8; 2] {
    EDGE_ENDPOINTS[e as usize]
}

/// The other end of `e` from `v`.
#[inline(always)]
pub fn edge_other(e: u8, v: u8) -> u8 {
    let [a, b] = EDGE_ENDPOINTS[e as usize];
    if a == v { b } else { a }
}

/// Roads sharing an intersection with `e`, as a bitset (including `e`).
#[inline(always)]
pub fn edge_adj(e: u8) -> EdgeSet {
    EDGE_ADJ_MASK[e as usize]
}

/// The two intersections of `e`, as a bitset.
#[inline(always)]
pub fn edge_endpoint_mask(e: u8) -> VertexSet {
    EDGE_ENDPOINT_MASK[e as usize]
}

/// Edges meeting at an intersection, as a bitset. Degree is 2 or 3.
#[inline(always)]
pub fn edges_at(v: u8) -> EdgeSet {
    VERTEX_EDGE_MASK[v as usize]
}

/// Iterator over set bits of an edge set, lowest first.
#[inline(always)]
pub fn iter_edges(mut set: EdgeSet) -> impl Iterator<Item = u8> {
    core::iter::from_fn(move || {
        if set == 0 {
            None
        } else {
            let e = set.trailing_zeros() as u8;
            set &= set - 1;
            Some(e)
        }
    })
}

/// Iterator over set bits of a vertex set, lowest first.
#[inline(always)]
pub fn iter_vertices(mut set: VertexSet) -> impl Iterator<Item = u8> {
    core::iter::from_fn(move || {
        if set == 0 {
            None
        } else {
            let v = set.trailing_zeros() as u8;
            set &= set - 1;
            Some(v)
        }
    })
}

/// The intersections touched by a set of edges.
#[inline]
pub fn endpoints_of(edges: EdgeSet) -> VertexSet {
    let mut vs = 0u64;
    for e in iter_edges(edges) {
        let [a, b] = EDGE_ENDPOINTS[e as usize];
        vs |= vertex_bit(a) | vertex_bit(b);
    }
    vs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_dimensions() {
        assert_eq!(HEX_COUNT, 19);
        assert_eq!(VERTEX_COUNT, 54);
        assert_eq!(EDGE_COUNT, 72);
    }

    #[test]
    fn degrees_sum_to_twice_the_edges() {
        let total: u32 = (0..VERTEX_COUNT)
            .map(|v| edges_at(v as u8).count_ones())
            .sum();
        assert_eq!(total as usize, 2 * EDGE_COUNT);
    }

    #[test]
    fn every_intersection_has_degree_two_or_three() {
        // The whole longest-road argument leans on max degree 3; assert it.
        for v in 0..VERTEX_COUNT {
            let d = edges_at(v as u8).count_ones();
            assert!((2..=3).contains(&d), "intersection {v} has degree {d}");
        }
    }

    #[test]
    fn adjacency_is_symmetric() {
        for e in 0..EDGE_COUNT as u8 {
            let [a, b] = edge_endpoints(e);
            assert_ne!(a, b);
            assert!(edges_at(a) & edge_bit(e) != 0);
            assert!(edges_at(b) & edge_bit(e) != 0);
            assert_eq!(edge_other(e, a), b);
            assert_eq!(edge_other(e, b), a);
        }
    }

    #[test]
    fn hex_corners_are_distinct_and_connected() {
        for (h, &vs) in HEX_VERTICES.iter().enumerate() {
            let mut sorted = vs;
            let sorted = sorted.as_mut_slice();
            sorted.sort_unstable();
            let distinct = sorted.windows(2).filter(|w| w[0] != w[1]).count() + 1;
            assert_eq!(distinct, 6, "hex {h} must have 6 distinct corners");
            // Consecutive corners around a hex must share an edge.
            for i in 0..6 {
                let (a, b) = (vs[i], vs[(i + 1) % 6]);
                assert!(
                    edges_at(a) & edges_at(b) != 0,
                    "hex {h}: corners {a},{b} not adjacent"
                );
            }
        }
    }

    #[test]
    fn board_graph_is_connected() {
        let mut seen = vertex_bit(0);
        let mut stack = vec![0u8];
        while let Some(v) = stack.pop() {
            for e in iter_edges(edges_at(v)) {
                let w = edge_other(e, v);
                if seen & vertex_bit(w) == 0 {
                    seen |= vertex_bit(w);
                    stack.push(w);
                }
            }
        }
        assert_eq!(seen, ALL_VERTICES);
    }

    #[test]
    fn endpoints_of_all_edges_is_all_vertices() {
        assert_eq!(endpoints_of(ALL_EDGES), ALL_VERTICES);
    }
}
