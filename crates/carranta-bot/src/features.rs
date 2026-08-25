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
use carranta_core::topology::{
    ALL_VERTICES, HEX_COUNT, edges_at, endpoints_of, hex_vertices, iter_vertices, neighbors,
    vertex_bit,
};

/// Width of the observation vector.
///
/// Grew from 32 when the frontier senses were added. A network file carries
/// its own input count, and an older, narrower network reads the first slice
/// of this vector, so the first 32 entries keep their exact meaning and
/// anything new is appended.
pub const FEATURES: usize = 38;

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

/// Expected yield of every intersection at once: each vertex's entry is the
/// pips of the hexes it touches, robber-aware, the same arithmetic as
/// [`production`] counts for a settlement standing there.
///
/// One pass over the hexes, because this backs [`frontier`] and `encode` runs
/// for every candidate action of every decision: asking per vertex would walk
/// the board a hundred times per evaluation and was measured to cost a third
/// of the whole game rate.
fn vertex_pips(state: &State) -> [f64; 54] {
    let mut pips = [0.0f64; 54];
    for h in 0..HEX_COUNT as u8 {
        if h == state.robber || state.number[h as usize] == 0 {
            continue;
        }
        if state.terrain[h as usize].yields().is_none() {
            continue;
        }
        let worth = (6 - (7i32 - state.number[h as usize] as i32).abs()) as f64;
        for v in iter_vertices(hex_vertices(h)) {
            pips[v as usize] += worth;
        }
    }
    pips
}

/// Where one seat's network can still grow: the best open intersection at
/// each road-distance, and how much room is left before the frontier closes.
///
/// This is the sense the first 32 features lack. Without it two candidate
/// roads read identically unless one lengthens the measured route, so a road
/// toward a rich open intersection and a road into a dead corner scored the
/// same and the network built dead wood. With it, the road toward the rich
/// spot raises "best one road away" in the very next evaluation, which is the
/// same one-ply loop that already makes settlement placement look considered.
///
/// `now` is the best spot buildable this instant, `after_one` the best
/// after laying one legal road, `after_two` after two, and `room` counts the
/// open spots worth having within one road of the network.
struct Frontier {
    now: f64,
    after_one: f64,
    after_two: f64,
    room: u32,
}

fn frontier(state: &State, p: usize, pips: &[f64; 54]) -> Frontier {
    // Open under the distance rule (R-8.5): not built on, not adjacent to a
    // building. The same set `settlement_spots` starts from.
    let taken = state.all_buildings();
    let mut forbidden = taken;
    for v in iter_vertices(taken) {
        forbidden |= neighbors(v);
    }
    let open = ALL_VERTICES & !forbidden;

    let best = |spots: u64| {
        iter_vertices(spots)
            .map(|v| pips[v as usize])
            .fold(0.0f64, f64::max)
    };

    // Buildable now: open spots on the network (R-8.4).
    let ends = endpoints_of(state.roads[p]);
    let now = best(open & ends);

    // One road away: the far ends of every road that may legally be laid.
    let legal_roads = state.road_spots(p);
    let reach_one = endpoints_of(legal_roads);
    let after_one = best(open & reach_one);

    // Two roads away: grow one more edge from there, not through an
    // opponent's building (R-8.3) and not along a road already laid.
    let occupied = state.all_roads();
    let mut second = 0u128;
    for v in iter_vertices(reach_one & !state.blocking(p)) {
        second |= edges_at(v);
    }
    let reach_two = endpoints_of(second & !occupied);
    let after_two = best(open & (reach_one | reach_two));

    // Expansion room: open spots within one road that produce at all well. A
    // frontier of one great spot and a frontier of four are different
    // positions even when their best is equal, and the difference is what an
    // opponent's next settlement takes away.
    let room = iter_vertices(open & (ends | reach_one))
        .filter(|&v| pips[v as usize] >= 5.0)
        .count() as u32;

    Frontier {
        now,
        after_one,
        after_two,
        room,
    }
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

    // Every intersection's yield, once: both frontiers below read it.
    let spot_pips = vertex_pips(state);

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
        // ---- The rival's frontier ----
        //
        // What the strongest opponent can still reach, so a road or a
        // settlement that closes their best expansion reads as the position
        // change it is rather than a wasted turn.
        let theirs = frontier(state, q, &spot_pips);
        f[36] = theirs.now / 15.0;
        f[37] = theirs.after_one / 15.0;
    }

    // ---- My frontier ----
    //
    // Where this network can still grow. A candidate road toward a rich open
    // intersection raises `after_one` in the next evaluation; a dead-end road
    // raises nothing, and now reads as the nothing it is.
    let mine = frontier(state, me, &spot_pips);
    f[32] = mine.now / 15.0;
    f[33] = mine.after_one / 15.0;
    f[34] = mine.after_two / 15.0;
    f[35] = mine.room as f64 / 6.0;

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

    #[test]
    fn two_roads_from_one_settlement_no_longer_read_the_same() {
        // The blindness the frontier senses exist to cure: before them, a
        // road toward a rich open intersection and a road into a dead corner
        // produced identical observations unless one lengthened the measured
        // route, so the evaluation could not prefer the one a person would
        // pick. Lay each candidate road from one settlement and the frontier
        // features must now tell at least two of them apart, and the road
        // whose reach holds the richest open spot must read best.
        let mut distinguished = 0;
        for seed in 0..6u64 {
            let mut state = State::new(4, seed);
            let v0 = 18u8; // an inland intersection on every board
            state.settlements[0] |= vertex_bit(v0);
            let mut best_read = f64::MIN;
            let mut best_reach = f64::MIN;
            let mut reads = Vec::new();
            for e in carranta_core::topology::iter_edges(edges_at(v0)) {
                let mut laid = state.clone();
                laid.roads[0] |= carranta_core::topology::edge_bit(e);
                let f = encode(&laid, 0, Pending::default());
                // The richest open spot this road's network can reach in one
                // more road, computed independently of the encoder.
                let taken = laid.all_buildings();
                let mut forbidden = taken;
                for v in iter_vertices(taken) {
                    forbidden |= neighbors(v);
                }
                let open = ALL_VERTICES & !forbidden;
                let reach = endpoints_of(laid.road_spots(0));
                let table = vertex_pips(&laid);
                let richest = iter_vertices(open & reach)
                    .map(|v| table[v as usize])
                    .fold(0.0f64, f64::max);
                if f[33] > best_read {
                    best_read = f[33];
                    best_reach = richest;
                }
                reads.push((f[33], richest));
            }
            if reads.iter().any(|&(r, _)| (r - reads[0].0).abs() > 1e-9) {
                distinguished += 1;
                let top = reads.iter().map(|&(_, p)| p).fold(0.0f64, f64::max);
                assert!(
                    (best_reach - top).abs() < 1e-9,
                    "the road reading best reaches {best_reach} pips, \
                     but {top} was reachable (seed {seed})"
                );
            }
        }
        assert!(
            distinguished >= 3,
            "roads read identically on nearly every board: {distinguished}/6"
        );
    }

    #[test]
    fn the_frontier_senses_ride_behind_the_original_thirty_two() {
        // The append-only contract: a network trained at 32 inputs reads the
        // first slice of today's vector, so those entries may never move.
        // Spot-check the seam: the last original feature and the first new
        // one are both live, on their own sides of it.
        let state = State::new(4, 5);
        let f = encode(&state, 0, Pending::default());
        assert_eq!(FEATURES, 38, "the width this test pins down");
        assert!(f.len() == FEATURES);
        // A fresh deal has no buildings, so every frontier entry is at its
        // floor; a settlement wakes them.
        let mut built = state.clone();
        built.settlements[0] |= vertex_bit(18);
        built.roads[0] |= carranta_core::topology::edge_bit(
            carranta_core::topology::iter_edges(edges_at(18))
                .next()
                .expect("an edge"),
        );
        let g = encode(&built, 0, Pending::default());
        assert!(
            g[32] > 0.0 || g[33] > 0.0,
            "a settled network has a frontier: {:?}",
            &g[32..]
        );
    }
}
