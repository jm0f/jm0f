//! How much the bots actually trade with each other, over sixty self-play games.
//!
//! Public API only, so the same file measures any version of the policy: run it,
//! change the rule, run it again.

use carranta_bot::{Heuristic, Policy, settle_market};
use carranta_core::state::{Phase, State, TradeMode};

fn main() {
    let (mut trades, mut games, mut offers) = (0u32, 0u32, 0u32);
    for seed in 0..60u64 {
        let mut state = State::new(4, seed).with_trade_mode(TradeMode::Full);
        let mut bots: Vec<Heuristic> = (0..4).map(|s| Heuristic::new(seed * 13 + s)).collect();
        let mut buf = Vec::new();
        for _ in 0..20_000 {
            if matches!(state.phase, Phase::GameOver { .. }) {
                games += 1;
                break;
            }
            state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = state.decider() as usize;
            let action = {
                let mut refs: Vec<&mut dyn Policy> =
                    bots.iter_mut().map(|b| b as &mut dyn Policy).collect();
                refs[seat].choose(&state, &buf)
            };
            if matches!(action, carranta_core::Action::ProposeTrade { .. }) {
                offers += 1;
            }
            if state.apply(action).is_err() {
                break;
            }
            let mut refs: Vec<&mut dyn Policy> =
                bots.iter_mut().map(|b| b as &mut dyn Policy).collect();
            trades += settle_market(&mut state, &mut refs);
        }
    }
    println!("{games} games finished, {offers} offers made, {trades} taken");
}
