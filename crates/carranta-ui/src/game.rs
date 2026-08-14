//! One local game: the human at seat 0, heuristic bots elsewhere.
//!
//! **The browser is served a redacted view, never the state.** Everything the
//! page receives goes through [`carranta_record::fog`], the same projection a
//! real server would use, so the client physically cannot be sent another
//! seat's cards or the deck order, because the type it is built from has no
//! field for them. That is worth doing here rather than later: a local UI that
//! reads the raw state would grow a habit the server then has to unpick.

use carranta_bot::{Heuristic, Policy};
use carranta_core::action::{Action, Illegal};
use carranta_core::rng::{Rng, Stream};
use carranta_core::state::{MAX_OFFERS, MAX_PLAYERS, Phase, State, TradeMode};
use carranta_record::fog::{Fog, Viewer, fog};

/// The seat a person plays.
pub const HUMAN: u8 = 0;

/// A seed as something a person can read out loud, copy, or type back in.
///
/// Base 36 rather than decimal, grouped, because twenty digits in a row cannot
/// be checked by eye and cannot be dictated. The width is thirteen characters
/// because that is what a u64 takes in base 36 and the engine seeds from a
/// u64. Padding it out to look longer would be claiming entropy that is not
/// there.
pub fn seed_code(seed: u64) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = [b'0'; 13];
    let mut n = seed;
    for slot in buf.iter_mut().rev() {
        *slot = DIGITS[(n % 36) as usize];
        n /= 36;
    }
    let s = std::str::from_utf8(&buf).unwrap_or("0");
    format!("{}-{}-{}", &s[0..5], &s[5..9], &s[9..13])
}

/// The inverse, tolerant of how it comes back: hyphens optional, case ignored.
pub fn parse_seed(text: &str) -> Option<u64> {
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    u64::from_str_radix(&cleaned, 36).ok()
}

/// How long people get to think.
///
/// Kept at the session layer on purpose. A clock is a house rule about how long
/// anyone is prepared to sit and wait, not a rule of the game, so nothing in
/// `carranta-core` knows one exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    /// Take as long as you like.
    Off,
    /// A fresh allowance every turn. Run it out and your turn is ended for you.
    PerTurn(u64),
    /// A chess clock: one bank each for the whole game, draining only while it
    /// is your move, with an increment credited back for each turn you finish.
    /// Empty the bank and your turns end the moment they begin.
    ///
    /// The increment is what makes it a chess clock rather than a sudden-death
    /// timer. Without one a long game is decided by the clock rather than by
    /// the board; with one, a player who keeps moving keeps playing.
    Chess { bank: u64, increment: u64 },
}

impl Clock {
    /// What the lobby sends: a name and a number of seconds.
    pub fn parse(kind: Option<&str>, secs: u64, increment: u64) -> Clock {
        match (kind, secs) {
            (_, 0) => Clock::Off,
            (Some("chess"), bank) => Clock::Chess { bank, increment },
            (Some("turn"), s) => Clock::PerTurn(s),
            _ => Clock::Off,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Clock::Off => "off",
            Clock::PerTurn(_) => "turn",
            Clock::Chess { .. } => "chess",
        }
    }

    pub fn secs(self) -> u64 {
        match self {
            Clock::Off => 0,
            Clock::PerTurn(s) => s,
            Clock::Chess { bank, .. } => bank,
        }
    }

    /// Seconds credited back for finishing a turn. Zero for anything but a
    /// chess clock, and legitimately zero for a sudden-death one.
    pub fn increment(self) -> u64 {
        match self {
            Clock::Chess { increment, .. } => increment,
            _ => 0,
        }
    }
}

/// One line of history.
///
/// Carries who did it and which turn it happened in, so the page can group and
/// colour rather than parsing sentences back apart.
pub struct LogLine {
    pub turn: u32,
    /// Whether this happened while the board was still being dealt.
    pub setup: bool,
    /// The seat responsible. `None` for things the table did rather than a
    /// player: the deal, the result.
    pub seat: Option<u8>,
    pub text: String,
}

/// The same action, phrased for the history rather than for a button.
///
/// A button has to tell two otherwise identical choices apart, so it carries
/// the vertex or edge. The history does not: the board already shows where the
/// road went, and "Build road at 68" asks the reader to hold a number that
/// means nothing to them.
fn log_phrase(a: &Action, state: &State, seat: usize) -> String {
    // The dice are read after the roll, so the caller rewrites that one; see
    // `rolled`.
    match *a {
        Action::PlaceSettlement(_) => "Place a settlement".to_string(),
        Action::PlaceRoad(_) => "Place a road".to_string(),
        Action::BuildRoad(_) => "Build a road".to_string(),
        Action::BuildSettlement(_) => "Build a settlement".to_string(),
        Action::BuildCity(_) => "Upgrade to a city".to_string(),
        Action::MoveRobber { victim, .. } => match victim {
            Some(v) => format!("Move the robber onto seat {v}"),
            None => "Move the robber".to_string(),
        },
        ref other => describe(other, state, seat),
    }
}

/// The roll, once it has happened.
///
/// `log_phrase` runs before the action is applied, when the dice still hold the
/// previous turn's numbers, so the one phrase that depends on the outcome is
/// built afterwards instead.
fn rolled(state: &State) -> String {
    let [a, b] = state.dice;
    // The total leads, because that is the number the board answers to. The
    // two dice follow in brackets. "Roll 4 and 5, 9" read as three numbers of
    // equal standing, which is not what a roll is.
    format!("Rolled {} ({a}, {b})", a + b)
}

/// What each seat gained between two snapshots, as words.
///
/// Only ever called where the change is **public**: production on a roll, the
/// grant from the second settlement, and a trade being taken. A robber steal
/// moves a card too, and deliberately is not reported this way, because which
/// card it was is not something the table gets to know.
fn gains(before: &[[u8; 5]; MAX_PLAYERS], after: &[[u8; 5]; MAX_PLAYERS], seat: usize) -> String {
    let mut parts = Vec::new();
    for r in 0..5 {
        let up = after[seat][r].saturating_sub(before[seat][r]);
        if up > 0 {
            parts.push(format!("{up} {}", RESOURCE_NAMES[r]));
        }
    }
    parts.join(", ")
}

/// Whether a seat's hand grew by exactly one card without it being said which.
///
/// A robber steal is public in the fact and private in the detail: the table
/// sees a card cross, and only the two of them see what it was. The count
/// moves, so the count is what gets reported.
fn took_a_card(
    before: &[[u8; 5]; MAX_PLAYERS],
    after: &[[u8; 5]; MAX_PLAYERS],
    seat: usize,
) -> bool {
    let sum = |h: &[u8; 5]| h.iter().map(|&n| n as u32).sum::<u32>();
    sum(&after[seat]) > sum(&before[seat])
}

/// Why an action was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The page acted on a position that has since moved on.
    Stale,
    /// No such choice was offered.
    NoSuchChoice,
    /// The engine rejected it. Should not happen. Every choice offered comes
    /// from the engine's own legal set, so it is surfaced rather than hidden.
    Illegal(Illegal),
}

/// A live game.
pub struct Session {
    state: State,
    bots: Vec<Heuristic>,
    /// Bumped on every applied action, so a click made against a stale board is
    /// refused rather than applied to a different position.
    version: u64,
    log: Vec<LogLine>,
    /// Offers the human has already waved away, so they are asked once.
    declined: [bool; MAX_OFFERS],
    seed: u64,
    /// When this game was dealt. The clock belongs to the server rather than
    /// to the page, so reloading the browser does not restart it.
    started: std::time::Instant,
    clock: Clock,
    /// When the running seat's *turn* began. Reset only when the turn actually
    /// changes hands. A per-turn allowance must not refill because something
    /// happened mid-turn, or a clock that rolled for you would hand you a fresh
    /// allowance for doing nothing.
    turn_began: std::time::Instant,
    /// When time was last charged to anyone. Moves on at every settle.
    last_settle: std::time::Instant,
    /// Whose clock is running. Not always the decider: bots resolve inside the
    /// request that hands them the turn, so the seat being charged is whoever
    /// the clock was last handed to.
    charged_to: u8,
    /// Time each seat has already used, settled at each handover.
    spent: [std::time::Duration; MAX_PLAYERS],
    /// What this player calls themselves. Editable while nobody is signed in;
    /// an account would supply it instead.
    name: String,
    /// Turns taken, in one run from the first placement onwards. Four players
    /// place eight times between them, so the first turn of play is turn nine.
    /// Counted here because the engine has no notion of a turn: it tracks whose
    /// move it is, not how many have gone by.
    turns: u32,
    /// The phase at the last handover, for the two boundaries that a change of
    /// decider does not catch. Both are cases where the turn changes hands and
    /// comes back to the same person: the last player to place in the deal
    /// places first in the second round and then moves first in play.
    was_preroll: bool,
    was_placing: bool,
    /// Whether the table keeps a visible record. A table rule rather than a
    /// personal setting: playing from memory only works if nobody has the log.
    log_shown: bool,
    /// Picks a move when the clock runs out in a phase that cannot be passed.
    /// Its own generator rather than the game's, so forfeits never disturb the
    /// dice or the deck.
    forfeit: Rng,
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
            log: vec![LogLine {
                turn: 0,
                setup: true,
                seat: None,
                text: format!(
                    "{seats} players, {mode:?} market, seed {}",
                    seed_code(seed)
                ),
            }],
            declined: [false; MAX_OFFERS],
            seed,
            started: std::time::Instant::now(),
            clock: Clock::Off,
            turn_began: std::time::Instant::now(),
            last_settle: std::time::Instant::now(),
            charged_to: HUMAN,
            spent: [std::time::Duration::ZERO; MAX_PLAYERS],
            name: "you".to_string(),
            log_shown: true,
            turns: 1,
            was_preroll: false,
            // The game opens on a placement, which is turn one rather than a
            // boundary into it.
            was_placing: true,
            forfeit: Rng::new(seed ^ 0x5EED_C10C_C0FF_EE01),
        }
    }

    /// Put this game on a clock.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        let now = std::time::Instant::now();
        self.turn_began = now;
        self.last_settle = now;
        self.charged_to = self.on_clock();
        self
    }

    /// Name the person at this browser. Empty falls back rather than showing a
    /// blank seat.
    pub fn with_name(mut self, name: &str) -> Self {
        let name = name.trim();
        if !name.is_empty() {
            self.name = name.chars().take(24).collect();
        }
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Play with the record hidden, so the table has to remember.
    pub fn with_log(mut self, shown: bool) -> Self {
        self.log_shown = shown;
        self
    }

    pub fn log_shown(&self) -> bool {
        self.log_shown
    }

    /// Whole seconds since the game was dealt.
    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    pub fn clock(&self) -> Clock {
        self.clock
    }

    /// The turn number to show.
    pub fn turn_no(&self) -> u32 {
        self.turns
    }

    /// Whether the board is still being dealt.
    pub fn in_setup(&self) -> bool {
        matches!(
            self.state.phase,
            Phase::SetupSettlement { .. } | Phase::SetupRoad { .. }
        )
    }

    /// Whose clock should be running.
    ///
    /// Usually whoever is deciding, but not always: an offer left on the table
    /// stops the bots until the human answers it, and that wait belongs to the
    /// human even though the turn is somebody else's. Charging it to the seat
    /// whose turn it is meant their clock drained while they were not the one
    /// holding anybody up, and meant nothing was ever forced, because the
    /// enforcement only ever looked at the decider. The game simply stopped.
    pub fn on_clock(&self) -> u8 {
        let deciding = self.state.decider();
        if deciding != HUMAN && !self.choices().is_empty() {
            HUMAN
        } else {
            deciding
        }
    }

    /// Which turn it is right now, to be read *before* an action is applied.
    ///
    /// Applying can move the phase on, and the last placement of the deal moves
    /// it out of the deal entirely. Stamping afterwards filed that placement
    /// under a turn that had not started yet.
    fn stamp(&self) -> (u32, bool) {
        (self.turn_no(), self.in_setup())
    }

    /// Add a line of history under the turn it belongs to.
    fn note_at(&mut self, (turn, setup): (u32, bool), seat: Option<u8>, text: String) {
        self.log.push(LogLine {
            turn,
            setup,
            text,
            seat,
        });
    }

    /// Log what everybody drew from the board, comparing hands either side of
    /// an action. Public by definition: production is dealt in the open.
    fn note_production(&mut self, at: (u32, bool), before: &[[u8; 5]; MAX_PLAYERS]) {
        let after = self.state.hand;
        for seat in 0..self.state.players as usize {
            let got = gains(before, &after, seat);
            if !got.is_empty() {
                self.note_at(at, Some(seat as u8), format!("Collect {got}"));
            }
        }
    }

    /// Note a robber steal, which is a card of unknown kind changing hands.
    fn note_steal(
        &mut self,
        at: (u32, bool),
        action: &Action,
        seat: u8,
        before: &[[u8; 5]; MAX_PLAYERS],
    ) {
        let Action::MoveRobber {
            victim: Some(from), ..
        } = action
        else {
            return;
        };
        if took_a_card(before, &self.state.hand, seat as usize) {
            self.note_at(at, Some(seat), format!("Take a card, unseen, from seat {from}"));
        }
    }

    /// Add a line for something happening now rather than for an action.
    fn note(&mut self, seat: Option<u8>, text: String) {
        let at = self.stamp();
        self.note_at(at, seat, text);
    }

    /// Wave away every offer currently open, as declining does.
    fn decline_open_offers(&mut self) {
        for i in self.open_offers() {
            self.declined[i as usize] = true;
        }
    }

    /// Settle the running clock against whoever it belongs to, then hand it to
    /// whoever is deciding now. Called after anything that could move the turn.
    fn hand_over_clock(&mut self) {
        let now = std::time::Instant::now();
        let owner = self.charged_to as usize;
        if owner < MAX_PLAYERS {
            self.spent[owner] += now - self.last_settle;
        }
        self.last_settle = now;
        // A new turn is not only a new decider. The player who places last in
        // the deal is the player who moves first, so the turn changes with the
        // decider unchanged, and their first real turn used to inherit whatever
        // was left of their setup allowance. Entering PreRoll is the other
        // boundary, and it happens exactly once per turn.
        // A new turn is not only a new decider. The deal runs as a snake, so
        // the player who places last in the first round places first in the
        // second, and then moves first in play: the turn changes hands twice
        // without the decider changing, and both were being missed. Entering
        // PreRoll marks the start of a turn of play, entering SetupSettlement
        // the start of a placement, and each happens exactly once per turn.
        let deciding = self.on_clock();
        let preroll = matches!(self.state.phase, Phase::PreRoll);
        let placing = matches!(self.state.phase, Phase::SetupSettlement { .. });
        let starting_a_turn =
            (preroll && !self.was_preroll) || (placing && !self.was_placing);
        self.was_preroll = preroll;
        self.was_placing = placing;
        if deciding != self.charged_to || starting_a_turn {
            // Finishing a turn credits the increment back, which is the whole
            // difference between a chess clock and a countdown: a player who
            // keeps moving keeps playing, and the game is decided on the board.
            let inc = self.clock.increment();
            if inc > 0 {
                let prev = self.charged_to as usize;
                if prev < MAX_PLAYERS {
                    self.spent[prev] = self.spent[prev]
                        .saturating_sub(std::time::Duration::from_secs(inc));
                }
            }
            self.turn_began = now;
            self.charged_to = deciding;
            self.turns += 1;
        }
    }

    /// Time a seat has used, including the stretch not yet settled.
    fn used(&self, seat: u8) -> std::time::Duration {
        let mut d = self.spent[seat as usize];
        if seat == self.charged_to {
            d += self.last_settle.elapsed();
        }
        d
    }

    /// Seconds left on a seat's clock, negative once it has run out so the page
    /// can show the overrun rather than sitting at zero looking broken.
    ///
    /// A per-turn allowance only means anything for the seat currently on the
    /// clock; everyone else is shown the full allocation they will get.
    pub fn time_left(&self, seat: u8) -> Option<i64> {
        match self.clock {
            Clock::Off => None,
            Clock::PerTurn(n) => Some(if seat == self.charged_to {
                n as i64 - self.turn_began.elapsed().as_secs() as i64
            } else {
                n as i64
            }),
            Clock::Chess { bank, .. } => Some(bank as i64 - self.used(seat).as_secs() as i64),
        }
    }

    /// Whether that seat has nothing left to think with.
    pub fn out_of_time(&self, seat: u8) -> bool {
        self.time_left(seat).is_some_and(|t| t <= 0)
    }

    /// End the human's turn when their clock has run out.
    ///
    /// Enforced lazily, on the way in to a request, because a server that only
    /// wakes when asked cannot end a turn at the exact second. Only ever ends a
    /// turn that could legally be ended. A clock should not be able to skip a
    /// setup placement or a discard, which would leave the position illegal.
    pub fn enforce_clock(&mut self) {
        if self.clock == Clock::Off {
            return;
        }
        // Bounded rather than looped to exhaustion: an empty chess bank ends
        // every turn the moment it starts, and that should play out across
        // requests instead of inside one of them.
        for _ in 0..8 {
            if self.on_clock() != HUMAN || !self.out_of_time(HUMAN) {
                return;
            }
            // The turn is someone else's and the only thing owed is an answer
            // to what is on the table. Silence is a refusal: waiting cannot be
            // allowed to hold up three other people indefinitely.
            if self.state.decider() != HUMAN {
                if self.open_offers().is_empty() {
                    return;
                }
                self.decline_open_offers();
                self.note(Some(HUMAN), "Time ran out, declined the offers".to_string());
                self.finish_move();
                continue;
            }
            let mut buf = Vec::new();
            self.state.legal_into(&mut buf);
            if buf.is_empty() {
                return;
            }

            // Ending the turn is the forfeit, and rolling comes before it
            // because a turn cannot be ended before the dice.
            //
            // Everything left is a phase the rules do not let anyone pass:
            // placing a starting settlement, placing the road that goes with
            // it, discarding on a seven, and moving the robber. There the
            // clock picks a legal move at random, because the alternative is
            // a game that stops forever on someone who walked away. Random
            // and stated is worse for that player than choosing well and
            // better than everyone waiting.
            //
            // Drawn from the session's own generator, so a game with the same
            // seed and the same timings forfeits identically.
            let at = self.stamp();
            let forced = if buf.contains(&Action::EndTurn) {
                Action::EndTurn
            } else if buf.contains(&Action::Roll) {
                Action::Roll
            } else {
                buf[self.forfeit.below(Stream::Steal, buf.len() as u32) as usize]
            };

            if self.state.apply(forced).is_err() {
                return;
            }
            self.version += 1;
            let what = match forced {
                Action::EndTurn => "turn ended".to_string(),
                Action::Roll => format!("{} for you", rolled(&self.state)),
                other => format!("{} for you", log_phrase(&other, &self.state, HUMAN as usize)),
            };
            self.note_at(at, Some(HUMAN), format!("Time ran out, {what}"));
            self.forget_declines();
            self.finish_move();
        }
    }

    pub fn winner(&self) -> Option<u8> {
        match self.state.phase {
            Phase::GameOver { winner } => Some(winner),
            _ => None,
        }
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

    pub fn log(&self) -> &[LogLine] {
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
        // Not their turn, but an offer may be waiting for them.
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
                self.note(Some(HUMAN), "Declined the open offers".to_string());
            }
            Choice::Play(action) => {
                // Named before it is applied: a phrase describes the position
                // the action was taken in, not the one it produced.
                let phrase = log_phrase(&action, &self.state, HUMAN as usize);
                let at = self.stamp();
                let purse = self.state.hand;
                self.state.apply(action).map_err(Refused::Illegal)?;
                self.version += 1;
                let phrase = match action {
                    Action::Roll => rolled(&self.state),
                    _ => phrase,
                };
                self.note_at(at, Some(HUMAN), phrase);
                if pays_out(&action) {
                    self.note_production(at, &purse);
                }
                self.note_steal(at, &action, HUMAN, &purse);
                self.forget_declines();
            }
        }
        self.finish_move();
        Ok(())
    }

    /// Compose and make an offer of any shape, to anyone (R-7.19).
    ///
    /// Separate from [`Session::act`] because the engine *generates* only open,
    /// single-type offers, a bound on enumeration, not on legality. A person
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
        let phrase = log_phrase(&action, &self.state, HUMAN as usize);
        let at = self.stamp();
        self.state.apply(action).map_err(Refused::Illegal)?;
        self.version += 1;
        self.note_at(at, Some(HUMAN), phrase);
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
        // Three handovers, not one, because the turn can change hands twice
        // inside a single call and the clock has to see both.
        //
        // The first catches the move that was just made passing the turn on.
        // Without it the sequence human, bots, human looked to the clock like
        // the human never stopped deciding, so a per-turn allowance was never
        // refilled and an expired clock forfeited the next turn too. In setup
        // that meant one timeout took both of a player's placements.
        self.hand_over_clock();
        self.settle_between_bots();
        self.run_bots();
        self.hand_over_clock();
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
            let phrase = log_phrase(&action, &self.state, seat);
            let at = self.stamp();
            let purse = self.state.hand;
            if self.state.apply(action).is_err() {
                break;
            }
            self.version += 1;
            let phrase = match action {
                Action::Roll => rolled(&self.state),
                _ => phrase,
            };
            if worth_logging(&action) {
                self.note_at(at, Some(seat as u8), phrase);
            }
            if pays_out(&action) {
                self.note_production(at, &purse);
            }
            self.note_steal(at, &action, seat as u8, &purse);
            self.forget_declines();
            // Each bot pays for its own thinking, and the turn passing between
            // two bots is still the turn passing.
            self.hand_over_clock();
            self.settle_between_bots();
        }
        if let Phase::GameOver { winner } = self.state.phase {
            let who = if winner == HUMAN {
                "You win".to_string()
            } else {
                format!("Seat {winner} wins")
            };
            if self.log.last().map(|l| l.text.as_str()) != Some(who.as_str()) {
                self.note(None, who);
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
                        let from = self.state.offers[i as usize].from as usize;
                        let purse = self.state.hand;
                        if self.state.apply(take).is_ok() {
                            self.version += 1;
                            // Both halves, because a trade is two public
                            // transfers and "took an offer" said neither.
                            let got = gains(&purse, &self.state.hand, seat as usize);
                            let gave = gains(&purse, &self.state.hand, from);
                            self.note(
                                Some(seat as u8),
                                format!("Take {got} from seat {from} for {gave}"),
                            );
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
    /// Wave away the offers currently on the table. Not an engine action,
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
            "Play Invention, take {} and {}",
            RESOURCE_NAMES[a as usize], RESOURCE_NAMES[b as usize]
        ),
        Action::PlayMonopoly(r) => format!("Play Monopoly on {}", RESOURCE_NAMES[r as usize]),
        Action::Trade { give, take } => {
            // Four-for-one needs no port. It is the bank, open to everyone
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
/// Whether this action can hand anybody resources out of the supply.
///
/// A roll produces, the second settlement grants, and the two cards that take
/// from the bank do so in the open. A robber steal is left out on purpose: the
/// card moves but which card it was is not public.
fn pays_out(a: &Action) -> bool {
    matches!(
        a,
        Action::Roll
            | Action::PlaceSettlement(_)
            | Action::PlayInvention(_)
            | Action::PlayMonopoly(_)
    )
}

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
                // on, so what must hold is that no cards changed hands at the
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
        // them may take the offer. A fine outcome for a game, and a poor one
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
        // R-7.18), the composer does not get its own rulebook.
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
        let asked = s.log().iter().any(|l| l.text.contains("took an offer"));
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

    #[test]
    fn an_untimed_game_never_puts_anyone_on_a_clock() {
        let s = Session::new(4, 1, TradeMode::Full);
        assert_eq!(s.clock(), Clock::Off);
        assert_eq!(s.time_left(0), None);
        assert!(!s.out_of_time(0));
    }

    #[test]
    fn zero_seconds_means_untimed_whichever_kind_was_asked_for() {
        // The lobby sends "off" as zero seconds, so this is the ordinary path.
        assert_eq!(Clock::parse(Some("turn"), 0, 0), Clock::Off);
        assert_eq!(Clock::parse(Some("chess"), 0, 0), Clock::Off);
        assert_eq!(Clock::parse(None, 60, 0), Clock::Off);
        assert_eq!(Clock::parse(Some("turn"), 60, 0), Clock::PerTurn(60));
        assert_eq!(
            Clock::parse(Some("chess"), 600, 5),
            Clock::Chess {
                bank: 600,
                increment: 5
            }
        );
    }

    #[test]
    fn a_per_turn_allowance_only_counts_down_for_whoever_holds_the_turn() {
        let s = Session::new(4, 1, TradeMode::Full).with_clock(Clock::PerTurn(60));
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        // Seat 0 is deciding, so seat 0 is being charged.
        assert!(s.time_left(0).is_some_and(|t| t < 60), "the mover is charged");
        assert_eq!(s.time_left(1), Some(60), "everyone else still has the full turn");
        assert_eq!(s.time_left(2), Some(60));
    }

    #[test]
    fn a_chess_clock_credits_the_increment_back_for_a_finished_turn() {
        // Without an increment this is a sudden-death timer, and a long game is
        // decided by the clock rather than by the board.
        let mut s = Session::new(4, 5, TradeMode::Full).with_clock(Clock::Chess {
            bank: 60,
            increment: 10,
        });
        std::thread::sleep(std::time::Duration::from_millis(2_100));
        let before = s.time_left(HUMAN).expect("a bank");
        assert!(before <= 58, "two seconds of thinking are gone, left {before}");
        // Half a turn is not a finished turn: placing the settlement leaves
        // the road still owed, so nothing is credited yet.
        let v = s.version();
        s.act(0, v).expect("a legal settlement");
        assert_eq!(
            s.time_left(HUMAN),
            Some(before),
            "no credit for a turn still in progress"
        );
        // Finishing it credits the increment.
        let v = s.version();
        s.act(0, v).expect("a legal road");
        let after = s.time_left(HUMAN).expect("a bank");
        assert!(
            after > before,
            "the increment is credited, {before} to {after}"
        );
    }

    #[test]
    fn a_sudden_death_chess_clock_credits_nothing() {
        assert_eq!(Clock::parse(Some("chess"), 600, 0).increment(), 0);
        assert_eq!(Clock::parse(Some("chess"), 600, 10).increment(), 10);
        // Only a chess clock has one.
        assert_eq!(Clock::parse(Some("turn"), 60, 10).increment(), 0);
    }

    #[test]
    fn a_chess_bank_drains_only_while_it_is_your_move() {
        let s = Session::new(4, 1, TradeMode::Full).with_clock(Clock::Chess { bank: 600, increment: 0 });
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(s.time_left(0).is_some_and(|t| t < 600), "the mover's bank drains");
        assert_eq!(s.time_left(1), Some(600), "a seat not moving spends nothing");
    }

    #[test]
    fn running_out_ends_the_turn_rather_than_the_game() {
        let mut s = Session::new(4, 1, TradeMode::Full).with_clock(Clock::PerTurn(1));
        // Play out setup so that ending a turn is a legal thing to do at all.
        while matches!(
            s.state.phase,
            Phase::SetupSettlement { .. } | Phase::SetupRoad { .. }
        ) {
            let i = s
                .choices()
                .iter()
                .position(|c| matches!(c, Choice::Play(_)))
                .expect("setup always offers a placement");
            let v = s.version();
            s.act(i, v).expect("a legal placement");
        }
        // Setup leaves the human before the roll, where ending a turn is not
        // legal yet, the clock has to roll first or the game would stall.
        assert!(matches!(s.state.phase, Phase::PreRoll));
        let before = s.version();
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(s.out_of_time(HUMAN), "the allowance has gone");
        s.enforce_clock();
        assert!(s.version() > before, "the turn was moved on for the player");
        assert!(s.winner().is_none(), "a clock does not decide the game");
        assert!(
            s.log().iter().any(|l| l.text.contains("ran out")),
            "and it says so"
        );
        assert!(
            s.log()
                .iter()
                .any(|l| l.text.contains("Rolled ") && l.text.contains("for you")),
            "rolling came first, because a turn cannot be ended before the dice"
        );
        assert!(
            s.log().iter().any(|l| l.text.contains("turn ended")),
            "and the turn ended in the same sweep, rolling must not refill the \
             allowance, or a clock could roll for you forever"
        );
    }

    #[test]
    fn a_clock_forces_a_placement_rather_than_stalling_on_it() {
        // Setup cannot be passed, so an expired clock has to choose. It used
        // to decline and wait, which meant a game stopped forever on anyone
        // who walked away during setup.
        let mut s = Session::new(4, 1, TradeMode::Full).with_clock(Clock::PerTurn(1));
        assert!(matches!(s.state.phase, Phase::SetupSettlement { .. }));
        let before = s.version();
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        s.enforce_clock();
        assert!(s.version() > before, "a placement was made");
        assert!(
            !matches!(s.state.phase, Phase::SetupSettlement { round: 0 }),
            "and the position moved on rather than staying stuck"
        );
        assert!(
            s.log().iter().any(|l| l.text.contains("ran out")),
            "and it says the clock did it, not the player"
        );
    }

    #[test]
    fn public_resource_movements_are_logged() {
        let mut s = Session::new(4, 5, TradeMode::Full);
        // The second settlement pays out (R-3.10), and that is public.
        while s.in_setup() {
            let v = s.version();
            s.act(0, v).expect("a legal placement");
        }
        let setup_pay: Vec<_> = s
            .log()
            .iter()
            .filter(|l| l.setup && l.text.starts_with("Collect "))
            .collect();
        assert!(
            !setup_pay.is_empty(),
            "the second settlement grants resources and the table can see it"
        );
        assert_eq!(
            setup_pay.len(),
            4,
            "one line per seat, since all four place a second settlement"
        );

        // Production on a roll, likewise.
        let before = s.log().len();
        let v = s.version();
        s.act(0, v).expect("the roll");
        let produced = s.log()[before..]
            .iter()
            .filter(|l| l.text.starts_with("Collect "))
            .count();
        let [a, b] = s.state.dice;
        if a + b != 7 {
            // A seven pays nobody, so only assert when something was produced.
            let any = s.state.hand.iter().any(|h| h.iter().any(|&n| n > 0));
            assert!(any || produced == 0);
        }
    }

    #[test]
    fn a_robber_steal_never_says_which_card_moved() {
        // The card moves, and which card it was is not public. Reporting it
        // through the same diff that reports production would leak it.
        assert!(!pays_out(&Action::MoveRobber {
            hex: 0,
            victim: Some(1)
        }));
        assert!(pays_out(&Action::Roll));
        assert!(pays_out(&Action::PlaceSettlement(0)));
    }

    #[test]
    fn the_log_records_what_the_dice_showed() {
        // The phrase for every other action is built before it is applied. A
        // roll cannot be: the dice still hold the previous turn's numbers until
        // the engine has thrown them.
        let mut s = Session::new(4, 5, TradeMode::Full);
        while s.in_setup() {
            let v = s.version();
            s.act(0, v).expect("a legal placement");
        }
        let v = s.version();
        s.act(0, v).expect("the roll");
        let [a, b] = s.state.dice;
        let line = s
            .log()
            .iter()
            .rev()
            .find(|l| l.text.starts_with("Rolled "))
            .expect("the roll is logged");
        assert_eq!(line.text, format!("Rolled {} ({a}, {b})", a + b));
        assert_ne!(line.text, "Roll the dice", "the outcome, not the intent");
    }

    #[test]
    fn a_seed_code_round_trips_and_reads_in_groups() {
        for seed in [0u64, 1, 42, 1 << 32, u64::MAX, 12614495042559003205] {
            let code = seed_code(seed);
            assert_eq!(code.len(), 15, "{code} is thirteen characters and two hyphens");
            assert_eq!(code.matches('-').count(), 2);
            assert_eq!(parse_seed(&code), Some(seed), "{code} should read back");
        }
        // Tolerant of how it comes back from a person.
        let code = seed_code(9_876_543_210);
        assert_eq!(parse_seed(&code.replace('-', "")), Some(9_876_543_210));
        assert_eq!(parse_seed(&code.to_uppercase()), Some(9_876_543_210));
        assert_eq!(parse_seed(""), None);
    }

    /// The phases a player cannot pass, and so the complete set where a clock
    /// has to choose for them. Locked to the engine's own phase list: if a new
    /// blocking phase is ever added, this fails rather than silently letting a
    /// game stall on it.
    #[test]
    fn every_phase_that_cannot_be_passed_has_something_to_force() {
        let mut seen: Vec<std::mem::Discriminant<Phase>> = Vec::new();
        for seed in 0..60u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..400 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                if s.state.decider() == HUMAN {
                    let mut buf = Vec::new();
                    s.state.legal_into(&mut buf);
                    // Whatever the phase, the clock must have a move to make.
                    assert!(
                        !buf.is_empty(),
                        "seed {seed}: {:?} offers the human nothing",
                        s.state.phase
                    );
                    let d = std::mem::discriminant(&s.state.phase);
                    if !seen.contains(&d) {
                        seen.push(d);
                    }
                    let passable =
                        buf.contains(&Action::EndTurn) || buf.contains(&Action::Roll);
                    if !passable {
                        // A forfeit here is a random legal move, so every
                        // option has to be applicable.
                        assert!(
                            matches!(
                                s.state.phase,
                                Phase::SetupSettlement { .. }
                                    | Phase::SetupRoad { .. }
                                    | Phase::Discard
                                    | Phase::MoveRobber { .. }
                            ),
                            "seed {seed}: {:?} cannot be passed and is not on the \
                             forfeit list in enforce_clock",
                            s.state.phase
                        );
                    }
                    let v = s.version();
                    let _ = s.act(0, v);
                } else {
                    let v = s.version();
                    if s.act(0, v).is_err() {
                        break;
                    }
                }
            }
        }
        assert!(seen.len() >= 5, "the sweep should reach most phases, saw {}", seen.len());
    }

    #[test]
    fn the_next_turn_starts_on_a_full_allowance() {
        // The bug this pins: the sequence human, bots, human looked to the
        // clock like the human never stopped deciding, because the handover
        // was only checked after the bots had finished and by then the decider
        // was the human again. So the allowance was never refilled, and a
        // player who had used most of it, or all of it, carried that into
        // their next turn. In setup one lapse took both placements.
        let mut s = Session::new(4, 5, TradeMode::Full).with_clock(Clock::PerTurn(4));
        assert!(matches!(s.state.phase, Phase::SetupSettlement { round: 0 }));

        // Spend most of round one's allowance, then place.
        std::thread::sleep(std::time::Duration::from_millis(2_100));
        assert!(
            s.time_left(HUMAN).is_some_and(|t| t <= 2),
            "two of the four seconds are gone"
        );
        for _ in 0..2 {
            let v = s.version();
            s.act(0, v).expect("a legal setup placement");
        }

        // The bots have played and it is the human's turn again.
        assert_eq!(s.state.decider(), HUMAN, "back to the human for round two");
        assert_eq!(
            s.time_left(HUMAN),
            Some(4),
            "round two starts full, not with round one's remainder"
        );
    }

    #[test]
    fn the_first_real_turn_is_not_the_last_setup_turn() {
        // The player who places last in the deal is the player who moves first,
        // so the turn changes without the decider changing. That boundary was
        // invisible to the clock, and the first real turn ran on whatever the
        // setup turn had left.
        let mut s = Session::new(4, 5, TradeMode::Full).with_clock(Clock::PerTurn(4));
        // Play the deal out, pausing so an allowance is visibly spent.
        while s.in_setup() {
            if s.state.decider() == HUMAN {
                std::thread::sleep(std::time::Duration::from_millis(1_100));
            }
            let v = s.version();
            s.act(0, v).expect("a legal placement");
        }
        assert!(matches!(s.state.phase, Phase::PreRoll), "the deal is done");
        assert_eq!(s.state.decider(), HUMAN, "and the first mover is the human");
        assert_eq!(
            s.time_left(HUMAN),
            Some(4),
            "the first real turn starts full, not on what setup left"
        );
    }

    #[test]
    fn the_last_placement_of_the_deal_is_filed_under_the_deal() {
        // Applying moves the phase on, and the last placement moves it out of
        // the deal entirely. Stamping the line afterwards filed it under a turn
        // that had not started yet, so the log showed a stray group between the
        // last setup turn and the first real one.
        let mut s = Session::new(4, 5, TradeMode::Full);
        while s.in_setup() {
            let v = s.version();
            s.act(0, v).expect("a legal placement");
        }
        let strays: Vec<_> = s
            .log()
            .iter()
            .filter(|l| !l.setup && l.turn == 0 && l.seat.is_some())
            .map(|l| l.text.clone())
            .collect();
        assert!(
            strays.is_empty(),
            "no move belongs to turn zero, found {strays:?}"
        );
        // The final placement is the deal's last turn, not play's first.
        let last = s
            .log()
            .iter()
            .rev()
            .find(|l| l.seat.is_some())
            .expect("something was placed");
        assert!(last.setup, "the last placement is still part of the deal");
    }

    #[test]
    fn four_players_place_eight_times_and_then_play_turn_nine() {
        // The deal is a snake: the player who places last in the first round
        // places first in the second. The decider does not change across that
        // fold, so their two placements were counted as one turn and the deal
        // came out one short.
        let mut s = Session::new(4, 5, TradeMode::Full);
        assert!(s.in_setup());
        assert_eq!(s.turn_no(), 1, "the deal opens on turn one");
        let mut placements = 0;
        while s.in_setup() {
            let v = s.version();
            s.act(0, v).expect("a legal placement");
            placements += 1;
        }
        // The human acts four times, a settlement and a road in each round;
        // the bots take theirs inside the same calls.
        assert_eq!(placements, 4);
        assert_eq!(s.turn_no(), 9, "eight placement turns, then the first of play");
    }

    #[test]
    fn three_players_place_six_times_and_then_play_turn_seven() {
        let mut s = Session::new(3, 9, TradeMode::Full);
        while s.in_setup() {
            let v = s.version();
            s.act(0, v).expect("a legal placement");
        }
        assert_eq!(s.turn_no(), 7);
    }

    #[test]
    fn an_unanswered_offer_is_declined_rather_than_stopping_the_game() {
        // An offer waiting on the human halts the bots, because the human owes
        // an answer. The turn belongs to somebody else, though, so the clock
        // used to be charged to them and enforcement never ran: the game stood
        // still for as long as nobody clicked.
        let mut found = None;
        'seeds: for seed in 0..200u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..300 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                // An offer is on the table and it is not the human's turn.
                if s.state.decider() != HUMAN && !s.open_offers().is_empty() {
                    found = Some(s);
                    break 'seeds;
                }
                let v = s.version();
                if s.act(0, v).is_err() {
                    break;
                }
            }
        }
        let mut s = found.expect("some seed reaches an offer on a bot's turn");

        // The wait is the human's, so the human's clock is the one running.
        assert_eq!(s.on_clock(), HUMAN, "the hold-up belongs to whoever must answer");

        s = s.with_clock(Clock::PerTurn(1));
        let before = s.version();
        let offers = s.open_offers().len();
        assert!(offers > 0);
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(s.out_of_time(HUMAN));
        s.enforce_clock();

        assert!(
            s.open_offers().is_empty(),
            "silence is a refusal, so the table is cleared"
        );
        assert!(s.version() >= before, "and play carries on");
        assert!(
            s.log().iter().any(|l| l.text.contains("declined")),
            "and the log says the clock did it"
        );
    }

    #[test]
    fn a_forfeit_uses_its_own_generator_and_repeats_for_a_seed() {
        // Same seed, same forced pick: a game is reproducible even when the
        // clock is the one moving.
        let pick = |seed: u64| {
            let mut s = Session::new(4, seed, TradeMode::Full).with_clock(Clock::PerTurn(1));
            std::thread::sleep(std::time::Duration::from_millis(1_100));
            s.enforce_clock();
            s.log().last().map(|l| l.text.clone()).unwrap_or_default()
        };
        assert_eq!(pick(7), pick(7));
    }

    #[test]
    fn a_name_is_kept_trimmed_and_falls_back_when_blank() {
        assert_eq!(Session::new(4, 1, TradeMode::Full).name(), "you");
        assert_eq!(
            Session::new(4, 1, TradeMode::Full).with_name("  Robin  ").name(),
            "Robin"
        );
        assert_eq!(
            Session::new(4, 1, TradeMode::Full).with_name("   ").name(),
            "you"
        );
    }
}
