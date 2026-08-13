//! Game records: the omniscient log, replay, and per-viewer redaction (§7).
//!
//! One recorded game is the source material for a replay *and* the raw data
//! for statistics (§10). Two properties make that work:
//!
//! **Outcomes, not seeds** (H-1). Every random result is stored as the concrete
//! value it took. A seed-only log would require bit-exact determinism forever,
//! so a later rules correction would silently reinterpret every historical game
//! rather than failing loudly. Here replay is a fold over data, and a
//! divergence is caught by a snapshot comparison the moment it happens.
//!
//! **Omniscient store, redacted on serve** (H-2). The log holds the whole
//! truth, including the development deck order. Nothing here may be handed to a
//! client. Serving goes through [`fog`], which projects the state a viewer is
//! entitled to see — never by filtering this type.

use carranta_core::action::Resolved;
use carranta_core::state::{MAX_PLAYERS, TradeMode};
use carranta_core::{Action, Illegal, Phase, State};

pub mod fog;

pub use fog::{Fog, Seen, SeenResolved, Viewer};

/// The rules revision a game was played under (§7.4).
///
/// Mandatory on every game rather than nullable: design decisions and options
/// change actual gameplay, so an aggregate that mixes revisions is comparing
/// incomparable games.
pub const RULES_VERSION: u16 = 1;

/// The engine build that produced a game. Bumped when behaviour changes.
pub const ENGINE_VERSION: u16 = 1;

/// Who a seat belongs to, durably (H-6).
///
/// Pseudonymous by construction: `player` is an opaque id, and nothing here
/// carries personal content. That is what lets logs be kept indefinitely while
/// chat expires (H-8).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SeatId {
    /// Durable, opaque player identity, stable across games.
    pub player: u64,
    /// Set when the seat is played by software, so agent versions can be
    /// compared head to head across thousands of games (§7.4).
    pub agent: Option<AgentId>,
}

impl SeatId {
    /// A seat played by a named agent version.
    pub fn agent(player: u64, name: &str, version: u32) -> Self {
        SeatId {
            player,
            agent: Some(AgentId {
                name: name.to_string(),
                version,
            }),
        }
    }

    /// A seat played by a person.
    pub fn human(player: u64) -> Self {
        SeatId {
            player,
            agent: None,
        }
    }
}

/// A software player, identified by name and version (H-6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentId {
    pub name: String,
    pub version: u32,
}

/// Wall and monotonic timestamps (§7.2).
///
/// Supplied by the caller rather than read from a clock: this crate stays
/// clock-free so that recording a self-play corpus costs nothing, while a
/// server stamps every event for pacing on replay and think-time analytics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stamp {
    /// Milliseconds since the Unix epoch, for ordering against the outside
    /// world.
    pub wall_ms: u64,
    /// Microseconds on a monotonic clock, for durations — which wall time
    /// cannot measure across an adjustment.
    pub mono_us: u64,
}

/// Who caused an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Actor {
    Seat(u8),
    /// The engine itself: game creation, the end of the game.
    System,
}

/// The opening of a game: everything fixed before the first decision.
///
/// Carries the whole generated board *and the development deck order*, which
/// is what makes replay independent of the generator. It is also the single
/// most sensitive thing in the log — see the type's placement behind [`fog`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
    pub rules_version: u16,
    pub engine_version: u16,
    pub trade_mode: TradeMode,
    /// The seed that produced `opening`. Kept for provenance and for
    /// reproducing a board, never as the mechanism of replay (H-1).
    pub seed: u64,
    pub seats: Vec<SeatId>,
    /// The generated state before any placement.
    pub opening: Box<State>,
}

/// What happened.
///
/// Randomness rides on the decision that resolved it rather than arriving as a
/// separate event. The two cannot be separated in time — a roll *is* its dice —
/// and pairing them removes the only ordering ambiguity a replayer would
/// otherwise have to resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    /// A decision from the §5.4 catalogue, with whatever it resolved.
    Decision { action: Action, resolved: Resolved },
    /// An offer was declined. Recorded because negotiation churn is data
    /// (H-4) — under the open market it is most of the interaction in the game
    /// — even though it changes no state.
    Declined { offer: u8, by: u8 },
    /// The game reached a winner, or ran out of road.
    Ended {
        winner: Option<u8>,
        /// True victory points including hidden cards, revealed at the end
        /// (R-9.11, R-11.3).
        vp: [u32; MAX_PLAYERS],
    },
}

/// One entry in the log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// Position in the total order, from 0.
    pub seq: u32,
    pub at: Stamp,
    pub actor: Actor,
    pub payload: Payload,
}

/// A complete recorded game.
///
/// Omniscient (H-2). Never serialize this to a client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Log {
    pub game_id: u64,
    pub created: Created,
    pub events: Vec<Event>,
    /// Periodic states for seeking, keyed by the sequence number they follow.
    ///
    /// An index rather than history: every entry is regenerable by folding the
    /// events, so the event stream stays the canonical record (H-7). Each also
    /// serves as a checksum — [`Log::verify`] replays into them.
    pub snapshots: Vec<(u32, Box<State>)>,
}

/// Events between snapshots. 64 keeps seeking cheap without meaningful bloat:
/// a few hundred actions per game means a handful of snapshots.
pub const SNAPSHOT_EVERY: u32 = 64;

/// Why a log could not be replayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReplayError {
    /// An action the engine rejects. The log was written by a different rules
    /// version, or is corrupt.
    Illegal { seq: u32, why: Illegal },
    /// The engine resolved randomness differently from what was recorded,
    /// which means the log and this build disagree about the game.
    Diverged { seq: u32 },
    /// A replayed state did not match the snapshot taken at the same point.
    SnapshotMismatch { seq: u32 },
    /// `seq` is past the end of the log.
    PastEnd,
}

impl Log {
    /// Fold the whole log into the final state.
    ///
    /// Deliberately does *not* take the snapshot shortcut. The events are the
    /// canonical record and the snapshots are a derived index (H-7), so
    /// "replay the game" means read the game — starting from a snapshot would
    /// step over most of the log and trust an index to stand in for it.
    pub fn replay(&self) -> Result<State, ReplayError> {
        let mut state = *self.created.opening;
        for event in &self.events {
            apply_event(&mut state, event)?;
        }
        Ok(state)
    }

    /// Fold the log up to and including `seq`, seeking from the nearest
    /// snapshot at or before it.
    ///
    /// This is the seeking path, and it *trusts the index*: a corrupt snapshot
    /// yields a wrong position rather than an error. [`Log::verify`] is what
    /// checks the index against the events.
    pub fn replay_to(&self, seq: u32) -> Result<State, ReplayError> {
        let last = self.events.last().map_or(0, |e| e.seq);
        if seq != u32::MAX && seq > last && !self.events.is_empty() {
            return Err(ReplayError::PastEnd);
        }

        // Start from the latest snapshot that does not overshoot.
        let (mut state, from) = match self
            .snapshots
            .iter()
            .filter(|(s, _)| *s <= seq)
            .max_by_key(|(s, _)| *s)
        {
            Some((s, st)) => (**st, *s + 1),
            None => (*self.created.opening, 0),
        };

        for event in self.events.iter().filter(|e| e.seq >= from && e.seq <= seq) {
            apply_event(&mut state, event)?;
        }
        Ok(state)
    }

    /// Replay into every snapshot, checking each one.
    ///
    /// This is the guard H-1 buys: a rules or engine change that reinterprets
    /// history fails here, loudly, instead of quietly producing different
    /// games from the same log.
    pub fn verify(&self) -> Result<State, ReplayError> {
        let mut state = *self.created.opening;
        let mut snaps = self.snapshots.iter().peekable();
        for event in &self.events {
            apply_event(&mut state, event)?;
            while let Some((s, want)) = snaps.peek() {
                if *s > event.seq {
                    break;
                }
                if *s == event.seq && !state.same_game_as(want) {
                    return Err(ReplayError::SnapshotMismatch { seq: *s });
                }
                snaps.next();
            }
        }
        Ok(state)
    }

    /// Actions recorded, ignoring negotiation churn and lifecycle.
    pub fn decisions(&self) -> impl Iterator<Item = (&Event, Action, Resolved)> {
        self.events.iter().filter_map(|e| match e.payload {
            Payload::Decision { action, resolved } => Some((e, action, resolved)),
            _ => None,
        })
    }
}

/// Apply one recorded event to a state, holding the engine to what was
/// recorded.
fn apply_event(state: &mut State, event: &Event) -> Result<(), ReplayError> {
    let Payload::Decision { action, resolved } = event.payload else {
        return Ok(()); // Declined and Ended change nothing
    };
    let got = state
        .apply_scripted(action, resolved)
        .map_err(|why| ReplayError::Illegal {
            seq: event.seq,
            why,
        })?;
    // `apply_scripted` falls back to a live draw when the script does not fit
    // the action, so this comparison is what turns a mismatch into an error
    // rather than a quietly different game.
    if got != resolved {
        return Err(ReplayError::Diverged { seq: event.seq });
    }
    Ok(())
}

/// Builds a [`Log`] while a game is played.
///
/// Recording is configurable per session (H-3) — a self-play rollout that
/// wants no log simply does not construct one.
pub struct Recorder {
    log: Log,
    state: State,
    seq: u32,
}

impl Recorder {
    /// Begin recording a game from its opening state.
    pub fn new(game_id: u64, seed: u64, opening: State, seats: Vec<SeatId>) -> Self {
        Recorder {
            log: Log {
                game_id,
                created: Created {
                    rules_version: RULES_VERSION,
                    engine_version: ENGINE_VERSION,
                    trade_mode: opening.trade_mode,
                    seed,
                    seats,
                    opening: Box::new(opening),
                },
                events: Vec::new(),
                snapshots: Vec::new(),
            },
            state: opening,
            seq: 0,
        }
    }

    /// The live state.
    pub fn state(&self) -> &State {
        &self.state
    }

    /// Apply an action and record it with whatever randomness it resolved.
    pub fn apply(&mut self, action: Action) -> Result<Resolved, Illegal> {
        self.apply_at(action, Stamp::default())
    }

    /// Apply and record an action, stamped.
    pub fn apply_at(&mut self, action: Action, at: Stamp) -> Result<Resolved, Illegal> {
        let actor = Actor::Seat(self.state.decider());
        let resolved = self.state.apply_recorded(action)?;
        self.push(at, actor, Payload::Decision { action, resolved });
        if let Phase::GameOver { winner } = self.state.phase {
            self.finish(Some(winner), at);
        }
        Ok(resolved)
    }

    /// Record an offer that was declined (H-4). Changes no state.
    pub fn decline(&mut self, offer: u8, by: u8) {
        self.push(
            Stamp::default(),
            Actor::Seat(by),
            Payload::Declined { offer, by },
        );
    }

    /// Close the log, recording the outcome and a final snapshot.
    pub fn finish(&mut self, winner: Option<u8>, at: Stamp) {
        if matches!(
            self.log.events.last().map(|e| &e.payload),
            Some(Payload::Ended { .. })
        ) {
            return;
        }
        let vp = core::array::from_fn(|p| {
            if p < self.state.players as usize {
                self.state.victory_points(p)
            } else {
                0
            }
        });
        self.push(at, Actor::System, Payload::Ended { winner, vp });
        let seq = self.seq - 1;
        if self.log.snapshots.last().map(|(s, _)| *s) != Some(seq) {
            self.log.snapshots.push((seq, Box::new(self.state)));
        }
    }

    /// Take the finished log.
    pub fn finish_into(mut self, winner: Option<u8>) -> Log {
        self.finish(winner, Stamp::default());
        self.log
    }

    fn push(&mut self, at: Stamp, actor: Actor, payload: Payload) {
        let seq = self.seq;
        self.seq += 1;
        self.log.events.push(Event {
            seq,
            at,
            actor,
            payload,
        });
        if seq > 0 && seq.is_multiple_of(SNAPSHOT_EVERY) {
            self.log.snapshots.push((seq, Box::new(self.state)));
        }
    }
}
