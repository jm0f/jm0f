//! What a trained network sees (E-3).
//!
//! Engineered features rather than the raw board, because NEAT complexifies
//! badly with hundreds of inputs: the registered decision is to hand the
//! network the same kind of quantities the hand-set heuristic reads, and let
//! topology search find combinations, not rediscover the map.
//!
//! Everything is scaled into roughly the unit interval by fixed divisors. The
//! divisors are part of the observation contract: change one and every genome
//! ever trained reads the board differently, so they are constants here and
//! not knobs anywhere.
//!
//! Two inputs are not board facts but *pending consequences*, and they exist
//! because of the information rules in [`crate::Heuristic::score_action`]:
//! buying a development card must be scored without looking at the card, and
//! moving the robber must be scored without drawing the stolen card. The
//! candidate state for those actions cannot contain the outcome, so the fact
//! that one is pending is passed alongside instead, and the network learns
//! what a pending draw is worth exactly as the heuristic carries `buy_dev`
//! and `steal` weights.

use carranta_core::longest_road::longest_road;
use carranta_core::state::{PORT_KINDS, State};
use carranta_core::topology::{HEX_COUNT, hex_vertices};

/// Width of the observation vector.
pub const FEATURES: usize = 32;

/// The pending consequences of a candidate action. See the module note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    /// A development card has been paid for and is about to be drawn.
    pub bought_dev: bool,
    /// A robbery is about to draw from a victim holding this many cards.
    /// Zero when no steal is pending.
    pub steal_from: u8,
}

/// Expected production for one seat: pips per resource, and how many distinct
/// resources produce at all.
///
/// A number's pips are its ways of being rolled, so 6 and 8 are worth six
/// times a 2 or a 12; a city counts double; a hex under the robber counts
/// nothing while it sits there. The same arithmetic as the heuristic's value
/// function, exposed so an observation and a hand-set evaluation cannot drift
/// apart.
pub fn production(state: &State, p: usize) -> ([f64; 5], u32) {
    let mut pips = [0.0f64; 5];
    let mut kinds = 0u32;
    let mut produced = [false; 5];
    for h in 0..HEX_COUNT as u8 {
        if h == state.robber {
            continue;
        }
        let n = state.number[h as usize];
        if n == 0 {
            continue;
        }
        let Some(res) = state.terrain[h as usize].yields() else {
            continue;
        };
        let corners = hex_vertices(h);
        let count = (state.settlements[p] & corners).count_ones()
            + 2 * (state.cities[p] & corners).count_ones();
        if count > 0 {
            pips[res as usize] += (6 - (7i32 - n as i32).abs()) as f64 * count as f64;
            if !produced[res as usize] {
                produced[res as usize] = true;
                kinds += 1;
            }
        }
    }
    (pips, kinds)
}

/// Encode one seat's view of a position.
///
/// Public information plus this seat's own hand: exactly what a person at the
/// table knows, so nothing here peeks. Opponent hands appear only as sizes,
/// and only the strongest opponent's board position is summarised, the same
/// competitive framing the heuristic uses.
pub fn encode(state: &State, me: usize, pending: Pending) -> [f64; FEATURES] {
    let mut f = [0.0f64; FEATURES];
    let (pips, kinds) = production(state, me);

    // ---- Mine ----
    f[0] = state.victory_points(me) as f64 / 10.0;
    f[1] = pips.iter().sum::<f64>() / 30.0;
    f[2] = kinds as f64 / 5.0;
    let ports = (0..PORT_KINDS).filter(|&k| state.has_port(me, k)).count();
    f[3] = ports as f64 / 4.0;
    f[4] = longest_road(state.roads[me], state.blocking(me)) as f64 / 10.0;
    f[5] = state.militia_played[me] as f64 / 5.0;
    f[6] = state.dev_count(me) as f64 / 5.0;
    f[7] = (state.settlements_left[me] + state.cities_left[me]) as f64 / 9.0;
    for r in 0..5 {
        f[8 + r] = state.hand[me][r] as f64 / 7.0;
        f[14 + r] = pips[r] / 15.0;
    }
    f[13] = state.hand_size(me).saturating_sub(7) as f64 / 7.0;

    // ---- The strongest opponent ----
    //
    // By victory points, pips breaking ties, seat number breaking those, so
    // the pick is deterministic and the same for every candidate scored from
    // one position. Each opponent's production is computed once: this
    // function runs for every candidate action of every decision, and a
    // comparison sort that recomputed a board walk per comparison was most of
    // the evaluation bill.
    let mut rival: Option<(u32, u32, usize, f64)> = None;
    for q in 0..state.players as usize {
        if q == me {
            continue;
        }
        let (qpips, _) = production(state, q);
        let total: f64 = qpips.iter().sum();
        let key = (state.victory_points(q), total as u32, q);
        if rival.is_none_or(|(vp, pips, at, _)| key > (vp, pips, at)) {
            rival = Some((key.0, key.1, key.2, total));
        }
    }
    if let Some((_, _, q, qpips)) = rival {
        f[19] = state.victory_points(q) as f64 / 10.0;
        f[20] = qpips / 30.0;
        f[21] = longest_road(state.roads[q], state.blocking(q)) as f64 / 10.0;
        f[22] = state.militia_played[q] as f64 / 5.0;
    }

    // ---- The table ----
    for r in 0..5 {
        f[23 + r] = state.supply[r] as f64 / 19.0;
    }
    f[28] = state.offer_count as f64 / 8.0;

    // ---- Pending consequences ----
    f[29] = if pending.bought_dev { 1.0 } else { 0.0 };
    f[30] = pending.steal_from.min(6) as f64 / 6.0;

    // How many offers this seat has already put up this turn. The heuristic
    // charges a hand-set toll per request so it does not spam the market; a
    // network gets the count as an input and prices its own patience.
    f[31] = state.offers_made[me] as f64 / 20.0;

    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_observation_is_bounded_and_deterministic() {
        // The divisors are the contract: a feature that wanders far outside
        // the unit interval starves or saturates every downstream weight.
        for seed in 0..8u64 {
            let state = State::new(4, seed);
            for me in 0..4 {
                let a = encode(&state, me, Pending::default());
                let b = encode(&state, me, Pending::default());
                assert_eq!(a, b, "a pure function of the position");
                for (i, v) in a.iter().enumerate() {
                    assert!(
                        (-0.01..=2.0).contains(v),
                        "feature {i} out of range: {v} (seed {seed}, seat {me})"
                    );
                }
            }
        }
    }

    #[test]
    fn pending_consequences_reach_the_vector() {
        let state = State::new(4, 3);
        let base = encode(&state, 0, Pending::default());
        let bought = encode(
            &state,
            0,
            Pending {
                bought_dev: true,
                steal_from: 0,
            },
        );
        let stealing = encode(
            &state,
            0,
            Pending {
                bought_dev: false,
                steal_from: 4,
            },
        );
        assert_eq!(base[29], 0.0);
        assert_eq!(bought[29], 1.0);
        assert!((stealing[30] - 4.0 / 6.0).abs() < 1e-12);
        // And nothing else moves: the flags are channels, not modifiers.
        assert_eq!(&base[..29], &bought[..29]);
        assert_eq!(&base[..29], &stealing[..29]);
    }
}
