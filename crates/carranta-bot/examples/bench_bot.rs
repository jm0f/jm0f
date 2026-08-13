//! Strength and speed of the heuristic policy.
//!
//! `cargo run --release --example bench_bot`

use carranta_bot::{Heuristic, Policy, duel_random, play_game, settle_market};
use carranta_core::state::{Phase, State, TradeMode};
use std::time::Instant;

fn main() {
    println!("heuristic policy\n");

    // ---- Strength ----
    //
    // Paired: every board is played once from each seat, so the per-seat
    // split isolates first-player advantage (A-4) instead of measuring four
    // disjoint sets of boards against each other.
    let boards = 5_000;
    let t = Instant::now();
    let d = duel_random(boards, 4, 20_000, TradeMode::default());
    let dt = t.elapsed();
    println!(
        "versus 3 random opponents, {boards} boards x 4 seats = {} games:",
        d.games
    );
    println!(
        "  win rate            {:.2}%   (random baseline 25%)",
        d.rate() * 100.0
    );
    let pct =
        |seat: usize| 100.0 * d.wins_by_seat[seat] as f64 / d.games_by_seat[seat].max(1) as f64;
    println!(
        "  by seat             {:.2}%  {:.2}%  {:.2}%  {:.2}%   (same boards, all four)",
        pct(0),
        pct(1),
        pct(2),
        pct(3)
    );
    println!("  mean actions/game   {:.0}", d.mean_steps());
    println!(
        "  throughput          {:.0} games/s",
        d.games as f64 / dt.as_secs_f64()
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

    // ---- Open market ----
    //
    // Trading has to happen in self-play or strategies tuned there will not
    // transfer: a game where nobody trades is not the game.
    for mode in [TradeMode::Disabled, TradeMode::Restricted, TradeMode::Full] {
        let games = 400u64;
        let t = Instant::now();
        let (mut steps_total, mut trades_total, mut with_trades) = (0usize, 0u32, 0u32);
        let mut proposals = 0u32;
        for g in 0..games {
            let mut a = Heuristic::new(g);
            let mut b = Heuristic::new(g + 100_000);
            let mut c = Heuristic::new(g + 200_000);
            let mut d = Heuristic::new(g + 300_000);
            let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];

            let mut state = State::new(4, g).with_trade_mode(mode);
            let mut buf = Vec::new();
            let mut trades = 0;
            let mut steps = 0;
            while steps < 20_000 {
                if matches!(state.phase, Phase::GameOver { .. }) {
                    break;
                }
                state.legal_into(&mut buf);
                if buf.is_empty() {
                    break;
                }
                let seat = state.decider() as usize;
                let act = ps[seat].choose(&state, &buf);
                if matches!(act, carranta_core::action::Action::ProposeTrade { .. }) {
                    proposals += 1;
                }
                if state.apply(act).is_err() {
                    break;
                }
                steps += 1;
                trades += settle_market(&mut state, &mut ps);
            }
            steps_total += steps;
            trades_total += trades;
            with_trades += (trades > 0) as u32;
        }
        let dt = t.elapsed();
        println!(
            "\n{mode:?} market, {games} self-play games:\n  \
             actions/game {:.0}   trades/game {:.1}   games with a trade {}%\n  \
             proposals/game {:.1}   asks per trade {:.1}\n  \
             per game {:.0} µs   {:.0} games/s",
            steps_total as f64 / games as f64,
            trades_total as f64 / games as f64,
            with_trades * 100 / games as u32,
            proposals as f64 / games as f64,
            proposals as f64 / trades_total.max(1) as f64,
            dt.as_secs_f64() * 1e6 / games as f64,
            games as f64 / dt.as_secs_f64(),
        );
    }
}
