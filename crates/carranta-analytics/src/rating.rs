//! Player rating (§10.5) and luck-adjusted performance (§10.4).
//!
//! "The Halo ranking algorithm" is TrueSkill, and the instinct behind picking
//! it is right for a specific reason: **Elo is fundamentally a two-player
//! system**, and Carranta is a 3–4 player free-for-all. Elo extensions to
//! multiplayer are pairwise-decomposition hacks. TrueSkill models N-player
//! outcomes natively and keeps a Gaussian belief `(μ, σ)` per player instead
//! of a point estimate.
//!
//! Implemented here is the **Weng–Lin Plackett–Luce** update (A-1) — the
//! OpenSkill family: TrueSkill-family behaviour, no patent exposure. Two
//! consequences worth stating:
//!
//! - **The full finishing order is used, not just the winner.** Final VP
//!   totals rank all 3–4 players, so each game yields a complete ranking
//!   rather than one bit. That roughly triples the information per game, which
//!   matters given how slowly a high-variance game converges.
//! - **Bots share the pool.** A pinned heuristic with tight σ after thousands
//!   of games is an *absolute yardstick*: "trained agent v4 at μ=32 against the
//!   heuristic at μ=25" is directly meaningful, and human ratings land on the
//!   same scale.
//!
//! This is our implementation of the published update, checked against the
//! properties the model is supposed to have — order monotonicity, σ that only
//! shrinks, symmetry under a full tie, conservation under equal uncertainty,
//! and convergence on a known true ordering. It has *not* been cross-checked
//! against a reference implementation, which is the honest limit of the
//! assurance here.
//!
//! # Ties behave oddly, and that is not yet resolved
//!
//! With four equal players, a shared second place pays both tied players
//! *less* than the average of finishing second and third — and it changes what
//! fourth place loses, even though fourth finished fourth either way. Total μ
//! is still conserved, so this is a redistribution question rather than a
//! leak, and it follows from the tie-averaging convention in the published
//! update rather than from a transcription error as far as can be told without
//! a reference to compare against.
//!
//! It matters little today: only the active player can win (R-11.1), so the
//! top position is never shared, and ties arise only between non-winners on
//! equal points. `a_tie_is_not_the_average_of_the_positions_it_spans` pins the
//! current behaviour so a change is caught. **Worth checking against a
//! reference implementation before rated play carries any stakes.**

use std::collections::{HashMap, HashSet};

use carranta_core::state::{MAX_PLAYERS, TradeMode};
use carranta_record::{Log, Payload};

use crate::stats::{Fit, least_squares};

/// A Gaussian belief about one player's skill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rating {
    /// Mean skill.
    pub mu: f64,
    /// Uncertainty. Starts wide and only ever narrows within a game update.
    pub sigma: f64,
}

impl Default for Rating {
    fn default() -> Self {
        Rating {
            mu: 25.0,
            sigma: 25.0 / 3.0,
        }
    }
}

impl Rating {
    /// What to display: `μ − 3σ`.
    ///
    /// A new player is not shown an inflated number before their uncertainty
    /// collapses, which is also what makes A-3's provisional guest ratings
    /// honest rather than misleading.
    pub fn conservative(&self) -> f64 {
        self.mu - 3.0 * self.sigma
    }
}

/// Model parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    /// Per-game performance noise. Larger means one result says less.
    pub beta: f64,
    /// Uncertainty added back each game, so a rating can track real change
    /// rather than freezing.
    pub tau: f64,
    /// Floor on the variance multiplier, so σ cannot collapse to zero.
    pub kappa: f64,
}

impl Default for Model {
    fn default() -> Self {
        Model {
            beta: 25.0 / 6.0,
            tau: 25.0 / 300.0,
            kappa: 1e-4,
        }
    }
}

impl Model {
    /// Update every player's rating from one game's finishing order.
    ///
    /// `ranks` is 1-based and ties are allowed: equal ranks mean a genuine
    /// tie, which the Plackett–Luce update handles natively.
    pub fn rate(&self, ratings: &[Rating], ranks: &[u32]) -> Vec<Rating> {
        let k = ratings.len().min(ranks.len());
        if k < 2 {
            return ratings.to_vec();
        }

        // Dynamics first: yesterday's certainty is not today's.
        let sigma: Vec<f64> = (0..k)
            .map(|i| (ratings[i].sigma.powi(2) + self.tau.powi(2)).sqrt())
            .collect();
        let mu: Vec<f64> = (0..k).map(|i| ratings[i].mu).collect();

        let c = (0..k)
            .map(|i| sigma[i].powi(2) + self.beta.powi(2))
            .sum::<f64>()
            .sqrt();
        let exp_mu: Vec<f64> = mu.iter().map(|m| (m / c).exp()).collect();

        // For each position q: the exponentiated skill still "in the running"
        // at that point, and how many players share the rank.
        let sum_q: Vec<f64> = (0..k)
            .map(|q| {
                (0..k)
                    .filter(|&s| ranks[s] >= ranks[q])
                    .map(|s| exp_mu[s])
                    .sum()
            })
            .collect();
        let ties: Vec<f64> = (0..k)
            .map(|q| (0..k).filter(|&s| ranks[s] == ranks[q]).count() as f64)
            .collect();

        let mut out = Vec::with_capacity(k);
        for i in 0..k {
            let (mut omega, mut delta) = (0.0, 0.0);
            for q in 0..k {
                if ranks[q] > ranks[i] {
                    continue;
                }
                let quotient = exp_mu[i] / sum_q[q];
                omega += (if q == i { 1.0 } else { 0.0 } - quotient) / ties[q];
                delta += quotient * (1.0 - quotient) / ties[q];
            }
            let var = sigma[i].powi(2);
            let gamma = sigma[i] / c;
            omega *= var / c;
            delta *= gamma * var / (c * c);

            out.push(Rating {
                mu: mu[i] + omega,
                sigma: sigma[i] * (1.0 - delta).max(self.kappa).sqrt(),
            });
        }
        out
    }
}

/// Finishing order from final victory points, 1-based, ties shared.
///
/// The winner is placed first outright: only the active player can win
/// (R-11.1), so a tie in raw points at the top is not a tie in the game.
pub fn finishing_order(winner: Option<u8>, vp: &[u32; MAX_PLAYERS], players: usize) -> Vec<u32> {
    let mut ranks = vec![1u32; players];
    for i in 0..players {
        if Some(i as u8) == winner {
            continue;
        }
        // One rank per player who finished strictly ahead.
        let ahead = (0..players)
            .filter(|&j| j != i)
            .filter(|&j| Some(j as u8) == winner || vp[j] > vp[i])
            .count();
        ranks[i] = ahead as u32 + 1;
    }
    ranks
}

/// Which rating pool a game belongs to (A-2).
///
/// Deliberately coarse. Every extra pool fragments ratings and slows
/// convergence, so a configuration earns its own pool only when it genuinely
/// changes how the game is played — trade mode does, a cosmetic option does
/// not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PoolKey {
    pub trade_mode: TradeMode,
    pub rules_version: u16,
}

impl PoolKey {
    pub fn of(log: &Log) -> Self {
        PoolKey {
            trade_mode: log.created.trade_mode,
            rules_version: log.created.rules_version,
        }
    }
}

/// One rating pool.
#[derive(Clone, Debug, Default)]
pub struct Pool {
    pub model: Model,
    ratings: HashMap<u64, Rating>,
    games: HashMap<u64, u32>,
    pinned: HashSet<u64>,
}

impl Pool {
    pub fn new(model: Model) -> Self {
        Pool {
            model,
            ratings: HashMap::new(),
            games: HashMap::new(),
            pinned: HashSet::new(),
        }
    }

    /// Hold a player's μ fixed, so it defines the scale instead of moving on
    /// it (§10.5 design point 3).
    ///
    /// Without this a "pinned" reference is nothing of the kind: it sinks as
    /// it loses to a population that keeps improving, and the gap between an
    /// old version and a new one stops meaning the same thing over the course
    /// of a run — the drift between eras that afflicts any long-lived ladder.
    ///
    /// σ is deliberately left free. It reflects games genuinely played, and
    /// letting it tighten is what makes each of the anchor's games informative;
    /// it is μ alone that has to stay still to be an origin.
    pub fn pin(&mut self, player: u64) {
        self.ratings.entry(player).or_default();
        self.pinned.insert(player);
    }

    /// Whether a player's μ is held fixed.
    pub fn is_pinned(&self, player: u64) -> bool {
        self.pinned.contains(&player)
    }

    /// Every player the pool has a rating for.
    pub fn players(&self) -> Vec<u64> {
        self.ratings.keys().copied().collect()
    }

    /// Drop a player entirely.
    ///
    /// For identities that were never meant to persist — a placeholder that
    /// stood in for a different opponent each time it appeared. Leaving those
    /// in place lets one id accumulate a history belonging to nobody.
    pub fn forget(&mut self, player: u64) {
        self.ratings.remove(&player);
        self.games.remove(&player);
        self.pinned.remove(&player);
    }

    /// Put a player back at a known rating and game count.
    ///
    /// For restoring a saved pool, not for editing a live one: a rating that
    /// did not come from recorded games is a claim about a player that no
    /// result supports.
    pub fn restore(&mut self, player: u64, rating: Rating, games: u32) {
        self.ratings.insert(player, rating);
        self.games.insert(player, games);
    }

    /// The current belief about a player. Unseen players start at the prior.
    pub fn rating(&self, player: u64) -> Rating {
        self.ratings.get(&player).copied().unwrap_or_default()
    }

    pub fn games_played(&self, player: u64) -> u32 {
        self.games.get(&player).copied().unwrap_or(0)
    }

    /// Apply one finished game, given the seats and their finishing order.
    pub fn record_ranked(&mut self, players: &[u64], ranks: &[u32]) {
        let before: Vec<Rating> = players.iter().map(|&p| self.rating(p)).collect();
        let after = self.model.rate(&before, ranks);
        for ((&p, mut r), was) in players.iter().zip(after).zip(&before) {
            if self.pinned.contains(&p) {
                r.mu = was.mu;
            }
            self.ratings.insert(p, r);
            *self.games.entry(p).or_insert(0) += 1;
        }
    }

    /// Apply a recorded game.
    ///
    /// Returns `false` for a game that did not finish — an unfinished game
    /// ranks nobody. Games where a bot substituted for a departed human (P-2)
    /// must also be excluded, but nothing in the log marks them yet, so that
    /// remains the caller's filter rather than something checked here.
    pub fn record(&mut self, log: &Log) -> bool {
        let Some(Payload::Ended { winner, vp }) = log.events.last().map(|e| &e.payload) else {
            return false;
        };
        if winner.is_none() {
            return false;
        }
        let players: Vec<u64> = log.created.seats.iter().map(|s| s.player).collect();
        let ranks = finishing_order(*winner, vp, players.len());
        self.record_ranked(&players, &ranks);
        true
    }

    /// Rated players above a games-played threshold, best first (A-5).
    ///
    /// The threshold is not decoration: ranking players publicly before σ has
    /// converged shows noise as if it were skill.
    pub fn leaderboard(&self, min_games: u32) -> Vec<(u64, Rating, u32)> {
        let mut rows: Vec<(u64, Rating, u32)> = self
            .ratings
            .iter()
            .map(|(&p, &r)| (p, r, self.games_played(p)))
            .filter(|(_, _, n)| *n >= min_games)
            .collect();
        rows.sort_by(|a, b| {
            b.1.conservative()
                .total_cmp(&a.1.conservative())
                .then(a.0.cmp(&b.0))
        });
        rows
    }

    pub fn len(&self) -> usize {
        self.ratings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ratings.is_empty()
    }
}

/// Every pool, segmented per A-2.
#[derive(Clone, Debug, Default)]
pub struct Ratings {
    model: Model,
    pools: HashMap<PoolKey, Pool>,
}

impl Ratings {
    pub fn new(model: Model) -> Self {
        Ratings {
            model,
            pools: HashMap::new(),
        }
    }

    /// Route a game to its pool and apply it.
    pub fn record(&mut self, log: &Log) -> bool {
        let key = PoolKey::of(log);
        let model = self.model;
        self.pools
            .entry(key)
            .or_insert_with(|| Pool::new(model))
            .record(log)
    }

    pub fn pool(&self, key: PoolKey) -> Option<&Pool> {
        self.pools.get(&key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &PoolKey> {
        self.pools.keys()
    }
}

// ---------------------------------------------------------------------------
// §10.4 Luck-adjusted performance
// ---------------------------------------------------------------------------

/// How well players convert production into victory points.
///
/// Rating measures results, and results here carry a large chance component.
/// The complementary question is **VP earned relative to what a player's
/// production entitled them to**: fit final VP on total production across the
/// corpus, and report each player-game's residual. Above the curve means they
/// converted resources better than average.
///
/// This is only computable because §10.2 gives an exact expectation rather
/// than an estimate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LuckAdjustment {
    pub fit: Fit,
}

impl LuckAdjustment {
    /// Fit over `(total production, final VP)` pairs, one per player-game.
    pub fn fit(samples: &[(f64, f64)]) -> Option<Self> {
        let x: Vec<f64> = samples.iter().map(|s| s.0).collect();
        let y: Vec<f64> = samples.iter().map(|s| s.1).collect();
        least_squares(&x, &y).map(|fit| LuckAdjustment { fit })
    }

    /// How far above the curve a player-game landed, in victory points.
    ///
    /// The single most useful "were you good or lucky" number.
    pub fn residual(&self, production: f64, victory_points: f64) -> f64 {
        victory_points - self.fit.predict(production)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn four() -> Vec<Rating> {
        vec![Rating::default(); 4]
    }

    #[test]
    fn winning_raises_skill_and_losing_lowers_it() {
        let m = Model::default();
        let after = m.rate(&four(), &[1, 2, 3, 4]);
        assert!(after[0].mu > 25.0, "the winner did not gain");
        assert!(after[3].mu < 25.0, "last place did not lose");
    }

    #[test]
    fn the_update_is_monotone_in_finishing_order() {
        let m = Model::default();
        let after = m.rate(&four(), &[1, 2, 3, 4]);
        for i in 0..3 {
            assert!(
                after[i].mu > after[i + 1].mu,
                "position {i} did not beat {}: {:?}",
                i + 1,
                after.iter().map(|r| r.mu).collect::<Vec<_>>()
            );
        }
        // This is the point of using the whole order rather than just the
        // winner: third place is meaningfully distinguished from fourth.
        assert!(after[2].mu - after[3].mu > 1e-3);
    }

    #[test]
    fn uncertainty_only_ever_shrinks_within_a_game() {
        let m = Model::default();
        let start = four();
        let after = m.rate(&start, &[1, 2, 3, 4]);
        for i in 0..4 {
            assert!(
                after[i].sigma < start[i].sigma,
                "seat {i}: σ {} → {}",
                start[i].sigma,
                after[i].sigma
            );
        }
    }

    #[test]
    fn a_full_tie_moves_nobody() {
        let m = Model::default();
        let after = m.rate(&four(), &[1, 1, 1, 1]);
        for r in &after {
            assert!((r.mu - 25.0).abs() < 1e-12, "a tie moved μ to {}", r.mu);
        }
    }

    #[test]
    fn tied_players_receive_identical_updates() {
        let m = Model::default();
        let tied = m.rate(&four(), &[1, 2, 2, 4]);
        assert!((tied[1].mu - tied[2].mu).abs() < 1e-12);
        assert!((tied[1].sigma - tied[2].sigma).abs() < 1e-12);
        // The winner is untouched by what happens behind them.
        let split = m.rate(&four(), &[1, 2, 3, 4]);
        assert!((tied[0].mu - split[0].mu).abs() < 1e-12);
    }

    #[test]
    fn a_tie_is_not_the_average_of_the_positions_it_spans() {
        // Pinning down behaviour that is genuinely counterintuitive, so a
        // change to it is caught rather than absorbed. See the note on ties in
        // the module docs: a shared second place pays *less* than averaging
        // second and third, and it changes what fourth place loses even though
        // fourth finished fourth either way.
        let m = Model::default();
        let split = m.rate(&four(), &[1, 2, 3, 4]);
        let tied = m.rate(&four(), &[1, 2, 2, 4]);

        let spanned_average = (split[1].mu + split[2].mu) / 2.0;
        assert!(tied[1].mu < spanned_average - 0.5, "behaviour changed");
        assert!(
            tied[3].mu > split[3].mu + 0.5,
            "fourth place used to be affected by a tie above it"
        );
        // Conservation still holds, which is why this is a redistribution
        // question and not a leak.
        let total: f64 = tied.iter().map(|r| r.mu).sum();
        assert!((total - 100.0).abs() < 1e-9);
    }

    #[test]
    fn skill_is_conserved_when_uncertainty_is_equal() {
        let m = Model::default();
        let after = m.rate(&four(), &[1, 2, 3, 4]);
        let total: f64 = after.iter().map(|r| r.mu).sum();
        assert!(
            (total - 100.0).abs() < 1e-9,
            "μ leaked: total {total}, expected 100"
        );
    }

    #[test]
    fn a_stronger_player_gains_less_from_the_same_win() {
        let m = Model::default();
        let mut strong = four();
        strong[0].mu = 35.0;
        let baseline = m.rate(&four(), &[1, 2, 3, 4]);
        let after = m.rate(&strong, &[1, 2, 3, 4]);
        assert!(
            after[0].mu - 35.0 < baseline[0].mu - 25.0,
            "beating weaker players paid as much as an even game"
        );
    }

    #[test]
    fn ratings_converge_on_a_true_ordering() {
        // Four players of fixed, different strength. After enough games the
        // rating order should match the real one, and σ should have shrunk.
        let mut pool = Pool::new(Model::default());
        let strength = [0.55, 0.28, 0.12, 0.05];
        let mut rng = carranta_core::rng::Rng::new(99);

        for _ in 0..400 {
            // Sample a finishing order weighted by strength, without
            // replacement — a Plackett–Luce draw, which is the model's own
            // generative story.
            let mut left: Vec<usize> = (0..4).collect();
            let mut order = Vec::new();
            while !left.is_empty() {
                let total: f64 = left.iter().map(|&i| strength[i]).sum();
                let mut u = rng.below(carranta_core::rng::Stream::Dice, 1_000_000) as f64
                    / 1_000_000.0
                    * total;
                let mut pick = left.len() - 1;
                for (idx, &i) in left.iter().enumerate() {
                    u -= strength[i];
                    if u <= 0.0 {
                        pick = idx;
                        break;
                    }
                }
                order.push(left.remove(pick));
            }
            let mut ranks = [0u32; 4];
            for (place, &p) in order.iter().enumerate() {
                ranks[p] = place as u32 + 1;
            }
            pool.record_ranked(&[0, 1, 2, 3], &ranks);
        }

        let mus: Vec<f64> = (0..4).map(|p| pool.rating(p).mu).collect();
        for i in 0..3 {
            assert!(mus[i] > mus[i + 1], "converged to the wrong order: {mus:?}");
        }
        assert!(
            pool.rating(0).sigma < 25.0 / 3.0 * 0.5,
            "σ barely moved: {}",
            pool.rating(0).sigma
        );
        assert_eq!(pool.games_played(0), 400);
    }

    #[test]
    fn a_pinned_player_defines_the_scale_instead_of_moving_on_it() {
        let mut pool = Pool::new(Model::default());
        pool.pin(0);
        assert!(pool.is_pinned(0));
        let start = pool.rating(0);

        // Lose two hundred games outright. An unpinned player would sink.
        for _ in 0..200 {
            pool.record_ranked(&[0, 1, 2, 3], &[4, 1, 2, 3]);
        }
        assert_eq!(pool.rating(0).mu, start.mu, "the anchor moved");
        assert!(
            pool.rating(0).sigma < start.sigma,
            "sigma should still learn"
        );
        assert_eq!(pool.games_played(0), 200);
        // And everyone who beat it has risen above it on a fixed scale.
        assert!(pool.rating(1).mu > start.mu);
    }

    #[test]
    fn a_pool_survives_a_round_trip_through_restore() {
        let mut pool = Pool::new(Model::default());
        pool.pin(0);
        for _ in 0..50 {
            pool.record_ranked(&[0, 1, 2, 3], &[2, 1, 4, 3]);
        }

        let mut copy = Pool::new(Model::default());
        copy.pin(0);
        for p in 0..4 {
            copy.restore(p, pool.rating(p), pool.games_played(p));
        }
        for p in 0..4 {
            assert_eq!(copy.rating(p), pool.rating(p));
            assert_eq!(copy.games_played(p), pool.games_played(p));
        }
        // And carries on from there identically.
        pool.record_ranked(&[0, 1, 2, 3], &[1, 2, 3, 4]);
        copy.record_ranked(&[0, 1, 2, 3], &[1, 2, 3, 4]);
        for p in 0..4 {
            assert_eq!(copy.rating(p), pool.rating(p), "player {p} diverged");
        }
    }

    #[test]
    fn pinning_does_not_bias_the_pool() {
        // Holding one player's μ fixed breaks the conservation that an
        // ordinary update has, so it is worth checking that the rest of the
        // pool does not drift away from the pin as a result. Four identical
        // players, orders drawn uniformly at random: any gap that opens is
        // bias, not skill.
        let drift = |pinned: bool| {
            let mut pool = Pool::new(Model::default());
            if pinned {
                pool.pin(0);
            }
            let mut rng = carranta_core::rng::Rng::new(1);
            for _ in 0..20_000 {
                let mut order: Vec<usize> = (0..4).collect();
                for i in (1..4).rev() {
                    let j = rng.below(carranta_core::rng::Stream::Dice, i as u32 + 1) as usize;
                    order.swap(i, j);
                }
                let mut ranks = [0u32; 4];
                for (place, &p) in order.iter().enumerate() {
                    ranks[p] = place as u32 + 1;
                }
                pool.record_ranked(&[0, 1, 2, 3], &ranks);
            }
            (1..4)
                .map(|p| pool.rating(p).mu - pool.rating(0).mu)
                .sum::<f64>()
                / 3.0
        };
        // The pinned pool must be no worse than the unpinned one, and both
        // well inside anything a real effect would show as.
        assert!(drift(true).abs() < 1.0, "pinned drift {:.3}", drift(true));
        assert!(drift(true).abs() <= drift(false).abs() + 0.1);
    }

    #[test]
    fn a_pin_keeps_two_eras_comparable() {
        // The failure this exists to prevent: an early version and a late one
        // that never meet, each rated only against the reference. Without a
        // pin the reference sinks between the two sets of games, and the later
        // version is measured against a weaker opponent — so the better record
        // can come out looking worse.
        let record = |pinned: bool| {
            let mut pool = Pool::new(Model::default());
            if pinned {
                pool.pin(0);
            }
            for _ in 0..150 {
                pool.record_ranked(&[1, 0, 1, 0], &[1, 2, 3, 4]); // early: mean 2 vs 3
            }
            for _ in 0..150 {
                pool.record_ranked(&[2, 0, 2, 0], &[1, 3, 2, 4]); // late: mean 1.5 vs 3.5
            }
            (
                pool.rating(1).mu - pool.rating(0).mu,
                pool.rating(2).mu - pool.rating(0).mu,
            )
        };

        let (early, late) = record(false);
        assert!(
            early > late,
            "the drift this test describes has changed shape"
        );

        let (early, late) = record(true);
        assert!(
            late > early,
            "pinned: late {late:.2} should beat early {early:.2}"
        );
    }

    #[test]
    fn the_displayed_rating_is_conservative() {
        let fresh = Rating::default();
        assert_eq!(fresh.conservative(), 0.0);
        // Certainty is what earns a displayed rating near μ.
        let settled = Rating {
            mu: 25.0,
            sigma: 1.0,
        };
        assert_eq!(settled.conservative(), 22.0);
        assert!(settled.conservative() > fresh.conservative());
    }

    #[test]
    fn a_leaderboard_hides_players_who_have_not_converged() {
        let mut pool = Pool::new(Model::default());
        for _ in 0..30 {
            pool.record_ranked(&[1, 2, 3, 4], &[1, 2, 3, 4]);
        }
        pool.record_ranked(&[5, 2, 3, 4], &[1, 2, 3, 4]);

        let board = pool.leaderboard(10);
        assert!(
            board.iter().all(|(p, _, _)| *p != 5),
            "a 1-game player shown"
        );
        assert_eq!(board[0].0, 1, "the consistent winner is not on top");
        assert!(
            board
                .windows(2)
                .all(|w| w[0].1.conservative() >= w[1].1.conservative())
        );
    }

    #[test]
    fn finishing_order_puts_the_winner_first() {
        // The winner has fewer raw points than nobody here, but two others are
        // level behind them.
        let ranks = finishing_order(Some(2), &[8, 8, 10, 4], 4);
        assert_eq!(ranks[2], 1);
        assert_eq!(ranks[0], 2);
        assert_eq!(ranks[1], 2, "equal points is a genuine tie");
        assert_eq!(ranks[3], 4, "a tie consumes the position below it");
    }

    #[test]
    fn a_winner_on_fewer_visible_points_still_ranks_first() {
        // Hidden Victory Point cards mean the winner can trail on the board.
        let ranks = finishing_order(Some(1), &[9, 10, 9, 3], 4);
        assert_eq!(ranks[1], 1);
        assert_eq!(ranks[0], 2);
    }

    #[test]
    fn luck_adjustment_separates_conversion_from_production() {
        // Everyone on the same line except one player who turned the same
        // production into two extra points.
        let mut samples: Vec<(f64, f64)> = (0..200)
            .map(|i| {
                let prod = 40.0 + (i % 30) as f64;
                (prod, 2.0 + 0.1 * prod)
            })
            .collect();
        let fit = LuckAdjustment::fit(&samples).unwrap();
        assert!((fit.fit.slope - 0.1).abs() < 1e-9);
        assert!(fit.residual(60.0, 8.0).abs() < 1e-9);

        samples.push((60.0, 10.0));
        let fit = LuckAdjustment::fit(&samples).unwrap();
        assert!(
            fit.residual(60.0, 10.0) > 1.5,
            "an over-performer did not show above the curve"
        );
        assert!(fit.residual(60.0, 6.0) < -1.5);
    }
}
