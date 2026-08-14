//! One local game: the human at seat 0, heuristic bots elsewhere.
//!
//! **The browser is served a redacted view, never the state.** Everything the
//! page receives goes through [`carranta_record::fog`], the same projection a
//! real server would use — so the client physically cannot be sent another
//! seat's cards or the deck order, because the type it is built from has no
//! field for them. That is worth doing here rather than later: a local UI that
//! reads the raw state would grow a habit the server then has to unpick.

use carranta_bot::{Heuristic, Policy};
use carranta_core::action::{Action, Illegal};
use carranta_core::state::{MAX_OFFERS, MAX_PLAYERS, Phase, State, TradeMode};
use carranta_record::fog::{Fog, Viewer, fog};

/// The seat a person plays.
pub const HUMAN: u8 = 0;

/// Why an action was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The page acted on a position that has since moved on.
    Stale,
    /// No such choice was offered.
    NoSuchChoice,
    /// The engine rejected it. Should not happen — every choice offered comes
    /// from the engine's own legal set — so it is surfaced rather than hidden.
    Illegal(Illegal),
}

/// A live game.
pub struct Session {
    state: State,
    bots: Vec<Heuristic>,
    /// Bumped on every applied action, so a click made against a stale board is
    /// refused rather than applied to a different position.
    version: u64,
    log: Vec<String>,
    /// Offers the human has already waved away, so they are asked once.
    declined: [bool; MAX_OFFERS],
    seed: u64,
    /// When this game was dealt. The clock belongs to the server rather than
    /// to the page, so reloading the browser does not restart it.
    started: std::time::Instant,
}

impl Session {
    pub fn new(seats: u8, seed: u64, mode: TradeMode) -> Self {
        let seats = seats.clamp(3, MAX_PLAYERS as u8);
        Session {
            state: State::new(seats, seed).with_trade_mode(mode),
            bots: (0..seats)
                .map(|s| Heuristic::new(seed.wrapping_mul(31).wrapping_add(s as u64 + 1)))
                .collect(),
            version: 0,
            log: vec![format!(
                "New game — {seats} seats, {mode:?} market, seed {seed}"
            )],
            declined: [false; MAX_OFFERS],
            seed,
            started: std::time::Instant::now(),
        }
    }

    /// Whole seconds since the game was dealt.
    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// What the human is entitled to see.
    pub fn view(&self) -> Fog {
        fog(&self.state, Viewer::Seat(HUMAN))
    }

    /// The choices to put in front of the human, in a stable order.
    ///
    /// Empty while it is a bot's turn and nothing is being asked of the human.
    pub fn choices(&self) -> Vec<Choice> {
        if matches!(self.state.phase, Phase::GameOver { .. }) {
            return Vec::new();
        }
        if self.state.decider() == HUMAN {
            let mut buf = Vec::new();
            self.state.legal_into(&mut buf);
            return buf.into_iter().map(Choice::Play).collect();
        }
        // Not their turn — but an offer may be waiting for them.
        let mut out: Vec<Choice> = self
            .open_offers()
            .into_iter()
            .map(|i| {
                Choice::Play(Action::AcceptTrade {
                    offer: i,
                    by: HUMAN,
                })
            })
            .collect();
        if !out.is_empty() {
            out.push(Choice::Decline);
        }
        out
    }

    /// Whether the human could put an offer on the table at all.
    ///
    /// Composing one is pointless if the market is closed, full, or the
    /// per-turn allowance is spent, and a form that cannot succeed is worse
    /// than one that is not shown.
    pub fn can_propose(&self) -> bool {
        if self.state.trade_mode == TradeMode::Disabled {
            return false;
        }
        if !matches!(self.state.phase, Phase::Action) {
            return false;
        }
        // A probe rather than a second copy of the rules: whatever the engine
        // would accept is what the form should allow.
        for r in 0..5 {
            if self.state.hand[HUMAN as usize][r] == 0 {
                continue;
            }
            let mut give = [0u8; 5];
            give[r] = 1;
            let mut want = [0u8; 5];
            want[(r + 1) % 5] = 1;
            let mut probe = self.state;
            if probe
                .apply(Action::ProposeTrade {
                    by: HUMAN,
                    to: None,
                    give,
                    want,
                })
                .is_ok()
            {
                return true;
            }
        }
        false
    }

    /// Offers the human could take and has not already waved away.
    fn open_offers(&self) -> Vec<u8> {
        if self.state.trade_mode == TradeMode::Disabled {
            return Vec::new();
        }
        (0..self.state.offer_count)
            .filter(|&i| !self.declined[i as usize])
            .filter(|&i| {
                let mut probe = self.state;
                probe
                    .apply(Action::AcceptTrade {
                        offer: i,
                        by: HUMAN,
                    })
                    .is_ok()
            })
            .collect()
    }

    /// Apply the human's choice, then let the bots run on.
    pub fn act(&mut self, index: usize, version: u64) -> Result<(), Refused> {
        if version != self.version {
            return Err(Refused::Stale);
        }
        let choice = self
            .choices()
            .into_iter()
            .nth(index)
            .ok_or(Refused::NoSuchChoice)?;

        match choice {
            Choice::Decline => {
                for i in self.open_offers() {
                    self.declined[i as usize] = true;
                }
                self.log.push("You declined the open offers".to_string());
            }
            Choice::Play(action) => {
                // Named before it is applied: a phrase describes the position
                // the action was taken in, not the one it produced.
                let phrase = describe(&action, &self.state, HUMAN as usize);
                self.state.apply(action).map_err(Refused::Illegal)?;
                self.version += 1;
                self.log.push(format!("You: {phrase}"));
                self.forget_declines();
            }
        }
        self.finish_move();
        Ok(())
    }

    /// Compose and make an offer of any shape, to anyone (R-7.19).
    ///
    /// Separate from [`Session::act`] because the engine *generates* only open,
    /// single-type offers — a bound on enumeration, not on legality. A person
    /// composing "two wood and a brick for an ore, and only to seat 2" is
    /// making a perfectly legal offer that simply was not in the generated set,
    /// so it is built here and handed to the engine, which validates it exactly
    /// as it would any other.
    ///
    /// `to` is `None` for the open market and `Some(seat)` to address it.
    pub fn propose(
        &mut self,
        to: Option<u8>,
        give: [u8; 5],
        want: [u8; 5],
        version: u64,
    ) -> Result<(), Refused> {
        if version != self.version {
            return Err(Refused::Stale);
        }
        let action = Action::ProposeTrade {
            by: HUMAN,
            to,
            give,
            want,
        };
        let phrase = describe(&action, &self.state, HUMAN as usize);
        self.state.apply(action).map_err(Refused::Illegal)?;
        self.version += 1;
        self.log.push(format!("You: {phrase}"));
        self.forget_declines();
        self.finish_move();
        Ok(())
    }

    /// Put the market to the other seats, then let them play on.
    ///
    /// The settle has to happen even when it is still the human's turn: an
    /// offer nobody is asked about is not an offer, and before this the bots
    /// only ever saw one after the human had ended their turn.
    fn finish_move(&mut self) {
        self.settle_between_bots();
        self.run_bots();
    }

    /// Advance until the human has something to decide, or the game ends.
    fn run_bots(&mut self) {
        let mut buf = Vec::new();
        for _ in 0..20_000 {
            if matches!(self.state.phase, Phase::GameOver { .. }) {
                break;
            }
            if self.state.decider() == HUMAN || !self.choices().is_empty() {
                break;
            }
            self.state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = self.state.decider() as usize;
            let action = self.bots[seat].choose(&self.state, &buf);
            let phrase = describe(&action, &self.state, seat);
            if self.state.apply(action).is_err() {
                break;
            }
            self.version += 1;
            if worth_logging(&action) {
                self.log.push(format!("Seat {seat}: {phrase}"));
            }
            self.forget_declines();
            self.settle_between_bots();
        }
        if let Phase::GameOver { winner } = self.state.phase {
            let who = if winner == HUMAN {
                "You win".to_string()
            } else {
                format!("Seat {winner} wins")
            };
            if self.log.last().map(|l| l.as_str()) != Some(who.as_str()) {
                self.log.push(who);
            }
        }
    }

    /// Let bots take each other's offers. The human is asked separately, by
    /// being offered the choice rather than answered on their behalf.
    fn settle_between_bots(&mut self) {
        if self.state.trade_mode == TradeMode::Disabled || self.state.offer_count == 0 {
            return;
        }
        for _ in 0..16 {
            let mut acted = false;
            'outer: for i in 0..self.state.offer_count {
                for seat in 1..self.state.players {
                    if self.state.offers[i as usize].from == seat {
                        continue;
                    }
                    let take = Action::AcceptTrade { offer: i, by: seat };
                    let mut probe = self.state;
                    if probe.apply(take).is_err() {
                        continue;
                    }
                    if self.bots[seat as usize].accepts(&self.state, seat as usize, i as usize) {
                        if self.state.apply(take).is_ok() {
                            self.version += 1;
                            self.log.push(format!("Seat {seat} took an offer"));
                            self.forget_declines();
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
    }

    /// A decline applies to the offers that were on the table at the time.
    /// Once the market has moved, ask again.
    fn forget_declines(&mut self) {
        self.declined = [false; MAX_OFFERS];
    }
}

/// Something the human can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Play(Action),
    /// Wave away the offers currently on the table. Not an engine action —
    /// declining changes no state, which is exactly why it needs representing
    /// here rather than there.
    Decline,
}

impl Choice {
    /// Which board feature this choice attaches to, for highlighting.
    pub fn target(&self) -> Target {
        match self {
            Choice::Play(a) => match *a {
                Action::PlaceRoad(e) | Action::BuildRoad(e) => Target::Edge(e),
                Action::PlaceSettlement(v) | Action::BuildSettlement(v) | Action::BuildCity(v) => {
                    Target::Vertex(v)
                }
                Action::MoveRobber { hex, .. } => Target::Hex(hex),
                _ => Target::None,
            },
            Choice::Decline => Target::None,
        }
    }

    pub fn label(&self, state: &State) -> String {
        match self {
            Choice::Play(a) => describe(a, state, state.decider() as usize),
            Choice::Decline => "No thanks".to_string(),
        }
    }

    /// A coarse grouping, so the page can put builds under buildings and cards
    /// under cards.
    pub fn group(&self) -> &'static str {
        match self {
            Choice::Play(a) => match *a {
                Action::PlaceSettlement(_) | Action::PlaceRoad(_) => "setup",
                Action::Roll => "roll",
                Action::Discard { .. } => "discard",
                Action::MoveRobber { .. } => "robber",
                Action::BuildRoad(_)
                | Action::BuildSettlement(_)
                | Action::BuildCity(_)
                | Action::BuyDev => "build",
                Action::PlayMilitia
                | Action::PlayRoadBuilding
                | Action::PlayInvention(_)
                | Action::PlayMonopoly(_) => "card",
                Action::Trade { .. }
                | Action::ProposeTrade { .. }
                | Action::AcceptTrade { .. }
                | Action::WithdrawTrade { .. } => "trade",
                Action::EndTurn => "turn",
            },
            Choice::Decline => "trade",
        }
    }
}

/// What a choice points at on the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Vertex(u8),
    Edge(u8),
    Hex(u8),
    None,
}

const RESOURCE_NAMES: [&str; 5] = ["brick", "wood", "wool", "wheat", "ore"];

fn cards(counts: &[u8; 5]) -> String {
    let parts: Vec<String> = counts
        .iter()
        .enumerate()
        .filter(|&(_, &n)| n > 0)
        .map(|(r, &n)| format!("{n} {}", RESOURCE_NAMES[r]))
        .collect();
    if parts.is_empty() {
        "nothing".to_string()
    } else {
        parts.join(", ")
    }
}

/// A phrase for one action, for buttons and for the log.
///
/// `state` is the position the action is taken in and `actor` the seat taking
/// it, because a supply trade cannot be named without them: the rate depends
/// on which ports that seat holds.
pub fn describe(a: &Action, state: &State, actor: usize) -> String {
    match *a {
        Action::PlaceSettlement(v) => format!("Place settlement at {v}"),
        Action::PlaceRoad(e) => format!("Place road at {e}"),
        Action::Roll => "Roll the dice".to_string(),
        Action::Discard { resource, .. } => {
            format!("Discard {}", RESOURCE_NAMES[resource as usize])
        }
        Action::MoveRobber { hex, victim } => match victim {
            Some(v) => format!("Move robber to {hex} and rob seat {v}"),
            None => format!("Move robber to {hex}"),
        },
        Action::BuildRoad(e) => format!("Build road at {e}"),
        Action::BuildSettlement(v) => format!("Build settlement at {v}"),
        Action::BuildCity(v) => format!("Upgrade to city at {v}"),
        Action::BuyDev => "Buy a development card".to_string(),
        Action::PlayMilitia => "Play Militia".to_string(),
        Action::PlayRoadBuilding => "Play Road Building".to_string(),
        Action::PlayInvention([a, b]) => format!(
            "Play Invention — take {} and {}",
            RESOURCE_NAMES[a as usize], RESOURCE_NAMES[b as usize]
        ),
        Action::PlayMonopoly(r) => format!("Play Monopoly on {}", RESOURCE_NAMES[r as usize]),
        Action::Trade { give, take } => {
            // Four-for-one needs no port — it is the bank, open to everyone
            // always (R-7.6). Calling that "at the port" misnames the trade a
            // player makes most often, and does it while standing nowhere near
            // a port. Only the improved rates are a port's doing (R-7.7, R-7.8).
            let rate = state.trade_rate(actor, give);
            let with = if rate == 4 {
                "with the bank"
            } else {
                "at the port"
            };
            format!(
                "Trade {rate} {} for 1 {} {with}",
                RESOURCE_NAMES[give as usize], RESOURCE_NAMES[take as usize]
            )
        }
        Action::ProposeTrade { to, give, want, .. } => match to {
            Some(seat) => format!("Offer seat {seat} {} for {}", cards(&give), cards(&want)),
            None => format!("Offer {} for {}", cards(&give), cards(&want)),
        },
        Action::AcceptTrade { offer, .. } => format!("Accept offer {offer}"),
        Action::WithdrawTrade { offer, .. } => format!("Withdraw offer {offer}"),
        Action::EndTurn => "End turn".to_string(),
    }
}

/// Keep the log readable: the market is chatty and mostly noise to a reader.
fn worth_logging(a: &Action) -> bool {
    !matches!(
        a,
        Action::ProposeTrade { .. } | Action::WithdrawTrade { .. } | Action::Discard { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use carranta_core::action::Illegal;
    use carranta_core::state::Resource;
    use carranta_core::topology::{iter_vertices, vertex_bit};

    #[test]
    fn a_four_for_one_is_a_bank_trade_and_says_the_rate() {
        let mut s = State::new(4, 11);
        let a = Action::Trade {
            give: Resource::Ore,
            take: Resource::Wheat,
        };
        assert_eq!(
            describe(&a, &s, 0),
            "Trade 4 ore for 1 wheat with the bank",
            "no port is involved in a four-for-one, and none is needed"
        );

        // A building on a generic port is what buys the better rate, and only
        // then does a port have anything to do with it.
        let generic = iter_vertices(s.ports[0]).next().expect("a 3:1 port exists");
        s.settlements[0] |= vertex_bit(generic);
        assert_eq!(describe(&a, &s, 0), "Trade 3 ore for 1 wheat at the port");

        // The same trade from a seat without that building is still the bank's.
        assert_eq!(describe(&a, &s, 1), "Trade 4 ore for 1 wheat with the bank");
    }

    /// Play on until the human is building and trading.
    fn reach_action_phase(s: &mut Session) {
        for _ in 0..400 {
            if matches!(s.state.phase, Phase::Action) && s.state.decider() == HUMAN {
                return;
            }
            let choices = s.choices();
            if choices.is_empty() {
                return;
            }
            let pick = choices
                .iter()
                .position(|c| matches!(c, Choice::Play(Action::Roll)))
                .unwrap_or(0);
            let v = s.version();
            let _ = s.act(pick, v);
        }
    }

    impl Session {
        /// Put cards in the human's hand, keeping the supply consistent.
        fn set_hand(&mut self, cards: [u8; 5]) {
            for (r, &wanted) in cards.iter().enumerate() {
                let held = self.state.hand[HUMAN as usize][r];
                self.state.supply[r] += held;
                let give = wanted.min(self.state.supply[r]);
                self.state.hand[HUMAN as usize][r] = give;
                self.state.supply[r] -= give;
            }
        }
    }

    fn deal(s: &mut Session, cards: [u8; 5]) {
        s.set_hand(cards);
    }

    #[test]
    fn a_new_game_puts_the_human_on_the_clock() {
        let s = Session::new(4, 7, TradeMode::Full);
        assert_eq!(s.state().decider(), HUMAN, "seat 0 opens the setup");
        let choices = s.choices();
        assert!(!choices.is_empty());
        // Setup starts with a settlement, and each choice points at a vertex.
        assert!(
            choices
                .iter()
                .all(|c| matches!(c.target(), Target::Vertex(_)))
        );
    }

    #[test]
    fn the_browser_is_never_handed_another_seat_s_cards() {
        // The point of serving through the fog: what the page receives has no
        // field for another player's hand, so it cannot leak by oversight.
        let mut s = Session::new(4, 3, TradeMode::Full);
        for _ in 0..40 {
            if s.choices().is_empty() {
                break;
            }
            s.act(0, s.version()).expect("play");
        }
        let view = s.view();
        let own = view.own.expect("the human sees their own hand");
        assert_eq!(own.seat, HUMAN);
        // Others are counts only.
        assert!(view.hand_size.iter().any(|&n| n > 0));
        assert_eq!(view.own.map(|o| o.seat), Some(HUMAN));
    }

    #[test]
    fn a_click_against_a_stale_board_is_refused() {
        let mut s = Session::new(4, 11, TradeMode::Disabled);
        let stale = s.version();
        s.act(0, stale).expect("first click lands");
        assert_eq!(s.act(0, stale), Err(Refused::Stale));
    }

    #[test]
    fn a_choice_that_was_never_offered_is_refused() {
        let mut s = Session::new(4, 12, TradeMode::Disabled);
        let v = s.version();
        assert_eq!(s.act(9_999, v), Err(Refused::NoSuchChoice));
    }

    #[test]
    fn every_offered_choice_is_one_the_engine_accepts() {
        // The page only ever shows choices the engine generated, so applying
        // any of them must succeed. A failure here would mean the UI could
        // offer an illegal move.
        for seed in 0..12 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..300 {
                let choices = s.choices();
                if choices.is_empty() {
                    break;
                }
                let pick = (s.version() as usize) % choices.len();
                match s.act(pick, s.version()) {
                    Ok(()) => {}
                    Err(e) => panic!("seed {seed}: offered choice refused: {e:?}"),
                }
            }
        }
    }

    #[test]
    fn a_game_can_be_played_to_the_end_through_the_interface() {
        // End to end: only ever clicking things the interface offers must
        // reach a finished game, not a stuck one.
        let mut s = Session::new(4, 5, TradeMode::Full);
        let mut clicks = 0;
        while clicks < 4_000 {
            let choices = s.choices();
            if choices.is_empty() {
                break;
            }
            // Prefer ending the turn when it is offered, so the game moves on.
            let pick = choices
                .iter()
                .position(|c| matches!(c, Choice::Play(Action::EndTurn)))
                .unwrap_or(0);
            s.act(pick, s.version()).expect("play");
            clicks += 1;
        }
        assert!(
            matches!(s.state().phase, Phase::GameOver { .. }),
            "did not finish after {clicks} clicks"
        );
        assert!(s.log().len() > 20);
    }

    #[test]
    fn declining_leaves_the_position_untouched() {
        let mut s = Session::new(4, 21, TradeMode::Full);
        // Play until an offer is put to the human.
        for _ in 0..600 {
            let choices = s.choices();
            if choices.contains(&Choice::Decline) {
                let before = *s.state();
                let i = choices.iter().position(|c| *c == Choice::Decline).unwrap();
                let version = s.version();
                s.act(i, version).expect("decline");
                // Declining changes nothing itself, though the bots then move
                // on — so what must hold is that no cards changed hands at the
                // moment of declining.
                assert_eq!(before.hand[HUMAN as usize], s.state().hand[HUMAN as usize]);
                return;
            }
            if choices.is_empty() {
                break;
            }
            let pick = choices
                .iter()
                .position(|c| matches!(c, Choice::Play(Action::EndTurn)))
                .unwrap_or(0);
            s.act(pick, s.version()).expect("play");
        }
    }

    #[test]
    fn a_composed_offer_may_take_a_shape_the_engine_never_generates() {
        // The point of the composer. Generation is capped at one resource type
        // a side; legality is not. A person offering two of one thing and one
        // of another is making an ordinary offer that simply was not enumerated.
        let mut s = Session::new(4, 31, TradeMode::Full);
        reach_action_phase(&mut s);
        deal(&mut s, [2, 2, 0, 0, 0]);

        // What reaches the market is checked on a copy of the position rather
        // than after the fact, because `propose` lets the bots run and one of
        // them may take the offer — a fine outcome for a game, and a poor one
        // for a test of what was offered, which would then depend on how the
        // seat to the left happens to value ore.
        let before = *s.state();
        let mut probe = before;
        probe
            .apply(Action::ProposeTrade {
                by: HUMAN,
                to: None,
                give: [2, 1, 0, 0, 0],
                want: [0, 0, 0, 0, 1],
            })
            .expect("a mixed offer is legal");
        let mine = probe.offers[..probe.offer_count as usize]
            .iter()
            .find(|o| o.from == HUMAN)
            .expect("the offer reached the market");
        assert_eq!(mine.give, [2, 1, 0, 0, 0]);
        assert_eq!(mine.want, [0, 0, 0, 0, 1]);

        // And the composer takes it too, which is the path a person walks.
        let v = s.version();
        s.propose(None, [2, 1, 0, 0, 0], [0, 0, 0, 0, 1], v)
            .expect("the composer accepts a mixed offer");

        // It is a shape `legal_into` would never have produced.
        let mut buf = Vec::new();
        before.legal_into(&mut buf);
        assert!(
            !buf.iter().any(|a| matches!(
                a,
                Action::ProposeTrade { give, .. } if *give == [2, 1, 0, 0, 0]
            )),
            "generation should not enumerate mixed sides"
        );
    }

    #[test]
    fn a_composed_offer_is_still_judged_by_the_rules() {
        let mut s = Session::new(4, 32, TradeMode::Full);
        reach_action_phase(&mut s);
        deal(&mut s, [1, 0, 0, 0, 0]);
        let v = s.version();

        // A gift, a self-trade, and cards not held are all refused (R-7.5,
        // R-7.18) — the composer does not get its own rulebook.
        assert!(matches!(
            s.propose(None, [1, 0, 0, 0, 0], [0; 5], v),
            Err(Refused::Illegal(Illegal::EmptySide))
        ));
        assert!(matches!(
            s.propose(None, [1, 0, 0, 0, 0], [1, 0, 0, 0, 0], v),
            Err(Refused::Illegal(Illegal::TypeOverlap))
        ));
        assert!(matches!(
            s.propose(None, [9, 0, 0, 0, 0], [0, 1, 0, 0, 0], v),
            Err(Refused::Illegal(Illegal::CannotAfford))
        ));
        assert_eq!(s.version(), v, "a refused offer changes nothing");
    }

    #[test]
    fn a_composed_offer_against_a_stale_board_is_refused() {
        let mut s = Session::new(4, 33, TradeMode::Full);
        reach_action_phase(&mut s);
        deal(&mut s, [1, 0, 0, 0, 0]);
        assert_eq!(
            s.propose(None, [1, 0, 0, 0, 0], [0, 1, 0, 0, 0], s.version() + 5),
            Err(Refused::Stale)
        );
    }

    #[test]
    fn an_offer_is_put_to_the_other_seats_at_once() {
        // Before this, a human's offer sat untouched until they ended their
        // turn: `run_bots` returns immediately while it is still the human's
        // move, and the settle lived inside that loop. An offer nobody is
        // asked about is not an offer.
        let mut s = Session::new(4, 34, TradeMode::Full);
        reach_action_phase(&mut s);
        deal(&mut s, [3, 3, 3, 3, 3]);

        // Something generous enough that a bot should take it.
        let v = s.version();
        s.propose(None, [3, 0, 0, 0, 0], [0, 0, 0, 0, 1], v)
            .expect("offer");
        let settled = !s.state().offers[..s.state().offer_count as usize]
            .iter()
            .any(|o| o.from == HUMAN);
        let asked = s.log().iter().any(|l| l.contains("took an offer"));
        assert!(
            settled || asked || s.state().decider() != HUMAN,
            "the offer was never put to anyone"
        );
    }

    #[test]
    fn composing_is_offered_only_when_it_could_succeed() {
        let mut off = Session::new(4, 35, TradeMode::Disabled);
        reach_action_phase(&mut off);
        assert!(!off.can_propose(), "no market, no form");

        let mut open = Session::new(4, 35, TradeMode::Full);
        reach_action_phase(&mut open);
        deal(&mut open, [1, 1, 0, 0, 0]);
        assert!(open.can_propose());

        // With nothing to give there is nothing to offer.
        let mut broke = Session::new(4, 35, TradeMode::Full);
        reach_action_phase(&mut broke);
        broke.set_hand([0; 5]);
        assert!(!broke.can_propose());
    }

    #[test]
    fn every_action_has_a_phrase() {
        // A button with an empty label is a bug the compiler cannot catch.
        let state = State::new(4, 7);
        let all = [
            Action::PlaceSettlement(1),
            Action::PlaceRoad(2),
            Action::Roll,
            Action::Discard {
                player: 0,
                resource: Resource::Ore,
            },
            Action::MoveRobber {
                hex: 3,
                victim: Some(1),
            },
            Action::MoveRobber {
                hex: 3,
                victim: None,
            },
            Action::BuildRoad(4),
            Action::BuildSettlement(5),
            Action::BuildCity(6),
            Action::BuyDev,
            Action::PlayMilitia,
            Action::PlayRoadBuilding,
            Action::PlayInvention([Resource::Ore, Resource::Wood]),
            Action::PlayMonopoly(Resource::Wool),
            Action::Trade {
                give: Resource::Ore,
                take: Resource::Brick,
            },
            Action::ProposeTrade {
                by: 0,
                to: None,
                give: [1, 0, 0, 0, 0],
                want: [0, 0, 0, 0, 1],
            },
            Action::ProposeTrade {
                by: 0,
                to: Some(2),
                give: [1, 0, 0, 0, 0],
                want: [0, 0, 0, 0, 1],
            },
            Action::AcceptTrade { offer: 0, by: 1 },
            Action::WithdrawTrade { offer: 0, by: 0 },
            Action::EndTurn,
        ];
        for a in all {
            let phrase = describe(&a, &state, 0);
            assert!(!phrase.is_empty(), "{a:?} has no phrase");
            assert!(!Choice::Play(a).group().is_empty());
        }
    }
}
