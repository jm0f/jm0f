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

use crate::topology::{
    EdgeSet, VertexSet, edge_adj, edge_endpoint_mask, edge_endpoints, edge_other, edges_at,
};
use core::cell::RefCell;

/// Working arrays are sized to the full range of a `u8` index.
///
/// A performance decision, not a capacity one: a component yields at most
/// `2 * EDGE_COUNT` contracted nodes. Sizing at 256 lets the compiler prove
/// every `u8 as usize` index is in bounds and drop the check, and those reads
/// are the inner loop of the search tier.
const SLOTS: usize = 256;
const NO_NODE: u8 = u8::MAX;

/// Working memory for the search tier.
///
/// The common tiers need none of this — they run in registers on bitmasks.
/// It exists so the rare component with both a cycle and four odd junctions
/// can build a contracted graph without allocating.
pub struct Scratch {
    // Contracted multigraph: each maximal chain of degree-2 intersections
    // becomes one weighted edge.
    cu: [u8; SLOTS],
    cv: [u8; SLOTS],
    cw: [u8; SLOTS],
    cadj: [[u8; 3]; SLOTS],
    cdeg: [u8; SLOTS],
    n_nodes: usize,
    n_cedges: usize,

    node_of_vertex: [u8; SLOTS],
    vertex_stamp: [u64; SLOTS],

    dist: [u32; SLOTS],
    dist_stamp: [u64; SLOTS],
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
            cu: [0; SLOTS],
            cv: [0; SLOTS],
            cw: [0; SLOTS],
            cadj: [[NO_NODE; 3]; SLOTS],
            cdeg: [0; SLOTS],
            n_nodes: 0,
            n_cedges: 0,
            node_of_vertex: [NO_NODE; SLOTS],
            vertex_stamp: [0; SLOTS],
            dist: [0; SLOTS],
            dist_stamp: [0; SLOTS],
            tick: 0,
        }
    }

    #[inline(always)]
    fn new_node(&mut self) -> u8 {
        let id = self.n_nodes as u8;
        self.n_nodes += 1;
        self.cdeg[id as usize] = 0;
        id
    }

    /// The contracted node for an unblocked junction, created on first use.
    #[inline(always)]
    fn junction_node(&mut self, v: u8, stamp: u64) -> u8 {
        if self.vertex_stamp[v as usize] != stamp {
            self.vertex_stamp[v as usize] = stamp;
            let n = self.new_node();
            self.node_of_vertex[v as usize] = n;
            n
        } else {
            self.node_of_vertex[v as usize]
        }
    }

    /// Contract one component straight from bitmasks into a weighted multigraph.
    ///
    /// No intermediate graph is built: `edges_at(v) & comp` is the adjacency,
    /// and a chain is followed by clearing the edge just used from that mask.
    ///
    /// Blocked intersections need no special handling beyond being treated as
    /// chain terminators. Because each chain is discovered exactly once and
    /// takes a **fresh** node at a blocked end, two chains meeting at one
    /// blocked intersection get distinct nodes — which is precisely the split
    /// that stops a route passing through it.
    fn contract(&mut self, comp: EdgeSet, verts: VertexSet, blocked: VertexSet) -> u128 {
        self.tick += 1;
        let stamp = self.tick;
        self.n_nodes = 0;
        self.n_cedges = 0;

        let mut consumed: EdgeSet = 0;
        let mut cedges: u128 = 0;

        let mut vs = verts;
        while vs != 0 {
            let v = vs.trailing_zeros() as u8;
            vs &= vs - 1;

            let inc = edges_at(v) & comp;
            let v_blocked = blocked & (1u64 << v) != 0;
            if !v_blocked && inc.count_ones() == 2 {
                continue; // interior of a chain, not a junction
            }

            let mut m = inc;
            while m != 0 {
                let first = m.trailing_zeros() as u8;
                m &= m - 1;
                if consumed & (1u128 << first) != 0 {
                    continue; // already swallowed by a chain from the far end
                }
                consumed |= 1u128 << first;

                let a = if v_blocked {
                    self.new_node()
                } else {
                    self.junction_node(v, stamp)
                };

                // Follow the chain until the next junction or blocked end.
                let mut weight = 1u8;
                let mut prev = first;
                let mut at = edge_other(first, v);
                loop {
                    if blocked & (1u64 << at) != 0 {
                        break;
                    }
                    let ainc = edges_at(at) & comp;
                    if ainc.count_ones() != 2 {
                        break;
                    }
                    let next = (ainc & !(1u128 << prev)).trailing_zeros() as u8;
                    consumed |= 1u128 << next;
                    weight += 1;
                    prev = next;
                    at = edge_other(next, at);
                }

                let b = if blocked & (1u64 << at) != 0 {
                    self.new_node()
                } else {
                    self.junction_node(at, stamp)
                };

                let id = self.n_cedges as u8;
                self.cu[id as usize] = a;
                self.cv[id as usize] = b;
                self.cw[id as usize] = weight;
                self.n_cedges += 1;
                cedges |= 1u128 << id;
                for n in [a, b] {
                    let d = self.cdeg[n as usize] as usize;
                    if d < 3 {
                        self.cadj[n as usize][d] = id;
                        self.cdeg[n as usize] = d as u8 + 1;
                    }
                    if a == b {
                        break; // a self-loop takes one adjacency slot, not two
                    }
                }
            }
        }
        cedges
    }

    /// Lower bound on the total chain weight any trail must leave unused.
    ///
    /// Two independent relaxations, both admissible; the stronger wins.
    ///
    /// A component with `2k` odd-degree junctions decomposes into `k`
    /// edge-disjoint trails, so at least `k-1` whole chains go unused — worth
    /// at least the `k-1` lightest.
    ///
    /// That ignores *where* the odd junctions are, which is usually the
    /// binding constraint. Every odd junction but the two a trail may end on
    /// must have an incident chain dropped, and one dropped chain settles at
    /// most two junctions. Charging each junction half its cheapest incident
    /// chain therefore also bounds the removal, and on networks whose light
    /// chains are clustered away from the odd junctions it bounds it far more
    /// tightly.
    fn shed_bound(&self, cedges: u128, k: u32) -> u32 {
        let n_ce = cedges.count_ones() as usize;
        let drop_n = (k as usize).saturating_sub(1).min(n_ce);
        if drop_n == 0 {
            return 0;
        }

        let mut weights = [0u8; crate::topology::EDGE_COUNT];
        let mut cheapest = [u8::MAX; SLOTS];
        let mut n = 0usize;
        let mut rem = cedges;
        while rem != 0 {
            let ce = rem.trailing_zeros() as usize;
            rem &= rem - 1;
            let w = self.cw[ce];
            weights[n] = w;
            n += 1;
            for side in [self.cu[ce], self.cv[ce]] {
                let slot = &mut cheapest[side as usize];
                if w < *slot {
                    *slot = w;
                }
            }
        }
        weights[..n].sort_unstable();
        let by_count: u32 = weights[..drop_n].iter().map(|&w| w as u32).sum();

        // Per-junction charge. `2k` odd junctions, of which two may stay odd,
        // so drop the two most expensive charges.
        let mut charges = [0u8; SLOTS];
        let mut c = 0usize;
        for (node, &cheap) in cheapest.iter().enumerate().take(self.n_nodes) {
            let slots = self.cdeg[node];
            if slots == 0 {
                continue;
            }
            // A self-loop contributes 2 to the degree but occupies one slot.
            let mut deg = 0u32;
            for i in 0..slots as usize {
                let ce = self.cadj[node][i] as usize;
                deg += if self.cu[ce] == self.cv[ce] { 2 } else { 1 };
            }
            if !deg.is_multiple_of(2) {
                charges[c] = cheap;
                c += 1;
            }
        }
        charges[..c].sort_unstable();
        let keep = c.saturating_sub(2);
        let charged: u32 = charges[..keep].iter().map(|&w| w as u32).sum();
        let by_incidence = charged.div_ceil(2);

        by_count.max(by_incidence)
    }

    #[inline(always)]
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

    let mut best = 0u32;
    let mut remaining = roads;

    while remaining != 0 {
        // ---- Flood one component, entirely in bitmasks. ----
        //
        // No graph is built. `edge_adj(e)` is a precomputed mask of the roads
        // sharing an intersection with `e`, so expanding a road is one load
        // and one OR. Degree parity is accumulated in the same pass by XORing
        // each road's two-intersection mask: bit `v` of `parity` ends up as
        // `degree(v) & 1`, which is the entire input to the Euler test.
        let seed = remaining.trailing_zeros() as u8;
        let mut comp: EdgeSet = 1u128 << seed;
        let mut frontier: EdgeSet = comp;
        let mut parity: VertexSet = 0;
        let mut verts: VertexSet = 0;

        while frontier != 0 {
            let mut next: EdgeSet = 0;
            let mut f = frontier;
            while f != 0 {
                let e = f.trailing_zeros() as u8;
                f &= f - 1;
                let em = edge_endpoint_mask(e);
                parity ^= em;
                verts |= em;
                if blocked & em == 0 {
                    next |= edge_adj(e);
                } else {
                    // A route may end on an opponent's building but not pass
                    // through, so the component only grows via the free end.
                    let [a, b] = edge_endpoints(e);
                    if blocked & (1u64 << a) == 0 {
                        next |= edges_at(a);
                    }
                    if blocked & (1u64 << b) == 0 {
                        next |= edges_at(b);
                    }
                }
            }
            next &= roads & !comp;
            comp |= next;
            frontier = next;
        }
        remaining &= !comp;

        let free_verts = verts & !blocked;
        let mut odd = (parity & !blocked).count_ones();
        let mut n_verts = free_verts.count_ones();

        // Each blocked intersection contributes one degree-1 node per road,
        // and a degree-1 node is odd.
        let mut bs = verts & blocked;
        let mut blocked_multi = false;
        while bs != 0 {
            let v = bs.trailing_zeros() as u8;
            bs &= bs - 1;
            let d = (edges_at(v) & comp).count_ones();
            n_verts += d;
            odd += d;
            // Two roads at a blocked intersection means a cycle runs through
            // it that splitting breaks. Parity above stays right, but
            // intersection-space traversal would still see the cycle, so the
            // diameter shortcut is off the table.
            blocked_multi |= d >= 2;
        }

        let e = comp.count_ones();
        if e <= best {
            continue;
        }

        // Tier order puts the cheapest test first. Euler costs one parity
        // counter the flood already gathered, and it answers every straight
        // chain and every bare loop — which is nearly all real road networks —
        // without any traversal at all.
        let len = if odd <= 2 {
            // An Eulerian trail exists and uses every road.
            e
        } else if e == n_verts - 1 && !blocked_multi {
            // A tree, and every blocked intersection in it is a leaf, so
            // intersection space and split space agree: the answer is the
            // diameter.
            let root = free_verts.trailing_zeros() as u8;
            let (far, _) = farthest(s, comp, blocked, root);
            let (_, d) = farthest(s, comp, blocked, far);
            d
        } else {
            // Either a genuine cycle with four or more odd junctions, or a
            // component split apart at a blocked intersection. The general
            // path handles both; contraction earns its pass here and nowhere
            // else, and on a tree it collapses to a handful of chains.
            let cedges = s.contract(comp, verts, blocked);

            let bound = e - s.shed_bound(cedges, odd / 2);
            if bound <= best {
                continue;
            }
            search(s, cedges, e, bound, best)
        };

        best = best.max(len);
    }

    best
}

/// Farthest intersection from `from`, in roads, within one tree component.
///
/// Correct for trees only, which is all it is used for: a tree has exactly one
/// route between any two intersections, so a vertex's distance is fixed the
/// moment it is discovered and the traversal order does not matter.
fn farthest(s: &mut Scratch, comp_edges: EdgeSet, blocked: VertexSet, from: u8) -> (u8, u32) {
    s.tick += 1;
    let stamp = s.tick;
    s.dist[from as usize] = 0;
    s.dist_stamp[from as usize] = stamp;
    let mut pending: VertexSet = 1u64 << from;
    let (mut best_node, mut best_dist) = (from, 0u32);

    while pending != 0 {
        let v = pending.trailing_zeros() as u8;
        pending &= pending - 1;
        let d = s.dist[v as usize];
        if d > best_dist {
            best_dist = d;
            best_node = v;
        }
        // A route may not pass *through* an opponent's building, but it may
        // start at one — the second sweep legitimately begins at a blocked
        // leaf, and refusing to expand it there would report a length of 0.
        if v != from && blocked & (1u64 << v) != 0 {
            continue;
        }
        let mut m = edges_at(v) & comp_edges;
        while m != 0 {
            let e = m.trailing_zeros() as u8;
            m &= m - 1;
            let w = edge_other(e, v);
            if s.dist_stamp[w as usize] != stamp {
                s.dist_stamp[w as usize] = stamp;
                s.dist[w as usize] = d + 1;
                pending |= 1u64 << w;
            }
        }
    }
    (best_node, best_dist)
}

/// Exhaustive weighted-trail search over one contracted component.
fn search(s: &Scratch, cedges: u128, total: u32, bound: u32, best_so_far: u32) -> u32 {
    let mut best = best_so_far;
    for n in 0..s.n_nodes as u8 {
        if best >= bound {
            break;
        }
        if s.cdeg[n as usize] == 0 {
            continue;
        }
        walk(s, n, cedges, 0, total, &mut best, bound);
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

/// Per-player road lengths, recomputed only when they can have changed.
///
/// The single largest saving available here is not making the computation
/// faster but skipping it. A road length can only change for a player whose
/// own roads changed, or whose network an opponent has just built across, and
/// both are cheap to detect with a mask test. Over a game that turns roughly
/// 200 recomputations into 60.
///
/// The tracker owns one [`Scratch`], so steady-state operation allocates
/// nothing.
pub struct Tracker {
    roads: [EdgeSet; MAX_PLAYERS],
    buildings: [VertexSet; MAX_PLAYERS],
    cached: [u32; MAX_PLAYERS],
    dirty: u8,
    scratch: Scratch,
}

/// Seats supported by [`Tracker`]. The rules allow 3–4 players (R-1).
pub const MAX_PLAYERS: usize = 4;

impl Default for Tracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Tracker {
    pub const fn new() -> Self {
        Tracker {
            roads: [0; MAX_PLAYERS],
            buildings: [0; MAX_PLAYERS],
            cached: [0; MAX_PLAYERS],
            dirty: 0,
            scratch: Scratch::new(),
        }
    }

    /// Record a road built by `player` on `edge`.
    #[inline]
    pub fn add_road(&mut self, player: usize, edge: u8) {
        self.roads[player] |= 1u128 << edge;
        self.dirty |= 1 << player;
    }

    /// Record a settlement or city built by `player` at `vertex`.
    ///
    /// This can only shorten an **opponent's** route, and only one whose roads
    /// actually meet that intersection — so at most a couple of seats are
    /// invalidated, usually none.
    #[inline]
    pub fn add_building(&mut self, player: usize, vertex: u8) {
        self.buildings[player] |= 1u64 << vertex;
        let touching = edges_at(vertex);
        for q in 0..MAX_PLAYERS {
            if q != player && self.roads[q] & touching != 0 {
                self.dirty |= 1 << q;
            }
        }
    }

    /// Longest road for `player`, recomputing only if something relevant moved.
    #[inline]
    pub fn get(&mut self, player: usize) -> u32 {
        if self.dirty & (1 << player) != 0 {
            self.recompute(player);
        }
        self.cached[player]
    }

    /// Longest road for every seat.
    pub fn all(&mut self) -> [u32; MAX_PLAYERS] {
        let mut out = [0; MAX_PLAYERS];
        for (p, slot) in out.iter_mut().enumerate() {
            *slot = self.get(p);
        }
        out
    }

    /// The seat holding the Longest Road tile, if any (R-10.1, R-10.6).
    ///
    /// Requires 5 or more roads, and strictly more than every other seat — a
    /// tie leaves the tile where it was, which is the caller's business.
    pub fn leader(&mut self) -> Option<usize> {
        let lens = self.all();
        let best = *lens.iter().max()?;
        if best < 5 {
            return None;
        }
        let mut winner = None;
        for (p, &l) in lens.iter().enumerate() {
            if l == best {
                if winner.is_some() {
                    return None; // tied, so nobody takes it
                }
                winner = Some(p);
            }
        }
        winner
    }

    fn recompute(&mut self, player: usize) {
        let mut blocked = 0u64;
        for (q, &b) in self.buildings.iter().enumerate() {
            if q != player {
                blocked |= b;
            }
        }
        self.cached[player] = longest_road_in(&mut self.scratch, self.roads[player], blocked);
        self.dirty &= !(1 << player);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::topology::{
        ALL_EDGES, EDGE_COUNT, HEX_EDGES, HEX_VERTICES, VERTEX_COUNT, edges_at, endpoints_of,
        iter_edges, iter_vertices, vertex_bit,
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
        // Up to the full 15-road pool. The search tier and its bound only
        // engage on the larger, loopier networks, so stopping short of the
        // piece limit would leave them untested.
        let mut seed = 0x243F6A8885A308D3u64;
        for i in 0..20_000 {
            let n = (i % 14) + 2;
            let roads = grow(n, &mut seed);
            assert_eq!(longest_road(roads, 0), brute(roads, 0), "roads={roads:#x}");
        }
    }

    #[test]
    fn matches_brute_force_with_opponent_buildings() {
        // Blocking is where the shortcuts are most fragile: it splits the
        // graph, and a cycle through a blocked intersection turns into a tree
        // that intersection-space traversal still sees as cyclic. Block
        // densely and often so those shapes actually come up.
        let mut seed = 0x13198A2E03707344u64;
        for i in 0..20_000 {
            let n = (i % 14) + 2;
            let roads = grow(n, &mut seed);
            let touched: Vec<u8> = iter_vertices(endpoints_of(roads)).collect();
            let mut blocked = 0u64;
            for &v in touched.iter() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                if (seed >> 60).is_multiple_of(3) {
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
    fn tracker_matches_recomputing_from_scratch() {
        // Play out many random games' worth of building and check the cached
        // answer against a full recomputation after every single move.
        let mut seed = 0x2545F4914F6CDD1Du64;
        let next = |s: &mut u64| {
            *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            *s >> 33
        };
        for _ in 0..300 {
            let mut t = Tracker::new();
            let mut roads = [0u128; MAX_PLAYERS];
            let mut builds = [0u64; MAX_PLAYERS];
            for _ in 0..40 {
                let p = (next(&mut seed) as usize) % MAX_PLAYERS;
                if next(&mut seed).is_multiple_of(3) {
                    let v = (next(&mut seed) % VERTEX_COUNT as u64) as u8;
                    t.add_building(p, v);
                    builds[p] |= vertex_bit(v);
                } else {
                    let e = (next(&mut seed) % EDGE_COUNT as u64) as u8;
                    t.add_road(p, e);
                    roads[p] |= 1u128 << e;
                }
                for (q, &r) in roads.iter().enumerate() {
                    let blocked = (0..MAX_PLAYERS)
                        .filter(|&x| x != q)
                        .fold(0u64, |a, x| a | builds[x]);
                    assert_eq!(t.get(q), longest_road(r, blocked), "seat {q} diverged");
                }
            }
        }
    }

    #[test]
    fn tracker_leader_needs_five_and_a_strict_lead() {
        let mut t = Tracker::new();
        for e in iter_edges(path(0, 4)) {
            t.add_road(0, e);
        }
        assert_eq!(t.leader(), None, "four roads is not enough");

        let mut t = Tracker::new();
        let p0 = path(0, 5);
        for e in iter_edges(p0) {
            t.add_road(0, e);
        }
        assert_eq!(t.leader(), Some(0));

        // A second seat matching that length leaves the tile unclaimed.
        let p1 = simple_path(40, 5, 0, endpoints_of(p0)).unwrap();
        for e in iter_edges(p1) {
            t.add_road(1, e);
        }
        assert_eq!(t.leader(), None, "a tie transfers nothing");
    }

    #[test]
    fn whole_board_terminates_and_is_bounded() {
        let n = longest_road(ALL_EDGES, 0);
        assert!(n >= 54 && n <= EDGE_COUNT as u32, "got {n}");
    }
}
