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

use crate::game::Step;

/// What one file says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub id: String,
    pub seats: u8,
    pub seed: u64,
    pub mode: TradeMode,
    /// What the person at this table is called, for the analytics to name them.
    pub name: String,
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
    pub moves: Vec<Step>,
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
    let _ = writeln!(out, "carranta 1");
    let _ = writeln!(out, "id {}", g.id);
    let _ = writeln!(out, "seats {}", g.seats);
    let _ = writeln!(out, "seed {}", g.seed);
    let _ = writeln!(out, "mode {}", mode_code(g.mode));
    let _ = writeln!(out, "name {}", g.name);
    let _ = writeln!(out, "dealt {}", g.dealt);
    if let Some(w) = g.winner {
        let _ = writeln!(out, "winner {w}");
    }
    for step in &g.moves {
        let _ = writeln!(out, "{}", step_line(step));
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
        dealt: 0,
        winner: None,
        moves: Vec::new(),
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
            "dealt" => g.dealt = rest.parse().ok()?,
            "winner" => g.winner = Some(rest.parse().ok()?),
            _ => g.moves.push(step_of(line)?),
        }
    }
    (version == Some(1)).then_some(g)
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
            dealt: 1_755_300_000,
            winner: Some(2),
            moves: one_of_each(),
        };
        assert_eq!(decode(&encode(&g)), Some(g));
    }

    #[test]
    fn a_game_still_being_played_has_no_winner() {
        let mut g = Saved {
            id: game_id(1),
            seats: 3,
            seed: 1,
            mode: TradeMode::Full,
            name: String::new(),
            dealt: 1,
            winner: None,
            moves: vec![Step::Move(Action::Roll)],
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
            dealt: 0,
            winner: None,
            moves: vec![Step::Move(Action::Roll)],
        };
        let text = encode(&g);
        assert!(decode(&text).is_some());
        assert_eq!(decode(&text.replace("carranta 1", "carranta 2")), None);
        assert_eq!(decode(&text.replace("carranta 1\n", "")), None);
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
            dealt: 0,
            winner: None,
            moves: vec![Step::Move(Action::Roll), Step::Move(Action::EndTurn)],
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
                dealt: seed,
                winner: s.winner(),
                moves: s.moves().to_vec(),
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
                dealt,
                winner: None,
                moves: vec![Step::Move(Action::Roll)],
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
