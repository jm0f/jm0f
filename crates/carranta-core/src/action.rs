//! Actions: what may be done, and what doing it changes.
//!
//! `apply` is total on legal actions and a no-op error on illegal ones, so a
//! caller can trust the state after any accepted action. Legality is generated
//! rather than checked where possible (`legal_into`), because a search or a
//! policy network needs the whole legal set every step and generating it is
//! cheaper than filtering a candidate list.

use crate::longest_road::longest_road;
use crate::rng::Stream;
use crate::state::{
    CITY_POOL, DevCard, MAX_GENERATED_OFFER, MAX_OFFERS, MAX_PLAYERS, OFFERS_PER_TURN, Offer,
    Phase, RESOURCES, ROAD_POOL, Resource, SETTLEMENT_POOL, State, TradeMode, WINNING_VP,
};
use crate::topology::{
    HEX_COUNT, edge_bit, edge_endpoint_mask, hex_vertices, iter_edges, iter_vertices, vertex_bit,
};

/// Build costs (§2.8). Indexed by [`Resource`].
pub const ROAD_COST: [u8; 5] = [1, 1, 0, 0, 0];
pub const SETTLEMENT_COST: [u8; 5] = [1, 1, 1, 1, 0];
pub const CITY_COST: [u8; 5] = [0, 0, 0, 2, 3];
pub const DEV_COST: [u8; 5] = [0, 0, 1, 1, 1];

/// Hand size above which a 7 forces a discard (R-6.2).
pub const DISCARD_LIMIT: u32 = 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    /// Setup placement (R-3.7, R-3.8).
    PlaceSettlement(u8),
    PlaceRoad(u8),
    /// Roll both dice (R-5.2).
    Roll,
    /// Return one card toward the half owed on a 7 (R-6.2).
    ///
    /// One card per action rather than a whole split: the action space stays
    /// tiny and uniform, which is what a policy over legal moves needs.
    Discard {
        player: u8,
        resource: Resource,
    },
    /// Move the robber and rob someone standing there (R-6.3, R-6.4).
    MoveRobber {
        hex: u8,
        victim: Option<u8>,
    },
    BuildRoad(u8),
    BuildSettlement(u8),
    BuildCity(u8),
    BuyDev,
    PlayMilitia,
    PlayRoadBuilding,
    PlayInvention([Resource; 2]),
    PlayMonopoly(Resource),
    /// Supply trade at whatever rate the player's ports allow (R-7.6–R-7.9).
    Trade {
        give: Resource,
        take: Resource,
    },
    /// Put an offer on the market (R-7.4, R-7.19).
    ///
    /// Both sides are from the proposer's point of view. Every market action
    /// names its actor, because the market is the one place where that is not
    /// implied by whose turn it is: any seat may propose, and an offer may be
    /// open to several. Every offer still has the active player as one party
    /// (R-7.3), either they made it, or it is addressed to them.
    ProposeTrade {
        by: u8,
        /// Addressed to one seat, or `None` for the open market (R-7.19).
        ///
        /// Generated offers are always open: an addressed one multiplies the
        /// action space by the number of opponents for no gain to a search,
        /// and the enumeration bound exists precisely to keep that space
        /// small. `apply` accepts either, so a human client may address one.
        to: Option<u8>,
        give: [u8; 5],
        want: [u8; 5],
    },
    /// Take a live offer.
    AcceptTrade {
        offer: u8,
        by: u8,
    },
    /// Pull one's own offer back off the market (R-7.14).
    WithdrawTrade {
        offer: u8,
        by: u8,
    },
    EndTurn,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Illegal {
    WrongPhase,
    NotYourTurn,
    Occupied,
    TooClose,
    Disconnected,
    NoPieces,
    CannotAfford,
    NoSuchCard,
    CardBoughtThisTurn,
    AlreadyPlayedDev,
    RobberMustMove,
    BadVictim,
    SupplyEmpty,
    BadDiscard,
    GameOver,
    /// Player trading is switched off for this game (§6.5).
    TradeDisabled,
    /// A side of the offer was empty: a trade is not a gift (R-7.5).
    EmptySide,
    /// A resource appeared on both sides of the offer (R-7.18).
    TypeOverlap,
    /// Every trade needs the active player as one party (R-7.3).
    NotAParty,
    /// This seat has made its allowance of offers this turn (R-7.20).
    OfferLimit,
    /// The market is full.
    MarketFull,
    NoSuchOffer,
    NotYourOffer,
    /// The offer was legal when made but one side can no longer pay (R-7.19).
    OfferStale,
}

type Result = core::result::Result<(), Illegal>;

/// The randomness an action resolved.
///
/// Recording the resolved outcome rather than the seed is what decouples a
/// stored game from any one engine build (§7.1, H-1): replay becomes a fold
/// over data, and a later rules or RNG change fails loudly against a
/// checksum instead of silently reinterpreting history.
///
/// Only two sources are live during play. The board layout and the
/// development deck order are drawn once at [`State::new`] and are carried in
/// the state itself, so a log that stores the opening state needs no events
/// for them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Resolved {
    /// The action resolved nothing random.
    #[default]
    None,
    /// Both dice, in the order the engine produced them (R-5.2).
    Dice(u8, u8),
    /// A robbery took this card; `None` when the victim held nothing (R-6.4).
    Steal(Option<Resource>),
}

/// Where an action's randomness comes from.
#[derive(Clone, Copy)]
enum Source {
    /// Draw from the state's own generator.
    Live,
    /// Replay: take the recorded outcome and leave the generator alone.
    Script(Resolved),
}

#[inline]
fn can_pay(hand: &[u8; 5], cost: &[u8; 5]) -> bool {
    (0..5).all(|i| hand[i] >= cost[i])
}

#[inline]
fn pay(state: &mut State, p: usize, cost: &[u8; 5]) {
    for (i, &c) in cost.iter().enumerate() {
        state.hand[p][i] -= c;
        state.supply[i] += c;
    }
}

impl State {
    /// Which seat's decision the current legal actions belong to.
    ///
    /// Normally the player to act, but a 7 interrupts: every seat over the
    /// limit owes a discard, and they are resolved lowest seat first. Exposing
    /// this keeps the action stream single-agent at every step, which is what
    /// both a policy and a search need.
    #[inline]
    pub fn decider(&self) -> u8 {
        match self.phase {
            Phase::Discard => (0..self.players)
                .find(|&q| self.discard_left[q as usize] > 0)
                .unwrap_or(self.to_act),
            _ => self.to_act,
        }
    }

    /// Collect every legal action into `out`, which is cleared first.
    ///
    /// `out` is caller-owned so a rollout loop reuses one buffer and allocates
    /// nothing after the first turn.
    pub fn legal_into(&self, out: &mut Vec<Action>) {
        out.clear();
        let p = self.to_act as usize;
        match self.phase {
            Phase::GameOver { .. } => {}
            Phase::SetupSettlement { .. } => {
                for v in iter_vertices(self.settlement_spots(p, true)) {
                    out.push(Action::PlaceSettlement(v));
                }
            }
            Phase::SetupRoad { from, .. } => {
                // The second road must adjoin the settlement just placed
                // (R-3.7); anything else would leave it stranded.
                let free = crate::topology::edges_at(from) & !self.all_roads();
                for e in iter_edges(free) {
                    out.push(Action::PlaceRoad(e));
                }
            }
            Phase::PreRoll => {
                out.push(Action::Roll);
                self.push_dev_plays(p, out);
            }
            Phase::Discard => {
                // One seat at a time, so the stream stays single-agent.
                let q = self.decider() as usize;
                for r in RESOURCES {
                    if self.hand[q][r as usize] > 0 {
                        out.push(Action::Discard {
                            player: q as u8,
                            resource: r,
                        });
                    }
                }
            }
            Phase::MoveRobber { .. } => {
                for h in 0..HEX_COUNT as u8 {
                    if h == self.robber {
                        continue; // must move somewhere new (R-6.3)
                    }
                    let mut any = false;
                    for q in 0..self.players as usize {
                        if q != p && self.buildings(q) & hex_vertices(h) != 0 {
                            out.push(Action::MoveRobber {
                                hex: h,
                                victim: Some(q as u8),
                            });
                            any = true;
                        }
                    }
                    if !any {
                        out.push(Action::MoveRobber {
                            hex: h,
                            victim: None,
                        });
                    }
                }
            }
            Phase::Action => {
                out.push(Action::EndTurn);
                self.push_dev_plays(p, out);
                self.push_market(p, out);

                let free = self.free_roads > 0;
                if self.roads_left[p] > 0 && (free || can_pay(&self.hand[p], &ROAD_COST)) {
                    for e in iter_edges(self.road_spots(p)) {
                        out.push(Action::BuildRoad(e));
                    }
                }
                if self.settlements_left[p] > 0 && can_pay(&self.hand[p], &SETTLEMENT_COST) {
                    for v in iter_vertices(self.settlement_spots(p, false)) {
                        out.push(Action::BuildSettlement(v));
                    }
                }
                if self.cities_left[p] > 0 && can_pay(&self.hand[p], &CITY_COST) {
                    for v in iter_vertices(self.settlements[p]) {
                        out.push(Action::BuildCity(v));
                    }
                }
                if (self.dev_drawn as usize) < self.dev_deck.len()
                    && can_pay(&self.hand[p], &DEV_COST)
                {
                    out.push(Action::BuyDev);
                }
                for give in RESOURCES {
                    let rate = self.trade_rate(p, give);
                    if self.hand[p][give as usize] < rate {
                        continue;
                    }
                    for take in RESOURCES {
                        // The taken resource must differ (R-7.6) and the stack
                        // must be able to pay in full (R-7.17).
                        if take != give && self.supply[take as usize] > 0 {
                            out.push(Action::Trade { give, take });
                        }
                    }
                }
            }
        }
    }

    fn push_dev_plays(&self, p: usize, out: &mut Vec<Action>) {
        if self.dev_played_this_turn || self.free_roads > 0 {
            return; // one per turn (R-9.3)
        }
        if self.dev_playable(p, DevCard::Militia) > 0 {
            out.push(Action::PlayMilitia);
        }
        // Only the militia may be played before the roll (R-9.5). It is the
        // one card whose timing changes anything: moving the robber before
        // production decides which hexes pay this turn. The other three do the
        // same thing on either side of the dice, so offering them early was a
        // choice with no consequence, sitting in front of every roll.
        if self.phase == Phase::PreRoll {
            return;
        }
        if self.dev_playable(p, DevCard::RoadBuilding) > 0 {
            out.push(Action::PlayRoadBuilding);
        }
        if self.dev_playable(p, DevCard::Monopoly) > 0 {
            for r in RESOURCES {
                out.push(Action::PlayMonopoly(r));
            }
        }
        if self.dev_playable(p, DevCard::Invention) > 0 {
            for a in RESOURCES {
                for b in RESOURCES {
                    if (b as usize) >= (a as usize) {
                        out.push(Action::PlayInvention([a, b]));
                    }
                }
            }
        }
    }

    /// Apply an action, or report why it is not allowed.
    ///
    /// The state is left untouched when an action is rejected.
    pub fn apply(&mut self, action: Action) -> Result {
        let mut out = Resolved::None;
        self.apply_inner(action, Source::Live, &mut out)
    }

    /// Apply an action, reporting the randomness it resolved (§7.1, H-1).
    ///
    /// The recording counterpart of [`State::apply`]: same behaviour, but the
    /// dice and the stolen card come back so they can be written to a log.
    pub fn apply_recorded(&mut self, action: Action) -> core::result::Result<Resolved, Illegal> {
        let mut out = Resolved::None;
        self.apply_inner(action, Source::Live, &mut out)?;
        Ok(out)
    }

    /// Apply an action with its randomness supplied rather than drawn.
    ///
    /// The replay counterpart of [`State::apply_recorded`]. The state's
    /// generator is not advanced, so a replayed state matches the recorded one
    /// in every field but [`State::rng`], which is why replay comparisons go
    /// through [`State::same_game_as`].
    ///
    /// A `scripted` value that does not fit the action is ignored, and the
    /// action resolves live instead; callers that need this checked should
    /// compare the returned outcome against what they supplied.
    pub fn apply_scripted(
        &mut self,
        action: Action,
        scripted: Resolved,
    ) -> core::result::Result<Resolved, Illegal> {
        let mut out = Resolved::None;
        self.apply_inner(action, Source::Script(scripted), &mut out)?;
        Ok(out)
    }

    fn apply_inner(&mut self, action: Action, src: Source, out: &mut Resolved) -> Result {
        let p = self.to_act as usize;
        match (self.phase, action) {
            (Phase::GameOver { .. }, _) => Err(Illegal::GameOver),

            (Phase::SetupSettlement { round }, Action::PlaceSettlement(v)) => {
                if self.settlement_spots(p, true) & vertex_bit(v) == 0 {
                    return Err(Illegal::TooClose);
                }
                self.settlements[p] |= vertex_bit(v);
                self.settlements_left[p] -= 1;
                // The second settlement pays out immediately (R-3.10).
                if round == 1 {
                    for h in 0..HEX_COUNT as u8 {
                        if hex_vertices(h) & vertex_bit(v) == 0 {
                            continue;
                        }
                        if let Some(r) = self.terrain[h as usize].yields() {
                            self.hand[p][r as usize] += 1;
                            self.supply[r as usize] -= 1;
                        }
                    }
                }
                self.phase = Phase::SetupRoad { round, from: v };
                Ok(())
            }

            (Phase::SetupRoad { round, from }, Action::PlaceRoad(e)) => {
                if self.all_roads() & edge_bit(e) != 0 {
                    return Err(Illegal::Occupied);
                }
                if edge_endpoint_mask(e) & vertex_bit(from) == 0 {
                    return Err(Illegal::Disconnected);
                }
                self.roads[p] |= edge_bit(e);
                self.roads_left[p] -= 1;
                self.advance_setup(round);
                Ok(())
            }

            (Phase::PreRoll, Action::Roll) => {
                let (a, b) = match src {
                    Source::Script(Resolved::Dice(a, b)) => (a, b),
                    _ => {
                        let a = self.rng.die();
                        let b = self.rng.die();
                        (a, b)
                    }
                };
                *out = Resolved::Dice(a, b);
                self.dice = [a, b];
                if a + b == 7 {
                    self.begin_seven();
                } else {
                    self.distribute(a + b);
                    self.phase = Phase::Action;
                }
                Ok(())
            }

            (Phase::Discard, Action::Discard { player, resource }) => {
                let q = player as usize;
                if q >= self.players as usize || self.discard_left[q] == 0 {
                    return Err(Illegal::BadDiscard);
                }
                if self.hand[q][resource as usize] == 0 {
                    return Err(Illegal::BadDiscard);
                }
                self.hand[q][resource as usize] -= 1;
                self.supply[resource as usize] += 1;
                self.discard_left[q] -= 1;
                if self.discard_left[..self.players as usize]
                    .iter()
                    .all(|&n| n == 0)
                {
                    self.phase = Phase::MoveRobber {
                        from_militia: false,
                    };
                }
                Ok(())
            }

            (Phase::MoveRobber { from_militia }, Action::MoveRobber { hex, victim }) => {
                if hex == self.robber || hex as usize >= HEX_COUNT {
                    return Err(Illegal::RobberMustMove);
                }
                self.robber = hex;
                if let Some(v) = victim {
                    let q = v as usize;
                    // Only an opponent standing on the hex may be robbed
                    // (R-6.4).
                    if q == p || q >= self.players as usize {
                        return Err(Illegal::BadVictim);
                    }
                    if self.buildings(q) & hex_vertices(hex) == 0 {
                        return Err(Illegal::BadVictim);
                    }
                    *out = Resolved::Steal(self.steal(p, q, src));
                }
                // A Militia played before the roll returns to the pre-roll
                // phase; a rolled 7 has already produced, so play continues.
                self.phase = if from_militia && self.dice == [0, 0] {
                    Phase::PreRoll
                } else {
                    Phase::Action
                };
                self.check_victory();
                Ok(())
            }

            (Phase::Action, Action::BuildRoad(e)) => {
                if self.roads_left[p] == 0 {
                    return Err(Illegal::NoPieces);
                }
                if self.road_spots(p) & edge_bit(e) == 0 {
                    return Err(Illegal::Disconnected);
                }
                if self.free_roads > 0 {
                    self.free_roads -= 1;
                } else if can_pay(&self.hand[p], &ROAD_COST) {
                    pay(self, p, &ROAD_COST);
                } else {
                    return Err(Illegal::CannotAfford);
                }
                self.roads[p] |= edge_bit(e);
                self.roads_left[p] -= 1;
                self.update_longest_road();
                self.check_victory();
                Ok(())
            }

            (Phase::Action, Action::BuildSettlement(v)) => {
                if self.settlements_left[p] == 0 {
                    return Err(Illegal::NoPieces);
                }
                if self.settlement_spots(p, false) & vertex_bit(v) == 0 {
                    return Err(Illegal::TooClose);
                }
                if !can_pay(&self.hand[p], &SETTLEMENT_COST) {
                    return Err(Illegal::CannotAfford);
                }
                pay(self, p, &SETTLEMENT_COST);
                self.settlements[p] |= vertex_bit(v);
                self.settlements_left[p] -= 1;
                // A new settlement can cut an opponent's route (R-10.4).
                self.update_longest_road();
                self.check_victory();
                Ok(())
            }

            (Phase::Action, Action::BuildCity(v)) => {
                if self.cities_left[p] == 0 {
                    return Err(Illegal::NoPieces);
                }
                if self.settlements[p] & vertex_bit(v) == 0 {
                    return Err(Illegal::Occupied);
                }
                if !can_pay(&self.hand[p], &CITY_COST) {
                    return Err(Illegal::CannotAfford);
                }
                pay(self, p, &CITY_COST);
                // A city replaces the settlement, which returns to the pool
                // and may be rebuilt later (R-8.7).
                self.settlements[p] &= !vertex_bit(v);
                self.settlements_left[p] += 1;
                self.cities[p] |= vertex_bit(v);
                self.cities_left[p] -= 1;
                self.check_victory();
                Ok(())
            }

            (Phase::Action, Action::BuyDev) => {
                if self.dev_drawn as usize >= self.dev_deck.len() {
                    return Err(Illegal::SupplyEmpty);
                }
                if !can_pay(&self.hand[p], &DEV_COST) {
                    return Err(Illegal::CannotAfford);
                }
                pay(self, p, &DEV_COST);
                let card = self.dev_deck[self.dev_drawn as usize];
                self.dev_drawn += 1;
                self.dev_held[p][card as usize] += 1;
                self.dev_fresh[p][card as usize] += 1;
                // A Victory Point card can win the game the turn it is bought
                // (R-9.12).
                self.check_victory();
                Ok(())
            }

            (Phase::PreRoll | Phase::Action, Action::PlayMilitia) => {
                self.take_dev(p, DevCard::Militia)?;
                self.militia_played[p] += 1;
                self.update_largest_militia();
                self.phase = Phase::MoveRobber { from_militia: true };
                self.check_victory();
                Ok(())
            }

            (Phase::Action, Action::PlayRoadBuilding) => {
                self.take_dev(p, DevCard::RoadBuilding)?;
                self.free_roads = 2;
                Ok(())
            }

            (Phase::Action, Action::PlayInvention(pick)) => {
                self.take_dev(p, DevCard::Invention)?;
                // Takes as many as remain if a stack runs short (R-7.17).
                for r in pick {
                    if self.supply[r as usize] > 0 {
                        self.supply[r as usize] -= 1;
                        self.hand[p][r as usize] += 1;
                    }
                }
                Ok(())
            }

            (Phase::Action, Action::PlayMonopoly(r)) => {
                self.take_dev(p, DevCard::Monopoly)?;
                for q in 0..self.players as usize {
                    if q == p {
                        continue;
                    }
                    let n = self.hand[q][r as usize];
                    self.hand[q][r as usize] = 0;
                    self.hand[p][r as usize] += n;
                }
                Ok(())
            }

            (Phase::Action, Action::Trade { give, take }) => {
                if give == take {
                    return Err(Illegal::CannotAfford);
                }
                let rate = self.trade_rate(p, give);
                if self.hand[p][give as usize] < rate {
                    return Err(Illegal::CannotAfford);
                }
                if self.supply[take as usize] == 0 {
                    return Err(Illegal::SupplyEmpty);
                }
                self.hand[p][give as usize] -= rate;
                self.supply[give as usize] += rate;
                self.supply[take as usize] -= 1;
                self.hand[p][take as usize] += 1;
                Ok(())
            }

            (Phase::Action, Action::ProposeTrade { by, to, give, want }) => {
                let from = by as usize;
                if from >= self.players as usize {
                    return Err(Illegal::NotAParty);
                }
                self.check_offer(from, to, &give, &want)?;
                let i = self.offer_count as usize;
                self.offers[i] = Offer {
                    from: from as u8,
                    to,
                    give,
                    want,
                };
                self.offer_count += 1;
                self.offers_made[from] += 1;
                Ok(())
            }

            (Phase::Action, Action::AcceptTrade { offer: i, by }) => {
                let idx = i as usize;
                if idx >= self.offer_count as usize {
                    return Err(Illegal::NoSuchOffer);
                }
                let offer = self.offers[idx];
                let taker = by as usize;
                if taker >= self.players as usize || !self.may_accept(taker, &offer) {
                    return Err(Illegal::NotAParty);
                }
                // Re-validate both sides at execution, never at proposal
                // (R-7.19): an intervening build or Monopoly can have emptied
                // a hand since the offer was made.
                let from = offer.from as usize;
                if !self.holds(from, &offer.give) || !self.holds(taker, &offer.want) {
                    self.drop_offer(idx);
                    return Err(Illegal::OfferStale);
                }
                for r in 0..5 {
                    self.hand[from][r] -= offer.give[r];
                    self.hand[taker][r] += offer.give[r];
                    self.hand[taker][r] -= offer.want[r];
                    self.hand[from][r] += offer.want[r];
                }
                self.drop_offer(idx);
                Ok(())
            }

            (Phase::Action, Action::WithdrawTrade { offer: i, by }) => {
                let idx = i as usize;
                if idx >= self.offer_count as usize {
                    return Err(Illegal::NoSuchOffer);
                }
                if self.offers[idx].from != by {
                    return Err(Illegal::NotYourOffer);
                }
                self.drop_offer(idx);
                Ok(())
            }

            (Phase::Action, Action::EndTurn) => {
                self.end_turn();
                Ok(())
            }

            (Phase::SetupSettlement { .. } | Phase::SetupRoad { .. }, _) => {
                Err(Illegal::WrongPhase)
            }
            _ => Err(Illegal::WrongPhase),
        }
    }

    /// Everything `seat` may do right now, whoever is to act.
    ///
    /// [`Self::legal_into`] answers for the seat whose decision it is, which is
    /// what a search or a policy consumes. The open market breaks that
    /// single-agent shape. An opponent may propose or accept at any point in
    /// the active player's turn, so the live game asks this per connected
    /// seat instead.
    pub fn legal_for(&self, seat: u8, out: &mut Vec<Action>) {
        if seat == self.decider() {
            self.legal_into(out);
            return;
        }
        out.clear();
        if self.phase != Phase::Action || self.trade_mode == TradeMode::Disabled {
            return;
        }
        self.push_market(seat as usize, out);
    }

    /// Offers and responses available to `p`.
    ///
    /// Generated proposals put a **single resource type on each side**, up to
    /// [`MAX_GENERATED_OFFER`] cards. Mixed-type offers are legal and can be
    /// accepted, but they cannot be enumerated: multisets of size 1..=3 drawn
    /// from five resources give 55 possibilities a side, so the cross product
    /// is about 3 000 candidates per decision. Single-type sides reduce that to
    /// at most 180 before affordability prunes it, and cover the offers real
    /// play actually makes.
    fn push_market(&self, p: usize, out: &mut Vec<Action>) {
        if self.trade_mode == TradeMode::Disabled {
            return;
        }
        for (i, o) in self.live_offers().iter().enumerate() {
            if o.from as usize == p {
                out.push(Action::WithdrawTrade {
                    offer: i as u8,
                    by: p as u8,
                });
            } else if self.may_accept(p, o) && self.holds(p, &o.want) {
                out.push(Action::AcceptTrade {
                    offer: i as u8,
                    by: p as u8,
                });
            }
        }
        if self.offers_made[p] >= OFFERS_PER_TURN || self.offer_count as usize >= MAX_OFFERS {
            return;
        }
        let max = if self.trade_mode == TradeMode::Restricted {
            1
        } else {
            MAX_GENERATED_OFFER
        };
        for give in RESOURCES {
            let held = self.hand[p][give as usize].min(max);
            for gn in 1..=held {
                for want in RESOURCES {
                    if want == give {
                        continue; // R-7.18
                    }
                    for wn in 1..=max {
                        let mut g = [0u8; 5];
                        let mut w = [0u8; 5];
                        g[give as usize] = gn;
                        w[want as usize] = wn;
                        out.push(Action::ProposeTrade {
                            by: p as u8,
                            // Generated offers are open: see the field's note.
                            to: None,
                            give: g,
                            want: w,
                        });
                    }
                }
            }
        }
    }

    /// May `p` take this offer? Every trade needs the active player as one
    /// party (R-7.3), and nobody trades with themselves.
    #[inline]
    /// Whether `p` is one of the two parties an offer was put to.
    ///
    /// Public because it is the question "was this seat asked?", which a
    /// display needs and cannot answer without restating R-7.3. It reads
    /// nothing but seat numbers and the offer, so a caller learns nothing
    /// about anybody's hand from it. Whether they can *cover* it is a
    /// different question, and that one is private.
    pub fn may_accept(&self, p: usize, offer: &Offer) -> bool {
        let active = self.to_act as usize;
        let from = offer.from as usize;
        if p == from {
            return false;
        }
        // An addressed offer is for that seat alone, and R-7.3 still requires
        // the active player to be one of the two parties.
        if let Some(to) = offer.to {
            return p == to as usize && (from == active || p == active);
        }
        if from == active {
            true // the active player's offer is open to any opponent
        } else {
            p == active // an opponent's offer is addressed to the active player
        }
    }

    fn check_offer(&self, p: usize, to: Option<u8>, give: &[u8; 5], want: &[u8; 5]) -> Result {
        if self.trade_mode == TradeMode::Disabled {
            return Err(Illegal::TradeDisabled);
        }
        if let Some(t) = to {
            let t = t as usize;
            // Addressing yourself is not a trade, and R-7.3 turn-gates the
            // pair: one of the two parties must be the active player.
            if t == p || t >= self.players as usize {
                return Err(Illegal::NotAParty);
            }
            if p != self.to_act as usize && t != self.to_act as usize {
                return Err(Illegal::NotAParty);
            }
        }
        // A trade must give and take (R-7.5).
        let (gn, wn): (u32, u32) = (
            give.iter().map(|&n| n as u32).sum(),
            want.iter().map(|&n| n as u32).sum(),
        );
        if gn == 0 || wn == 0 {
            return Err(Illegal::EmptySide);
        }
        // No resource on both sides (R-7.18).
        if (0..5).any(|r| give[r] > 0 && want[r] > 0) {
            return Err(Illegal::TypeOverlap);
        }
        if self.trade_mode == TradeMode::Restricted && (gn != 1 || wn != 1) {
            return Err(Illegal::TradeDisabled);
        }
        if !self.holds(p, give) {
            return Err(Illegal::CannotAfford);
        }
        if self.offers_made[p] >= OFFERS_PER_TURN {
            return Err(Illegal::OfferLimit);
        }
        if self.offer_count as usize >= MAX_OFFERS {
            return Err(Illegal::MarketFull);
        }
        Ok(())
    }

    fn take_dev(&mut self, p: usize, card: DevCard) -> Result {
        if self.dev_played_this_turn || self.free_roads > 0 {
            return Err(Illegal::AlreadyPlayedDev);
        }
        if self.dev_playable(p, card) == 0 {
            return if self.dev_held[p][card as usize] > 0 {
                Err(Illegal::CardBoughtThisTurn)
            } else {
                Err(Illegal::NoSuchCard)
            };
        }
        // Played cards never return to the deck (R-9.6); they simply leave the
        // hand, and Militia are counted separately for Largest Militia.
        self.dev_held[p][card as usize] -= 1;
        self.dev_played_this_turn = true;
        Ok(())
    }

    /// Setup runs forward through the seats, then back (R-3.7, R-3.8).
    fn advance_setup(&mut self, round: u8) {
        let last = self.players - 1;
        if round == 0 {
            if self.to_act == last {
                self.phase = Phase::SetupSettlement { round: 1 }; // same seat starts back
            } else {
                self.to_act += 1;
                self.phase = Phase::SetupSettlement { round: 0 };
            }
        } else if self.to_act == 0 {
            self.phase = Phase::PreRoll;
        } else {
            self.to_act -= 1;
            self.phase = Phase::SetupSettlement { round: 1 };
        }
    }

    fn begin_seven(&mut self) {
        self.discard_left = [0; MAX_PLAYERS];
        let mut any = false;
        for q in 0..self.players as usize {
            let n = self.hand_size(q);
            if n > DISCARD_LIMIT {
                // Half, rounded down (R-6.2).
                self.discard_left[q] = (n / 2) as u8;
                any = true;
            }
        }
        self.phase = if any {
            Phase::Discard
        } else {
            Phase::MoveRobber {
                from_militia: false,
            }
        };
    }

    /// Pay out a roll, applying the shortage rule (R-5.6).
    fn distribute(&mut self, roll: u8) {
        let owed = self.production(roll);
        let seats = self.players as usize;
        for r in 0..5 {
            let total: u32 = owed[..seats].iter().map(|o| o[r] as u32).sum();
            if total == 0 {
                continue;
            }
            if total <= self.supply[r] as u32 {
                for (p, o) in owed[..seats].iter().enumerate() {
                    self.hand[p][r] += o[r];
                    self.supply[r] -= o[r];
                }
                continue;
            }
            // Not enough to go round: nobody gets any, unless exactly one
            // player is owed, who takes what is left.
            let mut claimants = owed[..seats].iter().enumerate().filter(|(_, o)| o[r] > 0);
            if let (Some((p, _)), None) = (claimants.next(), claimants.next()) {
                let give = self.supply[r];
                self.hand[p][r] += give;
                self.supply[r] -= give;
            }
        }
    }

    /// Take one card at random from `victim`, returning what was taken.
    fn steal(&mut self, thief: usize, victim: usize, src: Source) -> Option<Resource> {
        let n = self.hand_size(victim);
        if n == 0 {
            return None; // nothing to take, and no second choice of victim (R-6.4)
        }
        // A scripted card the victim does not hold would corrupt the hand, so
        // it falls back to a live draw rather than going through.
        if let Source::Script(Resolved::Steal(Some(r))) = src
            && self.hand[victim][r as usize] > 0
        {
            self.hand[victim][r as usize] -= 1;
            self.hand[thief][r as usize] += 1;
            return Some(r);
        }
        let mut pick = self.rng.below(Stream::Steal, n);
        for (r, &res) in RESOURCES.iter().enumerate() {
            let have = self.hand[victim][r] as u32;
            if pick < have {
                self.hand[victim][r] -= 1;
                self.hand[thief][r] += 1;
                return Some(res);
            }
            pick -= have;
        }
        unreachable!("the pick is always inside the hand");
    }

    /// Recompute who holds Longest Road (R-10.1 … R-10.6).
    pub fn update_longest_road(&mut self) {
        let mut len = [0u32; MAX_PLAYERS];
        for (p, slot) in len.iter_mut().enumerate().take(self.players as usize) {
            *slot = longest_road(self.roads[p], self.blocking(p));
        }
        let best = len[..self.players as usize]
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        if best < 5 {
            self.longest_road = None;
            return;
        }
        // The holder keeps it while still level with the best; a rival needs
        // strictly more (R-10.6).
        if self.longest_road.is_some_and(|h| len[h as usize] == best) {
            return;
        }
        let leaders: Vec<usize> = (0..self.players as usize)
            .filter(|&p| len[p] == best)
            .collect();
        self.longest_road = if leaders.len() == 1 {
            Some(leaders[0] as u8)
        } else {
            None // tied, so it sits unclaimed (R-10.5)
        };
    }

    /// Recompute who holds Largest Militia (R-10.8).
    fn update_largest_militia(&mut self) {
        let best = self.militia_played[..self.players as usize]
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        if best < 3 {
            return;
        }
        if self
            .largest_militia
            .is_some_and(|h| self.militia_played[h as usize] >= best)
        {
            return;
        }
        let leaders: Vec<usize> = (0..self.players as usize)
            .filter(|&p| self.militia_played[p] == best)
            .collect();
        if leaders.len() == 1 {
            self.largest_militia = Some(leaders[0] as u8);
        }
    }

    /// Victory is checked, and claimed, only on the winner's own turn (R-11.1).
    fn check_victory(&mut self) {
        let p = self.to_act as usize;
        if self.victory_points(p) >= WINNING_VP {
            self.phase = Phase::GameOver { winner: p as u8 };
        }
    }

    fn drop_offer(&mut self, idx: usize) {
        let last = self.offer_count as usize - 1;
        self.offers[idx] = self.offers[last];
        self.offers[last] = Offer::default();
        self.offer_count -= 1;
    }

    fn end_turn(&mut self) {
        let p = self.to_act as usize;
        // Offers do not survive the turn they were made in.
        self.offer_count = 0;
        self.offers = [Offer::default(); MAX_OFFERS];
        self.offers_made = [0; MAX_PLAYERS];
        self.dev_fresh[p] = [0; 5];
        self.dev_played_this_turn = false;
        self.free_roads = 0;
        self.to_act = (self.to_act + 1) % self.players;
        self.phase = Phase::PreRoll;
    }

    /// Structural checks that must hold after every action (§5.5).
    ///
    /// Debug-only: these are invariants of correct play, not conditions to
    /// test for at runtime.
    pub fn assert_invariants(&self) {
        for r in 0..5 {
            let held: u32 = (0..self.players as usize)
                .map(|p| self.hand[p][r] as u32)
                .sum();
            assert_eq!(
                held + self.supply[r] as u32,
                crate::state::SUPPLY_PER_RESOURCE as u32,
                "resource {r} leaked"
            );
        }
        let mut dev_out = 0u32;
        for p in 0..self.players as usize {
            dev_out += self.dev_count(p) + self.militia_played[p] as u32;
        }
        // Played non-Militia cards are gone from every count, so the deck can
        // only ever be ahead of what is visible.
        assert!(dev_out <= self.dev_drawn as u32, "development cards leaked");

        for p in 0..self.players as usize {
            assert_eq!(
                self.roads[p].count_ones() + self.roads_left[p] as u32,
                ROAD_POOL as u32
            );
            assert_eq!(
                self.settlements[p].count_ones() + self.settlements_left[p] as u32,
                SETTLEMENT_POOL as u32
            );
            assert_eq!(
                self.cities[p].count_ones() + self.cities_left[p] as u32,
                CITY_POOL as u32
            );
        }

        // At most one owner per edge and per intersection.
        let mut seen_e = 0u128;
        let mut seen_v = 0u64;
        for p in 0..self.players as usize {
            assert_eq!(seen_e & self.roads[p], 0, "two roads on one edge");
            seen_e |= self.roads[p];
            let b = self.buildings(p);
            assert_eq!(seen_v & b, 0, "two buildings on one intersection");
            assert_eq!(self.settlements[p] & self.cities[p], 0);
            seen_v |= b;
        }

        // The Distance Rule holds continuously (R-8.5).
        for v in iter_vertices(seen_v) {
            for e in iter_edges(crate::topology::edges_at(v)) {
                let w = crate::topology::edge_other(e, v);
                assert_eq!(seen_v & vertex_bit(w), 0, "buildings adjacent at {v},{w}");
            }
        }

        assert!((self.robber as usize) < HEX_COUNT);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::state::{DEV_DECK_SIZE, Terrain};

    /// Play a game to its end with uniformly random legal actions.
    ///
    /// Returns the number of actions applied. Invariants are checked after
    /// every one, which is what makes this the most valuable test here: it
    /// exercises rule interactions no hand-written case would think to reach.
    fn playout(seed: u64, players: u8, cap: usize) -> (State, usize) {
        let mut s = State::new(players, seed);
        let mut rng = Rng::new(seed ^ 0xD1CE);
        let mut buf = Vec::new();
        let mut steps = 0;
        s.assert_invariants();
        while !matches!(s.phase, Phase::GameOver { .. }) && steps < cap {
            s.legal_into(&mut buf);
            assert!(!buf.is_empty(), "no legal action in {:?}", s.phase);
            let pick = rng.below(crate::rng::Stream::Dice, buf.len() as u32) as usize;
            let a = buf[pick];
            s.apply(a).unwrap_or_else(|e| {
                panic!("generated action {a:?} rejected as {e:?} in {:?}", s.phase)
            });
            s.assert_invariants();
            steps += 1;
        }
        (s, steps)
    }

    #[test]
    fn random_games_stay_consistent_and_finish() {
        let mut finished = 0;
        for seed in 0..300 {
            let players = 3 + (seed % 2) as u8;
            let (s, steps) = playout(seed, players, 20_000);
            if let Phase::GameOver { winner } = s.phase {
                finished += 1;
                assert!(s.victory_points(winner as usize) >= WINNING_VP);
                assert!(steps > 20, "a game cannot end in {steps} actions");
            }
        }
        // Random play is poor, but it should still reach 10 VP most of the
        // time inside the cap; a collapse here means something is stuck.
        assert!(finished > 250, "only {finished}/300 games finished");
    }

    #[test]
    fn every_generated_action_is_accepted() {
        // Legality generation and legality checking must agree exactly: a
        // policy is only ever offered generated actions.
        for seed in 300..340 {
            let mut s = State::new(4, seed);
            let mut rng = Rng::new(seed);
            let mut buf = Vec::new();
            for _ in 0..400 {
                if matches!(s.phase, Phase::GameOver { .. }) {
                    break;
                }
                s.legal_into(&mut buf);
                for &a in &buf {
                    let mut probe = s;
                    assert!(probe.apply(a).is_ok(), "{a:?} generated but rejected");
                }
                let pick = rng.below(crate::rng::Stream::Dice, buf.len() as u32) as usize;
                let a = buf[pick];
                s.apply(a).unwrap();
            }
        }
    }

    #[test]
    fn setup_places_two_settlements_each_in_snake_order() {
        let mut s = State::new(4, 1);
        let mut buf = Vec::new();
        let mut order = Vec::new();
        while matches!(
            s.phase,
            Phase::SetupSettlement { .. } | Phase::SetupRoad { .. }
        ) {
            if matches!(s.phase, Phase::SetupSettlement { .. }) {
                order.push(s.to_act);
            }
            s.legal_into(&mut buf);
            s.apply(buf[0]).unwrap();
        }
        assert_eq!(order, vec![0, 1, 2, 3, 3, 2, 1, 0], "snake order (R-3.8)");
        for p in 0..4 {
            assert_eq!(s.settlements[p].count_ones(), 2);
            assert_eq!(s.roads[p].count_ones(), 2);
        }
        assert_eq!(s.phase, Phase::PreRoll);
    }

    #[test]
    fn only_the_second_settlement_pays_out() {
        let mut s = State::new(3, 2);
        let mut buf = Vec::new();
        // Round one: nobody collects.
        while matches!(
            s.phase,
            Phase::SetupSettlement { round: 0 } | Phase::SetupRoad { round: 0, .. }
        ) {
            s.legal_into(&mut buf);
            s.apply(buf[0]).unwrap();
        }
        for p in 0..3 {
            assert_eq!(s.hand_size(p), 0, "round one pays nothing (R-3.10)");
        }
        while matches!(
            s.phase,
            Phase::SetupSettlement { .. } | Phase::SetupRoad { .. }
        ) {
            s.legal_into(&mut buf);
            s.apply(buf[0]).unwrap();
        }
        let total: u32 = (0..3).map(|p| s.hand_size(p)).sum();
        assert!(total > 0, "second settlements pay out");
    }

    #[test]
    fn a_city_returns_its_settlement_to_the_pool() {
        let mut s = State::new(3, 3);
        s.phase = Phase::Action;
        let v = 10u8;
        s.settlements[0] |= vertex_bit(v);
        s.settlements_left[0] -= 1;
        s.hand[0] = [0, 0, 0, 2, 3];
        s.supply = [19, 19, 19, 17, 16];

        s.apply(Action::BuildCity(v)).unwrap();
        assert_eq!(s.cities[0] & vertex_bit(v), vertex_bit(v));
        assert_eq!(s.settlements[0] & vertex_bit(v), 0);
        assert_eq!(s.settlements_left[0], SETTLEMENT_POOL, "returned to pool");
        assert_eq!(s.cities_left[0], CITY_POOL - 1);
        s.assert_invariants();
    }

    #[test]
    fn only_the_militia_may_be_played_before_the_roll() {
        // R-9.5. Hold one of everything, bought last turn so nothing is fresh,
        // and ask what is on offer before the dice.
        let mut s = State::new(3, 9);
        s.phase = Phase::PreRoll;
        for card in [
            DevCard::Militia,
            DevCard::RoadBuilding,
            DevCard::Monopoly,
            DevCard::Invention,
        ] {
            s.dev_held[0][card as usize] = 1;
            s.dev_fresh[0][card as usize] = 0;
        }

        let mut buf = Vec::new();
        s.legal_into(&mut buf);
        assert!(
            buf.contains(&Action::PlayMilitia),
            "the militia is the one card the roll's timing matters to"
        );
        for a in [
            Action::PlayRoadBuilding,
            Action::PlayMonopoly(Resource::Ore),
            Action::PlayInvention([Resource::Wood, Resource::Ore]),
        ] {
            assert!(!buf.contains(&a), "{a:?} was offered before the roll");
            // Not merely unlisted: refused if asked for directly, or the rule
            // would hold only for a caller that reads the list first.
            assert_eq!(
                s.clone().apply(a),
                Err(Illegal::WrongPhase),
                "{a:?} was allowed before the roll"
            );
        }

        // All four are back once the dice have been thrown.
        s.phase = Phase::Action;
        buf.clear();
        s.legal_into(&mut buf);
        for a in [
            Action::PlayMilitia,
            Action::PlayRoadBuilding,
            Action::PlayMonopoly(Resource::Ore),
            Action::PlayInvention([Resource::Wood, Resource::Ore]),
        ] {
            assert!(
                buf.contains(&a),
                "{a:?} should be playable in the action phase"
            );
        }
    }

    #[test]
    fn a_development_card_cannot_be_played_the_turn_it_is_bought() {
        let mut s = State::new(3, 4);
        s.phase = Phase::Action;
        s.hand[0] = [0, 0, 1, 1, 1];
        s.supply = [19, 19, 18, 18, 18];
        s.apply(Action::BuyDev).unwrap();

        let bought = s.dev_deck[0];
        if bought != DevCard::VictoryPoint {
            assert_eq!(s.dev_playable(0, bought), 0, "fresh card is unplayable");
            let mut buf = Vec::new();
            s.legal_into(&mut buf);
            assert!(
                !buf.iter().any(
                    |a| matches!(a, Action::PlayMilitia | Action::PlayRoadBuilding)
                        || matches!(a, Action::PlayMonopoly(_) | Action::PlayInvention(_))
                ),
                "R-9.4 forbids playing it this turn"
            );
        }
        // After the turn ends it becomes available.
        s.apply(Action::EndTurn).unwrap();
        s.to_act = 0;
        s.phase = Phase::Action;
        assert_eq!(s.dev_playable(0, bought), 1);
    }

    #[test]
    fn only_one_development_card_per_turn() {
        let mut s = State::new(3, 5);
        s.phase = Phase::Action;
        s.dev_held[0][DevCard::Monopoly as usize] = 2;
        s.apply(Action::PlayMonopoly(Resource::Ore)).unwrap();
        assert_eq!(
            s.apply(Action::PlayMonopoly(Resource::Wood)),
            Err(Illegal::AlreadyPlayedDev),
            "R-9.3"
        );
    }

    #[test]
    fn monopoly_takes_every_opponents_holding() {
        let mut s = State::new(4, 6);
        s.phase = Phase::Action;
        s.dev_held[0][DevCard::Monopoly as usize] = 1;
        s.hand[1][Resource::Ore as usize] = 3;
        s.hand[2][Resource::Ore as usize] = 2;
        s.hand[3][Resource::Wood as usize] = 4;
        s.supply[Resource::Ore as usize] = 14;
        s.supply[Resource::Wood as usize] = 15;

        s.apply(Action::PlayMonopoly(Resource::Ore)).unwrap();
        assert_eq!(s.hand[0][Resource::Ore as usize], 5);
        assert_eq!(s.hand[1][Resource::Ore as usize], 0);
        assert_eq!(s.hand[2][Resource::Ore as usize], 0);
        assert_eq!(s.hand[3][Resource::Wood as usize], 4, "only the named type");
        s.assert_invariants();
    }

    #[test]
    fn a_short_supply_pays_nobody_unless_one_player_is_owed() {
        // R-5.6: all-or-nothing, except a lone claimant takes the remainder.
        let mut s = State::new(3, 7);
        let h = (0..HEX_COUNT)
            .find(|&h| s.terrain[h] == Terrain::Mountains && h as u8 != s.robber)
            .unwrap();
        let roll = s.number[h];
        let corners: Vec<u8> = iter_vertices(hex_vertices(h as u8)).collect();
        let ore = Resource::Ore as usize;

        // Two players owed 1 each, only 1 card left: nobody gets it.
        s.settlements[0] |= vertex_bit(corners[0]);
        s.settlements[1] |= vertex_bit(corners[2]);
        s.supply[ore] = 1;
        let mut two = s;
        two.distribute(roll);
        assert_eq!(two.hand[0][ore], 0);
        assert_eq!(two.hand[1][ore], 0);
        assert_eq!(two.supply[ore], 1, "the card stays in the supply");

        // One player owed 2, only 1 left: they take what remains.
        let mut one = s;
        one.settlements[1] = 0;
        one.cities[0] = vertex_bit(corners[0]);
        one.settlements[0] = 0;
        one.supply[ore] = 1;
        one.distribute(roll);
        assert_eq!(one.hand[0][ore], 1);
        assert_eq!(one.supply[ore], 0);
    }

    #[test]
    fn a_seven_forces_discards_of_half_rounded_down() {
        let mut s = State::new(3, 8);
        s.phase = Phase::Action;
        s.hand[0] = [9, 0, 0, 0, 0];
        s.hand[1] = [7, 0, 0, 0, 0];
        s.supply[0] = 3;
        s.begin_seven();
        assert_eq!(s.phase, Phase::Discard);
        assert_eq!(s.discard_left[0], 4, "9 cards discards 4");
        assert_eq!(s.discard_left[1], 0, "7 cards is not over the limit");
    }

    #[test]
    fn the_robber_must_move_somewhere_new() {
        let mut s = State::new(3, 9);
        s.phase = Phase::MoveRobber {
            from_militia: false,
        };
        let here = s.robber;
        assert_eq!(
            s.apply(Action::MoveRobber {
                hex: here,
                victim: None
            }),
            Err(Illegal::RobberMustMove),
            "R-6.3"
        );
        let elsewhere = (here + 1) % HEX_COUNT as u8;
        assert!(
            s.apply(Action::MoveRobber {
                hex: elsewhere,
                victim: None
            })
            .is_ok()
        );
        assert_eq!(s.robber, elsewhere);
    }

    #[test]
    fn robbing_an_empty_hand_takes_nothing() {
        let mut s = State::new(3, 10);
        s.phase = Phase::MoveRobber {
            from_militia: false,
        };
        let h = (0..HEX_COUNT as u8).find(|&h| h != s.robber).unwrap();
        let v = iter_vertices(hex_vertices(h)).next().unwrap();
        s.settlements[1] |= vertex_bit(v);
        s.settlements_left[1] -= 1;
        // Victim holds nothing; the move is still legal (R-6.4).
        s.apply(Action::MoveRobber {
            hex: h,
            victim: Some(1),
        })
        .unwrap();
        assert_eq!(s.hand_size(0), 0);
        s.assert_invariants();
    }

    #[test]
    fn longest_road_needs_five_and_a_strict_lead() {
        let mut s = State::new(3, 11);
        // Four roads is not enough (R-10.1).
        let mut roads = 0u128;
        let mut at = 0u8;
        for _ in 0..4 {
            let e = iter_edges(crate::topology::edges_at(at) & !roads)
                .next()
                .unwrap();
            roads |= edge_bit(e);
            at = crate::topology::edge_other(e, at);
        }
        s.roads[0] = roads;
        s.update_longest_road();
        assert_eq!(s.longest_road, None);

        let e = iter_edges(crate::topology::edges_at(at) & !roads)
            .next()
            .unwrap();
        s.roads[0] |= edge_bit(e);
        s.update_longest_road();
        assert_eq!(s.longest_road, Some(0));
    }

    #[test]
    fn victory_is_only_claimed_on_your_own_turn() {
        let mut s = State::new(3, 12);
        s.phase = Phase::Action;
        // Seat 1 is handed a winning position while seat 0 is to act.
        s.cities[1] = vertex_bit(3) | vertex_bit(9) | vertex_bit(15) | vertex_bit(21);
        s.cities_left[1] = 0;
        s.dev_held[1][DevCard::VictoryPoint as usize] = 2;
        assert!(s.victory_points(1) >= WINNING_VP);

        s.apply(Action::EndTurn).unwrap();
        assert!(
            !matches!(s.phase, Phase::GameOver { .. }),
            "seat 1 cannot win during seat 0's turn (R-11.1)"
        );
        // On its own turn the win registers.
        s.to_act = 1;
        s.phase = Phase::Action;
        s.apply(Action::EndTurn).unwrap();
        s.to_act = 1;
        s.check_victory();
        assert_eq!(s.phase, Phase::GameOver { winner: 1 });
    }

    #[test]
    fn the_deck_runs_out_rather_than_wrapping() {
        let mut s = State::new(3, 13);
        s.phase = Phase::Action;
        s.dev_drawn = DEV_DECK_SIZE as u8;
        s.hand[0] = [0, 0, 1, 1, 1];
        assert_eq!(s.apply(Action::BuyDev), Err(Illegal::SupplyEmpty), "R-8.10");
    }

    fn trading_game(seed: u64) -> State {
        let mut s = State::new(4, seed).with_trade_mode(TradeMode::Full);
        s.phase = Phase::Action;
        s
    }

    fn one(r: Resource, n: u8) -> [u8; 5] {
        let mut c = [0u8; 5];
        c[r as usize] = n;
        c
    }

    /// Deal cards from the supply, so conservation still holds.
    fn deal(s: &mut State, seat: usize, cards: [u8; 5]) {
        for (r, &n) in cards.iter().enumerate() {
            s.hand[seat][r] += n;
            s.supply[r] -= n;
        }
    }

    #[test]
    fn an_offer_is_taken_and_the_cards_change_hands() {
        let mut s = trading_game(20);
        deal(&mut s, 0, one(Resource::Ore, 2));
        deal(&mut s, 1, one(Resource::Wood, 3));
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Ore, 2),
            want: one(Resource::Wood, 3),
        })
        .unwrap();
        assert_eq!(s.offer_count, 1);

        s.apply(Action::AcceptTrade { offer: 0, by: 1 }).unwrap();
        assert_eq!(s.hand[0][Resource::Wood as usize], 3);
        assert_eq!(s.hand[1][Resource::Ore as usize], 2);
        assert_eq!(s.offer_count, 0, "a taken offer leaves the market");
        s.assert_invariants();
    }

    #[test]
    fn a_trade_must_give_and_take() {
        let mut s = trading_game(21);
        s.hand[0] = one(Resource::Ore, 2);
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Ore, 1),
                want: [0; 5]
            }),
            Err(Illegal::EmptySide),
            "R-7.5 forbids a gift"
        );
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: [0; 5],
                want: one(Resource::Wood, 1)
            }),
            Err(Illegal::EmptySide)
        );
    }

    #[test]
    fn no_resource_may_sit_on_both_sides() {
        let mut s = trading_game(22);
        s.hand[0] = one(Resource::Ore, 3);
        let mut want = one(Resource::Wood, 1);
        want[Resource::Ore as usize] = 1;
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Ore, 3),
                want
            }),
            Err(Illegal::TypeOverlap),
            "R-7.18"
        );
    }

    #[test]
    fn two_opponents_cannot_trade_with_each_other() {
        // Every trade needs the active player as one party (R-7.3).
        let mut s = trading_game(23);
        s.hand[1] = one(Resource::Ore, 1);
        s.hand[2] = one(Resource::Wood, 1);
        // Seat 1 proposes; seat 0 is active, so the offer is addressed to it.
        s.apply(Action::ProposeTrade {
            by: 1,
            to: None,
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 1),
        })
        .unwrap();
        assert_eq!(s.offers[0].from, 1);

        // Seat 2 holds what the offer wants, but may not take it.
        let mut buf = Vec::new();
        s.legal_for(2, &mut buf);
        assert!(
            !buf.iter().any(|a| matches!(a, Action::AcceptTrade { .. })),
            "seat 2 is not a party to this offer"
        );
    }

    #[test]
    fn a_second_acceptance_of_the_same_offer_is_stale() {
        // The acceptance race of R-7.19: offers resolve first-come, and the
        // loser is told why rather than silently executing.
        let mut s = trading_game(24);
        s.hand[0] = one(Resource::Ore, 1);
        s.hand[1] = one(Resource::Wood, 1);
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 1),
        })
        .unwrap();
        s.apply(Action::AcceptTrade { offer: 0, by: 1 }).unwrap();
        assert_eq!(
            s.apply(Action::AcceptTrade { offer: 0, by: 1 }),
            Err(Illegal::NoSuchOffer),
            "the offer is gone once taken"
        );
    }

    #[test]
    fn an_offer_the_proposer_can_no_longer_pay_is_rejected_not_executed() {
        // R-7.19: re-validate at execution, never against the state the offer
        // was authored in.
        let mut s = trading_game(25);
        deal(&mut s, 0, [1, 1, 1, 1, 0]);
        deal(&mut s, 1, one(Resource::Ore, 1));
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Brick, 1),
            want: one(Resource::Ore, 1),
        })
        .unwrap();

        // The proposer spends the brick on a settlement before anyone accepts.
        let v = iter_vertices(s.settlement_spots(0, true)).next().unwrap();
        s.settlements[0] |= vertex_bit(v);
        s.settlements_left[0] -= 1;
        pay(&mut s, 0, &SETTLEMENT_COST);

        assert_eq!(
            s.apply(Action::AcceptTrade { offer: 0, by: 1 }),
            Err(Illegal::OfferStale),
            "the offer went stale and must not execute"
        );
        assert_eq!(s.offer_count, 0, "and it is pruned from the market");
        s.assert_invariants();
    }

    #[test]
    fn an_offer_the_taker_cannot_pay_is_rejected_not_executed() {
        // The other half of R-7.19's re-validation. Nothing bounds how much an
        // offer may *ask* for: under an open market the taker is not known
        // when the offer is made, so affordability cannot be checked at
        // proposal. It is checked here instead, and a taker who cannot pay
        // must be refused rather than driven negative.
        let mut s = trading_game(27);
        deal(&mut s, 0, one(Resource::Ore, 1));
        deal(&mut s, 1, one(Resource::Wood, 1));

        // Legal to propose: the proposer holds what it offers, and both sides
        // are non-empty and disjoint. It simply asks for more wood than seat 1
        // has, or than anyone could have.
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 200),
        })
        .expect("an unaffordable ask is still a well-formed offer");

        let before = s.hand;
        assert_eq!(
            s.apply(Action::AcceptTrade { offer: 0, by: 1 }),
            Err(Illegal::OfferStale),
            "the taker cannot pay and the trade must not execute"
        );
        assert_eq!(s.hand, before, "no cards moved");
        assert_eq!(s.offer_count, 0, "and the offer is pruned");
        s.assert_invariants();
    }

    #[test]
    fn a_taker_short_by_a_single_card_is_still_refused() {
        // The boundary, since an off-by-one here would silently underflow a
        // hand rather than fail.
        let mut s = trading_game(28);
        deal(&mut s, 0, one(Resource::Ore, 1));
        deal(&mut s, 1, one(Resource::Wood, 2));
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 3),
        })
        .unwrap();
        assert_eq!(
            s.apply(Action::AcceptTrade { offer: 0, by: 1 }),
            Err(Illegal::OfferStale)
        );
        assert_eq!(s.hand[1][Resource::Wood as usize], 2, "untouched");

        // Exactly enough goes through.
        deal(&mut s, 1, one(Resource::Wood, 1));
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 3),
        })
        .unwrap();
        s.apply(Action::AcceptTrade { offer: 0, by: 1 }).unwrap();
        assert_eq!(s.hand[0][Resource::Wood as usize], 3);
        assert_eq!(s.hand[1][Resource::Ore as usize], 1);
        s.assert_invariants();
    }

    #[test]
    fn quantities_are_unsigned_so_a_negative_side_cannot_be_expressed() {
        // Recorded because it is a question worth being able to answer: there
        // is no rule rejecting negative quantities, because `give` and `want`
        // are `[u8; 5]` and no negative value exists to reject. A side that
        // would be "negative" is simply the same trade the other way round.
        let mut s = trading_game(29);
        deal(&mut s, 0, one(Resource::Ore, 1));
        deal(&mut s, 1, one(Resource::Wood, 1));

        // The only degenerate quantity expressible is zero, and both sides are
        // required to carry something (R-7.5).
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: [0; 5],
                want: [0; 5],
            }),
            Err(Illegal::EmptySide)
        );
        // Trading a resource for itself is the other degenerate case (R-7.18).
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Ore, 1),
                want: one(Resource::Ore, 1),
            }),
            Err(Illegal::TypeOverlap)
        );
        assert_eq!(s.offer_count, 0);
    }

    #[test]
    fn an_addressed_offer_is_for_that_seat_alone() {
        // R-7.19 leaves the market open by default; addressing one narrows it
        // to a single seat. Everyone else must be turned away, not merely
        // discouraged.
        let mut s = trading_game(41);
        deal(&mut s, 0, one(Resource::Ore, 1));
        deal(&mut s, 1, one(Resource::Wood, 1));
        deal(&mut s, 2, one(Resource::Wood, 1));

        s.apply(Action::ProposeTrade {
            by: 0,
            to: Some(2),
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 1),
        })
        .unwrap();
        assert_eq!(s.offers[0].to, Some(2));

        // Seat 1 could have taken this were it open, and holds what it asks.
        assert_eq!(
            s.apply(Action::AcceptTrade { offer: 0, by: 1 }),
            Err(Illegal::NotAParty),
            "an addressed offer is not open to the table"
        );
        // The seat it was addressed to takes it.
        s.apply(Action::AcceptTrade { offer: 0, by: 2 }).unwrap();
        assert_eq!(s.hand[2][Resource::Ore as usize], 1);
        assert_eq!(s.hand[0][Resource::Wood as usize], 1);
        s.assert_invariants();
    }

    #[test]
    fn an_open_offer_is_still_open() {
        // The default must not have moved: `to: None` behaves exactly as
        // before, or every existing offer changes meaning.
        let mut s = trading_game(42);
        deal(&mut s, 0, one(Resource::Ore, 1));
        deal(&mut s, 1, one(Resource::Wood, 1));
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 1),
        })
        .unwrap();
        s.apply(Action::AcceptTrade { offer: 0, by: 1 }).unwrap();
        assert_eq!(s.hand[1][Resource::Ore as usize], 1);
    }

    #[test]
    fn an_addressed_offer_still_obeys_the_turn_rule() {
        // R-7.3: whoever the parties are, one of them is the active player.
        // Addressing must not become a way around that, seat 1 offering seat 2
        // during seat 0's turn is the triangular trade the rule forbids.
        let mut s = trading_game(43);
        deal(&mut s, 1, one(Resource::Ore, 1));
        assert_eq!(s.to_act, 0);
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 1,
                to: Some(2),
                give: one(Resource::Ore, 1),
                want: one(Resource::Wood, 1),
            }),
            Err(Illegal::NotAParty),
            "R-7.3 forbids two non-active players trading"
        );
        // Addressed to the active player, the same offer is fine.
        s.apply(Action::ProposeTrade {
            by: 1,
            to: Some(0),
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 1),
        })
        .unwrap();
        assert_eq!(s.offer_count, 1);
    }

    #[test]
    fn an_offer_cannot_be_addressed_to_nobody_or_to_oneself() {
        let mut s = trading_game(44);
        deal(&mut s, 0, one(Resource::Ore, 1));
        for to in [Some(0), Some(9)] {
            assert_eq!(
                s.apply(Action::ProposeTrade {
                    by: 0,
                    to,
                    give: one(Resource::Ore, 1),
                    want: one(Resource::Wood, 1),
                }),
                Err(Illegal::NotAParty),
                "addressed to {to:?}"
            );
        }
        assert_eq!(s.offer_count, 0);
    }

    #[test]
    fn generated_offers_stay_open() {
        // The action space a search consumes must not multiply by the number
        // of opponents. Addressing is for clients, not for enumeration.
        let mut s = trading_game(45);
        deal(&mut s, 0, [2, 2, 2, 2, 2]);
        let mut buf = Vec::new();
        s.legal_into(&mut buf);
        let proposals: Vec<_> = buf
            .iter()
            .filter(|a| matches!(a, Action::ProposeTrade { to: None, .. }))
            .collect();
        assert!(!proposals.is_empty(), "a full hand should generate offers");
        assert!(
            proposals
                .iter()
                .all(|a| matches!(a, Action::ProposeTrade { to: None, .. })),
            "generation must not enumerate recipients"
        );
    }

    #[test]
    fn offers_are_capped_per_turn_and_cleared_at_its_end() {
        let mut s = trading_game(26);
        s.hand[0] = [9, 9, 9, 9, 9];
        s.supply = [10, 10, 10, 10, 10];
        // The market fills before the per-turn cap bites (R-7.20, D-7).
        for _ in 0..MAX_OFFERS {
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Brick, 1),
                want: one(Resource::Ore, 1),
            })
            .unwrap();
        }
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Brick, 1),
                want: one(Resource::Ore, 1)
            }),
            Err(Illegal::MarketFull)
        );
        assert_eq!(s.offers_made[0], MAX_OFFERS as u8);

        s.apply(Action::EndTurn).unwrap();
        assert_eq!(s.offer_count, 0, "offers do not survive the turn");
        assert_eq!(s.offers_made, [0; MAX_PLAYERS]);
    }

    #[test]
    fn the_per_turn_offer_cap_is_enforced() {
        let mut s = trading_game(27);
        s.hand[0] = [9, 0, 0, 0, 0];
        s.offers_made[0] = OFFERS_PER_TURN;
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Brick, 1),
                want: one(Resource::Ore, 1)
            }),
            Err(Illegal::OfferLimit),
            "R-7.20 bounds a turn's offers"
        );
    }

    #[test]
    fn only_the_proposer_withdraws() {
        let mut s = trading_game(28);
        s.hand[0] = one(Resource::Ore, 1);
        s.apply(Action::ProposeTrade {
            by: 0,
            to: None,
            give: one(Resource::Ore, 1),
            want: one(Resource::Wood, 1),
        })
        .unwrap();
        let mut buf = Vec::new();
        s.legal_for(1, &mut buf);
        assert!(
            !buf.iter()
                .any(|a| matches!(a, Action::WithdrawTrade { .. })),
            "an opponent cannot withdraw someone else's offer"
        );
        s.legal_into(&mut buf);
        assert!(buf.contains(&Action::WithdrawTrade { offer: 0, by: 0 }));
        s.apply(Action::WithdrawTrade { offer: 0, by: 0 }).unwrap();
        assert_eq!(s.offer_count, 0);
    }

    #[test]
    fn trading_is_off_unless_the_game_enables_it() {
        let mut s = State::new(4, 29);
        s.phase = Phase::Action;
        s.hand[0] = one(Resource::Ore, 1);
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Ore, 1),
                want: one(Resource::Wood, 1)
            }),
            Err(Illegal::TradeDisabled)
        );
        let mut buf = Vec::new();
        s.legal_for(1, &mut buf);
        assert!(buf.is_empty(), "no market when trading is off");
    }

    #[test]
    fn restricted_mode_allows_only_one_for_one() {
        let mut s = State::new(4, 30).with_trade_mode(TradeMode::Restricted);
        s.phase = Phase::Action;
        s.hand[0] = [3, 0, 0, 0, 0];
        assert!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Brick, 1),
                want: one(Resource::Ore, 1)
            })
            .is_ok()
        );
        assert_eq!(
            s.apply(Action::ProposeTrade {
                by: 0,
                to: None,
                give: one(Resource::Brick, 2),
                want: one(Resource::Ore, 1)
            }),
            Err(Illegal::TradeDisabled),
            "the restricted menu is one card for one card"
        );
    }

    #[test]
    fn an_open_market_game_stays_consistent() {
        // The market adds a second way for hands to move; the conservation
        // invariants must survive it.
        for seed in 400..440 {
            let mut s = State::new(4, seed).with_trade_mode(TradeMode::Full);
            let mut rng = Rng::new(seed ^ 0xBEEF);
            let mut buf = Vec::new();
            for _ in 0..3_000 {
                if matches!(s.phase, Phase::GameOver { .. }) {
                    break;
                }
                // Half the time let a non-active seat work the market.
                let seat = if rng.below(crate::rng::Stream::Dice, 2) == 0 {
                    s.decider()
                } else {
                    (rng.below(crate::rng::Stream::Dice, s.players as u32)) as u8
                };
                s.legal_for(seat, &mut buf);
                if buf.is_empty() {
                    s.legal_into(&mut buf);
                }
                let a = buf[rng.below(crate::rng::Stream::Dice, buf.len() as u32) as usize];
                let _ = s.apply(a);
                s.assert_invariants();
            }
        }
    }

    #[test]
    fn supply_trade_uses_the_best_available_rate() {
        let mut s = State::new(3, 14);
        s.phase = Phase::Action;
        s.hand[0] = [4, 0, 0, 0, 0];
        s.supply[0] = 15;
        s.apply(Action::Trade {
            give: Resource::Brick,
            take: Resource::Ore,
        })
        .unwrap();
        assert_eq!(s.hand[0][0], 0, "4:1 without a port");
        assert_eq!(s.hand[0][Resource::Ore as usize], 1);
        s.assert_invariants();

        // With the brick port it is 2:1.
        let port = iter_vertices(s.ports[Resource::Brick as usize + 1])
            .next()
            .unwrap();
        s.settlements[0] |= vertex_bit(port);
        s.hand[0] = [2, 0, 0, 0, 0];
        s.supply[0] = 17;
        s.apply(Action::Trade {
            give: Resource::Brick,
            take: Resource::Wood,
        })
        .unwrap();
        assert_eq!(s.hand[0][0], 0);
        assert_eq!(s.hand[0][Resource::Wood as usize], 1);
    }

    #[test]
    fn trading_needs_a_stack_that_can_pay_in_full() {
        let mut s = State::new(3, 15);
        s.phase = Phase::Action;
        s.hand[0] = [4, 0, 0, 0, 0];
        s.supply[Resource::Ore as usize] = 0;
        assert_eq!(
            s.apply(Action::Trade {
                give: Resource::Brick,
                take: Resource::Ore
            }),
            Err(Illegal::SupplyEmpty),
            "D-3"
        );
    }
}
