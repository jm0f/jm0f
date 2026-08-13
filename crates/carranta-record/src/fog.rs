//! What a given viewer is entitled to see (§4, §7.3).
//!
//! Redaction here is **structural, not filtered**. [`Fog`] has no field for
//! another seat's card identities and none for the deck order, so a leak is a
//! compile error rather than a missed branch. That is deliberate: §7.6 notes
//! that redaction leaks are silent, surfacing only when someone exploits them.
//!
//! It also projects *state*, not events. §7.3 warns that visibility is a
//! function of `(event, viewer, time)` and not a static classification of event
//! types — a card that is `OWNER` when drawn is `PUBLIC` when played, and
//! Monopoly forcibly exposes part of every hand (R-9.9). Masking events one at
//! a time gets those wrong. Projecting the position after each event gets them
//! right by construction: the thief's own hand shows the card they took, the
//! table sees only that hand sizes moved, and Monopoly needs no special case at
//! all.

use carranta_core::action::Resolved;
use carranta_core::longest_road::longest_road;
use carranta_core::state::{
    DEV_DECK_SIZE, DevCard, MAX_OFFERS, MAX_PLAYERS, Offer, PORT_KINDS, Terrain, TradeMode,
};
use carranta_core::topology::HEX_COUNT;
use carranta_core::{EdgeSet, Phase, State, VertexSet};

use crate::{Actor, Event, Log, Payload, ReplayError, Stamp};

/// Who is watching.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewer {
    /// A player, who additionally sees their own hand.
    Seat(u8),
    /// Someone watching the table (P-6). Public information only — the same
    /// view as a person standing behind the players.
    Spectator,
}

/// The private holdings of the seat a view belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Own {
    pub seat: u8,
    pub hand: [u8; 5],
    /// Development cards held and unplayed.
    pub dev_held: [u8; 5],
    /// Of those, the ones bought this turn and so unplayable (R-9.4).
    pub dev_fresh: [u8; 5],
    /// True victory points, counting one's own hidden cards (R-11.3).
    pub victory_points: u32,
}

/// A position as one viewer is entitled to know it.
///
/// Everything here is `PUBLIC` or `DERIVED` per §4.2 and §4.3, except [`own`],
/// which is the viewer's own `OWNER` data.
///
/// [`own`]: Fog::own
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fog {
    // ---- Board (§4.2: all PUBLIC once placed) ----
    pub terrain: [Terrain; HEX_COUNT],
    pub number: [u8; HEX_COUNT],
    pub ports: [VertexSet; PORT_KINDS],
    pub robber: u8,
    pub roads: [EdgeSet; MAX_PLAYERS],
    pub settlements: [VertexSet; MAX_PLAYERS],
    pub cities: [VertexSet; MAX_PLAYERS],

    // ---- Per seat: counts, never identities (§4.1's key asymmetry) ----
    /// Hand size, which must be public because the discard rule depends on it
    /// (R-6.2) — players answer honestly.
    pub hand_size: [u8; MAX_PLAYERS],
    /// Development cards held. Public as a count, excluded from the discard
    /// count (R-9.2).
    pub dev_count: [u8; MAX_PLAYERS],
    /// Militia played face up, which is what Largest Militia counts (R-10.8).
    pub militia_played: [u8; MAX_PLAYERS],
    pub roads_left: [u8; MAX_PLAYERS],
    pub settlements_left: [u8; MAX_PLAYERS],
    pub cities_left: [u8; MAX_PLAYERS],
    /// Victory points an onlooker can count: hidden cards excluded (R-9.11).
    ///
    /// Tracked apart from the true total on purpose (§4.3) — serving the real
    /// number would leak every held Victory Point card.
    pub apparent_vp: [u32; MAX_PLAYERS],
    /// Longest continuous route per seat: computed from public roads and
    /// public buildings, so public itself (§4.3).
    pub road_length: [u32; MAX_PLAYERS],

    // ---- Shared ----
    pub supply: [u8; 5],
    /// Cards left in the deck. The *count* is public; the order is not.
    pub dev_left: u8,
    pub longest_road: Option<u8>,
    pub largest_militia: Option<u8>,

    // ---- Turn ----
    pub players: u8,
    pub to_act: u8,
    pub phase: Phase,
    pub dice: [u8; 2],
    pub dev_played_this_turn: bool,
    pub free_roads: u8,
    pub discard_left: [u8; MAX_PLAYERS],

    // ---- Market: offers are public, a trade being a public act (R-7.4) ----
    pub trade_mode: TradeMode,
    pub offers: [Offer; MAX_OFFERS],
    pub offer_count: u8,
    pub offers_made: [u8; MAX_PLAYERS],

    /// The viewer's own private holdings; `None` for a spectator.
    pub own: Option<Own>,
}

/// Project a state into what `viewer` may see.
///
/// The four hidden things of §4.4 are handled thus: resource identities and
/// development card identities appear only in [`Fog::own`]; the deck order has
/// no representation at all, only `dev_left`; and the variable-setup facedown
/// step does not arise, since this engine generates a board atomically.
pub fn fog(state: &State, viewer: Viewer) -> Fog {
    let seats = state.players as usize;
    let per = |f: &dyn Fn(usize) -> u32| -> [u32; MAX_PLAYERS] {
        core::array::from_fn(|p| if p < seats { f(p) } else { 0 })
    };

    Fog {
        terrain: state.terrain,
        number: state.number,
        ports: state.ports,
        robber: state.robber,
        roads: state.roads,
        settlements: state.settlements,
        cities: state.cities,

        hand_size: core::array::from_fn(|p| {
            if p < seats {
                state.hand_size(p) as u8
            } else {
                0
            }
        }),
        dev_count: core::array::from_fn(|p| {
            if p < seats {
                state.dev_count(p) as u8
            } else {
                0
            }
        }),
        militia_played: state.militia_played,
        roads_left: state.roads_left,
        settlements_left: state.settlements_left,
        cities_left: state.cities_left,
        apparent_vp: per(&|p| state.public_victory_points(p)),
        road_length: per(&|p| longest_road(state.roads[p], state.blocking(p))),

        supply: state.supply,
        dev_left: (DEV_DECK_SIZE as u8).saturating_sub(state.dev_drawn),
        longest_road: state.longest_road,
        largest_militia: state.largest_militia,

        players: state.players,
        to_act: state.to_act,
        phase: state.phase,
        dice: state.dice,
        dev_played_this_turn: state.dev_played_this_turn,
        free_roads: state.free_roads,
        discard_left: state.discard_left,

        trade_mode: state.trade_mode,
        offers: state.offers,
        offer_count: state.offer_count,
        offers_made: state.offers_made,

        own: match viewer {
            Viewer::Spectator => None,
            Viewer::Seat(s) if (s as usize) < seats => Some(Own {
                seat: s,
                hand: state.hand[s as usize],
                dev_held: state.dev_held[s as usize],
                dev_fresh: state.dev_fresh[s as usize],
                victory_points: state.victory_points(s as usize),
            }),
            // A seat index outside the game sees what a spectator sees rather
            // than everything, which is the safe way for that to be wrong.
            Viewer::Seat(_) => None,
        },
    }
}

/// The randomness of an event, as a viewer may see it.
///
/// Note what is *absent*: a stolen card has no identity here. The thief learns
/// what they took from their own hand in the accompanying [`Fog`], and nobody
/// else ever learns it — which is exactly R-6.4, and is unrepresentable rather
/// than merely unset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeenResolved {
    None,
    /// Both dice. Always public (R-5.2).
    Dice(u8, u8),
    /// A robbery moved a card, or found an empty hand (R-6.4).
    Stolen {
        took_a_card: bool,
    },
}

impl SeenResolved {
    fn of(resolved: Resolved) -> Self {
        match resolved {
            Resolved::None => SeenResolved::None,
            Resolved::Dice(a, b) => SeenResolved::Dice(a, b),
            Resolved::Steal(taken) => SeenResolved::Stolen {
                took_a_card: taken.is_some(),
            },
        }
    }
}

/// One event as a viewer is entitled to see it, with the position it produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seen {
    pub seq: u32,
    pub at: Stamp,
    pub actor: Actor,
    pub what: SeenWhat,
    /// The position after the event, already redacted.
    pub after: Fog,
}

/// The visible content of an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeenWhat {
    Decision {
        action: carranta_core::Action,
        resolved: SeenResolved,
    },
    Declined {
        offer: u8,
        by: u8,
    },
    Ended {
        winner: Option<u8>,
        /// Now public: hidden cards are revealed on a win (R-9.11).
        vp: [u32; MAX_PLAYERS],
    },
}

/// Replay a log and emit it as one viewer may see it.
///
/// This is the only sanctioned path from a [`Log`] to something a client may
/// receive, and it runs server-side by construction: it needs the omniscient
/// state to compute each position, then discards what the viewer may not have
/// (§7.3). Never ship a log and filter in the UI.
pub fn project(log: &Log, viewer: Viewer) -> Result<Vec<Seen>, ReplayError> {
    let mut state = *log.created.opening;
    let mut out = Vec::with_capacity(log.events.len());
    for event in &log.events {
        if let Payload::Decision { action, resolved } = event.payload {
            let got =
                state
                    .apply_scripted(action, resolved)
                    .map_err(|why| ReplayError::Illegal {
                        seq: event.seq,
                        why,
                    })?;
            if got != resolved {
                return Err(ReplayError::Diverged { seq: event.seq });
            }
        }
        out.push(Seen {
            seq: event.seq,
            at: event.at,
            actor: event.actor,
            what: seen_what(event),
            after: fog(&state, viewer),
        });
    }
    Ok(out)
}

fn seen_what(event: &Event) -> SeenWhat {
    match event.payload {
        Payload::Decision { action, resolved } => SeenWhat::Decision {
            action,
            resolved: SeenResolved::of(resolved),
        },
        Payload::Declined { offer, by } => SeenWhat::Declined { offer, by },
        Payload::Ended { winner, vp } => SeenWhat::Ended { winner, vp },
    }
}

/// Victory point cards a seat holds — the one thing `apparent_vp` deliberately
/// omits. Exposed so tests can assert it never reaches the wrong viewer.
#[doc(hidden)]
pub fn hidden_vp(state: &State, p: usize) -> u8 {
    state.dev_held[p][DevCard::VictoryPoint as usize]
}
