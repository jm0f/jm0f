//! Benchmark for the longest-road computation.
//!
//! `cargo run --release --example bench_longest_road`
//!
//! No criterion: the core crate stays dependency-free, and what matters here
//! is the spread across network *shapes*, not confidence intervals on one
//! number. Timing is batched — per-call `Instant::now()` costs tens of ns and
//! its scheduler noise swamps a sub-microsecond measurement.

use carranta_core::longest_road::{Scratch, longest_road_in};
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
}
