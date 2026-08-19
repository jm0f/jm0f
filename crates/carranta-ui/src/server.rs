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

/// How long a state request may be held before answering anyway.
///
/// Under every proxy timeout worth worrying about: Railway and Cloudflare both
/// allow far longer, but a held request that a proxy kills looks to the page
/// like a network error rather than like nothing having happened. Twenty
/// seconds is short enough to be safe everywhere and long enough that an idle
/// table costs three answers a minute instead of twenty.
const HOLD: std::time::Duration = std::time::Duration::from_secs(20);

/// How often a held request looks again.
///
/// This is the latency of a move reaching the other screens, so it wants to be
/// small; it is also a lock acquisition per connection per interval, so it does
/// not want to be tiny. A tenth of a second is imperceptible to a person and
/// nothing to the machine.
const WAKE: std::time::Duration = std::time::Duration::from_millis(100);

/// The board page as served: the raw asset with the build stamped into its
/// header.
///
/// The other pages carry the stamp because the server renders them; this one is
/// a file, and its stamp was filled in by script from the first payload, which
/// works on a board and not on the lobby: the lobby has no game behind it, so
/// nothing ever arrived to fill it and the one screen most likely to be checked
/// after a rebuild said nothing. Substituted once, at first use, because the
/// answer cannot change while the process lives.
fn page_served() -> &'static str {
    static SERVED: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SERVED.get_or_init(|| {
        PAGE.replace(
            "<span class=\"build\" id=\"build\"></span>",
            &format!(
                "<span class=\"build\" id=\"build\">{}</span>",
                env!("CARRANTA_BUILD")
            ),
        )
    })
}

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
            chat: t.chat,
            chairs: t
                .chairs
                .iter()
                .enumerate()
                .map(|(seat, c)| match c {
                    Chair::Bot => SavedChair::bot(),
                    Chair::Open => SavedChair::open(),
                    Chair::Taken { key, name } => {
                        let mut chair = SavedChair::person(key, name);
                        // Whether they were at the table at this moment. Every
                        // move writes the file, so the last write is the end of
                        // the game and this is who was there for it, which is
                        // the one thing the rating needs that the moves cannot
                        // say. A person who steps out and comes back is written
                        // gone and then written back, and only the last write
                        // is read.
                        chair.left = !t.present(seat as u8);
                        chair
                    }
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

/// One thing somebody said at a table.
///
/// Deliberately not on the `Session`, and this is the one place in this file
/// where where a field lives is the point rather than a matter of tidiness.
///
/// §9.7.1 of the scoping document: free text from a player must never reach a
/// bot's input, because "give me all your wood" is a negotiation to a person and
/// an instruction to a model. Today's bots are heuristics that take a `&State`
/// and could not read this if it were handed to them; the guarantee is that they
/// are never in a position to. Chat lives on the table, the table is the
/// server's, and nothing that decides a move can see it. Keep it that way when
/// the bots learn to talk.
#[derive(Clone, Debug)]
struct Said {
    seat: u8,
    /// Copied rather than looked up, so a line keeps the name it was said under
    /// even after that seat changes hands.
    name: String,
    text: String,
}

/// How much of a table's talk is kept.
///
/// In memory only, and only this much of it. A conversation is not part of the
/// game: the record is the moves, and a game replayed from its file is the same
/// game whatever was said over it. What that costs is that a restart loses the
/// talk, which is the right thing to lose.
const TALK_KEPT: usize = 200;

/// The longest thing anybody can say at once.
const TALK_LIMIT: usize = 240;

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
    /// Whether the people here may talk to each other, as the lobby said.
    chat: bool,
    /// Bumped whenever anything a page draws has changed.
    ///
    /// The version counts moves and deliberately does not count anything else:
    /// it is what makes a click against a stale board refuse, and a remark is
    /// not a reason to refuse somebody's move. But a held request has to wake for
    /// every kind of change, not only for moves, so this counts all of them:
    /// somebody sitting down, standing up, saying they are ready, saying
    /// anything at all, the host changing a setting, the room closing.
    ///
    /// One counter rather than a signature of the state, because the question is
    /// only ever "is this different from what that page last saw".
    pulse: u64,
    /// Who has said they are ready, in seat order.
    ///
    /// Only meaningful while the table is a room. A room starts when everybody
    /// sitting at it has said so, which is a rule the people in the room can
    /// satisfy between them: the alternative was one person's button, and a host
    /// who closed their tab left everybody else sitting in a room nothing could
    /// start.
    ///
    /// Beside the chairs rather than inside them, like `seen`, because it is not
    /// part of who is in the seat and is never written down: a game that has
    /// begun has no use for it.
    ready: Vec<bool>,
    /// Whose keys the host has taken a seat back from.
    ///
    /// Without it, removing somebody does nothing at all: their page asks for
    /// the state a hundred milliseconds later, the room still has the chair
    /// free, and the state route seats whoever asks. That auto-seating is the
    /// whole of how an invitation works, so the room has to remember the one
    /// case where somebody asking is not somebody arriving.
    ///
    /// Keys rather than seats, because the point is the person and the seats
    /// move under them at the draw. Never written down: a room is not, and by
    /// the time there is a game to write, this has done its work.
    removed: Vec<String>,
    /// Whether this table was dealt as a room: somewhere people gather before
    /// the game begins, rather than a board that starts the moment it exists.
    ///
    /// True for every table the lobby deals. It began as true only when the
    /// host held a chair, which made the hold the price of being joinable: a
    /// solo table started on the next poll, so its bots' seats were takeable in
    /// theory and never in practice. Now the window an invitation needs is
    /// simply there, on every table, until its people say they are ready.
    /// False for what does not gather anybody: demo games the server plays
    /// against itself, and games taken back up off disk, which began long ago.
    lobby: bool,
    /// Whether the host has closed the composition.
    ///
    /// A bot's chair is takeable until the game begins, which is what lets
    /// somebody invited turn up after the table was dealt. The host needs a way
    /// to say that is enough, and it is the same button that gives the held
    /// chairs to the bots: **Start with bots** would not mean much if a stranger
    /// could still walk into one of them a moment later.
    ///
    /// Not written down either. A table is only ever filed once it has begun,
    /// and a game that has begun is shut by the moves.
    shut: bool,
    /// Whether the turn order has been drawn (§18).
    ///
    /// Once, and only once. The draw happens when the composition settles, and
    /// "settled" stayed true for as long as nothing had been played: with a bot's
    /// seat now takeable, a second and third arrival would each have re-drawn the
    /// order, moving people who had already seen where they were sitting.
    ///
    /// Not written down, because it is not a fact about the game: the file
    /// records which chair each person ended up in, which is the draw's result
    /// and all a replay needs.
    drawn: bool,
    /// What has been said at this table, oldest first.
    said: Vec<Said>,
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
    /// The seat an arriving person gets, if there is one.
    ///
    /// A chair held for somebody first, because that is one the host asked to
    /// keep and the table is already waiting on it. Then a bot's, because a bot
    /// is what a seat holds when nobody better has turned up: somebody walking
    /// in before the game starts is better, and asking a host to have predicted
    /// exactly how many friends would come is asking them to guess.
    ///
    /// Nothing when every seat is a person's, which is the whole of "the table
    /// is full": there is nobody to displace and the arrival watches.
    fn free_seat(&self) -> Option<u8> {
        if self.takeable() == 0 {
            return None;
        }
        self.chairs
            .iter()
            .position(|c| *c == Chair::Open)
            .or_else(|| self.chairs.iter().position(|c| *c == Chair::Bot))
            .map(|i| i as u8)
    }

    /// Seats the table is holding empty, which is what stops it starting.
    ///
    /// Only the chairs somebody asked to keep. A bot's seat is not waiting for
    /// anybody: it is being played, and a table of bots starts at once, which is
    /// what a solo game is.
    fn waiting(&self) -> usize {
        self.chairs.iter().filter(|c| **c == Chair::Open).count()
    }

    /// Note that something a page draws has changed.
    ///
    /// Paired with `stirred` everywhere: one says the table is alive so the
    /// sweep leaves it be, the other says a held request should answer. They are
    /// different questions and a request that only looked can bump one without
    /// the other.
    fn moved(&mut self) {
        self.pulse = self.pulse.wrapping_add(1);
        self.stirred = now();
    }

    /// What a page has seen, as one number.
    ///
    /// The version and the pulse together, because a move bumps the version
    /// through the session without going near the table.
    fn seen_mark(&self) -> u64 {
        self.session
            .version()
            .wrapping_mul(1_000_003)
            .wrapping_add(self.pulse)
    }

    /// The seats a person is sitting in, in seat order.
    fn people_seated(&self) -> Vec<u8> {
        self.chairs
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c, Chair::Taken { .. }))
            .map(|(i, _)| i as u8)
            .collect()
    }

    /// How many people are at this table, and how many have said they are ready.
    ///
    /// The pair the room shows. Two numbers rather than a flag, because the
    /// question somebody in a room actually has is "who are we still waiting
    /// for", and a button that only says whether *you* have pressed it does not
    /// answer it.
    fn ready_count(&self) -> (usize, usize) {
        let seated = self.people_seated();
        let said = seated
            .iter()
            .filter(|&&s| self.ready.get(s as usize).copied().unwrap_or(false))
            .count();
        (said, seated.len())
    }

    /// Whether everybody sitting at this table has said they are ready.
    ///
    /// Nobody seated is not everybody ready: a room with no people in it is a
    /// room waiting for its first, not a room about to start.
    fn all_ready(&self) -> bool {
        let (said, of) = self.ready_count();
        of > 0 && said == of
    }

    /// Whether this table is still a room rather than a game.
    ///
    /// Nothing is played while this holds: not by a person, because `api/act`
    /// refuses, and not by the bots, because the poll does not tick it. That is
    /// the lobby phase, and it ends when the host starts the game.
    fn in_lobby(&self) -> bool {
        self.lobby && !self.shut && !self.session.started()
    }

    /// Seats somebody arriving now could take.
    ///
    /// Every chair that is not a person's, while the game has not begun. The
    /// held ones and the bots' both, because both are seats a person gets, and
    /// this is the number the home page advertises and the invitation is for.
    /// Kept apart from `waiting` because they answer different questions and the
    /// answers differ: a table with one held chair and two bots is waiting for
    /// one person and has room for three.
    fn takeable(&self) -> usize {
        if self.shut || self.session.started() || self.session.winner().is_some() {
            return 0;
        }
        self.chairs
            .iter()
            .filter(|c| !matches!(c, Chair::Taken { .. }))
            .count()
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

    /// Everything one reader is sent, from one place.
    ///
    /// Every route that answers about a table goes through here, which is the
    /// point of it. The routes used to render straight off the session, which
    /// is a view with the table's half of the answer missing: no chat, nothing
    /// said, no chairs going. So a poll showed the conversation and the reply
    /// to a move wiped it, and the panel announced that the table had been
    /// dealt without chat until the next poll three seconds later put it back.
    ///
    /// The session cannot supply any of it. Who is waiting, who may start, and
    /// what was said are the table's, and §9.7.1 is why the last one stays
    /// there: free text from a player must never reach a bot's input, so it is
    /// carried to the page beside the game rather than through it.
    fn seen_by(&self, seat: Option<u8>, player: &str, note: Option<&str>) -> String {
        // Said wherever they look, rather than only in answer to the request
        // that did it. Somebody whose seat was taken back finds themselves
        // watching a room they were sitting in, and a screen that changes under
        // you without saying why is the worst version of this.
        let taken =
            seat.is_none() && !player.is_empty() && self.removed.iter().any(|k| k == player);
        let note = match note {
            Some(n) => Some(n),
            None if taken => Some("The host took your seat back. You can still watch."),
            None => None,
        };
        let room = view::Room {
            takeable: self.takeable(),
            lobby: self.in_lobby(),
            host: !player.is_empty() && self.by == player,
            chat_setting: self.chat,
            you_ready: self
                .seat_of(player)
                .and_then(|s| self.ready.get(s as usize).copied())
                .unwrap_or(false),
            ready: self.ready_count().0,
            of: self.ready_count().1,
            held: bits(self.chairs.iter().map(|c| *c == Chair::Open)),
            ready_seats: bits(self.ready.iter().copied()),
            // A room always talks, whatever the table was dealt with. See `say`.
            chat_open: self.chat || self.in_lobby(),
            mark: self.seen_mark(),
        };
        let talk: Vec<view::Talk<'_>> = self
            .said
            .iter()
            .map(|d| view::Talk {
                seat: d.seat,
                name: &d.name,
                text: &d.text,
            })
            .collect();
        view::render_all(&self.session, seat, room, &talk, note)
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

    // `may_start` lived here: whoever dealt the table, or whoever was in seat
    // nought if they had gone. It decided who could give the empty chairs to the
    // bots and begin, and it is gone because that was one person's button. A host
    // who closed their tab left everybody else in a room nothing could start, and
    // the fallback to seat nought only helped when a person happened to be in it.
    // The room ends when the room agrees now, which is a condition the people in
    // it can always satisfy between them.

    /// Deal the seats out again, once it is settled who is at the table.
    ///
    /// Turn order is seat order, and the seats were handed out in the order
    /// people arrived: the host at nought and therefore always first, then
    /// whoever joined next. Going first is worth something, so giving it to
    /// whoever pressed the button is a thumb on the scale in every game this
    /// server deals.
    ///
    /// Shuffled once, when the composition is known and before anything has
    /// happened, so nobody is moved after they have played. The chairs carry
    /// their people and their names with them and the session is told again who
    /// is where; the board is untouched, because the board is the seed's and has
    /// nothing to do with who sits where.
    ///
    /// Not from the game's own generator. That one is the board and the dice,
    /// and reaching into it here would mean the same seed dealt a different game
    /// depending on how many people happened to turn up.
    fn shuffle(&mut self) {
        self.drawn = true;
        let n = self.chairs.len();
        // Where each seat's occupant ends up, so anything already keyed to a
        // seat can follow them. Only what was said needs it today, and it needs
        // it badly: a remark made in the lobby is stamped with the seat its
        // speaker was in at the time, and the draw is precisely the moment those
        // stop being the seats they are in.
        let mut moved: Vec<u8> = (0..n as u8).collect();
        for i in (1..n).rev() {
            let j = (roll_below((i + 1) as u64)) as usize;
            self.chairs.swap(i, j);
            self.seen.swap(i, j);
            moved.swap(i, j);
            // Everything held per seat moves with the seat. Nothing reads the
            // ready flags after the draw, since the draw is what closing the
            // room causes, but leaving them behind would make two records of who
            // is where disagree, which is how the names came to be wrong before.
            self.ready.swap(i, j);
        }
        // `moved[new] = old`, so this inverts it into "the seat this speaker is
        // in now" and re-stamps every line that is already on the table.
        let mut now_at = vec![0u8; n];
        for (to, &from) in moved.iter().enumerate() {
            now_at[from as usize] = to as u8;
        }
        for said in self.said.iter_mut() {
            if let Some(&to) = now_at.get(said.seat as usize) {
                said.seat = to;
            }
        }
        self.seat_the_people();
        // Names follow their people. Cleared first, or a seat somebody moved out
        // of keeps the name of whoever was in it.
        for seat in 0..n as u8 {
            self.session.name_seat(seat, "");
        }
        self.name_the_seats();
    }

    /// Whether the table is settled: every chair has somebody or something in it
    /// and nothing has been played yet.
    fn settling(&self) -> bool {
        !self.in_lobby() && self.waiting() == 0 && !self.session.started() && !self.drawn
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

    /// Give up the chairs of people who are not there any more.
    ///
    /// Only in a room, and it does exactly what the leave button does on
    /// purpose: a chair whose person has not been heard from in two minutes goes
    /// back to being a bot's, and whatever they last said about being ready goes
    /// with it. Returns whether anything changed, so the caller can wake the
    /// other screens and check whether the room can now start.
    ///
    /// Without this a closed tab stops a room dead. Every seated person counts
    /// towards the ready check, so one who has gone is one the room can never
    /// have, and everybody else is left pressing a button that cannot be enough.
    /// The only end such a room had was the twenty minute sweep, which closes it
    /// under the people still sitting in it rather than letting them play.
    ///
    /// Mid-game the rule is the opposite and stays so: there the seat is theirs,
    /// the house bot plays it, and they can come back to a game in progress. A
    /// room has nothing to come back to, and holding a chair in one costs
    /// somebody else the game.
    fn let_go_of_the_gone(&mut self) -> bool {
        if !self.in_lobby() {
            return false;
        }
        let gone: Vec<u8> = self
            .people_seated()
            .into_iter()
            .filter(|&s| !self.present(s))
            .collect();
        for seat in &gone {
            let name = match &self.chairs[*seat as usize] {
                Chair::Taken { name, .. } => name.clone(),
                _ => String::new(),
            };
            self.chairs[*seat as usize] = Chair::Bot;
            self.session.name_seat(*seat, "");
            if let Some(r) = self.ready.get_mut(*seat as usize) {
                *r = false;
            }
            self.session.note_to_table(if name.is_empty() {
                format!(
                    "Player {} is no longer here, and the seat is free again",
                    seat + 1
                )
            } else {
                format!("{name} is no longer here, and the seat is free again")
            });
        }
        if gone.is_empty() {
            return false;
        }
        self.seat_the_people();
        self.moved();
        true
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
    /// What each player key last called themselves.
    ///
    /// A name is a fact about a person and not about a table, so remembering it
    /// here means somebody sitting down at their second game is already called
    /// what they were called at their first, without typing it again. The page
    /// kept one in `localStorage`, which is the same browser and not the same
    /// knowledge: the server could not put a name on a seat it had never been
    /// told, so a host who never committed the field showed as "Player 1" to
    /// everybody else while their own screen showed the name.
    ///
    /// Keyed by the cookie, so it is exactly as good as the cookie: one browser,
    /// no claim about who anybody is. When there are accounts the name comes
    /// from one and this becomes the fallback for whoever has not signed in.
    /// Not written down, because a name is cheap to say again and a file of
    /// them is a file of personal data this does not need to keep.
    names: Mutex<std::collections::HashMap<String, String>>,
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
            names: Mutex::new(std::collections::HashMap::new()),
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
                takeable: t.takeable(),
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
        let chairs = chairs_from(query, seats, player, &name);
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
            chairs: chairs.clone(),
            // Anything but an explicit "text" is a table that does not talk:
            // a missing or misspelled setting should leave people quiet rather
            // than open a channel nobody asked for.
            chat: param(query, "chat").as_deref() == Some("text"),
            said: Vec::new(),
            seen: vec![now(); seats as usize],
            stirred: now(),
            pulse: 0,
            drawn: false,
            shut: false,
            // Every dealt table is a room. It used to be only tables with a
            // chair held, and a solo table started the moment it was dealt,
            // which made its bots' seats joinable in theory and never in
            // practice: the game was under way before a friend could open the
            // link. The cost is one press for solo play, the ready button, and
            // it buys every table the same first screen.
            lobby: true,
            removed: Vec::new(),
            ready: vec![false; seats as usize],
        };
        table.seat_the_people();
        // Names come from the chairs and only from the chairs. The session keeps
        // a legacy default for seat nought, "you", and while dealing always
        // shuffled, the shuffle's own name-sync cleared it; a room does not
        // shuffle until it closes, and without this every guest read the host's
        // seat as literally "you".
        for seat in 0..seats {
            table.session.name_seat(seat, "");
        }
        table.name_the_seats();
        // A table that is not a room settles the moment it is dealt, so this is
        // where its order would be decided. Every dealt table is a room now, so
        // this is dormant here and the draw happens when the room closes; kept
        // because `deal` should not have to know that.
        if table.settling() {
            table.shuffle();
        }
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
        // Anything nobody has been near for twenty minutes, which is two
        // different mercies depending on what it is.
        //
        // A room is closed: nothing was played at it, there is nothing on disk
        // and nothing to come back to. It began as "a room with a chair still
        // empty", which left a room whose chairs had all filled and whose people
        // had then all closed their tabs sitting here for ever.
        //
        // A game is only taken off the table, and that is not the same as being
        // closed: every move writes the file, so asking for it again puts it
        // back on a table exactly as it was. What this frees is the memory, and
        // what it costs is the conversation, which is not part of the game and
        // is lost on a restart anyway.
        //
        // A game with no moves in it has no file to come back from. There should
        // be none: a table is written the moment anything is played at it, and
        // one with nothing played is a room. The check is here because "it is
        // safe to drop this" is the actual condition, and saying so is cheaper
        // than trusting the two rules elsewhere that make it true.
        self.tables
            .lock()
            .unwrap()
            .retain(|t| t.stirred > cutoff || (!t.in_lobby() && t.session.moves().is_empty()));
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
                chat: false,
                said: Vec::new(),
                seen: vec![0; self.seats as usize],
                stirred: now(),
                // Played out before it got here, so there is nothing to draw.
                pulse: 0,
                drawn: true,
                shut: true,
                lobby: false,
                removed: Vec::new(),
                ready: vec![false; self.seats as usize],
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
            chat: saved.setup.chat,
            // A restart loses the talk. The moves are the record; a
            // conversation is not part of the game.
            said: Vec::new(),
            seen: vec![0; saved.setup.chairs.len().max(saved.seats as usize)],
            // Taken up now, whenever it was dealt: what the waiting limit asks
            // is how long since anybody looked, and somebody just did.
            stirred: now(),
            // The chairs coming off the file are the draw's result, so the draw
            // has happened whether or not this process is the one that did it.
            // Drawing again on resume would move people between the game they
            // left and the game they came back to.
            pulse: 0,
            drawn: true,
            // Off disk is a game that has begun, and a game that has begun is
            // shut by its own moves.
            shut: true,
            lobby: false,
            removed: Vec::new(),
            ready: vec![false; saved.setup.chairs.len().max(saved.seats as usize)],
        };
        table.seat_the_people();
        table.name_the_seats();
        self.add(table);
        true
    }

    /// Put this visitor in a seat at this table, if they are not in one already.
    ///
    /// The whole of joining. A seat that is not a person's is a seat a person
    /// can have, until the game starts: a chair the host held open first, and a
    /// bot's after that. From then on the seat is theirs and the bots stop
    /// playing it.
    ///
    /// Taking a bot's chair is the part that makes an invitation work. Holding
    /// seats open asks the host to have predicted how many friends would turn
    /// up, and to have been right: one held chair and two friends meant one of
    /// them stood outside a table with two bots in it. A bot is what a seat
    /// holds when nobody better has arrived, and somebody arriving before the
    /// first move is better.
    ///
    /// Nothing happens when every seat is a person's. There is nobody to
    /// displace, so the arrival watches, and the page says so rather than
    /// pretending to seat them.
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
            // Sitting again under a new name is how a name is changed: one path
            // for "this is me, called this", whether it is the first time or a
            // correction.
            let name = called(name);
            if !name.is_empty()
                && let Chair::Taken { name: had, .. } = &mut table.chairs[seat as usize]
                && *had != name
            {
                *had = name.clone();
                table.session.name_seat(seat, &name);
                drop(tables);
                self.names.lock().unwrap().insert(player.to_string(), name);
            }
            return Some(seat);
        }
        if player.is_empty() || table.session.started() || table.session.winner().is_some() {
            return None;
        }
        // Somebody the host took a seat back from. They may still watch, which
        // is what anybody without a seat may do; what they may not do is sit
        // straight back down, which is what would otherwise happen on their very
        // next poll and would make the host's control do nothing.
        if table.removed.iter().any(|k| k == player) {
            return None;
        }
        let seat = table.free_seat()?;
        // What they said, or what they said last time. Somebody arriving through
        // a link has typed nothing yet, and a seat labelled "Player 2" when the
        // server already knows they are Vidal is the application forgetting on
        // purpose.
        let name = match called(name) {
            said if !said.is_empty() => said,
            _ => self
                .names
                .lock()
                .unwrap()
                .get(player)
                .cloned()
                .unwrap_or_default(),
        };
        table.chairs[seat as usize] = Chair::Taken {
            key: player.to_string(),
            name: name.clone(),
        };
        // Fresh in the chair is not ready in it, whoever sat there before.
        if let Some(r) = table.ready.get_mut(seat as usize) {
            *r = false;
        }
        table.session.name_seat(seat, &name);
        table.seat_the_people();
        table.session.note_to_table(if name.is_empty() {
            format!("Player {} sat down", seat + 1)
        } else {
            format!("{name} sat down")
        });
        if !name.is_empty() {
            self.names
                .lock()
                .unwrap()
                .insert(player.to_string(), name.clone());
        }
        // The last chair taken settles the table, and settling it is when the
        // order is drawn. Their own seat may move under them here, which is why
        // this returns the seat rather than the caller assuming one.
        if table.settling() {
            table.shuffle();
        }
        let seat = table.seat_of(player).unwrap_or(seat);
        table.moved();
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

    /// Say something at a table.
    ///
    /// Only from a seat: watching a game is standing behind the players, and
    /// this is the players talking. Refused when the table was dealt without
    /// chat, so the setting means something rather than decorating the lobby.
    ///
    /// The text is trimmed, bounded and otherwise kept exactly as typed. It is
    /// escaped once, where it is written into JSON, and put into the page as
    /// text rather than as markup; nothing here tries to be clever about its
    /// contents, because a filter that half understands somebody else's words is
    /// worse than one that does not try.
    fn say(&self, id: &str, player: &str, text: &str) -> bool {
        let text: String = text.trim().chars().take(TALK_LIMIT).collect();
        if text.is_empty() {
            return false;
        }
        let mut tables = self.tables.lock().unwrap();
        let Some(table) = tables.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        // A room always talks, whatever the table was dealt with. Gathering
        // people is a conversation by nature: "two minutes", "start without me",
        // "who else is coming". The chat setting is about the *game*, and a room
        // is not the game yet, so a table dealt in silence is still somewhere its
        // people can arrange themselves before the first move.
        if !table.chat && !table.in_lobby() {
            return false;
        }
        let Some(seat) = table.seat_of(player) else {
            return false;
        };
        let name = match &table.chairs[seat as usize] {
            Chair::Taken { name, .. } => name.clone(),
            _ => String::new(),
        };
        table.said.push(Said { seat, name, text });
        table.moved();
        // Oldest out. A table talks for an hour and a page should not be handed
        // all of it on every poll.
        let over = table.said.len().saturating_sub(TALK_KEPT);
        table.said.drain(..over);
        true
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
            // Back to a bot, which is what every unclaimed seat is: it plays
            // once the game starts and belongs to whoever arrives before then.
            table.chairs[seat as usize] = Chair::Bot;
            table.session.name_seat(seat, "");
            // And they are not ready any more, whatever they said before they
            // went: a ready flag left behind would be inherited by the next
            // person to take the chair, who never pressed anything.
            if let Some(r) = table.ready.get_mut(seat as usize) {
                *r = false;
            }
            table
                .session
                .note_to_table(format!("{name} left, and the seat is free again"));
        }
        table.seat_the_people();
        table.moved();
        // Leaving can be the thing everybody else was waiting for. Two ready and
        // one not, and the one who was not walks out: the two who pressed the
        // button have nothing left to press, so the room would stand agreed and
        // unstartable for ever.
        let agreed = table.in_lobby() && table.all_ready();
        let started = table.session.started();
        let saved = saved_of(table);
        drop(tables);
        if agreed {
            self.begin(id);
        }
        if started {
            let _ = self.store.save(&saved);
        }
        true
    }

    /// Say that this seat is ready, and start the game once every seat has.
    ///
    /// The room's own way out, and it belongs to the people in it rather than to
    /// one of them. It was the host's button: whoever dealt the table, or
    /// whoever was in seat nought if they had gone. That left a room nothing
    /// could start whenever the host closed their tab and a bot held seat
    /// nought, with three people sitting in it and no way forward but to deal
    /// another table.
    ///
    /// Everybody ready is the condition, so the room ends when the room agrees.
    /// Pressing it again takes it back, because somebody who said yes and then
    /// noticed a fourth friend arriving should be able to wait for them.
    ///
    /// Whatever is still empty when the last person says yes goes to the house
    /// bot, which is what the host's button used to do and is still the only
    /// sensible reading: a chair nobody took by the time everybody was ready is
    /// a chair nobody is coming to.
    /// Change a room's settings, which is the host's alone to do.
    ///
    /// The table is dealt again from the new description: the session is the
    /// board and the clock, and both may have changed, so a fresh one is the
    /// truth and a patched one is a bug factory. What survives is everything
    /// that belongs to the people rather than the game: the chairs, their
    /// names, what has been said, and the table's identity. What does not is
    /// the ready marks, because what everybody agreed to is not what the table
    /// is any more.
    ///
    /// Refused when shrinking would unseat somebody: a person is never moved by
    /// a settings change, so four seats cannot become three while a person is
    /// sitting in the fourth.
    fn setup(&self, id: &str, player: &str, query: &str) -> Result<(), &'static str> {
        let mut tables = self.tables.lock().unwrap();
        let Some(table) = tables.iter_mut().find(|t| t.id == id) else {
            return Err("that game is over");
        };
        if player.is_empty() || table.by != player {
            return Err("only the host changes the table");
        }
        if !table.in_lobby() {
            return Err("the game has started");
        }
        let seats: u8 = param(query, "seats")
            .and_then(|v| v.parse().ok())
            .unwrap_or(4)
            .clamp(3, 4);
        if table
            .chairs
            .iter()
            .enumerate()
            .any(|(i, c)| i >= seats as usize && matches!(c, Chair::Taken { .. }))
        {
            return Err("somebody is sitting in that seat");
        }
        let seed = param(query, "seed")
            .and_then(|v| crate::game::parse_seed(&decode(&v)))
            .unwrap_or_else(|| table.session.seed());
        let secs: u64 = param(query, "clockSecs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let increment: u64 = param(query, "clockInc")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let clock = Clock::parse(param(query, "clock").as_deref(), secs, increment);
        let discard_secs: u64 = param(query, "discardSecs")
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_DISCARD_SECS);
        let named = param(query, "game").unwrap_or_default();
        table.session = Session::new(seats, seed, TradeMode::Full)
            .with_clock(clock)
            .with_log(param(query, "log").as_deref() != Some("off"))
            .with_public(wants_public(query))
            .with_game(&decode(&named))
            .with_pace(Pace::parse(param(query, "pace").as_deref()))
            .with_bank_exact(param(query, "bank").as_deref() != Some("rough"))
            .with_discard_secs(discard_secs);
        table.chat = param(query, "chat").as_deref() == Some("text");
        // The people keep their chairs; the counts around them follow the new
        // width of the table.
        table.chairs.resize(seats as usize, Chair::Bot);
        table.seen.resize(seats as usize, now());
        table.ready = vec![false; seats as usize];
        table.seat_the_people();
        for seat in 0..seats {
            table.session.name_seat(seat, "");
        }
        table.name_the_seats();
        table.moved();
        Ok(())
    }

    /// Take a seat back from somebody, which is the host's alone to do.
    ///
    /// The room's exit belongs to the room and its composition belongs to the
    /// host, and those are different powers. Unanimous ready is the right rule
    /// for "are we all here", and it has one failure the presence rule cannot
    /// reach: somebody sits down, leaves the tab open, and never presses
    /// anything. They are present, so nothing frees their chair, and everybody
    /// else waits on a person who is not coming back to the screen.
    ///
    /// Only in a room. A game under way owes every seat an opponent rather than
    /// a gap, and a host who could unseat a player mid-game could hand
    /// themselves a table of bots at the first sign of losing.
    ///
    /// Never their own seat: that is Leave, which already does the right thing
    /// and does not need a second way in.
    fn unseat(&self, id: &str, player: &str, seat: u8) -> Result<(), &'static str> {
        let mut tables = self.tables.lock().unwrap();
        let Some(table) = tables.iter_mut().find(|t| t.id == id) else {
            return Err("that game is over");
        };
        if player.is_empty() || table.by != player {
            return Err("only the host changes the table");
        }
        if !table.in_lobby() {
            return Err("the game has started");
        }
        if table.seat_of(player) == Some(seat) {
            return Err("that is your own seat");
        }
        let Some(Chair::Taken { key, name }) = table.chairs.get(seat as usize).cloned() else {
            return Err("nobody is in that seat");
        };
        // Remembered before the chair is freed, or this does nothing at all:
        // their page asks for the state a hundred milliseconds later, the room
        // has a chair free, and the state route seats whoever asks. That
        // auto-seating is the whole of how an invitation works, so the room has
        // to remember the one case where asking is not arriving.
        if !table.removed.contains(&key) {
            table.removed.push(key);
        }
        table.chairs[seat as usize] = Chair::Bot;
        table.session.name_seat(seat, "");
        if let Some(r) = table.ready.get_mut(seat as usize) {
            *r = false;
        }
        table.session.note_to_table(if name.is_empty() {
            format!("Player {} was taken off the table", seat + 1)
        } else {
            format!("{name} was taken off the table")
        });
        table.seat_the_people();
        table.moved();
        // Which can be the thing everybody else was waiting for, exactly as
        // walking out and going quiet already are.
        let agreed = table.all_ready();
        drop(tables);
        if agreed {
            self.begin(id);
        }
        Ok(())
    }

    fn ready(&self, id: &str, player: &str) -> bool {
        let mut tables = self.tables.lock().unwrap();
        let Some(table) = tables.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        // Only from a seat, and only while there is a room to be ready in.
        let Some(seat) = table.seat_of(player) else {
            return false;
        };
        if !table.in_lobby() {
            return false;
        }
        let now_ready = !table.ready.get(seat as usize).copied().unwrap_or(false);
        if let Some(r) = table.ready.get_mut(seat as usize) {
            *r = now_ready;
        }
        let name = table.session.name_of(seat).to_string();
        let who = if name.is_empty() {
            format!("Player {}", seat + 1)
        } else {
            name
        };
        table.session.note_to_table(if now_ready {
            format!("{who} is ready")
        } else {
            format!("{who} is not ready yet")
        });
        table.moved();
        if !table.all_ready() {
            return true;
        }
        drop(tables);
        self.begin(id);
        true
    }

    /// Close the room: the empty chairs go to the bots and the table may move.
    ///
    /// Called when everybody says they are ready. Separate from `ready` so the
    /// two halves are readable apart: one is a person pressing a button, the
    /// other is what happens to a table when the last of them does.
    fn begin(&self, id: &str) -> bool {
        let mut tables = self.tables.lock().unwrap();
        let Some(table) = tables.iter_mut().find(|t| t.id == id) else {
            return false;
        };
        if table.session.started() {
            return false;
        }
        // The table is who it is now. This closes the door on its own, because a
        // bot's chair is otherwise takeable until the first move and a started
        // game would not mean much if somebody could walk into one a moment
        // later.
        table.shut = true;
        let short = table.waiting();
        for c in table.chairs.iter_mut() {
            if *c == Chair::Open {
                *c = Chair::Bot;
            }
        }
        table.seat_the_people();
        // The draw waits for this, not for the chairs filling. People can arrive
        // right up to the moment the room closes, so drawing when the last held
        // chair went would have settled the order before the table knew who was
        // at it.
        if table.settling() {
            table.shuffle();
        }
        if short > 0 {
            table.session.note_to_table(if short == 1 {
                "The last seat went to the house bot".to_string()
            } else {
                format!("{short} seats went to the house bot")
            });
        }
        table.moved();
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

    /// How long a connection may take to send its request.
    ///
    /// A client that opens a socket and says nothing used to hold the server for
    /// ever, which on one thread meant everybody. It is trivial to do by
    /// accident on a bad network and trivial to do on purpose, so the socket
    /// gives up on a request that has not arrived in ten seconds.
    ///
    /// Generous on purpose: this bounds how long a *request* may dribble in, not
    /// how long the answer may take, and a phone on a train is slow rather than
    /// hostile.
    const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

    /// Connections served at once.
    ///
    /// A thread apiece, which is the shape this server wants: a held state
    /// request sleeps for most of its life and a move is microseconds of work,
    /// so threads here are mostly waiting rather than running. The cap is what
    /// stops an unbounded number of them, and it is high because each one is
    /// cheap and a table of four is four of them.
    const MAX_CONNECTIONS: usize = 512;

    /// Serve until the process is stopped.
    ///
    /// A thread per connection. It was one at a time, on the grounds that there
    /// was one game and one player, and both halves of that stopped being true:
    /// with four people at a table and a held request each, serialising them
    /// means three people wait on the fourth's network. One slow client blocked
    /// everybody, which is a denial of service anybody could commit by accident.
    pub fn serve(&'static self, listener: TcpListener) {
        let live = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for stream in listener.incoming() {
            let s = match stream {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("accept: {e}");
                    continue;
                }
            };
            // Counted rather than pooled: a pool would queue behind busy workers,
            // and what this needs is to refuse rather than to queue. Over the cap
            // the connection is dropped, which is the honest answer and is what a
            // proxy in front will report as a failure rather than a hang.
            use std::sync::atomic::Ordering;
            if live.load(Ordering::Relaxed) >= Self::MAX_CONNECTIONS {
                eprintln!("refused: {} connections already", Self::MAX_CONNECTIONS);
                continue;
            }
            live.fetch_add(1, Ordering::Relaxed);
            let mine = live.clone();
            let spawned = std::thread::Builder::new()
                .name("carranta-conn".to_string())
                // Small on purpose: this stack holds a request and a response,
                // not a recursion. The default eight megabytes times five
                // hundred connections is address space for nothing.
                .stack_size(256 * 1024)
                .spawn(move || {
                    let _ = s.set_read_timeout(Some(Self::READ_TIMEOUT));
                    let _ = s.set_write_timeout(Some(Self::READ_TIMEOUT));
                    if let Err(e) = self.handle(s) {
                        eprintln!("connection: {e}");
                    }
                    mine.fetch_sub(1, Ordering::Relaxed);
                });
            if spawned.is_err() {
                live.fetch_sub(1, Ordering::Relaxed);
                eprintln!("could not spawn a thread for a connection");
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
            // Opening the lobby is creating one. The screen needs an address
            // the moment it exists, because its whole point is to be shared,
            // and an address needs a table behind it: so this mints one, with
            // the settings the form starts from, and sends the visitor to it.
            // Everything after that happens at the table's own address, host
            // and guests on the same screen.
            ("GET", "/lobby") => {
                let id = self.deal("seats=4&clock=turn&clockSecs=60&discardSecs=10", &player);
                let set = if issue {
                    cookie_header(&player)
                } else {
                    String::new()
                };
                redirect_with(&mut stream, &format!("/{id}/"), &set)
            }
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
                        page_served().as_bytes(),
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
                    Some((_, bytes)) => respond_kept(&mut stream, "image/jpeg", bytes),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", p) if p.starts_with("/art/") => {
                let name = p.trim_start_matches("/art/").trim_end_matches(".svg");
                match ART.iter().find(|(n, _)| *n == name) {
                    Some((_, body)) => respond_kept(&mut stream, "image/svg+xml", body.as_bytes()),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", p) if p.starts_with("/font/") => {
                let name = p.trim_start_matches("/font/").trim_end_matches(".woff2");
                match FONTS.iter().find(|(n, _)| *n == name) {
                    Some((_, bytes)) => respond_kept(&mut stream, "font/woff2", bytes),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", p) if p.starts_with("/sound/") => {
                let name = p.trim_start_matches("/sound/").trim_end_matches(".mp3");
                match SOUNDS.iter().find(|(n, _)| *n == name) {
                    Some((_, bytes)) => respond_kept(&mut stream, "audio/mpeg", bytes),
                    None => respond(&mut stream, 404, "text/plain", b"not found"),
                }
            }
            ("GET", "/api/state") => {
                let id = game.clone().unwrap_or_default();
                // An unfinished game off a table is put back on one first, so a
                // tab left open across a restart carries on where it was.
                self.seat(&id);
                self.stir(&id);
                // And here rather than only when somebody looks at the home
                // page. The limit used to be real only if a visitor happened to
                // arrive: a server whose players all reach their games by link
                // never swept anything. This table was just stirred, so it is
                // never the one that goes.
                self.sweep();
                // Which seat is theirs, and opening a room takes one. The link
                // is an invitation and following it is answering it, so a card
                // asking whether you meant it was a question with one answer,
                // standing between somebody and the table. The name comes from
                // whatever the server remembers of them, and the seat's own row
                // on the lobby screen is where it is typed or corrected.
                //
                // Only a room can seat anybody this way: a game under way has no
                // takeable chair, so nobody is walked into one.
                let seat = self
                    .seated(&id, &player)
                    .or_else(|| self.sit(&id, &player, ""));
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
                // And that the others still are. A room is the one place where
                // somebody who has gone holds everybody else up, so their chair
                // is let go of here. After the stamp above, or a page would hand
                // back the seat it just took.
                let emptied = {
                    let mut tables = self.tables.lock().unwrap();
                    tables
                        .iter_mut()
                        .find(|t| t.id == id)
                        .is_some_and(|t| t.let_go_of_the_gone() && t.all_ready())
                };
                // Losing somebody can be the thing everybody else was waiting
                // for, exactly as walking out is: two ready and one gone quiet
                // leaves two people who have nothing left to press.
                if emptied {
                    self.begin(&id);
                }
                // What the asking page last drew, if it says. A request that
                // names one is held until the table looks different from it, so
                // a move reaches the other screens in the time it takes to send
                // it rather than at the next tick of a three second poll, and a
                // table nobody is touching costs one held socket instead of
                // twelve hundred answers an hour.
                let since: Option<u64> = param(query, "since").and_then(|v| v.parse().ok());
                let until = std::time::Instant::now() + HOLD;
                let payload = loop {
                    let mut tables = self.tables.lock().unwrap();
                    let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                        // What is left is a finished game, which is read rather
                        // than played: hand it back as it stands. Nothing ticks,
                        // because nothing is waiting.
                        drop(tables);
                        return match self.stored(&id) {
                            Some(p) => respond(&mut stream, 200, "application/json", p.as_bytes()),
                            None => respond(&mut stream, 404, "text/plain", b"no such game"),
                        };
                    };
                    // A server only wakes when asked, so this poll is the whole
                    // clock: it is what lets a paced bot's wait expire, and what
                    // ends a turn whose time ran out. Inside the hold rather
                    // than before it, because a held request is the only thing
                    // asking for as long as it is held: without this a paced
                    // bot's move would wait for the hold to expire.
                    //
                    // Not while the table is still a room. Nothing is being
                    // played and nobody's turn is running down while people are
                    // walking in, and a turn clock left running over a lobby
                    // does not merely tick: it runs out, forfeits the turn, and
                    // that forfeit is a move, so the game starts itself about a
                    // minute after it is dealt with everybody still reading the
                    // settings.
                    if !t.in_lobby() {
                        t.session.tick();
                        t.session.enforce_clock();
                    }
                    let mark = t.seen_mark();
                    // Their own seat's view, or a spectator's if they have none:
                    // nobody is ever sent another seat's hand. Either way it
                    // carries how many chairs are still going, because that is
                    // the one thing about this table you can be too late for.
                    //
                    // Read again here rather than reused from above. A request
                    // is held for up to twenty seconds and a seat can move under
                    // it in that time: the host takes it back, or the room
                    // closes and the draw shuffles everybody. Answering with the
                    // seat somebody had when they asked would hand them a view
                    // of a chair that is no longer theirs, and the whole of the
                    // redaction is keyed off this number.
                    let now_seat = t.seat_of(&player);
                    if since != Some(mark) || std::time::Instant::now() >= until {
                        break t.seen_by(now_seat, &player, None);
                    }
                    // Nothing has changed and there is time left. The lock is
                    // dropped first, because sleeping on it would stop the very
                    // moves this is waiting for.
                    drop(tables);
                    std::thread::sleep(WAKE);
                };
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
                // A table that is still a room is not a game yet, whoever is
                // sitting at it. Moving would start it under everybody who was
                // invited and has not arrived, and it would shut the door on
                // them. The way past it is `api/start`, which is the host saying
                // the table is who it is now.
                if t.in_lobby() {
                    let payload =
                        t.seen_by(Some(seat), &player, Some("the table has not started yet"));
                    drop(tables);
                    return respond(&mut stream, 200, "application/json", payload.as_bytes());
                }
                let action = json::read_u64(&body, "action");
                let version = json::read_u64(&body, "version");
                let note = match (action, version) {
                    // As their own seat, and the index is into their own list of
                    // choices: one person cannot press another's button, because
                    // the only thing they can name is something on their screen.
                    (Some(a), Some(v)) => t
                        .session
                        .act_as(seat, a as usize, v)
                        .err()
                        .map(|e| refusal(&e)),
                    _ => Some("malformed request".to_string()),
                };
                let payload = t.seen_by(Some(seat), &player, note.as_deref());
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
                let note = match json::read_u64(&body, "version") {
                    Some(v) => t.session.cancel_as(seat, v).err().map(|e| refusal(&e)),
                    None => Some("malformed request".to_string()),
                };
                let payload = t.seen_by(Some(seat), &player, note.as_deref());
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
                let give = json::read_u8_array(&body, "give", 5);
                let want = json::read_u8_array(&body, "want", 5);
                let version = json::read_u64(&body, "version");
                // Absent means the open market; a seat number addresses it.
                let to = json::read_u64(&body, "to").map(|n| n as u8);
                let note = match (give, want, version) {
                    (Some(g), Some(w), Some(v)) => {
                        let g = [g[0], g[1], g[2], g[3], g[4]];
                        let w = [w[0], w[1], w[2], w[3], w[4]];
                        t.session
                            .propose_as(seat, to, g, w, v)
                            .err()
                            .map(|e| refusal(&e))
                    }
                    _ => Some("malformed request".to_string()),
                };
                let payload = t.seen_by(Some(seat), &player, note.as_deref());
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
            // Saying something at a table.
            ("POST", "/api/say") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                if !self.say(
                    &id,
                    &player,
                    &decode(&param(&body, "text").unwrap_or_default()),
                ) {
                    return respond(&mut stream, 403, "text/plain", b"nothing to say here");
                }
                respond(&mut stream, 200, "application/json", b"{}")
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
            // The host changing the room's settings.
            ("POST", "/api/setup") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                let note = self.setup(&id, &player, &body).err();
                let seat = self.seated(&id, &player);
                let payload = {
                    let tables = self.tables.lock().unwrap();
                    match tables.iter().find(|t| t.id == id) {
                        Some(t) => t.seen_by(seat, &player, note),
                        None => String::from("{}"),
                    }
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // Saying you are ready. The last person to say it starts the game.
            ("POST", "/api/ready") => {
                let id = game.clone().unwrap_or_default();
                self.seat(&id);
                self.stir(&id);
                if !self.ready(&id, &player) {
                    return respond(&mut stream, 403, "text/plain", b"no seat of yours");
                }
                let seat = self.seated(&id, &player);
                let payload = {
                    let tables = self.tables.lock().unwrap();
                    match tables.iter().find(|t| t.id == id) {
                        Some(t) => t.seen_by(seat, &player, None),
                        None => String::from("{}"),
                    }
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // The host, not waiting any longer. Unanimous ready is the right
            // rule for "are we all here" and cannot reach one case: somebody
            // sits down, leaves the tab open, and never presses anything. They
            // are present, so nothing frees their chair, and everybody else
            // waits on a person who is not looking at the screen.
            ("POST", "/api/begin") => {
                let id = game.clone().unwrap_or_default();
                self.stir(&id);
                {
                    let tables = self.tables.lock().unwrap();
                    let Some(t) = tables.iter().find(|t| t.id == id) else {
                        return respond(&mut stream, 409, "text/plain", b"that game is over");
                    };
                    if player.is_empty() || t.by != player {
                        return respond(&mut stream, 403, "text/plain", b"only the host starts");
                    }
                    if !t.in_lobby() {
                        return respond(&mut stream, 409, "text/plain", b"already started");
                    }
                }
                self.begin(&id);
                let seat = self.seated(&id, &player);
                let payload = {
                    let tables = self.tables.lock().unwrap();
                    match tables.iter().find(|t| t.id == id) {
                        Some(t) => t.seen_by(seat, &player, None),
                        None => String::from("{}"),
                    }
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // The host, taking a seat back. The other half of the same power:
            // one says the table is who it is now, the other says who that is.
            ("POST", "/api/unseat") => {
                let id = game.clone().unwrap_or_default();
                self.stir(&id);
                let seat: u8 = match param(&body, "seat").and_then(|v| v.parse().ok()) {
                    Some(s) => s,
                    None => return respond(&mut stream, 400, "text/plain", b"which seat"),
                };
                if let Err(why) = self.unseat(&id, &player, seat) {
                    return respond(&mut stream, 403, "text/plain", why.as_bytes());
                }
                let mine = self.seated(&id, &player);
                let payload = {
                    let tables = self.tables.lock().unwrap();
                    match tables.iter().find(|t| t.id == id) {
                        Some(t) => t.seen_by(mine, &player, None),
                        None => String::from("{}"),
                    }
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

/// One bit per seat, seat nought lowest.
///
/// A table has at most four seats and the view's `Room` is a `Copy` handful of
/// scalars, so the two per-seat facts it carries travel as bits rather than as
/// two more allocations on every render.
fn bits(of: impl Iterator<Item = bool>) -> u8 {
    of.take(8)
        .enumerate()
        .fold(0u8, |m, (i, on)| if on { m | 1 << i } else { m })
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

/// A number below `n`, from the clock and the process.
///
/// Not the game's generator: that one deals the board and the dice, and drawing
/// from it to decide seating would mean the same seed produced a different game
/// depending on how many people turned up. This has one job, once per table, and
/// nothing downstream depends on it being reproducible.
fn roll_below(n: u64) -> u64 {
    if n <= 1 {
        return 0;
    }
    let mut x = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
        ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // A round of mixing, because consecutive nanosecond readings differ in their
    // low bits only and a shuffle taking them modulo four would barely move.
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    x ^= x >> 33;
    x = x.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    x ^= x >> 33;
    x % n
}

/// A name for a seat, given one somebody typed.
///
/// Trimmed and bounded, and empty when they said nothing. It used to fill a
/// blank in with "Player 2", which was a name derived from a seat number and so
/// became a lie the moment the turn order was drawn and the seat moved. What a
/// seat with no name is called is the page's to decide, from the seat it is
/// actually in, and it is the only place that knows that for certain.
fn called(name: &str) -> String {
    name.trim().chars().take(24).collect()
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
            let _word = said.next().unwrap_or("");
            // Everything that is not the dealer is a bot, whatever the query
            // says. There used to be a third word, `open`, for a chair held
            // empty; it is read and ignored now, so old links still deal. The
            // hold was how a host said they were waiting for somebody, and it
            // earned its keep by being the only thing that stopped the table
            // starting instantly; the room does that for every dealt table now,
            // and a seat that blocks the game is strictly worse than a bot's,
            // which plays until somebody better arrives.
            if i == 0 {
                Chair::Taken {
                    key: player.to_string(),
                    name: called(name),
                }
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
/// A response for something that will never change.
///
/// The art, the fonts and the sounds are compiled into the binary and are the
/// same bytes for the life of the build, so they may be held for a year by
/// anything between here and the reader. That is what lets a cache in front of
/// this server, which is where compression is coming from, do the other half of
/// its job: a returning player fetches the board's markup and nothing else.
///
/// Everything else stays `no-store`. A board is different every few seconds and
/// a cached one is a lie.
fn respond_kept(stream: &mut TcpStream, kind: &str, body: &[u8]) -> std::io::Result<()> {
    respond_with_cache(
        stream,
        200,
        kind,
        body,
        "",
        "public, max-age=31536000, immutable",
    )
}

fn respond_with(
    stream: &mut TcpStream,
    status: u16,
    kind: &str,
    body: &[u8],
    extra: &str,
) -> std::io::Result<()> {
    respond_with_cache(stream, status, kind, body, extra, "no-store")
}

fn respond_with_cache(
    stream: &mut TcpStream,
    status: u16,
    kind: &str,
    body: &[u8],
    extra: &str,
    cache: &str,
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
         Cache-Control: {cache}\r\n\
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
            chat: true,
            said: Vec::new(),
            removed: Vec::new(),
            seen: vec![now(); 4],
            stirred: now(),
            pulse: 0,
            drawn: true,
            shut: true,
            lobby: false,
            ready: Vec::new(),
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
            chat: true,
            said: Vec::new(),
            removed: Vec::new(),
            seen: vec![now(); 4],
            stirred: now(),
            pulse: 0,
            drawn: true,
            shut: true,
            lobby: false,
            ready: Vec::new(),
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
            chat: true,
            said: Vec::new(),
            removed: Vec::new(),
            seen: vec![now(); 4],
            stirred: now(),
            pulse: 0,
            drawn: true,
            shut: true,
            lobby: false,
            ready: Vec::new(),
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
        // It belongs to whoever opened it, under the key they were just handed,
        // which is the only reading that puts it on the right home page.
        let key = answer
            .lines()
            .find_map(|l| l.strip_prefix("Set-Cookie: carranta="))
            .and_then(|v| v.split(';').next())
            .expect("a key was handed out");
        assert_eq!(t.by, key, "theirs, not the sender's");
        // Not the host's name: the link carries none, so the receiver's seat is
        // named for its number rather than sitting somebody else down under the
        // name of the person who sent it.
        let seat = t.seat_of(key).expect("the receiver has a seat");
        assert_eq!(t.session.name_of(seat), "", "nobody said what to call them");
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
        // A table's description is written by one function on the page and read
        // by one method here; this pins the halves that a rename would quietly
        // split.
        const PAGE: &str = include_str!("../assets/index.html");
        // The lobby's form writes the table through one route, and the server
        // reads it with the same parser dealing uses, so the form and the deal
        // cannot mean two slightly different tables.
        assert!(
            PAGE.contains("api('/api/setup')"),
            "the form writes the table's own description"
        );
        // And the invitation is not a description at all. A description deals a
        // table, so two people opening one are at two tables: the link that
        // seats somebody names a table that already exists, which is the page
        // they are on.
        assert!(
            PAGE.contains("return location.origin + location.pathname;"),
            "the invitation is the table's own address"
        );
        assert!(
            !PAGE.contains("tableQuery"),
            "and never the settings dressed up as one"
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

        // A table dealt the way the lobby deals every one: the dealer in seat
        // nought, bots behind them, and the whole thing a room until its people
        // are ready.
        let host = "hostkey000000000";
        let dealt = get(port, "/join?seats=4&pace=instant", host);
        let went = dealt
            .lines()
            .find_map(|l| l.strip_prefix("Location: "))
            .expect("somewhere to go")
            .trim()
            .to_string();
        let id = went.trim_matches('/').to_string();

        // The host has a seat and is the only person at the table.
        let host_seat = server.seated(&id, host).expect("the host has a seat");
        let mine = get(port, &format!("/{id}/api/state"), host);
        assert!(mine.contains(&format!("\"you\":{host_seat}")));
        assert!(
            mine.contains(&format!("\"people\":[{host_seat}]")),
            "and is alone at it"
        );

        // A second person opens the table, and opening a room is taking a seat
        // in it: following an invitation is answering it, and a card asking
        // whether you meant to come in was a question with one answer.
        let guest = "guestkey00000000";
        let looking = get(port, &format!("/{id}/api/state"), guest);
        assert!(
            !looking.contains("\"you\":-1"),
            "opening a room seats you: {looking:.200}"
        );
        assert!(
            looking.contains("\"seatsTakeable\":2"),
            "and there are two chairs left behind them"
        );
        assert!(
            looking.contains("\"started\":false"),
            "and the door is open"
        );

        // A name is said by sitting again under it, which is also how one is
        // corrected: one path for "this is me, called this".
        let sat = post(port, &format!("/{id}/api/sit"), guest, "name=Vidal");
        assert!(sat.contains("\"seat\":"), "{sat:.60}");
        // Nothing has been played yet, so the two bots are two chairs somebody
        // could still walk into: sitting down settles the table and draws the
        // order, and it is a poll that plays the first move. Read off the table
        // rather than off a request, because asking is what would start it.
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert!(!t.session.started(), "nobody has moved");
            assert_eq!(t.waiting(), 0, "and nothing is being waited for");
            assert_eq!(t.takeable(), 2, "but there is room for two more");
        }

        // Both of them say they are ready, which is what closes a room. The last
        // one to say it also gives the chairs still empty to the bots and shuts
        // the door, or "ready" would not mean much with somebody able to walk
        // into one of them afterwards.
        assert!(post(port, &format!("/{id}/api/ready"), guest, "").starts_with("HTTP/1.1 200"));
        assert!(post(port, &format!("/{id}/api/ready"), host, "").starts_with("HTTP/1.1 200"));
        // Which is also when the turn order is drawn, so the seats noted above
        // are seats these two may since have been moved out of.
        let host_seat = server.seated(&id, host).expect("still seated");
        let guest_seat = server.seated(&id, guest).expect("still seated");
        assert_ne!(host_seat, guest_seat, "two people, two seats");

        let theirs = get(port, &format!("/{id}/api/state"), guest);
        assert!(theirs.contains(&format!("\"you\":{guest_seat}")));
        assert!(theirs.contains("Vidal"), "under the name they gave");
        assert!(theirs.contains("\"seatsTakeable\":0"), "and nothing going");

        // A third finds it closed and watches: a seat of nobody's, an empty
        // hand, and nothing to press.
        let watcher = "watchkey00000000";
        let looking = get(port, &format!("/{id}/api/state"), watcher);
        assert!(looking.contains("\"you\":-1"), "no seat");
        assert!(looking.contains("\"choices\":[]"), "and nothing to do");
        assert!(looking.contains("\"seatsTakeable\":0"), "and nothing going");
        let refused = post(port, &format!("/{id}/api/sit"), watcher, "name=Late");
        assert!(refused.contains("\"seat\":-1"), "and no chair to take");

        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.seat_of(host), Some(host_seat));
            assert_eq!(t.seat_of(guest), Some(guest_seat));
            assert_eq!(t.seat_of(watcher), None, "watching is not sitting");
            assert_eq!(t.waiting(), 0, "the chair is taken");
            assert_eq!(
                t.session.people(),
                {
                    let mut both = vec![host_seat, guest_seat];
                    both.sort_unstable();
                    both
                },
                "the two of them and nobody else"
            );
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
        // Whoever is to act can move, and the other cannot. Which of them it is
        // depends on the order the table drew, and it may be neither for a beat
        // while the bots take their turns, so this asks until a person is being
        // asked. Version and turn come out of the same answer, because asking is
        // also what lets the bots move and a version read before that is stale
        // by the time it is used.
        let mut mover = host;
        let mut idler = guest;
        let mut v = 0;
        for _ in 0..40 {
            let answer = get(port, &format!("/{id}/api/state"), host);
            v = answer
                .rsplit("\"version\":")
                .next()
                .and_then(|t| t.split(&[',', '}'][..]).next())
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or_default();
            if answer.contains("\"yourTurn\":true") {
                (mover, idler) = (host, guest);
                break;
            }
            let theirs = get(port, &format!("/{id}/api/state"), guest);
            if theirs.contains("\"yourTurn\":true") {
                (mover, idler) = (guest, host);
                break;
            }
        }
        let idle = play(idler, v);
        assert!(
            idle.contains("no longer offered"),
            "the seat that is not being asked cannot move"
        );
        let moved = play(mover, v);
        assert!(moved.starts_with("HTTP/1.1 200 OK"), "{moved:.80}");
        assert!(
            version(host) > v,
            "the board moved on, but: {}",
            &moved[moved.len().saturating_sub(300)..]
        );

        // Somebody with no seat cannot act at all, whatever they send.
        let watched = play(watcher, version(host));
        assert!(watched.starts_with("HTTP/1.1 403"), "{watched:.40}");

        // The seating is written down, so it survives the table being put away.
        server.keep(&id);
        let saved = server.store().load(&id).expect("written");
        let mut who: Vec<&str> = saved.setup.chairs.iter().map(|c| c.who.as_str()).collect();
        who.sort_unstable();
        assert_eq!(who, vec!["bot", "bot", guest, host], "everybody, somewhere");
        server.tables.lock().unwrap().clear();
        assert!(server.seat(&id), "taken up again");
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("back");
            assert_eq!(t.seat_of(host), Some(host_seat), "still the host's chair");
            assert_eq!(t.seat_of(guest), Some(guest_seat), "and still the guest's");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_remark_keeps_its_speaker_across_the_draw() {
        // Every line is stamped with the seat its speaker was in when they said
        // it, and the draw is exactly the moment those stop being the seats they
        // are in. Without this, closing a room re-attributed everything said in
        // it: the page reads the seat's colour and its current name off that
        // number, so lines arrived under the wrong person.
        let dir = std::env::temp_dir().join(format!("carranta-attrib-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 82, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal("seats=4&name=Marta&chat=text", host);
        assert!(server.sit(&id, guest, "Vidal").is_some());
        assert!(server.say(&id, host, "mine"));
        assert!(server.say(&id, guest, "theirs"));
        assert!(server.begin(&id), "which draws the order");

        let tables = server.tables.lock().unwrap();
        let t = tables.iter().find(|t| t.id == id).expect("dealt");
        let host_seat = t.seat_of(host).expect("seated");
        let guest_seat = t.seat_of(guest).expect("seated");
        let said: Vec<(u8, &str)> = t.said.iter().map(|d| (d.seat, d.text.as_str())).collect();
        assert_eq!(
            said,
            vec![(host_seat, "mine"), (guest_seat, "theirs")],
            "each line is still its speaker's"
        );
        drop(tables);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_idle_game_leaves_the_table_but_not_the_store() {
        // Two different mercies. A room nobody has been near is closed, because
        // nothing was played at it and there is nothing to come back to. A game
        // is only taken off the table: every move writes the file, so asking for
        // it again puts it back exactly as it was, and what the sweep frees is
        // the memory rather than the game.
        let dir = std::env::temp_dir().join(format!("carranta-idle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 92, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let room = server.deal("seats=4&name=Marta", host);
        let game = server.deal("seats=4&name=Marta&pace=instant", host);
        assert!(server.begin(&game));
        {
            let mut tables = server.tables.lock().unwrap();
            for t in tables.iter_mut() {
                if t.id == game {
                    let acting = t.session.state().decider();
                    let v = t.session.version();
                    t.session.act_as(acting, 0, v).expect("playable");
                }
                // Both last looked at half an hour ago.
                t.stirred = now().saturating_sub(30 * 60 * 1000);
            }
        }
        server.keep(&game);

        server.sweep();
        {
            let tables = server.tables.lock().unwrap();
            assert!(!tables.iter().any(|t| t.id == room), "the room is closed");
            assert!(
                !tables.iter().any(|t| t.id == game),
                "and the game is off the table"
            );
        }
        // But only the room is gone. The game is on disk and comes back the
        // moment anybody asks for it.
        assert!(
            server.store().load(&room).is_none(),
            "a room leaves no file"
        );
        assert!(server.store().load(&game).is_some(), "a game does");
        assert!(server.seat(&game), "and is taken up again");
        {
            let tables = server.tables.lock().unwrap();
            assert!(tables.iter().any(|t| t.id == game), "back on a table");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_state_request_is_held_until_the_table_looks_different() {
        // The page used to ask every three seconds and be told nothing had
        // changed almost every time: twelve hundred answers an hour per open
        // page, and a move reaching the other screens up to three seconds late.
        // A request that says what it last saw is held until that stops being
        // true, so an answer means something happened.
        let dir = std::env::temp_dir().join(format!("carranta-hold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server =
            Box::leak(Box::new(Server::new(4, 91, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        let host = "hostkey000000000";
        let id = server.deal("seats=4&name=Marta&chat=text", host);
        let mark_of = |body: &str| -> u64 {
            body.rsplit("\"mark\":")
                .next()
                .and_then(|t| t.split(&[',', '}'][..]).next())
                .and_then(|t| t.trim().parse().ok())
                .expect("a mark")
        };
        let first = get(port, &format!("/{id}/api/state"), host);
        let mark = mark_of(&first);

        // Asking again with that mark is held: nothing has happened, so the
        // request waits rather than answering the same board over again.
        let held = {
            let id = id.clone();
            std::thread::spawn(move || {
                let began = std::time::Instant::now();
                let body = get(port, &format!("/{id}/api/state?since={mark}"), host);
                (began.elapsed(), body)
            })
        };
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(
            !held.is_finished(),
            "still waiting, because nothing has changed"
        );

        // Somebody says something, which is not a move and does not touch the
        // version: the hold has to wake for it anyway.
        assert!(server.say(&id, host, "anybody there"));
        let (took, body) = held.join().expect("the held request finished");
        assert!(
            took < std::time::Duration::from_secs(5),
            "answered when the table changed, not when the hold expired: {took:?}"
        );
        assert!(body.contains("anybody there"), "and carries the change");
        assert_ne!(mark_of(&body), mark, "under a new mark");

        // And a mark nobody recognises answers at once rather than waiting,
        // which is what a page reloading after a restart sends.
        let began = std::time::Instant::now();
        let fresh = get(port, &format!("/{id}/api/state?since=999999999"), host);
        assert!(fresh.starts_with("HTTP/1.1 200 OK"));
        assert!(
            began.elapsed() < std::time::Duration::from_secs(2),
            "an unfamiliar mark is answered, not held"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_host_can_start_over_somebody_who_never_pressed_anything() {
        // Unanimous ready is the right rule for "are we all here" and has one
        // failure the presence rule cannot reach: somebody sits down, leaves the
        // tab open, and never presses anything. They are present, so nothing
        // frees their chair, and the room waits on a person who is not looking.
        let dir = std::env::temp_dir().join(format!("carranta-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 17, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let idler = "idlerkey00000000";
        let id = server.deal("seats=4&name=Marta&pace=instant", host);
        assert_eq!(server.sit(&id, guest, "Vidal"), Some(1));
        assert_eq!(server.sit(&id, idler, "Nuria"), Some(2));

        // Nobody else may do this, whatever they think of the wait.
        assert_eq!(
            server.unseat(&id, guest, 2),
            Err("only the host changes the table")
        );
        assert_eq!(
            server.unseat(&id, host, 0),
            Err("that is your own seat"),
            "leaving is its own control and does the right thing"
        );

        // Taking a seat back has to stick. Without the room remembering, the
        // removed page asks for the state a hundred milliseconds later and the
        // route seats whoever asks, so the control did nothing at all. This is
        // the case a browser caught and this file could not: nothing here polls
        // on its own.
        assert!(server.unseat(&id, host, 2).is_ok());
        assert_eq!(
            server.sit(&id, idler, "Nuria"),
            None,
            "and they cannot sit straight back down"
        );
        assert!(
            server.seated(&id, idler).is_none(),
            "so they are watching, not playing"
        );
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("still there");
            assert!(
                t.seen_by(None, idler, None).contains("took your seat back"),
                "and are told why, wherever they look"
            );
        }
        // Somebody else arriving is unaffected: the room remembers a person,
        // not a closed chair.
        let late = "latekey000000000";
        assert_eq!(server.sit(&id, late, "Bea"), Some(2), "the chair is open");

        // And taking the last seat back is the thing the other two are waiting
        // for: the room stands agreed and starts on its own.
        assert!(server.ready(&id, host));
        assert!(server.ready(&id, guest));
        assert!(server.unseat(&id, host, 2).is_ok());
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("still there");
            assert!(!t.in_lobby(), "the room closed behind them");
            assert!(t.seat_of(late).is_none(), "and the seat is a bot's");
        }
        // Mid-game the power is gone. A game owes every seat an opponent rather
        // than a gap, and a host who could unseat a player at the first sign of
        // losing would be dealing themselves a table of bots.
        assert_eq!(server.unseat(&id, host, 1), Err("the game has started"));

        // And the plain case: two people, one of them silent, and the host
        // starting without them rather than taking their seat.
        let other = server.deal("seats=4&name=Marta&pace=instant", host);
        assert_eq!(server.sit(&other, guest, "Vidal"), Some(1));
        assert!(server.ready(&other, host));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == other).expect("dealt");
            assert_eq!(t.ready_count(), (1, 2), "one of two, and stuck there");
        }
        assert!(server.begin(&other));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == other).expect("dealt");
            assert!(!t.in_lobby(), "started");
            assert!(
                t.seat_of(guest).is_some(),
                "and the one who never pressed is still playing: starting \
                 without them is not removing them"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_held_request_answers_with_the_seat_you_have_now() {
        // A request is held for up to twenty seconds, and the seat it was asked
        // from can move under it in that time: the host takes it back, or the
        // room closes and the draw shuffles everybody. The seat used to be read
        // once, before the hold, so the answer described whichever chair the
        // asker had when they asked. That is the number the whole redaction is
        // keyed off, so it is the one thing here that must not be stale.
        let dir = std::env::temp_dir().join(format!("carranta-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server =
            Box::leak(Box::new(Server::new(4, 63, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal("seats=4&name=Marta", host);
        let seat_of = |body: &str| -> i64 {
            body.rsplit("\"you\":")
                .next()
                .and_then(|t| t.split(&[',', '}'][..]).next())
                .and_then(|t| t.trim().parse().ok())
                .expect("a seat")
        };
        let first = get(port, &format!("/{id}/api/state"), guest);
        assert_eq!(seat_of(&first), 1, "the guest walked into the free chair");
        let mark: u64 = first
            .rsplit("\"mark\":")
            .next()
            .and_then(|t| t.split(&[',', '}'][..]).next())
            .and_then(|t| t.trim().parse().ok())
            .expect("a mark");

        // Their page asks again and is held, because nothing has changed yet.
        let held = {
            let id = id.clone();
            std::thread::spawn(move || get(port, &format!("/{id}/api/state?since={mark}"), guest))
        };
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(!held.is_finished(), "still waiting");

        // And while it waits, the host takes their seat back.
        assert!(server.unseat(&id, host, 1).is_ok());
        let body = held.join().expect("the held request finished");
        assert_eq!(
            seat_of(&body),
            -1,
            "answered as somebody watching, not as the chair they used to be in"
        );
        assert!(
            body.contains("took your seat back"),
            "and said so: a screen that changes under you without saying why is \
             the worst version of this"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_game_writes_down_who_was_still_at_it() {
        // The one thing the rating needs that the moves cannot say. Every move
        // writes the file, so the last write is the end of the game and this is
        // who was there for it: a game the house bot finished on everybody's
        // behalf is nobody's result.
        let dir = std::env::temp_dir().join(format!("carranta-here-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 31, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal("seats=4&name=Marta&pace=instant", host);
        assert!(server.sit(&id, guest, "Vidal").is_some());
        assert!(server.begin(&id));
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let acting = t.session.state().decider();
            let v = t.session.version();
            t.session.act_as(acting, 0, v).expect("playable");
            // The host shut their laptop five minutes ago. The guest is still
            // at the screen, which is what a page polling looks like.
            let seat = t.seat_of(host).expect("seated");
            t.seen[seat as usize] = now().saturating_sub(5 * 60 * 1000);
            let seat = t.seat_of(guest).expect("seated");
            t.seen[seat as usize] = now();
        }
        server.keep(&id);
        let saved = server.store().load(&id).expect("written");
        let gone: Vec<&str> = saved
            .setup
            .chairs
            .iter()
            .filter(|c| c.left)
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(gone, ["Marta"], "the one who is not there");
        assert!(
            crate::store::encode(&saved).contains("gone hostkey000000000 Marta"),
            "and the file says so in words"
        );
        // Somebody was still at it, so it is somebody's result. Both of them:
        // walking out costs you the place you finished in rather than voiding
        // the game for everyone who stayed.
        let mut finished = saved.clone();
        finished.winner = Some(0);
        assert!(crate::analysis::rated(&finished));
        for c in finished.setup.chairs.iter_mut() {
            c.left = true;
        }
        assert!(
            !crate::analysis::rated(&finished),
            "and a game the bots finished alone is nobody's"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_room_lets_go_of_somebody_who_closed_the_tab() {
        // The case the ready check could not survive. Everybody seated counts
        // towards it, so somebody who shut their laptop was somebody the room
        // could never have, and the people still in it were left pressing a
        // button that could not be enough. Its only end was the twenty minute
        // sweep, which closes the room under them.
        let dir = std::env::temp_dir().join(format!("carranta-gone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server =
            Box::leak(Box::new(Server::new(4, 71, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal("seats=4&name=Marta&pace=instant", host);
        assert_eq!(server.sit(&id, guest, "Vidal"), Some(1), "the guest sits");
        // The guest is ready. The host is not, and never will be: their tab has
        // been shut for five minutes.
        assert!(server.ready(&id, guest));
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let host_seat = t.seat_of(host).expect("seated");
            t.seen[host_seat as usize] = now().saturating_sub(5 * 60 * 1000);
            assert!(t.in_lobby(), "still a room");
            assert_eq!(t.ready_count(), (1, 2), "one of two, and stuck there");
        }

        // The guest's own page asks for the state, which is the only thing that
        // happens: no button, no second person, just the room being looked at.
        let body = get(port, &format!("/{id}/api/state"), guest);
        assert!(body.starts_with("HTTP/1.1 200 OK"), "{body:.40}");
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("still there");
            assert!(t.seat_of(host).is_none(), "the host's chair was let go of");
            assert!(!t.in_lobby(), "and the room, being agreed, started");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_room_has_no_clock_running_over_it() {
        // The bug this is for started games on its own about a minute after they
        // were dealt, with everybody still reading the settings. The poll is the
        // whole clock, and a turn clock left running over a lobby does not merely
        // tick: it runs out, forfeits the turn, and a forfeit is a move, so the
        // room becomes a game nobody agreed to start.
        //
        // Asserted against the route rather than the session, because the route
        // is where the guard lives and the guard is what went missing.
        let dir = std::env::temp_dir().join(format!("carranta-noclock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server =
            Box::leak(Box::new(Server::new(4, 81, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        let host = "hostkey000000000";
        // The shortest clock the lobby allows, so this test takes a second and
        // not a minute. Everything else is what `/lobby` deals.
        let id = server.deal("seats=4&clock=turn&clockSecs=1&discardSecs=1", host);
        for _ in 0..12 {
            let answer = get(port, &format!("/{id}/api/state"), host);
            assert!(
                answer.contains("\"inLobby\":true"),
                "the room stands: {answer:.200}"
            );
            assert!(
                answer.contains("\"started\":false"),
                "and nothing is played"
            );
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        // And the clock is a clock again the moment the room closes.
        assert!(server.begin(&id));
        let answer = get(port, &format!("/{id}/api/state"), host);
        assert!(answer.contains("\"inLobby\":false"), "the game is on");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_table_dealt_with_a_chair_held_waits_for_its_host_rather_than_the_count() {
        // The window an invitation needs. Before this the held chair filling was
        // what let the table go: the friend who read the message first took it,
        // the next poll played the first move, and the second friend arrived at a
        // game already under way. A table with a chair held is a room now, and a
        // room is closed by its host.
        let dir = std::env::temp_dir().join(format!("carranta-lobby-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 71, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let id = server.deal("seats=4&roles=you,open,bot,bot&pace=instant", host);

        // One friend takes the held chair. That used to be the starting gun.
        assert!(server.sit(&id, "guest1key0000000", "Nel").is_some());
        for _ in 0..5 {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            assert!(t.in_lobby(), "still a room");
            assert_eq!(t.waiting(), 0, "with nothing left to wait for");
            assert_eq!(t.takeable(), 2, "and two chairs going");
            // The poll is what plays the bots, and it does not touch a room.
            if !t.in_lobby() {
                t.session.tick();
            }
            assert!(!t.session.started(), "so nothing has been played");
        }

        // Which is what lets the other two in.
        assert!(server.sit(&id, "guest2key0000000", "Rui").is_some());
        assert!(server.sit(&id, "guest3key0000000", "Ada").is_some());

        // The room closes when the room agrees, and not before. Three people said
        // yes and the fourth had not: one seat short is a room, not a game.
        assert!(server.ready(&id, host), "a seat may say so");
        assert!(server.ready(&id, "guest1key0000000"));
        assert!(server.ready(&id, "guest2key0000000"));
        assert!(
            !server.ready(&id, "nobodykey0000000"),
            "and somebody with no seat may not"
        );
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.ready_count(), (3, 4), "three of four");
            assert!(t.in_lobby(), "so it is still a room");
        }
        // The last one is what ends it.
        assert!(server.ready(&id, "guest3key0000000"));
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            assert!(!t.in_lobby(), "the room is closed");
            assert!(
                t.drawn,
                "and the order was drawn, once it knew who was here"
            );
            t.session.tick();
            assert_eq!(t.takeable(), 0, "nobody else is getting in");
        }
        assert_eq!(server.sit(&id, "latekey000000000", "Late"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn somebody_arriving_takes_a_bot_s_chair_once_the_held_ones_are_gone() {
        // There is no holding a chair any more: a seat that blocked the game was
        // strictly worse than a bot's, which plays until somebody better
        // arrives. So every seat behind the dealer is a bot's, and every one of
        // them is a person's the moment a person turns up. Asking a host to
        // predict how many friends would come was the old model's flaw: one held
        // chair and two friends meant one of them stood outside a table with two
        // bots in it.
        let dir = std::env::temp_dir().join(format!("carranta-displace-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 63, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let id = server.deal("seats=4&pace=instant", host);
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.waiting(), 0, "no chair blocks the game");
            assert_eq!(t.takeable(), 3, "and three could be taken");
            assert!(!t.session.started(), "and nothing has begun");
        }

        // Three friends arrive, each into a bot's chair.
        let first = server
            .sit(&id, "guest1key0000000", "Vidal")
            .expect("a seat");
        let second = server
            .sit(&id, "guest2key0000000", "Nel")
            .expect("a bot's seat");
        let third = server
            .sit(&id, "guest3key0000000", "Rui")
            .expect("the other one");
        let mut seats = vec![first, second, third];
        seats.sort_unstable();
        seats.dedup();
        assert_eq!(seats.len(), 3, "three different chairs");

        // And now there is nothing to give away: every seat is a person's, so
        // the fourth arrival is not seated at all.
        assert_eq!(
            server.sit(&id, "guest4key0000000", "Late"),
            None,
            "a full table seats nobody"
        );
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.takeable(), 0, "and says so");
            assert_eq!(t.waiting(), 0);
            // Not drawn yet, and that is the rule: the order is settled when
            // the room closes rather than when the chairs fill, because people
            // can arrive right up to that moment.
            assert!(!t.drawn, "nothing is settled while it is still a room");
            assert_eq!(
                t.chairs
                    .iter()
                    .filter(|c| matches!(c, Chair::Taken { .. }))
                    .count(),
                4,
                "four people, four chairs"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bot_s_chair_stops_being_takeable_once_the_game_begins() {
        let dir = std::env::temp_dir().join(format!("carranta-shut-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 64, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        // Nothing held. Until somebody moves, the three bots' chairs are three
        // chairs a person could have, which is the point: a table dealt for one
        // is a table three friends can still walk into.
        let id = server.deal("seats=4&roles=you,bot,bot,bot&pace=instant", host);
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            // Whether the draw put a bot or the host on the opening seat decides
            // whether this poll plays anything, so the move is made here rather
            // than waited for.
            t.session.tick();
            if !t.session.started() {
                assert_eq!(t.takeable(), 3, "three chairs, until one is played");
                let acting = t.session.state().decider();
                let v = t.session.version();
                t.session
                    .act_as(acting, 0, v)
                    .expect("the opening is playable");
            }
            assert!(t.session.started());
            assert_eq!(t.takeable(), 0, "and none once it has begun");
        }
        assert_eq!(
            server.sit(&id, "latekey000000000", "Late"),
            None,
            "the door is shut, whoever the seat was holding"
        );
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
        let dealt = get(port, "/join?seats=4&name=Marta&pace=instant", host);
        let went = dealt
            .lines()
            .find_map(|l| l.strip_prefix("Location: "))
            .expect("somewhere to go")
            .trim()
            .to_string();
        let id = went.trim_matches('/').to_string();

        // Three chairs going, and the host's own name on theirs.
        let mine = get(port, &format!("/{id}/api/state"), host);
        assert!(mine.contains("\"seatsTakeable\":3"));
        // And the room's own state: one person in it, not yet ready.
        assert!(mine.contains("\"ready\":0"));
        assert!(mine.contains("\"readyOf\":1"));
        assert!(mine.contains("\"youAreReady\":false"));
        assert!(mine.contains("Marta"));

        // And nobody can move while the table is a room: a move would start the
        // game under everybody invited and not yet arrived.
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
        assert!(held.contains("has not started yet"), "{held:.80}");

        // One person takes a chair before the game starts.
        let early = "earlykey00000000";
        assert!(
            post(port, &format!("/{id}/api/sit"), early, "name=Vidal").contains("\"seat\":"),
            "a chair going is a chair you may take"
        );

        // The host gives up on the last one, which is what starts the game.
        // Everybody's call now, not the host's: a room nothing could start was
        // exactly what one person's button produced whenever they closed the tab.
        assert!(
            post(port, &format!("/{id}/api/ready"), "nobodykey0000000", "")
                .starts_with("HTTP/1.1 403"),
            "though somebody with no seat has nothing to be ready for"
        );
        assert!(post(port, &format!("/{id}/api/ready"), early, "").starts_with("HTTP/1.1 200"));
        assert!(post(port, &format!("/{id}/api/ready"), host, "").starts_with("HTTP/1.1 200"));
        let _ = get(port, &format!("/{id}/api/state"), host);

        // Now the door is shut. Somebody arriving is a watcher, whatever they
        // ask for and however many bots are sitting where they might have been.
        let late = "latekey000000000";
        let turned_away = post(port, &format!("/{id}/api/sit"), late, "name=Late");
        assert!(turned_away.contains("\"seat\":-1"), "{turned_away:.60}");
        assert!(get(port, &format!("/{id}/api/state"), late).contains("\"you\":-1"));

        // A move, which is what makes this a game with a file. It may have
        // happened already: giving the last chair to a bot settles the table,
        // which draws the turn order (§18), and if a bot drew the opening seat
        // then the next poll plays it. Either way what the rest of this needs is
        // a game under way, so it is played only if nobody has.
        let (host_seat, early_seat) = (
            server.seated(&id, host).expect("seated"),
            server.seated(&id, early).expect("seated"),
        );
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            if !t.session.started() {
                let acting = t.session.state().decider();
                let v = t.session.version();
                t.session
                    .act_as(acting, 0, v)
                    .expect("the opening is playable");
            }
            assert!(t.session.started(), "and now it is under way");
        }

        // And the two who were in seats are still in them, across a restart.
        server.keep(&id);
        server.tables.lock().unwrap().clear();
        assert!(server.seat(&id));
        assert_eq!(
            server.seated(&id, host),
            Some(host_seat),
            "rejoining is by key"
        );
        assert_eq!(server.seated(&id, early), Some(early_seat));
        assert_eq!(server.seated(&id, late), None, "and was never a way in");
        let back = get(port, &format!("/{id}/api/state"), early);
        assert!(back.contains("Vidal"), "still under their own name");

        // A chair that comes free in a game already under way is not a way in:
        // the rule is about the game having begun, not about the seat.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let bot = t
                .chairs
                .iter()
                .position(|c| *c == Chair::Bot)
                .expect("a bot somewhere");
            t.chairs[bot] = Chair::Open;
        }
        let too_late = post(port, &format!("/{id}/api/sit"), late, "name=Late");
        assert!(
            too_late.contains("\"seat\":-1"),
            "the door is shut, not the seat"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_answer_about_a_table_carries_the_whole_table() {
        // A move used to be answered off the session alone, which is a view with
        // the table's half missing: no chat, nothing said, no chairs going. So
        // the panel emptied itself on every click and announced that the table
        // had been dealt without chat, until the next poll three seconds later
        // put the conversation back.
        let dir = std::env::temp_dir().join(format!("carranta-whole-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("a port");
        let port = listener.local_addr().expect("bound").port();
        let server: &'static Server =
            Box::leak(Box::new(Server::new(4, 51, TradeMode::Full, &dir)));
        std::thread::spawn(move || server.serve(listener));

        let host = "hostkey000000000";
        let id = server.deal(
            "seats=4&roles=you,bot,bot,bot&name=Marta&chat=text&pace=instant",
            host,
        );
        assert!(server.say(&id, host, "anybody there"));

        // Whatever it is asked, and however it answers: a good move, a refused
        // one, and a request it cannot read at all.
        let state = get(port, &format!("/{id}/api/state"), host);
        let v = state
            .rsplit("\"version\":")
            .next()
            .and_then(|t| t.split(&[',', '}'][..]).next())
            .and_then(|t| t.trim().parse::<u64>().ok())
            .expect("a version");
        let post = |body: String| -> String {
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
        for (what, answer) in [
            ("a move", post(format!("{{\"action\":0,\"version\":{v}}}"))),
            (
                "a stale click",
                post(format!("{{\"action\":0,\"version\":{v}}}")),
            ),
            ("nonsense", post("{}".to_string())),
            ("a poll", state.clone()),
        ] {
            assert!(
                answer.contains("\"chat\":true"),
                "{what} says the table has chat"
            );
            assert!(
                answer.contains("anybody there"),
                "{what} carries what was said"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_table_waiting_for_nobody_is_closed() {
        let dir = std::env::temp_dir().join(format!("carranta-sweep-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 44, TradeMode::Full, &dir);
        let host = "hostkey000000000";

        // Three tables: two rooms nobody stayed in, and one already played.
        let waiting = server.deal("seats=4", host);
        let full = server.deal("seats=4", host);
        let played = server.deal("seats=4", host);
        assert!(server.begin(&played), "the played one left its room");
        {
            let mut tables = server.tables.lock().unwrap();
            for t in tables.iter_mut() {
                if t.id == played {
                    let acting = t.session.state().decider();
                    let v = t.session.version();
                    t.session.act_as(acting, 0, v).expect("playable");
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
            "a room nobody stayed in is closed"
        );
        assert!(
            !left.contains(&full),
            "however many of its chairs the bots hold: nothing started, so \
             there is nothing to keep"
        );
        assert!(
            !left.contains(&played),
            "and a game nobody has been near for twenty minutes leaves the table \
             too, which is not the same as being closed: see \
             an_idle_game_leaves_the_table_but_not_the_store"
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
        let guest = "guestkey00000000";
        assert!(server.sit(&id, guest, "Vidal").is_some());
        assert!(server.store().all().is_empty(), "nor does sitting down");
        // Everybody at the table said they were ready, which is what closes a
        // room. Reached straight here rather than through four buttons.
        assert!(server.begin(&id));
        assert!(
            server.store().all().is_empty(),
            "nor does filling the chairs"
        );
        // The first move writes it, with everybody's seat and name in it.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let acting = t.session.state().decider();
            let v = t.session.version();
            t.session.act_as(acting, 0, v).expect("playable");
        }
        server.keep(&id);
        let saved = server.store().load(&id).expect("written now");
        let named = |key: &str| {
            saved
                .setup
                .chairs
                .iter()
                .find(|c| c.who == key)
                .map(|c| c.name.clone())
        };
        assert_eq!(named(host).as_deref(), Some("Marta"));
        assert_eq!(named(guest).as_deref(), Some("Vidal"));
        assert_eq!(
            saved.setup.chairs.iter().filter(|c| c.who == "bot").count(),
            2,
            "and the two bots are still bots"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn leaving_before_the_start_gives_the_chair_back() {
        let dir = std::env::temp_dir().join(format!("carranta-leave-early-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 61, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal("seats=4&name=Marta", host);
        let took = server.sit(&id, guest, "Vidal").expect("a chair going");
        assert!(
            server.ready(&id, guest),
            "and they even said they were ready"
        );

        // Nothing has happened yet, so standing up leaves nothing behind: the
        // chair is a bot's again and somebody else may take it.
        assert!(server.leave(&id, guest));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.chairs[took as usize], Chair::Bot);
            assert_eq!(t.takeable(), 3, "so the table has room again");
            assert_eq!(t.seat_of(guest), None, "they are not at it");
            assert_eq!(t.session.name_of(took), "", "and their name went with them");
            assert!(
                !t.ready.get(took as usize).copied().unwrap_or(true),
                "their yes went with them too, or the next sitter inherits it"
            );
            assert!(t.in_lobby(), "the host said nothing, so the room stands");
        }
        // Somebody else takes it. Which chair they end up in is the table's
        // again: filling the last seat settles it, and settling it redraws the
        // order, which nothing has happened yet to make unfair.
        assert!(server.sit(&id, "thirdkey00000000", "Nils").is_some());
        assert!(
            server.store().all().is_empty(),
            "and none of it was written"
        );

        // The host standing up before the start does not strand the table. It
        // used to be able to: starting was the host's button, with a fallback to
        // whoever held seat nought, so a host who left a table whose seat nought
        // then went to a bot left a room nothing could start. Nobody is special
        // now, so there is nothing to hand on.
        let id = server.deal("seats=4&name=Marta", host);
        assert_eq!(
            server.seated(&id, host),
            Some(0),
            "the host deals into nought"
        );
        assert!(server.leave(&id, host));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.chairs[0], Chair::Bot, "seat nought is a bot's again");
            assert_eq!(t.ready_count(), (0, 0), "and nobody is left to be ready");
            assert!(t.in_lobby(), "an empty room is a room, not a game");
        }
        // Somebody arriving into the empty room can still close it, which is the
        // whole of not being stranded.
        assert_eq!(server.sit(&id, "fourthkey0000000", "Aleks"), Some(0));
        assert!(
            server.ready(&id, "fourthkey0000000"),
            "and say they are ready"
        );
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert!(!t.in_lobby(), "which starts the game");
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
        assert!(server.sit(&id, guest, "Vidal").is_some(), "a chair going");
        // Everybody at the table said they were ready, which is what closes a
        // room. Reached straight here rather than through four buttons.
        assert!(server.begin(&id));
        // Read after, not before: closing the room is what draws the turn order,
        // so a seat noted before it is a seat somebody has since been moved out
        // of.
        let guest_seat = server.seated(&id, guest).expect("seated");
        let host_seat = server.seated(&id, host).expect("seated");
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let acting = t.session.state().decider();
            let v = t.session.version();
            t.session.act_as(acting, 0, v).expect("playable");
            assert!(t.session.started());
        }

        // Mid-game, the seat cannot go back to the table: the others are owed an
        // opponent rather than a gap. It stays theirs and the bot plays it.
        assert!(server.leave(&id, guest));
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(
                t.seat_of(guest),
                Some(guest_seat),
                "the seat is still theirs"
            );
            assert_eq!(t.waiting(), 0, "and is not on offer to anybody else");
            assert!(!t.present(guest_seat), "but nobody is in it");
            assert!(!t.session.is_person(guest_seat), "so the bots play it");
            assert!(
                t.session.is_person(host_seat),
                "and the person still there is asked"
            );
        }

        // Coming back is the ordinary path: their next request is the evidence.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            assert!(t.saw(guest_seat), "their return changes who is playing");
            t.seat_the_people();
            assert!(
                t.session.is_person(guest_seat),
                "and the seat is theirs again"
            );
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
        assert!(server.sit(&id, guest, "Vidal").is_some(), "a chair going");
        // Everybody at the table said they were ready, which is what closes a
        // room. Reached straight here rather than through four buttons.
        assert!(server.begin(&id));
        // Read after, not before: closing the room is what draws the turn order,
        // so a seat noted before it is a seat somebody has since been moved out
        // of.
        let guest_seat = server.seated(&id, guest).expect("seated");
        let host_seat = server.seated(&id, host).expect("seated");
        let both = {
            let mut both = vec![host_seat, guest_seat];
            both.sort_unstable();
            both
        };

        let mut tables = server.tables.lock().unwrap();
        let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
        assert_eq!(t.playing(), both, "both here");

        // One walks away.
        t.seen[guest_seat as usize] = 0;
        assert_eq!(t.playing(), vec![host_seat], "the other carries on");

        // Then so does the other, and the table waits for both of them.
        t.seen[host_seat as usize] = 0;
        assert_eq!(
            t.playing(),
            both,
            "an empty room waits rather than playing on"
        );
        drop(tables);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_turn_order_is_drawn_rather_than_handed_to_whoever_dealt() {
        // Turn order is seat order, and the seats used to be handed out in the
        // order people arrived: the host at nought and therefore always first.
        // Going first is worth something, so that was a thumb on the scale in
        // every game this server dealt.
        let dir = std::env::temp_dir().join(format!("carranta-order-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 71, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let mut seen = [0usize; 4];
        for _ in 0..80 {
            let id = server.deal("seats=4&name=Marta", host);
            // The draw happens when the room closes, not when it is dealt:
            // people can arrive right up to that moment.
            assert_eq!(server.seated(&id, host), Some(0), "in the room, unmoved");
            assert!(server.begin(&id));
            let seat = server.seated(&id, host).expect("the host has a seat");
            seen[seat as usize] += 1;
            server.tables.lock().unwrap().clear();
        }
        assert_eq!(seen.iter().sum::<usize>(), 80);
        // Every seat, and no seat most of the time. Loose bounds on purpose:
        // this is a draw, and a test that demanded a tidy split would fail on
        // the honest ones.
        for (seat, &n) in seen.iter().enumerate() {
            assert!(n > 0, "seat {seat} never came up in eighty deals: {seen:?}");
            assert!(n < 60, "seat {seat} came up {n} times in eighty: {seen:?}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_order_is_drawn_once_the_table_is_settled_and_not_after() {
        // Before anybody has played, so nobody is moved out from under a game
        // they are in the middle of.
        let dir = std::env::temp_dir().join(format!("carranta-order-when-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 72, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        // A room is not settled, so the host is where they were put and the
        // order is not drawn yet, however many people arrive.
        let id = server.deal("seats=4&name=Marta", host);
        assert_eq!(server.seated(&id, host), Some(0), "not settled, not drawn");
        let sat = server.sit(&id, guest, "Vidal").expect("a chair going");
        assert_eq!(sat, 1, "seated in arrival order while it is a room");
        // Closing the room is what settles it, and the seats are dealt out.
        assert!(server.begin(&id));
        let guest_seat = server.seated(&id, guest).expect("still at the table");
        let host_seat = server.seated(&id, host).expect("still at the table");
        assert_ne!(host_seat, guest_seat);
        // Their names came with them rather than staying with the chairs.
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.session.name_of(host_seat), "Marta");
            assert_eq!(t.session.name_of(guest_seat), "Vidal");
            for bot in 0..4u8 {
                if bot != host_seat && bot != guest_seat {
                    assert_eq!(t.session.name_of(bot), "", "a bot is nobody's name");
                    assert!(!t.session.is_person(bot), "and is played as a bot");
                }
            }
        }
        // Once it is under way nothing moves again, however the table changes.
        {
            let mut tables = server.tables.lock().unwrap();
            let t = tables.iter_mut().find(|t| t.id == id).expect("dealt");
            let acting = t.session.state().decider();
            let v = t.session.version();
            t.session.act_as(acting, 0, v).expect("playable");
        }
        assert!(server.leave(&id, guest), "somebody leaves mid-game");
        assert_eq!(server.seated(&id, host), Some(host_seat), "nobody moved");
        assert_eq!(server.seated(&id, guest), Some(guest_seat));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_the_people_at_a_table_can_talk_at_it() {
        let dir = std::env::temp_dir().join(format!("carranta-talk-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 73, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let guest = "guestkey00000000";
        let id = server.deal("seats=4&name=Marta&chat=text", host);
        server.sit(&id, guest, "Vidal").expect("a chair going");

        assert!(server.say(&id, host, "wood for sheep?"));
        assert!(server.say(&id, guest, "never"));
        // Nobody else: watching a game is standing behind the players.
        assert!(
            !server.say(&id, "watchkey00000000", "hello"),
            "not from a watcher"
        );
        assert!(!server.say(&id, "", "hello"), "nor from nobody at all");
        // And nothing is not something to say.
        assert!(!server.say(&id, host, "   "), "an empty line is not a line");

        let tables = server.tables.lock().unwrap();
        let t = tables.iter().find(|t| t.id == id).expect("dealt");
        assert_eq!(t.said.len(), 2);
        assert_eq!(t.said[0].text, "wood for sheep?");
        assert_eq!(
            t.said[0].name, "Marta",
            "under the name they are sitting as"
        );
        assert_eq!(t.said[1].name, "Vidal");
        drop(tables);

        // A table dealt without chat still talks while it is a room, because
        // gathering people is a conversation by nature; the setting binds the
        // moment the game starts.
        let quiet = server.deal("seats=4&name=Marta", host);
        assert!(server.say(&quiet, host, "hello?"), "a room always talks");
        assert!(server.begin(&quiet), "the room closes");
        assert!(
            !server.say(&quiet, host, "hello??"),
            "and then the setting means something"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_is_said_is_bounded_kept_out_of_the_game_and_never_markup() {
        let dir = std::env::temp_dir().join(format!("carranta-talk-safe-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 74, TradeMode::Full, &dir);
        let host = "hostkey000000000";
        let id = server.deal("seats=4&roles=you,bot,bot,bot&name=Marta&chat=text", host);

        // Bounded, so one person cannot hand everybody else a novel.
        assert!(server.say(&id, host, &"x".repeat(4000)));
        for i in 0..TALK_KEPT + 40 {
            server.say(&id, host, &format!("line {i}"));
        }
        {
            let tables = server.tables.lock().unwrap();
            let t = tables.iter().find(|t| t.id == id).expect("dealt");
            assert_eq!(t.said.len(), TALK_KEPT, "the oldest fall off the front");
            assert!(t.said.iter().all(|d| d.text.chars().count() <= TALK_LIMIT));
            // The oldest kept is not the first said: the front was dropped.
            assert_ne!(t.said[0].text, "line 0");
        }

        // It is escaped where it becomes JSON, once, and is never anything but
        // text after that.
        assert!(server.say(&id, host, "</script><b>\"hi\"</b>"));
        let tables = server.tables.lock().unwrap();
        let t = tables.iter().find(|t| t.id == id).expect("dealt");
        let talk: Vec<view::Talk<'_>> = t
            .said
            .iter()
            .map(|d| view::Talk {
                seat: d.seat,
                name: &d.name,
                text: &d.text,
            })
            .collect();
        let out = view::render_all(&t.session, Some(0), view::Room::default(), &talk, None);
        // Two things make this safe and neither is a filter on the words. The
        // payload is JSON fetched over HTTP, so what must not happen is the text
        // breaking out of its string: the quotes are escaped.
        assert!(
            out.contains(r#"\"hi\""#),
            "quotes escaped rather than closing"
        );
        assert!(
            strings_are_closed(&out),
            "and the payload is still one well-formed object"
        );
        // And the page writes it with `textContent`, so it is never markup on
        // the way in either. Checked against the page's own source, because the
        // day somebody reaches for `innerHTML` there is the day this matters.
        const PAGE: &str = include_str!("../assets/index.html");
        assert!(
            PAGE.contains("what.textContent = t.said;"),
            "the talk goes into the page as text"
        );

        // And the game itself knows nothing about any of it. §9.7.1: free text
        // from a player must never reach a bot's input, and the way to keep that
        // promise is to leave no path for it to travel. The talk is on the table;
        // the bots are handed a `State`.
        let bare = view::render_for(&t.session, 0);
        assert!(
            !bare.contains("wood for sheep"),
            "not in the game's own view"
        );
        assert!(
            !t.session.log().iter().any(|l| l.text.contains("script")),
            "and not in the log the session keeps"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Whether every quote in a payload is either escaped or a delimiter.
    ///
    /// A whole JSON parser is not the point: what is being checked is that
    /// somebody else's words cannot end the string they are in, which is the one
    /// way text in this payload could become something other than text.
    fn strings_are_closed(s: &str) -> bool {
        let (mut inside, mut escaped) = (false, false);
        for c in s.chars() {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                inside = !inside;
            }
        }
        !inside
    }

    #[test]
    fn a_name_is_what_somebody_said_and_nothing_else() {
        // Trimmed and bounded, because it is somebody else's text, and empty
        // when they said nothing: what an unnamed seat is called depends on
        // which seat it is, which is not settled until the order is drawn.
        assert_eq!(called("Vidal"), "Vidal");
        assert_eq!(called("  Vidal  "), "Vidal");
        assert_eq!(called(""), "");
        assert_eq!(called("   "), "");
        assert_eq!(called(&"x".repeat(80)).chars().count(), 24);
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
        // Seat nought is the dealer whatever the query says about it, and the
        // old `open` word deals a bot too: a link written when a chair could be
        // held empty still deals a table, and a bot's chair is a person's the
        // moment they arrive, so nothing the hold offered is lost.
        let chairs = chairs_from("roles=open,open,bot,bot", 4, "keytest0000000000", "Egon");
        assert_eq!(
            chairs[0],
            Chair::Taken {
                key: "keytest0000000000".to_string(),
                name: "Egon".to_string()
            }
        );
        assert_eq!(chairs[1], Chair::Bot);
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
        // An idle server is the button and nothing else: a card saying it holds
        // no tables is a hole in the page rather than an answer.
        assert!(!first.contains(">Tables</h2>"), "nothing dealt yet");
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

        // Opening the lobby is creating one: the visitor is sent to a fresh
        // table's own address, key in hand, because the screen's whole point is
        // to be shared and an address needs a table behind it.
        let lobby = get(port, "/lobby", "");
        assert!(lobby.starts_with("HTTP/1.1 303 See Other"), "{lobby:.40}");
        assert!(lobby.contains("Set-Cookie: carranta="));
        let went = lobby
            .lines()
            .find_map(|l| l.strip_prefix("Location: "))
            .expect("somewhere to go")
            .trim()
            .to_string();
        assert!(
            is_game_id(went.trim_matches('/')),
            "to a table of its own: {went}"
        );

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
