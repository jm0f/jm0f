//! Heuristic policy.
//!
//! Instant, in-process and free, which is what its four jobs demand (§9.3):
//! taking over a disconnected seat, filling a lobby, standing in when the LLM
//! player fails, and being the fixed yardstick every trained agent is measured
//! against.
//!
//! # How it plays
//!
//! One ply of greedy search. For each legal action it copies the state, applies
//! the action, and scores the result; the best score wins. Copying is a
//! `memcpy` of 384 bytes and applying is ~35 ns, so a whole turn's deliberation
//! costs microseconds.
//!
//! The score is **competitive**, not absolute: `value(me) − best value(any
//! opponent)`. That one choice is what makes blocking play fall out for free, //! the robber lands on whoever is strongest rather than wherever is nearest,
//! and a settlement that cuts a rival's route scores its damage.
//!
//! # Where one ply is blind
//!
//! Some moves pay off only after a follow-up the search never sees. Playing a
//! Militia leads to a robber move whose benefit is a ply away, and rolling the
//! dice has an outcome that has not happened yet. Both are handled by scoring
//! the *position* rather than the outcome, progress toward Largest Militia is
//! a feature in its own right, and rolling is scored as the status quo rather
//! than by sampling a roll the bot cannot choose. This is a real limitation,
//! not an oversight: a bot that needs to see two plies wants the search tier,
//! which is a different bot (B-5).

use carranta_core::action::{Action, CITY_COST, DEV_COST, ROAD_COST, SETTLEMENT_COST};
use carranta_core::rng::{Rng, Stream};
use carranta_core::state::{MAX_PLAYERS, Phase, State, TradeMode};
use carranta_core::topology::hex_vertices;

/// Anything that can take a seat: human, heuristic, LLM, trained agent.
///
/// The engine cannot tell them apart, which is what makes a bot swappable
/// (§9.2). Implementations must answer within a bounded time and must always
/// return one of the offered actions.
pub trait Policy {
    fn choose(&mut self, state: &State, legal: &[Action]) -> Action;

    /// Whether `seat` takes live offer `offer`.
    ///
    /// Separate from [`Policy::choose`] because responding to the market is
    /// not a turn: an opponent is asked whenever an offer is on the table, and
    /// declining has to be possible without picking some other action. The
    /// default is to refuse everything.
    fn accepts(&mut self, state: &State, seat: usize, offer: usize) -> bool {
        let _ = (state, seat, offer);
        false
    }
}

/// Feature weights. Integers, not floats: this bot is a measuring stick, and
/// float results can differ across platforms in ways that would quietly
/// invalidate a benchmark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weights {
    /// Victory points. Dominant, everything else is a means to these.
    pub vp: i32,
    /// Expected production, in dice-pips weighted by building size.
    pub pips: i32,
    /// Distinct resources produced. Being locked out of one is crippling.
    pub diversity: i32,
    /// Ports held, which cheapen the conversion of a surplus.
    pub port: i32,
    /// Length of the longest route, as progress toward the tile.
    pub road: i32,
    /// Militia played, as progress toward Largest Militia.
    pub militia: i32,
    /// Cards in hand: useful, but only up to the discard limit.
    pub hand: i32,
    /// Penalty per card above the discard limit (R-6.2).
    pub over_limit: i32,
    /// Development cards held.
    pub dev: i32,
    /// Settlements and cities still available to place.
    pub pieces: i32,
    /// Progress toward affording the next purchase, per build type.
    ///
    /// Without this the bot never trades: a supply trade is a net loss of
    /// cards, and one ply cannot see the build it enables. Scoring partial
    /// progress toward a cost makes the enabling trade pay for itself.
    pub build_progress: i32,
    /// Expected value of an unseen development card.
    pub buy_dev: i32,
    /// Value of robbing a card, per card in the victim's hand.
    pub steal: i32,
    /// Percentage of a proposed trade's gain that is credited when deciding to
    /// offer it. A proposal may be refused, so it is worth less than the swap.
    pub offer_discount: i32,
    /// Percentage of the other side's gain charged against one's own when
    /// judging a swap.
    ///
    /// A trade is the one move that helps two players at once, and this is the
    /// only weight that says how much that matters. At 100 a deal has to be
    /// better for me than for them, which sounds like hard bargaining and is
    /// actually a refusal to trade at all: the seat making an offer picks the
    /// one it likes best, so its own gain is nearly always the larger, and
    /// every offer dies. At 0 the bot takes any swap that helps it, including
    /// the one that hands the leader a city. Somewhere in between it trades on
    /// roughly even terms and turns down a fleecing, which is what a person
    /// does.
    pub rival_gain: i32,
    /// Penalty per offer this seat has already *requested* this turn.
    ///
    /// Cumulative, not a count of live offers: asking costs something whether
    /// or not the previous ask came to anything, so the bar rises with each
    /// one and only a clearly good trade is worth raising. That is roughly how
    /// a person weighs it. The third request of the same turn has to be worth
    /// more than the first, because people stop listening.
    ///
    /// Counting live offers instead lets the bot churn: make an offer, have it
    /// taken, make another at no cost, indefinitely.
    pub offer_cost: i32,
}

impl Default for Weights {
    fn default() -> Self {
        // Hand-set, not tuned: they are a starting point that plays sensibly,
        // and the surface a later optimiser would work on.
        Weights {
            vp: 1000,
            pips: 12,
            diversity: 25,
            port: 15,
            road: 8,
            militia: 20,
            hand: 3,
            over_limit: -12,
            dev: 10,
            pieces: 2,
            build_progress: 6,
            buy_dev: 40,
            steal: 8,
            offer_discount: 55,
            offer_cost: 8,
            rival_gain: 60,
        }
    }
}

/// What the house bot is called wherever a game is written down.
///
/// A name and a number, together, because a rating is a claim about a player
/// and a program whose play has changed is a different player. Two builds
/// sharing one identity pool their results and the pool then describes neither
/// of them: the newer one is dragged towards the older one's record and the
/// older one's record is quietly overwritten by a program nobody measured.
///
/// So the pair goes into every game file, and a seat identity is built from it
/// rather than from "a bot". Raise the number whenever the play changes, which
/// includes the weights: a rating earned by one set of numbers is not evidence
/// about another. Nothing merges the two afterwards, and that is the point.
/// Comparing them is a matter of reading two rows rather than of trusting that
/// one row means one thing.
///
/// A second kind of bot is a second name. `llm-<model>` and `trained` are the
/// two the scoping doc expects (§8.2), and neither needs anything here beyond
/// its own pair of constants.
pub const HOUSE: &str = "house";

/// The house bot's build number. See [`HOUSE`].
pub const HOUSE_VERSION: u32 = 1;

/// The heuristic policy.
///
/// Deterministic: the same position yields the same move. Exact ties break on
/// a seeded stream, so a bot constructed with a given seed replays identically
/// while four bots in one lobby do not mirror each other.
pub struct Heuristic {
    pub weights: Weights,
    rng: Rng,
}

impl Heuristic {
    pub fn new(seed: u64) -> Self {
        Heuristic {
            weights: Weights::default(),
            rng: Rng::new(seed),
        }
    }

    pub fn with_weights(seed: u64, weights: Weights) -> Self {
        Heuristic {
            weights,
            rng: Rng::new(seed),
        }
    }

    /// How good this position is for `me`, relative to the best opponent.
    pub fn score(&self, state: &State, me: usize) -> i32 {
        self.value(state, me) - self.best_opponent(state, me)
    }

    fn best_opponent(&self, state: &State, me: usize) -> i32 {
        let mut best = i32::MIN;
        for q in 0..state.players as usize {
            if q != me {
                best = best.max(self.value(state, q));
            }
        }
        best
    }

    /// Absolute value of one seat's position.
    fn value(&self, state: &State, p: usize) -> i32 {
        let w = &self.weights;
        let mut v = state.victory_points(p) as i32 * w.vp;

        // Expected production. A number's pips are its ways of being rolled,
        // so 6 and 8 are worth six times a 2 or a 12.
        let mut pips = 0i32;
        let mut produced = [false; 5];
        for h in 0..carranta_core::topology::HEX_COUNT as u8 {
            if h == state.robber {
                continue; // blocked, so worth nothing while it sits there
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
                pips += (6 - (7i32 - n as i32).abs()) * count as i32;
                produced[res as usize] = true;
            }
        }
        v += pips * w.pips;
        v += produced.iter().filter(|&&b| b).count() as i32 * w.diversity;

        for kind in 0..carranta_core::state::PORT_KINDS {
            if state.has_port(p, kind) {
                v += w.port;
            }
        }

        v += carranta_core::longest_road::longest_road(state.roads[p], state.blocking(p)) as i32
            * w.road;
        v += state.militia_played[p] as i32 * w.militia;
        v += state.dev_count(p) as i32 * w.dev;
        v += (state.settlements_left[p] + state.cities_left[p]) as i32 * w.pieces;

        // The cards are all in `hand_value`, including the discard-limit
        // penalty. They were also inline here, which counted both twice: a card
        // was worth 6 where the weight says 3, and being over the limit cost
        // 24 where it says 12. Nothing compared two positions wrongly because of
        // it, since every position was inflated alike, but the weights did not
        // mean what they said and a trade was judged against a hand it thought
        // twice as valuable as it is.
        v + self.hand_value(&state.hand[p])
    }

    /// What a swap does to a hand: out with `out`, back with `back`.
    ///
    /// A trade touches nothing else, which is why this is the whole of it.
    /// Production, ports, routes and points are all where they were.
    ///
    /// `risk` decides whether the discard limit counts. It should for one's own
    /// hand, where going over it is a real cost, and it should *not* when
    /// measuring what a swap does for the other side. A seat shedding cards it
    /// cannot hold is getting safer rather than stronger, and charging that
    /// relief against one's own gain is how a bot talks itself out of a deal
    /// that hands it three cards for one. Their progress counts against me;
    /// their luck at dodging a seven does not.
    fn swap_gain(&self, hand: &[u8; 5], out: &[u8; 5], back: &[u8; 5], risk: bool) -> i32 {
        let mut after = *hand;
        for r in 0..5 {
            after[r] = after[r].saturating_sub(out[r]) + back[r];
        }
        self.hand_worth(&after, risk) - self.hand_worth(hand, risk)
    }

    /// Whether a swap worth `mine` to me and `theirs` to the other side is one
    /// to take.
    ///
    /// Both halves matter. A deal that does nothing for me is not a deal, and a
    /// deal that does far more for them is how a table loses to whoever is
    /// asking. Between those the bot trades, which it has to: a game where
    /// nobody trades is not this game being played badly, it is a different and
    /// easier game.
    fn worth_taking(&self, mine: i32, theirs: i32) -> bool {
        mine > 0 && mine * 100 > theirs * self.weights.rival_gain
    }

    /// The part of a position's value that depends only on the cards held.
    ///
    /// Split out because a trade changes nothing else: production, ports,
    /// routes and points are all untouched, so a candidate swap can be valued
    /// from this alone instead of re-walking the board.
    fn hand_value(&self, hand: &[u8; 5]) -> i32 {
        self.hand_worth(hand, true)
    }

    /// The same, with the discard limit optional. See [`Heuristic::swap_gain`].
    fn hand_worth(&self, hand: &[u8; 5], risk: bool) -> i32 {
        let w = &self.weights;
        let total: i32 = hand.iter().map(|&n| n as i32).sum();
        let mut v = total.min(7) * w.hand;
        if risk && total > 7 {
            v += (total - 7) * w.over_limit;
        }
        // How close the hand is to each purchase, scaled so a complete set is
        // worth `build_progress` and a half-set half of it.
        for cost in [&ROAD_COST, &SETTLEMENT_COST, &CITY_COST, &DEV_COST] {
            let need: i32 = cost.iter().map(|&c| c as i32).sum();
            let have: i32 = cost
                .iter()
                .enumerate()
                .map(|(r, &c)| (hand[r] as i32).min(c as i32))
                .sum();
            v += have * w.build_progress / need;
        }
        v
    }

    /// Can this action change what an *opponent's* position is worth?
    ///
    /// Most cannot: buying, trading, upgrading and road-building all touch
    /// only the mover. Knowing that lets the opponent half of the score be
    /// evaluated once per decision instead of once per candidate, which is
    /// most of the work. Each evaluation walks 19 hexes and computes a
    /// longest route.
    fn affects_opponents(action: Action) -> bool {
        matches!(
            action,
            Action::MoveRobber { .. }
                | Action::PlayMonopoly(_)
                // A settlement can cut a rival's route (R-10.4).
                | Action::BuildSettlement(_)
                | Action::PlaceSettlement(_)
        )
    }

    /// Score one candidate action by the position it leads to.
    ///
    /// Three actions are scored *without* applying them, because applying
    /// would let the bot see what it is not entitled to know. Buying a
    /// development card draws the real top of the deck, so an applied score
    /// would tell the bot whether the next card is a Victory Point before it
    /// decides to buy. Robbing takes a real random card, so an applied score
    /// would let it pick the victim whose card it liked best. Rolling has an
    /// outcome that has not happened and that the bot cannot shape anyway.
    /// Each is scored from what a player legitimately knows.
    fn score_action(&self, state: &State, me: usize, action: Action, base_other: i32) -> i32 {
        let w = &self.weights;
        // Reuse the precomputed opponent term unless this action can move it.
        let against = |s: &State| {
            if Self::affects_opponents(action) {
                self.best_opponent(s, me)
            } else {
                base_other
            }
        };
        match action {
            Action::Roll => self.value(state, me) - base_other,

            Action::ProposeTrade {
                to: None,
                give,
                want,
                ..
            } => {
                // One ply cannot value a proposal: making it changes nothing
                // until someone takes it, so a brilliant offer and an absurd
                // one score identically. Value it by the swap it would produce
                // instead, discounted because it may simply be refused.
                let gain = self.swap_gain(&state.hand[me], &give, &want, true);
                // And discounted to nothing when it certainly will be refused,
                // which is the difference between offering and haggling with
                // yourself. Whether anybody would take it is answerable right
                // here: they will judge it by [`Policy::accepts`], which is this
                // same rule, so the bot can ask the question they will ask.
                //
                // This reads opponents' hands, which the competitive score has
                // always done through `best_opponent`, so it is not a new kind
                // of peeking. It is also the half of trading that a market
                // without it cannot do: an offer priced only for its maker is an
                // offer nobody wants.
                // Their side of the same rule, so the answer is theirs and not a
                // guess: what they gain, against what this offer gains me, which
                // they will weigh without my discard troubles in it.
                let mine_to_them = self.swap_gain(&state.hand[me], &give, &want, false);
                let taker = (0..state.players as usize).any(|q| {
                    q != me
                        && state.holds(q, &want)
                        && self.worth_taking(
                            self.swap_gain(&state.hand[q], &want, &give, true),
                            mine_to_them,
                        )
                });
                let credit = if taker {
                    gain * w.offer_discount / 100
                } else {
                    0
                };
                // Every candidate proposal carries the same toll, so this does
                // not pick between offers. It decides whether making *any* is
                // worth more than getting on with the turn.
                let asked = state.offers_made[me] as i32;
                self.value(state, me) - base_other + credit - asked * w.offer_cost
            }

            Action::BuyDev => {
                // Pay the cost, but do not look at the card.
                let mut next = *state;
                for (r, &c) in DEV_COST.iter().enumerate() {
                    next.hand[me][r] -= c;
                    next.supply[r] += c;
                }
                self.value(&next, me) - base_other + w.buy_dev
            }

            Action::MoveRobber { hex, victim } => {
                // Move the robber, but do not draw the stolen card.
                let mut next = *state;
                next.robber = hex;
                let mut s = self.value(&next, me) - against(&next);
                if let Some(v) = victim {
                    // Worth more against a full hand, but with diminishing
                    // returns: one card is one card however rich the victim.
                    s += (state.hand_size(v as usize).min(6) as i32) * w.steal;
                }
                s
            }

            _ => {
                let mut next = *state;
                if next.apply(action).is_err() {
                    return i32::MIN;
                }
                self.value(&next, me) - against(&next)
            }
        }
    }
}

impl Policy for Heuristic {
    /// Whether this seat takes a live offer.
    ///
    /// Judged on the swap rather than on the whole position, which is both
    /// cheaper and the right question. A trade moves cards between two hands and
    /// touches nothing else, so the two hand values are all of it; and "is this
    /// deal good for me" is not the same question as "does this deal leave me
    /// ahead of the table", which is what the competitive score answers.
    ///
    /// Asking the second question is what made the bot refuse most offers.
    /// Accepting only when the position improves *against the best opponent*
    /// means accepting only when a deal is better for me than for the seat
    /// offering it, and the seat offering it picked the offer it liked best. Over
    /// sixty self-play games that took 1 205 of 8 053 offers, one in seven, and
    /// the ones it took were the ones where the offerer had blundered. On this
    /// rule the same measurement takes 3 709 of 6 974: fewer offers, because a
    /// dead one is no longer worth making, and half of them dealt.
    ///
    /// (Every trade in the *recorded* games was with the bank or a port, and
    /// that was a different bug: the path that plays a game out never put an
    /// offer to anybody at all. Both were found by the same analytics card.)
    fn accepts(&mut self, state: &State, seat: usize, offer: usize) -> bool {
        let Some(o) = state.live_offers().get(offer) else {
            return false;
        };
        if !state.may_accept(seat, o) || !state.holds(seat, &o.want) {
            return false;
        }
        // `give` and `want` are the proposer's side, so the seat taking it hands
        // over `want` and receives `give`.
        let mine = self.swap_gain(&state.hand[seat], &o.want, &o.give, true);
        let theirs = self.swap_gain(&state.hand[o.from as usize], &o.give, &o.want, false);
        self.worth_taking(mine, theirs)
    }

    fn choose(&mut self, state: &State, legal: &[Action]) -> Action {
        debug_assert!(!legal.is_empty());
        // A discard is decided by the seat that owes it, not the seat to act.
        let me = state.decider() as usize;

        let base_other = self.best_opponent(state, me);
        let mut best = i32::MIN;
        let mut ties = 0u32;
        let mut chosen = legal[0];
        for &a in legal {
            let s = self.score_action(state, me, a, base_other);
            if s > best {
                best = s;
                ties = 1;
                chosen = a;
            } else if s == best {
                // Reservoir sampling over the tied set, so every equally good
                // move is equally likely without collecting them.
                ties += 1;
                if self.rng.below(Stream::Dice, ties) == 0 {
                    chosen = a;
                }
            }
        }
        chosen
    }
}

/// A uniformly random policy, the baseline the heuristic is measured against.
pub struct RandomPolicy {
    rng: Rng,
}

impl RandomPolicy {
    pub fn new(seed: u64) -> Self {
        RandomPolicy {
            rng: Rng::new(seed),
        }
    }
}

impl Policy for RandomPolicy {
    fn choose(&mut self, state: &State, legal: &[Action]) -> Action {
        let _ = state;
        legal[self.rng.below(Stream::Dice, legal.len() as u32) as usize]
    }
}

/// Play one game to its end, returning the winner and the actions taken.
///
/// `policies` is indexed by seat. Every decision is routed to the seat that
/// owns it, which during a discard is not the seat to act.
pub fn play_game(seed: u64, policies: &mut [&mut dyn Policy], cap: usize) -> (Option<u8>, usize) {
    play_game_with(seed, policies, cap, TradeMode::default())
}

/// Play one game with a chosen trade mode.
///
/// After every action the market is settled: an offer is only worth making if
/// somebody is asked whether to take it, and opponents never reach
/// [`Policy::choose`] during another seat's turn.
pub fn play_game_with(
    seed: u64,
    policies: &mut [&mut dyn Policy],
    cap: usize,
    trade: TradeMode,
) -> (Option<u8>, usize) {
    let players = policies.len() as u8;
    let mut state = State::new(players, seed).with_trade_mode(trade);
    let mut buf = Vec::new();
    let mut steps = 0;

    while steps < cap {
        if let Phase::GameOver { winner } = state.phase {
            return (Some(winner), steps);
        }
        state.legal_into(&mut buf);
        if buf.is_empty() {
            break;
        }
        let seat = state.decider() as usize;
        let action = policies[seat].choose(&state, &buf);
        if state.apply(action).is_err() {
            break;
        }
        steps += 1;
        settle_market(&mut state, policies);
    }
    (None, steps)
}

/// Offer everyone entitled to take a live offer the chance to do so.
///
/// Resolves lowest seat first, which is this driver's stand-in for arrival
/// order. The engine re-validates on every acceptance either way, so a seat
/// that loses the race fails cleanly rather than trading against stale state.
pub fn settle_market(state: &mut State, policies: &mut [&mut dyn Policy]) -> u32 {
    if state.trade_mode == TradeMode::Disabled || state.offer_count == 0 {
        return 0;
    }
    let mut done_trades = 0;
    // Bounded so a pathological policy cannot spin here.
    for _ in 0..16 {
        let mut acted = false;
        'outer: for i in 0..state.offer_count as usize {
            for (seat, policy) in policies.iter_mut().enumerate() {
                if state.offers[i].from as usize == seat {
                    continue;
                }
                let take = Action::AcceptTrade {
                    offer: i as u8,
                    by: seat as u8,
                };
                let mut probe = *state;
                if probe.apply(take).is_err() {
                    continue; // not a party to it, or cannot pay
                }
                if policy.accepts(state, seat, i) {
                    if state.apply(take).is_ok() {
                        done_trades += 1;
                    }
                    acted = true;
                    break 'outer;
                }
            }
        }
        if !acted {
            break;
        }
    }
    done_trades
}

/// Salts that keep a policy's RNG off the game's own stream.
///
/// [`Rng::new`] derives every stream from the seed, and both a policy's
/// tie-breaks and the game's dice are drawn from `Stream::Dice`. Seeding a
/// policy with the game seed therefore makes its choices track the dice, a
/// correlation with no place in a strength measurement.
const BOT_SALT: u64 = 0x9E37_79B9_7F4A_7C15;
const RANDOM_SALT: u64 = 0xBF58_476D_1CE4_E5B9;

/// The outcome of a paired duel between the heuristic and random opponents.
#[derive(Clone, Copy, Debug, Default)]
pub struct Duel {
    /// Games played: `boards * seats`.
    pub games: u32,
    /// Games the heuristic won.
    pub wins: u32,
    /// Games that reached a winner rather than the action cap.
    pub finished: u32,
    /// Actions applied across all games.
    pub steps: usize,
    /// Wins split by the seat the heuristic occupied.
    pub wins_by_seat: [u32; MAX_PLAYERS],
    /// Games played from each seat, equal to `boards` by construction.
    pub games_by_seat: [u32; MAX_PLAYERS],
}

impl Duel {
    /// Share of games won.
    pub fn rate(&self) -> f64 {
        self.wins as f64 / self.games.max(1) as f64
    }

    /// Mean actions per game.
    pub fn mean_steps(&self) -> f64 {
        self.steps as f64 / self.games.max(1) as f64
    }

    /// Win rate from each seat, as whole percent.
    pub fn by_seat(&self) -> [u32; MAX_PLAYERS] {
        core::array::from_fn(|i| {
            if self.games_by_seat[i] == 0 {
                0
            } else {
                self.wins_by_seat[i] * 100 / self.games_by_seat[i]
            }
        })
    }
}

/// Play the heuristic against random opponents on `boards` distinct boards,
/// each board once from every seat.
///
/// The pairing matters. Rotating the seat with the board, seat `g % seats` on
/// board `g`, gives each seat a *disjoint* set of boards, so a per-seat
/// breakdown mixes seat effects with board luck and the seats cannot be
/// compared to one another. Replaying the same board from every seat holds the
/// board fixed and leaves the seat as the only difference, which is what the
/// first-player-advantage check (A-4) is actually asking about.
pub fn duel_random(boards: u32, seats: u8, cap: usize, trade: TradeMode) -> Duel {
    let mut out = Duel::default();
    for board in 0..boards {
        // Fixed across the seatings of one board, so the pairing is tight.
        let bot_seed = board as u64 ^ BOT_SALT;
        for bot_seat in 0..seats as usize {
            let mut bot = Heuristic::new(bot_seed);
            let mut randoms: Vec<RandomPolicy> = (0..seats)
                .map(|i| RandomPolicy::new((board as u64 * 31 + i as u64) ^ RANDOM_SALT))
                .collect();

            // Seat table with the bot in `bot_seat`; every other seat keeps
            // the random policy belonging to that seat index.
            let (lo, hi) = randoms.split_at_mut(bot_seat);
            let mut policies: Vec<&mut dyn Policy> = Vec::with_capacity(seats as usize);
            for r in lo.iter_mut() {
                policies.push(r);
            }
            policies.push(&mut bot);
            for r in hi.iter_mut().skip(1) {
                policies.push(r);
            }
            debug_assert_eq!(policies.len(), seats as usize);

            let (winner, steps) = play_game_with(board as u64, &mut policies, cap, trade);
            out.games += 1;
            out.steps += steps;
            out.games_by_seat[bot_seat] += 1;
            if let Some(w) = winner {
                out.finished += 1;
                if w as usize == bot_seat {
                    out.wins += 1;
                    out.wins_by_seat[bot_seat] += 1;
                }
            }
        }
    }
    out
}

/// Victory points for every seat, for reporting.
pub fn final_scores(state: &State) -> [u32; MAX_PLAYERS] {
    let mut out = [0; MAX_PLAYERS];
    for (p, slot) in out.iter_mut().enumerate().take(state.players as usize) {
        *slot = state.victory_points(p);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beats_random_almost_always() {
        // The acceptance bar: random play in this game is genuinely bad, so a
        // merely-functional bot would clear 95%. 99% is the real test.
        let d = duel_random(500, 4, 20_000, TradeMode::default());
        assert_eq!(d.finished, d.games, "every game must reach a winner");
        assert!(
            d.rate() >= 0.99,
            "win rate {:.3} over {} games (mean {:.0} actions)",
            d.rate(),
            d.games,
            d.mean_steps()
        );
    }

    #[test]
    fn no_seat_carries_the_win_rate() {
        // Paired: the same 250 boards played from each seat, so a seat that
        // trailed the others would be a seat effect and not board luck.
        let d = duel_random(250, 4, 20_000, TradeMode::default());
        for (seat, pct) in d.by_seat().iter().enumerate().take(4) {
            assert!(*pct >= 97, "seat {seat} won only {pct}% of its boards");
        }
    }

    #[test]
    fn is_deterministic() {
        // Same seed, same game, which is what makes it usable as a fixed
        // measuring stick.
        for seed in 0..40 {
            let run = || {
                let mut a = Heuristic::new(7);
                let mut b = Heuristic::new(8);
                let mut c = Heuristic::new(9);
                let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c];
                play_game(seed, &mut ps, 20_000)
            };
            assert_eq!(run(), run(), "seed {seed} diverged");
        }
    }

    #[test]
    fn finishes_far_faster_than_random_play() {
        // Random games take ~1058 actions, which is what invalidated the
        // whole-game performance target. Competent play should be far shorter.
        let mut total = 0usize;
        let games = 200;
        for seed in 0..games {
            let mut a = Heuristic::new(seed);
            let mut b = Heuristic::new(seed + 1000);
            let mut c = Heuristic::new(seed + 2000);
            let mut d = Heuristic::new(seed + 3000);
            let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];
            let (winner, steps) = play_game(seed, &mut ps, 20_000);
            assert!(winner.is_some(), "seed {seed} did not finish");
            total += steps;
        }
        let mean = total as f64 / games as f64;
        assert!(mean < 700.0, "bot games average {mean:.0} actions");
    }

    /// Trades actually executed in a self-play game with an open market.
    fn trades_in_selfplay(seed: u64, trade: TradeMode) -> (u32, Option<u8>) {
        let mut a = Heuristic::new(seed);
        let mut b = Heuristic::new(seed + 1000);
        let mut c = Heuristic::new(seed + 2000);
        let mut d = Heuristic::new(seed + 3000);
        let mut ps: Vec<&mut dyn Policy> = vec![&mut a, &mut b, &mut c, &mut d];

        let mut state = State::new(4, seed).with_trade_mode(trade);
        let mut buf = Vec::new();
        let mut trades = 0;
        for _ in 0..20_000 {
            if let Phase::GameOver { winner } = state.phase {
                return (trades, Some(winner));
            }
            state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = state.decider() as usize;
            let a = ps[seat].choose(&state, &buf);
            if state.apply(a).is_err() {
                break;
            }
            trades += settle_market(&mut state, &mut ps);
            state.assert_invariants();
        }
        (trades, None)
    }

    #[test]
    fn bots_actually_trade_with_each_other() {
        // Without this the market is decoration: strategies tuned in a game
        // where nobody trades would not transfer to real play.
        let mut games_with_trades = 0;
        let mut total = 0;
        for seed in 0..60 {
            let (trades, winner) = trades_in_selfplay(seed, TradeMode::Full);
            assert!(winner.is_some(), "seed {seed} did not finish");
            total += trades;
            games_with_trades += (trades > 0) as u32;
        }
        assert!(
            games_with_trades >= 50,
            "only {games_with_trades}/60 games saw a trade ({total} total)"
        );
    }

    #[test]
    fn most_offers_are_priced_to_be_taken() {
        // Not merely "a trade happened somewhere", which a market can pass while
        // being decoration: an offer nobody will take is a wasted turn, and a bot
        // that makes hundreds of them is not trading, it is talking to itself.
        let (mut offers, mut taken) = (0u32, 0u32);
        for seed in 0..20u64 {
            let mut state = State::new(4, seed).with_trade_mode(TradeMode::Full);
            let mut bots: Vec<Heuristic> = (0..4).map(|s| Heuristic::new(seed * 13 + s)).collect();
            let mut buf = Vec::new();
            for _ in 0..20_000 {
                if matches!(state.phase, Phase::GameOver { .. }) {
                    break;
                }
                state.legal_into(&mut buf);
                if buf.is_empty() {
                    break;
                }
                let seat = state.decider() as usize;
                let action = {
                    let mut ps: Vec<&mut dyn Policy> =
                        bots.iter_mut().map(|b| b as &mut dyn Policy).collect();
                    ps[seat].choose(&state, &buf)
                };
                offers += u32::from(matches!(action, Action::ProposeTrade { .. }));
                if state.apply(action).is_err() {
                    break;
                }
                let mut ps: Vec<&mut dyn Policy> =
                    bots.iter_mut().map(|b| b as &mut dyn Policy).collect();
                taken += settle_market(&mut state, &mut ps);
            }
        }
        assert!(offers > 100, "only {offers} offers to judge");
        assert!(
            taken * 100 >= offers * 30,
            "{taken} of {offers} offers were taken, which is a market in name only"
        );
    }

    #[test]
    fn an_open_market_does_not_break_the_win_rate() {
        // Trading must not make the bot worse than it was without it.
        let d = duel_random(200, 4, 20_000, TradeMode::Full);
        assert!(
            d.rate() >= 0.99,
            "win rate {:.3} with an open market",
            d.rate()
        );
    }

    #[test]
    fn always_returns_an_offered_action() {
        let mut bot = Heuristic::new(1);
        let mut state = State::new(4, 5);
        let mut buf = Vec::new();
        for _ in 0..2_000 {
            if matches!(state.phase, Phase::GameOver { .. }) {
                break;
            }
            state.legal_into(&mut buf);
            let a = bot.choose(&state, &buf);
            assert!(buf.contains(&a), "chose an action it was not offered");
            state.apply(a).expect("chosen action must be legal");
            state.assert_invariants();
        }
    }
}
