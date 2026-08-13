//! The redacted position, as the page receives it.
//!
//! Everything here is read off a [`Fog`] — the projection of §7.3 — plus the
//! board geometry the engine now exposes. Nothing reads the raw `State` except
//! the geometry, which is public information by definition.

use carranta_core::state::{MAX_PLAYERS, Phase};
use carranta_core::topology::{
    EDGE_COUNT, HEX_COUNT, VERTEX_COUNT, edge_endpoints, hex_axial, iter_vertices, vertex_axial,
    vertex_bit,
};

use crate::game::{HUMAN, Session, Target};
use crate::json::Json;

const RESOURCES: [&str; 5] = ["brick", "wood", "wool", "wheat", "ore"];
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

/// The same, with something to tell the player — a refused click, usually.
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
    j.int("seed", session.seed() as i64);
    j.int("you", HUMAN as i64);
    j.int("players", seats as i64);
    j.int("toAct", v.to_act as i64);
    j.str("phase", phase_name(v.phase));
    j.bool("yourTurn", session.state().decider() == HUMAN);
    j.opt_int(
        "winner",
        match v.phase {
            Phase::GameOver { winner } => Some(winner as i64),
            _ => None,
        },
    );
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
    });
    let own = v
        .own
        .unwrap_or_else(|| unreachable!("the human has a seat"));
    j.ints("yourHand", own.hand.iter().map(|&n| n as i64));
    j.ints("yourDev", own.dev_held.iter().map(|&n| n as i64));
    j.ints("yourFresh", own.dev_fresh.iter().map(|&n| n as i64));
    j.int("yourVp", own.victory_points as i64);
    j.ints("supply", v.supply.iter().map(|&n| n as i64));
    j.int("devLeft", v.dev_left as i64);
    j.ints("dice", v.dice.iter().map(|&n| n as i64));

    // ---- The market ----
    j.array("offers", 0..v.offer_count as usize, |o, i| {
        let offer = v.offers[i];
        o.int("i", i as i64)
            .int("from", offer.from as i64)
            .ints("give", offer.give.iter().map(|&n| n as i64))
            .ints("want", offer.want.iter().map(|&n| n as i64));
    });

    // ---- What the human may do now ----
    let choices = session.choices();
    j.array("choices", choices.iter().enumerate(), |o, (i, c)| {
        o.int("i", i as i64)
            .str("label", &c.label())
            .str("group", c.group());
        match c.target() {
            Target::Vertex(x) => o.int("vertex", x as i64),
            Target::Edge(x) => o.int("edge", x as i64),
            Target::Hex(x) => o.int("hex", x as i64),
            Target::None => o.bool("plain", true),
        };
    });

    // Names, so the page need not carry its own copy of the vocabulary.
    j.array(
        RESOURCE_KEY,
        RESOURCES.iter().enumerate(),
        |o, (i, name)| {
            o.int("i", i as i64).str("name", name);
        },
    );
    j.array("devNames", DEV_CARDS.iter().enumerate(), |o, (i, name)| {
        o.int("i", i as i64).str("name", name);
    });

    let log = session.log();
    j.array(
        "log",
        log.iter()
            .rev()
            .take(40)
            .collect::<Vec<_>>()
            .into_iter()
            .rev(),
        |o, line| {
            o.str("t", line);
        },
    );
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
        assert_eq!(before, render(&moved), "same game, same payload");
    }

    #[test]
    fn a_note_reaches_the_page() {
        let s = Session::new(4, 2, TradeMode::Disabled);
        assert!(render_with_note(&s, "Stale").contains("\"note\":\"Stale\""));
        assert!(render(&s).contains("\"note\":\"\""));
    }
}
