//! Games on disk, and the game ids that address them.
//!
//! A game is a seed, a table and a list of moves. The engine is deterministic,
//! so that is the whole game: replaying those moves from that seed rebuilds the
//! position down to the next random number, which is what lets a file of a few
//! hundred bytes stand in for everything the analytics need to read. It is the
//! same argument `carranta-record` makes about its own event log (H-1), at the
//! size this server is.
//!
//! The format is lines of text rather than anything packed. A game is small, a
//! local server writes a handful of them, and a file you can open and read is
//! worth more here than a file you can parse quickly.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use carranta_core::action::Action;
use carranta_core::state::{RESOURCES, Resource, TradeMode};

use crate::game::{Clock, Pace, Step};

/// How the table was set up to play, as the lobby asked for it.
///
/// Separate from the game rather than beside it, because it is a different kind
/// of fact. Seats, seed and moves *are* the game and rebuild it exactly; these
/// are the arrangements around it, and a file that has lost them is still a whole
/// game played under arrangements nobody wrote down. Grouped here so that reads
/// out of the type rather than out of a comment.
///
/// They were not written down at all until version 4, which showed up the moment
/// a game could be taken up again after a restart: the position came back exact
/// and the table came back with a different clock on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Setup {
    /// What the table is called, or empty.
    pub game: String,
    /// Whether it was dealt as a listed table.
    pub public: bool,
    /// How fast the bots move.
    pub pace: Pace,
    pub clock: Clock,
    /// The discard's own allowance, which is not the turn clock: a seven is an
    /// interruption rather than part of anybody's turn.
    pub discard_secs: u64,
    /// Whether the bank shows exact counts or stack sizes.
    pub bank_exact: bool,
    /// Whether the table keeps a log.
    pub log: bool,
    /// Whether the people at the table can talk to each other.
    ///
    /// The conversation itself is not written down anywhere: the record is the
    /// moves, and a game replayed from its file is the same game whatever was
    /// said over it. What is written down is whether talking was allowed, which
    /// is a lobby answer like the clock and the bank.
    pub chat: bool,
    /// Who is in each seat, in seat order.
    ///
    /// Empty for a game written before there were chairs, and for one nobody
    /// was sitting at. The server reads that as the table it used to be: the
    /// dealer at seat nought and bots behind them.
    pub chairs: Vec<Chair>,
}

/// One seat, written down.
///
/// `who` is `bot`, `open`, or a person's key; `name` is what they called
/// themselves, and is empty for anything that is not a person. Strings rather
/// than a type of their own, because the store's job is to write down what it
/// was told: the server owns what the words mean, and the file stays a thing you
/// can open and read, which is the whole argument for this format.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Chair {
    pub who: String,
    pub name: String,
    /// Which software played this seat, as `name@version`, when `who` is `bot`.
    ///
    /// Empty for a person, and for a file written before version 8, which had
    /// only the word `bot` and meant the one build that existed then. A rating
    /// is a claim about a player and two builds of a program are two players
    /// (see `carranta_bot::HOUSE`), so the seat identity is built from this
    /// rather than from the fact that something automatic was sitting here.
    pub agent: String,
    /// Whether this seat's person was away when the file was last written.
    ///
    /// Every move writes the file, so the last write is the end of the game and
    /// this is who was still at the table for it. That is the one thing the
    /// rating needs that the moves cannot say: a game finished by the house bot
    /// on everybody's behalf is not a game anybody played (P-2).
    ///
    /// Always false for a bot and for a held chair, and for a file written
    /// before version 8, which is the right reading of a file that does not
    /// say: those were all written under a rule that stops a table dead as soon
    /// as nobody is at it, so every game in them was finished by somebody.
    pub left: bool,
}

impl Chair {
    /// A seat played by software, under the build that is playing it.
    pub fn bot() -> Self {
        Chair::bot_as(&format!(
            "{}@{}",
            carranta_bot::HOUSE,
            carranta_bot::HOUSE_VERSION
        ))
    }

    /// A seat played by named software: `agent` is `name@version`, and the
    /// name is the whole identity, so a trained champion's chair says which
    /// champion rather than only that something automatic sat here.
    pub fn bot_as(agent: &str) -> Self {
        Chair {
            who: "bot".to_string(),
            name: String::new(),
            agent: agent.to_string(),
            left: false,
        }
    }

    pub fn open() -> Self {
        Chair {
            who: "open".to_string(),
            ..Default::default()
        }
    }

    pub fn person(key: &str, name: &str) -> Self {
        Chair {
            who: key.to_string(),
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Whether a person is in this chair, as opposed to a bot or nobody.
    pub fn is_person(&self) -> bool {
        self.who != "bot" && self.who != "open" && !self.who.is_empty()
    }

    /// The agent's name and build, for a bot's chair.
    ///
    /// Falls back to the house bot as it was when the word `bot` meant only one
    /// thing, which is what every file before version 8 says.
    pub fn agent_id(&self) -> (String, u32) {
        let (name, version) = self.agent.split_once('@').unwrap_or((&self.agent, ""));
        let name = if name.is_empty() {
            carranta_bot::HOUSE
        } else {
            name
        };
        (
            name.to_string(),
            version.parse().unwrap_or(carranta_bot::HOUSE_VERSION),
        )
    }
}

impl Default for Setup {
    /// What a session is when nobody has said otherwise, matched to
    /// `Session::new` so that a file written before version 4 comes back as the
    /// game it was rather than as a differently arranged one.
    ///
    /// The one exception is the pace, which is `Instant` in `Session::new` so
    /// that tests do not wait on a wall clock, and is the lobby's default here
    /// because a game read off disk is a game somebody is going to watch.
    fn default() -> Self {
        Setup {
            game: String::new(),
            public: false,
            pace: Pace::parse(None),
            clock: Clock::Off,
            discard_secs: crate::game::DEFAULT_DISCARD_SECS,
            bank_exact: true,
            log: true,
            // Off, matching the lobby: a table talks because somebody chose that
            // it should, and a game written before the setting existed was
            // played without one.
            chat: false,
            chairs: Vec::new(),
        }
    }
}

/// What one file says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub id: String,
    pub seats: u8,
    pub seed: u64,
    pub mode: TradeMode,
    /// What the person at this table is called, for the analytics to name them.
    pub name: String,
    /// Which visitor dealt this table, as an opaque key from their cookie.
    ///
    /// Not a login and not a name: a random string this browser was handed on
    /// its first visit, so a home page can show somebody their own games
    /// without knowing anything else about them. Empty for a game dealt before
    /// there were keys, and for the demo games the server plays itself, which
    /// belong to nobody.
    ///
    /// When there are accounts this becomes the account's key and the cookie
    /// becomes one way of proving which account you are. That is why it is
    /// stored as a key rather than as a cookie value with a name attached.
    pub by: String,
    /// Unix milliseconds when the game was dealt, so a list of games can be
    /// ordered by when they happened rather than by how the directory reads.
    ///
    /// Milliseconds rather than seconds because the order is load-bearing: a
    /// rating is a function of every game before it, so two games dealt in the
    /// same second were being put in whatever order their addresses happened to
    /// sort in, and the ratings followed.
    pub dealt: u64,
    /// Set once somebody has won, so a finished game can be told from one that
    /// was abandoned halfway.
    pub winner: Option<u8>,
    /// The lobby's answers, so a table taken up again is the table it was.
    pub setup: Setup,
    pub moves: Vec<Step>,
    /// Milliseconds from the deal to each step, one per entry in `moves`.
    ///
    /// Either empty or exactly as long as `moves`. Empty means the game was
    /// written before this was recorded, and empty rather than zeroes because
    /// "nobody knows" and "it took no time" are different answers and only one
    /// of them is true of an old file.
    ///
    /// A separate list rather than a field on `Step` for that reason: a step is
    /// what happened and is enough to rebuild the game on its own, which is the
    /// whole argument this format rests on. When it happened is something known
    /// about the step rather than part of it, and a file that has lost it is
    /// still a whole game.
    pub times: Vec<u32>,
}

/// A game id: three groups, the way the lobby already prints a seed.
///
/// Not the seed. Two games can be dealt the same board, and an address that
/// collided in that case would quietly serve one game's analytics for another.
pub fn game_id(mut n: u64) -> String {
    const ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";
    let mut out = String::new();
    for group in 0..3 {
        if group > 0 {
            out.push('-');
        }
        for _ in 0..4 {
            out.push(ALPHABET[(n % ALPHABET.len() as u64) as usize] as char);
            n /= ALPHABET.len() as u64;
        }
    }
    out
}

/// Whether a path segment could be one of ours.
///
/// Checked before a segment is ever joined to a directory: an id arrives in a
/// URL, and a URL is somebody else's text until it has been looked at.
pub fn is_game_id(s: &str) -> bool {
    s.len() == 14
        && s.as_bytes().iter().enumerate().all(|(i, c)| {
            if i % 5 == 4 {
                *c == b'-'
            } else {
                c.is_ascii_alphanumeric()
            }
        })
}

/// What this build writes.
///
/// Version 2 added the `at` lines, which say when each step landed. Version 3
/// added `by`, the key of whoever dealt the table. Version 4 added the lobby's
/// settings: what the table is called, whether it is listed, the pace, the clock,
/// the discard allowance, the bank and the log. Version 5 added `chairs`, who is
/// sitting in each seat, once more than one person could be. Older files are
/// still read and simply have less to say: every addition sits beside the moves
/// rather than inside them, so an older game is not an unreadable game, one
/// written before version 4 comes back on a table set up the way a fresh one is,
/// and one written before version 5 comes back with its dealer alone at it.
/// Version 6 gave each seat a line of its own so it could carry a name, and
/// still reads version 5's single `chairs` line as seats nobody named. Version 7
/// added `chat`, whether the table may talk, which is a setting and not a
/// transcript: what was said is never written down.
///
/// Version 8 made the chair lines say two more things, both of which the rating
/// needs and neither of which the moves can carry. A bot's line names the build
/// that played the seat, because two builds of a program are two players and a
/// rating that pools them describes neither. And a person's line is written as
/// `gone` rather than `chair` when they were away at the time, which for the
/// last write of a finished game is who was not there at the end. Version 8 also
/// writes the chairs for every table rather than only for one with somebody at
/// it: four lines of `bot` said nothing worth the space until the day they named
/// a version, and a self-play corpus is exactly where that is the question.
/// Older files read as the one build there was, with nobody having left, which
/// is what they meant.
const VERSION: u32 = 8;

/// Times per `at` line. Forty numbers is a line you can still read.
const TIMES_PER_LINE: usize = 40;

fn res_code(r: Resource) -> char {
    match r {
        Resource::Brick => 'b',
        Resource::Wood => 'w',
        Resource::Wool => 'o',
        Resource::Wheat => 'h',
        Resource::Ore => 'r',
    }
}

fn res_of(c: &str) -> Option<Resource> {
    RESOURCES
        .iter()
        .copied()
        .find(|r| res_code(*r).to_string() == c)
}

fn seat_code(v: Option<u8>) -> String {
    v.map_or_else(|| "-".to_string(), |n| n.to_string())
}

fn seat_of(s: &str) -> Option<Option<u8>> {
    if s == "-" {
        return Some(None);
    }
    s.parse().ok().map(Some)
}

/// One step, as a line.
fn step_line(step: &Step) -> String {
    match *step {
        Step::Move(a) => move_line(&a),
        // A refusal is not a move and reads as one: "seat 2 said no to offer 0".
        Step::Passed { offer, by } => format!("no {offer} {by}"),
    }
}

fn step_of(line: &str) -> Option<Step> {
    if let Some(rest) = line.strip_prefix("no ") {
        let mut t = rest.split_whitespace();
        let offer = t.next()?.parse().ok()?;
        let by = t.next()?.parse().ok()?;
        return t.next().is_none().then_some(Step::Passed { offer, by });
    }
    move_of(line).map(Step::Move)
}

/// One move, as a line.
fn move_line(a: &Action) -> String {
    let five = |v: &[u8; 5]| {
        v.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    match *a {
        Action::PlaceSettlement(v) => format!("ps {v}"),
        Action::PlaceRoad(e) => format!("pr {e}"),
        Action::Roll => "roll".to_string(),
        Action::Discard { player, resource } => {
            format!("dis {player} {}", res_code(resource))
        }
        Action::MoveRobber { hex, victim } => format!("rob {hex} {}", seat_code(victim)),
        Action::BuildRoad(e) => format!("br {e}"),
        Action::BuildSettlement(v) => format!("bs {v}"),
        Action::BuildCity(v) => format!("bc {v}"),
        Action::BuyDev => "buy".to_string(),
        Action::PlayMilitia => "mil".to_string(),
        Action::PlayRoadBuilding => "rdb".to_string(),
        Action::PlayInvention([a, b]) => format!("inv {} {}", res_code(a), res_code(b)),
        Action::PlayMonopoly(r) => format!("mon {}", res_code(r)),
        Action::Trade { give, take } => format!("tr {} {}", res_code(give), res_code(take)),
        Action::ProposeTrade { by, to, give, want } => {
            format!("off {by} {} {} {}", seat_code(to), five(&give), five(&want))
        }
        Action::AcceptTrade { offer, by } => format!("acc {offer} {by}"),
        Action::WithdrawTrade { offer, by } => format!("wd {offer} {by}"),
        Action::EndTurn => "end".to_string(),
    }
}

fn move_of(line: &str) -> Option<Action> {
    let mut t = line.split_whitespace();
    let five = |s: &str| -> Option<[u8; 5]> {
        let mut out = [0u8; 5];
        let mut parts = s.split(',');
        for slot in &mut out {
            *slot = parts.next()?.parse().ok()?;
        }
        parts.next().is_none().then_some(out)
    };
    let kind = t.next()?;
    let a = match kind {
        "ps" => Action::PlaceSettlement(t.next()?.parse().ok()?),
        "pr" => Action::PlaceRoad(t.next()?.parse().ok()?),
        "roll" => Action::Roll,
        "dis" => Action::Discard {
            player: t.next()?.parse().ok()?,
            resource: res_of(t.next()?)?,
        },
        "rob" => Action::MoveRobber {
            hex: t.next()?.parse().ok()?,
            victim: seat_of(t.next()?)?,
        },
        "br" => Action::BuildRoad(t.next()?.parse().ok()?),
        "bs" => Action::BuildSettlement(t.next()?.parse().ok()?),
        "bc" => Action::BuildCity(t.next()?.parse().ok()?),
        "buy" => Action::BuyDev,
        "mil" => Action::PlayMilitia,
        "rdb" => Action::PlayRoadBuilding,
        "inv" => Action::PlayInvention([res_of(t.next()?)?, res_of(t.next()?)?]),
        "mon" => Action::PlayMonopoly(res_of(t.next()?)?),
        "tr" => Action::Trade {
            give: res_of(t.next()?)?,
            take: res_of(t.next()?)?,
        },
        "off" => Action::ProposeTrade {
            by: t.next()?.parse().ok()?,
            to: seat_of(t.next()?)?,
            give: five(t.next()?)?,
            want: five(t.next()?)?,
        },
        "acc" => Action::AcceptTrade {
            offer: t.next()?.parse().ok()?,
            by: t.next()?.parse().ok()?,
        },
        "wd" => Action::WithdrawTrade {
            offer: t.next()?.parse().ok()?,
            by: t.next()?.parse().ok()?,
        },
        "end" => Action::EndTurn,
        _ => return None,
    };
    t.next().is_none().then_some(a)
}

/// A flag, as a word. `on`/`off` rather than `true`/`false` because the file is
/// meant to be read, and these are settings rather than assertions.
fn yes_no(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

/// Anything but an explicit `on` is off, so a line this build does not
/// understand leaves the setting alone rather than turning it on.
fn is_yes(s: &str) -> bool {
    s == "on"
}

fn mode_code(m: TradeMode) -> &'static str {
    match m {
        TradeMode::Disabled => "off",
        TradeMode::Restricted => "restricted",
        TradeMode::Full => "full",
    }
}

fn mode_of(s: &str) -> Option<TradeMode> {
    match s {
        "off" => Some(TradeMode::Disabled),
        "restricted" => Some(TradeMode::Restricted),
        "full" => Some(TradeMode::Full),
        _ => None,
    }
}

/// The whole file.
pub fn encode(g: &Saved) -> String {
    let mut out = String::new();
    // A version on the first line, so a file written by an older build says so
    // rather than being read as though it were this one.
    let _ = writeln!(out, "carranta {VERSION}");
    let _ = writeln!(out, "id {}", g.id);
    let _ = writeln!(out, "seats {}", g.seats);
    let _ = writeln!(out, "seed {}", g.seed);
    let _ = writeln!(out, "mode {}", mode_code(g.mode));
    let _ = writeln!(out, "name {}", g.name);
    // Omitted when there is nobody to name, so a demo game's file says nothing
    // about an owner rather than saying it has an empty one.
    if !g.by.is_empty() {
        let _ = writeln!(out, "by {}", g.by);
    }
    let _ = writeln!(out, "dealt {}", g.dealt);
    if let Some(w) = g.winner {
        let _ = writeln!(out, "winner {w}");
    }
    // The lobby's answers. Written out in full rather than only where they
    // differ from the defaults: a setting that is absent because nobody chose it
    // and one that is absent because it happens to match today's default read
    // the same in the file and stop reading the same the day a default changes.
    // The table's name is the exception, and is omitted when there is none, the
    // way `by` is: an empty name is not a name.
    let s = &g.setup;
    if !s.game.is_empty() {
        let _ = writeln!(out, "game {}", s.game);
    }
    let _ = writeln!(out, "public {}", yes_no(s.public));
    let _ = writeln!(out, "pace {}", s.pace.name());
    // Kind, seconds and increment on one line, which is exactly what `Clock`
    // parses: the file says what the lobby said.
    let _ = writeln!(
        out,
        "clock {} {} {}",
        s.clock.name(),
        s.clock.secs(),
        s.clock.increment()
    );
    let _ = writeln!(out, "discard {}", s.discard_secs);
    let _ = writeln!(out, "bank {}", if s.bank_exact { "exact" } else { "rough" });
    let _ = writeln!(out, "log {}", yes_no(s.log));
    let _ = writeln!(out, "chat {}", yes_no(s.chat));
    // One line a seat, in seat order, because a name is somebody else's text and
    // has spaces and commas in it: everything after the first word is the name,
    // so there is nothing to escape and nothing to get wrong.
    //
    // Written for every table now, including one the server played itself. Four
    // lines of `bot` used to say the same thing as no lines at all in more
    // words, and stopped the day the line named a build: a self-play corpus is
    // precisely where "which version played this" is the question.
    //
    // `gone` rather than `chair` for somebody who was not at the table when this
    // was written, which for the last write of a finished game is who was not
    // there at the end. A second keyword rather than a fourth field because the
    // name runs to the end of the line and there is nowhere after it to put
    // anything.
    for c in &s.chairs {
        let word = if c.left { "gone" } else { "chair" };
        if c.who == "bot" {
            let _ = writeln!(out, "{word} bot {}", c.agent);
        } else {
            let _ = writeln!(out, "{word} {} {}", c.who, c.name);
        }
    }
    for step in &g.moves {
        let _ = writeln!(out, "{}", step_line(step));
    }
    // The clock, after the game rather than through it, so the moves still read
    // as a list of moves. Wrapped because a nine-hundred-move game is a
    // nine-hundred-number line otherwise, and a file you can open is the point.
    for chunk in g.times.chunks(TIMES_PER_LINE) {
        let _ = writeln!(
            out,
            "at {}",
            chunk
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    out
}

pub fn decode(text: &str) -> Option<Saved> {
    let mut g = Saved {
        id: String::new(),
        seats: 4,
        seed: 0,
        mode: TradeMode::Full,
        name: String::new(),
        by: String::new(),
        dealt: 0,
        winner: None,
        // The defaults, so a file written before version 4 comes back as a table
        // set up the way a fresh one is rather than as an unreadable file.
        setup: Setup::default(),
        moves: Vec::new(),
        times: Vec::new(),
    };
    let mut version = None;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        let (head, rest) = line.split_once(' ').unwrap_or((line, ""));
        match head {
            "carranta" => version = rest.parse::<u32>().ok(),
            "id" => g.id = rest.to_string(),
            "seats" => g.seats = rest.parse().ok()?,
            "seed" => g.seed = rest.parse().ok()?,
            "mode" => g.mode = mode_of(rest)?,
            "name" => g.name = rest.to_string(),
            "by" => g.by = rest.to_string(),
            "dealt" => g.dealt = rest.parse().ok()?,
            "winner" => g.winner = Some(rest.parse().ok()?),
            "game" => g.setup.game = rest.to_string(),
            "public" => g.setup.public = is_yes(rest),
            "pace" => g.setup.pace = Pace::parse(Some(rest)),
            "clock" => {
                // Kind, seconds, increment: whatever `Clock::parse` is given, so
                // the file cannot describe a clock the lobby could not ask for.
                let mut t = rest.split_whitespace();
                let kind = t.next()?;
                let secs = t.next()?.parse().ok()?;
                let inc = t.next()?.parse().ok()?;
                g.setup.clock = Clock::parse(Some(kind), secs, inc);
            }
            "discard" => g.setup.discard_secs = rest.parse().ok()?,
            "bank" => g.setup.bank_exact = rest != "rough",
            "log" => g.setup.log = is_yes(rest),
            "chat" => g.setup.chat = is_yes(rest),
            // Version 5 wrote them on one line and had no names in them. Read
            // rather than dropped: a table somebody is sitting at is exactly the
            // thing that must not be lost to a format change.
            "chairs" => {
                g.setup.chairs = rest
                    .split(',')
                    .map(|w| Chair {
                        who: w.to_string(),
                        ..Default::default()
                    })
                    .collect();
            }
            // A person who was at the table when this was written, and one who
            // was not. Otherwise the same line.
            "chair" | "gone" => {
                let (who, rest) = rest.split_once(' ').unwrap_or((rest, ""));
                // A bot's line carries its build where a person's carries their
                // name. Before version 8 it carried nothing, which meant the one
                // build there was, and `agent_id` reads an empty string as that.
                let bot = who == "bot";
                g.setup.chairs.push(Chair {
                    who: who.to_string(),
                    name: if bot { String::new() } else { rest.to_string() },
                    agent: if bot { rest.to_string() } else { String::new() },
                    left: head == "gone",
                });
            }
            "at" => {
                for n in rest.split(',') {
                    g.times.push(n.parse().ok()?);
                }
            }
            _ => g.moves.push(step_of(line)?),
        }
    }
    // A version this build knows, and a clock that is either whole or absent:
    // times that have fallen out of step with the moves would be attributed to
    // the wrong turns, which is worse than having none.
    let known = version.is_some_and(|v| (1..=VERSION).contains(&v));
    let timed = g.times.is_empty() || g.times.len() == g.moves.len();
    (known && timed).then_some(g)
}

/// Where games live.
#[derive(Clone, Debug)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let _ = std::fs::create_dir_all(&dir);
        Store { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, id: &str) -> Option<PathBuf> {
        // An id from a URL is somebody else's text. Checked rather than
        // escaped, because the shape is narrow enough to check exactly and a
        // path built from unchecked text is how a server reads its own disk.
        is_game_id(id).then(|| self.dir.join(format!("{id}.carranta")))
    }

    pub fn save(&self, g: &Saved) -> std::io::Result<()> {
        let path = self
            .path(&g.id)
            .ok_or_else(|| std::io::Error::other("not a game id"))?;
        std::fs::write(path, encode(g))
    }

    pub fn load(&self, id: &str) -> Option<Saved> {
        let path = self.path(id)?;
        decode(&std::fs::read_to_string(path).ok()?)
    }

    /// Throw a game away.
    ///
    /// For the one case that earns it: a file this build can no longer replay,
    /// which is not a game any more but a row on the home page that refuses to
    /// open. Keeping it helps nobody, and the analytics cannot read it either.
    pub fn remove(&self, id: &str) -> bool {
        self.path(id)
            .is_some_and(|p| std::fs::remove_file(p).is_ok())
    }

    /// Every game on disk, newest first.
    pub fn all(&self) -> Vec<Saved> {
        let mut out: Vec<Saved> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                (path.extension()? == "carranta")
                    .then(|| decode(&std::fs::read_to_string(&path).ok()?))
                    .flatten()
            })
            .collect();
        out.sort_by(|a, b| b.dealt.cmp(&a.dealt).then_with(|| b.id.cmp(&a.id)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of every action, so the codec is exercised on all sixteen rather
    /// than on the handful an ordinary game happens to contain.
    fn one_of_each() -> Vec<Step> {
        [
            Action::PlaceSettlement(17),
            Action::PlaceRoad(41),
            Action::Roll,
            Action::Discard {
                player: 2,
                resource: Resource::Wheat,
            },
            Action::MoveRobber {
                hex: 9,
                victim: Some(3),
            },
            Action::MoveRobber {
                hex: 0,
                victim: None,
            },
            Action::BuildRoad(5),
            Action::BuildSettlement(6),
            Action::BuildCity(7),
            Action::BuyDev,
            Action::PlayMilitia,
            Action::PlayRoadBuilding,
            Action::PlayInvention([Resource::Ore, Resource::Brick]),
            Action::PlayMonopoly(Resource::Wool),
            Action::Trade {
                give: Resource::Wood,
                take: Resource::Ore,
            },
            Action::ProposeTrade {
                by: 1,
                to: Some(0),
                give: [1, 0, 2, 0, 0],
                want: [0, 3, 0, 0, 1],
            },
            Action::ProposeTrade {
                by: 0,
                to: None,
                give: [0, 0, 0, 0, 1],
                want: [1, 0, 0, 0, 0],
            },
            Action::AcceptTrade { offer: 2, by: 3 },
            Action::WithdrawTrade { offer: 0, by: 1 },
            Action::EndTurn,
        ]
        .into_iter()
        .map(Step::Move)
        // And the one step that is not a move.
        .chain([Step::Passed { offer: 1, by: 2 }])
        .collect()
    }

    #[test]
    fn every_step_survives_the_round_trip() {
        for step in one_of_each() {
            let line = step_line(&step);
            assert_eq!(step_of(&line), Some(step), "{line}");
        }
    }

    #[test]
    fn a_whole_game_survives_the_round_trip() {
        let g = Saved {
            id: game_id(12_345_678),
            seats: 4,
            seed: 99,
            mode: TradeMode::Restricted,
            name: "Egon".to_string(),
            by: String::new(),
            dealt: 1_755_300_000,
            winner: Some(2),
            setup: Setup::default(),
            moves: one_of_each(),
            times: Vec::new(),
        };
        assert_eq!(decode(&encode(&g)), Some(g));
    }

    #[test]
    fn the_lobby_s_answers_survive_the_round_trip() {
        // Every one of them different from its default, so a setting that is
        // written but never read, or read into the wrong field, fails here
        // rather than showing up as a table that came back with the wrong clock.
        let g = Saved {
            id: game_id(4),
            seats: 3,
            seed: 7,
            mode: TradeMode::Full,
            name: "Egon".to_string(),
            by: "keytest0000000000".to_string(),
            dealt: 9,
            winner: None,
            setup: Setup {
                game: "Kitchen table".to_string(),
                public: true,
                pace: Pace::Slow,
                clock: Clock::Chess {
                    bank: 300,
                    increment: 5,
                },
                discard_secs: 25,
                bank_exact: false,
                log: false,
                chat: true,
                chairs: vec![
                    Chair::person("keytest0000000000", "Egon of the Long Name, and a comma"),
                    Chair::bot(),
                    Chair::open(),
                ],
            },
            moves: vec![Step::Move(Action::Roll)],
            times: vec![4],
        };
        let back = decode(&encode(&g)).expect("a whole game");
        assert_eq!(back, g);
        // And the file says so in words, because a file you can open is the
        // point of this format.
        let text = encode(&g);
        for line in [
            "game Kitchen table",
            "public on",
            "pace slow",
            "clock chess 300 5",
            "discard 25",
            "bank rough",
            "log off",
        ] {
            assert!(text.contains(line), "the file says `{line}`");
        }
    }

    #[test]
    fn a_chair_says_which_build_played_it_and_who_was_still_there() {
        // The two things version 8 added, both of which the rating needs and
        // neither of which the moves can carry.
        let g = Saved {
            id: game_id(11),
            seats: 4,
            seed: 3,
            mode: TradeMode::Full,
            name: "Marta".to_string(),
            by: "keytest0000000000".to_string(),
            dealt: 9,
            winner: Some(2),
            setup: Setup {
                chairs: vec![
                    Chair::bot(),
                    Chair {
                        left: true,
                        ..Chair::person("keytest0000000000", "Marta")
                    },
                    Chair::person("otherkey00000000", "Vidal"),
                    Chair {
                        agent: "llm-fable@3".to_string(),
                        ..Chair::bot()
                    },
                ],
                ..Default::default()
            },
            moves: vec![Step::Move(Action::Roll)],
            times: vec![4],
        };
        let text = encode(&g);
        assert_eq!(decode(&text).expect("a whole game"), g);
        for line in [
            "chair bot house@1",
            "gone keytest0000000000 Marta",
            "chair otherkey00000000 Vidal",
            "chair bot llm-fable@3",
        ] {
            assert!(text.contains(line), "the file says `{line}`:\n{text}");
        }
    }

    #[test]
    fn a_named_agent_chair_reads_back_as_that_player() {
        // The constructor a trained champion's chair goes through: the string
        // is the identity, so it has to survive the split intact.
        let c = Chair::bot_as("trained@12");
        assert!(!c.is_person());
        assert_eq!(c.agent_id(), ("trained".to_string(), 12));
        assert_eq!(Chair::bot().agent_id(), ("house".to_string(), 1));
    }

    #[test]
    fn a_file_from_before_the_builds_were_named_reads_as_the_one_there_was() {
        // Every file before version 8 wrote the bare word `bot`, at a time when
        // there was exactly one thing it could mean. Reading it as anything else
        // would silently split the house bot's record in two.
        let text = "carranta 7\nid 9222-2222-2222\nseats 4\nseed 3\nmode full\n\
                    name Egon\ndealt 1\nchair bot \nchair sd2v5zlwmnmgxdfw Egon\n\
                    chair bot \nchair bot \n";
        let g = decode(text).expect("an older file");
        assert_eq!(g.setup.chairs[0].agent_id(), ("house".to_string(), 1));
        assert!(!g.setup.chairs[0].is_person(), "a bot");
        assert!(g.setup.chairs[1].is_person(), "and a person at seat two");
        assert_eq!(g.setup.chairs[1].name, "Egon");
        assert!(
            g.setup.chairs.iter().all(|c| !c.left),
            "and nobody left, which is what a file that does not say meant: it \
             was written under a rule that stops a table as soon as nobody is at \
             it, so every game in one was finished by somebody"
        );
    }

    #[test]
    fn a_setting_is_written_even_when_it_matches_the_default() {
        // A setting absent because nobody chose it and one absent because it
        // happens to match today's default read the same in the file, and stop
        // reading the same the day a default changes.
        let g = Saved {
            id: game_id(5),
            seats: 4,
            seed: 1,
            mode: TradeMode::Full,
            name: String::new(),
            by: String::new(),
            dealt: 0,
            winner: None,
            setup: Setup::default(),
            moves: Vec::new(),
            times: Vec::new(),
        };
        let text = encode(&g);
        for head in ["public ", "pace ", "clock ", "discard ", "bank ", "log "] {
            assert!(text.contains(head), "the file says `{head}`");
        }
        // The table's name is the exception: an empty name is not a name, the
        // same way an empty owner is not one.
        assert!(!text.contains("\ngame "), "and says nothing about a name");
    }

    #[test]
    fn a_game_written_before_the_settings_were_kept_still_reads() {
        // Version 3 knew the game and nothing about the table it was played at.
        // It comes back as a table set up the way a fresh one is, which is what
        // it was doing before any of this was written down.
        let text = "carranta 3\nid 9222-2222-2222\nseats 4\nseed 3\nmode full\n\
                    name Egon\ndealt 5\nroll\nat 7\n";
        let g = decode(text).expect("an older file is still a game");
        assert_eq!(g.setup, Setup::default());
        assert_eq!(g.moves.len(), 1);
    }

    #[test]
    fn a_game_still_being_played_has_no_winner() {
        let mut g = Saved {
            id: game_id(1),
            seats: 3,
            seed: 1,
            mode: TradeMode::Full,
            name: String::new(),
            by: String::new(),
            dealt: 1,
            winner: None,
            setup: Setup::default(),
            moves: vec![Step::Move(Action::Roll)],
            times: vec![7],
        };
        let back = decode(&encode(&g)).expect("it reads back");
        assert_eq!(back.winner, None);
        g.winner = Some(0);
        assert_eq!(decode(&encode(&g)).unwrap().winner, Some(0));
    }

    #[test]
    fn a_file_from_another_version_is_not_read_as_this_one() {
        let g = Saved {
            id: game_id(7),
            seats: 4,
            seed: 3,
            mode: TradeMode::Full,
            name: String::new(),
            by: String::new(),
            dealt: 0,
            winner: None,
            setup: Setup::default(),
            moves: vec![Step::Move(Action::Roll)],
            times: vec![7],
        };
        let text = encode(&g);
        assert!(decode(&text).is_some());
        // A version this build has never heard of, and no version at all. Named
        // against `VERSION` rather than written out, so this keeps testing what
        // it says it tests the next time the format grows a line.
        let mine = format!("carranta {VERSION}");
        let future = format!("carranta {}", VERSION + 1);
        assert_eq!(decode(&text.replace(&mine, &future)), None);
        assert_eq!(decode(&text.replace(&format!("{mine}\n"), "")), None);
    }

    #[test]
    fn a_game_written_before_it_belonged_to_anybody_still_reads() {
        // Version 3 added `by`. A version 2 file has no owner rather than an
        // unreadable one, which is the same promise version 2 made about the
        // clock: an addition beside the moves, not inside them.
        let text = "carranta 2\nid 9222-2222-2222\nseats 4\nseed 3\nmode full\n\
                    name Egon\ndealt 5\nroll\nat 7\n";
        let g = decode(text).expect("an older file is still a game");
        assert_eq!(g.name, "Egon");
        assert_eq!(g.by, "", "and it belongs to nobody");
        assert_eq!(g.moves.len(), 1);
    }

    #[test]
    fn a_game_written_before_there_was_a_clock_still_reads() {
        // The whole reason the times are beside the moves rather than inside
        // them: a version 1 file is a whole game that happens not to know when
        // anything happened, and refusing it would lose the game to keep the
        // clock.
        let g = Saved {
            id: game_id(11),
            seats: 4,
            seed: 3,
            mode: TradeMode::Full,
            name: "Egon".to_string(),
            by: String::new(),
            dealt: 5,
            winner: Some(1),
            setup: Setup::default(),
            moves: vec![Step::Move(Action::Roll), Step::Move(Action::EndTurn)],
            times: vec![120, 340],
        };
        let old = encode(&g)
            .replace("carranta 2", "carranta 1")
            .lines()
            .filter(|l| !l.starts_with("at "))
            .fold(String::new(), |mut acc, l| {
                acc.push_str(l);
                acc.push('\n');
                acc
            });
        let back = decode(&old).expect("an older game is still a game");
        assert_eq!(back.moves, g.moves);
        assert_eq!(back.winner, Some(1));
        // Empty rather than zeroes: nobody knows is not the same as no time.
        assert!(back.times.is_empty());
        // And a clock that has fallen out of step with the moves is refused
        // outright, since the wrong seconds on the wrong turns is a wrong
        // answer where none at all is only a missing one.
        let short = encode(&g).replace("at 120,340", "at 120");
        assert_eq!(decode(&short), None);
    }

    #[test]
    fn a_line_nobody_wrote_is_not_guessed_at() {
        // A corrupt file is refused whole rather than read up to the damage:
        // half a game replays into a position nobody played.
        let g = Saved {
            id: game_id(7),
            seats: 4,
            seed: 3,
            mode: TradeMode::Full,
            name: String::new(),
            by: String::new(),
            dealt: 0,
            winner: None,
            setup: Setup::default(),
            moves: vec![Step::Move(Action::Roll), Step::Move(Action::EndTurn)],
            times: vec![1, 2],
        };
        let text = encode(&g);
        assert_eq!(decode(&text.replace("roll", "rolll")), None);
        assert_eq!(decode(&text.replace("roll", "dis 9")), None);
        // Trailing rubbish on a line that is otherwise fine, too.
        assert_eq!(decode(&text.replace("end", "end 4")), None);
    }

    #[test]
    fn an_id_is_three_groups_and_nothing_else_passes() {
        let id = game_id(918_273_645);
        assert_eq!(id.len(), 14);
        assert!(is_game_id(&id), "{id}");
        // The shapes a URL might arrive in, none of which may reach the disk.
        for bad in [
            "",
            "abcd-efgh-ijk",
            "abcd-efgh-ijklm",
            "../../etc/passwd",
            "abcd/efgh/ijkl",
            "abcd.efgh.ijkl",
            "abcd-efgh-ijk.",
        ] {
            assert!(!is_game_id(bad), "{bad} is not an id");
        }
        let store = Store::new(std::env::temp_dir().join("carranta-id-test"));
        assert_eq!(store.load("../../etc/passwd"), None);
    }

    #[test]
    fn different_numbers_are_different_ids() {
        let a = game_id(1);
        let b = game_id(2);
        let c = game_id(u64::MAX);
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert!(is_game_id(&c));
    }

    #[test]
    fn a_played_game_survives_a_trip_through_a_file() {
        // The whole point, end to end: play one, write it, read it back, and
        // get the same game with the same account of it.
        use crate::game::Session;
        let dir = std::env::temp_dir().join(format!("carranta-trip-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::new(&dir);
        for seed in 0..6u64 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..400 {
                let v = s.version();
                if s.choices().is_empty() || s.act(0, v).is_err() {
                    break;
                }
            }
            let (seats, dealt, mode) = s.table();
            let g = Saved {
                id: game_id(seed),
                seats,
                seed: dealt,
                mode,
                name: "Egon".to_string(),
                by: String::new(),
                dealt: seed,
                winner: s.winner(),
                setup: Setup::default(),
                moves: s.moves().to_vec(),
                times: s.times().to_vec(),
            };
            store.save(&g).expect("it writes");
            let back = store.load(&g.id).expect("it reads");
            assert_eq!(back, g, "seed {seed}: the file is the game");
            let session = Session::resume(back.seats, back.seed, back.mode, &back.moves)
                .expect("and the game replays");
            let said = |x: &Session| x.log().iter().map(|l| l.text.clone()).collect::<Vec<_>>();
            assert_eq!(said(&session), said(&s), "seed {seed}: the same account");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_store_writes_reads_and_lists_newest_first() {
        let dir = std::env::temp_dir().join(format!("carranta-store-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::new(&dir);
        let mut ids = Vec::new();
        for (n, dealt) in [(1u64, 300u64), (2, 100), (3, 200)] {
            let g = Saved {
                id: game_id(n),
                seats: 4,
                seed: n,
                mode: TradeMode::Full,
                name: "Egon".to_string(),
                by: String::new(),
                dealt,
                winner: None,
                setup: Setup::default(),
                moves: vec![Step::Move(Action::Roll)],
                times: vec![3],
            };
            store.save(&g).expect("it writes");
            ids.push(g.id);
        }
        assert_eq!(store.load(&ids[0]).unwrap().seed, 1);
        assert_eq!(store.load("zzzz-zzzz-zzzz"), None);
        let all = store.all();
        assert_eq!(all.len(), 3);
        assert_eq!(
            all.iter().map(|g| g.dealt).collect::<Vec<_>>(),
            vec![300, 200, 100],
            "newest first"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
