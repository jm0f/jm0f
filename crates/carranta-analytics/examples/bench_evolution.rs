//! Can a population-based trainer actually run on one laptop?
//!
//! Two numbers decide it, and neither is guessable:
//!
//! 1. **How self-play scales across cores.** Evaluation is embarrassingly
//!    parallel, so the question is only whether anything shared gets in the
//!    way.
//! 2. **How many games it takes to tell two nearly-equal agents apart.** This
//!    is the one that matters. Carranta has enormous variance, so a genome's
//!    fitness is a noisy estimate, and the games needed to resolve a small
//!    improvement set the cost of a generation. Throughput is meaningless
//!    without it.
//!
//! `cargo run --release -p carranta-analytics --example bench_evolution`

use carranta_bot::{Heuristic, Policy, Weights};
use carranta_core::state::{Phase, State, TradeMode};
use std::time::Instant;

/// Play one game and return each seat's finishing position (1 = winner).
///
/// `weights` is indexed by seat. Positions rather than wins: a win is one bit
/// per game, while the full order is what a fitness signal should read (the
/// same argument as §10.5's design point 1).
fn positions(seed: u64, weights: &[Weights; 4], mode: TradeMode) -> [f64; 4] {
    let mut bots: Vec<Heuristic> = (0..4)
        .map(|s| {
            let mut b = Heuristic::new(seed * 31 + s as u64 + 1);
            b.weights = weights[s];
            b
        })
        .collect();

    let mut state = State::new(4, seed).with_trade_mode(mode);
    let mut buf = Vec::new();
    for _ in 0..20_000 {
        if matches!(state.phase, Phase::GameOver { .. }) {
            break;
        }
        state.legal_into(&mut buf);
        if buf.is_empty() {
            break;
        }
        let seat = state.decider() as usize;
        let a = bots[seat].choose(&state, &buf);
        if state.apply(a).is_err() {
            break;
        }
    }

    let winner = match state.phase {
        Phase::GameOver { winner } => Some(winner),
        _ => None,
    };
    let vp: [u32; 4] = core::array::from_fn(|p| state.victory_points(p));
    core::array::from_fn(|i| {
        if Some(i as u8) == winner {
            return 1.0;
        }
        let ahead = (0..4)
            .filter(|&j| j != i)
            .filter(|&j| Some(j as u8) == winner || vp[j] > vp[i])
            .count();
        ahead as f64 + 1.0
    })
}

/// One paired trial: the same board played with the two variants swapped
/// between seat pairs, so seat effects cancel within the pair.
///
/// Returns the challenger's mean position minus the baseline's. Negative means
/// the challenger finished higher.
fn paired_trial(seed: u64, base: Weights, challenger: Weights, mode: TradeMode) -> f64 {
    // Arrangement A: challenger in seats 0 and 2.
    let a = positions(seed, &[challenger, base, challenger, base], mode);
    // Arrangement B: the mirror, same board.
    let b = positions(seed, &[base, challenger, base, challenger], mode);

    let challenger_mean = (a[0] + a[2] + b[1] + b[3]) / 4.0;
    let baseline_mean = (a[1] + a[3] + b[0] + b[2]) / 4.0;
    challenger_mean - baseline_mean
}

fn mean_sd(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, var.sqrt())
}

/// Games needed to resolve an effect of size `delta` at 95% confidence.
fn games_to_resolve(sd: f64, delta: f64) -> f64 {
    if delta.abs() < 1e-12 {
        return f64::INFINITY;
    }
    (1.96 * sd / delta.abs()).powi(2)
}

fn main() {
    let mode = TradeMode::Disabled; // what training consumes (§6.5)
    println!("population training feasibility — {mode:?} market\n");

    // ---- 1. Does self-play scale across cores? ----
    println!("== parallel scaling ==");
    let per_thread = 400u64;
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let mut baseline = 0.0;
    for threads in [1usize, 2, cores] {
        if threads > cores || (threads == 2 && cores < 2) {
            continue;
        }
        let t = Instant::now();
        std::thread::scope(|s| {
            for w in 0..threads {
                s.spawn(move || {
                    let base = Weights::default();
                    for g in 0..per_thread {
                        let seed = w as u64 * 1_000_000 + g;
                        std::hint::black_box(positions(
                            seed,
                            &[base, base, base, base],
                            TradeMode::Disabled,
                        ));
                    }
                });
            }
        });
        let games = (threads as u64 * per_thread) as f64;
        let rate = games / t.elapsed().as_secs_f64();
        if threads == 1 {
            baseline = rate;
        }
        println!(
            "  {threads} thread(s)   {rate:7.0} games/s   {:.2}x   {:.0}% efficiency",
            rate / baseline,
            rate / baseline / threads as f64 * 100.0
        );
    }

    // ---- 2. How noisy is a fitness estimate? ----
    //
    // Pairing is exact here: identical weights play identical games on the
    // same board, so the null difference is *exactly* zero. Board luck and
    // seat effects are removed by construction rather than averaged away —
    // which is the whole reason to pair. What is left is the only noise that
    // matters: the perturbation sending the game down a different path.
    println!("\n== how much signal does one game carry? ==");
    let base = Weights::default();
    let trials = 600u64;
    assert_eq!(
        paired_trial(0, base, base, mode),
        0.0,
        "pairing should be exact"
    );

    // Variants, from a wholesale change down to a nudge. Weights are integers,
    // so each is checked to be a real change rather than a rounded no-op.
    let variants: Vec<(&str, Weights)> = {
        let mut v = Vec::new();
        let mut w = base;
        w.vp = base.vp / 4;
        v.push(("points weight quartered", w));
        let mut w = base;
        w.pips = base.pips * 3;
        v.push(("production x3", w));
        let mut w = base;
        w.pips = base.pips * 3 / 2;
        v.push(("production +50%", w));
        let mut w = base;
        w.pips = base.pips + 2;
        v.push(("production +2 (+17%)", w));
        let mut w = base;
        w.pips = base.pips + 1;
        v.push(("production +1 (+8%)", w));
        v
    };

    println!("  variant                   effect     sd     paired trials to resolve");
    let mut per_trial = 0.0f64;
    for (name, challenger) in &variants {
        assert_ne!(challenger.pips == base.pips, challenger.vp == base.vp);
        let t = Instant::now();
        let deltas: Vec<f64> = (0..trials)
            .map(|g| paired_trial(g, base, *challenger, mode))
            .collect();
        per_trial = per_trial.max(t.elapsed().as_secs_f64() / trials as f64);
        let (mean, sd) = mean_sd(&deltas);
        let n = games_to_resolve(sd, mean);
        println!(
            "  {name:<24} {mean:+.4}  {sd:.3}   {}",
            if n.is_finite() && n < 1e7 {
                format!("{n:>9.0}")
            } else {
                "      >1e7".to_string()
            }
        );
    }
    println!("  (position, 1 = winner: a positive effect means the change made it worse)");

    // ---- 3. What does that cost? ----
    println!("\n== what a generation costs ==");
    let games_per_trial = 2.0;
    let trial_rate = 1.0 / per_trial;
    println!(
        "  one paired trial    {:.1} ms   ({:.0} trials/s/core, {:.0} games/s/core)",
        per_trial * 1e3,
        trial_rate,
        trial_rate * games_per_trial
    );
    println!("\n  population 150, per genome:");
    println!(
        "     trials each   games/generation   4 cores      8 cores      generations/day (8 cores)"
    );
    for trials_each in [50.0, 200.0, 1_000.0, 5_000.0] {
        let evals = 150.0 * trials_each;
        let core_seconds = evals / trial_rate;
        let h4 = core_seconds / 3600.0 / 4.0;
        let h8 = core_seconds / 3600.0 / 8.0;
        println!(
            "     {trials_each:>11.0}   {:>16.2e}   {h4:>6.2} h    {h8:>6.2} h    {:>8.0}",
            evals * games_per_trial,
            24.0 / h8
        );
    }

    println!(
        "\n  a year of continuous running on 8 cores plays about {:.2e} games",
        trial_rate * 8.0 * games_per_trial * 365.0 * 24.0 * 3600.0
    );
}
