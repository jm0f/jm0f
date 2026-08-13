//! How the population *plays*, not just how well.
//!
//! A rating that climbs says something improved; it does not say what. These
//! are the markers that answer the second question — did the champion start
//! trading more, building cities earlier, picking different openings — and they
//! come from running the existing §10 analysis over a small sample of the games
//! a generation already played.
//!
//! Sampling is the point. Recording every game would multiply the cost of a
//! run for data nobody reads; one game in a few hundred is enough to see a
//! trend across generations, and costs nothing measurable.

use carranta_analytics::{game, production};
use carranta_record::Log;

/// Behavioural markers averaged over the sampled games of one generation.
///
/// All per game unless noted. Nothing here is a fitness signal — selection
/// never sees it — so it can be read as an honest description of play rather
/// than as something the population is being pushed toward.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Behaviour {
    pub games: u32,
    pub turns: f64,
    /// Trades completed, counted once per party.
    pub trades: f64,
    pub offers_made: f64,
    pub maritime_trades: f64,
    pub settlements_built: f64,
    pub cities_built: f64,
    pub roads_built: f64,
    pub dev_bought: f64,
    pub militia_played: f64,
    pub robber_moves: f64,
    /// Cards lost to robberies, per seat.
    pub robbed_of: f64,
    /// Pips on the two starting settlements, per seat.
    pub opening_pips: f64,
    /// Distinct resources the opening reaches, per seat.
    pub opening_diversity: f64,
    /// Cards actually produced, per seat.
    pub production: f64,
    /// Production lost to the robber, per seat — a social cost, not a random
    /// one (§10.2).
    pub robber_cost: f64,
    /// Winner's victory points.
    pub winning_vp: f64,
}

/// Accumulates markers over a generation's sampled games.
#[derive(Clone, Copy, Debug, Default)]
pub struct Sampler {
    total: Behaviour,
}

impl Sampler {
    /// Fold one recorded game in. Games that fail to analyse are skipped
    /// rather than aborting a run — a training run should not die because one
    /// sampled game was odd.
    pub fn add(&mut self, log: &Log) {
        let (Ok(summary), Ok(prod)) = (game::analyse(log), production::analyse(log)) else {
            return;
        };
        let seats = summary.players.max(1) as f64;
        let t = &mut self.total;
        t.games += 1;
        t.turns += summary.turns as f64;
        // Halved: a completed trade is recorded for both parties.
        t.trades += (0..4)
            .map(|p| summary.trades_completed[p] as f64)
            .sum::<f64>()
            / 2.0;
        t.offers_made += (0..4).map(|p| summary.offers_made[p] as f64).sum::<f64>() / seats;
        t.maritime_trades += (0..4)
            .map(|p| summary.maritime_trades[p] as f64)
            .sum::<f64>()
            / seats;
        t.settlements_built += (0..4)
            .map(|p| summary.builds[p].settlements as f64)
            .sum::<f64>()
            / seats;
        t.cities_built += (0..4).map(|p| summary.builds[p].cities as f64).sum::<f64>() / seats;
        t.roads_built += (0..4).map(|p| summary.builds[p].roads as f64).sum::<f64>() / seats;
        t.dev_bought += (0..4).map(|p| summary.dev_bought[p] as f64).sum::<f64>() / seats;
        t.militia_played += (0..4)
            .map(|p| summary.dev_played[p][carranta_core::state::DevCard::Militia as usize] as f64)
            .sum::<f64>()
            / seats;
        t.robber_moves += summary.robber_moves as f64;
        t.robbed_of += (0..4).map(|p| summary.robbed_of(p) as f64).sum::<f64>() / seats;
        t.opening_pips += (0..4).map(|p| summary.opening[p].pips as f64).sum::<f64>() / seats;
        t.opening_diversity += (0..4)
            .map(|p| summary.opening[p].diversity as f64)
            .sum::<f64>()
            / seats;
        t.production += (0..4).map(|p| prod.decompose(p).actual).sum::<f64>() / seats;
        t.robber_cost += (0..4).map(|p| prod.decompose(p).robber_cost).sum::<f64>() / seats;
        if let Some(w) = summary.winner {
            t.winning_vp += summary.vp[w as usize] as f64;
        }
    }

    /// The averages. Zeroed when nothing was sampled.
    pub fn finish(&self) -> Behaviour {
        let n = self.total.games;
        if n == 0 {
            return Behaviour::default();
        }
        let d = n as f64;
        let t = self.total;
        Behaviour {
            games: n,
            turns: t.turns / d,
            trades: t.trades / d,
            offers_made: t.offers_made / d,
            maritime_trades: t.maritime_trades / d,
            settlements_built: t.settlements_built / d,
            cities_built: t.cities_built / d,
            roads_built: t.roads_built / d,
            dev_bought: t.dev_bought / d,
            militia_played: t.militia_played / d,
            robber_moves: t.robber_moves / d,
            robbed_of: t.robbed_of / d,
            opening_pips: t.opening_pips / d,
            opening_diversity: t.opening_diversity / d,
            production: t.production / d,
            robber_cost: t.robber_cost / d,
            winning_vp: t.winning_vp / d,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::{Arena, Job};
    use crate::genome::Genome;

    #[test]
    fn a_sampled_game_yields_plausible_markers() {
        let arena = Arena::default();
        let base = Genome::default();
        let mut sampler = Sampler::default();
        for seed in 0..8 {
            let (_, log) = arena.play_recorded(&Job {
                seed,
                seats: [base; 4],
            });
            sampler.add(&log);
        }
        let b = sampler.finish();
        assert_eq!(b.games, 8);
        assert!(b.turns > 20.0, "turns {}", b.turns);
        assert!(b.winning_vp >= 10.0, "R-11.1: {}", b.winning_vp);
        assert!(b.settlements_built > 1.0, "setup alone places two");
        assert!(b.production > 20.0, "production {}", b.production);
        assert!(b.robber_cost >= 0.0, "the robber never pays a bonus");
        assert!(b.opening_pips > 0.0 && b.opening_pips < 30.0);
        assert!((1.0..=5.0).contains(&b.opening_diversity));
        // The market is open by default, so trades must show up.
        assert!(b.trades > 0.0, "no trades under {:?}", arena.mode);
    }

    #[test]
    fn markers_show_a_strategy_shift_and_not_just_a_number() {
        // The whole point: a change a rating alone would describe only as
        // "worse". Suppressing development-card purchases does not just move
        // one marker — it cascades. No cards bought means no Militia to play,
        // and the resources go into cities instead.
        let arena = Arena::default();
        let base = Genome::default();
        let mut no_dev = base;
        no_dev.genes[11] = -100; // buy_dev

        let sample = |g: Genome| {
            let mut s = Sampler::default();
            for seed in 0..10 {
                let (_, log) = arena.play_recorded(&Job {
                    seed,
                    seats: [g; 4],
                });
                s.add(&log);
            }
            s.finish()
        };
        let without = sample(no_dev);
        let normal = sample(base);

        assert_eq!(without.dev_bought, 0.0, "the lever did not bite");
        assert!(
            normal.dev_bought > 3.0,
            "baseline buys cards: {:.2}",
            normal.dev_bought
        );
        assert_eq!(without.militia_played, 0.0, "no cards, no Militia");
        assert!(
            without.cities_built > normal.cities_built,
            "the resources went somewhere: {:.2} cities against {:.2}",
            without.cities_built,
            normal.cities_built
        );
    }

    #[test]
    fn the_offer_toll_cannot_quiet_the_first_ask_of_a_turn() {
        // Structural, and worth pinning: the toll is charged per offer already
        // *made this turn*, so the first ask of every turn is always free.
        // Raising `offer_cost` from its default to a punitive value changes
        // nothing, because the bot is already down to about one offer a turn.
        //
        // If offers ever need to be suppressed outright, this is not the lever
        // — `offer_discount`, which scales the credit a proposal earns, is.
        let arena = Arena::default();
        let base = Genome::default();
        let mut punitive = base;
        punitive.genes[14] = 500; // offer_cost

        let sample = |g: Genome| {
            let mut s = Sampler::default();
            for seed in 0..8 {
                let (_, log) = arena.play_recorded(&Job {
                    seed,
                    seats: [g; 4],
                });
                s.add(&log);
            }
            s.finish()
        };
        assert_eq!(
            sample(punitive).offers_made,
            sample(base).offers_made,
            "the toll has started biting on the first ask"
        );
    }

    #[test]
    fn a_negative_offer_discount_inverts_the_bot_rather_than_silencing_it() {
        // Found while writing the test above, and worth pinning because it is
        // counterintuitive and evolution can wander into it.
        //
        // The discount multiplies the *gain* a proposal would bring. Negative,
        // it does not make offers unattractive — it makes the bot prefer
        // proposals whose gain is negative, which is to say deals that are bad
        // for itself. Those are far more plentiful than good ones, so the bot
        // gets *louder*, not quieter.
        //
        // This is exactly what behavioural markers are for. A genome like this
        // is pathological in a way its fitness alone would describe only as
        // "worse", with no hint of why.
        let arena = Arena::default();
        let base = Genome::default();
        let mut inverted = base;
        inverted.genes[13] = -200; // offer_discount

        let sample = |g: Genome| {
            let mut s = Sampler::default();
            for seed in 0..10 {
                let (_, log) = arena.play_recorded(&Job {
                    seed,
                    seats: [g; 4],
                });
                s.add(&log);
            }
            s.finish()
        };
        assert!(
            sample(inverted).offers_made > sample(base).offers_made,
            "the inversion described above has changed shape"
        );
    }

    #[test]
    fn an_empty_sample_is_not_an_error() {
        assert_eq!(Sampler::default().finish(), Behaviour::default());
    }
}
