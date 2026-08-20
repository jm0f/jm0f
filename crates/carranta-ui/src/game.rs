//! One local game: the human at seat 0, heuristic bots elsewhere.
//!
//! **The browser is served a redacted view, never the state.** Everything the
//! page receives goes through [`carranta_record::fog`], the same projection a
//! real server would use, so the client physically cannot be sent another
//! seat's cards or the deck order, because the type it is built from has no
//! field for them. That is worth doing here rather than later: a local UI that
//! reads the raw state would grow a habit the server then has to unpick.

use carranta_bot::net::Net;
use carranta_bot::policy_net::NetPolicy;
use carranta_bot::{Heuristic, Policy};
use carranta_core::action::{Action, Illegal};
use carranta_core::rng::{Rng, Stream};
use carranta_core::state::{
    MAX_OFFERS, MAX_PLAYERS, OFFERS_PER_TURN, Offer, OfferShapes, Phase, State, TradeMode,
};
use carranta_record::fog::{Fog, Viewer, fog};

/// The seat the person who dealt the table plays.
///
/// Kept as a name rather than dissolved into `people`, because it is still what
/// a table with nobody else at it means: the host sits at nought and the bots
/// fill in behind them. What it stopped being is the definition of "a person",
/// which is what let a second one sit down.
pub const HUMAN: u8 = 0;

/// The most seats a table can have, which is what the rules allow (R-1).
pub const SEATS: usize = 4;

/// How long a seven's discard gets when the lobby does not say otherwise.
///
/// Short, because it is an interruption rather than a turn: everyone else is
/// waiting on it, the decision is small, and the hand is laid out to choose
/// from. Ten seconds is long enough to read a hand of eight and pick four.
pub const DEFAULT_DISCARD_SECS: u64 = 10;

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
/// A log phrase with its first letter dropped to lower case, for the middle of
/// a sentence: "Time ran out, discarded a wheat for you".
fn lower_first(t: String) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => t,
    }
}

fn log_phrase(a: &Action, state: &State, seat: usize) -> String {
    match *a {
        Action::PlaceSettlement(_) => "Placed a settlement".to_string(),
        Action::PlaceRoad(_) => "Placed a road".to_string(),
        // The dice are read after the roll, so the caller rewrites this one;
        // see `rolled`.
        Action::Roll => "Rolled".to_string(),
        Action::Discard { resource, .. } => {
            format!("Discarded {}", RESOURCE_NAMES[resource as usize])
        }
        Action::MoveRobber { victim, .. } => match victim {
            Some(v) => format!("Moved the robber onto seat {v}"),
            None => "Moved the robber".to_string(),
        },
        Action::BuildRoad(_) => "Built a road".to_string(),
        Action::BuildSettlement(_) => "Built a settlement".to_string(),
        Action::BuildCity(_) => "Upgraded to a city".to_string(),
        Action::BuyDev => "Bought a development card".to_string(),
        Action::PlayMilitia => "Played Militia".to_string(),
        Action::PlayRoadBuilding => "Played Road Building".to_string(),
        Action::PlayInvention([a, b]) => format!(
            "Played Invention, took {} and {}",
            RESOURCE_NAMES[a as usize], RESOURCE_NAMES[b as usize]
        ),
        Action::PlayMonopoly(r) => format!("Played Monopoly on {}", RESOURCE_NAMES[r as usize]),
        Action::Trade { give, take } => {
            let rate = state.trade_rate(seat, give);
            let with = if rate == 4 {
                "with the bank"
            } else {
                "at the port"
            };
            format!(
                "Traded {rate} {} for 1 {} {with}",
                RESOURCE_NAMES[give as usize], RESOURCE_NAMES[take as usize]
            )
        }
        Action::ProposeTrade { to, give, want, .. } => match to {
            Some(s) => format!("Offered seat {s} {} for {}", cards(&give), cards(&want)),
            None => format!("Offered {} for {}", cards(&give), cards(&want)),
        },
        // Named while the offer is still on the table, which is the only time
        // it can be: taking one is what removes it. "Accepted an offer" said
        // who but not with whom or for what, and which offer was taken is the
        // whole of the news when several are standing.
        Action::AcceptTrade { offer, .. } => match state.offers.get(offer as usize) {
            Some(o) => format!(
                "Took {} from seat {} for {}",
                cards(&o.give),
                o.from,
                cards(&o.want)
            ),
            None => "Accepted an offer".to_string(),
        },
        Action::WithdrawTrade { .. } => "Withdrew an offer".to_string(),
        Action::EndTurn => "Ended the turn".to_string(),
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
    for r in LISTING_ORDER {
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
    /// Everything that happened, in order.
    ///
    /// The whole game, because the engine is deterministic: the same seats, the
    /// same seed and the same steps rebuild this position down to the next
    /// random number. That is what makes a game something you can put on disk
    /// in a few hundred bytes and analyse afterwards, and it is the same
    /// argument `carranta-record` makes about its own event log (H-1).
    moves: Vec<Step>,
    /// When each of them landed, in milliseconds since the deal.
    ///
    /// Kept beside the moves rather than inside them, for the reason the moves
    /// are what they are: a step is enough to rebuild the game on its own, and
    /// when it happened is something known *about* the step. Written down so
    /// the analytics can say where a game's time went, which is the one
    /// question the moves alone cannot answer.
    ///
    /// Always exactly as long as `moves`, or empty on a session rebuilt from a
    /// file that had no clock in it.
    times: Vec<u32>,
    /// The two other things `Session::new` was given, kept so a saved game can
    /// be rebuilt from its file without the caller having to remember them.
    seats: u8,
    mode: TradeMode,
    /// Which seats a person is sitting in. Everything else is played by the
    /// house bot, and the difference is the whole of what "a person" means here:
    /// a person's seat waits to be asked, a bot's answers immediately.
    ///
    /// Seat nought alone by default, so a table nobody has joined is the game
    /// this was before anybody could join one.
    people: [bool; SEATS],
    /// What each seat is called, empty where nobody has said. A seat's name is
    /// the seat's, not the table's: two people means two answers.
    names: [String; SEATS],
    /// Who plays each seat a person does not, and which player that is. The
    /// identity lives on the brain rather than beside it, so a seat and the
    /// name written into its chair cannot drift apart.
    bots: Vec<Brain>,
    /// Bumped on every applied action, so a click made against a stale board is
    /// refused rather than applied to a different position.
    version: u64,
    log: Vec<LogLine>,
    /// Every offer put on the table this turn, and what each seat said to it.
    ///
    /// The engine does not carry this and should not: an offer nobody took and
    /// an offer three people turned down are the same position to it, and it is
    /// right about that. They are not the same thing to watch. It also drops an
    /// offer the moment it is taken, which is exactly when there is something
    /// worth showing about it.
    deals: Vec<Deal>,
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
    /// How long a seven's discard gets, in seconds. Its own allowance, because
    /// it is not part of anybody's turn: see `discard_left`.
    discard_secs: u64,
    /// When the discard now owed began, if one is owed.
    discard_at: Option<std::time::Instant>,
    /// How long this turn has been held while discards were owed, so the turn
    /// clock can be paused for them and resumed where it stopped. Reset when
    /// the turn is.
    turn_paused: std::time::Duration,
    /// How much of that hold has already been taken off a seat's account.
    ///
    /// Two questions, and one accumulator cannot answer both: a per-turn
    /// allowance wants every pause since the turn began, and a chess bank wants
    /// only the pause since the last settle. This is the mark between them.
    paused_settled: std::time::Duration,
    /// Whose turn it is: what the turn counter and the log group by, and whose
    /// clock is running. There used to be a second field beside this one for
    /// the clock's owner, on the theory that an unanswered offer is charged to
    /// whoever owes the answer. It is not, so the two collapsed into one; see
    /// `on_clock` for why.
    turn_holder: u8,
    /// When the current turn began, for reporting how long it took. Distinct
    /// from `turn_began`, which is the allowance and restarts whenever the
    /// turn changes hands.
    turn_at: std::time::Instant,
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
    /// How long each finished turn took, indexed by turn number less one. The
    /// clock already knows; recording it lets the log say so afterwards.
    turn_ms: Vec<u32>,
    /// The phase at the last handover, for the two boundaries that a change of
    /// decider does not catch. Both are cases where the turn changes hands and
    /// comes back to the same person: the last player to place in the deal
    /// places first in the second round and then moves first in play.
    was_preroll: bool,
    was_placing: bool,
    /// Whether the table keeps a visible record. A table rule rather than a
    /// personal setting: playing from memory only works if nobody has the log.
    log_shown: bool,
    /// What this table is called, chosen in the lobby. Empty when unnamed.
    game: String,
    /// How long bots wait between moves, and when the next one may go.
    pace: Pace,
    bot_ready: std::time::Instant,
    /// Draws the waits. Its own generator, so pacing never disturbs the dice.
    tempo: Rng,
    /// The position before a development card that opens a second decision,
    /// and how long the log was then, so an unfinished play can be put back.
    undo: Option<(State, usize, usize)>,
    /// Whether the bank's stacks are counted exactly or only sized.
    bank_exact: bool,
    /// Whether the table is listed for anyone to join. Private by default:
    /// listing a table is publishing it, and that should be asked for.
    public: bool,
    /// Picks a move when the clock runs out in a phase that cannot be passed.
    /// Its own generator rather than the game's, so forfeits never disturb the
    /// dice or the deck.
    forfeit: Rng,
    /// When the game ended, once it has.
    ///
    /// Every clock on the page is a subtraction from *now*, which keeps being
    /// true of a finished game and keeps producing a different answer: the
    /// turn clock counted down under the winner's own dialog and the game
    /// timer went on climbing for as long as the tab stayed open. Nobody is
    /// thinking any more, so there is nothing left to measure. This is the
    /// moment every reading is taken against once it exists, which freezes the
    /// clocks at the values they held when the game was won rather than
    /// blanking them: how long the game took is worth reading afterwards.
    ended: Option<std::time::Instant>,
}

/// The software behind a seat nobody is sitting in.
///
/// The house heuristic unless the server was handed a trained champion. An
/// enum rather than a boxed trait object because there are exactly two of
/// these, both already [`Policy`], and a session that can *name* its player
/// can write that name into the game file, which a box of anonymous behaviour
/// could not.
enum Brain {
    House(Heuristic),
    /// A trained champion, carrying the generation it came from. The number is
    /// the player's identity rather than a label: two generations of one run
    /// are two players, and the chair records which one sat here.
    Trained(NetPolicy, u32),
}

impl Brain {
    fn choose(&mut self, state: &State, legal: &[Action]) -> Action {
        match self {
            Brain::House(h) => h.choose(state, legal),
            Brain::Trained(n, _) => n.choose(state, legal),
        }
    }

    fn accepts(&mut self, state: &State, seat: usize, offer: usize) -> bool {
        match self {
            Brain::House(h) => h.accepts(state, seat, offer),
            Brain::Trained(n, _) => n.accepts(state, seat, offer),
        }
    }
}

/// One seed per seat off the table's, so four bots at one table do not mirror
/// each other while the same table replays identically.
fn seat_seed(seed: u64, seat: u8) -> u64 {
    seed.wrapping_mul(31).wrapping_add(seat as u64 + 1)
}

impl Session {
    pub fn new(seats: u8, seed: u64, mode: TradeMode) -> Self {
        let seats = seats.clamp(3, MAX_PLAYERS as u8);
        Session {
            state: State::new(seats, seed).with_trade_mode(mode),
            moves: Vec::new(),
            times: Vec::new(),
            seats,
            mode,
            people: {
                let mut who = [false; SEATS];
                who[HUMAN as usize] = true;
                who
            },
            names: Default::default(),
            bots: (0..seats)
                .map(|s| Brain::House(Heuristic::new(seat_seed(seed, s))))
                .collect(),
            version: 0,
            log: vec![LogLine {
                turn: 0,
                setup: true,
                seat: None,
                text: format!("{seats} players, {mode:?} market, seed {}", seed_code(seed)),
            }],
            deals: Vec::new(),
            seed,
            started: std::time::Instant::now(),
            clock: Clock::Off,
            turn_began: std::time::Instant::now(),
            last_settle: std::time::Instant::now(),
            discard_secs: DEFAULT_DISCARD_SECS,
            discard_at: None,
            turn_paused: std::time::Duration::ZERO,
            paused_settled: std::time::Duration::ZERO,
            turn_holder: HUMAN,
            turn_at: std::time::Instant::now(),
            spent: [std::time::Duration::ZERO; MAX_PLAYERS],
            name: "you".to_string(),
            log_shown: true,
            // Supply counts are public and may be checked (R-5.6), so counting
            // them is the default and judging them by eye is the house rule.
            bank_exact: true,
            // A bare session runs at engine speed. Pacing is a table setting,
            // asked for in the lobby and applied there; defaulting to it here
            // would make every test that drives a session wait on a wall clock
            // it has no reason to care about.
            pace: Pace::Instant,
            bot_ready: std::time::Instant::now(),
            tempo: Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15),
            undo: None,
            game: String::new(),
            public: false,
            turns: 1,
            turn_ms: Vec::new(),
            was_preroll: false,
            // The game opens on a placement, which is turn one rather than a
            // boundary into it.
            was_placing: true,
            forfeit: Rng::new(seed ^ 0x5EED_C10C_C0FF_EE01),
            ended: None,
        }
    }

    /// The moment every clock reading is taken against: now, or the moment the
    /// game ended, whichever came first.
    ///
    /// A finished game has no time passing in it. Routing every elapsed
    /// calculation through here is what makes that true everywhere at once,
    /// rather than in each place somebody remembered to ask whether the game
    /// was over.
    fn measured_at(&self) -> std::time::Instant {
        self.ended.unwrap_or_else(std::time::Instant::now)
    }

    /// How long a moment in the past has been going, with a finished game's
    /// clocks stopped at the end.
    fn since(&self, mark: std::time::Instant) -> std::time::Duration {
        self.measured_at().saturating_duration_since(mark)
    }

    /// Put this game on a clock.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        self.clock = clock;
        let now = std::time::Instant::now();
        self.turn_began = now;
        self.last_settle = now;
        self.turn_paused = std::time::Duration::ZERO;
        self.paused_settled = std::time::Duration::ZERO;
        self.turn_holder = self.state.decider();
        self.turn_at = now;
        self
    }

    /// How long a seven's discard gets. Zero is no limit.
    pub fn with_discard_secs(mut self, secs: u64) -> Self {
        self.discard_secs = secs;
        self
    }

    pub fn discard_secs(&self) -> u64 {
        self.discard_secs
    }

    /// Seconds left to discard, or `None` when nothing is being discarded.
    ///
    /// Its own allowance, separate from every turn clock, because a seven is
    /// not part of anybody's turn. It interrupts the turn holder, who did
    /// nothing but roll, and it asks the other players for something on a turn
    /// that is not theirs. Charging it to the turn punished the roller for the
    /// dice; charging it to the players who owe cards would have been a second
    /// clock on people who are not playing. So it is neither: a short fixed
    /// window belonging to the seven itself, with the turn clock held while it
    /// runs.
    pub fn discard_left(&self) -> Option<i64> {
        let at = self.discard_at?;
        if self.discard_secs == 0 {
            return None;
        }
        Some(self.discard_secs as i64 - self.since(at).as_secs() as i64)
    }

    /// Whether a discard is being waited on right now.
    pub fn discarding(&self) -> bool {
        self.discard_at.is_some()
    }

    /// How long this turn's clock has been held, including the hold running now.
    fn paused(&self) -> std::time::Duration {
        self.turn_paused
            + self
                .discard_at
                .map_or(std::time::Duration::ZERO, |t| self.since(t))
    }

    /// Of that, how much has not yet been taken off anybody's account.
    fn paused_unsettled(&self) -> std::time::Duration {
        self.paused().saturating_sub(self.paused_settled)
    }

    /// Start or finish holding the turn clock, following the phase.
    ///
    /// Called after anything that could change it. Entering the discard stops
    /// the turn clock where it stands; leaving it adds the held time to the
    /// turn's account so the allowance resumes rather than restarting.
    fn follow_discard(&mut self) {
        let owed = matches!(self.state.phase, Phase::Discard);
        match (owed, self.discard_at) {
            (true, None) => self.discard_at = Some(std::time::Instant::now()),
            (false, Some(at)) => {
                self.turn_paused += at.elapsed();
                self.discard_at = None;
            }
            _ => {}
        }
    }

    /// Hand one seat to a trained champion.
    ///
    /// Per seat rather than per table, because two champions at one table is
    /// the only way to ask which is better and get an answer that means
    /// anything: they play the same board, from different chairs, and the game
    /// file names each of them separately, so the ratings compare them
    /// directly rather than through a third party. A generation is a player
    /// (E-8), and the chair records which one.
    ///
    /// The market's *enumeration* switches to the mixed shapes champions train
    /// under (up to two cards a side, the training default) as soon as any seat
    /// is a champion's, because a policy chooses from what the engine
    /// generates, and one that learned to offer wood-and-brick for ore would
    /// have that repertoire silently amputated at a table that only generates
    /// one-type offers. It is a property of the table rather than of the seat,
    /// so one champion is enough to set it, and the same shapes then apply to
    /// everybody, which is what keeps the comparison fair. Nothing about
    /// *legality* moves: people compose whatever they like through the form.
    pub fn seat_trained(&mut self, seat: u8, net: &Net, generation: u32) {
        let Some(slot) = self.bots.get_mut(seat as usize) else {
            return;
        };
        *slot = Brain::Trained(
            NetPolicy::new(net.clone(), seat_seed(self.seed, seat)),
            generation,
        );
        self.state.offer_shapes = OfferShapes::Mixed {
            give: Some(2),
            want: 2,
        };
        // And the ask allowance it trained under (E-15): three generated
        // proposals per seat per turn. The same number in training and here,
        // or the served bot would be free to spend a table's time in a way
        // the measured one never could.
        self.state.ask_allowance = 3;
    }

    /// Put the house heuristic back in a seat.
    ///
    /// The inverse of [`Session::seat_trained`], and the reason a table can be
    /// arranged rather than only dealt: a lobby that can seat a champion has
    /// to be able to unseat one. The market's shapes follow the table's
    /// occupants, so unseating the last champion closes the enumeration back
    /// to single types, which is what a table of house bots has always played.
    pub fn seat_house(&mut self, seat: u8) {
        let Some(slot) = self.bots.get_mut(seat as usize) else {
            return;
        };
        *slot = Brain::House(Heuristic::new(seat_seed(self.seed, seat)));
        if self.champions().is_empty() {
            self.state.offer_shapes = OfferShapes::SingleType;
            self.state.ask_allowance = OFFERS_PER_TURN;
        }
    }

    /// Move two seats' players, as part of a draw for seats.
    ///
    /// Whoever is holding a chair takes their player with them. Without this
    /// the chairs move and the brains stay, so a champion chosen for the chair
    /// a person ends up in would be playing a seat nobody asked it to, and the
    /// chair would name a player that is not the one deciding its moves.
    ///
    /// The seat's generator is not re-seeded to match its new index. It could
    /// be, and it would buy nothing: a table whose seats were drawn is not
    /// reproducible from its seed anyway, and every seat still has a distinct
    /// stream, which is the property that matters.
    pub fn swap_bots(&mut self, a: usize, b: usize) {
        if a < self.bots.len() && b < self.bots.len() {
            self.bots.swap(a, b);
        }
    }

    /// Deal champions round the bot seats, repeating if there are fewer
    /// champions than seats.
    ///
    /// Two champions at a four seat table is one each on two chairs, which is
    /// the pairing worth running: seats differ in value, so a champion that
    /// only ever played one of them would be rated partly on its luck of the
    /// draw.
    pub fn with_trained(mut self, champions: &[(Net, u32)]) -> Self {
        if champions.is_empty() {
            return self;
        }
        for seat in 0..self.seats {
            let (net, generation) = &champions[seat as usize % champions.len()];
            self.seat_trained(seat, net, *generation);
        }
        self
    }

    /// Which software plays one seat, as `name@version`: the identity a game
    /// file writes into that chair, and the player a rating is about.
    pub fn agent_of(&self, seat: u8) -> String {
        match self.bots.get(seat as usize) {
            Some(Brain::Trained(_, generation)) => {
                format!("{}@{generation}", carranta_bot::TRAINED)
            }
            _ => format!("{}@{}", carranta_bot::HOUSE, carranta_bot::HOUSE_VERSION),
        }
    }

    /// Every champion generation seated here, lowest first, for saying who is
    /// at the table without asking seat by seat.
    pub fn champions(&self) -> Vec<u32> {
        let mut seen: Vec<u32> = self
            .bots
            .iter()
            .filter_map(|b| match b {
                Brain::Trained(_, generation) => Some(*generation),
                Brain::House(_) => None,
            })
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen
    }

    /// Name the person at this browser. Empty falls back rather than showing a
    /// blank seat.
    pub fn with_name(mut self, name: &str) -> Self {
        let name = name.trim();
        if !name.is_empty() {
            self.name = name.chars().take(24).collect();
        }
        self.names[HUMAN as usize] = self.name.clone();
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// What one seat is called, or empty if nobody has said.
    ///
    /// Every seat rather than the one at the keyboard, because with two people
    /// at a table there is no such thing as "the" name. A seat with nobody in it
    /// has none: the page names the house bot itself, from a list it owns, and a
    /// name invented here would be a second opinion about it.
    pub fn name_of(&self, seat: u8) -> &str {
        self.names.get(seat as usize).map_or("", String::as_str)
    }

    /// Name a seat, which is what sitting down in one does.
    pub fn name_seat(&mut self, seat: u8, name: &str) {
        let name = name.trim();
        if (seat as usize) < SEATS {
            self.names[seat as usize] = name.chars().take(24).collect();
        }
        if seat == HUMAN && !name.is_empty() {
            self.name = self.names[HUMAN as usize].clone();
        }
    }

    /// Every seat's name, in seat order, for writing down and for the view.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Play with the record hidden, so the table has to remember.
    pub fn with_log(mut self, shown: bool) -> Self {
        self.log_shown = shown;
        self
    }

    pub fn log_shown(&self) -> bool {
        self.log_shown
    }

    /// What this table is called. Empty means it was never named, which is not
    /// an error: a table of four people who all know each other needs no name,
    /// and inventing one for them would put a label on the screen that nobody
    /// chose.
    pub fn with_game(mut self, game: &str) -> Self {
        self.game = game.trim().chars().take(40).collect();
        self
    }

    pub fn game(&self) -> &str {
        &self.game
    }

    /// Show the bank as exact counts, or only as how big each stack looks.
    pub fn with_bank_exact(mut self, exact: bool) -> Self {
        self.bank_exact = exact;
        self
    }

    pub fn bank_exact(&self) -> bool {
        self.bank_exact
    }

    /// Whether the table is listed for anyone to join, or reachable only by
    /// its invite link.
    ///
    /// A property of the table rather than of the browser that dealt it: the
    /// listing will be read from here, not from whoever happens to be looking.
    /// Nothing lists tables yet, so nothing reads this yet either.
    /// Sit people in these seats, and bots in the rest.
    ///
    /// Exactly these seats. It used to force seat nought as well, on the grounds
    /// that a table always has its dealer at it, which stopped being true the
    /// moment the turn order was shuffled: a bot dealt into seat nought would
    /// have been treated as a person, and the table would have waited on it for
    /// ever.
    pub fn with_people(mut self, seats: &[u8]) -> Self {
        self.seat_people(seats);
        self
    }

    /// The same, on a table already dealt, which is what somebody sitting down
    /// mid-game is.
    pub fn seat_people(&mut self, seats: &[u8]) {
        self.people = [false; SEATS];
        for &s in seats {
            if (s as usize) < SEATS {
                self.people[s as usize] = true;
            }
        }
    }

    /// Whether a person is sitting in this seat, rather than the house bot.
    pub fn is_person(&self, seat: u8) -> bool {
        self.people.get(seat as usize).copied().unwrap_or(false)
    }

    /// The seats people are sitting in, in seat order.
    pub fn people(&self) -> Vec<u8> {
        (0..self.seats).filter(|&s| self.is_person(s)).collect()
    }

    /// Whether the game has begun.
    ///
    /// The first move is the door closing: before it the table is still being
    /// filled and anybody may take a chair, after it the seating is what it is.
    /// The moves are the record, so this asks the record rather than keeping a
    /// flag that could disagree with it.
    pub fn started(&self) -> bool {
        !self.moves.is_empty()
    }

    /// Whether anything is being asked of any person at the table.
    ///
    /// What stops the bots. It used to be "is anything being asked of the
    /// human", which is the same sentence with one seat in it, and is why a
    /// second person's turn would have been played over the top of them.
    fn waiting_on_a_person(&self) -> bool {
        (0..self.seats).any(|s| self.is_person(s) && !self.choices_for(s).is_empty())
    }

    pub fn with_public(mut self, public: bool) -> Self {
        self.public = public;
        self
    }

    pub fn is_public(&self) -> bool {
        self.public
    }

    /// Whole seconds the game has been going, and no longer than it went.
    pub fn elapsed_secs(&self) -> u64 {
        self.since(self.started).as_secs()
    }

    pub fn clock(&self) -> Clock {
        self.clock
    }

    /// The turn number to show.
    pub fn turn_no(&self) -> u32 {
        self.turns
    }

    /// Seconds each finished turn took, oldest first.
    pub fn turn_ms(&self) -> &[u32] {
        &self.turn_ms
    }

    /// Whether the board is still being dealt.
    pub fn in_setup(&self) -> bool {
        matches!(
            self.state.phase,
            Phase::SetupSettlement { .. } | Phase::SetupRoad { .. }
        )
    }

    /// Whose clock is running: whoever's turn it is, and nobody else.
    ///
    /// What the passive players do inside that turn, answering an offer,
    /// discarding on a seven, being robbed, is their business and costs them
    /// nothing. If none of them reacts, the turn simply runs out and the next
    /// one starts.
    ///
    /// This used to hand the clock to whoever owed an answer, on the reasoning
    /// that the wait belongs to whoever is holding everyone up. That put a
    /// player's own expired allowance onto somebody else's turn, with two
    /// consequences: their clock read a stuck 0:00 through turns that were not
    /// theirs, and every offer that arrived afterwards was declined by the
    /// clock on the same request that created it, so the trade card never got
    /// drawn once.
    pub fn on_clock(&self) -> u8 {
        self.turn_holder
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
                self.note_at(at, Some(seat as u8), format!("Collected {got}"));
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
            self.note_at(
                at,
                Some(seat),
                format!("Took a card, unseen, from seat {from}"),
            );
        }
    }

    /// Add a line for something happening now rather than for an action.
    fn note(&mut self, seat: Option<u8>, text: String) {
        let at = self.stamp();
        self.note_at(at, seat, text);
    }

    /// Say something to the whole table that is not a move.
    ///
    /// Somebody sitting down, which happens to a game rather than in it: it
    /// changes who is answering for a seat and changes nothing about the
    /// position, so it is a line in the log and not a step in the record. A
    /// game replayed from its file is the same game whether or not anybody
    /// joined it halfway.
    pub fn note_to_table(&mut self, text: String) {
        self.note(None, text);
    }

    /// Wave away every offer currently open, as declining does.
    fn decline_open_offers(&mut self, seat: u8) {
        // Everything put to you, not only what you could have taken: turning
        // down an offer you cannot cover is the same answer and the table needs
        // it just as much.
        for i in self.offers_to(seat) {
            self.answer(i, seat, Answer::No);
        }
    }

    /// Settle the running clock against whoever it belongs to, then hand it to
    /// whoever is deciding now. Called after anything that could move the turn.
    fn hand_over_clock(&mut self) {
        let now = std::time::Instant::now();
        // Charged to whoever's turn it is, which is the only seat a clock ever
        // runs against. Read before the turn changes hands below, so the time
        // lands on the player who spent it rather than on the next one.
        //
        // Less whatever of the stretch went on a discard: the seven's own
        // allowance covers that, and the turn holder is not paying for it.
        //
        // Before the turn boundary below, so a hold that has just ended is
        // credited against the turn it interrupted and not the next one.
        self.follow_discard();
        let owner = self.turn_holder as usize;
        if owner < MAX_PLAYERS {
            self.spent[owner] += (now - self.last_settle).saturating_sub(self.paused_unsettled());
        }
        self.last_settle = now;
        self.paused_settled = self.paused();

        // A turn is everything between one player ending theirs and the next
        // player ending theirs, and a placement in the deal is a turn of its
        // own. Nothing else starts one.
        //
        // A change of decider does not. Discarding on a seven, choosing whether
        // to take an offer, being robbed: those are things the passive players
        // do inside somebody else's turn, and counting them as turns split one
        // turn into four and filed each player's discard under a turn of their
        // own. The deal runs as a snake, so the turn also changes hands twice
        // without the decider changing, which is the other half of why the
        // decider cannot be the signal.
        //
        // Entering PreRoll marks the start of a turn of play, entering
        // SetupSettlement the start of a placement, and each happens exactly
        // once per turn.
        let deciding = self.state.decider();
        let preroll = matches!(self.state.phase, Phase::PreRoll);
        let placing = matches!(self.state.phase, Phase::SetupSettlement { .. });
        let starting_a_turn = (preroll && !self.was_preroll) || (placing && !self.was_placing);
        self.was_preroll = preroll;
        self.was_placing = placing;
        if starting_a_turn {
            // Close the books on the turn that is ending before the next one
            // starts.
            // Milliseconds, not seconds. A bot answers in a fraction of one,
            // and truncating that to zero left every turn but a slow human's
            // with no duration at all against it.
            let took = (now - self.turn_at).as_millis() as u32;
            let ending = self.turns as usize;
            if ending >= 1 {
                while self.turn_ms.len() < ending {
                    self.turn_ms.push(0);
                }
                self.turn_ms[ending - 1] = took;
            }
            // Finishing a turn credits the increment back, which is the whole
            // difference between a chess clock and a countdown: a player who
            // keeps moving keeps playing, and the game is decided on the board.
            let inc = self.clock.increment();
            if inc > 0 {
                let prev = self.turn_holder as usize;
                if prev < MAX_PLAYERS {
                    self.spent[prev] =
                        self.spent[prev].saturating_sub(std::time::Duration::from_secs(inc));
                }
            }
            // Offers do not survive the turn they were made in, and neither
            // does the record of who said what to them. A card still showing
            // last turn's replies is a question about a table that has been
            // cleared.
            self.deals.clear();
            self.turn_holder = deciding;
            self.turn_at = now;
            self.turn_began = now;
            // A fresh turn owes nothing to the last one's interruptions, and a
            // hold still running belongs to the turn that is starting.
            self.turn_paused = std::time::Duration::ZERO;
            self.paused_settled = std::time::Duration::ZERO;
            self.turns += 1;
        }
    }

    /// Time a seat has used, including the stretch not yet settled and less
    /// whatever of it went on waiting for a discard, which is nobody's turn.
    fn used(&self, seat: u8) -> std::time::Duration {
        let mut d = self.spent[seat as usize];
        if seat == self.turn_holder {
            d += self
                .since(self.last_settle)
                .saturating_sub(self.paused_unsettled());
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
            Clock::PerTurn(n) => Some(if seat == self.turn_holder {
                // Less the time held for a discard: a seven interrupts the turn
                // holder, who did nothing to deserve it but roll.
                let spent = self.since(self.turn_began).saturating_sub(self.paused());
                n as i64 - spent.as_secs() as i64
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

    /// Discard for whoever ran out of time to do it themselves.
    ///
    /// The one place a clock takes cards out of a hand. A discard cannot be
    /// passed: the rules give no way to decline it and the position is illegal
    /// until it is done, so the alternative to choosing badly for someone is
    /// the game stopping on them. Random and stated is the lesser of those.
    ///
    /// Drawn from the forfeit's own generator, like every other forced move, so
    /// the same seed with the same timings discards the same cards.
    fn enforce_discard(&mut self) {
        if self.discard_secs == 0 || !self.discarding() {
            return;
        }
        if self.discard_left().is_some_and(|t| t > 0) {
            return;
        }
        let mut buf = Vec::new();
        // Every discard still owed, by everyone who owes one: the seven is over
        // when the table has paid for it, not when one player has.
        for _ in 0..64 {
            if !matches!(self.state.phase, Phase::Discard) {
                break;
            }
            let seat = self.state.decider();
            let at = self.stamp();
            self.state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let forced = buf[self.forfeit.below(Stream::Steal, buf.len() as u32) as usize];
            let phrase = log_phrase(&forced, &self.state, seat as usize);
            if self.state.apply(forced).is_err() {
                break;
            }
            self.record(Step::Move(forced));
            self.version += 1;
            self.note_at(
                at,
                Some(seat),
                format!("Time ran out, {}", lower_first(phrase)),
            );
        }
        self.finish_move();
    }

    /// End a turn whose clock has run out.
    ///
    /// Enforced lazily, on the way in to a request, because a server that only
    /// wakes when asked cannot end a turn at the exact second. Only ever ends a
    /// turn that could legally be ended. A clock should not be able to skip a
    /// setup placement or a discard, which would leave the position illegal.
    pub fn enforce_clock(&mut self) {
        self.enforce_discard();
        if self.clock == Clock::Off {
            return;
        }
        // Bounded rather than looped to exhaustion: an empty chess bank ends
        // every turn the moment it starts, and that should play out across
        // requests instead of inside one of them.
        for _ in 0..8 {
            // The clock belongs to the turn, so it is the turn holder's
            // allowance that says whether time is up, whoever happens to be
            // deciding at this instant.
            // A discard has an allowance of its own, and `enforce_discard` is
            // what spends it. The turn clock is held while one is owed, so it
            // must not reach past the hold and force the same cards on a
            // different timer.
            if self.discarding() {
                return;
            }
            let holder = self.on_clock();
            if !self.out_of_time(holder) {
                return;
            }
            // An offer nobody took before the turn ran out is an offer refused.
            // Clearing it is what lets the turn end rather than standing open
            // on a passive player who never clicked, and it is the only thing a
            // turn's clock does to a seat that is not holding the turn.
            //
            // Everything put to any person, not only what they could have
            // covered. What blocks the table is being *asked*: `choices_for`
            // offers a decline for any question put to a seat, and a pending
            // choice is what stops the bots. Keying the escape on what could
            // have been accepted meant an offer nobody could afford blocked the
            // turn and then was not cleared when the clock ran out, so the table
            // sat at 0:00 waiting on an answer to an offer nobody could take.
            //
            // Every person's, not only the turn holder's: an offer is put to the
            // whole table, and one unanswered question anywhere stops it.
            let asked: Vec<u8> = (0..self.seats)
                .filter(|&s| self.is_person(s) && !self.offers_to(s).is_empty())
                .collect();
            if !asked.is_empty() {
                for seat in asked {
                    self.decline_open_offers(seat);
                    self.note(Some(seat), "Time ran out, declined the offers".to_string());
                }
                self.finish_move();
                continue;
            }
            // What is left to force is a turn, whoever holds it. This used to
            // exempt bots, on the argument that a bot moves the moment it is
            // asked to, so its allowance could only run out while an offer
            // blocked it, and the offers above are cleared. That was true of
            // the heuristic and stopped being true the day a trained bot sat
            // down: its market appetite runs to twenty paced offers a turn,
            // each drawn on its own beat and answered on the table's, which
            // outlasts any allowance a person would accept. The screen showed
            // exactly what the exemption implies, a turn standing at 0:00 for
            // minutes while the bot went on trading. The clock is a table
            // rule, and a seat is on it whoever plays the seat.
            //
            // A mandatory answer owed by a *passive* player, discarding on a
            // seven, is still not the clock's to skip, since the position
            // would be illegal without it: that is the `holder != acting`
            // half, unchanged.
            let acting = self.state.decider();
            if holder != acting {
                return;
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
            // Read before the move, because what it pays out is the difference
            // between the hands either side of it.
            let purse = self.state.hand;
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
            self.record(Step::Move(forced));
            self.version += 1;
            // "for you" is only true of a person: the line is addressed to
            // somebody whose move was made on their behalf. A bot forced by
            // the clock did the thing, and its line reads like any other move
            // of its, only prefixed with why.
            let done = |text: String| {
                if self.is_person(acting) {
                    format!("{} for you", lower_first(text))
                } else {
                    lower_first(text)
                }
            };
            let what = match forced {
                Action::EndTurn => "the turn was ended".to_string(),
                Action::Roll => done(rolled(&self.state)),
                other => done(log_phrase(&other, &self.state, acting as usize)),
            };
            self.note_at(at, Some(acting), format!("Time ran out, {what}"));
            // A forced move pays out exactly like a chosen one, because the
            // engine does not know the difference: the same roll deals the same
            // cards to the same seats. What was missing was the record of it.
            // The log showed "Time ran out, rolled 8 for you" and then nothing,
            // and a roll that pays nobody is the one thing a roll cannot do.
            if pays_out(&forced) {
                self.note_production(at, &purse);
            }
            self.note_steal(at, &forced, acting, &purse);
            self.sync_deals();
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

    /// What this game was dealt from, and everything that has happened since.
    ///
    /// Enough to rebuild the game exactly: `Session::replay` folds it back into
    /// the same position, and `carranta-record` folds it into a `Log` for the
    /// analytics to read.
    pub fn table(&self) -> (u8, u64, TradeMode) {
        (self.seats, self.seed, self.mode)
    }

    pub fn moves(&self) -> &[Step] {
        &self.moves
    }

    /// When each step landed, or empty on a game rebuilt without its clock.
    pub fn times(&self) -> &[u32] {
        &self.times
    }

    /// Write a step down, and when it happened.
    ///
    /// The one place either list grows, so they cannot fall out of step with
    /// each other. Times that had drifted by one would be attributed to the
    /// wrong turns, which is a wrong answer rather than a missing one.
    fn record(&mut self, step: Step) {
        self.moves.push(step);
        // Saturating rather than wrapping: a game left open for seven weeks
        // should read as a very long game, not as a very short one.
        self.times
            .push(self.started.elapsed().as_millis().min(u32::MAX as u128) as u32);
    }

    /// Play the whole game out, every seat by the table's own hand.
    ///
    /// For filling a store with games to look at, and for tests that need a
    /// finished one. The human's seat is played by the same heuristic the bots
    /// use rather than by picking the first legal move, so the result is a game
    /// somebody could have played instead of a walk through the rules.
    ///
    /// Runs to a winner in practice and is not promised one: the loop stops if
    /// the table runs out of legal moves or refuses one, so callers who need a
    /// finished game check [`Session::winner`] rather than assume.
    pub fn play_out(&mut self) {
        let mut buf = Vec::new();
        for _ in 0..20_000 {
            if matches!(self.state.phase, Phase::GameOver { .. }) {
                break;
            }
            self.state.legal_into(&mut buf);
            if buf.is_empty() {
                break;
            }
            let seat = self.state.decider() as usize;
            let action = self.bots[seat].choose(&self.state, &buf);
            if !self.narrate(action) {
                break;
            }
            // And then the table answers whatever was put to it. Without this
            // an offer made here was never put to anybody at all: not taken,
            // not turned down, just left on the table until its maker withdrew
            // it. Every game played this way had a market that could only trade
            // with the bank, which is not a market, and the analytics said so
            // before anybody noticed the code did it.
            //
            // Every seat, the human's included, because in a game played out
            // there is nobody here to ask and the seat is the table's own hand.
            self.settle_between_bots(true);
        }
        self.note_winner();
    }

    /// The board a table starts from, before anybody has placed anything.
    pub fn opening(seats: u8, seed: u64, mode: TradeMode) -> State {
        State::new(seats.clamp(3, MAX_PLAYERS as u8), seed).with_trade_mode(mode)
    }

    /// Rebuild a played game's position from what was written down about it.
    ///
    /// Returns `None` at the first move the engine refuses, which means the
    /// file and this build disagree about the rules rather than that the file
    /// is unreadable. Better to say so than to serve half a game as a whole one.
    pub fn replay(seats: u8, seed: u64, mode: TradeMode, moves: &[Step]) -> Option<State> {
        let mut state = State::new(seats.clamp(3, MAX_PLAYERS as u8), seed).with_trade_mode(mode);
        for step in moves {
            if let Step::Move(action) = step {
                state.apply(*action).ok()?;
            }
        }
        Some(state)
    }

    /// Play a whole recorded game back into a session, narrating as it goes.
    ///
    /// The moves alone rebuild the position; this rebuilds the *account* of it
    /// as well, so a game reopened from disk reads the way it read while it was
    /// being played. There is no pace and no clock here: the game already
    /// happened, and this is reading it rather than playing it.
    pub fn resume(seats: u8, seed: u64, mode: TradeMode, moves: &[Step]) -> Option<Session> {
        let mut s = Session::new(seats, seed, mode);
        for step in moves {
            let ok = match *step {
                Step::Move(action) => s.narrate(action),
                Step::Passed { offer, by } => s.pass_again(offer, by),
            };
            if !ok {
                return None;
            }
        }
        s.note_winner();
        Some(s)
    }

    /// Take a resumed game's clock up where its record left off.
    ///
    /// [`Session::resume`] replays the moves and stamps each one at the moment
    /// it is replayed, so a game an hour long comes back as a game four
    /// milliseconds long. The recorded times are the real ones and are put back,
    /// and the session's own origin is wound back to the last of them: whatever
    /// happens next lands after everything that already has, rather than in the
    /// first second of a game that is an hour old. The time the server spent
    /// stopped is not counted, because nobody was thinking during it.
    ///
    /// Ignored rather than trusted when the two lists are different lengths. A
    /// step with the wrong time on it is a wrong answer, where a step with no
    /// time is a missing one, and the file format has always allowed the second.
    pub fn with_record(mut self, times: Vec<u32>) -> Self {
        if times.len() != self.moves.len() {
            return self;
        }
        if let Some(&last) = times.last()
            && let Some(origin) = std::time::Instant::now()
                .checked_sub(std::time::Duration::from_millis(u64::from(last)))
        {
            self.started = origin;
        }
        self.times = times;
        self
    }

    /// Say no again, on the way back through a recorded game.
    ///
    /// Written out rather than routed through `answer`, which would push the
    /// refusal onto the record a second time: this is reading the record, not
    /// adding to it.
    fn pass_again(&mut self, at: u8, seat: u8) -> bool {
        let from = match self.deals.iter().find(|d| d.at == Some(at)) {
            Some(d) => d.offer.from,
            None => return false,
        };
        if let Some(d) = self.deals.iter_mut().find(|d| d.at == Some(at)) {
            d.answers[seat as usize] = Answer::No;
        }
        self.record(Step::Passed {
            offer: at,
            by: seat,
        });
        // First person for a person's seat, because the page prefixes a line
        // with whose it is unless it is yours: "Passed on the offers" reads
        // right on your own screen and as "Ines passed on the offers" on
        // somebody else's. A bot's line names the offer it turned down, since
        // nobody is reading it as their own.
        if self.is_person(seat) {
            self.note(Some(seat), "Passed on the offers".to_string());
        } else {
            self.note(Some(seat), format!("Passed on seat {from}'s offer"));
        }
        self.version += 1;
        true
    }

    /// The line that closes a finished game, wherever the game finished.
    fn note_winner(&mut self) {
        if let Phase::GameOver { winner } = self.state.phase {
            // Stop the clocks, once, at the first moment anybody noticed the
            // game was over. Every path that can finish a game comes through
            // here, which is why the stamp lives here and not beside each of
            // them. `get_or_insert_with` rather than a plain assignment
            // because this is called again on every later poll, and a stamp
            // that moved would be a stopped clock that crept.
            self.ended.get_or_insert_with(std::time::Instant::now);
            // One person at the table and they won: "You win" is addressed to
            // the only reader there is. With two, the log is shared and read
            // from two seats at once, so the line has to name the winner rather
            // than address one of them.
            let who = if self.people().len() == 1 && self.is_person(winner) {
                "You win".to_string()
            } else {
                format!("Seat {winner} wins")
            };
            if self.log.last().map(|l| l.text.as_str()) != Some(who.as_str()) {
                self.note(None, who);
            }
        }
    }

    /// Apply one action and write down what it did, as any of the paths that
    /// apply an action would have.
    fn narrate(&mut self, action: Action) -> bool {
        let seat = self.state.decider();
        let phrase = log_phrase(&action, &self.state, seat as usize);
        let at = self.stamp();
        let purse = self.state.hand;
        if let Action::AcceptTrade { offer, .. } = action {
            self.answer(offer, seat, Answer::Yes);
        }
        if self.state.apply(action).is_err() {
            return false;
        }
        self.record(Step::Move(action));
        self.version += 1;
        let phrase = match action {
            Action::Roll => rolled(&self.state),
            _ => phrase,
        };
        if worth_logging(&action) {
            self.note_at(at, Some(seat), phrase);
        }
        if pays_out(&action) {
            self.note_production(at, &purse);
        }
        self.note_steal(at, &action, seat, &purse);
        self.sync_deals();
        self.hand_over_clock();
        true
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn log(&self) -> &[LogLine] {
        &self.log
    }

    /// What the seat at the keyboard is entitled to see.
    pub fn view(&self) -> Fog {
        self.view_for(HUMAN)
    }

    /// What one seat is entitled to see.
    pub fn view_for(&self, seat: u8) -> Fog {
        fog(&self.state, Viewer::Seat(seat))
    }

    /// What somebody watching the table is entitled to see: the public position
    /// and nobody's hand, which is what a person standing behind the players
    /// sees (P-6).
    pub fn view_watching(&self) -> Fog {
        fog(&self.state, Viewer::Spectator)
    }

    /// The choices to put in front of the seat at the keyboard.
    pub fn choices(&self) -> Vec<Choice> {
        self.choices_for(HUMAN)
    }

    /// The choices to put in front of one seat, in a stable order.
    ///
    /// Empty while it is somebody else's turn and nothing is being asked of
    /// this seat. Every action arrives with an index into this list, so the
    /// order is part of the interface and the list is per seat: two people at
    /// one table are looking at two different lists at the same moment.
    pub fn choices_for(&self, seat: u8) -> Vec<Choice> {
        // Somebody watching has nothing to press, and asking on their behalf
        // reaches code that indexes by seat. See `is_seat`.
        if !self.is_seat(seat) {
            return Vec::new();
        }
        if matches!(self.state.phase, Phase::GameOver { .. }) {
            return Vec::new();
        }
        if self.state.decider() == seat {
            let mut buf = Vec::new();
            self.state.legal_into(&mut buf);
            return buf.into_iter().map(Choice::Play).collect();
        }
        // Not their turn, but an offer may be waiting for them.
        let mut out: Vec<Choice> = self
            .open_offers_for(seat)
            .into_iter()
            .map(|i| Choice::Play(Action::AcceptTrade { offer: i, by: seat }))
            .collect();
        // Saying no is offered for anything put to you, not only for what you
        // could say yes to. An offer you cannot cover is still a question, and
        // one you can neither take nor turn down is a question with no answer:
        // it sat there invisible, and the table waited on it until the turn ran
        // out.
        if !self.offers_to(seat).is_empty() {
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
        self.can_propose_for(HUMAN)
    }

    /// Whether a number names a seat at this table.
    ///
    /// Asked before anything indexes by it. The view renders for somebody with
    /// no seat by passing one that does not exist, which is what makes the
    /// redaction safe rather than careful: a hand nobody is holding is a hand
    /// of nothing. That only holds if every question asked of such a seat
    /// *answers*, and one of them indexed instead, which took the process down
    /// rather than merely answering wrongly. See `can_propose_for`.
    fn is_seat(&self, seat: u8) -> bool {
        (seat as usize) < self.state.players as usize
    }

    /// The same question for one seat.
    pub fn can_propose_for(&self, seat: u8) -> bool {
        // Nobody's seat can put nothing on the table. This is the guard whose
        // absence crashed the server: a spectator is rendered for seat 255,
        // the two checks below let that through once a game reached its action
        // phase, and the probe underneath indexed a four-seat hand with it.
        if !self.is_seat(seat) {
            return false;
        }
        if self.state.trade_mode == TradeMode::Disabled {
            return false;
        }
        if !matches!(self.state.phase, Phase::Action) {
            return false;
        }
        // A probe rather than a second copy of the rules: whatever the engine
        // would accept is what the form should allow.
        for r in 0..5 {
            if self.state.hand[seat as usize][r] == 0 {
                continue;
            }
            let mut give = [0u8; 5];
            give[r] = 1;
            let mut want = [0u8; 5];
            want[(r + 1) % 5] = 1;
            let mut probe = self.state;
            if probe
                .apply(Action::ProposeTrade {
                    by: seat,
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
    fn open_offers_for(&self, seat: u8) -> Vec<u8> {
        self.offers_to(seat)
            .into_iter()
            .filter(|&i| {
                let mut probe = self.state;
                probe
                    .apply(Action::AcceptTrade { offer: i, by: seat })
                    .is_ok()
            })
            .collect()
    }

    /// Offers the human was *asked* about and has not answered, whether or not
    /// they can cover them.
    ///
    /// Not the same list as `open_offers`, and the difference was a bug worth
    /// naming. An offer you cannot afford is still a question put to you: it
    /// has to appear, say that you cannot cover it, and let you say no. Keying
    /// the whole card on what you could accept meant an offer you could not
    /// afford was silently invisible, which reads exactly like the game
    /// dropping trades on the floor.
    ///
    /// Whether you were asked at all is R-7.3 and nothing to do with your hand:
    /// a passive player's offer goes to the active player alone, so on a bot's
    /// turn another bot's offer is genuinely not yours to answer, and no card
    /// for that one is right.
    fn offers_to(&self, seat: u8) -> Vec<u8> {
        if self.state.trade_mode == TradeMode::Disabled {
            return Vec::new();
        }
        (0..self.state.offer_count)
            // Asked once. A decline sticks to the offer it was made about
            // rather than to a slot in the market, so an offer arriving later
            // is a new question and gets asked.
            .filter(|&i| {
                self.deals
                    .iter()
                    .find(|d| d.at == Some(i))
                    .is_none_or(|d| d.answers[seat as usize] == Answer::Waiting)
            })
            .filter(|&i| {
                self.state
                    .may_accept(seat as usize, &self.state.offers[i as usize])
            })
            .collect()
    }

    /// Apply the choice of the seat at the keyboard, then let the bots run on.
    pub fn act(&mut self, index: usize, version: u64) -> Result<(), Refused> {
        self.act_as(HUMAN, index, version)
    }

    /// Apply one seat's choice, then let the bots run on.
    ///
    /// The index is into that seat's own `choices_for`, so a click from one
    /// person cannot land on another person's list: the worst a stale or
    /// mischievous index can do is name a choice this seat does not have, which
    /// is refused. Whether the seat is one this caller is allowed to play is a
    /// question about who is asking and belongs to the server, which knows.
    pub fn act_as(&mut self, seat: u8, index: usize, version: u64) -> Result<(), Refused> {
        if version != self.version {
            return Err(Refused::Stale);
        }
        let choice = self
            .choices_for(seat)
            .into_iter()
            .nth(index)
            .ok_or(Refused::NoSuchChoice)?;

        match choice {
            Choice::Decline => {
                self.undo = None;
                self.decline_open_offers(seat);
                self.note(Some(seat), "Passed on the offers".to_string());
            }
            Choice::Play(action) => {
                // Named before it is applied: a phrase describes the position
                // the action was taken in, not the one it produced.
                let phrase = log_phrase(&action, &self.state, seat as usize);
                let at = self.stamp();
                let purse = self.state.hand;
                // A militia is not one decision but two, and the card is spent
                // on the first of them. Keep the position from before it so the
                // half-made move can be put back, see `cancel`.
                let mark = matches!(action, Action::PlayMilitia)
                    .then(|| (self.state, self.log.len(), self.moves.len()));
                // Recorded before it is applied, because taking an offer is
                // what removes it: afterwards there is no offer to answer.
                if let Action::AcceptTrade { offer, .. } = action {
                    self.answer(offer, seat, Answer::Yes);
                }
                self.state.apply(action).map_err(Refused::Illegal)?;
                self.record(Step::Move(action));
                self.undo = mark;
                self.version += 1;
                let phrase = match action {
                    Action::Roll => rolled(&self.state),
                    _ => phrase,
                };
                self.note_at(at, Some(seat), phrase);
                if pays_out(&action) {
                    self.note_production(at, &purse);
                }
                self.note_steal(at, &action, seat, &purse);
                self.sync_deals();
            }
        }
        self.finish_move();
        Ok(())
    }

    /// Whether the half-made move on the table can still be put back.
    ///
    /// Having a snapshot is not enough: the position has to still be the one it
    /// was taken for. Checking that here rather than clearing the snapshot at
    /// every place a move can come from means a path nobody thought of cannot
    /// leave a stale offer to undo something else.
    pub fn can_cancel(&self) -> bool {
        self.can_cancel_for(HUMAN)
    }

    /// The same question for one seat: only the seat that played the card can
    /// put it back, which is the seat now being asked where the robber goes.
    pub fn can_cancel_for(&self, seat: u8) -> bool {
        self.undo.is_some()
            && self.state.decider() == seat
            && matches!(self.state.phase, Phase::MoveRobber { from_militia: true })
    }

    /// Put back a development card whose action was never finished.
    ///
    /// A militia spends the card the moment it is played, and only then asks
    /// where the robber goes. Between those two things the player has committed
    /// to nothing and learned nothing, so the position from before is restored
    /// whole, along with the log line that announced it.
    ///
    /// The snapshot is the entire `State`, which carries its own generator, so
    /// this cannot be used to fish for a different roll or a different steal:
    /// what comes back is the same position down to the next random number.
    /// It is dropped the moment anything else happens, so there is never more
    /// than the current half-move to take back.
    pub fn cancel(&mut self, version: u64) -> Result<(), Refused> {
        self.cancel_as(HUMAN, version)
    }

    /// The same, for one seat.
    pub fn cancel_as(&mut self, seat: u8, version: u64) -> Result<(), Refused> {
        if version != self.version {
            return Err(Refused::Stale);
        }
        if !self.can_cancel_for(seat) {
            self.undo = None;
            return Err(Refused::NoSuchChoice);
        }
        let (state, lines, moves) = self.undo.take().ok_or(Refused::NoSuchChoice)?;
        self.state = state;
        self.log.truncate(lines);
        // And the move with it. The position is being put back whole, so a
        // record that still carried the militia would replay into a different
        // game from the one being played.
        self.moves.truncate(moves);
        self.version += 1;
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
        self.propose_as(HUMAN, to, give, want, version)
    }

    /// The same, from one seat.
    pub fn propose_as(
        &mut self,
        seat: u8,
        to: Option<u8>,
        give: [u8; 5],
        want: [u8; 5],
        version: u64,
    ) -> Result<(), Refused> {
        if version != self.version {
            return Err(Refused::Stale);
        }
        let action = Action::ProposeTrade {
            by: seat,
            to,
            give,
            want,
        };
        let phrase = log_phrase(&action, &self.state, seat as usize);
        let at = self.stamp();
        self.state.apply(action).map_err(Refused::Illegal)?;
        self.record(Step::Move(action));
        self.version += 1;
        self.note_at(at, Some(seat), phrase);
        self.sync_deals();
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
        self.settle_between_bots(false);
        self.run_bots();
        self.hand_over_clock();
    }

    /// How the bots are paced, and whether one is mid-thought right now.
    pub fn pace(&self) -> Pace {
        self.pace
    }

    pub fn with_pace(mut self, pace: Pace) -> Self {
        self.pace = pace;
        self
    }

    /// Whether the table has something for a bot to do right now.
    ///
    /// Nothing to do with pace: this is about the position, not about how long
    /// the answer is being held back for. An offer on the table is a bot about
    /// to answer it, whoever's turn it is. Otherwise a bot is to move when the
    /// seat deciding is not a person and nothing is being asked of anybody: the
    /// same question with "a person" in place of "seat nought", so a table with
    /// two people at it is not read as waiting on the bots through both of
    /// their turns.
    fn bots_to_move(&self) -> bool {
        if matches!(self.state.phase, Phase::GameOver { .. }) {
            return false;
        }
        self.state.offer_count > 0
            || (!self.is_person(self.state.decider()) && !self.waiting_on_a_person())
    }

    /// Whether a bot has a move to make and is still waiting to make it.
    ///
    /// The page uses this to poll quickly while the table is moving and slowly
    /// while it is not, rather than either polling fast forever or letting a
    /// paced move sit unseen until the next lazy tick. An instant table has
    /// nothing to wait for, so there is never anything mid-thought on one.
    pub fn bot_thinking(&self) -> bool {
        self.pace != Pace::Instant && self.bots_to_move()
    }

    /// Let a stalled table move on.
    ///
    /// Bots are advanced from the human's move, which is fine when they answer
    /// instantly and a deadlock when they do not: a paced bot breaks out of the
    /// loop to wait, and without this nothing ever asks it again. The server
    /// calls this on every poll, which is also the only clock this process has.
    ///
    /// Asked of the position rather than of the pace, which is the fix for a
    /// table that never began. A move is what runs the bots, and before the
    /// first one there has been no move: while seat nought was always the human
    /// that was invisible, because the human was the one being asked. Once the
    /// turn order is drawn a bot can hold seat nought, and on an instant table
    /// `bot_thinking` is false by definition, so nothing ever asked it to play
    /// and the table sat at turn one for ever.
    pub fn tick(&mut self) {
        if self.bots_to_move() {
            self.finish_move();
        }
    }

    /// Whether the next bot action may happen yet, arming the wait if it may.
    ///
    /// One gate for everything a bot does, because everything a bot does is
    /// something a person is watching for. Making a move and answering somebody
    /// else's offer are both events to a reader, so both come through here;
    /// they are drawn from different windows, because reading an offer takes
    /// longer than watching a move.
    fn beat_due(&mut self, (lo, hi): (u64, u64)) -> bool {
        if hi == 0 {
            return true;
        }
        let now = std::time::Instant::now();
        if now < self.bot_ready {
            return false;
        }
        let span = hi - lo + 1;
        // Its own generator, so which stream it draws from does not matter:
        // nothing else reads this one. The same convention the forfeit picker
        // uses.
        let wait = lo + self.tempo.below(Stream::Steal, span as u32) as u64;
        self.bot_ready = now + std::time::Duration::from_millis(wait);
        true
    }

    /// Advance until the human has something to decide, or the game ends.
    ///
    /// A paced table stops as soon as the next move is not due yet and comes
    /// back on the following request. The position is left exactly where the
    /// bot found it, so a page that never asks again simply never sees the
    /// move; nothing is half-applied while the wait runs.
    fn run_bots(&mut self) {
        let mut buf = Vec::new();
        for _ in 0..20_000 {
            if matches!(self.state.phase, Phase::GameOver { .. }) {
                break;
            }
            // Somebody's move, or somebody's question: either way the table
            // waits. This used to name one seat, which is exactly the sentence
            // that made a second person impossible.
            if self.is_person(self.state.decider()) || self.waiting_on_a_person() {
                break;
            }
            if !self.beat_due(self.pace.window()) {
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
            // The active player may take an offer as an ordinary move rather
            // than through the market settle, and that is still an answer:
            // recorded here too, or the card would leave them at "???" over an
            // offer they had already taken.
            if let Action::AcceptTrade { offer, .. } = action {
                self.answer(offer, seat as u8, Answer::Yes);
            }
            if self.state.apply(action).is_err() {
                break;
            }
            self.record(Step::Move(action));
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
            self.sync_deals();
            // Each bot pays for its own thinking, and the turn passing between
            // two bots is still the turn passing.
            self.hand_over_clock();
            self.settle_between_bots(false);
        }
        self.note_winner();
    }

    /// Let the bots answer the offers on the table, one seat per beat.
    ///
    /// The human is asked separately, by being offered the choice rather than
    /// answered on their behalf.
    ///
    /// One seat at a time and not the whole table at once, because a proposal
    /// being answered is something to watch: three refusals landing together
    /// is a verdict, three arriving in turn is the table thinking. Every answer
    /// is recorded, so a proposal can be shown being considered rather than
    /// only reported once it has failed.
    /// `from` is the lowest seat to ask. It is seat one while somebody is
    /// playing, since the human answers for themselves by being offered the
    /// choice, and seat nought in a game played out, where nobody is.
    fn settle_between_bots(&mut self, everyone: bool) {
        if self.state.trade_mode == TradeMode::Disabled {
            return;
        }
        for _ in 0..16 {
            // Every time round, because taking an offer removes it and the
            // engine fills the gap by moving another one into it: an index
            // read before that is a name for the wrong offer afterwards.
            self.sync_deals();
            if self.state.offer_count == 0 {
                return;
            }
            // Who has still to answer, worked out *before* the wait is armed.
            //
            // Arming it first spent the beat whether or not there was anything
            // to spend it on, and an offer nobody wants stays on the table for
            // the rest of the turn. So every tick went on a trade that was
            // never going to happen, `run_bots` was told to wait each time it
            // was reached, and the table stopped dead: the turn holder's clock
            // ran to zero with nobody able to move and the next turn never
            // began.
            let mut next = None;
            'outer: for d in self.deals.iter() {
                let Some(i) = d.at else { continue };
                for seat in 0..self.state.players {
                    // A person answers for themselves. Their card is on their
                    // screen and the table waits for it, which is the whole
                    // difference between a seat with somebody in it and a seat
                    // with the house bot in it.
                    if !everyone && self.is_person(seat) {
                        continue;
                    }
                    // Only the seats it was actually put to. A seat that was
                    // never asked has nothing to say and is not left waiting
                    // on the card for the rest of the turn.
                    if d.answers[seat as usize] != Answer::Waiting
                        || !self.state.may_accept(seat as usize, &d.offer)
                    {
                        continue;
                    }
                    next = Some((i, seat));
                    break 'outer;
                }
            }
            let Some((i, seat)) = next else {
                return;
            };
            // Reading an offer is not watching a move, so it is drawn from its
            // own window. Without any wait at all the table settled its own
            // trades in the tick the offer was made, and an offer a person
            // might have taken was gone before it had been drawn once.
            if !self.beat_due(self.pace.answer_window()) {
                return;
            }

            let take = Action::AcceptTrade { offer: i, by: seat };
            let from = self.state.offers[i as usize].from;
            // A seat that was asked always answers, whether it turned the trade
            // down or could not have covered it either way. Both are "no" out
            // loud and neither says which, so answering does not report on a
            // hand nobody is entitled to see.
            let could = {
                let mut probe = self.state;
                probe.apply(take).is_ok()
            };
            if !could || !self.bots[seat as usize].accepts(&self.state, seat as usize, i as usize) {
                self.answer(i, seat, Answer::No);
                self.note(Some(seat), format!("Passed on seat {from}'s offer"));
                // A refusal moves nothing on the board, but it is news, so the
                // page has to be told something changed or it would arrive
                // whenever the idle poll next happened to land.
                self.version += 1;
                continue;
            }
            let from = from as usize;
            let purse = self.state.hand;
            self.answer(i, seat, Answer::Yes);
            if self.state.apply(take).is_err() {
                return;
            }
            self.record(Step::Move(take));
            self.version += 1;
            // Both halves, because a trade is two public transfers and "took an
            // offer" said neither.
            let got = gains(&purse, &self.state.hand, seat as usize);
            let gave = gains(&purse, &self.state.hand, from);
            self.note(
                Some(seat),
                format!("Took {got} from seat {from} for {gave}"),
            );
            self.sync_deals();
        }
    }

    /// Re-read the market and keep the turn's record of it in step.
    ///
    /// The engine's table is not a record. It drops an offer the moment it is
    /// taken, clears the lot at the end of a turn, and reindexes what is left
    /// by swapping the last entry into the gap, so an index is not a name for
    /// an offer. The record is therefore kept beside it and matched back to it
    /// by value, which is stable across all three.
    fn sync_deals(&mut self) {
        let live = self.state.live_offers();
        if live.is_empty() && self.deals.is_empty() {
            return;
        }
        // Each live offer answers to at most one record, so two identical
        // offers from the same seat stay two records rather than collapsing.
        let mut claimed = [false; MAX_OFFERS];
        for d in self.deals.iter_mut() {
            if d.at.is_none() {
                continue;
            }
            d.at = live
                .iter()
                .enumerate()
                .find(|&(k, o)| !claimed[k] && *o == d.offer)
                .map(|(k, _)| {
                    claimed[k] = true;
                    k as u8
                });
        }
        for (k, o) in live.iter().enumerate() {
            if claimed[k] {
                continue;
            }
            self.deals.push(Deal {
                offer: *o,
                answers: [Answer::Waiting; MAX_PLAYERS],
                at: Some(k as u8),
            });
        }
    }

    /// Record an answer against the offer at a market index.
    fn answer(&mut self, at: u8, seat: u8, said: Answer) {
        if let Some(d) = self.deals.iter_mut().find(|d| d.at == Some(at)) {
            d.answers[seat as usize] = said;
        }
        // A yes is an `AcceptTrade` and is written down as the move it is. A no
        // moves nothing, so it has no move to be written down as, and without
        // this the record would replay a table that never answered.
        if said == Answer::No {
            self.record(Step::Passed {
                offer: at,
                by: seat,
            });
        }
    }

    /// The turn's offers and what was said to them, newest last.
    pub fn deals(&self) -> &[Deal] {
        &self.deals
    }
}

/// What a seat has said to an offer.
///
/// Only the seats an offer was actually put to ever leave `Waiting`: a seat
/// that cannot cover it, or is not a party to it, was never asked and is not
/// shown as having refused.
/// One thing that happened.
///
/// Not every one of them is a move. Turning an offer down changes no position
/// and the engine is right not to have an action for it, but it is the answer
/// the table was waiting on and the record would be telling a different story
/// without it. `carranta-record` draws the same distinction (`Recorder::decline`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Move(Action),
    Passed { offer: u8, by: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Answer {
    #[default]
    Waiting,
    No,
    Yes,
}

/// An offer made this turn, and the round of replies to it.
///
/// Outlives the engine's copy on purpose. A deal that has been taken or has
/// lapsed keeps its answers until the turn ends, because "Ines took it" is
/// the part of a trade a person watches for and the engine has thrown the
/// offer away by the time it is true.
#[derive(Clone, Debug)]
pub struct Deal {
    pub offer: Offer,
    pub answers: [Answer; MAX_PLAYERS],
    /// Where it sits in the engine's market, or `None` once it has left it.
    pub at: Option<u8>,
}

impl Deal {
    /// Whether it is still open to be answered.
    pub fn live(&self) -> bool {
        self.at.is_some()
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

/// How long a bot waits before each move.
///
/// A bot decides in microseconds, and a table where three opponents take their
/// whole turn between two of your frames is not a game anyone can follow. The
/// wait is drawn from a range rather than fixed: a metronome reads as a machine
/// working through a list, and the point of pacing is that it does not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pace {
    /// No wait. The bots finish before the page has drawn.
    Instant,
    /// Quick enough to keep up with, slow enough to see what happened.
    Fast,
    /// Time to read each move as it lands.
    Slow,
}

impl Pace {
    pub fn parse(name: Option<&str>) -> Self {
        match name {
            Some("slow") => Pace::Slow,
            Some("instant") => Pace::Instant,
            _ => Pace::Fast,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Pace::Instant => "instant",
            Pace::Fast => "fast",
            Pace::Slow => "slow",
        }
    }

    /// The window a single move's wait is drawn from, in milliseconds.
    ///
    /// Fast is a beat: long enough that a move is a thing that happened rather
    /// than a thing that had already happened, short enough that a round does
    /// not become a wait. Slow is reading speed, with time to look at the board
    /// between moves. The spread on each is wide enough to break the rhythm
    /// without either overlapping the other.
    fn window(self) -> (u64, u64) {
        match self {
            Pace::Instant => (0, 0),
            Pace::Fast => (420, 980),
            Pace::Slow => (1400, 3000),
        }
    }

    /// The window an answer to an offer is drawn from, in milliseconds.
    ///
    /// Longer than a move's, because it is not the same act. A move is watched;
    /// an offer is read, weighed against a hand, and possibly contested, and
    /// all three have to fit before the table has settled it. At a move's beat
    /// an offer was gone before it could be reached for, which made the market
    /// something that happened to the player rather than something they were in.
    ///
    /// One seat answers per beat rather than the table answering at once, so
    /// the answers arrive as a round of replies and not as a verdict.
    fn answer_window(self) -> (u64, u64) {
        match self {
            Pace::Instant => (0, 0),
            Pace::Fast => (900, 1800),
            Pace::Slow => (2200, 4000),
        }
    }
}

/// The order the five are listed in, as indices into [`RESOURCE_NAMES`].
///
/// Wood before brick, matching the hand and every list on the page. The engine
/// numbers them for its own reasons and that numbering is the wire format, so
/// the reading order is kept here rather than by renumbering the game.
const LISTING_ORDER: [usize; 5] = [1, 0, 2, 3, 4];

fn cards(counts: &[u8; 5]) -> String {
    let parts: Vec<String> = LISTING_ORDER
        .iter()
        .filter(|&&r| counts[r] > 0)
        .map(|&r| format!("{} {}", counts[r], RESOURCE_NAMES[r]))
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

/// Everything a player does is public and goes in the log.
///
/// Offers used to be left out, which was a mistake carried over from the
/// choice list: the engine enumerates roughly a hundred and eighty possible
/// offers and none of them belong in front of a person, but an offer that was
/// actually made is one line and is on the table for everyone to see. The same
/// goes for withdrawing one, and for a discard.
fn worth_logging(_a: &Action) -> bool {
    true
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
    fn a_table_whose_first_seat_is_a_bot_starts_itself() {
        // A move is what runs the bots, and before the first move there has been
        // no move. While seat nought was always the person that was invisible,
        // because the person was the one being asked. Once the turn order is
        // drawn a bot can hold seat nought, and then something has to play it:
        // the poll, which is the only clock this process has.
        //
        // Instant is the case that broke. A paced table looked mid-thought and
        // was ticked along; an instant one has nothing to wait for, so nothing
        // ever asked it to play and it sat at turn one for ever.
        // One poll is enough to get a paced table moving, and that is all this
        // asserts of one: the rest of its opening arrives a beat at a time on
        // later polls, which is the pacing working rather than a stall.
        for pace in [Pace::Instant, Pace::Fast] {
            let mut s = Session::new(4, 21, TradeMode::Full)
                .with_pace(pace)
                .with_people(&[2]);
            assert!(!s.is_person(s.state().decider()), "a bot opens the setup");
            assert!(s.choices_for(2).is_empty(), "and it is not seat two's turn");
            s.tick();
            assert!(s.started(), "the table played on its own: {pace:?}");
        }
        // An instant one runs the whole opening in that one poll and stops where
        // it should: at the person, with something to answer.
        let mut s = Session::new(4, 21, TradeMode::Full)
            .with_pace(Pace::Instant)
            .with_people(&[2]);
        s.tick();
        assert!(!s.choices_for(2).is_empty(), "and stopped at the person");
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
    fn the_bots_wait_for_every_person_at_the_table() {
        // The sentence that used to make a second person impossible was "stop
        // when it is seat nought's turn". Two people means stopping for either.
        let mut s = Session::new(4, 9, TradeMode::Full).with_people(&[0, 2]);
        assert!(s.is_person(0) && s.is_person(2));
        assert!(!s.is_person(1) && !s.is_person(3));
        assert_eq!(s.people(), vec![0, 2]);

        // Play whichever person is being asked, for as long as either is. The
        // invariant under it is the point: control comes back either on a
        // person's turn, or on a bot's with a question waiting for a person,
        // and never on a bot's turn with nothing asked of anybody, which would
        // be the table stopped for no reason.
        let mut asked = [0usize; 4];
        for _ in 0..400 {
            if s.winner().is_some() {
                break;
            }
            let seat = s.state().decider();
            let playing = if s.is_person(seat) {
                seat
            } else {
                match (0..4).find(|&p| s.is_person(p) && !s.choices_for(p).is_empty()) {
                    Some(p) => p,
                    None => panic!("the table stopped on a bot with nothing asked of anybody"),
                }
            };
            let v = s.version();
            if s.act_as(playing, 0, v).is_err() {
                break;
            }
            asked[playing as usize] += 1;
        }
        assert!(asked[0] > 0, "seat nought was asked");
        assert!(asked[2] > 0, "and so was seat two");
    }

    #[test]
    fn one_seat_cannot_play_the_move_of_another() {
        // Choices are per seat and an action is an index into the seat's own
        // list, so the worst a wrong index can do is name a choice that is not
        // there. A seat with nothing to answer has no list at all.
        let mut s = Session::new(4, 4, TradeMode::Full).with_people(&[0, 1]);
        let holder = s.state().decider();
        let other = if holder == 0 { 1 } else { 0 };
        assert!(
            s.choices_for(other).is_empty(),
            "nothing is being asked of the seat that is not deciding"
        );
        let v = s.version();
        assert!(matches!(s.act_as(other, 0, v), Err(Refused::NoSuchChoice)));
        assert_eq!(s.version(), v, "and the board did not move");
        // The seat that is being asked can play.
        assert!(s.act_as(holder, 0, v).is_ok());
        assert!(s.version() > v);
    }

    #[test]
    fn nobody_answers_the_market_for_a_person() {
        // What "a person is in this seat" means to the market: their card waits
        // on their screen, where a bot's is answered as the table settles.
        let mut s = Session::new(4, 6, TradeMode::Full)
            .with_people(&[0, 1])
            .with_pace(Pace::Instant);
        for _ in 0..200 {
            if s.winner().is_some() {
                break;
            }
            let seat = s.state().decider();
            if !s.is_person(seat) {
                break;
            }
            let v = s.version();
            if s.act_as(seat, 0, v).is_err() {
                break;
            }
        }
        for seat in s.people() {
            for d in s.deals.iter() {
                assert!(
                    d.answers[seat as usize] == Answer::Waiting || d.offer.from == seat,
                    "seat {seat} was answered for"
                );
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

    /// A small champion for the tests: every input wired straight to the
    /// output, the shape a minimal NEAT start has.
    fn tiny_champion() -> Net {
        champion_like(7)
    }

    /// A small distinct network per seed, so two champions in one test are two
    /// different players rather than one twice.
    fn champion_like(spread: u32) -> Net {
        let out = Net::output_id(carranta_bot::features::FEATURES);
        let links: Vec<(u32, u32, f64)> = (0..=carranta_bot::features::FEATURES as u32)
            .map(|i| (i, out, ((i % spread) as f64 - 3.0) / 10.0))
            .collect();
        Net::assemble(carranta_bot::features::FEATURES, &links).expect("acyclic")
    }

    #[test]
    fn the_clocks_stop_when_the_game_does() {
        // A finished game has no time passing in it. Before this the turn
        // clock counted down under the winner's own dialog and the game timer
        // climbed for as long as the tab stayed open.
        let mut s = Session::new(4, 5, TradeMode::Full)
            .with_clock(Clock::PerTurn(60))
            .with_pace(Pace::Instant);
        assert!(s.ended.is_none(), "nothing has ended yet");
        // While it runs, time passes: the control for the assertion below.
        let a = s.since(s.started);
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert!(s.since(s.started) > a, "a live game keeps time");

        s.play_out();
        assert!(s.winner().is_some(), "need a finished game");
        let stopped = s.ended.expect("the end was stamped");

        // Every reading is taken against the same frozen moment, so they are
        // equal however long the page is left open.
        let game = s.since(s.started);
        let turn = s.time_left(s.on_clock());
        let spent = s.used(s.on_clock());
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert_eq!(s.since(s.started), game, "the game timer stopped");
        assert_eq!(s.time_left(s.on_clock()), turn, "the turn clock stopped");
        assert_eq!(
            s.used(s.on_clock()),
            spent,
            "and nobody is still being charged"
        );
        assert_eq!(s.elapsed_secs(), game.as_secs());

        // Frozen rather than blanked: how long the game took is worth reading
        // afterwards. And the stamp is the first end, not the latest poll.
        s.note_winner();
        assert_eq!(s.ended, Some(stopped), "the end does not creep");
    }

    #[test]
    fn two_champions_share_a_table_and_are_told_apart() {
        // The whole point of seating champions per seat: they play the same
        // board from different chairs, and each chair names its own player, so
        // the ratings compare them directly rather than through a third party.
        let mut s = Session::new(4, 11, TradeMode::Full)
            .with_trained(&[(champion_like(5), 12), (champion_like(9), 40)])
            .with_pace(Pace::Instant);
        // Dealt round the seats, so neither champion is stuck with one chair's
        // luck of the draw.
        assert_eq!(s.agent_of(0), "trained@12");
        assert_eq!(s.agent_of(1), "trained@40");
        assert_eq!(s.agent_of(2), "trained@12");
        assert_eq!(s.agent_of(3), "trained@40");
        assert_eq!(s.champions(), vec![12, 40]);
        // One champion is enough to open the market up, and it stays open for
        // everybody, which is what keeps the comparison fair.
        assert_eq!(
            s.state().offer_shapes,
            OfferShapes::Mixed {
                give: Some(2),
                want: 2
            }
        );
        s.play_out();
        assert!(s.moves().len() > 50, "a real game happened");

        // A mixed table: one champion, and the house in the other chairs.
        let mut mixed = Session::new(4, 12, TradeMode::Full);
        mixed.seat_trained(2, &champion_like(5), 12);
        assert_eq!(mixed.agent_of(2), "trained@12");
        assert_eq!(mixed.agent_of(0), "house@1");
        assert_eq!(mixed.champions(), vec![12]);
        assert_eq!(
            mixed.state().ask_allowance,
            3,
            "a champion's table generates the asks it trained under (E-15)"
        );
        // And unseating the last champion closes the market back down.
        mixed.seat_house(2);
        assert_eq!(mixed.champions(), Vec::<u32>::new());
        assert_eq!(mixed.state().offer_shapes, OfferShapes::SingleType);
        assert_eq!(mixed.state().ask_allowance, OFFERS_PER_TURN);
    }

    #[test]
    fn a_trained_champion_plays_every_bot_seat_and_says_so() {
        let mut s = Session::new(4, 5, TradeMode::Full)
            .with_trained(&[(tiny_champion(), 7)])
            .with_pace(Pace::Instant);
        // The identity the game file will carry, and the market the champion
        // trained in: both come with the builder or the deployment lies about
        // who played and what they were allowed to offer.
        assert_eq!(s.agent_of(0), "trained@7");
        assert_eq!(s.agent_of(3), "trained@7");
        assert_eq!(
            s.state().offer_shapes,
            OfferShapes::Mixed {
                give: Some(2),
                want: 2
            }
        );
        assert_eq!(Session::new(4, 5, TradeMode::Full).agent_of(0), "house@1");
        // And it can actually hold a table: a whole game, every seat the
        // champion's, without the engine refusing a move.
        s.play_out();
        assert!(s.moves().len() > 50, "a real game happened");
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
    fn a_game_played_out_puts_its_offers_to_the_table() {
        // `play_out` chose a move, narrated it and looped, and never settled the
        // market: every offer in every demo game sat there until its maker
        // withdrew it. Nobody accepted, nobody even refused, and the analytics
        // page reported a table that only ever traded with the bank. The market
        // was decoration in exactly the games the page was built to read.
        //
        // Two seeds, because a market is a property of the whole game rather
        // than of one turn, and one seed that happens to trade would pass this
        // while the settle was gone again.
        let mut seen = 0;
        for seed in [7u64, 21] {
            let mut s = Session::new(4, seed, TradeMode::Full).with_pace(Pace::Instant);
            s.play_out();
            assert!(s.winner().is_some(), "seed {seed} did not finish");
            let offered = s
                .moves()
                .iter()
                .filter(|m| matches!(m, Step::Move(Action::ProposeTrade { .. })))
                .count();
            let took = s
                .moves()
                .iter()
                .filter(|m| matches!(m, Step::Move(Action::AcceptTrade { .. })))
                .count();
            let refused = s
                .moves()
                .iter()
                .filter(|m| matches!(m, Step::Passed { .. }))
                .count();
            assert!(offered > 0, "seed {seed} made no offers at all");
            assert!(
                took + refused > 0,
                "seed {seed}: {offered} offers and not one answer, so nobody was asked"
            );
            seen += took;
        }
        assert!(seen > 0, "no offer was taken in either game");
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
        assert!(
            s.time_left(0).is_some_and(|t| t < 60),
            "the mover is charged"
        );
        assert_eq!(
            s.time_left(1),
            Some(60),
            "everyone else still has the full turn"
        );
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
        assert!(
            before <= 58,
            "two seconds of thinking are gone, left {before}"
        );
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
        let s = Session::new(4, 1, TradeMode::Full).with_clock(Clock::Chess {
            bank: 600,
            increment: 0,
        });
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(
            s.time_left(0).is_some_and(|t| t < 600),
            "the mover's bank drains"
        );
        assert_eq!(
            s.time_left(1),
            Some(600),
            "a seat not moving spends nothing"
        );
    }

    #[test]
    fn a_bots_turn_is_on_the_clock_too() {
        // Production showed a table standing at 0:00 for minutes, turn 31,
        // while a trained bot went on making paced offers. The clock exempted
        // bots on the argument that they never dawdle, which was true of the
        // heuristic and false of a bot whose market appetite is twenty paced
        // offers a turn. The clock is a table rule: a seat is on it whoever
        // plays the seat.
        let mut s = Session::new(4, 5, TradeMode::Full)
            .with_clock(Clock::PerTurn(60))
            .with_pace(Pace::Instant);
        // Through setup and to the end of the human's first turn.
        while s.state.decider() == HUMAN {
            let choices = s.choices();
            let end = choices
                .iter()
                .position(|c| matches!(c, Choice::Play(Action::EndTurn)))
                .unwrap_or(0);
            let v = s.version();
            if s.act(end, v).is_err() {
                break;
            }
        }
        // Freeze the bots mid-turn the way a paced table holds them, then
        // spend the whole allowance. This is the abandoned-table poll.
        s.pace = Pace::parse(Some("slow"));
        s.bot_ready = std::time::Instant::now() + std::time::Duration::from_secs(3_600);
        let holder = s.on_clock();
        assert!(
            !s.is_person(holder),
            "need a bot on the clock: got {holder}"
        );
        let long_ago = std::time::Instant::now() - std::time::Duration::from_secs(600);
        s.turn_began = long_ago;
        s.last_settle = long_ago;
        assert!(s.out_of_time(holder), "the allowance has gone");

        s.enforce_clock();

        assert_ne!(s.on_clock(), holder, "the bot's turn was ended");
        assert!(
            s.log().iter().any(|l| l.text.contains("Time ran out")),
            "and the log says why"
        );
        assert!(
            !s.log().iter().any(|l| l.text.contains("for you")),
            "a bot's forced move is not addressed to a person"
        );
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
                .any(|l| l.text.contains("rolled ") && l.text.contains("for you")),
            "rolling came first, because a turn cannot be ended before the dice"
        );
        assert!(
            s.log()
                .iter()
                .any(|l| l.text.contains("the turn was ended")),
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
            .filter(|l| l.setup && l.text.starts_with("Collected "))
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
            .filter(|l| l.text.starts_with("Collected "))
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
            assert_eq!(
                code.len(),
                15,
                "{code} is thirteen characters and two hyphens"
            );
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
                    let passable = buf.contains(&Action::EndTurn) || buf.contains(&Action::Roll);
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
        assert!(
            seen.len() >= 5,
            "the sweep should reach most phases, saw {}",
            seen.len()
        );
    }

    #[test]
    fn everything_between_two_turn_ends_is_one_turn() {
        // A turn runs from one player ending theirs to the next ending theirs.
        // A seven hands the decision round the table for discards, and a
        // militia hands it round again, but all of that happens inside the turn
        // it interrupted and has to be filed under it.
        for seed in 0..60u64 {
            let mut g = Session::new(4, seed, TradeMode::Full);
            for _ in 0..700 {
                if g.choices().is_empty() {
                    break;
                }
                let v = g.version();
                let _ = g.act(0, v);
            }
            // Walk the record: between one "Ended the turn" and the next,
            // every line has to carry the same turn number.
            let mut current: Option<u32> = None;
            for line in g.log().iter().filter(|l| !l.setup) {
                match current {
                    None => current = Some(line.turn),
                    Some(n) => assert_eq!(
                        line.turn, n,
                        "seed {seed}: \"{}\" was filed under turn {} inside turn {n}",
                        line.text, line.turn
                    ),
                }
                if line.text == "Ended the turn" || line.text.contains("the turn was ended") {
                    current = None;
                }
            }
        }
    }

    #[test]
    fn a_militia_can_be_put_back_before_the_robber_moves() {
        // Find a position where the human can play a militia.
        let mut s = None;
        for seed in 0..60u64 {
            let mut g = Session::new(4, seed, TradeMode::Full);
            for _ in 0..400 {
                if g.choices().is_empty() {
                    break;
                }
                if let Some(i) = g
                    .choices()
                    .iter()
                    .position(|c| c.label(g.state()) == "Play Militia")
                {
                    let v = g.version();
                    let before = *g.state();
                    let lines = g.log().len();
                    g.act(i, v).unwrap();
                    s = Some((g, before, lines));
                    break;
                }
                let buy = g
                    .choices()
                    .iter()
                    .position(|c| c.label(g.state()) == "Buy a development card");
                let v = g.version();
                let _ = g.act(buy.unwrap_or(0), v);
            }
            if s.is_some() {
                break;
            }
        }
        let (mut g, before, lines) = s.expect("no militia ever became playable");

        // The card is spent and the robber is waiting to be placed.
        assert!(
            g.can_cancel(),
            "a half-played militia should be takeable back"
        );
        assert_ne!(*g.state(), before, "the play did nothing");
        assert!(g.log().len() > lines, "the play was not logged");

        let v = g.version();
        g.cancel(v).unwrap();
        // Everything back, the generator included: this cannot be used to fish
        // for a different steal by playing the card again.
        assert_eq!(*g.state(), before, "the position did not come back whole");
        assert_eq!(g.log().len(), lines, "the log line did not come back");
        assert!(!g.can_cancel(), "there is nothing left to take back");
        assert_eq!(g.cancel(g.version()), Err(Refused::NoSuchChoice));
    }

    #[test]
    fn nothing_else_can_be_taken_back() {
        // Only a militia arms the undo. Rolling, building and ending a turn
        // are done when they are done.
        let mut g = Session::new(4, 3, TradeMode::Full);
        for _ in 0..120 {
            if g.choices().is_empty() {
                break;
            }
            let v = g.version();
            let _ = g.act(0, v);
            assert!(
                !g.can_cancel(),
                "an ordinary move offered itself to be undone"
            );
        }
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

    /// Deal the board out and then put one offer of the human's on the table.
    fn table_with_an_offer_from_the_human(seed: u64) -> Option<Session> {
        let mut s = Session::new(4, seed, TradeMode::Full);
        while s.in_setup() {
            let v = s.version();
            s.act(0, v).ok()?;
        }
        for _ in 0..300 {
            if s.can_propose() {
                let hand = s.state.hand[HUMAN as usize];
                if let Some(give) = (0..5).find(|&r| hand[r] > 0) {
                    let want = (0..5).find(|&w| w != give).expect("five resources");
                    let mut out = [0u8; 5];
                    let mut back = [0u8; 5];
                    out[give] = 1;
                    back[want] = 1;
                    let v = s.version();
                    if s.propose(None, out, back, v).is_ok() {
                        return Some(s);
                    }
                }
            }
            let v = s.version();
            s.act(0, v).ok()?;
        }
        None
    }

    #[test]
    fn a_deal_keeps_who_said_what_after_it_has_left_the_table() {
        // A trade is watched as much as it is played, and the engine throws the
        // offer away at exactly the moment there is most to say about it. So
        // the session keeps its own record, and every seat the offer was put to
        // answers it rather than some of them being left unheard from.
        let s = (0..40u64)
            .find_map(table_with_an_offer_from_the_human)
            .expect("some seed lets the human put an offer up");

        let deal = s
            .deals()
            .iter()
            .find(|d| d.offer.from == HUMAN)
            .expect("the offer is on the record");
        for seat in 0..s.state.players {
            if !s.state.may_accept(seat as usize, &deal.offer) {
                continue;
            }
            assert_ne!(
                deal.answers[seat as usize],
                Answer::Waiting,
                "seat {seat} was asked and never answered"
            );
        }
        // An unpaced table settles inside the propose, so by now the offer has
        // either been taken or been turned down by everyone. Either way what
        // was said survives the offer.
        if !deal.live() {
            assert!(
                deal.answers.iter().any(|&a| a != Answer::Waiting),
                "a deal that has left the table still says what happened to it"
            );
        }
    }

    #[test]
    fn the_turn_takes_the_answers_with_it() {
        // Offers do not survive the turn they were made in (the engine's rule),
        // and neither may the round of replies: a card still showing last
        // turn's answers is a question about a table that has been cleared.
        let mut s = (0..40u64)
            .find_map(table_with_an_offer_from_the_human)
            .expect("some seed lets the human put an offer up");
        let mine = s
            .deals()
            .iter()
            .find(|d| d.offer.from == HUMAN)
            .expect("the offer is on the record")
            .offer;

        let ended = s.turn_no();
        for _ in 0..60 {
            if s.turn_no() > ended {
                break;
            }
            let v = s.version();
            if s.act(0, v).is_err() {
                break;
            }
        }
        assert!(s.turn_no() > ended, "the turn moved on");
        assert!(
            !s.deals().iter().any(|d| d.offer == mine),
            "the deal went with the turn that made it"
        );
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
        assert_eq!(
            s.turn_no(),
            9,
            "eight placement turns, then the first of play"
        );
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
        // an answer. Nothing about that is charged to the human: it is the
        // turn holder's own allowance running down while they wait, and when
        // it runs out the offer dies with the turn rather than the game
        // standing still for as long as nobody clicks.
        let mut found = None;
        'seeds: for seed in 0..200u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..300 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                // An offer is on the table and it is not the human's turn.
                if s.state.decider() != HUMAN && !s.open_offers_for(HUMAN).is_empty() {
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

        // The turn is not the human's, so neither is the clock, however much
        // the table is waiting on them.
        let holder = s.on_clock();
        assert_ne!(holder, HUMAN, "the clock stays with the turn");

        s = s.with_clock(Clock::PerTurn(1));
        let before = s.version();
        let offers = s.open_offers_for(HUMAN).len();
        assert!(offers > 0);
        // The human is not on the clock, so their own allowance is untouched
        // and reads full while somebody else's turn runs down.
        assert_eq!(
            s.time_left(HUMAN),
            Some(1),
            "a passive seat is never charged for thinking"
        );
        // Reported as "the trade card never popped up at all". An expired
        // allowance of the human's used to carry onto the next player's turn,
        // and enforcement ran on the same request that opened the offer, so
        // the clock refused it before the page had drawn it once. While the
        // turn that made the offer has time left, the offer stands.
        s.enforce_clock();
        assert_eq!(
            s.open_offers_for(HUMAN).len(),
            offers,
            "an offer is answerable while the turn that made it has time"
        );
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(s.out_of_time(holder), "the turn holder's time is what runs");
        s.enforce_clock();

        assert!(
            s.open_offers_for(HUMAN).is_empty(),
            "silence is a refusal, so the table is cleared"
        );
        assert!(s.version() >= before, "and play carries on");
        assert!(
            s.log().iter().any(|l| l.text.contains("declined")),
            "and the log says the clock did it"
        );
    }

    /// Play until the human owes cards to a seven, on the clocks given.
    fn table_owing_a_discard(clock: Clock, discard: u64) -> Option<Session> {
        for seed in 0..60u64 {
            let mut s = Session::new(4, seed, TradeMode::Full)
                .with_clock(clock)
                .with_discard_secs(discard);
            for _ in 0..400 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                if matches!(s.state.phase, Phase::Discard) && s.state.decider() == HUMAN {
                    return Some(s);
                }
                let v = s.version();
                if s.act(0, v).is_err() {
                    break;
                }
            }
        }
        None
    }

    #[test]
    fn a_discard_holds_the_turn_clock_rather_than_spending_it() {
        // A seven is an interruption, not a turn. It stops the player who rolled
        // from playing and asks everyone else for cards on a turn that is not
        // theirs, so the turn's allowance is held while it is answered. Before
        // this the roller paid for the dice: a table that took ten seconds over
        // its discards took them out of the roller's minute.
        let mut s = table_owing_a_discard(Clock::PerTurn(60), 30)
            .expect("some seed rolls a seven onto the human");
        let holder = s.on_clock();
        let before = s.time_left(holder).expect("a clock");
        assert!(s.discarding(), "a discard is owed");

        std::thread::sleep(std::time::Duration::from_millis(1_400));
        assert_eq!(
            s.time_left(holder),
            Some(before),
            "the turn clock is held while the table discards"
        );
        // And the seven's own allowance is the one running.
        let left = s.discard_left().expect("a discard clock");
        assert!(left < 30, "the discard's own clock is counting: {left}");

        // Finishing it starts the turn clock again from where it stopped,
        // rather than from the beginning.
        while matches!(s.state.phase, Phase::Discard) && s.state.decider() == HUMAN {
            let v = s.version();
            if s.act(0, v).is_err() {
                break;
            }
        }
        assert!(!s.discarding(), "the discard is done");
        assert!(
            s.time_left(s.on_clock()).is_some_and(|t| t <= before),
            "the turn clock resumes rather than refilling"
        );
    }

    #[test]
    fn a_discard_nobody_makes_in_time_is_made_at_random() {
        // The one place a clock takes cards out of a hand. A discard cannot be
        // declined and the position is illegal until it is done, so the choice
        // is between choosing badly for someone and the game stopping on them.
        let mut s = table_owing_a_discard(Clock::PerTurn(60), 1).expect("some seed rolls a seven");
        let held: u8 = s.state.hand[HUMAN as usize].iter().sum();
        assert!(s.discarding());

        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(s.discard_left().is_some_and(|t| t <= 0), "time is up");
        s.enforce_clock();

        assert!(!s.discarding(), "the seven is settled");
        assert!(
            s.state.hand[HUMAN as usize].iter().sum::<u8>() < held,
            "and the cards went back"
        );
        assert!(
            s.log()
                .iter()
                .any(|l| l.text.contains("Time ran out") && l.text.contains("discarded")),
            "and the log says the clock did it"
        );
    }

    #[test]
    fn an_untimed_discard_waits_however_long_it_takes() {
        // Zero seconds is no limit, which a table may reasonably want. Nothing
        // is forced and nothing is counted.
        let mut s = table_owing_a_discard(Clock::PerTurn(60), 0)
            .expect("some seed rolls a seven onto the human");
        assert_eq!(s.discard_left(), None, "no allowance to report");
        let hand = s.state.hand[HUMAN as usize];
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        s.enforce_clock();
        assert_eq!(
            s.state.hand[HUMAN as usize], hand,
            "nothing is taken from a discard that was never on a clock"
        );
    }

    #[test]
    fn an_offer_you_cannot_cover_can_still_be_turned_down() {
        // Reported as "often I don't see the trading modals". Two different
        // things looked the same from the outside. An offer made on somebody
        // else's turn by a passive player is not yours to answer at all
        // (R-7.3), and showing nothing for it is right. An offer that *was* put
        // to you but that you cannot cover is a question, and it was silently
        // invisible: the card was keyed on what you could accept, so with
        // nothing acceptable there was no card, no explanation and no way to
        // say no. The table then waited on an answer you had no way to give.
        let mut found = None;
        'seeds: for seed in 0..120u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..300 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                let asked = s.offers_to(HUMAN);
                if !asked.is_empty() && s.open_offers_for(HUMAN).is_empty() {
                    found = Some(s);
                    break 'seeds;
                }
                let v = s.version();
                if s.act(0, v).is_err() {
                    break;
                }
            }
        }
        let mut s = found.expect("some seed puts an unaffordable offer to the human");

        assert!(
            s.choices().contains(&Choice::Decline),
            "an offer you were asked about can always be turned down"
        );
        // By value rather than by index: declining lets the bots play on, and
        // what they do next can put new offers on the table. Those are new
        // questions and being asked them again is right.
        let asked: Vec<Offer> = s
            .offers_to(HUMAN)
            .into_iter()
            .map(|i| s.state.offers[i as usize])
            .collect();
        assert!(!asked.is_empty());
        let i = s
            .choices()
            .iter()
            .position(|c| *c == Choice::Decline)
            .expect("the decline is on the list");
        let v = s.version();
        s.act(i, v).expect("declining is a legal answer");

        let still: Vec<Offer> = s
            .offers_to(HUMAN)
            .into_iter()
            .map(|i| s.state.offers[i as usize])
            .collect();
        for o in &asked {
            assert!(
                !still.contains(o),
                "declining answers everything that was put to you"
            );
        }
    }

    #[test]
    fn what_is_written_down_replays_into_the_game_that_was_played() {
        // The whole basis for saving a game in a few hundred bytes: the engine
        // is deterministic, so the seats, the seed and the moves are the game.
        // If this ever stops holding, every stored game becomes a different
        // game the next time it is read, silently.
        for seed in 0..40u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..400 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                let v = s.version();
                if s.choices().is_empty() || s.act(0, v).is_err() {
                    break;
                }
            }
            let (seats, dealt, mode) = s.table();
            let again = Session::replay(seats, dealt, mode, s.moves())
                .expect("every move the session applied is legal on replay");
            // The whole position, down to the generator: two games that agree
            // on everything but the next random number are not the same game.
            assert_eq!(again, s.state, "seed {seed} replays into a different game");
        }
    }

    #[test]
    fn a_resumed_game_reads_the_way_it_was_played() {
        // Reopening a game from disk has to give back the account of it too,
        // not only the position: a board with no history is not the game.
        for seed in 0..12u64 {
            let mut s = Session::new(4, seed, TradeMode::Full).with_name("Egon");
            for _ in 0..400 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                let v = s.version();
                if s.choices().is_empty() || s.act(0, v).is_err() {
                    break;
                }
            }
            let (seats, dealt, mode) = s.table();
            let back = Session::resume(seats, dealt, mode, s.moves()).expect("it replays");
            assert_eq!(back.state, s.state, "seed {seed}: the same position");
            assert_eq!(back.moves(), s.moves(), "and the same record of it");
            // The log is the account. Compared by what it says rather than by
            // the whole line, since a line carries the turn it was filed under
            // and those agree by construction.
            let said = |x: &Session| x.log().iter().map(|l| l.text.clone()).collect::<Vec<_>>();
            assert_eq!(said(&back), said(&s), "seed {seed}: the same account");
        }
    }

    #[test]
    fn taking_a_militia_back_takes_its_move_back_too() {
        // The position is restored whole, and the record has to go with it or
        // the file would replay a card that was never played.
        // Buy cards until one of them is a militia, then play it. Playing the
        // first legal move every turn almost never buys a card.
        let mut found = None;
        'seeds: for seed in 0..200u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..400 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                let choices = s.choices();
                if choices.is_empty() {
                    break;
                }
                if let Some(i) = choices
                    .iter()
                    .position(|c| *c == Choice::Play(Action::PlayMilitia))
                {
                    let v = s.version();
                    s.act(i, v)
                        .expect("a militia offered is a militia playable");
                    found = Some(s);
                    break 'seeds;
                }
                let pick = choices
                    .iter()
                    .position(|c| *c == Choice::Play(Action::BuyDev))
                    .or_else(|| {
                        choices
                            .iter()
                            .position(|c| *c == Choice::Play(Action::Roll))
                    })
                    .or_else(|| {
                        choices
                            .iter()
                            .position(|c| *c == Choice::Play(Action::EndTurn))
                    })
                    .unwrap_or(0);
                let v = s.version();
                if s.act(pick, v).is_err() {
                    break;
                }
            }
        }
        let mut s = found.expect("some seed reaches a playable militia");
        assert_eq!(s.moves().last(), Some(&Step::Move(Action::PlayMilitia)));
        let before = s.moves().len();
        let v = s.version();
        s.cancel(v)
            .expect("a half-played militia can be taken back");
        assert_eq!(s.moves().len(), before - 1, "the move goes back with it");
        // The count and the replay, not the absence of every militia ever
        // played: a bot may well have played one earlier and that one stands.
        let (seats, seed, mode) = s.table();
        assert_eq!(
            Session::replay(seats, seed, mode, s.moves()).as_ref(),
            Some(&s.state),
            "and what is left still replays into the position on the table"
        );
    }

    #[test]
    fn a_forced_roll_pays_the_table_and_says_so() {
        // Reported from the log: "Time ran out, rolled 8 for you" with nothing
        // under it, then the turn ended. A roll that pays nobody is the one
        // thing a roll cannot do. The cards were always dealt, because the
        // engine does not know who asked for the move, but the forfeit path
        // wrote the move down without the production it caused, so from the
        // record the table had been skipped.
        let mut found = None;
        'seeds: for seed in 0..80u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..300 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                // The human's own turn, with the dice still to throw, and a
                // roll that will actually pay somebody. A number nobody is
                // settled on pays nothing and would prove nothing, so the
                // search looks ahead on a copy and keeps playing until it
                // finds one that does.
                if s.state.decider() == HUMAN && matches!(s.state.phase, Phase::PreRoll) {
                    let mut probe = s.state;
                    if probe.apply(Action::Roll).is_ok() && probe.hand != s.state.hand {
                        found = Some(s);
                        break 'seeds;
                    }
                }
                let v = s.version();
                if s.act(0, v).is_err() {
                    break;
                }
            }
        }
        let mut s = found.expect("some seed reaches the human's roll");

        // Exactly what this roll will pay, worked out on a copy. `State` carries
        // its own generator, so the copy throws the same dice as the forfeit is
        // about to: the payouts are the real ones and not a second sample.
        // Taken this way because the forfeit runs the bots on afterwards, and a
        // hand compared across that has the next two turns in it as well.
        let hands = s.state.hand;
        let paid = {
            let mut probe = s.state;
            probe
                .apply(Action::Roll)
                .expect("the roll is the only move");
            probe.hand
        };
        let lines = s.log().len();
        s = s.with_clock(Clock::PerTurn(1));
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        s.enforce_clock();

        assert!(
            s.log()[lines..]
                .iter()
                .any(|l| l.text.contains("Time ran out") && l.text.contains("rolled")),
            "the clock rolls for a player who ran out of time"
        );
        // Seat by seat, what the roll paid against what the log says it paid.
        assert_ne!(hands, paid, "the position was chosen for a roll that pays");
        for seat in 0..s.state.players as usize {
            let got = gains(&hands, &paid, seat);
            if got.is_empty() {
                continue;
            }
            assert!(
                s.log()[lines..]
                    .iter()
                    .any(|l| l.seat == Some(seat as u8) && l.text == format!("Collected {got}")),
                "seat {seat} was paid {got} and the log has to say so"
            );
        }
    }

    #[test]
    fn an_offer_you_cannot_cover_dies_with_the_turn_that_made_it() {
        // Reported as "the new turn doesn't start until the offer is accepted
        // or declined". A bot offered the human something they could not
        // afford, the human never clicked, and the table stopped: being asked
        // is what blocks the bots, and the clock's escape was keyed on what the
        // human could have *accepted*. With nothing acceptable there was
        // nothing to clear, so the turn holder's allowance ran to zero and the
        // game sat there.
        let mut found = None;
        'seeds: for seed in 0..120u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..300 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                // Asked, and could not cover it, on somebody else's turn.
                if s.state.decider() != HUMAN
                    && !s.offers_to(HUMAN).is_empty()
                    && s.open_offers_for(HUMAN).is_empty()
                {
                    found = Some(s);
                    break 'seeds;
                }
                let v = s.version();
                if s.act(0, v).is_err() {
                    break;
                }
            }
        }
        let mut s = found.expect("some seed puts an unaffordable offer on a bot's turn");

        let holder = s.on_clock();
        assert_ne!(holder, HUMAN, "the clock stays with the turn");
        // The question is what stops the table, whether or not it can be
        // answered yes.
        assert!(
            !s.choices().is_empty(),
            "an unanswered question holds the bots up"
        );
        // By value rather than by index: clearing these lets the bots play on,
        // and what they do next can put new offers on the table. Those are new
        // questions and being asked them is right.
        let asked: Vec<Offer> = s
            .offers_to(HUMAN)
            .into_iter()
            .map(|i| s.state.offers[i as usize])
            .collect();

        s = s.with_clock(Clock::PerTurn(1));
        let before = s.version();
        std::thread::sleep(std::time::Duration::from_millis(1_100));
        assert!(s.out_of_time(holder), "the turn holder's time is what runs");
        s.enforce_clock();

        let still: Vec<Offer> = s
            .offers_to(HUMAN)
            .into_iter()
            .map(|i| s.state.offers[i as usize])
            .collect();
        for o in &asked {
            assert!(
                !still.contains(o),
                "silence is a refusal even when yes was never available"
            );
        }
        assert!(s.version() > before, "and the game moves on");
        assert!(
            s.log().iter().any(|l| l.text.contains("declined")),
            "and the log says the clock did it"
        );
    }

    #[test]
    fn an_offer_nobody_wants_does_not_stop_a_paced_table() {
        // Reported as "why did the clock stop there and not start the new
        // turn?". A bot offered something no other bot would take and the
        // human could not afford, so the offer simply sat there. Settling the
        // market armed the pace wait on every tick regardless, which left no
        // beat for the seat whose turn it actually was, and the table stopped
        // with the clock at zero on a turn nobody could move in.
        let mut found = None;
        'seeds: for seed in 0..40u64 {
            // Dealt at speed, because the deal is not what is being tested,
            // then paced from the first turn of play so a bot's turn is left
            // part-played the way the browser finds it.
            let mut s = Session::new(4, seed, TradeMode::Full);
            while s.in_setup() {
                let v = s.version();
                if s.act(0, v).is_err() {
                    continue 'seeds;
                }
            }
            s = s.with_pace(Pace::Fast);
            for _ in 0..400 {
                if matches!(s.state.phase, Phase::GameOver { .. }) {
                    break;
                }
                // An offer stands that the human is not being asked about,
                // which is the position the report came from.
                if s.state.offer_count > 0 && s.state.decider() != HUMAN && s.choices().is_empty() {
                    found = Some(s);
                    break 'seeds;
                }
                if s.state.decider() == HUMAN || !s.choices().is_empty() {
                    let v = s.version();
                    if s.act(0, v).is_err() {
                        break;
                    }
                } else {
                    // The bots owe a move and are waiting out their beat, so
                    // give them one the way the page's poll does.
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    s.tick();
                }
            }
        }
        let mut s = found.expect("some seed leaves an offer nobody is answering");

        let before = s.version();
        // Poll the way the page does, over more than one pace window.
        for _ in 0..40 {
            s.tick();
            if s.version() > before {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(60));
        }
        assert!(
            s.version() > before,
            "the table moves on rather than waiting out a trade nobody wants"
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
            Session::new(4, 1, TradeMode::Full)
                .with_name("  Robin  ")
                .name(),
            "Robin"
        );
        assert_eq!(
            Session::new(4, 1, TradeMode::Full).with_name("   ").name(),
            "you"
        );
    }
}
