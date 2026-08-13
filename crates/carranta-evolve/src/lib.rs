//! Population training for Carranta (§9.5).
//!
//! Phase one of E-1: an evolution strategy over the fifteen weights the
//! heuristic already carries. They were hand-set and never tuned, so this is
//! the step that is certain to pay — and it exercises the whole harness
//! (parallel paired evaluation, versioned agents, a rating anchor) before
//! topology search is asked to rely on it.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`genome`] | What is optimised, and how it mutates |
//! | [`checkpoint`] | Saving a run and resuming it exactly |
//! | [`behaviour`] | How the population plays, from a sample of recorded games |
//! | [`arena`] | Where genomes are measured — trading on, common random numbers, deterministic under parallelism |
//! | [`ladder`] | Versioned agents on one rating scale, anchored to the pinned heuristic |
//! | [`train`] | The generation loop and its adaptive budget |

pub mod arena;
pub mod behaviour;
pub mod checkpoint;
pub mod genome;
pub mod ladder;
pub mod train;

pub use arena::{Arena, Job, Outcome};
pub use behaviour::{Behaviour, Sampler};
pub use genome::Genome;
pub use ladder::{ANCHOR, Ladder, Versioned};
pub use train::{Config, Report, Trainer};
