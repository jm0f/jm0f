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
use crate::store::{Saved, Store, game_id, is_game_id};
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
        moves: t.session.moves().to_vec(),
        times: t.session.times().to_vec(),
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
}

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
        let (mut mine, mut others) = (Vec::new(), Vec::new());
        for g in self.store.all() {
            if live.contains(&g.id) {
                continue;
            }
            if !player.is_empty() && g.by == player {
                mine.push(g);
            } else {
                others.push(g);
            }
        }
        // A name to put in the form, taken from the last table they dealt, so a
        // second game does not ask again for something already answered. Tables
        // before stored games: a table dealt and not yet played in is the most
        // recent thing they said their name was, and it is the case that matters,
        // since a game with no moves in it has no file to read the name out of.
        let name = open
            .iter()
            .filter(|t| t.mine)
            .map(|t| t.host.clone())
            .chain(mine.iter().map(|g| g.name.clone()))
            .find(|n| !n.is_empty())
            .unwrap_or_default();
        crate::home::page(&open, &mine, &others, &name)
    }

    /// Deal a table from the home page's form.
    ///
    /// The form's fields are the lobby's, so the two ways of dealing a table
    /// cannot drift apart: this reads the same names out of the body that
    /// `api/new` reads out of a query.
    fn deal(&self, form: &str, player: &str) -> String {
        let seats = param(form, "seats")
            .and_then(|v| v.parse().ok())
            .unwrap_or(4);
        let name = decode(&param(form, "name").unwrap_or_default());
        let named = decode(&param(form, "game").unwrap_or_default());
        let pace = Pace::parse(param(form, "pace").as_deref());
        let session = Session::new(seats, self.next_seed(), TradeMode::Full)
            .with_public(wants_public(form))
            .with_game(&named)
            .with_pace(pace)
            .with_name(&name);
        // Nothing is written here: a table nobody has moved on is not a game
        // (see `keep`), and it is on the home page's list from memory either way.
        self.add(Table {
            id: mint_id(),
            session,
            dealt: now(),
            by: player.to_string(),
        })
    }

    /// A seed for the next table, and the cursor moved on past it.
    fn next_seed(&self) -> u64 {
        let mut seed = self.seed.lock().unwrap();
        let mine = *seed;
        *seed = seed.wrapping_add(1);
        mine
    }

    /// Put a table at the front, and drop the ones that no longer fit.
    ///
    /// Finished first, then oldest, because a game somebody is still playing is
    /// the last thing to take off the table.
    fn add(&self, table: Table) -> String {
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
            });
            self.keep(&id);
            if finished {
                have += 1;
                out.push(id);
            }
        }
        out
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
            // Deal a table from the home page's form, and go and sit at it.
            //
            // A form post rather than the board page's `api/new`, because a page
            // with no script has to be able to say "make me one" in the one way
            // HTML has always had, and because the answer to a form is a place
            // to go rather than a payload to draw.
            ("POST", "/new") => {
                let form = if body.is_empty() { query } else { &body };
                let id = self.deal(form, &player);
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
                let mut tables = self.tables.lock().unwrap();
                let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                    // A game no longer on a table is one this server has put
                    // away: read it off disk and hand it back as it stands.
                    // Nothing ticks, because nothing is waiting.
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
                let payload = view::render(&t.session);
                drop(tables);
                self.keep(&id);
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            ("POST", "/api/act") => {
                let id = game.clone().unwrap_or_default();
                let mut tables = self.tables.lock().unwrap();
                let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                    return respond(&mut stream, 409, "text/plain", b"that game is over");
                };
                let session = &mut t.session;
                let action = json::read_u64(&body, "action");
                let version = json::read_u64(&body, "version");
                let payload = match (action, version) {
                    (Some(a), Some(v)) => match session.act(a as usize, v) {
                        Ok(()) => view::render(&session),
                        Err(e) => view::render_with_note(&session, &refusal(&e)),
                    },
                    _ => view::render_with_note(session, "malformed request"),
                };
                drop(tables);
                self.keep(&id);
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // Put back a development card whose action was never finished.
            ("POST", "/api/cancel") => {
                let id = game.clone().unwrap_or_default();
                let mut tables = self.tables.lock().unwrap();
                let Some(t) = tables.iter_mut().find(|t| t.id == id) else {
                    return respond(&mut stream, 409, "text/plain", b"that game is over");
                };
                let session = &mut t.session;
                let payload = match json::read_u64(&body, "version") {
                    Some(v) => match session.cancel(v) {
                        Ok(()) => view::render(&session),
                        Err(e) => view::render_with_note(&session, &refusal(&e)),
                    },
                    None => view::render_with_note(&session, "malformed request"),
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            ("POST", "/api/propose") => {
                let id = game.clone().unwrap_or_default();
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
                        match session.propose(to, g, w, v) {
                            Ok(()) => view::render(&session),
                            Err(e) => view::render_with_note(&session, &refusal(&e)),
                        }
                    }
                    _ => view::render_with_note(&session, "malformed request"),
                };
                respond(&mut stream, 200, "application/json", payload.as_bytes())
            }
            // Starting a game is the one thing that does not belong to a
            // game, so it is not scoped to one: any page may ask for a table.
            ("POST", "/api/new") => {
                let seats = param(query, "seats")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(4);
                let mode = match param(query, "mode").as_deref() {
                    Some("disabled") => TradeMode::Disabled,
                    Some("restricted") => TradeMode::Restricted,
                    _ => TradeMode::Full,
                };
                let seed = param(query, "seed")
                    .and_then(|v| crate::game::parse_seed(&decode(&v)))
                    // No clock dependency: the newest table's seed advances.
                    .unwrap_or_else(|| self.next_seed());
                // The clock is a lobby setting: which kind, and how many
                // seconds it allows. Zero seconds is untimed either way.
                let secs: u64 = param(query, "clockSecs")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let increment: u64 = param(query, "clockInc")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let clock = Clock::parse(param(query, "clock").as_deref(), secs, increment);
                // The discard has an allowance of its own, because a seven is
                // an interruption and not part of anybody's turn. Zero is no
                // limit, and an absent parameter means the default rather than
                // none: a lobby that does not mention it still wants one.
                let discard_secs: u64 = param(query, "discardSecs")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(DEFAULT_DISCARD_SECS);
                let name = param(query, "name").unwrap_or_default();
                let log_shown = param(query, "log").as_deref() != Some("off");
                let public = wants_public(query);
                let named = param(query, "game").unwrap_or_default();
                let pace = Pace::parse(param(query, "pace").as_deref());
                // Anything but an explicit "rough" counts the stacks, since
                // that is what the rules already let anybody do (R-5.6).
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
                // A new game is a new address *and* a new table. The old one
                // keeps its file, keeps its address and keeps being playable,
                // which is what a table has to be once there is a page listing
                // more than one of them.
                let id = self.add(Table {
                    id: mint_id(),
                    session,
                    dealt: now(),
                    by: player.clone(),
                });
                self.keep(&id);
                // The page is told where it now is, so it can move there
                // without asking again.
                let payload = format!("{{\"went\":\"/{id}/\"}}");
                respond(&mut stream, 200, "application/json", payload.as_bytes())
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
/// Always a 303: the answer to a form post is a page to look at rather than the
/// post repeated, which is what a browser does with a 307 on a reload.
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
        // Every visit to `/` deals a table. Writing one at that point put a
        // game on disk for every time the page was opened, each a seed and
        // nothing else, and every figure computed across the store was then
        // divided by them.
        let dir = std::env::temp_dir().join(format!("carranta-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let server = Server::new(4, 7, TradeMode::Full, &dir);
        assert!(
            server.store().all().is_empty(),
            "a fresh server has no games"
        );
        // Nor does dealing again, which is what the home page's form does.
        let id = server.deal("name=Egon", "keytest0000000000");
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
    fn the_home_page_deals_a_table_and_remembers_whose_it_is() {
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
        assert!(first.contains("Deal a table"));
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

        // The form deals a table and says where to go and sit at it.
        let form = "name=Egon&game=Test&seats=3&visibility=public&pace=instant";
        let dealt = ask(
            port,
            &format!(
                "POST /new HTTP/1.1\r\nHost: localhost\r\n\
                 Cookie: {PLAYER_COOKIE}={key}\r\n\
                 Content-Length: {}\r\n\r\n{form}",
                form.len()
            ),
        );
        assert!(dealt.starts_with("HTTP/1.1 303 See Other"), "{dealt:.40}");
        let went = dealt
            .lines()
            .find_map(|l| l.strip_prefix("Location: "))
            .expect("somewhere to go")
            .trim()
            .to_string();
        let id = went.trim_matches('/').to_string();
        assert!(is_game_id(&id), "{went} is a game's address");
        assert!(get(port, &went, &key).starts_with("HTTP/1.1 200 OK"));

        // And it is on the home page as this visitor's, with what was asked for.
        let listed = get(port, "/", &key);
        assert!(listed.contains(&id), "the table is listed");
        assert!(listed.contains("Test"), "under the name it was given");
        assert!(listed.contains("yours"));
        assert!(listed.contains("Sit down"));
        assert!(listed.contains("<td>3</td>"), "three seats, as asked");
        // And the form does not ask for a name it has already been told, which
        // has to come off the table rather than the store: nothing is written
        // down until somebody moves.
        assert!(listed.contains("value=\"Egon\""));

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
