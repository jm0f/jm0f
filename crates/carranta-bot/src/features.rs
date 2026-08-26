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
};

/// Width of the observation vector.
///
/// Grew from 32 when the frontier senses were added (E-29), and from 38 when
/// the observation became the full table context a person at it has (E-30):
/// every opponent rather than only the strongest, the race for contested
/// spots, robber leverage per victim, trade rates, own development faces and
/// the standing market. A network file carries its own input count, and an
/// older, narrower network reads the first slice of this vector, so existing
/// entries keep their exact meaning and anything new is appended.
///
/// 78 to 93 added the public record (E-33): each ranked opponent's expected
/// hand per resource, counted by the engine from the public card movements
/// the whole table saw. What a person who pays attention knows, the network
/// now reads.
pub const FEATURES: usize = 93;

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
/// after laying one legal road, `after_two` after two, `room` counts the
/// open spots worth having within one road of the network, and `reach01` is
/// that within-one-road set itself, kept so the race for a spot two players
/// can both reach is computable as an intersection.
struct Frontier {
    now: f64,
    after_one: f64,
    after_two: f64,
    room: u32,
    reach01: u64,
}

/// The set every frontier is measured against, computed once per position:
/// spots open under the distance rule (R-8.5), and the roads already laid.
/// `encode` evaluates one frontier per seat and this walk is identical for
/// all of them.
fn openings(state: &State) -> (u64, u128) {
    let taken = state.all_buildings();
    let mut forbidden = taken;
    for v in iter_vertices(taken) {
        forbidden |= neighbors(v);
    }
    (ALL_VERTICES & !forbidden, state.all_roads())
}

fn frontier(state: &State, p: usize, pips: &[f64; 54], open: u64, occupied: u128) -> Frontier {
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
    let reach01 = open & (ends | reach_one);
    let room = iter_vertices(reach01)
        .filter(|&v| pips[v as usize] >= 5.0)
        .count() as u32;

    Frontier {
        now,
        after_one,
        after_two,
        room,
        reach01,
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

    // Every intersection's yield and the shared frontier ground, once: every
    // seat's frontier below reads them.
    let spot_pips = vertex_pips(state);
    let (open, occupied) = openings(state);

    // ---- The opponents, strongest first ----
    //
    // Ranked by victory points, pips breaking ties, seat number breaking
    // those, so the order is deterministic and the same for every candidate
    // scored from one position. Rank slots rather than seat slots: "the
    // leader", "the second", "the third" are the roles a person tracks, and
    // they survive any permutation of who sits where. Each opponent's
    // production is computed once: this function runs for every candidate
    // action of every decision, and a comparison sort that recomputed a
    // board walk per comparison was most of the evaluation bill.
    // A stack array, not a Vec: this is the hottest loop in the engine and
    // an allocation per candidate evaluation would be most of its bill.
    let mut ranked = [(0u32, 0u32, 0usize, 0.0f64); 3];
    let mut rivals = 0usize;
    for q in 0..state.players as usize {
        if q == me {
            continue;
        }
        let (qpips, _) = production(state, q);
        let total: f64 = qpips.iter().sum();
        ranked[rivals] = (state.victory_points(q), total as u32, q, total);
        rivals += 1;
    }
    let ranked = &mut ranked[..rivals];
    ranked.sort_by(|a, b| (b.0, b.1, b.2).cmp(&(a.0, a.1, a.2)));

    // The union of everything any opponent can settle within one road: what
    // the contested features below measure my own frontier against.
    let mut their_reach = 0u64;
    for (rank, &(vp, _, q, qpips)) in ranked.iter().enumerate() {
        let theirs = frontier(state, q, &spot_pips, open, occupied);
        their_reach |= theirs.reach01;
        // ---- The public record (E-33) ----
        //
        // What this opponent's hand is expected to hold, counted from the
        // card movements everyone at the table watched. This is what prices
        // a robbery by what it can take, a Monopoly by what it will gather,
        // and an offer by whether the taker can actually pay.
        for r in 0..5 {
            f[78 + rank * 5 + r] = state.counting.cards(q, r) / 7.0;
        }
        if rank == 0 {
            // The strongest opponent keeps the slots it has had since the
            // observation was 32 wide: older networks read these.
            f[19] = vp as f64 / 10.0;
            f[20] = qpips / 30.0;
            f[21] = longest_road(state.roads[q], state.blocking(q)) as f64 / 10.0;
            f[22] = state.militia_played[q] as f64 / 5.0;
            f[36] = theirs.now / 15.0;
            f[37] = theirs.after_one / 15.0;
            // And the rest of what a person knows about the leader, appended
            // with the full-context widening (E-30).
            f[38] = state.hand_size(q) as f64 / 7.0;
            f[39] = state.dev_count(q) as f64 / 5.0;
            f[40] = (state.settlements_left[q] + state.cities_left[q]) as f64 / 9.0;
        } else {
            // The second and third opponents, nine senses each: the same
            // public facts, so a threat from anyone at the table reads, not
            // only from whoever happens to lead.
            let base = 41 + (rank - 1) * 9;
            f[base] = vp as f64 / 10.0;
            f[base + 1] = qpips / 30.0;
            f[base + 2] = longest_road(state.roads[q], state.blocking(q)) as f64 / 10.0;
            f[base + 3] = state.militia_played[q] as f64 / 5.0;
            f[base + 4] = state.hand_size(q) as f64 / 7.0;
            f[base + 5] = state.dev_count(q) as f64 / 5.0;
            f[base + 6] = (state.settlements_left[q] + state.cities_left[q]) as f64 / 9.0;
            f[base + 7] = theirs.now / 15.0;
            f[base + 8] = theirs.after_one / 15.0;
        }
    }

    // ---- My frontier ----
    //
    // Where this network can still grow. A candidate road toward a rich open
    // intersection raises `after_one` in the next evaluation; a dead-end road
    // raises nothing, and now reads as the nothing it is.
    let mine = frontier(state, me, &spot_pips, open, occupied);
    f[32] = mine.now / 15.0;
    f[33] = mine.after_one / 15.0;
    f[34] = mine.after_two / 15.0;
    f[35] = mine.room as f64 / 6.0;

    // ---- The race (E-30) ----
    //
    // My frontier and an opponent's frontier were separate readings: the
    // network could see that my best spot and theirs were both worth twelve
    // pips, never that they were the same intersection. The intersection is
    // the fact that decides "build there before they do", so it is a sense of
    // its own: the best spot both I and somebody can reach within one road,
    // and how many of my good spots are contested at all.
    let contested = mine.reach01 & their_reach;
    f[59] = iter_vertices(contested)
        .map(|v| spot_pips[v as usize])
        .fold(0.0f64, f64::max)
        / 15.0;
    f[60] = iter_vertices(contested)
        .filter(|&v| spot_pips[v as usize] >= 5.0)
        .count() as f64
        / 6.0;

    // ---- Robber leverage (E-30) ----
    //
    // What the robber's current square is costing each of us, in pips. A
    // candidate robber move is scored on the resulting state, so these are
    // the senses that make placement against *anyone* visible: parking it on
    // the second player's best hex used to read identically to parking it on
    // a desert unless the victim happened to lead.
    let robbed = |p: usize| -> f64 {
        let h = state.robber;
        if state.number[h as usize] == 0 || state.terrain[h as usize].yields().is_none() {
            return 0.0;
        }
        let worth = (6 - (7i32 - state.number[h as usize] as i32).abs()) as f64;
        let corners = hex_vertices(h);
        let count = (state.settlements[p] & corners).count_ones()
            + 2 * (state.cities[p] & corners).count_ones();
        worth * count as f64
    };
    f[61] = robbed(me) / 15.0;
    for (rank, &(_, _, q, _)) in ranked.iter().enumerate() {
        f[62 + rank] = robbed(q) / 15.0;
    }

    // ---- My trade geometry (E-30) ----
    //
    // The effective exchange rate per resource, ports folded in: 0 at the
    // bank's four, half at a generic port's three, one at the matching
    // port's two. Ports were a count before; this is what the count buys.
    for r in 0..5 {
        let rate = if state.has_port(me, r + 1) {
            2
        } else if state.has_port(me, 0) {
            3
        } else {
            4
        };
        f[65 + r] = (4 - rate) as f64 / 2.0;
    }

    // ---- My development faces (E-30) ----
    //
    // Which cards this hand can actually play, by face. The count at f[6]
    // says a card is held; these say what it is, which a person holding the
    // hand knows.
    use carranta_core::state::DevCard;
    for (i, (card, cap)) in [
        (DevCard::Militia, 3.0),
        (DevCard::VictoryPoint, 3.0),
        (DevCard::Monopoly, 2.0),
        (DevCard::RoadBuilding, 2.0),
        (DevCard::Invention, 2.0),
    ]
    .into_iter()
    .enumerate()
    {
        f[70 + i] = f64::from(state.dev_playable(me, card)).min(cap) / cap;
    }

    // ---- The standing market and the clock (E-30) ----
    //
    // Offers addressed to this seat in particular, whether the leader has an
    // offer up, and whether the dice are still to be thrown this turn: the
    // table facts a person weighs before spending a card or a call.
    let offers = state.live_offers();
    f[75] = offers
        .iter()
        .filter(|o| o.to == Some(me as u8))
        .count()
        .min(3) as f64
        / 3.0;
    if let Some(&(_, _, leader, _)) = ranked.first() {
        f[76] = if offers.iter().any(|o| o.from == leader as u8) {
            1.0
        } else {
            0.0
        };
    }
    f[77] = if state.to_act == me as u8 && state.dice == [0, 0] {
        1.0
    } else {
        0.0
    };

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
    use carranta_core::topology::vertex_bit;

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
    fn the_public_record_reaches_the_observation_and_a_steal_stays_blurred() {
        // An opponent's cards arrived in public, so their rank slot must say
        // what they hold; a robbery moves one card only two seats saw, so
        // after it the record must spread, never name.
        let mut state = State::new(4, 5);
        // Seat 2 publicly gains three brick and one ore, and leads on VPs.
        state.settlements[2] |= vertex_bit(18);
        state.hand[2] = [3, 0, 0, 0, 1];
        state.counting.public(2, [3, 0, 0, 0, 1]);
        let f = encode(&state, 0, Pending::default());
        // Seat 2 is the only opponent with a settlement, so it ranks first:
        // its expected brick sits at f[78].
        assert!((f[78] - 3.0 / 7.0).abs() < 1e-9, "three brick, watched");
        assert!((f[82] - 1.0 / 7.0).abs() < 1e-9, "one ore, watched");

        // Seat 1 steals one of the four. The table saw a card move and no
        // more: seat 2's brick belief drops fractionally, and no cell of
        // seat 1's row claims a whole known card.
        state.hand[1][0] += 1;
        state.hand[2][0] -= 1;
        state.counting.steal(1, 2, 4);
        let g = encode(&state, 0, Pending::default());
        assert!(
            g[78] < f[78] && g[78] > 2.0 / 7.0,
            "the victim's brick belief blurred rather than resolved: {}",
            g[78]
        );
        // The thief ranks below the leader; its row holds one card's worth
        // of fractional belief, most of it brick, none of it certain.
        let thief_brick = state.counting.cards(1, 0);
        assert!(
            thief_brick > 0.5 && thief_brick < 1.0,
            "the table suspects brick without knowing it: {thief_brick}"
        );
    }

    #[test]
    fn the_race_for_a_spot_is_a_fact_of_its_own() {
        // Two networks one road from the same rich intersection is the
        // position "build there before they do". Before E-30 the observation
        // held my frontier and theirs as separate numbers and the sameness of
        // the spot was unrepresentable. Put two players either side of one
        // intersection and the contested senses must light; give each a
        // private corner instead and they must not.
        let state = State::new(4, 5);
        let spot = 18u8;
        let mut racing = state.clone();
        let mut around: Vec<u8> = iter_vertices(neighbors(spot)).collect();
        assert!(around.len() >= 2, "an inland intersection has neighbours");
        around.sort_unstable();
        // Each player owns a settlement two steps out, roaded to a
        // neighbour of the spot: both are one road from it.
        for (p, &v) in [around[0], around[1]]
            .iter()
            .enumerate()
            .map(|(i, v)| (i, v))
        {
            let far: Vec<u8> = iter_vertices(neighbors(v)).filter(|&w| w != spot).collect();
            racing.settlements[p] |= vertex_bit(far[0]);
            let e = carranta_core::topology::iter_edges(edges_at(v) & edges_at(far[0]))
                .next()
                .expect("adjacent vertices share an edge");
            racing.roads[p] |= carranta_core::topology::edge_bit(e);
        }
        let f = encode(&racing, 0, Pending::default());
        // The spot itself must be open (no buildings adjoin it in this
        // construction) and reachable by both, so the contested senses read.
        assert!(
            f[59] > 0.0,
            "two players one road from one spot is a race: {:?}",
            &f[59..61]
        );

        // Same pieces, but player 1 exiled to the far side of the board:
        // nothing I can reach is contested by anyone.
        let mut apart = state.clone();
        apart.settlements[0] = racing.settlements[0];
        apart.roads[0] = racing.roads[0];
        let f = encode(&apart, 0, Pending::default());
        assert_eq!(f[59], 0.0, "no opponent near, no race");
        assert_eq!(f[60], 0.0);
    }

    #[test]
    fn the_robber_reads_per_victim_not_only_against_the_leader() {
        // Robber leverage: what the robber's square costs each seat. Park it
        // on a hex where only the third-ranked player produces and that
        // player's slot must light while the others stay dark, which is
        // exactly the placement that was invisible when only the strongest
        // opponent was observed.
        let mut state = State::new(4, 7);
        // Find a producing hex and give player 3 (weakest: no other pieces)
        // a settlement on its corner, players 1 and 2 pieces elsewhere.
        let hex = (0..HEX_COUNT as u8)
            .find(|&h| {
                state.number[h as usize] != 0 && state.terrain[h as usize].yields().is_some()
            })
            .expect("a producing hex");
        let corner = iter_vertices(hex_vertices(hex)).next().expect("a corner");
        state.settlements[3] |= vertex_bit(corner);
        // Give ranks: players 1 and 2 stronger via settlements far away with
        // no production overlap needed; VPs from settlements rank them.
        let far: Vec<u8> = iter_vertices(ALL_VERTICES & !hex_vertices(hex) & !neighbors(corner))
            .filter(|&v| v != corner)
            .collect();
        state.settlements[1] |= vertex_bit(far[0]) | vertex_bit(far[2]);
        state.settlements[2] |= vertex_bit(far[4]) | vertex_bit(far[6]);
        state.robber = hex;
        let f = encode(&state, 0, Pending::default());
        // Player 3 holds one settlement, players 1 and 2 hold two each, so 3
        // ranks last: its robber loss lands in f[64] and it is the only one.
        assert!(
            f[64] > 0.0,
            "the robbed third player's slot lights: {:?}",
            &f[61..65]
        );
        assert_eq!(f[61], 0.0, "I lose nothing");
    }

    #[test]
    fn every_opponent_is_observed_strongest_first() {
        // Rank slots, not seat slots: the leader's block must hold the
        // highest victory points whatever seat the leader sits in.
        let mut state = State::new(4, 9);
        let spots: Vec<u8> = iter_vertices(ALL_VERTICES).collect();
        // Seat 3 gets three settlements, seat 1 two, seat 2 one, spaced out
        // so the distance rule stays honoured.
        let mut used = 0u64;
        let mut give = |state: &mut State, p: usize, n: usize| {
            let mut placed = 0;
            for &v in &spots {
                if placed == n {
                    break;
                }
                if (used | neighbors(v)) & vertex_bit(v) == 0 && used & neighbors(v) == 0 {
                    state.settlements[p] |= vertex_bit(v);
                    used |= vertex_bit(v) | neighbors(v);
                    placed += 1;
                }
            }
            assert_eq!(placed, n, "room for the fixture");
        };
        give(&mut state, 3, 3);
        give(&mut state, 1, 2);
        give(&mut state, 2, 1);
        let f = encode(&state, 0, Pending::default());
        assert!(
            f[19] >= f[41] && f[41] >= f[50],
            "victory points descend down the ranks: {} {} {}",
            f[19],
            f[41],
            f[50]
        );
        assert!((f[19] - 0.3).abs() < 1e-9, "the leader's three points");
        assert!((f[50] - 0.1).abs() < 1e-9, "the third's one");
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
        assert_eq!(FEATURES, 93, "the width this test pins down");
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
