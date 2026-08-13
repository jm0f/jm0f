//! Recorded games for tests and benchmarks.

use carranta_bot::{Heuristic, Policy};
use carranta_core::Action;
use carranta_core::state::{MAX_OFFERS, Phase, State, TradeMode};
use carranta_record::{Log, Recorder, SeatId};

/// Offer everyone entitled to take a live offer the chance to do so, through
/// the recorder so the trade lands in the log.
///
/// The bot crate's `settle_market` writes straight to a `State`, which would
/// leave every completed trade out of the record — so a self-play corpus built
/// on it would show a busy market and no trades. Declines are recorded once
/// per offer and seat (H-4), not once per pass, or the churn would be an
/// artefact of this loop rather than of the game.
pub fn settle(rec: &mut Recorder, bots: &mut [Heuristic]) -> u32 {
    if rec.state().trade_mode == TradeMode::Disabled || rec.state().offer_count == 0 {
        return 0;
    }
    let mut declined = [[false; MAX_OFFERS]; 4];
    let mut trades = 0;
    for _ in 0..16 {
        let mut acted = false;
        #[allow(clippy::needless_range_loop)] // `i` indexes both offers and `declined`
        'outer: for i in 0..rec.state().offer_count as usize {
            for seat in 0..bots.len() {
                if rec.state().offers[i].from as usize == seat {
                    continue;
                }
                let take = Action::AcceptTrade {
                    offer: i as u8,
                    by: seat as u8,
                };
                // Not a party to it, or cannot pay.
                let mut probe = *rec.state();
                if probe.apply(take).is_err() {
                    continue;
                }
                if bots[seat].accepts(rec.state(), seat, i) {
                    if rec.apply(take).is_ok() {
                        trades += 1;
                    }
                    acted = true;
                    break 'outer;
                } else if !declined[seat][i] {
                    declined[seat][i] = true;
                    rec.decline(i as u8, seat as u8);
                }
            }
        }
        if !acted {
            break;
        }
    }
    trades
}

/// Record one four-seat self-play game.
///
/// `rotation` moves the agents around the table: the seat at index `s` is
/// played by agent `(s + rotation) % 4`. Without it an agent's rating would
/// measure its seat rather than its play (A-4).
pub fn self_play_rotated(seed: u64, mode: TradeMode, rotation: usize) -> Log {
    let agent_at = |seat: usize| (seat + rotation) % 4;
    // Seat-indexed, but each seat's generator follows its *agent* — so an
    // agent plays the same way wherever it sits.
    let mut bots: Vec<Heuristic> = (0..4)
        .map(|seat| Heuristic::new(seed * 31 + agent_at(seat) as u64 + 1))
        .collect();
    let mut rec = Recorder::new(
        seed,
        seed,
        State::new(4, seed).with_trade_mode(mode),
        (0..4)
            .map(|seat| SeatId::agent(agent_at(seat) as u64, "heuristic", 1))
            .collect(),
    );

    let mut buf = Vec::new();
    for _ in 0..20_000 {
        if matches!(rec.state().phase, Phase::GameOver { .. }) {
            break;
        }
        rec.state().legal_into(&mut buf);
        if buf.is_empty() {
            break;
        }
        let seat = rec.state().decider() as usize;
        let action = bots[seat].choose(rec.state(), &buf);
        if rec.apply(action).is_err() {
            break;
        }
        settle(&mut rec, &mut bots);
    }
    let winner = match rec.state().phase {
        Phase::GameOver { winner } => Some(winner),
        _ => None,
    };
    rec.finish_into(winner)
}

/// Record one four-seat self-play game with agent `i` in seat `i`.
pub fn self_play(seed: u64, mode: TradeMode) -> Log {
    self_play_rotated(seed, mode, 0)
}
