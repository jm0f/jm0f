//! A seat played by an evolved network.
//!
//! The same one-ply shape as the heuristic: copy the state, apply the
//! candidate, score the result, best score wins. What changes is who does the
//! scoring: a [`Net`] over the [`features`] observation instead of a hand-set
//! linear form. Every rule about information stays, because they are rules
//! about fairness rather than about evaluation:
//!
//! - **Buying a development card** is scored with the cost paid and the card
//!   undrawn, the pending purchase flagged in the observation instead.
//! - **Moving the robber** is scored with the robber moved and the stolen
//!   card undrawn, the victim's hand size flagged in the observation.
//! - **Rolling** is scored as the status quo, because its outcome has not
//!   happened and the seat cannot shape it.
//!
//! Proposals are the case the heuristic prices with three hand-set weights,
//! and the network prices with none. A proposal changes nothing until it is
//! taken, so it is scored by the swap it would produce, credited at half
//! because it may be refused, and credited at nothing when no opponent who
//! could cover it would gain by taking it, judged by this same network from
//! *their* seat. The toll for asking over and over is not priced here at all:
//! the observation carries the seat's own offers-made count, and the network
//! learns its own patience.

use carranta_core::action::{Action, DEV_COST};
use carranta_core::rng::{Rng, Stream};
use carranta_core::state::State;

use crate::Policy;
use crate::features::{self, Pending};
use crate::net::Net;

/// How much of a proposed swap's gain is credited when deciding to offer it.
///
/// Scaffolding, not knowledge: it says only that an offer is worth less than
/// a certainty, and the network prices everything else.
const OFFER_CREDIT: f64 = 0.5;

/// A [`Policy`] over an evolved network.
pub struct NetPolicy {
    net: Net,
    rng: Rng,
}

impl NetPolicy {
    pub fn new(net: Net, seed: u64) -> Self {
        NetPolicy {
            net,
            rng: Rng::new(seed),
        }
    }

    fn value(&self, state: &State, me: usize, pending: Pending) -> f64 {
        self.net.eval(&features::encode(state, me, pending))
    }

    /// Score one candidate action. Mirrors the heuristic's information rules;
    /// see the module note.
    ///
    /// `ctx` carries what is identical across every candidate of one decision:
    /// each seat's value of the standing position, and the value of having
    /// asked one more time. Proposals are the bulk of a trading decision's
    /// candidates and share those numbers, so computing them per candidate was
    /// most of the cost of a decision.
    fn score(&self, state: &State, me: usize, action: Action, ctx: &Decision) -> f64 {
        match action {
            Action::Roll => self.value(state, me, Pending::default()),

            Action::BuyDev => {
                // Pay the cost, but do not look at the card.
                let mut next = *state;
                for (r, &c) in DEV_COST.iter().enumerate() {
                    next.hand[me][r] -= c;
                    next.supply[r] += c;
                }
                self.value(
                    &next,
                    me,
                    Pending {
                        bought_dev: true,
                        steal_from: 0,
                    },
                )
            }

            Action::MoveRobber { hex, victim } => {
                // Move the robber, but do not draw the stolen card.
                let mut next = *state;
                next.robber = hex;
                let steal_from = victim.map_or(0, |v| state.hand_size(v as usize).min(6) as u8);
                self.value(
                    &next,
                    me,
                    Pending {
                        bought_dev: false,
                        steal_from,
                    },
                )
            }

            Action::ProposeTrade {
                to: None,
                give,
                want,
                ..
            } => {
                // Making an offer changes nothing until somebody takes it, so
                // it is valued by the swap it would produce. The probe carries
                // the bumped offer counters, so the network sees asking as a
                // state change and can learn what asking again is worth.
                let mut swapped = *state;
                swapped.offers_made[me] += 1;
                for r in 0..5 {
                    swapped.hand[me][r] = swapped.hand[me][r] - give[r] + want[r];
                }
                let gain = self.value(&swapped, me, Pending::default()) - ctx.asked;
                // Would anybody take it? Each candidate taker is judged by this
                // same network from their own seat, which is the question they
                // will actually be asked. Reading their hands to answer it is
                // what the competitive evaluation has always done.
                let taker = (0..state.players as usize).any(|q| {
                    if q == me || !state.holds(q, &want) {
                        return false;
                    }
                    let mut theirs = *state;
                    for r in 0..5 {
                        theirs.hand[q][r] = theirs.hand[q][r] - want[r] + give[r];
                    }
                    self.value(&theirs, q, Pending::default()) > ctx.standing[q]
                });
                let credit = if taker { gain * OFFER_CREDIT } else { 0.0 };
                ctx.asked + credit
            }

            _ => {
                let mut next = *state;
                if next.apply(action).is_err() {
                    return f64::NEG_INFINITY;
                }
                self.value(&next, me, Pending::default())
            }
        }
    }
}

/// What every candidate of one decision shares.
struct Decision {
    /// Each seat's value of the standing position, by that seat's own lights.
    standing: [f64; carranta_core::state::MAX_PLAYERS],
    /// The proposer's value of the standing position with one more ask spent.
    asked: f64,
}

impl Policy for NetPolicy {
    fn choose(&mut self, state: &State, legal: &[Action]) -> Action {
        debug_assert!(!legal.is_empty());
        // A discard is decided by the seat that owes it, not the seat to act.
        let me = state.decider() as usize;
        // Shared once, not per candidate. The asked value only matters when a
        // proposal is among the candidates, and it costs one evaluation.
        let ctx = Decision {
            standing: core::array::from_fn(|q| {
                if q < state.players as usize {
                    self.value(state, q, Pending::default())
                } else {
                    0.0
                }
            }),
            asked: {
                let mut asked = *state;
                asked.offers_made[me] += 1;
                self.value(&asked, me, Pending::default())
            },
        };
        let mut best = f64::NEG_INFINITY;
        let mut ties = 0u32;
        let mut chosen = legal[0];
        for &a in legal {
            let s = self.score(state, me, a, &ctx);
            if s > best {
                best = s;
                ties = 1;
                chosen = a;
            } else if s == best {
                // Reservoir sampling over the tied set, so every equally good
                // move is equally likely without collecting them. Ties are
                // exact float equality, which identical probes really produce.
                ties += 1;
                if self.rng.below(Stream::Dice, ties) == 0 {
                    chosen = a;
                }
            }
        }
        chosen
    }

    fn accepts(&mut self, state: &State, seat: usize, offer: usize) -> bool {
        let Some(o) = state.live_offers().get(offer) else {
            return false;
        };
        if !state.may_accept(seat, o) || !state.holds(seat, &o.want) {
            return false;
        }
        // `give` and `want` are the proposer's side: the taker hands over
        // `want` and receives `give`. Strictly better or it stays where it is.
        let mut next = *state;
        for r in 0..5 {
            next.hand[seat][r] = next.hand[seat][r] - o.want[r] + o.give[r];
        }
        self.value(&next, seat, Pending::default()) > self.value(state, seat, Pending::default())
    }
}

/// The same network, one ply deeper.
///
/// Blondie24's lesson at the smallest useful depth: an evolved evaluation
/// inside a search is worth more than the same evaluation used greedily,
/// because a candidate is finally worth what it enables rather than what it
/// is. A one-ply policy cannot see that a port trade buys the settlement it
/// could not otherwise afford; a two-ply one prices the pair.
///
/// The information rules are [`NetPolicy`]'s, kept by construction: only
/// candidates whose true `apply` reveals nothing are expanded to the second
/// ply. Rolling, buying a development card and moving the robber all resolve
/// chance the seat is not entitled to see, so they keep their calibrated
/// one-ply scores, exactly as before. Proposals keep theirs too: an offer's
/// value is the credited swap, and a follow-up from an untaken offer would
/// price a different question.
///
/// Depth is bought with a beam rather than breadth: every candidate is
/// scored at one ply first, and only the strongest few are expanded, since
/// the ordering is decided among the leaders. Follow-up proposals are not
/// generated at the second ply: their credit machinery costs a table of
/// evaluations per expansion and asking-later is nearly free.
pub struct DeepNetPolicy {
    inner: NetPolicy,
    beam: usize,
    /// Whether market answers get the second ply too. On by default; the
    /// flat-market variant exists so the difference can be measured.
    deep_market: bool,
    /// Scratch for the second ply's legal actions, reused per expansion.
    buf: Vec<Action>,
    scored: Vec<(f64, usize)>,
}

/// How many candidates the beam expands. Eight covers every build and bank
/// trade a hand can usually afford while keeping a decision under a
/// millisecond; the long tail is proposals, which are never expanded.
const BEAM: usize = 8;

impl DeepNetPolicy {
    pub fn new(net: Net, seed: u64) -> Self {
        DeepNetPolicy {
            inner: NetPolicy::new(net, seed),
            beam: BEAM,
            deep_market: true,
            buf: Vec::new(),
            scored: Vec::new(),
        }
    }

    /// The ablation: deep moves, one-ply market answers. Kept so the worth
    /// of the deep answer can be measured rather than assumed.
    pub fn flat_market(net: Net, seed: u64) -> Self {
        DeepNetPolicy {
            deep_market: false,
            ..Self::new(net, seed)
        }
    }

    /// Whether this candidate's true `apply` reveals no chance outcome, so
    /// the position after it is honest to look at.
    fn expandable(a: &Action) -> bool {
        !matches!(
            a,
            Action::Roll
                | Action::BuyDev
                | Action::MoveRobber { .. }
                | Action::ProposeTrade { .. }
                | Action::EndTurn
        )
    }

    /// The best own follow-up from the position `a1` produces, or the
    /// one-ply score when the position is not this seat's to continue.
    fn second_ply(&mut self, state: &State, me: usize, a1: Action, fallback: f64) -> f64 {
        let mut s2 = *state;
        if s2.apply(a1).is_err() {
            return f64::NEG_INFINITY;
        }
        if s2.decider() as usize != me {
            return fallback;
        }
        self.buf.clear();
        s2.legal_into(&mut self.buf);
        // Proposals are skipped (see the type note); everything else is
        // scored by the same fair one-ply rules, so the leaf values of the
        // second ply are the values the first ply has always produced.
        let ctx = Decision {
            standing: [0.0; carranta_core::state::MAX_PLAYERS],
            asked: 0.0,
        };
        let mut best = f64::NEG_INFINITY;
        let candidates = std::mem::take(&mut self.buf);
        for &a2 in &candidates {
            if matches!(a2, Action::ProposeTrade { .. }) {
                continue;
            }
            let v = self.inner.score(&s2, me, a2, &ctx);
            if v > best {
                best = v;
            }
        }
        self.buf = candidates;
        if best == f64::NEG_INFINITY {
            // Nothing to continue with: the position's own value stands.
            self.inner.value(&s2, me, Pending::default())
        } else {
            best
        }
    }
}

impl Policy for DeepNetPolicy {
    fn choose(&mut self, state: &State, legal: &[Action]) -> Action {
        debug_assert!(!legal.is_empty());
        let me = state.decider() as usize;
        let ctx = Decision {
            standing: core::array::from_fn(|q| {
                if q < state.players as usize {
                    self.inner.value(state, q, Pending::default())
                } else {
                    0.0
                }
            }),
            asked: {
                let mut asked = *state;
                asked.offers_made[me] += 1;
                self.inner.value(&asked, me, Pending::default())
            },
        };
        // Every candidate at one ply first.
        self.scored.clear();
        for (i, &a) in legal.iter().enumerate() {
            self.scored.push((self.inner.score(state, me, a, &ctx), i));
        }
        let mut scored = std::mem::take(&mut self.scored);
        // The strongest expandable candidates go a ply deeper. Sorting the
        // whole list costs nothing next to one network evaluation.
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut expanded = 0;
        for k in 0..scored.len() {
            if expanded >= self.beam {
                break;
            }
            let (one_ply, i) = scored[k];
            if one_ply == f64::NEG_INFINITY || !Self::expandable(&legal[i]) {
                continue;
            }
            scored[k].0 = self.second_ply(state, me, legal[i], one_ply);
            expanded += 1;
        }
        self.scored = Vec::new();
        // Reservoir over exact ties, as the one-ply policy does.
        let mut best = f64::NEG_INFINITY;
        let mut ties = 0u32;
        let mut chosen = legal[0];
        for &(s, i) in &scored {
            if s > best {
                best = s;
                ties = 1;
                chosen = legal[i];
            } else if s == best {
                ties += 1;
                if self.inner.rng.below(Stream::Dice, ties) == 0 {
                    chosen = legal[i];
                }
            }
        }
        self.scored = scored;
        chosen
    }

    fn accepts(&mut self, state: &State, seat: usize, offer: usize) -> bool {
        if !self.deep_market {
            return self.inner.accepts(state, seat, offer);
        }
        // The active seat's accepts go through `choose` and are already deep;
        // this is the passive seat answering somebody's offer. One ply asked
        // "is the swap alone better than standing pat"; the second ply asks
        // what the swap enables, and in an open market one accept can enable
        // another (R-7.19). The guards are the one-ply policy's own.
        let Some(o) = state.live_offers().get(offer) else {
            return false;
        };
        if !state.may_accept(seat, o) || !state.holds(seat, &o.want) {
            return false;
        }
        let take = Action::AcceptTrade {
            offer: offer as u8,
            by: seat as u8,
        };
        let mut s2 = *state;
        if s2.apply(take).is_err() {
            return false;
        }
        let ctx = Decision {
            standing: [0.0; carranta_core::state::MAX_PLAYERS],
            asked: 0.0,
        };
        // Both sides of the question get the same optimism, or the maximum
        // flatters whichever side carries it: the best plan after taking the
        // offer, against the best plan while letting it stand, which includes
        // taking a different one.
        let mut plan = |s: &State, skip: Option<usize>| -> f64 {
            let mut best = self.inner.value(s, seat, Pending::default());
            self.buf.clear();
            s.legal_for(seat as u8, &mut self.buf);
            let candidates = std::mem::take(&mut self.buf);
            for &a2 in &candidates {
                if let Action::AcceptTrade { offer: o2, .. } = a2 {
                    if skip == Some(o2 as usize) {
                        continue;
                    }
                    let v = self.inner.score(s, seat, a2, &ctx);
                    if v > best {
                        best = v;
                    }
                }
            }
            self.buf = candidates;
            best
        };
        plan(&s2, None) > plan(state, Some(offer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Net;
    use crate::{Heuristic, Policy, settle_market};
    use carranta_core::state::{OfferShapes, Phase, TradeMode};

    /// A minimal classic-NEAT starting network: every input wired straight to
    /// the output with small deterministic weights.
    fn minimal_net(seed: u64) -> Net {
        let mut rng = Rng::new(seed);
        let out = Net::output_id(features::FEATURES);
        let links: Vec<(u32, u32, f64)> = (0..=features::FEATURES as u32)
            .map(|i| {
                let w = (rng.below(Stream::Dice, 2001) as f64 - 1000.0) / 1000.0;
                (i, out, w)
            })
            .collect();
        Net::assemble(features::FEATURES, &links).expect("minimal is acyclic")
    }

    /// Drive a full game with the given policies under the training market.
    fn play(seed: u64, policies: &mut [&mut dyn Policy]) -> (Option<u8>, u32) {
        let mut state = State::new(4, seed)
            .with_trade_mode(TradeMode::Full)
            .with_offer_shapes(OfferShapes::Mixed {
                give: Some(2),
                want: 2,
            });
        let mut buf = Vec::new();
        let mut actions = 0u32;
        while actions < 20_000 {
            if matches!(state.phase, Phase::GameOver { .. }) {
                break;
            }
            state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = state.decider() as usize;
            let action = policies[seat].choose(&state, &buf);
            state
                .apply(action)
                .expect("a policy must pick a legal action");
            actions += 1;
            settle_market(&mut state, policies);
        }
        let winner = match state.phase {
            Phase::GameOver { winner } => Some(winner),
            _ => None,
        };
        (winner, actions)
    }

    #[test]
    fn the_deep_policy_plays_legally_and_is_not_the_shallow_one_renamed() {
        // A whole game with a deep seat at the table: every action legal, the
        // game terminating. And the depth has to matter: on the same net and
        // the same positions, two plies must sometimes disagree with one, or
        // the beam is decoration.
        let net = minimal_net(5);
        let mut a = DeepNetPolicy::new(net.clone(), 11);
        let mut b = DeepNetPolicy::flat_market(net.clone(), 12);
        let mut c = NetPolicy::new(net.clone(), 13);
        let mut d = NetPolicy::new(net.clone(), 14);
        let (_, actions) = play(31, &mut [&mut a, &mut b, &mut c, &mut d]);
        assert!(actions > 50, "the game was played out");

        // Disagreement, measured on real positions: walk a game and compare
        // the two policies' picks on every decision. RNG seeds are equal so
        // tie-breaking cannot manufacture a difference.
        let mut deep = DeepNetPolicy::new(net.clone(), 77);
        let mut shallow = NetPolicy::new(net, 77);
        let mut state = State::new(4, 8)
            .with_trade_mode(TradeMode::Full)
            .with_offer_shapes(OfferShapes::Mixed {
                give: Some(2),
                want: 2,
            });
        let mut buf = Vec::new();
        let (mut seen, mut differed) = (0, 0);
        for _ in 0..4_000 {
            if matches!(state.phase, Phase::GameOver { .. }) {
                break;
            }
            state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let deep_pick = deep.choose(&state, &buf);
            let shallow_pick = shallow.choose(&state, &buf);
            seen += 1;
            if deep_pick != shallow_pick {
                differed += 1;
            }
            // The game advances by the shallow pick either way, so both see
            // identical positions throughout.
            state.apply(shallow_pick).expect("legal");
        }
        assert!(seen > 100, "enough decisions to compare: {seen}");
        assert!(
            differed > 0,
            "two plies never disagreed with one across {seen} decisions"
        );
    }

    #[test]
    fn four_networks_play_a_whole_game_of_mixed_offers_legally() {
        // The integration that matters: an evolved policy in the exact market
        // it will train in, every chosen action legal, the game terminating.
        for seed in [1u64, 2, 3] {
            let mut a = NetPolicy::new(minimal_net(10), seed * 31 + 1);
            let mut b = NetPolicy::new(minimal_net(11), seed * 31 + 2);
            let mut c = NetPolicy::new(minimal_net(12), seed * 31 + 3);
            let mut d = NetPolicy::new(minimal_net(13), seed * 31 + 4);
            let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];
            let (_, actions) = play(seed, &mut ps);
            assert!(actions > 50, "a real game happened: {actions} actions");
        }
    }

    #[test]
    fn a_network_seat_is_deterministic() {
        // Same net, same seeds, same game, twice. This is the property every
        // paired trial and every exact resume stands on.
        let run = || {
            let mut a = NetPolicy::new(minimal_net(20), 71);
            let mut b = NetPolicy::new(minimal_net(21), 72);
            let mut c = Heuristic::new(73);
            let mut d = Heuristic::new(74);
            let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];
            play(9, &mut ps)
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn an_empty_network_still_finishes_games() {
        // Generation zero of a minimal start contains genomes whose output is
        // barely connected. They must be terrible, not stuck.
        let net = Net::assemble(features::FEATURES, &[]).expect("no links is a network");
        for seed in [4u64, 5] {
            let mut a = NetPolicy::new(net.clone(), 1);
            let mut b = NetPolicy::new(net.clone(), 2);
            let mut c = NetPolicy::new(net.clone(), 3);
            let mut d = NetPolicy::new(net.clone(), 4);
            let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];
            let (_, actions) = play(seed, &mut ps);
            assert!(actions > 0);
        }
    }
}
