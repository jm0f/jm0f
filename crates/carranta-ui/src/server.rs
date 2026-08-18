//! A local HTTP server, on `std` alone.
//!
//! Deliberately small and deliberately local. It binds the loopback address
//! only and it is still not the server of §6.2: one connection at a time, no
//! authentication, no framework. What it is for is putting the engine in front
//! of a person.
//!
//! It does now hold more than one game. It held exactly one for as long as the
//! only way to reach a board was to open the root, and the home page is what
//! changed that: a page listing tables is a page whose links have to work, so a
//! table stays playable when the next one is dealt. Sixteen of them, newest
//! first, and a table that falls off the end still has its file. Persistence is
//! the store's job and always was; memory is only what is playable right now.
//!
//! Who is asking is a key in a cookie and nothing more. It is enough to answer
//! "show me my games" on one machine and it is not an account, which is the
//! next thing this needs rather than something it pretends to have.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Mutex;

use carranta_core::state::TradeMode;

use carranta_core::action::Illegal;

use crate::game::{Clock, DEFAULT_DISCARD_SECS, Pace, Refused, Session};
use crate::json;
use crate::store::{Chair as SavedChair, Saved, Setup, Store, game_id, is_game_id};
use crate::view;

const PAGE: &str = include_str!("../assets/index.html");

/// The board art, compiled in and served from memory.
///
/// Named here rather than read from disk so the binary stays a single file
/// that runs from anywhere, and so a missing drawing is a build error rather
/// than a board with holes in it. The page fetches these once and reuses them;
/// they are the drawings in `art/`, unmodified, which keeps one copy of each
/// rather than a second pasted into the page to drift from the first.
/// The terrain photographs, served as bytes rather than text. Every terrain
/// has one, so a hex is a picture of the thing it produces.
const PHOTOS: [(&str, &[u8]); 6] = [
    ("hills", include_bytes!("../../../art/hills.jpg")),
    ("forest", include_bytes!("../../../art/forest.jpg")),
    ("pasture", include_bytes!("../../../art/pasture.jpg")),
    ("fields", include_bytes!("../../../art/fields.jpg")),
    ("mountains", include_bytes!("../../../art/mountains.jpg")),
    ("desert", include_bytes!("../../../art/desert.jpg")),
];

const ART: [(&str, &str); 13] = [
    ("road-30", include_str!("../../../art/road-30.svg")),
    ("road-90", include_str!("../../../art/road-90.svg")),
    ("road-150", include_str!("../../../art/road-150.svg")),
    ("settlement", include_str!("../../../art/settlement.svg")),
    ("city", include_str!("../../../art/city.svg")),
    ("robber", include_str!("../../../art/robber.svg")),
    // The five development card faces. One per card in `DEV_CARDS`, named for
    // it, so the page asks for the face of the card it is holding rather than
    // keeping a table that maps one to the other.
    ("dev-militia", include_str!("../../../art/dev-militia.svg")),
    (
        "dev-victory-point",
        include_str!("../../../art/dev-victory-point.svg"),
    ),
    (
        "dev-monopoly",
        include_str!("../../../art/dev-monopoly.svg"),
    ),
    (
        "dev-road-building",
        include_str!("../../../art/dev-road-building.svg"),
    ),
    (
        "dev-invention",
        include_str!("../../../art/dev-invention.svg"),
    ),
    // The two bonus tiles (R-10). Same faces as the development cards, because
    // they are the same object: a card in your hand with a name, a picture of
    // what it does and what it is worth.
    (
        "award-longest-road",
        include_str!("../../../art/award-longest-road.svg"),
    ),
    (
        "award-largest-militia",
        include_str!("../../../art/award-largest-militia.svg"),
    ),
];

/// The two typefaces, carried in the binary like everything else.
///
/// Both are under the SIL Open Font Licence, which is why they can live in the
/// repository at all: a commercial webfont licence would let us *use* the font
/// but not redistribute the file, and this page has no external requests to
/// load one from. The licences ship beside them in `assets/fonts/`, which the
/// OFL requires.
///
/// Figtree is Google's latin subset as served. Fraunces is cut down to the
/// printable latin range. It is a display face used for a wordmark and a
/// dozen headings, and the full build is six times the size for glyphs no
/// heading will ever contain. All four of its axes survive the cut, including
/// the optical size and the wonk that give it its character.
const FONTS: [(&str, &[u8]); 3] = [
    ("figtree", include_bytes!("../assets/fonts/figtree.woff2")),
    ("fraunces", include_bytes!("../assets/fonts/fraunces.woff2")),
    (
        "audiowide",
        include_bytes!("../assets/fonts/audiowide.woff2"),
    ),
];

/// Three sounds, carried in the binary like everything else.
///
/// All by Kenney and all CC0, which is why they can be in the repository: the
/// files are redistributed, and the page makes no external request to fetch
/// one. `audio/SOURCES.md` says which pack each came from and by what route.
///
/// MP3 rather than the OGG originals, because every browser plays MP3 and the
/// difference is a couple of kilobytes on a file already under six.
const SOUNDS: [(&str, &[u8]); 8] = [
    ("error-008", include_bytes!("../../../audio/error-008.mp3")),
    (
        "jingles-hit-10",
        include_bytes!("../../../audio/jingles-hit-10.mp3"),
    ),
    (
        "jingles-hit-15",
        include_bytes!("../../../audio/jingles-hit-15.mp3"),
    ),
    (
        "confirmation-001",
        include_bytes!("../../../audio/confirmation-001.mp3"),
    ),
    (
        "dice-throw-3",
        include_bytes!("../../../audio/dice-throw-3.mp3"),
    ),
    (
        "card-place-1",
        include_bytes!("../../../audio/card-place-1.mp3"),
    ),
    (
        "impact-generic-light-002",
        include_bytes!("../../../audio/impact-generic-light-002.mp3"),
    ),
    ("drop-002", include_bytes!("../../../audio/drop-002.mp3")),
];

/// Largest request body accepted. A click is a few dozen bytes; anything
/// larger is a mistake or a probe, and is refused rather than buffered.
const MAX_BODY: usize = 4 * 1024;

/// What a table looks like written down.
fn saved_of(t: &Table) -> Saved {
    let (seats, seed, mode) = t.session.table();
    Saved {
        id: t.id.clone(),
        seats,
        seed,
        mode,
        name: t.session.name().to_string(),
        by: t.by.clone(),
        dealt: t.dealt,
        winner: t.session.winner(),
        setup: Setup {
            game: t.session.game().to_string(),
            public: t.session.is_public(),
            pace: t.session.pace(),
            clock: t.session.clock(),
            discard_secs: t.session.discard_secs(),
            bank_exact: t.session.bank_exact(),
            log: t.session.log_shown(),
            chairs: t
                .chairs
                .iter()
                .map(|c| match c {
                    Chair::Bot => SavedChair::bot(),
                    Chair::Open => SavedChair::open(),
                    Chair::Taken { key, name } => SavedChair::person(key, name),
                })
                .collect(),
        },
        moves: t.session.moves().to_vec(),
        times: t.session.times().to_vec(),
    }
}

/// Who is in one seat.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Chair {
    /// The house bot plays it.
    Bot,
    /// Waiting for somebody, and holding the game up until they arrive or the
    /// host gives up on them.
    ///
    /// A table with an open chair has not started. That is the whole reason the
    /// state exists: joining a game already in progress means being handed
    /// whatever a bot built for you, which is not joining a game, and letting
    /// people in only before the first move is the difference between a table
    /// filling up and a table being walked into.
    Open,
    /// A person, by the key their browser was handed and the name they gave.
    Taken { key: String, name: String },
}

impl Chair {
    fn key(&self) -> Option<&str> {
        match self {
            Chair::Taken { key, .. } => Some(key),
            _ => None,
        }
    }
}

/// One game in memory, and the name it answers to.
struct Table {
    id: String,
    session: Session,
    /// What the file says about it, less the moves, which come off the session.
    dealt: u64,
    /// The key of whoever dealt it, so a home page can say which are yours.
    by: String,
    /// Who is in each seat, in seat order. Seat nought is whoever dealt it.
    chairs: Vec<Chair>,
    /// When each seat was last heard from, in seat order, Unix milliseconds.
    ///
    /// Beside the chairs rather than inside them, because it is not part of who
    /// is in the seat: the chairs are what gets written down and this never is.
    /// A restarted server has heard from nobody, which is the truth, and the
    /// people come back the moment their pages ask again.
    seen: Vec<u64>,
    /// When anything last happened here: dealt, asked about, sat down at,
    /// moved on. Unix milliseconds.
    ///
    /// The signal a table waiting for people is judged on. A page open on the
    /// waiting screen polls every three seconds, so somebody who is still there
    /// keeps their table alive without doing anything, and somebody who closed
    /// the tab stops. That is a truer test of "is anybody coming" than a timer
    /// from when it was dealt, and it costs nothing to collect.
    stirred: u64,
}

impl Table {
    /// Which seat this visitor is playing, if any.
    ///
    /// By key, so coming back to a game is the same operation as never having
    /// left it: a person who was in a seat when the game started is in it still,
    /// whatever the server has done in between.
    fn seat_of(&self, player: &str) -> Option<u8> {
        if player.is_empty() {
            return None;
        }
        self.chairs
            .iter()
            .position(|c| c.key() == Some(player))
            .map(|i| i as u8)
    }

    /// The first seat anybody could sit down in.
    fn free_seat(&self) -> Option<u8> {
        self.chairs
            .iter()
            .position(|c| *c == Chair::Open)
            .map(|i| i as u8)
    }

    /// Seats still waiting for somebody.
    fn waiting(&self) -> usize {
        self.chairs.iter().filter(|c| **c == Chair::Open).count()
    }

    /// Tell the session which seats have people in them.
    ///
    /// The session is the one that has to know, because it is what stops the
    /// bots and hands out the choices. Called after anybody sits down, and the
    /// only place the two representations are brought into line.
    /// Tell the session what each seat is called.
    ///
    /// The other half of `seat_the_people`, and needed for the same reason: the
    /// chairs are the server's record of who is at the table, and the session is
    /// what the view is rendered from. A table taken up again had its people
    /// back in their seats and everybody called nothing.
    fn name_the_seats(&mut self) {
        for (i, c) in self.chairs.iter().enumerate() {
            if let Chair::Taken { name, .. } = c {
                let (seat, name) = (i as u8, name.clone());
                self.session.name_seat(seat, &name);
            }
        }
    }

    fn seat_the_people(&mut self) {
        let playing = self.playing();
        self.session.seat_people(&playing);
    }

    /// Whether this seat's person has been heard from lately.
    ///
    /// The page asks for the state every three seconds, so absence means the tab
    /// is closed rather than that somebody is thinking. Not stored anywhere: a
    /// seat is present because somebody just asked about it, which is as direct
    /// a measure of "are they there" as this server can take.
    fn present(&self, seat: u8) -> bool {
        matches!(self.chairs.get(seat as usize), Some(Chair::Taken { .. }))
            && self
                .seen
                .get(seat as usize)
                .is_some_and(|&t| now().saturating_sub(t) <= AWAY_LIMIT)
    }

    /// Which seats the table is actually waiting for.
    ///
    /// A seat whose person has gone is played by the house bot, so one person
    /// leaving does not stop the game for everybody still at it. With one
    /// exception, and it is the whole reason this is a method rather than a
    /// filter: if *nobody* is there, the table waits for all of them. A game
    /// that played itself to the end because everyone stepped out for ten
    /// minutes would be a game destroyed rather than a game continued.
    fn playing(&self) -> Vec<u8> {
        let seated: Vec<u8> = self
            .chairs
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c, Chair::Taken { .. }))
            .map(|(i, _)| i as u8)
            .collect();
        let here: Vec<u8> = seated
            .iter()
            .copied()
            .filter(|&s| self.present(s))
            .collect();
        if here.is_empty() { seated } else { here }
    }

    /// Whether this visitor may give the empty chairs to the bots.
    ///
    /// Whoever dealt the table, or whoever is sitting in seat nought, which is
    /// the same person until the host stands up before the game starts. Without
    /// the second half their table would be left with nobody able to start it,
    /// waiting on somebody who had already gone.
    ///
    /// Not any seat: the others are waiting for the same person the host is, and
    /// one of them deciding for everybody would be a different rule.
    fn may_start(&self, player: &str) -> bool {
        if player.is_empty() {
            return false;
        }
        self.by == player || self.chairs.first().and_then(Chair::key) == Some(player)
    }

    /// Note that this seat's person is still there.
    ///
    /// Returns whether it changed anything, so the caller can re-seat the table
    /// only when somebody has actually arrived or gone rather than on every
    /// poll.
    fn saw(&mut self, seat: u8) -> bool {
        let before = self.playing();
        if let Some(t) = self.seen.get_mut(seat as usize) {
            *t = now();
        }
        before != self.playing()
    }
}

/// How long a seat is held for somebody who has stopped asking about it.
///
/// The page polls every three seconds, so this is not a measure of how long
/// somebody has been thinking: it is how long since their browser was there at
/// all. Two minutes is long enough to survive a laptop lid and a lost network,
/// and short enough that a table is not stopped for a quarter of an hour by
/// somebody who has gone to lunch.
///
/// What it costs the person who went away is that the house bot plays their seat
/// while they are gone. What it saves is everybody else's game, and the seat is
/// still theirs to come back to.
const AWAY_LIMIT: u64 = 2 * 60 * 1000;

/// How long a table waiting for people is held before it is closed.
///
/// A host who deals a table with a chair open and walks away leaves it holding a
/// seat for somebody who is not coming. Nothing used to resolve that: the table
/// sat on the home page advertising a seat, and the only end it had was falling
/// off the back of the sixteen, which is a way to stop existing rather than a
/// way to be settled.
///
/// Twenty minutes, measured from the last time anybody looked at it rather than
/// from when it was dealt, because an open page keeps asking: a host still at
/// the screen holds their table indefinitely, and one who closed the tab holds
/// it for twenty minutes. Long enough to make tea, short enough that the list of
/// tables is true.
const WAITING_LIMIT: u64 = 20 * 60 * 1000;

/// Tables kept in memory at once.
///
/// A table that falls off the end is not lost: every move writes the file, so it
/// is still readable and still has its analytics. What it loses is the ability
/// to be played on, which is the right thing to lose first, and finished games
/// go before unfinished ones.
const TABLES: usize = 16;

pub struct Server {
    /// Newest first. A `Vec` rather than a map because sixteen is small, the
    /// order is what the home page wants anyway, and one lock covers both.
    tables: Mutex<Vec<Table>>,
    store: Store,
    /// The command line's answers, for the tables this server deals itself.
    seats: u8,
    mode: TradeMode,
    /// The seed the next table gets. Counted rather than taken from the clock,
    /// so a fresh server deals the same sequence of boards every time and a
    /// game can be found again by its number.
    seed: Mutex<u64>,
}

impl Server {
    pub fn new(seats: u8, seed: u64, mode: TradeMode, dir: impl Into<std::path::PathBuf>) -> Self {
        // No table is dealt here. It used to be, because the root *was* a board
        // and there had to be one to show; the root is the home page now, every
        // table is dealt from it or from the lobby, and a table nobody asked for
        // is a row on that page for a game nobody is playing.
        Server {
            tables: Mutex::new(Vec::new()),
            store: Store::new(dir),
            seats,
            mode,
            seed: Mutex::new(seed),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The home page, for whoever is asking.
    ///
    /// Everything on it is read here rather than in the page: the tables from
    /// memory, the games from the store, and which of them are this visitor's.
    fn home(&self, player: &str) -> String {
        // The list is the one place that promises these tables exist, so it is
        // the place to stop promising the ones that no longer should.
        self.sweep();
        let open: Vec<crate::home::Open> = self
            .tables
            .lock()
            .unwrap()
            .iter()
            .map(|t| crate::home::Open {
                id: t.id.clone(),
                game: t.session.game().to_string(),
                host: t.session.name().to_string(),
                seats: t.session.table().0,
                mode: t.session.table().2,
                public: t.session.is_public(),
                started: !t.session.moves().is_empty(),
                turns: t.session.turn_no(),
                winner: t.session.winner(),
                age: now().saturating_sub(t.dealt),
                mine: !player.is_empty() && t.by == player,
                waiting: t.waiting(),
                seated: t.seat_of(player).is_some(),
            })
            .collect();
        // A table in memory is also a file on disk once it has been moved in, so
        // one of the two lists has to give it up or a game appears twice, once as
        // somewhere to sit and once as history. The line is whether it can still
        // be played: an unfinished table belongs to the joining list and nowhere
        // else, and a finished one is history whether or not it is still in
        // memory, because there is nothing left to do at it.
        let live: Vec<String> = open
            .iter()
            .filter(|t| t.winner.is_none())
            .map(|t| t.id.clone())
            .collect();
        // Theirs only. Everything else in the store is somebody else's game and
        // has no business on their front page.
        //
        // Theirs means played in, not dealt: a game somebody invited you to is
        // one of your games, and the chairs are what say so. Before people could
        // join, the two were the same question.
        let mine: Vec<Saved> = self
            .store
            .all()
            .into_iter()
            .filter(|g| {
                !live.contains(&g.id)
                    && !player.is_empty()
                    && (g.by == player || g.setup.chairs.iter().any(|c| c.who == player))
            })
            .collect();
        crate::home::page(&open, &mine)
    }

    /// Deal a table from a lobby's query.
    ///
    /// One reader for the whole of it, because there are two ways to arrive with
    /// one: the lobby posting to `api/new`, and an invite link opened as a GET.
    /// A link that meant something slightly different from the screen that wrote
    /// it is exactly the drift worth spending a method to avoid.
    ///
    /// Everything is optional and everything has a default that plays an
    /// ordinary game, so a query missing half its parameters still deals a
    /// table. That matters for a link: it is text somebody may have truncated,
    /// and a dead link is worse than a table with a default clock on it.
    fn deal(&self, query: &str, player: &str) -> String {
        let seats = param(query, "seats")
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);
        let mode = match param(query, "mode").as_deref() {
            Some("disabled") => TradeMode::Disabled,
            Some("restricted") => TradeMode::Restricted,
            _ => TradeMode::Full,
        };
        let seed = param(query, "seed")
            .or_else(|| param(query, "table"))
            .and_then(|v| crate::game::parse_seed(&decode(&v)))
            // No clock dependency: the newest table's seed advances.
            .unwrap_or_else(|| self.next_seed());
        // The clock is a lobby setting: which kind, and how many seconds it
        // allows. Zero seconds is untimed either way.
        let secs: u64 = param(query, "clockSecs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let increment: u64 = param(query, "clockInc")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let clock = Clock::parse(param(query, "clock").as_deref(), secs, increment);
        // The discard has an allowance of its own, because a seven is an
        // interruption and not part of anybody's turn. Zero is no limit, and an
        // absent parameter means the default rather than none: a lobby that does
        // not mention it still wants one.
        let discard_secs: u64 = param(query, "discardSecs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_DISCARD_SECS);
        let name = param(query, "name").unwrap_or_default();
        let log_shown = param(query, "log").as_deref() != Some("off");
        let public = wants_public(query);
        let named = param(query, "game").unwrap_or_default();
        let pace = Pace::parse(param(query, "pace").as_deref());
        // Anything but an explicit "rough" counts the stacks, since that is what
        // the rules already let anybody do (R-5.6).
        let bank_exact = param(query, "bank").as_deref() != Some("rough");
        let session = Session::new(seats, seed, mode)
            .with_clock(clock)
            .with_log(log_shown)
            .with_public(public)
            .with_game(&decode(&named))
            .with_pace(pace)
            .with_bank_exact(bank_exact)
            .with_discard_secs(discard_secs)
            .with_name(&decode(&name));
        // A new game is a new address *and* a new table. The old one keeps its
        // file, keeps its address and keeps being playable, which is what a
        // table has to be once there is a page listing more than one of them.
        let mut table = Table {
            id: mint_id(),
            session,
            dealt: now(),
            by: player.to_string(),
            chairs: chairs_from(query, seats, player, &name),
            seen: vec![now(); seats as usize],
            stirred: now(),
        };
        table.seat_the_people();
        let id = self.add(table);
        self.keep(&id);
        id
    }

    /// A seed for the next table, and the cursor moved on past it.
    fn next_seed(&self) -> u64 {
        let mut seed = self.seed.lock().unwrap();
        let mine = *seed;
        *seed = seed.wrapping_add(1);
        mine
    }

    /// Note that somebody is still there.
    ///
    /// Any request about a table counts, the page's own three second poll
    /// included, which is what makes an open page hold a seat and a closed one
    /// let it go.
    fn stir(&self, id: &str) {
        if let Some(t) = self.tables.lock().unwrap().iter_mut().find(|t| t.id == id) {
            t.stirred = now();
        }
    }

    /// Close the tables that were waiting for somebody who never came.
    ///
    /// Only tables that never started. A game with moves in it has a file and an
    /// address and somebody's afternoon in it; this is for the ones where a chair
    /// was held open, nobody arrived, and whoever dealt it went away. There is
    /// nothing to write down, because nothing happened: the store never had them.
    ///
    /// Called wherever the tables are read as a list, which is the same place the
    /// clock is wound anywhere else here. A server that only wakes when it is
    /// asked has no other moment to do it in.
    fn sweep(&self) {
        let cutoff = now().saturating_sub(WAITING_LIMIT);
        self.tables
            .lock()
            .unwrap()
            .retain(|t| t.session.started() || t.waiting() == 0 || t.stirred > cutoff);
    }

    /// Put a table at the front, and drop the ones that no longer fit.
    ///
    /// Finished first, then oldest, because a game somebody is still playing is
    /// the last thing to take off the table.
    fn add(&self, table: Table) -> String {
        // Before the eviction below, so a table nobody is coming to is closed
        // rather than pushing a game somebody is playing off the end.
        self.sweep();
        let id = table.id.clone();
        let mut tables = self.tables.lock().unwrap();
        tables.insert(0, table);
        while tables.len() > TABLES {
            let victim = tables
                .iter()
                .enumerate()
                .filter(|(i, t)| *i > 0 && t.session.winner().is_some())
                .map(|(i, _)| i)
                .next_back()
                .unwrap_or(tables.len() - 1);
            tables.remove(victim);
        }
        id
    }

    /// Make sure there are at least `want` finished games to look at.
    ///
    /// The analytics are the one part of this that cannot be looked at without
    /// a finished game behind it, and playing one out by hand to see whether a
    /// table renders is a poor way to spend an afternoon. Every seat is played
    /// by the same heuristic the bots use, so these are real games rather than
    /// random legal moves: the numbers on the page mean what they would mean.
    ///
    /// A floor rather than a count, because `./play` restarts the server on
    /// every change pushed to the branch and hands it the same options each
    /// time. "Play six" would play six more on every restart; "have six" is the
    /// same request asked in a way that can be asked twice.
    pub fn demo(&self, want: u32) -> Vec<String> {
        let mut have = self
            .store
            .all()
            .into_iter()
            .filter(|g| g.winner.is_some())
            .count() as u32;
        let mut out = Vec::new();
        // Attempts rather than games, because what is wanted is finished games
        // and a game is only finished once it has been played. The cap is there
        // so a table that cannot reach a winner stops rather than spinning: a
        // deal that goes nowhere is a bug to see, not a loop to hide in.
        let mut left = (want.saturating_sub(have)) * 4 + 4;
        while have < want && left > 0 {
            left -= 1;
            let mut session = Session::new(self.seats, self.next_seed(), self.mode)
                .with_pace(Pace::Instant)
                .with_name("Egon");
            // Every seat played by the table's own hand, the human's included:
            // there is nobody here to ask.
            session.play_out();
            let finished = session.winner().is_some();
            let id = self.add(Table {
                id: mint_id(),
                session,
                dealt: now(),
                // Nobody's: the server played it, so it belongs to no visitor and
                // shows up on nobody's home page as theirs.
                by: String::new(),
                // Nor is anybody sitting at it. It is finished by the time it
                // gets here, so there is nothing to sit down to.
                chairs: vec![Chair::Bot; self.seats as usize],
                seen: vec![0; self.seats as usize],
                stirred: now(),
            });
            self.keep(&id);
            if finished {
                have += 1;
                out.push(id);
            }
        }
        out
    }

    /// Put a stored game back on a table, so it can be played on again.
    ///
    /// This is what writing a game down as its moves was always for: seats, seed
    /// and the ordered steps rebuild the position exactly, so restarting the
    /// server costs the table and not the game. Before this, an unfinished game
    /// whose table had gone answered every click with "that game is over", which
    /// was true of the table and false of the game, and the only way out of it
    /// was to abandon a game somebody was in the middle of.
    ///
    /// Returns whether the game is now on a table. A finished one is not put back
    /// on one: there is nothing to play, and reading it off disk is what the
    /// report and the board's read-only view already do.
    ///
    /// **A game this build cannot replay is deleted.** The rules moved under it,
    /// so it is not a game any more; leaving it is a row on the home page that
    /// refuses to open, which is worse than either keeping it or losing it.
    fn seat(&self, id: &str) -> bool {
        if self.tables.lock().unwrap().iter().any(|t| t.id == id) {
            return true;
        }
        let Some(saved) = self.store.load(id) else {
            return false;
        };
        if saved.winner.is_some() {
            return false;
        }
        let Some(session) = Session::resume(saved.seats, saved.seed, saved.mode, &saved.moves)
        else {
            eprintln!("cannot replay {id}, deleting it");
            self.store.remove(id);
            return false;
        };
        // The position comes out of the moves and the arrangements out of the
        // file's settings, so a table taken up again is the table it was: the
        // same clock, the same pace, the same listing. It used to be the same
        // game on a differently arranged table, which was the thing you noticed
        // second, right after noticing that the game had come back at all.
        let session = session
            .with_name(&saved.name)
            .with_game(&saved.setup.game)
            .with_public(saved.setup.public)
            .with_pace(saved.setup.pace)
            .with_clock(saved.setup.clock)
            .with_discard_secs(saved.setup.discard_secs)
            .with_bank_exact(saved.setup.bank_exact)
            .with_log(saved.setup.log)
            .with_record(saved.times.clone());
        let mut table = Table {
            id: saved.id.clone(),
            session,
            dealt: saved.dealt,
            by: saved.by.clone(),
            chairs: saved
                .setup
                .chairs
                .iter()
                .map(|c| match c.who.as_str() {
                    "open" => Chair::Open,
                    "bot" => Chair::Bot,
                    key => Chair::Taken {
                        key: key.to_string(),
                        name: c.name.clone(),
                    },
                })
                .collect(),
            // Nobody has been heard from since the restart, which is the truth:
            // the bots hold the seats until their people's pages ask again, and
            // a table nobody comes back to waits rather than playing itself out.
            seen: vec![0; saved.setup.chairs.len().max(saved.seats as usize)],
            // Taken up now, whenever it was dealt: what the waiting limit asks
            // is how long since anybody looked, and somebody just did.
            stirred: now(),
        };
        table.seat_the_people();
        table.name_the_seats();
        self.add(table);
        true
    }

    /// Put this visitor in a seat at this table, if they are not in one already.
    ///
    /// The whole of joining. A table dealt with an open seat is a table with a
    /// chair nobody is in; the first person to open it takes the chair, and from
    /// then on the seat is theirs and the bots stop playing it. There is no
    /// lobby to wait in and no ready-check, because the game is already running:
    /// an open seat is played by the house bot until somebody takes it, which is
    /// what lets a table start with one person and finish with two.
    ///
    /// Returns which seat they are in, if any. Somebody who arrives at a full
    /// table gets nothing and watches, which is a real answer rather than a
    /// refusal: a game in progress is a thing you can look at (P-6).
    fn seated(&self, id: &str, player: &str) -> Option<u8> {
        let tables = self.tables.lock().unwrap();
        tables.iter().find(|t| t.id == id)?.seat_of(player)
    }

    /// Take an open chair, under a name.
    ///
    /// Only before the game starts. A game already running has nothing to offer
    /// somebody arriving: the seat has been played by a bot for forty turns and
    /// what they would be handed is whatever it built, which is not joining a
    /// game. So the first move closes the door, and after it the only way into a
    /// seat is to have been in it, which `seated` answers by key.
    ///
    /// Returns the seat taken, or `None` when there is nothing to take: no such
    /// table, no chair free, no key to hold it with, or a game already under way.
    fn sit(&self, id: &str, player: &str, name: &str) -> Option<u8> {
        let mut tables = self.tables.lock().unwrap();
        let table = tables.iter_mut().find(|t| t.id == id)?;
        if let Some(seat) = table.seat_of(player) {
            return Some(seat);
        }
        if player.is_empty() || table.session.started() || table.session.winner().is_some() {
            return None;
        }
        let seat = table.free_seat()?;
        let name = called(name, seat);
        table.chairs[seat as usize] = Chair::Taken {
            key: player.to_string(),
            name: name.clone(),
        };
        table.session.name_seat(seat, &name);
        table.seat_the_people();
        table.session.note_to_table(format!("{name} sat down"));
        table.stirred = now();
        let started = table.session.started();
        let saved = saved_of(table);
        drop(tables);
        // Only if the game is under way. A table still filling up is not a game
        // yet and has no business on disk: that is the same rule that keeps a
        // dealt-and-abandoned table out of the store, and writing the seating
        // down before the first move would have put every one of them there.
        if started {
            let _ = self.store.save(&saved);
        }
        Some(seat)
    }

    /// Give up a seat.
    ///
    /// Two different things, and which one it is depends on whether the game has
    /// begun. Before the first move nothing has happened yet, so the chair goes
    /// back to the table for somebody else to take and the person is simply not
    /// at it. After the first move the seat is part of a game in progress: it
    /// stays theirs, the house bot plays it, and they can come back to it, which
    /// is the same arrangement as somebody whose laptop shut.
    ///
    /// A seat cannot be handed to the table mid-game because the rules need every
    /// seat to move and the other players are owed an opponent, not a gap.
    fn leave(&self, id: &str, player: &str) -> bool {
        let mut tables = self.tables.lock().unwrap();
        let Some(table) = tables.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        let Some(seat) = table.seat_of(player) else {
            return false;
        };
        let name = match &table.chairs[seat as usize] {
            Chair::Taken { name, .. } => name.clone(),
            _ => String::new(),
        };
        if table.session.started() {
            // Heard from a long time ago, which is exactly what having gone is.
            // The seat keeps their key, so coming back is the ordinary path and
            // not a second kind of joining.
            table.seen[seat as usize] = 0;
            table
                .session
                .note_to_table(format!("{name} left the table"));
        } else {
            table.chairs[seat as usize] = Chair::Open;
            table.session.name_seat(seat, "");
            table
                .session
                .note_to_table(format!("{name} left, and the seat is free again"));
        }
        table.seat_the_people();
        let started = table.session.started();
        let saved = saved_of(table);
        drop(tables);
        if started {
            let _ = self.store.save(&saved);
        }
        true
    }

    /// Fill the open chairs with bots and let the game begin.
    ///
    /// The host's answer to nobody turning up. Without it a table dealt with a
    /// seat open waits for a person who may never arrive, and there would be no
    /// way out of it that was not "deal another table".
    fn start(&self, id: &str, player: &str) -> bool {
        let mut tables = self.tables.lock().unwrap();
        let Some(table) = tables.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        // Whoever dealt it. Not any seat: the other people at the table are
        // waiting for the same person the host is, and one of them deciding for
        // everybody is a different rule than the one this is.
        if !table.may_start(player) || table.session.started() {
            return false;
        }
        let short = table.waiting();
        if short == 0 {
            return true;
        }
        for c in table.chairs.iter_mut() {
            if *c == Chair::Open {
                *c = Chair::Bot;
            }
        }
        table.seat_the_people();
        table.session.note_to_table(if short == 1 {
            "The last seat went to the house bot".to_string()
        } else {
            format!("{short} seats went to the house bot")
        });
        table.stirred = now();
        let started = table.session.started();
        let saved = saved_of(table);
        drop(tables);
        if started {
            let _ = self.store.save(&saved);
        }
        true
    }

    /// A game as it stands, live or stored.
    ///
    /// The live one is taken from memory rather than from its file, so the
    /// analytics of a game in progress are the analytics of the position on the
    /// table and not of the last write.
    fn current(&self, id: &str) -> Option<Saved> {
        let tables = self.tables.lock().unwrap();
        if let Some(t) = tables.iter().find(|t| t.id == id) {
            return Some(saved_of(t));
        }
        drop(tables);
        self.store.load(id)
    }

    /// A game off disk, rendered the way the live one is.
    ///
    /// Read and replayed on every request rather than cached. A game is a few
    /// hundred bytes and replaying it costs microseconds, so a cache here would
    /// be a second copy of the truth to keep in step for no gain.
    fn stored(&self, id: &str) -> Option<String> {
        let saved = self.store.load(id)?;
        let session = Session::resume(saved.seats, saved.seed, saved.mode, &saved.moves)?
            .with_name(&saved.name);
        Some(view::render(&session))
    }

    /// Write the live game down, as it stands.
    ///
    /// After every move rather than at the end. A game abandoned halfway is
    /// still a game that happened, and a file only written when somebody wins
    /// is a file that mostly does not exist.
    ///
    /// A game nobody has moved in is the exception, and is not written at all.
    /// Every visit to `/` deals a table, so a file at that point would mean a
    /// game on disk for every time the page was opened and closed again, each
    /// of them a seed and nothing else. Those are not abandoned games, they are
    /// games that never started, and they were diluting every figure the
    /// analytics computed across the store. The first move writes the file.
    fn keep(&self, id: &str) {
        let tables = self.tables.lock().unwrap();
        if let Some(t) = tables.iter().find(|t| t.id == id)
            && !t.session.moves().is_empty()
        {
            let _ = self.store.save(&saved_of(t));
        }
    }

    /// Serve until the process is stopped.
    pub fn serve(&self, listener: TcpListener) {
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    // One connection at a time: there is one game and one
                    // player, and a thread pool would be pretending otherwise.
                    if let Err(e) = self.handle(s) {
                        eprintln!("connection: {e}");
                    }
                }
                Err(e) => eprintln!("accept: {e}"),
            }
        }
    }

    fn handle(&self, mut stream: TcpStream) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut request = String::new();
        if reader.read_line(&mut request)? == 0 {
            return Ok(());
        }
        let mut parts = request.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("/").to_string();

        // Headers, for the body length only.
        let mut length = 0usize;
        let mut cookies = String::new();
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line)? == 0 {
                break;
            }
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line
                .strip_prefix("Content-Length:")
                .or_else(|| line.strip_prefix("content-length:"))
            {
                length = v.trim().parse().unwrap_or(0);
            }
            if let Some(v) = line
                .strip_prefix("Cookie:")
                .or_else(|| line.strip_prefix("cookie:"))
            {
                cookies = v.trim().to_string();
            }
        }
        if length > MAX_BODY {
            return respond(&mut stream, 413, "text/plain", b"body too large");
        }
        let mut body = vec![0u8; length];
        if length > 0 {
            reader.read_exact(&mut body)?;
        }
        let body = String::from_utf8_lossy(&body).to_string();

        let (path, query) = match path.split_once('?') {
            Some((p, q)) => (p, q),
            None => (path.as_str(), ""),
        };

        // A game has an address, so the api lives under it: `/<id>/api/state`
        // rather than `/api/state`. One game is live at a time, but the page
        // asking has to say which game it thinks it is looking at, or a stale
        // tab would drive somebody else's board.
        let (game, path) = split_game(path);

        // Who is asking, as far as this server can tell: a key it handed this
        // browser on a first visit and nothing more. Not a login, not a name.
        // When there are accounts this is where one is looked up, and the cookie
        // becomes one way of proving which account you are rather than the whole
        // of the identity.
        let known = cookie(&cookies, PLAYER_COOKIE);
        let player = known.clone().unwrap_or_else(mint_key);
        // Handed back only when it is new, so an ordinary request carries no
        // header nobody needed.
        let issue = known.is_none();

        match (method.as_str(), path) {
            // The root is not a game, it is where you go to get one.
            ("GET", "/") if game.is_none() => {
                let page = self.home(&player);
                let set = if issue {
                    cookie_header(&player)
                } else {
                    String::new()
                };
                respond_with(
                    &mut stream,
                    200,
                    "text/html; charset=utf-8",
                    page.as_bytes(),
                    &set,
                )
            }
            // The lobby, which is where the home page's one button leads. The
            // board page served with no game behind it: the lobby is a screen of
            // that application and always was, and serving it from here means the
            // settings live in one place rather than in two forms that would
            // drift. The key is handed out here as well, because dealing from the
            // lobby is the first thing many visitors do and the table has to know
            // whose it is.
            ("GET", "/lobby") => respond_with(
                &mut stream,
                200,
                "text/html; charset=utf-8",
                PAGE.as_bytes(),
                &if issue {
                    cookie_header(&player)
                } else {
                    String::new()
                },
            ),
            ("GET", "/") => {
                let id = game.unwrap_or_default();
                // On a table, or on disk, or neither. A game nobody has heard of
                // is a 404 rather than a fresh board, because an address that
                // silently becomes a different game is worse than a dead link.
                let known = self.tables.lock().unwrap().iter().any(|t| t.id == id);
                if known || self.store.load(&id).is_some() {
                    respond_with(
                        &mut stream,
                        200,
                        "text/html; charset=utf-8",
                        PAGE.as_bytes(),
                        // The board page is often the first page somebody opens,
                        // from a link, so the key is handed out here too or their
                        // first game would belong to nobody.
                        &if issue {
                            cookie_header(&player)
                        } else {
                            String::new()
                        },
                    )
                } else {
                    respond(&mut stream, 404, "text/plain", b"no such game")
                }
            }
            // The analytics for one game (§10). Rendered here rather than
            // fetched and drawn: everything on it settled the moment the game
            // ended, so there is nothing for a script to do.
            ("GET", "/analytics") => {
                let id = game.clone().unwrap_or_default();
                let Some(saved) = self.current(&id) else {
                    return respond(&mut stream, 404, "text/plain", b"no such game");
                };
                // Every game here, this one included, oldest first: a rating is
                // a function of everything before it, so what this result did
                // cannot be worked out from this result alone.
                let mut history = self.store.all();
                history.reverse();
                if !history.iter().any(|g| g.id == saved.id) {
                    history.push(saved.clone());
                }
                match crate::analysis::study(&saved, &history) {
                    Some(study) => respond(
                        &mut stream,
                        200,
                        "text/html; charset=utf-8",
                        crate::report::page(&saved, &study).as_bytes(),
                    ),
                    None => respond(
                        &mut stream,
                        500,
                        "text/plain",
                        b"this game and this build disagree about the rules",
                    ),
                }
            }
            ("GET", p) if p.starts_with("/art/") && p.ends_with(".jpg") => {
                let name = p.trim_start_matches("/art/").trim_end_matches(".jpg");
                match PHOTOS.iter().find(|(n, _)| *n == name) {
                    Some((_, bytes)) => respond(&mut stream, 200, "image/jpeg", bytes),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", p) if p.starts_with("/art/") => {
                let name = p.trim_start_matches("/art/").trim_end_matches(".svg");
                match ART.iter().find(|(n, _)| *n == name) {
                    Some((_, body)) => respond(&mut stream, 200, "image/svg+xml", body.as_bytes()),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", p) if p.starts_with("/font/") => {
                let name = p.trim_start_matches("/font/").trim_end_matches(".woff2");
                match FONTS.iter().find(|(n, _)| *n == name) {
                    Some((_, bytes)) => respond(&mut stream, 200, "font/woff2", bytes),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", p) if p.starts_with("/sound/") => {
                let name = p.trim_start_matches("/sound/").trim_end_matches(".mp3");
                match SOUNDS.iter().find(|(n, _)| *n == name) {
                    Some((_, bytes)) => respond(&mut stream, 200, "audio/mpeg", bytes),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", "/api/state") => {
                let id = game.clone().unwrap_or_default();
                // An unfinished game off a table is put back on one first, so a
                // tab left open across a restart carries on where it was.
                self.seat(&id);
                self.stir(&id);
                // Which seat is theirs, if any. Looking at a table is not
                // sitting down at it any more: a chair is taken deliberately,
                // under a name, through `api/sit`, because with the door closing
                // at the first move it matters that you meant to come in.
                let seat = self.seated(&id, &player);
                // And that they are still there. A page asking is a person at
                // it, which is the only evidence this server can have; the seat
                // is re-let to the bots or taken back from them only when that
                // answer actually changes.
                if let Some(s) = seat {
                    let mut tables = self.tables.lock().unwrap();
                    if let Some(t) = tables.iter_mut().find(|t| t.id == id)
                        && t.saw(s)
                    {
                        t.seat_the_people();
                    }
                }
                let mut tables = self.tables.lock().unwrap();
                let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                    // What is left is a finished game, which is read rather than
                    // played: hand it back as it stands. Nothing ticks, because
                    // nothing is waiting.
                    drop(tables);
                    return match self.stored(&id) {
                        Some(p) => respond(&mut stream, 200, "application/json", p.as_bytes()),
                        None => respond(&mut stream, 404, "text/plain", b"no such game"),
                    };
                };
                // A server only wakes when asked, so this poll is the whole
                // clock: it is what lets a paced bot's wait expire, and what
                // ends a turn whose time ran out.
                t.session.tick();
                t.session.enforce_clock();
                // Their own seat's view, or a spectator's if they have none:
                // nobody is ever sent another seat's hand. Either way it carries
                // how many chairs are still going, because that is the one thing
                // about this table you can be too late for.
                let room = view::Room {
                    free: t.waiting(),
                    host: t.may_start(&player),
                };
                let payload = match seat {
                    Some(s) => view::render_seated(&t.session, s, room),
                    None => view::render_watching_room(&t.session, room),
                };
                drop(tables);
                self.keep(&id);
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            ("POST", "/api/act") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                let Some(seat) = self.seated(&id, &player) else {
                    return respond(&mut stream, 403, "text/plain", b"no seat of yours");
                };
                let mut tables = self.tables.lock().unwrap();
                let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                    return respond(&mut stream, 409, "text/plain", b"that game is over");
                };
                // A table with a chair nobody is in has not settled who is
                // playing, and the first move is what shuts the door. Moving
                // before that would leave those chairs open and unjoinable: the
                // home page would go on advertising seats that could not be
                // taken. The way past it is `api/start`, which is the host
                // saying the bots may have them.
                if t.waiting() > 0 {
                    let payload = view::render_for_with_note(
                        &t.session,
                        seat,
                        "the table is still waiting for people",
                    );
                    drop(tables);
                    return respond(&mut stream, 200, "application/json", payload.as_bytes());
                }
                let session = &mut t.session;
                let action = json::read_u64(&body, "action");
                let version = json::read_u64(&body, "version");
                let payload = match (action, version) {
                    // As their own seat, and the index is into their own list of
                    // choices: one person cannot press another's button, because
                    // the only thing they can name is something on their screen.
                    (Some(a), Some(v)) => match session.act_as(seat, a as usize, v) {
                        Ok(()) => view::render_for(&session, seat),
                        Err(e) => view::render_for_with_note(&session, seat, &refusal(&e)),
                    },
                    _ => view::render_for_with_note(session, seat, "malformed request"),
                };
                drop(tables);
                self.keep(&id);
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // Put back a development card whose action was never finished.
            ("POST", "/api/cancel") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                let Some(seat) = self.seated(&id, &player) else {
                    return respond(&mut stream, 403, "text/plain", b"no seat of yours");
                };
                let mut tables = self.tables.lock().unwrap();
                let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                    return respond(&mut stream, 409, "text/plain", b"that game is over");
                };
                let session = &mut t.session;
                let payload = match json::read_u64(&body, "version") {
                    Some(v) => match session.cancel_as(seat, v) {
                        Ok(()) => view::render_for(&session, seat),
                        Err(e) => view::render_for_with_note(&session, seat, &refusal(&e)),
                    },
                    None => view::render_for_with_note(&session, seat, "malformed request"),
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            ("POST", "/api/propose") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                let Some(seat) = self.seated(&id, &player) else {
                    return respond(&mut stream, 403, "text/plain", b"no seat of yours");
                };
                let mut tables = self.tables.lock().unwrap();
                let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                    return respond(&mut stream, 409, "text/plain", b"that game is over");
                };
                let session = &mut t.session;
                let give = json::read_u8_array(&body, "give", 5);
                let want = json::read_u8_array(&body, "want", 5);
                let version = json::read_u64(&body, "version");
                // Absent means the open market; a seat number addresses it.
                let to = json::read_u64(&body, "to").map(|n| n as u8);
                let payload = match (give, want, version) {
                    (Some(g), Some(w), Some(v)) => {
                        let g = [g[0], g[1], g[2], g[3], g[4]];
                        let w = [w[0], w[1], w[2], w[3], w[4]];
                        match session.propose_as(seat, to, g, w, v) {
                            Ok(()) => view::render_for(&session, seat),
                            Err(e) => view::render_for_with_note(&session, seat, &refusal(&e)),
                        }
                    }
                    _ => view::render_for_with_note(&session, seat, "malformed request"),
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // Take a chair at this table, under a name.
            //
            // Its own request rather than a side effect of opening the page,
            // because the door closes at the first move: walking past a table
            // should not seat you at it, and a seat you took should have your
            // name on it.
            ("POST", "/api/sit") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                let name = decode(&param(&body, "name").unwrap_or_default());
                let taken = self.sit(&id, &player, &name);
                let set = if issue {
                    cookie_header(&player)
                } else {
                    String::new()
                };
                let payload = format!("{{\"seat\":{}}}", taken.map_or(-1, i64::from));
                respond_with(
                    &mut stream,
                    200,
                    "application/json",
                    payload.as_bytes(),
                    &set,
                )
            }
            // Standing up from a seat.
            ("POST", "/api/leave") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                if !self.leave(&id, &player) {
                    return respond(&mut stream, 403, "text/plain", b"no seat of yours");
                }
                respond(&mut stream, 200, "application/json", b"{}")
            }
            // The host giving up on the seats nobody took.
            ("POST", "/api/start") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                if !self.start(&id, &player) {
                    return respond(&mut stream, 403, "text/plain", b"not yours to start");
                }
                let payload = match self.seated(&id, &player) {
                    Some(seat) => {
                        let tables = self.tables.lock().unwrap();
                        match tables.iter().find(|t| t.id == id) {
                            Some(t) => view::render_for(&t.session, seat),
                            None => String::from("{}"),
                        }
                    }
                    None => String::from("{}"),
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // Starting a game is the one thing that does not belong to a
            // game, so it is not scoped to one: any page may ask for a table.
            ("POST", "/api/new") => {
                let id = self.deal(query, &player);
                // The page is told where it now is, so it can move there
                // without asking again.
                let payload = format!("{{\"went\":\"/{id}/\"}}");
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // An invite link, opened. The same query `api/new` takes, as a GET,
            // so a link is a table somebody already described rather than a
            // second idea of what a table is.
            //
            // Not the host's name: whoever opens this is not them. Not their
            // key either, so the table belongs to whoever opened it, which is
            // the only reading that makes their home page useful.
            ("GET", "/join") => {
                let id = self.deal(query, &player);
                let set = if issue {
                    cookie_header(&player)
                } else {
                    String::new()
                };
                redirect_with(&mut stream, &format!("/{id}/"), &set)
            }
            _ => respond(&mut stream, 404, "text/plain", b"not found"),
        }
    }
}

/// Split a leading game id off a path.
///
/// `/abcd-efgh-ijkl/api/state` is the state of that game; `/api/state` on its
/// own is nobody's. The id is checked for shape here, so everything downstream
/// is working with something that could be an address rather than with whatever
/// arrived.
fn split_game(path: &str) -> (Option<String>, &str) {
    let rest = path.strip_prefix('/').unwrap_or(path);
    let (head, tail) = rest.split_once('/').unwrap_or((rest, ""));
    if !is_game_id(head) {
        return (None, path);
    }
    (
        Some(head.to_string()),
        if tail.is_empty() {
            "/"
        } else {
            &path[head.len() + 1..]
        },
    )
}

/// Somewhere else to go, with one extra header line if there is one.
///
/// Always a 303: the answer to "open this link" is a page to look at, at the
/// address that page belongs to, rather than the request repeated. It is what
/// puts an invite's receiver on their table's own address, so reloading is a
/// reload and not a second deal.
fn redirect_with(stream: &mut TcpStream, to: &str, extra: &str) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 303 See Other\r\n\
         Location: {to}\r\n\
         Content-Length: 0\r\n\
         Cache-Control: no-store\r\n\
         {extra}\
         Connection: close\r\n\r\n"
    );
    stream.write_all(head.as_bytes())?;
    stream.flush()
}

/// Unix milliseconds, or zero if the clock is behind 1970.
///
/// Milliseconds because a game's place in the order decides what the ratings
/// say about it, and a handful of games played back to back all land in the
/// same second.
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A fresh address for a game.
///
/// From the clock and the process, which is enough for a local server writing a
/// handful of games: two ids collide only if two games are dealt in the same
/// nanosecond by the same process, and the second would overwrite the first's
/// file rather than corrupt it.
fn mint_id() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    game_id(n ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
}

/// The cookie a visitor is known by.
const PLAYER_COOKIE: &str = "carranta";

/// Read one cookie out of a `Cookie:` header.
///
/// The header is somebody else's text, so the value is checked rather than
/// trusted: our keys are lower-case letters and digits, and anything else is
/// treated as no cookie at all. A key is only ever compared and stored, never
/// interpolated anywhere it could matter, and this keeps it that way.
fn cookie(header: &str, name: &str) -> Option<String> {
    header.split(';').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        if k.trim() != name {
            return None;
        }
        let v = v.trim();
        let ours = v.len() == KEY_LEN
            && v.bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        ours.then(|| v.to_string())
    })
}

/// Length of a visitor key. Sixteen of thirty-six characters is eighty-two bits,
/// which is far more than a local server needs and costs nothing.
const KEY_LEN: usize = 16;

/// A fresh key for a visitor nobody has seen before.
///
/// From the clock and the process, like the game addresses, because this server
/// has no other source of noise and does not need one: the key is a name for a
/// browser, not a secret that guards anything. When this becomes a login it will
/// be issued by whatever holds the accounts.
fn mint_key() -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
        ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut out = String::with_capacity(KEY_LEN);
    for _ in 0..KEY_LEN {
        out.push(ALPHABET[(n % ALPHABET.len() as u64) as usize] as char);
        // A different multiplier from the one above, so the two do not walk in
        // step and give a key that reads like the game address beside it.
        n = (n / ALPHABET.len() as u64) ^ n.wrapping_mul(0x2545_F491_4F6C_DD1D);
    }
    out
}

/// The header that hands a new visitor their key.
///
/// A year, because a home page that forgets your games when you close the tab is
/// not a home page. `HttpOnly` so no script can read it, `SameSite=Lax` so it is
/// not sent from another site's page, and no `Secure`, because this server is
/// loopback and http and the flag would stop the cookie working at all.
fn cookie_header(key: &str) -> String {
    format!(
        "Set-Cookie: {PLAYER_COOKIE}={key}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax\r\n"
    )
}

/// A name for a seat, given one somebody typed.
///
/// Trimmed, bounded, and never empty: a seat at a table has to be callable
/// something, and "Player 2" is a better answer than a blank where a name goes.
/// The engine keeps its own bound too, so this is about what the table shows
/// rather than about what it can hold.
fn called(name: &str, seat: u8) -> String {
    let name: String = name.trim().chars().take(24).collect();
    if name.is_empty() {
        format!("Player {}", seat + 1)
    } else {
        name
    }
}

/// Who sits in each seat, as the lobby says it.
///
/// `roles=you,open,bot,bot`: the same three words the lobby's own seat list
/// uses, in seat order. Seat nought is always whoever dealt the table, whatever
/// the query claims, because dealing a table you are not at is not a thing this
/// server can mean.
///
/// A missing or malformed list gives a table of bots behind the dealer, which is
/// the game this was before anybody could join one, and is the right answer for
/// a link somebody truncated.
fn chairs_from(query: &str, seats: u8, player: &str, name: &str) -> Vec<Chair> {
    // Decoded first: the page sends this through `URLSearchParams`, which
    // percent-encodes the commas, so splitting the raw value found one word
    // where there were four and put a bot in every seat.
    let said = decode(&param(query, "roles").unwrap_or_default());
    let mut said = said.split(',');
    (0..seats)
        .map(|i| {
            let word = said.next().unwrap_or("");
            if i == 0 {
                Chair::Taken {
                    key: player.to_string(),
                    name: called(name, 0),
                }
            } else if word == "open" {
                Chair::Open
            } else {
                Chair::Bot
            }
        })
        .collect()
}

/// Whether the lobby asked for a listed table.
///
/// Anything other than an explicit "public" is private, because a missing or
/// misspelled setting should leave the table unlisted rather than publish it by
/// accident. Listing is the answer that cannot be taken back.
fn wants_public(query: &str) -> bool {
    param(query, "visibility").as_deref() == Some("public")
}

/// A refusal in words a player can act on.
///
/// The engine's reasons are precise but terse; "a trade must give and take"
/// tells someone what to change, where `EmptySide` does not.
fn refusal(e: &Refused) -> String {
    match e {
        Refused::Stale => "the board moved on, try again".to_string(),
        Refused::NoSuchChoice => "that choice is no longer offered".to_string(),
        Refused::Illegal(why) => match why {
            Illegal::EmptySide => "a trade must give and take".to_string(),
            Illegal::TypeOverlap => "the same resource cannot be on both sides".to_string(),
            Illegal::CannotAfford => "you do not hold what you are offering".to_string(),
            Illegal::OfferLimit => "you have made enough offers this turn".to_string(),
            Illegal::MarketFull => "the market is full".to_string(),
            Illegal::TradeDisabled => "that offer is not allowed in this game".to_string(),
            Illegal::NotAParty => "you are not a party to that trade".to_string(),
            Illegal::OfferStale => "that offer can no longer be paid".to_string(),
            Illegal::WrongPhase => "not now".to_string(),
            other => format!("{other:?}"),
        },
    }
}

fn param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

/// Percent-decoding, enough for a query value.
///
/// Names arrive from a text field, so spaces and accents are ordinary rather
/// than exotic. Anything that is not valid UTF-8 after decoding is dropped
/// rather than guessed at.
fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                match u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                    Ok(b) => out.push(b),
                    // Not a real escape; keep it as typed.
                    Err(_) => out.push(b'%'),
                }
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn respond(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8]) -> std::io::Result<()> {
    respond_with(stream, status, kind, body, "")
}

/// The same, with one more header line, already terminated.
///
/// Which is only ever the cookie that hands a visitor their key, and only on the
/// two pages a visitor can arrive at. Threading it through rather than setting it
/// on every response keeps it out of the api answers, where nothing needs it.
fn respond_with(
    stream: &mut TcpStream,
    status: u16,
    kind: &str,
    body: &[u8],
    extra: &str,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        403 => "Forbidden",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n\
         Content-Type: {kind}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-store\r\n\
         {extra}\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sound_the_page_asks_for_is_carried_in_the_binary() {
        // The page names these three and there is nowhere else to get them:
        // no external request loads a sound, so a missing one is silence with
        // no error anywhere. Names checked against the page itself, so
        // renaming a file without renaming its use fails here.
        for name in [
            "error-008",
            "jingles-hit-10",
            "jingles-hit-15",
            "confirmation-001",
            "dice-throw-3",
            "card-place-1",
            "impact-generic-light-002",
            "drop-002",
        ] {
            let (_, bytes) = SOUNDS
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("{name} is served"));
            // ID3 or a bare MPEG frame. Either way it is audio and not an
            // empty file or a stray text asset.
            assert!(
                bytes.starts_with(b"ID3") || bytes.starts_with(&[0xFF]),
                "{name} is an mp3"
            );
            assert!(
                PAGE.contains(&format!("/sound/{name}.mp3")),
                "{name} is the name the page asks for"
            );
        }
    }

    #[test]
    fn every_development_card_has_a_face_the_page_asks_for() {
        // One face per card in `DEV_CARDS`, named for it. The page fetches them
        // by name at start-up and there is nowhere else to get them, so a
        // renamed file is a card with nothing on it and no error anywhere.
        for card in [
            "militia",
            "victory-point",
            "monopoly",
            "road-building",
            "invention",
        ] {
            let name = format!("dev-{card}");
            let (_, svg) = ART
                .iter()
                .find(|(n, _)| *n == name)
                .unwrap_or_else(|| panic!("{name} is served"));
            assert!(svg.starts_with("<svg"), "{name} is a drawing");
            // The page inlines these, so a rule inside one is a rule aimed at
            // the whole document. They style themselves through their own
            // names and nothing shorter.
            assert!(
                svg.contains("class=\"devName\""),
                "{name} names its classes"
            );
            assert!(
                PAGE.contains(&format!("'{name}'")),
                "{name} is the name the page asks for"
            );
        }
    }

    #[test]
    fn asking_for_played_games_twice_does_not_play_them_twice() {
        // `./play` restarts on every change pushed and passes the same options
        // back in, so this has to be a floor rather than a tally.
        let dir = std::env::temp_dir().join(format!("carranta-demo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 1, TradeMode::Full, &dir);
        let first = server.demo(2);
        assert_eq!(first.len(), 2, "two were missing");
        let again = server.demo(2);
        assert!(again.is_empty(), "and now none are");
        let more = server.demo(3);
        assert_eq!(more.len(), 1, "one short of three");
        let finished = server
            .store()
            .all()
            .into_iter()
            .filter(|g| g.winner.is_some())
            .count();
        assert_eq!(finished, 3);
        // And every game it wrote is one somebody won: `--demo` exists to give
        // the analytics something to read, and a game nobody won is exactly
        // what the analytics cannot read.
        let all = server.store().all();
        assert_eq!(all.len(), 3, "nothing on disk but the three");
        for g in &all {
            assert!(g.winner.is_some(), "{} was played out", g.id);
            assert!(!g.moves.is_empty());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_game_nobody_moved_in_is_not_written_down() {
        // Every visit to `/` used to deal a table, and writing one at that point
        // put a game on disk for every time the page was opened: a seed and
        // nothing else, and every figure computed across the store was then
        // divided by them.
        let dir = std::env::temp_dir().join(format!("carranta-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 7, TradeMode::Full, &dir);
        assert!(
            server.store().all().is_empty(),
            "a fresh server has no games"
        );
        // Nor does dealing one, which is what the lobby does.
        let id = server.add(Table {
            id: mint_id(),
            session: Session::new(4, 7, TradeMode::Full).with_name("Egon"),
            dealt: now(),
            by: "keytest0000000000".to_string(),
            // One person at seat nought and bots behind them, which is what a
            // table was before there was a second chair to sit in.
            chairs: vec![
                Chair::Taken {
                    key: "keytest0000000000".to_string(),
                    name: "Egon".to_string(),
                },
                Chair::Bot,
                Chair::Bot,
                Chair::Bot,
            ],
            seen: vec![now(); 4],
            stirred: now(),
        });
        server.keep(&id);
        assert!(server.store().all().is_empty(), "still nothing played");
        // The first move writes the file, and writes down whose it is.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let v = t.session.version();
            t.session.act(0, v).expect("the opening is playable");
        }
        server.keep(&id);
        let all = server.store().all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, id);
        assert_eq!(all[0].by, "keytest0000000000", "and whose game it is");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_game_off_its_table_is_taken_up_again_rather_than_frozen() {
        // The point of writing a game down as its moves. Restarting the server,
        // or dealing sixteen more tables, used to leave an unfinished game
        // answering every click with "that game is over": true of the table and
        // false of the game, and there was no way back into it.
        let dir = std::env::temp_dir().join(format!("carranta-resume-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 12, TradeMode::Full, &dir);
        let id = server.add(Table {
            id: mint_id(),
            // Set up the way a lobby would set it up, and not the way a fresh
            // session is: the arrangements have to come back too, or the game
            // returns to a differently arranged table.
            session: Session::new(4, 12, TradeMode::Full)
                .with_name("Egon")
                .with_game("Kitchen table")
                .with_public(true)
                // Instant so the bots answer inside this loop rather than
                // holding their move; it is not the default the file falls back
                // to either, so it still proves the pace came back.
                .with_pace(Pace::Instant)
                .with_clock(Clock::PerTurn(45))
                .with_discard_secs(20)
                .with_bank_exact(false)
                .with_log(false),
            dealt: now(),
            by: "keytest0000000000".to_string(),
            // One person at seat nought and bots behind them, which is what a
            // table was before there was a second chair to sit in.
            chairs: vec![
                Chair::Taken {
                    key: "keytest0000000000".to_string(),
                    name: "Egon".to_string(),
                },
                Chair::Bot,
                Chair::Bot,
                Chair::Bot,
            ],
            seen: vec![now(); 4],
            stirred: now(),
        });
        for _ in 0..6 {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let v = t.session.version();
            t.session.act(0, v).expect("the opening is playable");
        }
        server.keep(&id);
        let before = server.store().load(&id).expect("written down");
        assert!(before.winner.is_none(), "still being played");
        assert_eq!(before.times.len(), before.moves.len(), "and timed");

        // The table goes; the game does not.
        server.tables.lock().unwrap().clear();
        assert!(server.seat(&id), "taken up again");
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables
                .iter_mut()
                .find(|t| t.id == id)
                .expect("back on a table");
            assert_eq!(t.session.moves(), &before.moves[..], "the same game");
            assert_eq!(t.session.times(), &before.times[..], "with its own clock");
            assert_eq!(t.by, before.by, "still theirs");
            assert_eq!(t.dealt, before.dealt, "and still as old as it is");
            // And the same table: the lobby's answers came back with the game.
            assert_eq!(t.session.game(), "Kitchen table");
            assert!(t.session.is_public());
            assert_eq!(t.session.pace(), Pace::Instant);
            assert_ne!(
                Setup::default().pace,
                Pace::Instant,
                "which is not what a file with no pace in it falls back to"
            );
            assert_eq!(t.session.clock(), Clock::PerTurn(45));
            assert_eq!(t.session.discard_secs(), 20);
            assert!(!t.session.bank_exact());
            assert!(!t.session.log_shown());
            // And it can be played on, which is the whole point.
            let v = t.session.version();
            t.session.act(0, v).expect("playable");
        }
        server.keep(&id);
        let after = server.store().load(&id).expect("written down again");
        // More than one: a click is the human's move and then whatever the bots
        // do before the turn comes back round.
        assert!(after.moves.len() > before.moves.len(), "the game went on");
        assert_eq!(
            &after.times[..before.times.len()],
            &before.times[..],
            "history kept"
        );
        assert!(
            after.times[before.times.len()] >= before.times[before.times.len() - 1],
            "and the clock went forwards, not back to nought"
        );

        // A finished game is not put back on a table: there is nothing to play.
        let done = server.add(Table {
            id: mint_id(),
            session: {
                let mut s = Session::new(4, 3, TradeMode::Full);
                s.play_out();
                s
            },
            dealt: now(),
            by: String::new(),
            // One person at seat nought and bots behind them, which is what a
            // table was before there was a second chair to sit in.
            chairs: vec![
                Chair::Taken {
                    key: "keytest0000000000".to_string(),
                    name: "Egon".to_string(),
                },
                Chair::Bot,
                Chair::Bot,
                Chair::Bot,
            ],
            seen: vec![now(); 4],
            stirred: now(),
        });
        server.keep(&done);
        server.tables.lock().unwrap().clear();
        assert!(!server.seat(&done), "a game with a winner stays read-only");
        assert!(server.store().load(&done).is_some(), "and is still on disk");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_game_this_build_cannot_replay_is_thrown_away() {
        // The rules moved under it, so it is not a game any more. Left alone it
        // is a row on the home page that refuses to open, which is worse than
        // either keeping it or losing it.
        let dir = std::env::temp_dir().join(format!("carranta-broken-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 1, TradeMode::Full, &dir);
        let broken = Saved {
            id: game_id(77),
            seats: 4,
            seed: 5,
            mode: TradeMode::Full,
            name: "Egon".to_string(),
            by: "keytest0000000000".to_string(),
            dealt: now(),
            winner: None,
            setup: Setup::default(),
            // Ending a turn before the board has been dealt is not a move any
            // build of these rules will replay.
            moves: vec![crate::game::Step::Move(
                carranta_core::action::Action::EndTurn,
            )],
            times: vec![1],
        };
        server.store().save(&broken).expect("written");
        assert!(server.store().load(&broken.id).is_some());
        assert!(!server.seat(&broken.id), "it cannot be taken up");
        assert!(server.store().load(&broken.id).is_none(), "so it is gone");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_invite_link_deals_the_table_it_describes() {
        let dir = std::env::temp_dir().join(format!("carranta-join-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server = Box::leak(Box::new(Server::new(4, 7, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        // The query the lobby writes into the link: everything about the table,
        // and no name, because whoever opens it is not the host.
        let link = "/join?seats=3&mode=full&clock=chess&clockSecs=300&clockInc=7\
                    &discardSecs=25&log=off&visibility=public&game=Kitchen+table\
                    &pace=slow&bank=rough&seed=0abcd-0000-0001";
        let answer = get(port, link, "");
        assert!(answer.starts_with("HTTP/1.1 303 See Other"), "{answer:.40}");
        // A first-time visitor is handed a key here as well, or the table they
        // were invited to would belong to nobody and never reach their history.
        assert!(answer.contains("Set-Cookie: carranta="));
        let went = answer
            .lines()
            .find_map(|l| l.strip_prefix("Location: "))
            .expect("somewhere to go")
            .trim()
            .to_string();
        let id = went.trim_matches('/').to_string();
        assert!(is_game_id(&id), "{went} is a game's address");

        // And the table is the one the link described, all of it.
        let tables = server.tables.lock().unwrap();
        let t = tables.iter().find(|t| t.id == id).expect("dealt");
        let (seats, seed, mode) = t.session.table();
        assert_eq!(seats, 3);
        assert_eq!(mode, TradeMode::Full);
        assert_eq!(
            seed,
            crate::game::parse_seed("0abcd-0000-0001").expect("a seed")
        );
        assert_eq!(t.session.game(), "Kitchen table");
        assert!(t.session.is_public());
        assert_eq!(t.session.pace(), Pace::Slow);
        assert_eq!(
            t.session.clock(),
            Clock::Chess {
                bank: 300,
                increment: 7
            }
        );
        assert_eq!(t.session.discard_secs(), 25);
        assert!(!t.session.bank_exact());
        assert!(!t.session.log_shown());
        // Not the host's name: the link carries none, so the seat falls back to
        // the blank-name default rather than sitting somebody else down under
        // the name of the person who sent it.
        assert_eq!(t.session.name(), "you");
        // It belongs to whoever opened it, under the key they were just handed,
        // which is the only reading that puts it on the right home page.
        let key = answer
            .lines()
            .find_map(|l| l.strip_prefix("Set-Cookie: carranta="))
            .and_then(|v| v.split(';').next())
            .expect("a key was handed out");
        assert_eq!(t.by, key, "theirs, not the sender's");
        drop(tables);

        // A link somebody truncated still deals a table. A dead link is worse
        // than a table with a default clock on it.
        let bare = get(port, "/join", "");
        assert!(bare.starts_with("HTTP/1.1 303 See Other"), "{bare:.40}");
        let mangled = get(port, "/join?seats=&seed=not-a-seed&clock=", "");
        assert!(
            mangled.starts_with("HTTP/1.1 303 See Other"),
            "{mangled:.40}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_share_link_and_the_deal_describe_one_table() {
        // Two ways to arrive with a lobby's query, and they have to mean the
        // same thing. They are built by one function on the page and read by one
        // method here; this pins the halves that a rename would quietly split.
        const PAGE: &str = include_str!("../assets/index.html");
        assert!(
            PAGE.contains("load(`/api/new?${tableQuery(true)}`"),
            "dealing posts the table's own description"
        );
        assert!(
            PAGE.contains("return `${location.origin}/join?${tableQuery(false)}`;"),
            "and the link is the same description, without the host's name"
        );
        // Every field the server reads has to be one the page writes.
        for key in [
            "seats",
            "mode",
            "clock",
            "clockSecs",
            "clockInc",
            "discardSecs",
            "log",
            "visibility",
            "game",
            "pace",
            "bank",
        ] {
            assert!(PAGE.contains(&format!("{key}:")) || PAGE.contains(&format!("'{key}'")));
        }
    }

    #[test]
    fn a_second_person_can_sit_down_and_play() {
        let dir = std::env::temp_dir().join(format!("carranta-join-two-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server =
            Box::leak(Box::new(Server::new(4, 21, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        // A table dealt with one chair held open, the way the lobby now deals it.
        let host = "hostkey000000000";
        let dealt = get(
            port,
            "/join?seats=4&roles=you,open,bot,bot&pace=instant",
            host,
        );
        let went = dealt
            .lines()
            .find_map(|l| l.strip_prefix("Location: "))
            .expect("somewhere to go")
            .trim()
            .to_string();
        let id = went.trim_matches('/').to_string();

        // The host is in seat nought and nobody else is at the table yet.
        let mine = get(port, &format!("/{id}/api/state"), host);
        assert!(mine.contains("\"you\":0"), "the host sits at nought");
        assert!(mine.contains("\"people\":[0]"), "and alone");

        // A second person opens the table. Looking is not sitting: they are told
        // there is a chair going and are still in no seat.
        let guest = "guestkey00000000";
        let looking = get(port, &format!("/{id}/api/state"), guest);
        assert!(looking.contains("\"you\":-1"), "looking is not sitting");
        assert!(
            looking.contains("\"seatsFree\":1"),
            "but the chair is offered"
        );
        assert!(
            looking.contains("\"started\":false"),
            "and the door is open"
        );

        // Taking it is its own act, and it carries a name.
        let sat = post(port, &format!("/{id}/api/sit"), guest, "name=Vidal");
        assert!(sat.contains("\"seat\":1"), "{sat:.60}");
        let theirs = get(port, &format!("/{id}/api/state"), guest);
        assert!(theirs.contains("\"you\":1"), "the guest is in the chair");
        assert!(theirs.contains("\"people\":[0,1]"), "and the table knows");
        assert!(theirs.contains("Vidal"), "under the name they gave");
        assert!(theirs.contains("\"seatsFree\":0"), "and the table is full");

        // A third finds it full and watches: a seat of nobody's, an empty hand,
        // and nothing to press.
        let watcher = "watchkey00000000";
        let looking = get(port, &format!("/{id}/api/state"), watcher);
        assert!(looking.contains("\"you\":-1"), "no seat");
        assert!(looking.contains("\"choices\":[]"), "and nothing to do");
        let refused = post(port, &format!("/{id}/api/sit"), watcher, "name=Late");
        assert!(refused.contains("\"seat\":-1"), "and no chair to take");

        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.seat_of(host), Some(0));
            assert_eq!(t.seat_of(guest), Some(1));
            assert_eq!(t.seat_of(watcher), None, "watching is not sitting");
            assert_eq!(t.waiting(), 0, "the chair is taken");
            assert!(t.session.is_person(0) && t.session.is_person(1));
            assert!(!t.session.is_person(2) && !t.session.is_person(3));
        }

        // Only the seat being asked can move. The other's list is empty, so any
        // index they send names a choice they do not have.
        let version = |key: &str| -> u64 {
            let s = get(port, &format!("/{id}/api/state"), key);
            s.rsplit("\"version\":")
                .next()
                .and_then(|t| t.split(&[',', '}'][..]).next())
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or_default()
        };
        let play = |key: &str, v: u64| -> String {
            let body = format!("{{\"action\":0,\"version\":{v}}}");
            ask(
                port,
                &format!(
                    "POST /{id}/api/act HTTP/1.1\r\nHost: localhost\r\n\
                     Cookie: {PLAYER_COOKIE}={key}\r\n\
                     Content-Length: {}\r\n\r\n{body}",
                    body.len()
                ),
            )
        };
        let v = version(host);
        let idle = play(guest, v);
        assert!(
            idle.contains("no longer offered"),
            "the seat that is not being asked cannot move"
        );
        // And the host, who is, can.
        let moved = play(host, v);
        assert!(moved.starts_with("HTTP/1.1 200 OK"));
        assert!(version(host) > v, "the board moved on");

        // Somebody with no seat cannot act at all, whatever they send.
        let watched = play(watcher, version(host));
        assert!(watched.starts_with("HTTP/1.1 403"), "{watched:.40}");

        // The seating is written down, so it survives the table being put away.
        server.keep(&id);
        let saved = server.store().load(&id).expect("written");
        assert_eq!(
            saved
                .setup
                .chairs
                .iter()
                .map(|c| c.who.as_str())
                .collect::<Vec<_>>(),
            vec![host, guest, "bot", "bot"]
        );
        server.tables.lock().unwrap().clear();
        assert!(server.seat(&id), "taken up again");
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("back");
            assert_eq!(t.seat_of(host), Some(0), "still the host's chair");
            assert_eq!(t.seat_of(guest), Some(1), "and still the guest's");
            assert!(t.session.is_person(1), "and the bots still leave it alone");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_door_closes_on_the_first_move_and_only_rejoining_gets_you_back() {
        let dir = std::env::temp_dir().join(format!("carranta-door-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server =
            Box::leak(Box::new(Server::new(4, 33, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        let host = "hostkey000000000";
        let dealt = get(
            port,
            "/join?seats=4&roles=you,open,open,bot&name=Marta&pace=instant",
            host,
        );
        let went = dealt
            .lines()
            .find_map(|l| l.strip_prefix("Location: "))
            .expect("somewhere to go")
            .trim()
            .to_string();
        let id = went.trim_matches('/').to_string();

        // Two chairs going, and the host's own name on theirs.
        let mine = get(port, &format!("/{id}/api/state"), host);
        assert!(mine.contains("\"seatsFree\":2"));
        assert!(mine.contains("\"youMayStart\":true"));
        assert!(mine.contains("Marta"));

        // And nobody can move while a chair is empty: the first move shuts the
        // door, so making one now would leave those chairs open and unjoinable.
        let held = {
            let v = get(port, &format!("/{id}/api/state"), host);
            let v: u64 = v
                .rsplit("\"version\":")
                .next()
                .and_then(|t| t.split(&[',', '}'][..]).next())
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or_default();
            let body = format!("{{\"action\":0,\"version\":{v}}}");
            ask(
                port,
                &format!(
                    "POST /{id}/api/act HTTP/1.1\r\nHost: localhost\r\n\
                     Cookie: {PLAYER_COOKIE}={host}\r\n\
                     Content-Length: {}\r\n\r\n{body}",
                    body.len()
                ),
            )
        };
        assert!(held.contains("still waiting for people"), "{held:.80}");

        // One person takes a chair before the game starts.
        let early = "earlykey00000000";
        assert!(
            post(port, &format!("/{id}/api/sit"), early, "name=Vidal").contains("\"seat\":1"),
            "a chair going is a chair you may take"
        );

        // The host gives up on the last one, which is what starts the game.
        assert!(
            post(port, &format!("/{id}/api/start"), early, "").starts_with("HTTP/1.1 403"),
            "and it is not the other players' call"
        );
        assert!(post(port, &format!("/{id}/api/start"), host, "").starts_with("HTTP/1.1 200"));
        let after = get(port, &format!("/{id}/api/state"), host);
        assert!(after.contains("\"seatsFree\":0"), "the chair went to a bot");

        // Now the door is shut. Somebody arriving is a watcher, whatever they
        // ask for and however many bots are sitting where they might have been.
        let late = "latekey000000000";
        let turned_away = post(port, &format!("/{id}/api/sit"), late, "name=Late");
        assert!(turned_away.contains("\"seat\":-1"), "{turned_away:.60}");
        assert!(get(port, &format!("/{id}/api/state"), late).contains("\"you\":-1"));

        // A move, which is what makes this a game with a file: an unstarted
        // table has nothing on disk, because nothing has happened at it.
        assert!(server.store().load(&id).is_none(), "nothing written yet");
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            assert!(!t.session.started(), "dealt, filled, and not yet played");
            let v = t.session.version();
            t.session.act_as(0, 0, v).expect("the opening is playable");
            assert!(t.session.started(), "and now it is under way");
        }

        // And the two who were in seats are still in them, across a restart.
        server.keep(&id);
        server.tables.lock().unwrap().clear();
        assert!(server.seat(&id));
        assert_eq!(server.seated(&id, host), Some(0), "rejoining is by key");
        assert_eq!(server.seated(&id, early), Some(1));
        assert_eq!(server.seated(&id, late), None, "and was never a way in");
        let back = get(port, &format!("/{id}/api/state"), early);
        assert!(back.contains("Vidal"), "still under their own name");

        // A chair that comes free in a game already under way is not a way in:
        // the rule is about the game having begun, not about the seat.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            t.chairs[3] = Chair::Open;
        }
        let too_late = post(port, &format!("/{id}/api/sit"), late, "name=Late");
        assert!(
            too_late.contains("\"seat\":-1"),
            "the door is shut, not the seat"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_table_waiting_for_nobody_is_closed() {
        let dir = std::env::temp_dir().join(format!("carranta-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 44, TradeMode::Full, &dir);
        let host = "hostkey000000000";

        // Three tables: one holding a chair, one full, and one already played.
        let waiting = server.deal("seats=4&roles=you,open,bot,bot", host);
        let full = server.deal("seats=4&roles=you,bot,bot,bot", host);
        let played = server.deal("seats=4&roles=you,open,bot,bot", host);
        {
            let mut tables = server.tables.lock().unwrap();
            for t in tables.iter_mut() {
                if t.id == played {
                    let v = t.session.version();
                    t.session.act_as(0, 0, v).expect("playable");
                }
                // Every one of them last looked at half an hour ago.
                t.stirred = now().saturating_sub(30 * 60 * 1000);
            }
        }
        server.keep(&played);

        server.sweep();
        let left: Vec<String> = server
            .tables
            .lock()
            .unwrap()
            .iter()
            .map(|t| t.id.clone())
            .collect();
        assert!(
            !left.contains(&waiting),
            "a table holding a chair nobody took is closed"
        );
        assert!(
            left.contains(&full),
            "a table with every seat settled is not: nothing is being waited for"
        );
        assert!(
            left.contains(&played),
            "and a game somebody is playing is never swept, whatever it is short"
        );
        // Nothing was written for the one that closed, because nothing happened
        // at it: the store's own rule, and this is the case it was written for.
        assert!(server.store().load(&waiting).is_none(), "and left no file");

        // Somebody still looking at a table keeps it. The page's own poll is
        // what does this in practice; here it is the same call by hand.
        let held = server.deal("seats=4&roles=you,open,bot,bot", host);
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == held).expect("dealt");
            t.stirred = now().saturating_sub(30 * 60 * 1000);
        }
        server.stir(&held);
        server.sweep();
        assert!(
            server.tables.lock().unwrap().iter().any(|t| t.id == held),
            "an open page holds its table"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_table_still_filling_up_is_not_written_down() {
        // The store's oldest rule: a game nobody moved in is not a game. Sitting
        // down and starting both used to write the file anyway, which put every
        // dealt-and-abandoned table into the store for the analytics to divide
        // by.
        let dir = std::env::temp_dir().join(format!("carranta-unwritten-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 5, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let id = server.deal("seats=4&roles=you,open,bot,bot&name=Marta", host);
        assert!(server.store().all().is_empty(), "dealing writes nothing");
        assert_eq!(server.sit(&id, "guestkey00000000", "Vidal"), Some(1));
        assert!(server.store().all().is_empty(), "nor does sitting down");
        assert!(server.start(&id, host));
        assert!(
            server.store().all().is_empty(),
            "nor does filling the chairs"
        );
        // The first move writes it, with everybody's seat and name in it.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let v = t.session.version();
            t.session.act_as(0, 0, v).expect("playable");
        }
        server.keep(&id);
        let saved = server.store().load(&id).expect("written now");
        assert_eq!(saved.setup.chairs[0].name, "Marta");
        assert_eq!(saved.setup.chairs[1].name, "Vidal");
        assert_eq!(saved.setup.chairs[3].who, "bot");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn leaving_before_the_start_gives_the_chair_back() {
        let dir = std::env::temp_dir().join(format!("carranta-leave-early-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 61, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal("seats=4&roles=you,open,bot,bot&name=Marta", host);
        assert_eq!(server.sit(&id, guest, "Vidal"), Some(1));

        // Nothing has happened yet, so standing up leaves nothing behind: the
        // chair is the table's again and somebody else may take it.
        assert!(server.leave(&id, guest));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.chairs[1], Chair::Open);
            assert_eq!(t.waiting(), 1, "and the table is short again");
            assert_eq!(t.seat_of(guest), None, "they are not at it");
            assert_eq!(t.session.name_of(1), "", "and their name went with them");
        }
        assert_eq!(server.sit(&id, "thirdkey00000000", "Nils"), Some(1));
        assert!(
            server.store().all().is_empty(),
            "and none of it was written"
        );

        // The host standing up before the start hands the table on rather than
        // stranding it: whoever is in seat nought may start it.
        let id = server.deal("seats=4&roles=you,open,bot,bot&name=Marta", host);
        assert_eq!(server.sit(&id, guest, "Vidal"), Some(1));
        assert!(server.leave(&id, host));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert!(!t.may_start(guest), "not any seat");
        }
        assert_eq!(server.sit(&id, guest, "Vidal"), Some(1), "still theirs");
        // Somebody takes the empty seat nought, and it is theirs to start.
        assert_eq!(server.sit(&id, "fourthkey0000000", "Aleks"), Some(0));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert!(t.may_start("fourthkey0000000"), "seat nought may start it");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn leaving_a_game_under_way_hands_the_seat_to_a_bot_and_keeps_it_yours() {
        let dir = std::env::temp_dir().join(format!("carranta-leave-late-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 62, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal(
            "seats=4&roles=you,open,bot,bot&name=Marta&pace=instant",
            host,
        );
        assert_eq!(server.sit(&id, guest, "Vidal"), Some(1));
        assert!(server.start(&id, host));
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let v = t.session.version();
            t.session.act_as(0, 0, v).expect("playable");
            assert!(t.session.started());
        }

        // Mid-game, the seat cannot go back to the table: the others are owed an
        // opponent rather than a gap. It stays theirs and the bot plays it.
        assert!(server.leave(&id, guest));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.seat_of(guest), Some(1), "the seat is still theirs");
            assert_eq!(t.waiting(), 0, "and is not on offer to anybody else");
            assert!(!t.present(1), "but nobody is in it");
            assert!(!t.session.is_person(1), "so the bots play it");
            assert!(
                t.session.is_person(0),
                "and the person still there is asked"
            );
        }

        // Coming back is the ordinary path: their next request is the evidence.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            assert!(t.saw(1), "their return changes who is playing");
            t.seat_the_people();
            assert!(t.session.is_person(1), "and the seat is theirs again");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_table_nobody_is_at_waits_rather_than_playing_itself_out() {
        // One person gone is a seat the bots cover. Everybody gone is a game
        // that must not finish without them: a table that played itself to the
        // end while the room was empty would be a game destroyed, not continued.
        let dir = std::env::temp_dir().join(format!("carranta-empty-room-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 63, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal(
            "seats=4&roles=you,open,bot,bot&name=Marta&pace=instant",
            host,
        );
        assert_eq!(server.sit(&id, guest, "Vidal"), Some(1));
        assert!(server.start(&id, host));

        let mut tables = server.tables.lock().unwrap();
        let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
        assert_eq!(t.playing(), vec![0, 1], "both here");

        // One walks away.
        t.seen[1] = 0;
        assert_eq!(t.playing(), vec![0], "the other carries on");

        // Then so does the other, and the table waits for both of them.
        t.seen[0] = 0;
        assert_eq!(
            t.playing(),
            vec![0, 1],
            "an empty room waits rather than playing on"
        );
        drop(tables);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_seat_is_always_called_something() {
        // A blank is not a name, and a table has to be able to say whose turn it
        // is. Trimmed and bounded, because it is somebody else's text.
        assert_eq!(called("Vidal", 1), "Vidal");
        assert_eq!(called("  Vidal  ", 1), "Vidal");
        assert_eq!(called("", 1), "Player 2");
        assert_eq!(called("   ", 3), "Player 4");
        assert_eq!(called(&"x".repeat(80), 0).chars().count(), 24);
    }

    #[test]
    fn a_table_of_bots_is_dealt_when_the_roles_are_missing_or_broken() {
        // A link somebody truncated, or a page from an older build. Bots behind
        // the dealer is the table this was before there was a second chair, and
        // is the one answer that cannot leave a seat waiting for nobody.
        for query in ["", "roles=", "roles=nonsense", "roles=you"] {
            let chairs = chairs_from(query, 4, "keytest0000000000", "Egon");
            assert_eq!(
                chairs[0],
                Chair::Taken {
                    key: "keytest0000000000".to_string(),
                    name: "Egon".to_string()
                }
            );
            assert!(
                chairs[1..].iter().all(|c| *c == Chair::Bot),
                "{query:?} left a chair open"
            );
        }
        // Seat nought is the dealer whatever the query says about it.
        let chairs = chairs_from("roles=open,open,bot,bot", 4, "keytest0000000000", "Egon");
        assert_eq!(
            chairs[0],
            Chair::Taken {
                key: "keytest0000000000".to_string(),
                name: "Egon".to_string()
            }
        );
        assert_eq!(chairs[1], Chair::Open);
        assert_eq!(chairs[2], Chair::Bot);
    }

    #[test]
    fn a_path_says_which_game_it_is_about() {
        let id = "6t8y-tghb-2t2x";
        assert_eq!(split_game("/"), (None, "/"));
        assert_eq!(split_game("/api/state"), (None, "/api/state"));
        assert_eq!(split_game(&format!("/{id}/")), (Some(id.to_string()), "/"));
        assert_eq!(
            split_game(&format!("/{id}/api/state")),
            (Some(id.to_string()), "/api/state")
        );
        assert_eq!(
            split_game(&format!("/{id}/analytics")),
            (Some(id.to_string()), "/analytics")
        );
        // Anything that is not an id is left where it is, so a path that
        // happens to start with a word is not read as an address.
        assert_eq!(split_game("/art/city.svg"), (None, "/art/city.svg"));
        assert_eq!(split_game("/../../etc/passwd"), (None, "/../../etc/passwd"));
    }

    #[test]
    fn query_parameters_are_read_or_ignored() {
        assert_eq!(param("seats=3&mode=full", "seats").as_deref(), Some("3"));
        assert_eq!(param("seats=3&mode=full", "mode").as_deref(), Some("full"));
        assert_eq!(param("seats=3", "seed"), None);
        assert_eq!(param("", "seats"), None);
        assert_eq!(param("broken", "seats"), None);
    }

    #[test]
    fn a_table_is_private_unless_it_asks_to_be_listed() {
        assert!(wants_public("seats=4&visibility=public"));
        assert!(!wants_public("seats=4&visibility=private"));
        // The cases that must not publish: absent, empty, misspelled, and the
        // wrong case. Every one of them leaves the table unlisted.
        assert!(!wants_public("seats=4"));
        assert!(!wants_public(""));
        assert!(!wants_public("visibility="));
        assert!(!wants_public("visibility=publi"));
        assert!(!wants_public("visibility=Public"));
        assert!(!wants_public("visibility=true"));
    }

    #[test]
    fn a_session_starts_unlisted() {
        let s = Session::new(4, 7, TradeMode::Full);
        assert!(!s.is_public());
        assert!(
            Session::new(4, 7, TradeMode::Full)
                .with_public(true)
                .is_public()
        );
    }

    #[test]
    fn a_cookie_is_read_only_when_it_looks_like_one_of_ours() {
        let key = "abc123def456ghi7";
        assert_eq!(key.len(), KEY_LEN);
        assert_eq!(
            cookie(&format!("carranta={key}"), "carranta").as_deref(),
            Some(key)
        );
        // Beside other people's cookies, and with the spacing a browser sends.
        assert_eq!(
            cookie(&format!("other=1; carranta={key}; third=x"), "carranta").as_deref(),
            Some(key)
        );
        // A key is only ever compared and stored, and this is what keeps it that
        // way: the wrong length, the wrong alphabet or the wrong name is no
        // cookie at all rather than a value to be careful with later.
        assert_eq!(cookie("carranta=short", "carranta"), None);
        assert_eq!(cookie("carranta=ABC123DEF456GHI7", "carranta"), None);
        assert_eq!(cookie("carranta=abc123def456gh!7", "carranta"), None);
        assert_eq!(cookie("carrantaX=abc123def456ghi7", "carranta"), None);
        assert_eq!(cookie("", "carranta"), None);
        assert_eq!(cookie("carranta", "carranta"), None);
    }

    #[test]
    fn a_minted_key_is_the_shape_the_reader_accepts() {
        // The two halves of this have to agree or every visitor is a new one on
        // every request, and nothing would say so: the page would simply never
        // show anybody their games.
        let key = mint_key();
        assert_eq!(key.len(), KEY_LEN);
        assert_eq!(
            cookie(&format!("carranta={key}"), PLAYER_COOKIE).as_deref(),
            Some(key.as_str())
        );
    }

    #[test]
    fn the_cookie_outlives_the_tab_and_is_kept_from_scripts() {
        let header = cookie_header("abc123def456ghi7");
        // A year, because a home page that forgets your games when you close the
        // tab is not a home page.
        assert!(header.contains("Max-Age=31536000"));
        assert!(header.contains("Path=/"));
        assert!(header.contains("HttpOnly"));
        assert!(header.contains("SameSite=Lax"));
        // Terminated, because it is written into the head between two others.
        assert!(header.ends_with("\r\n"));
    }

    /// One request, and the whole response as text.
    ///
    /// Raw rather than through a client, because there is no client: this server
    /// is `std` alone and so is anything that talks to it.
    fn ask(port: u16, request: &str) -> String {
        use std::net::TcpStream;
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("the door is open");
        s.write_all(request.as_bytes()).expect("asked");
        let mut out = Vec::new();
        s.read_to_end(&mut out).expect("answered");
        String::from_utf8_lossy(&out).into_owned()
    }

    /// A form post, which is how the page asks for a seat.
    fn post(port: u16, path: &str, cookie: &str, form: &str) -> String {
        ask(
            port,
            &format!(
                "POST {path} HTTP/1.1\r\nHost: localhost\r\n\
                 Cookie: {PLAYER_COOKIE}={cookie}\r\n\
                 Content-Type: application/x-www-form-urlencoded\r\n\
                 Content-Length: {}\r\n\r\n{form}",
                form.len()
            ),
        )
    }

    fn get(port: u16, path: &str, cookie: &str) -> String {
        let jar = if cookie.is_empty() {
            String::new()
        } else {
            format!("Cookie: {PLAYER_COOKIE}={cookie}\r\n")
        };
        ask(
            port,
            &format!("GET {path} HTTP/1.1\r\nHost: localhost\r\n{jar}\r\n"),
        )
    }

    #[test]
    fn the_home_page_hands_out_a_key_and_lists_what_it_should() {
        let dir = std::env::temp_dir().join(format!("carranta-home-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        // Leaked rather than joined: this server serves until the process ends,
        // which is what `serve` is, and the test is the process.
        let server: &'static Server = Box::leak(Box::new(Server::new(4, 7, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        // A first visit is met with a page and a key.
        let first = get(port, "/", "");
        assert!(first.starts_with("HTTP/1.1 200 OK"), "{first:.40}");
        assert!(first.contains("href=\"/lobby\""), "somewhere to start one");
        assert!(first.contains("No tables."), "nothing dealt yet");
        let key = first
            .lines()
            .find_map(|l| l.strip_prefix("Set-Cookie: carranta="))
            .and_then(|v| v.split(';').next())
            .expect("a key was handed out")
            .to_string();
        assert_eq!(
            cookie(&format!("carranta={key}"), PLAYER_COOKIE),
            Some(key.clone())
        );

        // A visit carrying that key is handed no second one.
        let again = get(port, "/", &key);
        assert!(!again.contains("Set-Cookie"), "one key per browser");

        // The lobby is the board page with no game behind it, and it hands out a
        // key too, because dealing from it is the first thing many visitors do.
        let lobby = get(port, "/lobby", "");
        assert!(lobby.starts_with("HTTP/1.1 200 OK"), "{lobby:.40}");
        assert!(lobby.contains("id=\"lobby\""), "it is the lobby screen");
        assert!(lobby.contains("Set-Cookie: carranta="));

        // Deal one the way the lobby does, and it is listed as this visitor's.
        let dealt = ask(
            port,
            &format!(
                "POST /api/new?seats=3&name=Egon&game=Test&visibility=public                  HTTP/1.1\r\nHost: localhost\r\n\
                 Cookie: {PLAYER_COOKIE}={key}\r\n\r\n"
            ),
        );
        assert!(dealt.starts_with("HTTP/1.1 200 OK"), "{dealt:.40}");
        let went = dealt
            .rsplit("\"went\":\"")
            .next()
            .and_then(|t| t.split('"').next())
            .expect("somewhere to go")
            .to_string();
        let id = went.trim_matches('/').to_string();
        assert!(is_game_id(&id), "{went} is a game's address");
        assert!(get(port, &went, &key).starts_with("HTTP/1.1 200 OK"));

        let listed = get(port, "/", &key);
        assert!(listed.contains(&id), "the table is listed");
        assert!(listed.contains("Test"), "under the name it was given");
        assert!(listed.contains("yours"));
        // Not "Sit down": they are already in it. Sitting down is what a chair
        // nobody is in offers, and this table's other seats are bots.
        assert!(listed.contains("Back to it"));
        assert!(!listed.contains("a seat free"));
        assert!(listed.contains("<td>3</td>"), "three seats, as asked");

        // Somebody else sees it too, because it was dealt as a listed table, but
        // not as theirs.
        let stranger = get(port, "/", "zzzz999999999999");
        assert!(stranger.contains(&id));
        assert!(!stranger.contains("yours"));

        // An address nobody dealt is a dead link rather than a fresh board.
        assert!(get(port, "/9999-9999-9999/", &key).starts_with("HTTP/1.1 404"));

        // Play it to the end, from underneath, and it moves from the tables to
        // the history: a game with a winner is not somewhere to sit, and the two
        // lists must not both claim it.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            t.session.play_out();
            assert!(t.session.winner().is_some(), "the table reached a winner");
        }
        server.keep(&id);
        let after = get(port, "/", &key);
        let tables_card = after.split("Your games").next().expect("a tables card");
        assert!(!tables_card.contains(&id), "no longer somewhere to sit");
        assert!(after.contains(&id), "but still on the page");
        assert!(
            after.contains(&format!("/{id}/analytics")),
            "with its report, which is what a finished game has"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
