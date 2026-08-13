//! Benchmark for the longest-road computation.
//!
//! `cargo run --release --example bench_longest_road`
//!
//! No criterion: the core crate stays dependency-free, and what matters here
//! is the spread across network *shapes*, not confidence intervals on one
//! number. Timing is batched — per-call `Instant::now()` costs tens of ns and
//! its scheduler noise swamps a sub-microsecond measurement.

use carranta_core::longest_road::{
    MAX_PLAYERS, Scratch, Tracker, longest_road_exceeds, longest_road_in,
};
use carranta_core::topology::*;
use std::time::Instant;

/// Grow a connected road network of `n` roads.
///
/// `dense` picks uniformly from the frontier, which closes loops readily.
/// Otherwise growth prefers extending an existing road end, which is what
/// actual play produces — and the difference turns out to matter enormously.
fn grow(n: usize, seed: &mut u64, dense: bool) -> EdgeSet {
    let mut next = || {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        *seed >> 33
    };
    let mut roads: EdgeSet = 1u128 << (next() % EDGE_COUNT as u64);
    while roads.count_ones() < n as u32 {
        let mut frontier: Vec<u8> = iter_vertices(endpoints_of(roads))
            .flat_map(|v| iter_edges(edges_at(v) & !roads))
            .collect();
        if !dense {
            let ends: Vec<u8> = frontier
                .iter()
                .copied()
                .filter(|&e| {
                    let [a, b] = edge_endpoints(e);
                    (edges_at(a) & roads).count_ones() + (edges_at(b) & roads).count_ones() <= 1
                })
                .collect();
            if !ends.is_empty() {
                frontier = ends;
            }
        }
        if frontier.is_empty() {
            break;
        }
        roads |= 1u128 << frontier[(next() as usize) % frontier.len()];
    }
    roads
}

fn batch(label: &str, nets: &[EdgeSet], blocked: VertexSet) {
    let mut s = Scratch::new();
    for &r in nets {
        std::hint::black_box(longest_road_in(&mut s, r, blocked));
    }
    let reps = (2_000_000 / nets.len().max(1)).max(1);
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..reps {
        for &r in nets {
            acc += longest_road_in(&mut s, std::hint::black_box(r), blocked) as u64;
        }
    }
    let dt = t.elapsed();
    std::hint::black_box(acc);
    let per = dt.as_nanos() as f64 / (reps * nets.len()) as f64;
    let mean_len = nets
        .iter()
        .map(|&r| longest_road_in(&mut s, r, blocked) as f64)
        .sum::<f64>()
        / nets.len() as f64;
    println!("{label:<34} {per:>8.1} ns   (mean result {mean_len:.1})");
}

fn main() {
    let mut seed = 0xC0FFEE_u64;
    let mk = |n, dense, seed: &mut u64| (0..512).map(|_| grow(n, seed, dense)).collect::<Vec<_>>();

    println!("{:<34} {:>11}", "case", "per call");
    println!("{}", "-".repeat(62));

    for n in [5usize, 10, 15] {
        let nets = mk(n, false, &mut seed);
        batch(&format!("realistic, {n} roads"), &nets, 0);
    }
    for n in [10usize, 15] {
        let nets = mk(n, true, &mut seed);
        batch(&format!("dense/adversarial, {n} roads"), &nets, 0);
    }

    // With opponent buildings scattered over the network.
    let nets = mk(15, false, &mut seed);
    let blocked = iter_vertices(endpoints_of(nets[0]))
        .step_by(3)
        .fold(0u64, |a, v| a | vertex_bit(v));
    batch("realistic, 15 roads, blocked", &nets, blocked);

    // A single four-player sweep after one road is built: the number the
    // engine actually pays per turn.
    let four: Vec<EdgeSet> = (0..4).map(|_| grow(15, &mut seed, false)).collect();
    let mut s = Scratch::new();
    let reps = 500_000;
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..reps {
        for &r in &four {
            acc += longest_road_in(&mut s, std::hint::black_box(r), 0) as u64;
        }
    }
    std::hint::black_box(acc);
    println!(
        "\nfour-player sweep: {:.1} ns",
        t.elapsed().as_nanos() as f64 / reps as f64
    );

    // ---- Whole-game cost: the number that actually matters. ----
    //
    // A game is roughly 60 roads and 20 buildings across four seats, with
    // every seat's length needed after each move (the Longest Road tile can
    // change hands at any point). Compare recomputing all four every time
    // against the tracker, which recomputes only what can have changed.
    let moves: Vec<(usize, bool, u8)> = {
        let mut v = Vec::new();
        let mut sd = 0xA5A5_u64;
        let nx = |s: &mut u64| {
            *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            *s >> 33
        };
        for i in 0..80 {
            let p = (nx(&mut sd) as usize) % MAX_PLAYERS;
            let building = i % 4 == 3;
            let id = if building {
                (nx(&mut sd) % 54) as u8
            } else {
                (nx(&mut sd) % 72) as u8
            };
            v.push((p, building, id));
        }
        v
    };

    let reps = 20_000;
    let t = Instant::now();
    let mut acc = 0u64;
    for _ in 0..reps {
        let mut roads = [0u128; MAX_PLAYERS];
        let mut builds = [0u64; MAX_PLAYERS];
        let mut s = Scratch::new();
        for &(p, building, id) in &moves {
            if building {
                builds[p] |= 1u64 << id;
            } else {
                roads[p] |= 1u128 << id;
            }
            for (q, &r) in roads.iter().enumerate() {
                let blocked = (0..MAX_PLAYERS)
                    .filter(|&x| x != q)
                    .fold(0u64, |a, x| a | builds[x]);
                acc += longest_road_in(&mut s, r, blocked) as u64;
            }
        }
    }
    let naive = t.elapsed().as_nanos() as f64 / reps as f64;

    let t = Instant::now();
    let mut acc2 = 0u64;
    for _ in 0..reps {
        let mut tr = Tracker::new();
        for &(p, building, id) in &moves {
            if building {
                tr.add_building(p, id);
            } else {
                tr.add_road(p, id);
            }
            for q in 0..MAX_PLAYERS {
                acc2 += tr.get(q) as u64;
            }
        }
    }
    let tracked = t.elapsed().as_nanos() as f64 / reps as f64;
    assert_eq!(acc, acc2, "tracker must agree with full recomputation");

    println!(
        "\nwhole game ({} moves, all seats queried after each):",
        moves.len()
    );
    println!("  recompute every time  {naive:>9.0} ns");
    println!(
        "  incremental tracker   {tracked:>9.0} ns   ({:.1}x faster)",
        naive / tracked
    );

    // ---- Does a floor pay? ----
    //
    // `longest_road_exceeds` starts the search at a floor instead of zero, so
    // it can stop as soon as a network clears the bar rather than proving its
    // exact length. Measured against the exact call at three floors: none, the
    // network's own length (the worst case — it must still prove it), and a
    // floor no network can reach (the best case — every one is dismissed).
    let nets = mk(15, false, &mut seed);
    let exact: Vec<u32> = {
        let mut s = Scratch::new();
        nets.iter()
            .map(|&r| longest_road_in(&mut s, r, 0))
            .collect()
    };

    let mut sc = Scratch::new();
    let reps = 4_000;
    let mut row = |label: &str, floor: Box<dyn Fn(usize) -> u32>| {
        let t = Instant::now();
        let mut acc = 0u64;
        for _ in 0..reps {
            for (i, &r) in nets.iter().enumerate() {
                acc += longest_road_exceeds(&mut sc, std::hint::black_box(r), 0, floor(i))
                    .unwrap_or(0) as u64;
            }
        }
        std::hint::black_box(acc);
        let per = t.elapsed().as_nanos() as f64 / (reps * nets.len()) as f64;
        println!("  {label:<38} {per:>7.1} ns");
    };

    println!("\nfloored queries, realistic 15-road networks:");
    row("floor 0 (equivalent to exact)", Box::new(|_| 0));
    let ex = exact.clone();
    row("floor = the network's own length", Box::new(move |i| ex[i]));
    row("floor 99 (nothing can clear it)", Box::new(|_| 99));

    // ---- Whole game, tile holder only. ----
    //
    // `Tracker::leader` uses exact lengths. The floored version was built and
    // measured slower in every scenario tried — early game 0.6x, cold position
    // 1.0x, whole game 0.8x — because the tracker's caching had already removed
    // the redundancy the floor was meant to prune. Kept here as the shape of
    // the query a rollout actually issues.
    let t = Instant::now();
    let mut acc3 = 0u64;
    for _ in 0..20_000 {
        let mut tr = Tracker::new();
        for &(p, building, id) in &moves {
            if building {
                tr.add_building(p, id);
            } else {
                tr.add_road(p, id);
            }
            acc3 += tr.leader().map_or(0, |l| l as u64 + 1);
        }
    }
    std::hint::black_box(acc3);
    println!(
        "\nwhole game, tile holder only: {:.0} ns",
        t.elapsed().as_nanos() as f64 / 20_000.0
    );
}
