//! Versioned agents on one rating scale (E-8).
//!
//! Every genome that is kept gets a durable identity. H-6 already requires
//! this of recorded games, and all versions share a single §10.5 rating pool.
//! Progress is then not a separate metric at all: it is generation 50's μ read
//! against generation 1's, on the same scale, in the same units as a human
//! player's rating will eventually be.
//!
//! # Why the anchor still matters
//!
//! Versioning alone is not enough, and the reason is easy to miss. Ratings are
//! only comparable if the games that produced them *connect*. If generation 50
//! played only generations 49 and 50, and generation 1 played only 1 and 2,
//! then comparing them means chaining forty-odd pairwise comparisons, and the
//! error compounds along the chain. The classic drift between eras of a
//! long-running ladder.
//!
//! The fix here is deliberately blunt: **every version plays the anchor
//! directly.** The comparison graph is then a star centred on the pinned
//! heuristic rather than a chain, and any two versions are two hops apart no
//! matter how many generations separate them. [`Ladder::connectivity`] reports
//! it so a run cannot quietly lose the property.

use std::collections::HashMap;

use carranta_analytics::rating::{Model, Pool, Rating};
use carranta_core::state::MAX_PLAYERS;

use crate::genome::Genome;

/// The anchor's identity. Fixed, so a checkpoint from any run refers to the
/// same agent.
pub const ANCHOR: u64 = 0;

/// A genome that has been given a durable identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Versioned {
    pub id: u64,
    pub generation: u32,
    pub label: String,
    pub genome: Genome,
}

/// Every version ever kept, and what they are worth.
#[derive(Clone, Debug)]
pub struct Ladder {
    pool: Pool,
    members: HashMap<u64, Versioned>,
    /// Games each version has played against the anchor, which is what makes
    /// its rating comparable to every other version's.
    anchored: HashMap<u64, u32>,
    next_id: u64,
}

impl Default for Ladder {
    fn default() -> Self {
        Ladder::new(Model::default())
    }
}

impl Ladder {
    /// A fresh ladder holding only the pinned heuristic.
    pub fn new(model: Model) -> Self {
        let mut members = HashMap::new();
        members.insert(
            ANCHOR,
            Versioned {
                id: ANCHOR,
                generation: 0,
                label: "heuristic-v1".to_string(),
                genome: Genome::default(),
            },
        );
        let mut pool = Pool::new(model);
        // The anchor's μ is the origin of the scale, so it must not move,
        // otherwise it sinks against a population that keeps improving, and a
        // gap of "+4 μ" stops meaning the same thing in generation 40 as it
        // did in generation 1. σ is left free: it reflects games genuinely
        // played, and tightening it is what makes each anchored game count.
        pool.pin(ANCHOR);
        Ladder {
            pool,
            members,
            anchored: HashMap::new(),
            next_id: 1,
        }
    }

    /// The pinned heuristic: generation zero, and the thing every version is
    /// measured against.
    pub fn anchor(&self) -> &Versioned {
        &self.members[&ANCHOR]
    }

    /// Give a genome an identity and add it to the ladder.
    pub fn enrol(&mut self, genome: Genome, generation: u32) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.members.insert(
            id,
            Versioned {
                id,
                generation,
                label: format!("g{generation:03}-{id:04}"),
                genome,
            },
        );
        id
    }

    pub fn get(&self, id: u64) -> Option<&Versioned> {
        self.members.get(&id)
    }

    pub fn rating(&self, id: u64) -> Rating {
        self.pool.rating(id)
    }

    pub fn games_played(&self, id: u64) -> u32 {
        self.pool.games_played(id)
    }

    /// Record one finished game between four versions.
    pub fn record(&mut self, ids: &[u64; MAX_PLAYERS], positions: &[u32; MAX_PLAYERS]) {
        self.pool.record_ranked(ids, positions);
        if ids.contains(&ANCHOR) {
            for &id in ids {
                if id != ANCHOR {
                    *self.anchored.entry(id).or_insert(0) += 1;
                }
            }
        }
    }

    /// How well the ladder hangs together: the share of versions that have
    /// played the anchor at least `min_games` times.
    ///
    /// Below 1.0, some version's rating rests on a chain of comparisons rather
    /// than a direct one, and its distance from the anchor is not trustworthy.
    pub fn connectivity(&self, min_games: u32) -> f64 {
        let rated: Vec<u64> = self
            .members
            .keys()
            .copied()
            .filter(|&id| id != ANCHOR && self.pool.games_played(id) > 0)
            .collect();
        if rated.is_empty() {
            return 1.0;
        }
        let ok = rated
            .iter()
            .filter(|&&id| self.anchored.get(&id).copied().unwrap_or(0) >= min_games)
            .count();
        ok as f64 / rated.len() as f64
    }

    /// How far a version stands above the pinned heuristic, in μ.
    ///
    /// The headline number of a training run, and the reason the anchor never
    /// changes: this is the same quantity on day one and day thirty.
    pub fn above_anchor(&self, id: u64) -> f64 {
        self.rating(id).mu - self.rating(ANCHOR).mu
    }

    /// Every rated version, best first.
    pub fn standings(&self, min_games: u32) -> Vec<(&Versioned, Rating, u32)> {
        let mut rows: Vec<(&Versioned, Rating, u32)> = self
            .members
            .values()
            .map(|v| (v, self.rating(v.id), self.games_played(v.id)))
            .filter(|(_, _, n)| *n >= min_games)
            .collect();
        rows.sort_by(|a, b| {
            b.1.conservative()
                .total_cmp(&a.1.conservative())
                .then(a.0.id.cmp(&b.0.id))
        });
        rows
    }

    /// Every version's id, unordered.
    pub fn ids(&self) -> Vec<u64> {
        self.members.keys().copied().collect()
    }

    /// Games a version has played against the anchor.
    pub fn anchored_games(&self, id: u64) -> u32 {
        self.anchored.get(&id).copied().unwrap_or(0)
    }

    /// Drop every rating that does not belong to a known version.
    ///
    /// The population's own members play under throwaway ids, because a genome
    /// that may not survive the generation has no durable identity. Those ids
    /// must not persist: "population slot 3" is a different genome every
    /// generation, so a rating accumulating against that id is a history
    /// belonging to nobody, and it would quietly influence every champion's
    /// update thereafter.
    pub fn forget_transients(&mut self) {
        for id in self.pool.players() {
            if !self.members.contains_key(&id) {
                self.pool.forget(id);
            }
        }
    }

    /// Put a version back with a known rating, for resuming a run.
    ///
    /// The anchor stays pinned when restored, or a resumed run would silently
    /// lose the fixed origin every cross-generation comparison rests on.
    pub fn restore(&mut self, version: Versioned, rating: Rating, games: u32, anchored: u32) {
        let id = version.id;
        self.next_id = self.next_id.max(id + 1);
        self.members.insert(id, version);
        self.pool.restore(id, rating, games);
        if id == ANCHOR {
            self.pool.pin(ANCHOR);
        }
        if anchored > 0 {
            self.anchored.insert(id, anchored);
        }
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Write the ladder as text, one version per line.
    ///
    /// Deliberately plain: a run that dies overnight should be resumable, and
    /// its state should be readable without the program that wrote it.
    pub fn encode(&self) -> String {
        let mut ids: Vec<u64> = self.members.keys().copied().collect();
        ids.sort_unstable();
        let mut out = String::from("# id generation games mu sigma anchored label genes...\n");
        for id in ids {
            let v = &self.members[&id];
            let r = self.rating(id);
            out.push_str(&format!(
                "{} {} {} {:.6} {:.6} {} {} {}\n",
                v.id,
                v.generation,
                self.games_played(id),
                r.mu,
                r.sigma,
                self.anchored.get(&id).copied().unwrap_or(0),
                v.label,
                v.genome.encode(),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_anchor_is_present_from_the_start_and_never_moves() {
        let mut ladder = Ladder::default();
        assert_eq!(ladder.anchor().id, ANCHOR);
        assert_eq!(ladder.anchor().genome, Genome::default());
        assert_eq!(ladder.above_anchor(ANCHOR), 0.0);

        // Its genome is fixed whatever happens on the ladder.
        let before = ladder.anchor().genome;
        for g in 1..20 {
            let id = ladder.enrol(Genome::default(), g);
            ladder.record(&[ANCHOR, id, ANCHOR, id], &[1, 2, 3, 4]);
        }
        assert_eq!(ladder.anchor().genome, before);
    }

    #[test]
    fn a_stronger_version_climbs_above_the_anchor() {
        let mut ladder = Ladder::default();
        let strong = ladder.enrol(Genome::default(), 5);
        for _ in 0..200 {
            // The challenger takes the top two positions every time.
            ladder.record(&[strong, ANCHOR, strong, ANCHOR], &[1, 3, 2, 4]);
        }
        assert!(
            ladder.above_anchor(strong) > 2.0,
            "gap is only {:.2}",
            ladder.above_anchor(strong)
        );
        assert!(ladder.rating(strong).sigma < 25.0 / 3.0);
    }

    #[test]
    fn versions_are_comparable_across_generations_through_the_anchor() {
        // Generation 1 beats the anchor moderately; generation 40 beats it
        // decisively. They never play each other, which is exactly the case
        // the anchor exists to handle.
        let mut ladder = Ladder::default();
        let early = ladder.enrol(Genome::default(), 1);
        let late = ladder.enrol(Genome::default(), 40);

        for _ in 0..150 {
            // Early: wins one of the two contested seats.
            ladder.record(&[early, ANCHOR, early, ANCHOR], &[1, 2, 3, 4]);
        }
        for _ in 0..150 {
            // Late: takes both.
            ladder.record(&[late, ANCHOR, late, ANCHOR], &[1, 3, 2, 4]);
        }

        assert!(
            ladder.above_anchor(late) > ladder.above_anchor(early),
            "late {:.2} did not beat early {:.2} despite a better record",
            ladder.above_anchor(late),
            ladder.above_anchor(early),
        );
        assert_eq!(ladder.connectivity(50), 1.0, "both should be anchored");
        // And the origin has not moved, which is what made that comparison
        // legitimate rather than a coincidence of ordering.
        assert_eq!(ladder.rating(ANCHOR).mu, 25.0);
    }

    #[test]
    fn connectivity_notices_a_version_that_never_met_the_anchor() {
        let mut ladder = Ladder::default();
        let a = ladder.enrol(Genome::default(), 1);
        let b = ladder.enrol(Genome::default(), 2);
        let orphan = ladder.enrol(Genome::default(), 3);

        for _ in 0..20 {
            ladder.record(&[a, ANCHOR, b, ANCHOR], &[1, 2, 3, 4]);
        }
        assert_eq!(ladder.connectivity(10), 1.0);

        // The orphan plays only its peers, so its rating rests on a chain.
        for _ in 0..20 {
            ladder.record(&[orphan, a, orphan, b], &[1, 2, 3, 4]);
        }
        assert!(
            ladder.connectivity(10) < 1.0,
            "an unanchored version went unnoticed"
        );
    }

    #[test]
    fn standings_hide_versions_that_have_not_played() {
        let mut ladder = Ladder::default();
        let played = ladder.enrol(Genome::default(), 1);
        let _unplayed = ladder.enrol(Genome::default(), 1);
        for _ in 0..30 {
            ladder.record(&[played, ANCHOR, played, ANCHOR], &[1, 2, 3, 4]);
        }
        let rows = ladder.standings(10);
        assert_eq!(rows.len(), 2, "the anchor and the one that played");
        assert!(rows.iter().all(|(v, _, n)| *n >= 10 && v.id != 2));
    }

    #[test]
    fn a_throwaway_opponent_does_not_accumulate_a_history() {
        let mut ladder = Ladder::default();
        let champion = ladder.enrol(Genome::default(), 1);
        let transient = 1 << 40;
        for _ in 0..20 {
            ladder.record(&[champion, ANCHOR, transient, transient + 1], &[1, 2, 3, 4]);
        }
        ladder.forget_transients();
        // The placeholder is back at the prior, as though it had never played.
        assert_eq!(ladder.rating(transient), Rating::default());
        assert_eq!(ladder.games_played(transient), 0);
        // Real versions are untouched.
        assert_eq!(ladder.games_played(champion), 20);
        assert!(ladder.above_anchor(champion) > 0.0);
        assert_eq!(ladder.rating(ANCHOR).mu, 25.0, "the anchor stays pinned");
    }

    #[test]
    fn a_ladder_encodes_to_readable_text() {
        let mut ladder = Ladder::default();
        let id = ladder.enrol(Genome::default(), 7);
        ladder.record(&[id, ANCHOR, id, ANCHOR], &[1, 2, 3, 4]);
        let text = ladder.encode();
        assert!(text.starts_with("# id generation"));
        let lines: Vec<&str> = text.lines().skip(1).collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("g007-0001"));
        // The genome tail is decodable on its own.
        let genes = lines[1]
            .split_whitespace()
            .skip(7)
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(Genome::decode(&genes), Some(Genome::default()));
    }
}
