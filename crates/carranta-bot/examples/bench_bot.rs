//! Strength and speed of the heuristic policy.
//!
//! `cargo run --release --example bench_bot`

use carranta_bot::{Heuristic, Policy, RandomPolicy, play_game};
use carranta_core::state::{Phase, State};
use std::time::Instant;

/// Win rate against random opponents, with the bot's seat rotated so that
/// first-player advantage cannot inflate the result (A-4).
fn versus_random(games: u32, seats: u8) -> (f64, f64, [u32; 4]) {
    let mut wins = 0u32;
    let mut steps_total = 0usize;
    let mut wins_by_seat = [0u32; 4];
    let mut games_by_seat = [0u32; 4];

    for g in 0..games {
        let bot_seat = (g % seats as u32) as usize;
        let mut bot = Heuristic::new(g as u64);
        let mut randoms: Vec<RandomPolicy> = (0..seats)
            .map(|i| RandomPolicy::new(g as u64 * 31 + i as u64))
            .collect();

        let (lo, hi) = randoms.split_at_mut(bot_seat);
        let mut policies: Vec<&mut dyn Policy> = Vec::new();
        for r in lo.iter_mut() {
            policies.push(r);
        }
        policies.push(&mut bot);
        for r in hi.iter_mut().skip(1) {
            policies.push(r);
        }

        let (winner, steps) = play_game(g as u64, &mut policies, 20_000);
        steps_total += steps;
        games_by_seat[bot_seat] += 1;
        if winner == Some(bot_seat as u8) {
            wins += 1;
            wins_by_seat[bot_seat] += 1;
        }
    }

    let by_seat = core::array::from_fn(|i| {
        if games_by_seat[i] == 0 {
            0
        } else {
            wins_by_seat[i] * 100 / games_by_seat[i]
        }
    });
    (
        wins as f64 / games as f64,
        steps_total as f64 / games as f64,
        by_seat,
    )
}

fn main() {
    println!("heuristic policy\n");

    // ---- Strength ----
    let games = 20_000;
    let t = Instant::now();
    let (rate, mean_steps, by_seat) = versus_random(games, 4);
    let dt = t.elapsed();
    println!("versus 3 random opponents, {games} games, seat rotated:");
    println!(
        "  win rate            {:.2}%   (random baseline 25%)",
        rate * 100.0
    );
    println!(
        "  by seat             {}%  {}%  {}%  {}%",
        by_seat[0], by_seat[1], by_seat[2], by_seat[3]
    );
    println!("  mean actions/game   {mean_steps:.0}");
    println!(
        "  throughput          {:.0} games/s",
        games as f64 / dt.as_secs_f64()
    );

    // ---- All four seats playing well ----
    let games = 5_000;
    let t = Instant::now();
    let mut steps_total = 0usize;
    let mut finished = 0;
    for g in 0..games {
        let mut a = Heuristic::new(g);
        let mut b = Heuristic::new(g + 100_000);
        let mut c = Heuristic::new(g + 200_000);
        let mut d = Heuristic::new(g + 300_000);
        let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];
        let (w, steps) = play_game(g, &mut ps, 20_000);
        steps_total += steps;
        finished += w.is_some() as u32;
    }
    let dt = t.elapsed();
    let per_game = dt.as_secs_f64() / games as f64;
    println!("\nfour heuristics, {games} games:");
    println!(
        "  mean actions/game   {:.0}   ({finished}/{games} finished)",
        steps_total as f64 / games as f64
    );
    println!("  per game            {:.0} µs", per_game * 1e6);
    println!("  throughput          {:.0} games/s", 1.0 / per_game);

    // ---- Decision cost ----
    let mut bot = Heuristic::new(1);
    let mut positions = Vec::new();
    let mut buf = Vec::new();
    for seed in 0..200 {
        let mut s = State::new(4, 5000 + seed);
        for _ in 0..(30 + seed as usize % 200) {
            if matches!(s.phase, Phase::GameOver { .. }) {
                break;
            }
            s.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let a = bot.choose(&s, &buf);
            if s.apply(a).is_err() {
                break;
            }
        }
        if !matches!(s.phase, Phase::GameOver { .. }) {
            positions.push(s);
        }
    }
    let mean_legal: f64 = positions
        .iter()
        .map(|s| {
            let mut b = Vec::new();
            s.legal_into(&mut b);
            b.len() as f64
        })
        .sum::<f64>()
        / positions.len() as f64;

    let reps = 2_000;
    let t = Instant::now();
    let mut sink = 0u64;
    for _ in 0..reps {
        for s in &positions {
            s.legal_into(&mut buf);
            sink += matches!(bot.choose(s, &buf), carranta_core::action::Action::EndTurn) as u64;
        }
    }
    std::hint::black_box(sink);
    println!(
        "\ndecision cost         {:.0} ns   ({mean_legal:.0} candidates on average)",
        t.elapsed().as_nanos() as f64 / (reps * positions.len()) as f64
    );
}
