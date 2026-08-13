//! How many games must join two groups of players before their ratings can be
//! compared?
//!
//! The question behind benchmarking a trained agent against people. Agents
//! play each other constantly and will be tightly rated among themselves;
//! humans play each other and will be tightly rated among themselves. Neither
//! fact says anything about how the two compare — that rests entirely on the
//! games that *cross* between them.
//!
//! This simulates the rating model directly from known skills, so the answer is
//! not confounded by anything about Carranta: outcomes are drawn from the
//! Plackett–Luce model the rating assumes, and the recovered gaps are checked
//! against the truth.
//!
//! `cargo run --release -p carranta-analytics --example bridge`

use carranta_analytics::rating::{Model, Pool};
use carranta_core::rng::{Rng, Stream};

/// Skill scale of the generative model. Larger means one game says less, so
/// this is the knob that decides how many games anything takes.
const BETA: f64 = 25.0 / 6.0;

const AGENTS: [f64; 4] = [32.0, 30.0, 29.0, 28.0];
const HUMANS: [f64; 4] = [27.0, 25.0, 23.0, 20.0];

/// Draw a finishing order for four players from their true skills.
fn sample_order(skills: &[f64; 4], rng: &mut Rng) -> [u32; 4] {
    let mut left: Vec<usize> = (0..4).collect();
    let mut order = Vec::with_capacity(4);
    while !left.is_empty() {
        let weights: Vec<f64> = left.iter().map(|&i| (skills[i] / BETA).exp()).collect();
        let total: f64 = weights.iter().sum();
        let mut u = rng.below(Stream::Dice, 1 << 24) as f64 / (1u32 << 24) as f64 * total;
        let mut pick = left.len() - 1;
        for (idx, w) in weights.iter().enumerate() {
            u -= w;
            if u <= 0.0 {
                pick = idx;
                break;
            }
        }
        order.push(left.remove(pick));
    }
    let mut ranks = [0u32; 4];
    for (place, &p) in order.iter().enumerate() {
        ranks[p] = place as u32 + 1;
    }
    ranks
}

/// Run one scenario: `within` games inside each group, `bridge` games across.
///
/// Returns the mean cross-group gap the ratings claim, and whether every
/// cross-group pair came out in the right order.
fn scenario(within: u32, bridge: u32, seed: u64) -> (f64, bool) {
    let mut pool = Pool::new(Model::default());
    let mut rng = Rng::new(seed);
    // Player 0 is the pinned anchor: an agent of known, fixed strength that
    // both groups meet. Without a pin the two groups' scales float apart.
    pool.pin(0);

    // Agents are ids 0..4, humans 4..8.
    for _ in 0..within {
        let ranks = sample_order(&AGENTS, &mut rng);
        pool.record_ranked(&[0, 1, 2, 3], &ranks);
        let ranks = sample_order(&HUMANS, &mut rng);
        pool.record_ranked(&[4, 5, 6, 7], &ranks);
    }

    // The bridge: mixed tables. Two agents, two humans.
    for b in 0..bridge {
        let a0 = (b % 4) as usize;
        let a1 = ((b / 4) % 4) as usize;
        let h0 = ((b / 2) % 4) as usize;
        let h1 = ((b / 8) % 4) as usize;
        let skills = [AGENTS[a0], HUMANS[h0], AGENTS[a1], HUMANS[h1]];
        let ranks = sample_order(&skills, &mut rng);
        pool.record_ranked(
            &[a0 as u64, 4 + h0 as u64, a1 as u64, 4 + h1 as u64],
            &ranks,
        );
    }

    // Report the claimed gap itself rather than an "error", because μ lives on
    // the rating's own scale and not on the generative one: the update divides
    // by `c = sqrt(Σ(σ²+β²))`, which for four players is about 2β, so a
    // faithful rating lands at roughly *twice* the skill gap that produced it.
    // Inventing an error metric against the wrong scale would just hide that.
    //
    // What is genuinely scale-free — and what actually matters for
    // benchmarking — is whether the ordering across the two groups is right.
    let mut claimed_total = 0.0;
    let mut pairs = 0.0;
    let mut ordering_right = true;
    for (a, &sa) in AGENTS.iter().enumerate() {
        for (h, &sh) in HUMANS.iter().enumerate() {
            let gap = pool.rating(a as u64).mu - pool.rating(4 + h as u64).mu;
            claimed_total += gap;
            pairs += 1.0;
            if (gap > 0.0) != (sa - sh > 0.0) {
                ordering_right = false;
            }
        }
    }
    (claimed_total / pairs, ordering_right)
}

fn main() {
    println!("comparing two groups of players that mostly play among themselves\n");
    println!("  agents  {AGENTS:?}");
    println!("  humans  {HUMANS:?}");
    let true_gap: f64 = AGENTS
        .iter()
        .flat_map(|a| HUMANS.iter().map(move |h| a - h))
        .sum::<f64>()
        / 16.0;
    println!("  (true skills; mean cross-group gap {true_gap:.1})\n");
    println!("  mu is the rating's own scale, not the skill scale: a faithful");
    println!(
        "  rating settles near 2x the generative gap, so ~{:.0} here.\n",
        true_gap * 2.0
    );

    println!("  bridge games   claimed gap (mu)   every cross pair ordered right");
    for bridge in [0u32, 25, 50, 100, 200, 400, 800, 1_600, 3_200] {
        // Averaged over seeds, since one run of a stochastic model is an
        // anecdote.
        let runs = 12;
        let mut total = 0.0;
        let mut right = 0;
        for s in 0..runs {
            let (err, ok) = scenario(2_000, bridge, 700 + s);
            total += err;
            right += ok as u32;
        }
        println!(
            "  {bridge:>12}   {:>16.2}   {right:>2}/{runs} runs",
            total / runs as f64
        );
    }

    println!("\n  within-group games are held at 2000 throughout: playing more");
    println!("  among yourselves does nothing for a cross-group comparison.");

    // The asymmetry that makes this affordable: the agent side can play as
    // much as it likes, so what has to be bought is bridge games, not games.
    println!("\n== what that costs in practice ==");
    for (label, per_week) in [
        ("10 human games a day", 70.0),
        ("50 human games a day", 350.0),
        ("500 human games a day", 3_500.0),
    ] {
        // Assume one seat in four is a rated bot.
        let bridge_per_week = per_week;
        println!(
            "  {label:<22} -> {bridge_per_week:>6.0} bridge games/week, \
             400 reached in {:.1} weeks",
            400.0 / bridge_per_week
        );
    }
}
