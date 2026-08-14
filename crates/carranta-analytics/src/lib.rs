//! Metrics derived from Carranta game records (§10).
//!
//! Everything here reads a [`carranta_record::Log`] and computes; nothing is
//! stored. That is deliberate and cheap, replaying a game costs ~35 µs, so a
//! changed metric is recomputed over the corpus rather than migrated (H-7).
//!
//! | Module | Section | What it answers |
//! |---|---|---|
//! | [`dice`] | §10.1 | Were this game's dice unusual? Is the generator fair? |
//! | [`production`] | §10.2 | Expected versus actual production, and why they differ |
//! | [`game`] | §10.3 | Everything countable about one game |
//! | [`corpus`] | §10.3, §10.4 | Balance across many games, and who converts production into points |
//! | [`rating`] | §10.4, §10.5 | Player skill, and luck-adjusted performance |
//! | [`stats`] |, | The statistics the rest is built on |
//!
//! One theme runs through all of it, from §10.1: **small n makes p-values
//! invalid, large n makes them uninformative.** Every test here is paired with
//! an effect size, and per-game results are presented as percentiles against
//! recorded games rather than as significance claims.

pub mod corpus;
pub mod dice;
pub mod game;
pub mod production;
pub mod rating;
pub mod stats;

#[cfg(test)]
mod testing;
