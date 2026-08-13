//! Engine throughput: state clone, legal-move generation, action application,
//! and whole random games.
//!
//! `cargo run --release --example bench_engine`

use carranta_core::action::Action;
use carranta_core::rng::{Rng, Stream};
use carranta_core::state::{Phase, State};
use std::time::Instant;

/// Play one game to the end with uniformly random legal actions.
fn playout(seed: u64, buf: &mut Vec<Action>, cap: usize) -> (usize, bool) {
    let mut s = State::new(4, seed);
    let mut rng = Rng::new(seed ^ 0xD1CE);
    let mut steps = 0;
    while !matches!(s.phase, Phase::GameOver { .. }) && steps < cap {
        s.legal_into(buf);
        if buf.is_empty() {
            break;
        }
        let a = buf[rng.below(Stream::Dice, buf.len() as u32) as usize];
        if s.apply(a).is_err() {
            break;
        }
        steps += 1;
    }
    (steps, matches!(s.phase, Phase::GameOver { .. }))
}

fn main() {
    println!("{:<34} {:>12}", "operation", "cost");
    println!("{}", "-".repeat(50));

    // ---- State clone ----
    let s0 = State::new(4, 1);
    let reps = 20_000_000u64;
    let t = Instant::now();
    let mut acc = 0u64;
    for i in 0..reps {
        let c = std::hint::black_box(s0);
        acc += c.robber as u64 + i % 2;
    }
    std::hint::black_box(acc);
    println!(
        "{:<34} {:>9.2} ns   ({} bytes)",
        "state clone",
        t.elapsed().as_nanos() as f64 / reps as f64,
        core::mem::size_of::<State>()
    );

    // ---- A spread of real positions ----
    //
    // One position is not representative: the legal set swings from two
    // actions in a discard to dozens mid-Action-phase, and so does the cost.
    let mut buf = Vec::new();
    let mut rng = Rng::new(99);
    let positions: Vec<State> = (0..400)
        .map(|seed| {
            let mut s = State::new(4, 1000 + seed);
            // Stop short of the end: a finished game has no legal actions and
            // would drag the average toward zero.
            let steps = 40 + (seed as usize * 13) % 700;
            for _ in 0..steps {
                if matches!(s.phase, Phase::GameOver { .. }) {
                    break;
                }
                s.legal_into(&mut buf);
                if buf.is_empty() {
                    break;
                }
                let a = buf[rng.below(Stream::Dice, buf.len() as u32) as usize];
                if s.apply(a).is_err() {
                    break;
                }
            }
            s
        })
        .filter(|s| !matches!(s.phase, Phase::GameOver { .. }))
        .take(256)
        .collect();
    assert!(!positions.is_empty());
    let mean_legal: f64 = positions
        .iter()
        .map(|s| {
            let mut b = Vec::new();
            s.legal_into(&mut b);
            b.len() as f64
        })
        .sum::<f64>()
        / positions.len() as f64;

    let reps = 8_000u64;
    let t = Instant::now();
    for _ in 0..reps {
        for s in &positions {
            std::hint::black_box(s).legal_into(&mut buf);
            std::hint::black_box(&buf);
        }
    }
    println!(
        "{:<34} {:>9.1} ns   ({mean_legal:.0} actions on average)",
        "legal move generation",
        t.elapsed().as_nanos() as f64 / (reps * positions.len() as u64) as f64
    );

    // Apply is measured on a fresh copy each time so the state cannot drift;
    // the clone cost above is subtracted out.
    let acts: Vec<Action> = positions
        .iter()
        .map(|s| {
            let mut b = Vec::new();
            s.legal_into(&mut b);
            b[0]
        })
        .collect();
    let t = Instant::now();
    let mut sink = 0u64;
    for _ in 0..reps {
        for (s, &a) in positions.iter().zip(&acts) {
            let mut c = std::hint::black_box(*s);
            let _ = c.apply(std::hint::black_box(a));
            sink += c.robber as u64;
        }
    }
    std::hint::black_box(sink);
    println!(
        "{:<34} {:>9.1} ns   (clone included)",
        "apply one action",
        t.elapsed().as_nanos() as f64 / (reps * positions.len() as u64) as f64
    );

    // ---- Whole games ----
    let games = 20_000u64;
    let t = Instant::now();
    let (mut total_steps, mut finished) = (0usize, 0u64);
    for seed in 0..games {
        let (steps, done) = playout(seed, &mut buf, 20_000);
        total_steps += steps;
        finished += done as u64;
    }
    let dt = t.elapsed();
    let per_game = dt.as_nanos() as f64 / games as f64;
    println!(
        "{:<34} {:>9.1} µs   ({:.0} actions/game, {finished}/{games} finished)",
        "full random game",
        per_game / 1000.0,
        total_steps as f64 / games as f64
    );
    println!(
        "{:<34} {:>9.0}      games/s on one core",
        "self-play throughput",
        1e9 / per_game
    );
    println!(
        "{:<34} {:>9.1} ns",
        "per action, whole-game average",
        dt.as_nanos() as f64 / total_steps as f64
    );
}
