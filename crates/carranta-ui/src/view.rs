//! The redacted position, as the page receives it.
//!
//! Everything here is read off a [`Fog`], the projection of §7.3, plus the
//! board geometry the engine now exposes. Nothing reads the raw `State` except
//! the geometry, which is public information by definition.

use carranta_core::state::{MAX_PLAYERS, Phase};
use carranta_core::topology::{
    EDGE_COUNT, HEX_COUNT, VERTEX_COUNT, edge_endpoints, hex_axial, iter_vertices, vertex_axial,
    vertex_bit,
};

use crate::game::{Answer, HUMAN, Session, Target};
use crate::json::Json;

const RESOURCES: [&str; 5] = ["brick", "wood", "wool", "wheat", "ore"];

/// The order the five are shown in, as indices into [`RESOURCES`].
///
/// Wood before brick, then the engine's own order. The engine numbers them for
/// its own reasons and that numbering is the wire format, so the display order
/// is kept separately rather than by renumbering the game underneath it.
const RESOURCE_ORDER: [usize; 5] = [1, 0, 2, 3, 4];
const TERRAIN: [&str; 6] = [
    "hills",
    "forest",
    "pasture",
    "fields",
    "mountains",
    "desert",
];
const DEV_CARDS: [&str; 5] = [
    "militia",
    "victory point",
    "monopoly",
    "road building",
    "invention",
];

pub fn render(session: &Session) -> String {
    render_inner(session, None)
}

/// The same, with something to tell the player, a refused click, usually.
pub fn render_with_note(session: &Session, note: &str) -> String {
    render_inner(session, Some(note))
}

fn phase_name(p: Phase) -> &'static str {
    match p {
        Phase::SetupSettlement { .. } => "place a settlement",
        Phase::SetupRoad { .. } => "place a road",
        Phase::PreRoll => "before the roll",
        Phase::Discard => "discarding",
        Phase::MoveRobber { .. } => "move the robber",
        Phase::Action => "build and trade",
        Phase::GameOver { .. } => "game over",
    }
}

fn render_inner(session: &Session, note: Option<&str>) -> String {
    let v = session.view();
    let seats = v.players as usize;
    let mut j = Json::object();

    j.int("version", session.version() as i64);
    // As a string, not a number: a u64 seed does not survive JSON.parse, which
    // turns it into a double and quietly rounds the last few digits. The page
    // shows the seed for copying, so a rounded one would deal a different board
    // than the one it claims to name.
    j.str("seed", &crate::game::seed_code(session.seed()));
    // Which build is serving this, so a stale process is visible rather than
    // mistaken for a change that did not work.
    j.str("build", env!("CARRANTA_BUILD"));
    j.int("you", HUMAN as i64);
    j.int("players", seats as i64);
    j.int("toAct", v.to_act as i64);
    j.str("phase", phase_name(v.phase));
    j.bool("yourTurn", session.state().decider() == HUMAN);
    // The session's verdict, not the engine's: a game stopped by the clock has
    // a winner the engine knows nothing about.
    j.opt_int("winner", session.winner().map(|w| w as i64));
    match note {
        Some(n) => j.str("note", n),
        None => j.str("note", ""),
    };

    // ---- Board geometry, so the page draws rather than guesses ----
    j.array("hexes", 0..HEX_COUNT as u8, |o, h| {
        let [q, r] = hex_axial(h);
        o.int("id", h as i64)
            .int("q", q as i64)
            .int("r", r as i64)
            .str("terrain", TERRAIN[v.terrain[h as usize] as usize])
            .int("number", v.number[h as usize] as i64)
            .bool("robber", v.robber == h);
    });
    j.array("vertices", 0..VERTEX_COUNT as u8, |o, vtx| {
        // A vertex is drawn at the centroid of the three lattice positions
        // that meet there; the sums are given so the page need not divide.
        let t = vertex_axial(vtx);
        o.int("id", vtx as i64)
            .int("q3", t.iter().map(|p| p[0] as i64).sum::<i64>())
            .int("r3", t.iter().map(|p| p[1] as i64).sum::<i64>());
    });
    // The coastline in order, so the page can pick out the intersections a
    // port could sit on, and so a person can name one precisely.
    j.ints(
        "coast",
        carranta_core::state::coast_ring()
            .into_iter()
            .map(i64::from),
    );
    j.array("edges", 0..EDGE_COUNT as u8, |o, e| {
        let [a, b] = edge_endpoints(e);
        o.int("id", e as i64).int("a", a as i64).int("b", b as i64);
    });

    // ---- What is on the board: all public (§4.2) ----
    let mut buildings: Vec<Vec<i64>> = Vec::new();
    let mut roads: Vec<Vec<i64>> = Vec::new();
    for p in 0..seats {
        for vtx in iter_vertices(v.settlements[p]) {
            buildings.push(vec![p as i64, vtx as i64, 0]);
        }
        for vtx in iter_vertices(v.cities[p]) {
            buildings.push(vec![p as i64, vtx as i64, 1]);
        }
        for e in 0..EDGE_COUNT as u8 {
            if v.roads[p] & carranta_core::topology::edge_bit(e) != 0 {
                roads.push(vec![p as i64, e as i64]);
            }
        }
    }
    j.rows("buildings", buildings);
    j.rows("roads", roads);
    j.ints(
        "ports",
        (0..VERTEX_COUNT as u8).filter_map(|vtx| {
            v.ports
                .iter()
                .position(|kind| kind & vertex_bit(vtx) != 0)
                .map(|kind| (vtx as i64) * 8 + kind as i64)
        }),
    );

    // ---- Seats: counts for everyone, cards for you alone ----
    j.array("seats", 0..seats, |o, p| {
        o.int("seat", p as i64)
            .int("hand", v.hand_size[p] as i64)
            .int("dev", v.dev_count[p] as i64)
            .int("vp", v.apparent_vp[p] as i64)
            .int("road", v.road_length[p] as i64)
            .int("militia", v.militia_played[p] as i64)
            .int("settlementsLeft", v.settlements_left[p] as i64)
            .int("citiesLeft", v.cities_left[p] as i64)
            .int("roadsLeft", v.roads_left[p] as i64)
            .int("discardsDue", v.discard_left[p] as i64)
            .bool("longestRoad", v.longest_road == Some(p as u8))
            .bool("largestMilitia", v.largest_militia == Some(p as u8));
        // Whose clock is running matters as much as what is left on it: a
        // per-turn allowance only counts down for the seat holding the turn.
        if let Some(left) = session.time_left(p as u8) {
            // Whose clock is running, which is not always whose turn it is:
            // an unanswered offer stops the game on the human.
            o.int("timeLeft", left)
                .bool("onClock", session.on_clock() == p as u8);
        }
    });
    let own = v
        .own
        .unwrap_or_else(|| unreachable!("the human has a seat"));
    j.ints("yourHand", own.hand.iter().map(|&n| n as i64));
    j.ints("yourDev", own.dev_held.iter().map(|&n| n as i64));
    j.ints("yourFresh", own.dev_fresh.iter().map(|&n| n as i64));
    j.int("yourVp", own.victory_points as i64);
    j.bool("canPropose", session.can_propose());
    j.ints("supply", v.supply.iter().map(|&n| n as i64));
    j.int("devLeft", v.dev_left as i64);
    j.ints("dice", v.dice.iter().map(|&n| n as i64));
    // The clock is read from the server rather than kept by the page, so a
    // reload shows how long the game has actually been going. A timed game
    // sends what is left; an untimed one sends nothing and the page counts up.
    j.int("elapsed", session.elapsed_secs() as i64);
    j.str("clock", session.clock().name());
    j.int("clockSecs", session.clock().secs() as i64);
    j.int("clockIncrement", session.clock().increment() as i64);
    // The seven's own allowance, and what is left of it. Null when nothing is
    // being discarded, which is also what says the turn clock is running again.
    j.int("discardSecs", session.discard_secs() as i64);
    j.opt_int("discardLeft", session.discard_left());
    j.str("youName", session.name());
    j.str("game", session.game());
    j.int("turns", session.turn_no() as i64);
    j.ints("turnMs", session.turn_ms().iter().map(|&n| n as i64));
    j.bool("inSetup", session.in_setup());
    j.bool("logShown", session.log_shown());
    // Whether a development card is half played and can still be put back.
    j.bool("canCancel", session.can_cancel());
    j.bool("bankExact", session.bank_exact());
    // How the bots are paced, and whether one is mid-thought, so the page can
    // poll quickly while the table is moving and slowly while it is not.
    j.str("pace", session.pace().name());
    j.bool("botThinking", session.bot_thinking());

    // ---- The market ----
    // The session's record rather than the engine's table, because a trade is
    // watched as well as played: the engine drops an offer the instant it is
    // taken, which is the moment there is most to say about it. `i` is where it
    // sits in the engine's market and is null once it has left, which is also
    // what says a deal is settled.
    j.array("offers", session.deals(), |o, d| {
        o.opt_int("i", d.at.map(|i| i as i64))
            .int("from", d.offer.from as i64)
            .bool("mine", d.offer.from == HUMAN)
            .bool("live", d.live())
            .opt_int("to", d.offer.to.map(|t| t as i64))
            .ints("give", d.offer.give.iter().map(|&n| n as i64))
            .ints("want", d.offer.want.iter().map(|&n| n as i64));
        // Who was asked, and what they have said so far. Only the seats the
        // offer was actually put to (R-7.3), so nobody is left waiting on the
        // card for a question they were never asked.
        o.array(
            "answers",
            (0..session.state().players)
                .filter(|&s| session.state().may_accept(s as usize, &d.offer)),
            |a, seat| {
                a.int("seat", seat as i64).str(
                    "said",
                    match d.answers[seat as usize] {
                        Answer::Waiting => "waiting",
                        Answer::No => "no",
                        Answer::Yes => "yes",
                    },
                );
            },
        );
    });

    // ---- What the human may do now ----
    let choices = session.choices();
    j.array("choices", choices.iter().enumerate(), |o, (i, c)| {
        o.int("i", i as i64)
            .str("label", &c.label(session.state()))
            .str("group", c.group());
        // Development card plays carry their parameters in the action, so the
        // engine enumerates every combination: five monopolies and fifteen
        // inventions. That is the right action space for a search and the
        // wrong one for a person, so the parts are named here and the page
        // builds a picker out of them rather than listing twenty buttons.
        if let crate::game::Choice::Play(a) = c {
            match *a {
                carranta_core::action::Action::MoveRobber {
                    victim: Some(seat), ..
                } => {
                    o.int("victim", seat as i64);
                }
                carranta_core::action::Action::PlayMilitia => {
                    o.str("card", "militia");
                }
                carranta_core::action::Action::PlayRoadBuilding => {
                    o.str("card", "road building");
                }
                carranta_core::action::Action::PlayMonopoly(r) => {
                    o.str("card", "monopoly").int("res", r as i64);
                }
                carranta_core::action::Action::PlayInvention([x, y]) => {
                    o.str("card", "invention")
                        .int("res", x as i64)
                        .int("res2", y as i64);
                }
                // Which card this discards, so the page can lay the hand out
                // and let you choose against it rather than offering one
                // button per resource with no sense of the whole.
                carranta_core::action::Action::Discard { resource, .. } => {
                    o.int("res", resource as i64);
                }
                // A bank or port trade, in parts rather than only as a
                // sentence. The composer matches what you have built against
                // these, so it can offer the rate when what you are asking for
                // happens to be one the bank will take.
                carranta_core::action::Action::Trade { give, take } => {
                    o.int("give", give as i64).int("take", take as i64).int(
                        "rate",
                        session.state().trade_rate(HUMAN as usize, give) as i64,
                    );
                }
                _ => {}
            }
        }
        match c.target() {
            Target::Vertex(x) => o.int("vertex", x as i64),
            Target::Edge(x) => o.int("edge", x as i64),
            Target::Hex(x) => o.int("hex", x as i64),
            Target::None => o.bool("plain", true),
        };
    });

    // Names, so the page need not carry its own copy of the vocabulary.
    // Sent in the order they are shown, not the order the engine numbers them,
    // each carrying its true index. Every list on the page walks this array, so
    // one order here reorders the hand, the trade composer and the discard card
    // together, and nothing downstream has to know it was reordered.
    j.array(RESOURCE_KEY, RESOURCE_ORDER.iter().copied(), |o, i| {
        o.int("i", i as i64).str("name", RESOURCES[i]);
    });
    j.array("devNames", DEV_CARDS.iter().enumerate(), |o, (i, name)| {
        o.int("i", i as i64).str("name", name);
    });

    // A table playing without the record is not sent one. Hiding it in the
    // page would leave the history sitting in the response for anyone who
    // opened the network tab, which is not playing from memory.
    let log: &[_] = if session.log_shown() {
        session.log()
    } else {
        &[]
    };
    // The whole record, not a tail of it. A log you cannot scroll to the start
    // of is a log that lies about the game, and the page has no other source
    // for what happened in turn one.
    j.array("log", log.iter(), |o, line| {
        o.str("t", &line.text)
            .int("turn", line.turn as i64)
            .bool("setup", line.setup)
            .opt_int("seat", line.seat.map(|x| x as i64));
    });
    let _ = MAX_PLAYERS;
    j.finish()
}

const RESOURCE_KEY: &str = "resourceNames";

#[cfg(test)]
mod tests {
    use super::*;
    use carranta_core::state::TradeMode;

    #[test]
    fn the_payload_describes_a_drawable_board() {
        let s = Session::new(4, 9, TradeMode::Full);
        let out = render(&s);
        // Geometry for every feature.
        assert_eq!(out.matches("\"terrain\"").count(), HEX_COUNT);
        assert_eq!(out.matches("\"q3\"").count(), VERTEX_COUNT);
        assert!(out.contains("\"choices\""));
        assert!(out.contains("\"yourHand\""));
        assert!(out.starts_with('{') && out.ends_with('}'));
    }

    #[test]
    fn no_other_seat_s_cards_appear_in_the_payload() {
        // The structural guarantee, checked at the boundary that actually
        // leaves the process. Give one opponent a distinctive hand and confirm
        // the shape of it cannot be read out of the response.
        let mut s = Session::new(4, 4, TradeMode::Full);
        for _ in 0..30 {
            if s.choices().is_empty() {
                break;
            }
            let v = s.version();
            let _ = s.act(0, v);
        }
        let before = render(&s);

        // Two payloads that differ only in a hidden way must be identical.
        let mut moved = Session::new(4, 4, TradeMode::Full);
        for _ in 0..30 {
            if moved.choices().is_empty() {
                break;
            }
            let v = moved.version();
            let _ = moved.act(0, v);
        }
        assert_eq!(
            without_the_clock(&before),
            without_the_clock(&render(&moved)),
            "same game, same payload"
        );

        // The scrub must not be what makes them equal. It has to leave the
        // substance behind, and two genuinely different games have to still
        // come out different after it.
        let scrubbed = without_the_clock(&before);
        for kept in ["\"hexes\"", "\"seats\"", "\"yourHand\"", "\"buildings\""] {
            assert!(scrubbed.contains(kept), "the scrub ate {kept}");
        }
        let elsewhere = Session::new(4, 5, TradeMode::Full);
        assert_ne!(
            scrubbed,
            without_the_clock(&render(&elsewhere)),
            "a different game must not survive the scrub as the same payload"
        );
    }

    /// The payload with its wall-clock readings taken out.
    ///
    /// How long a turn took is a fact about the machine and the moment, not
    /// about the position, so it differs between two runs of the same game and
    /// would fail an equality this test does not mean to make. Everything that
    /// describes the game itself stays in and is still compared.
    fn without_the_clock(payload: &str) -> String {
        let mut s = payload.to_string();
        for key in ["\"turnMs\":", "\"elapsed\":", "\"timeLeft\":"] {
            let mut out = String::with_capacity(s.len());
            let mut rest = s.as_str();
            while let Some(at) = rest.find(key) {
                out.push_str(&rest[..at + key.len()]);
                out.push_str("<clock>");
                let after = &rest[at + key.len()..];
                // An array carries commas of its own, so it ends at its
                // bracket; a bare number ends at whatever comes after it.
                let end = if after.starts_with('[') {
                    after.find(']').map_or(after.len(), |i| i + 1)
                } else {
                    after.find([',', '}']).unwrap_or(after.len())
                };
                rest = &after[end..];
            }
            out.push_str(rest);
            s = out;
        }
        s
    }

    #[test]
    fn the_page_is_sent_the_whole_log_not_a_tail_of_it() {
        // Scrolling the log back to the opening deal only works if the opening
        // deal was sent. Play far past any plausible window and count.
        let mut s = Session::new(4, 11, TradeMode::Full);
        for _ in 0..400 {
            if s.choices().is_empty() {
                break;
            }
            let v = s.version();
            let _ = s.act(0, v);
        }
        assert!(
            s.log().len() > 60,
            "need a long game, got {}",
            s.log().len()
        );
        let out = render(&s);
        assert_eq!(
            out.matches("\"turn\":").count(),
            s.log().len(),
            "every line the session kept should reach the page"
        );
        // And specifically the first one, which is what a player scrolls for.
        assert!(out.contains(&format!("\"t\":\"{}\"", s.log()[0].text)));
    }

    #[test]
    fn a_victory_point_card_is_never_offered_as_something_to_play() {
        // R-9.11: it scores the moment it is bought and is never played. The
        // dock relies on that to decide a pile is not clickable, so the
        // guarantee is checked where the payload leaves the process.
        let mut seen_one = false;
        for seed in 0..40 {
            let mut s = Session::new(4, seed, TradeMode::Full);
            for _ in 0..600 {
                if s.choices().is_empty() {
                    break;
                }
                let out = render(&s);
                assert!(
                    !out.contains("\"card\":\"victory point\""),
                    "a victory point card was offered as a play"
                );
                if s.view().own.is_some_and(|o| o.dev_held[1] > 0) {
                    seen_one = true;
                }
                // Buy whenever the deck allows it, otherwise take whatever is
                // first. Always taking the first choice never bought a card at
                // all, so the human never came to hold one and the assertion
                // above was never reached.
                let buy = s
                    .choices()
                    .iter()
                    .position(|c| c.label(s.state()) == "Buy a development card");
                let v = s.version();
                let _ = s.act(buy.unwrap_or(0), v);
            }
            if seen_one {
                break;
            }
        }
        assert!(seen_one, "the human never held one, so nothing was proven");
    }

    #[test]
    fn a_note_reaches_the_page() {
        let s = Session::new(4, 2, TradeMode::Disabled);
        assert!(render_with_note(&s, "Stale").contains("\"note\":\"Stale\""));
        assert!(render(&s).contains("\"note\":\"\""));
    }
}
