//! Carranta rules engine core.
//!
//! Pure and deterministic: no I/O, no async, no clock, no global state. Every
//! consumer — the game server, the training environment, the browser client —
//! is a thin adapter over this crate, so anything that cannot run inside a
//! tight rollout loop does not belong here.

pub mod longest_road;
pub mod topology;

pub use longest_road::longest_road;
pub use topology::{EdgeSet, VertexSet};
