//! Population training for Carranta (§9.5).
//!
//! Both phases of E-1. Phase one, [`genome`] and [`train`]: an evolution
//! strategy over the sixteen weights the heuristic already carries,
//! hand-set and never tuned, the step that was certain to pay and that
//! exercised the whole harness before topology search was asked to rely on
//! it. Phase two, [`neat`] and [`train_neat`]: NEAT proper, minimal networks
//! growing structure over the engineered observation, trained in the full
//! mixed-offer market.
//!
//! | Module | What it holds |
//! |---|---|
//! | [`genome`] | Phase one's genome: the heuristic's weights, and how they mutate |
//! | [`neat`] | Phase two's genome: topology, innovation history, speciation |
//! | [`checkpoint`] | Saving a run and resuming it exactly |
//! | [`behaviour`] | How the population plays, from a sample of recorded games |
//! | [`arena`] | Where genomes are measured, trading on, common random numbers, deterministic under parallelism |
//! | [`ladder`] | Versioned agents on one rating scale, anchored to the pinned heuristic |
//! | [`mapelites`] | The quality-diversity archive: the best player at each style of play |
//! | [`train`] | Phase one's generation loop and the adaptive budget |
//! | [`train_neat`] | Phase two's generation loop, species and all |

pub mod arena;
pub mod behaviour;
pub mod checkpoint;
pub mod genome;
pub mod ladder;
pub mod mapelites;
pub mod neat;
pub mod train;
pub mod train_neat;

pub use arena::{Arena, Job, Outcome};
pub use behaviour::{Behaviour, Sampler};
pub use genome::Genome;
pub use ladder::{ANCHOR, Ladder, Versioned};
pub use train::{Config, Report, Trainer};
