//! How sharp is the public record (E-33)?
//!
//! Plays full games and measures, at every decision, how far the table's
//! shared belief about each hand sits from the truth: mean absolute error in
//! cards, per seat and resource. The yardstick is what the old observation
//! implied, a hand of known size and unknown composition, spread evenly over
//! the five resources. The gap between the two is the information the record
//! recovers; the strength it buys is a separate question for a network
//! trained to read it.

use carranta_bot::policy_net::DeepNetPolicy;
use carranta_bot::{Heuristic, Policy};
use carranta_core::state::{Phase, State, TradeMode};

fn main() {
    let mut args = std::env::args().skip(1);
    let mut games = 200u32;
    let mut champion = String::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--games" => games = args.next().and_then(|v| v.parse().ok()).unwrap_or(200),
            "--champion" => champion = args.next().unwrap_or_default(),
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
    }
    let net = if champion.is_empty() {
        None
    } else {
        let text = std::fs::read_to_string(&champion).unwrap_or_else(|e| {
            eprintln!("cannot read {champion}: {e}");
            std::process::exit(1);
        });
        let Some((net, _)) = carranta_bot::net::Net::parse(&text) else {
            eprintln!("{champion} is not a champion network file");
            std::process::exit(1);
        };
        Some(net)
    };

    let (mut tracked, mut uniform, mut samples) = (0.0f64, 0.0f64, 0u64);
    let (mut late_tracked, mut late_uniform, mut late_samples) = (0.0f64, 0.0f64, 0u64);
    let mut buf = Vec::new();
    for seed in 0..games as u64 {
        let mut policies: Vec<Box<dyn Policy>> = (0..4)
            .map(|s| match &net {
                Some(n) => Box::new(DeepNetPolicy::new(n.clone(), seed * 7 + s)) as Box<dyn Policy>,
                None => Box::new(Heuristic::new(seed * 7 + s)) as Box<dyn Policy>,
            })
            .collect();
        let mut state = State::new(4, seed).with_trade_mode(TradeMode::Full);
        let mut turns = 0u32;
        for _ in 0..6_000 {
            if matches!(state.phase, Phase::GameOver { .. }) {
                break;
            }
            let seat = state.decider() as usize;
            state.legal_into(&mut buf);
            let a = policies[seat].choose(&state, &buf);
            if state.apply(a).is_err() {
                break;
            }
            if !matches!(state.phase, Phase::Action) {
                continue;
            }
            turns += 1;
            // One sample per seat per action-phase visit: the belief error
            // against the truth, and the sizeless baseline's.
            for p in 0..4 {
                let size = state.hand_size(p) as f64;
                for r in 0..5 {
                    let truth = state.hand[p][r] as f64;
                    let t = (state.counting.cards(p, r) - truth).abs();
                    let u = (size / 5.0 - truth).abs();
                    tracked += t;
                    uniform += u;
                    samples += 1;
                    if turns > 60 {
                        late_tracked += t;
                        late_uniform += u;
                        late_samples += 1;
                    }
                }
            }
        }
    }
    println!("{games} games, {samples} belief samples");
    println!(
        "  public record error   {:.4} cards per resource per seat",
        tracked / samples as f64
    );
    println!(
        "  sizeless baseline     {:.4} cards per resource per seat",
        uniform / samples as f64
    );
    if late_samples > 0 {
        println!(
            "  late game (past 60 decisions): record {:.4}, baseline {:.4}",
            late_tracked / late_samples as f64,
            late_uniform / late_samples as f64
        );
    }
}
