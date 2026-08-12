//! Longest road (R-10.3).
//!
//! # What is being computed
//!
//! A route is a **trail**: a walk that never reuses a road, but *may* revisit
//! an intersection. This is forced by our own rule that a closed loop counts
//! as all of its segments — a 6-road ring must score 6, and no walk over 6
//! distinct edges can avoid returning to where it started. So this is the
//! longest trail, not the longest simple path.
//!
//! # Why it is tractable, despite being NP-hard in general
//!
//! Four structural properties collapse the problem, applied in this order:
//!
//! 1. **Blocked intersections split the graph.** An opponent's building lets a
//!    route end there but not pass through, so each blocked intersection
//!    becomes one degree-1 node per incident road. Blocking then lives in the
//!    graph's shape, every later stage stays oblivious to buildings, and a
//!    cycle running through a blocked intersection is correctly broken.
//!
//! 2. **Degree-2 chains contract.** An optimal trail never starts or ends
//!    strictly inside a chain of degree-2 intersections — it could always
//!    extend to the chain's end and pick up more roads. So a trail traverses
//!    each chain whole or not at all, and every chain collapses to a single
//!    weighted edge. This is the decisive optimisation: a 15-road network with
//!    three junctions searches ~4 weighted edges rather than 15.
//!
//! 3. **Trees are diameters.** With no cycle, trail equals simple path, so the
//!    answer is the weighted diameter — two linear sweeps, no search.
//!
//! 4. **Euler's theorem.** A connected component with 0 or 2 odd-degree
//!    vertices has an Eulerian trail, so its answer is exactly its total
//!    weight — again no search.
//!
//! Only a component with both a cycle and four or more odd-degree junctions
//! reaches the search tier, and there it is capped: a component with `2k` odd
//! vertices decomposes into `k` edge-disjoint trails, so at least `k-1` whole
//! chains go unused, giving an admissible bound of `total - (k-1 lightest)`.

use crate::topology::{EDGE_COUNT, EdgeSet, VERTEX_COUNT, VertexSet, edge_endpoints, iter_edges};
use core::cell::RefCell;

/// An intersection splits into at most one node per incident road, so two
/// nodes per road bounds the working graph.
const MAX_NODES: usize = 2 * EDGE_COUNT;
const MAX_CEDGES: usize = EDGE_COUNT;
const NO_NODE: u8 = u8::MAX;

/// Reusable working memory.
///
/// The engine holds one of these and passes it to every call, so recomputing a
/// road length performs **no allocation and no bulk clearing**. Visited marks
/// use a monotonic tick compared against per-slot stamps; at this size zeroing
/// arrays would cost more than the search itself.
pub struct Scratch {
    // Split graph over the player's roads.
    adj_edge: [[u8; 3]; MAX_NODES],
    adj_node: [[u8; 3]; MAX_NODES],
    deg: [u8; MAX_NODES],
    n_nodes: usize,

    node_of_vertex: [u8; VERTEX_COUNT],
    vertex_stamp: [u64; VERTEX_COUNT],

    // Contracted multigraph: chains of degree-2 nodes become weighted edges.
    cu: [u8; MAX_CEDGES],
    cv: [u8; MAX_CEDGES],
    cw: [u8; MAX_CEDGES],
    cadj: [[u8; 3]; MAX_NODES],
    cdeg: [u8; MAX_NODES],
    n_cedges: usize,

    consumed: EdgeSet,
    seen_stamp: [u64; MAX_NODES],
    dist: [u32; MAX_NODES],
    dist_stamp: [u64; MAX_NODES],
    stack: [u8; MAX_NODES],
    comp: [u8; MAX_NODES],
    tick: u64,
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

impl Scratch {
    pub const fn new() -> Self {
        Scratch {
            adj_edge: [[NO_NODE; 3]; MAX_NODES],
            adj_node: [[NO_NODE; 3]; MAX_NODES],
            deg: [0; MAX_NODES],
            n_nodes: 0,
            node_of_vertex: [NO_NODE; VERTEX_COUNT],
            vertex_stamp: [0; VERTEX_COUNT],
            cu: [0; MAX_CEDGES],
            cv: [0; MAX_CEDGES],
            cw: [0; MAX_CEDGES],
            cadj: [[NO_NODE; 3]; MAX_NODES],
            cdeg: [0; MAX_NODES],
            n_cedges: 0,
            consumed: 0,
            seen_stamp: [0; MAX_NODES],
            dist: [0; MAX_NODES],
            dist_stamp: [0; MAX_NODES],
            stack: [0; MAX_NODES],
            comp: [0; MAX_NODES],
            tick: 0,
        }
    }

    #[inline]
    fn new_node(&mut self) -> u8 {
        let id = self.n_nodes as u8;
        self.n_nodes += 1;
        // Only degree needs resetting; adjacency slots below `deg` are always
        // written before they are read.
        self.deg[id as usize] = 0;
        self.cdeg[id as usize] = 0;
        id
    }

    #[inline]
    fn node_for(&mut self, v: u8, blocked: VertexSet, stamp: u64) -> u8 {
        if blocked & (1u64 << v) != 0 {
            return self.new_node();
        }
        if self.vertex_stamp[v as usize] != stamp {
            self.vertex_stamp[v as usize] = stamp;
            let n = self.new_node();
            self.node_of_vertex[v as usize] = n;
            n
        } else {
            self.node_of_vertex[v as usize]
        }
    }

    fn build_split(&mut self, roads: EdgeSet, blocked: VertexSet) {
        self.tick += 1;
        let stamp = self.tick;
        self.n_nodes = 0;
        for e in iter_edges(roads) {
            let [va, vb] = edge_endpoints(e);
            let a = self.node_for(va, blocked, stamp);
            let b = self.node_for(vb, blocked, stamp);
            for (n, other) in [(a, b), (b, a)] {
                let d = self.deg[n as usize] as usize;
                self.adj_edge[n as usize][d] = e;
                self.adj_node[n as usize][d] = other;
                self.deg[n as usize] = d as u8 + 1;
            }
        }
    }

    /// Collapse every maximal chain of degree-2 nodes into one weighted edge.
    ///
    /// Returns the longest pure-cycle component (one with no junction at all).
    /// Those are Eulerian by construction, so they need no further work, but
    /// each is a candidate answer in its own right.
    fn contract(&mut self, roads: EdgeSet) -> u32 {
        self.n_cedges = 0;
        self.consumed = 0;

        for j in 0..self.n_nodes as u8 {
            if self.deg[j as usize] == 2 {
                continue; // not a junction
            }
            for i in 0..self.deg[j as usize] as usize {
                let first = self.adj_edge[j as usize][i];
                if self.consumed & (1u128 << first) != 0 {
                    continue;
                }
                self.consumed |= 1u128 << first;
                let mut weight = 1u8;
                let mut prev_edge = first;
                let mut at = self.adj_node[j as usize][i];
                while self.deg[at as usize] == 2 {
                    let e0 = self.adj_edge[at as usize][0];
                    let slot = if e0 == prev_edge { 1 } else { 0 };
                    let next = self.adj_edge[at as usize][slot];
                    self.consumed |= 1u128 << next;
                    weight += 1;
                    prev_edge = next;
                    at = self.adj_node[at as usize][slot];
                }
                self.add_cedge(j, at, weight);
            }
        }

        // Anything unconsumed is a component with no junction: a bare cycle,
        // a closed Eulerian trail worth all of its roads.
        let mut best_cycle = 0u32;
        let mut left = roads & !self.consumed;
        while left != 0 {
            let start = left.trailing_zeros() as u8;
            let [va, _] = edge_endpoints(start);
            let n0 = self.node_of_vertex[va as usize];
            let mut count = 0u32;
            let mut at = n0;
            let mut prev_edge = NO_NODE;
            loop {
                let e0 = self.adj_edge[at as usize][0];
                let slot = if e0 == prev_edge { 1 } else { 0 };
                let next = self.adj_edge[at as usize][slot];
                left &= !(1u128 << next);
                count += 1;
                prev_edge = next;
                at = self.adj_node[at as usize][slot];
                if at == n0 {
                    break;
                }
            }
            best_cycle = best_cycle.max(count);
        }
        best_cycle
    }

    #[inline]
    fn add_cedge(&mut self, a: u8, b: u8, w: u8) {
        let id = self.n_cedges as u8;
        self.cu[id as usize] = a;
        self.cv[id as usize] = b;
        self.cw[id as usize] = w;
        self.n_cedges += 1;
        for n in [a, b] {
            let d = self.cdeg[n as usize] as usize;
            if d < 3 {
                self.cadj[n as usize][d] = id;
                self.cdeg[n as usize] = d as u8 + 1;
            }
            if a == b {
                break; // a self-loop occupies one adjacency slot, not two
            }
        }
    }

    #[inline]
    fn cother(&self, ce: u8, at: u8) -> u8 {
        if self.cu[ce as usize] == at {
            self.cv[ce as usize]
        } else {
            self.cu[ce as usize]
        }
    }
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = const { RefCell::new(Scratch::new()) };
}

/// Length of the player's longest continuous road.
///
/// Convenience wrapper over a thread-local [`Scratch`]. Hot paths should own a
/// `Scratch` and call [`longest_road_in`].
pub fn longest_road(roads: EdgeSet, blocked: VertexSet) -> u32 {
    SCRATCH.with(|s| longest_road_in(&mut s.borrow_mut(), roads, blocked))
}

/// Length of the player's longest continuous road, using caller-owned memory.
///
/// `roads` is the player's road bitset; `blocked` is the set of intersections
/// carrying an **opponent's** building. A player's own buildings never break
/// their route, so they must not appear in `blocked`.
pub fn longest_road_in(s: &mut Scratch, roads: EdgeSet, blocked: VertexSet) -> u32 {
    let n_roads = roads.count_ones();
    if n_roads <= 1 {
        return n_roads;
    }

    s.build_split(roads, blocked);
    let mut best = s.contract(roads);

    s.tick += 1;
    let seen = s.tick;

    for start in 0..s.n_nodes as u8 {
        if s.cdeg[start as usize] == 0 || s.seen_stamp[start as usize] == seen {
            continue;
        }

        // Flood one contracted component.
        let mut sp = 1usize;
        s.stack[0] = start;
        s.seen_stamp[start as usize] = seen;
        let mut n_comp = 0usize;
        let mut comp_edges: u128 = 0;
        let mut odd = 0u32;

        while sp > 0 {
            sp -= 1;
            let n = s.stack[sp];
            s.comp[n_comp] = n;
            n_comp += 1;
            // Parity comes from the *original* degree; contraction preserves it
            // at junctions, and a self-loop contributes 2 either way.
            odd += (s.deg[n as usize] & 1) as u32;
            for i in 0..s.cdeg[n as usize] as usize {
                let ce = s.cadj[n as usize][i];
                comp_edges |= 1u128 << ce;
                let w = s.cother(ce, n);
                if s.seen_stamp[w as usize] != seen {
                    s.seen_stamp[w as usize] = seen;
                    s.stack[sp] = w;
                    sp += 1;
                }
            }
        }

        let total: u32 = iter_edges(comp_edges)
            .map(|ce| s.cw[ce as usize] as u32)
            .sum();
        if total <= best {
            continue;
        }
        let (ec, vc) = (comp_edges.count_ones(), n_comp as u32);

        let len = if ec == vc - 1 {
            weighted_diameter(s, n_comp)
        } else if odd <= 2 {
            total
        } else {
            let k = odd / 2;
            let mut weights = [0u8; MAX_CEDGES];
            let mut n = 0;
            for ce in iter_edges(comp_edges) {
                weights[n] = s.cw[ce as usize];
                n += 1;
            }
            weights[..n].sort_unstable();
            let shed: u32 = weights[..(k as usize - 1).min(n)]
                .iter()
                .map(|&w| w as u32)
                .sum();
            let bound = total - shed;
            if bound <= best {
                continue;
            }
            search(s, n_comp, comp_edges, total, bound, best)
        };

        best = best.max(len);
    }

    best
}

/// Longest weighted path in a contracted tree: farthest node, then measure.
fn weighted_diameter(s: &mut Scratch, n_comp: usize) -> u32 {
    debug_assert!(n_comp > 0);
    let (far, _) = farthest(s, s.comp[0]);
    let (_, d) = farthest(s, far);
    d
}

fn farthest(s: &mut Scratch, from: u8) -> (u8, u32) {
    s.tick += 1;
    let stamp = s.tick;
    s.dist[from as usize] = 0;
    s.dist_stamp[from as usize] = stamp;
    s.stack[0] = from;
    let mut sp = 1usize;
    let (mut best_node, mut best_dist) = (from, 0u32);

    while sp > 0 {
        sp -= 1;
        let n = s.stack[sp];
        let d = s.dist[n as usize];
        if d > best_dist {
            best_dist = d;
            best_node = n;
        }
        for i in 0..s.cdeg[n as usize] as usize {
            let ce = s.cadj[n as usize][i];
            let w = s.cother(ce, n);
            if s.dist_stamp[w as usize] != stamp {
                s.dist_stamp[w as usize] = stamp;
                s.dist[w as usize] = d + s.cw[ce as usize] as u32;
                s.stack[sp] = w;
                sp += 1;
            }
        }
    }
    (best_node, best_dist)
}

/// Exhaustive weighted-trail search over one contracted component.
fn search(
    s: &Scratch,
    n_comp: usize,
    comp_edges: u128,
    total: u32,
    bound: u32,
    best_so_far: u32,
) -> u32 {
    let mut best = best_so_far;
    for i in 0..n_comp {
        if best >= bound {
            break;
        }
        walk(s, s.comp[i], comp_edges, 0, total, &mut best, bound);
    }
    best
}

fn walk(s: &Scratch, at: u8, unused: u128, len: u32, left: u32, best: &mut u32, bound: u32) {
    if len > *best {
        *best = len;
        if *best >= bound {
            return;
        }
    }
    if len + left <= *best {
        return;
    }
    for i in 0..s.cdeg[at as usize] as usize {
        let ce = s.cadj[at as usize][i];
        let bit = 1u128 << ce;
        if unused & bit == 0 {
            continue;
        }
        let w = s.cw[ce as usize] as u32;
        walk(
            s,
            s.cother(ce, at),
            unused & !bit,
            len + w,
            left - w,
            best,
            bound,
        );
        if *best >= bound {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{
        ALL_EDGES, HEX_EDGES, HEX_VERTICES, edge_other, edges_at, endpoints_of, iter_edges,
        iter_vertices, vertex_bit,
    };

    fn set(edges: &[u8]) -> EdgeSet {
        edges.iter().fold(0, |a, &e| a | 1u128 << e)
    }

    /// Naive reference: exhaustive trail search over the raw board, with no
    /// contraction, no tiers and no bounds. Obviously correct, hopelessly slow
    /// — exactly what a differential test wants.
    fn brute(roads: EdgeSet, blocked: VertexSet) -> u32 {
        fn go(
            roads: EdgeSet,
            blocked: VertexSet,
            v: u8,
            used: EdgeSet,
            len: u32,
            arrived: bool,
        ) -> u32 {
            // A route may end on an opponent's building, never pass through.
            if arrived && blocked & vertex_bit(v) != 0 {
                return len;
            }
            let mut best = len;
            for e in iter_edges(edges_at(v) & roads & !used) {
                let w = edge_other(e, v);
                best = best.max(go(roads, blocked, w, used | 1u128 << e, len + 1, true));
            }
            best
        }
        iter_vertices(endpoints_of(roads))
            .map(|v| go(roads, blocked, v, 0, 0, false))
            .max()
            .unwrap_or(0)
    }

    /// Grow a random connected network of `n` roads, as a player would.
    fn grow(n: usize, seed: &mut u64) -> EdgeSet {
        let mut next = || {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *seed >> 33
        };
        let mut roads: EdgeSet = 1u128 << (next() % EDGE_COUNT as u64);
        while roads.count_ones() < n as u32 {
            let frontier: Vec<u8> = iter_vertices(endpoints_of(roads))
                .flat_map(|v| iter_edges(edges_at(v) & !roads))
                .collect();
            if frontier.is_empty() {
                break;
            }
            roads |= 1u128 << frontier[(next() as usize) % frontier.len()];
        }
        roads
    }

    fn simple_path(start: u8, n: usize, banned: EdgeSet, seen0: VertexSet) -> Option<EdgeSet> {
        fn go(
            at: u8,
            left: usize,
            used: EdgeSet,
            seen: VertexSet,
            banned: EdgeSet,
        ) -> Option<EdgeSet> {
            if left == 0 {
                return Some(used);
            }
            for e in iter_edges(edges_at(at) & !used & !banned) {
                let w = edge_other(e, at);
                if seen & vertex_bit(w) != 0 {
                    continue;
                }
                if let Some(r) = go(w, left - 1, used | 1u128 << e, seen | vertex_bit(w), banned) {
                    return Some(r);
                }
            }
            None
        }
        go(start, n, 0, seen0 | vertex_bit(start), banned)
    }

    fn path(start: u8, n: usize) -> EdgeSet {
        simple_path(start, n, 0, 0).expect("board admits a path this long")
    }

    #[test]
    fn trivial_cases() {
        assert_eq!(longest_road(0, 0), 0);
        assert_eq!(longest_road(set(&[0]), 0), 1);
    }

    #[test]
    fn straight_path_counts_every_road() {
        for n in 1..=15 {
            assert_eq!(longest_road(path(0, n), 0), n as u32, "path of {n}");
        }
    }

    #[test]
    fn ring_around_a_hex_counts_all_six() {
        // Forces trail semantics: a closed loop scores 6, which no simple path
        // over 6 distinct edges could achieve.
        let ring = set(&HEX_EDGES[9]);
        assert_eq!(ring.count_ones(), 6);
        assert_eq!(longest_road(ring, 0), 6);
    }

    #[test]
    fn a_loop_with_a_tail_traverses_both() {
        // Walk the tail in, then all the way round: 9 roads, revisiting the
        // junction once. A longest simple path would score only 8.
        let ring = set(&HEX_EDGES[9]);
        let attach = HEX_VERTICES[9][0];
        let tail = simple_path(attach, 3, ring, endpoints_of(ring) & !vertex_bit(attach)).unwrap();
        let net = ring | tail;
        assert_eq!(net.count_ones(), 9);
        assert_eq!(longest_road(net, 0), 9);
    }

    #[test]
    fn a_fork_does_not_add_length() {
        let p = path(0, 6);
        let mid = iter_edges(p)
            .flat_map(crate::topology::edge_endpoints)
            .find(|&v| (edges_at(v) & p).count_ones() == 2 && edges_at(v) & !p != 0)
            .unwrap();
        let spur = simple_path(mid, 1, p, endpoints_of(p) & !vertex_bit(mid)).unwrap();
        let net = p | spur;
        assert_eq!(net.count_ones(), 7);
        assert_eq!(longest_road(net, 0), 6);
    }

    #[test]
    fn opponent_building_breaks_a_route() {
        let p = path(0, 8);
        assert_eq!(longest_road(p, 0), 8);
        let interior = iter_edges(p)
            .flat_map(crate::topology::edge_endpoints)
            .find(|&v| (edges_at(v) & p).count_ones() == 2)
            .unwrap();
        let split = longest_road(p, vertex_bit(interior));
        assert!(
            (4..8).contains(&split),
            "expected a split route, got {split}"
        );

        let endpoint = iter_edges(p)
            .flat_map(crate::topology::edge_endpoints)
            .find(|&v| (edges_at(v) & p).count_ones() == 1)
            .unwrap();
        assert_eq!(longest_road(p, vertex_bit(endpoint)), 8);
    }

    #[test]
    fn blocking_a_ring_breaks_the_cycle() {
        let ring = set(&HEX_EDGES[9]);
        let v = HEX_VERTICES[9][0];
        assert_eq!(longest_road(ring, vertex_bit(v)), 6);
        let w = HEX_VERTICES[9][3];
        assert_eq!(longest_road(ring, vertex_bit(v) | vertex_bit(w)), 3);
    }

    #[test]
    fn disjoint_networks_take_the_longer() {
        let a = set(&HEX_EDGES[0]);
        let far = set(&HEX_EDGES[18]);
        assert_eq!(endpoints_of(a) & endpoints_of(far), 0);
        let b = simple_path(HEX_VERTICES[18][0], 2, 0, endpoints_of(a)).unwrap();
        assert_eq!(longest_road(a | b, 0), 6);
    }

    #[test]
    fn matches_brute_force_on_random_networks() {
        // The contraction, tier and bound machinery is where subtle wrongness
        // would hide. Check it against an obviously-correct reference.
        let mut seed = 0x243F6A8885A308D3u64;
        for i in 0..4_000 {
            let n = (i % 11) + 2; // up to 12 roads: brute force stays viable
            let roads = grow(n, &mut seed);
            assert_eq!(longest_road(roads, 0), brute(roads, 0), "roads={roads:#x}");
        }
    }

    #[test]
    fn matches_brute_force_with_opponent_buildings() {
        let mut seed = 0x13198A2E03707344u64;
        for i in 0..4_000 {
            let n = (i % 9) + 2;
            let roads = grow(n, &mut seed);
            // Block a random subset of the network's own intersections.
            let touched: Vec<u8> = iter_vertices(endpoints_of(roads)).collect();
            let mut blocked = 0u64;
            for (k, &v) in touched.iter().enumerate() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                if (seed >> 60).is_multiple_of(4) && k.is_multiple_of(2) {
                    blocked |= vertex_bit(v);
                }
            }
            assert_eq!(
                longest_road(roads, blocked),
                brute(roads, blocked),
                "roads={roads:#x} blocked={blocked:#x}"
            );
        }
    }

    #[test]
    fn whole_board_terminates_and_is_bounded() {
        let n = longest_road(ALL_EDGES, 0);
        assert!(n >= 54 && n <= EDGE_COUNT as u32, "got {n}");
    }
}
