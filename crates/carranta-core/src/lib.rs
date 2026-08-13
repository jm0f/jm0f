//! Carranta rules engine core.
//!
//! Pure and deterministic: no I/O, no async, no clock, no global state. Every
//! consumer — the game server, the training environment, the browser client —
//! is a thin adapter over this crate, so anything that cannot run inside a
//! tight rollout loop does not belong here.

pub mod action;
pub mod longest_road;
pub mod rng;
pub mod state;
pub mod topology;

pub use action::{Action, Illegal};
pub use longest_road::{Tracker, longest_road, longest_road_exceeds};
pub use state::{DevCard, Phase, Resource, State, Terrain};
pub use topology::{EdgeSet, VertexSet};
