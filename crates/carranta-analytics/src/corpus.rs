//! Aggregates across many games (§10.3 "corpus and balance", §10.4).
//!
//! What goes into a corpus is the caller's decision, and it is a real one.
//! §10.6 lists the ways an aggregate here can mislead, and two of them are
//! decided at this boundary rather than inside any calculation:
//!
//! - **Configuration heterogeneity.** `rules_version`, trade mode and setup
//!   variant all change gameplay. Mixing them silently compares incomparable
//!   games, so [`Corpus::accepts`] refuses a game that does not match the
//!   configuration the corpus was opened with.
//! - **Bot games swamp human data.** Self-play corpora are orders of magnitude
//!   larger. Nothing here pools them for you; build separate corpora.
//!
//! One more, which cannot be fixed at this boundary and must be remembered
//! when reading anything below: **players within a game are not independent**.
//! One player's gain is literally another's loss, so treating player-games as
//! i.i.d. understates the variance of any aggregate over them.

use carranta_core::state::{MAX_PLAYERS, TradeMode};
use carranta_record::{Log, Payload};

use crate::dice::{self, Audit};
use crate::game;
use crate::production;
use crate::rating::{LuckAdjustment, Ratings};
use crate::stats::benjamini_hochberg;

/// The configuration a corpus is restricted to (§7.4's mandatory filters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub trade_mode: TradeMode,
    pub rules_version: u16,
}

impl Config {
    pub fn of(log: &Log) -> Self {
        Config {
            trade_mode: log.created.trade_mode,
            rules_version: log.created.rules_version,
        }
    }
}

/// One player-game, for the luck adjustment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerGame {
    pub player: u64,
    pub seat: u8,
    pub production: f64,
    pub victory_points: f64,
    pub won: bool,
}

/// Accumulated statistics over a body of games sharing one configuration.
#[derive(Clone, Debug)]
pub struct Corpus {
    pub config: Config,
    pub games: u32,
    /// Games that reached a winner.
    pub finished: u32,
    pub seat_wins: [u32; MAX_PLAYERS],
    pub seat_games: [u32; MAX_PLAYERS],
    /// Every roll, in order, for the generator audit (§10.1b).
    pub rolls: Vec<u8>,
    /// Per-game dice effect sizes, for percentile reporting (§10.1a).
    pub dice_deviations: Vec<f64>,
    /// Per-game dice p-values, kept only so they can be corrected together.
    pub dice_p_values: Vec<f64>,
    pub player_games: Vec<PlayerGame>,
    pub ratings: Ratings,
    /// Mean turns per finished game.
    turns: u64,
}

impl Corpus {
    /// Open a corpus for one configuration.
    pub fn new(config: Config) -> Self {
        Corpus {
            config,
            games: 0,
            finished: 0,
            seat_wins: [0; MAX_PLAYERS],
            seat_games: [0; MAX_PLAYERS],
            rolls: Vec::new(),
            dice_deviations: Vec::new(),
            dice_p_values: Vec::new(),
            player_games: Vec::new(),
            ratings: Ratings::default(),
            turns: 0,
        }
    }

    /// Whether a game belongs in this corpus at all.
    pub fn accepts(&self, log: &Log) -> bool {
        Config::of(log) == self.config
    }

    /// Fold one game in. Returns false if it does not match the configuration.
    ///
    /// `sims` is the Monte Carlo budget for the per-game dice p-value; pass 0
    /// to skip it, which is worth doing on a large corpus where only the
    /// pooled audit and the effect sizes matter.
    pub fn add(&mut self, log: &Log, sims: u32) -> bool {
        if !self.accepts(log) {
            return false;
        }
        let summary = match game::analyse(log) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let prod = match production::analyse(log) {
            Ok(p) => p,
            Err(_) => return false,
        };

        self.games += 1;
        self.turns += summary.turns as u64;
        let seats = summary.players as usize;
        for p in 0..seats {
            self.seat_games[p] += 1;
        }
        if let Some(w) = summary.winner {
            self.finished += 1;
            self.seat_wins[w as usize] += 1;
        }

        let game_rolls = dice::rolls(log);
        let d = if sims > 0 {
            dice::analyse_game(&game_rolls, sims, log.game_id)
        } else {
            let counts = dice::histogram(&game_rolls);
            dice::GameDice {
                rolls: counts.iter().sum(),
                counts,
                sevens: counts[5],
                kl_bits: crate::stats::kl_divergence_bits(&counts, &dice::REFERENCE),
                chi_squared: 0.0,
                p_value: 1.0,
            }
        };
        self.dice_deviations.push(d.kl_bits);
        if sims > 0 {
            self.dice_p_values.push(d.p_value);
        }
        self.rolls.extend_from_slice(&game_rolls);

        for p in 0..seats {
            self.player_games.push(PlayerGame {
                player: log
                    .created
                    .seats
                    .get(p)
                    .map(|s| s.player)
                    .unwrap_or(p as u64),
                seat: p as u8,
                production: prod.total_actual(p) as f64,
                victory_points: summary.vp[p] as f64,
                won: summary.winner == Some(p as u8),
            });
        }
        self.ratings.record(log);
        true
    }

    /// Win rate by seat — the first-player-advantage question (A-4).
    ///
    /// A corpus with randomised seating makes this the honest measure; one
    /// where a strong player always sat first does not, and no calculation
    /// here can tell the difference.
    pub fn seat_win_rate(&self) -> [f64; MAX_PLAYERS] {
        core::array::from_fn(|p| {
            if self.seat_games[p] == 0 {
                0.0
            } else {
                self.seat_wins[p] as f64 / self.seat_games[p] as f64
            }
        })
    }

    /// The generator audit over every pooled roll (§10.1b).
    pub fn dice_audit(&self) -> Audit {
        dice::audit(&self.rolls)
    }

    /// Per-game deviations, for reporting one game as a percentile.
    pub fn dice_corpus(&self) -> dice::Corpus {
        dice::Corpus::from_games(self.dice_deviations.iter().copied())
    }

    /// Games whose dice survive false-discovery-rate control at level `q`.
    ///
    /// Without this, ~5% of games clear p<0.05 by construction and every one
    /// of them looks like evidence. Returns the game indices, in insertion
    /// order.
    pub fn dice_outliers(&self, q: f64) -> Vec<usize> {
        benjamini_hochberg(&self.dice_p_values, q)
            .into_iter()
            .enumerate()
            .filter(|(_, flagged)| *flagged)
            .map(|(i, _)| i)
            .collect()
    }

    /// Fit victory points on production across the corpus (§10.4).
    pub fn luck_adjustment(&self) -> Option<LuckAdjustment> {
        let samples: Vec<(f64, f64)> = self
            .player_games
            .iter()
            .map(|g| (g.production, g.victory_points))
            .collect();
        LuckAdjustment::fit(&samples)
    }

    /// How far above the production curve each player sits, averaged over
    /// their games — the "were you good or lucky" number.
    pub fn conversion_residuals(&self) -> Vec<(u64, f64, u32)> {
        let Some(fit) = self.luck_adjustment() else {
            return Vec::new();
        };
        let mut sums: std::collections::HashMap<u64, (f64, u32)> = Default::default();
        for g in &self.player_games {
            let e = sums.entry(g.player).or_insert((0.0, 0));
            e.0 += fit.residual(g.production, g.victory_points);
            e.1 += 1;
        }
        let mut out: Vec<(u64, f64, u32)> = sums
            .into_iter()
            .map(|(p, (sum, n))| (p, sum / n as f64, n))
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }

    pub fn mean_turns(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.turns as f64 / self.games as f64
        }
    }
}

/// Split games into one corpus per configuration, so nothing incomparable is
/// ever pooled (§10.6.4).
pub fn segment<'a>(logs: impl IntoIterator<Item = &'a Log>, sims: u32) -> Vec<Corpus> {
    let mut out: Vec<Corpus> = Vec::new();
    for log in logs {
        let config = Config::of(log);
        if let Some(c) = out.iter_mut().find(|c| c.config == config) {
            c.add(log, sims);
        } else {
            let mut c = Corpus::new(config);
            c.add(log, sims);
            out.push(c);
        }
    }
    out
}

/// Total actions in a log, for reporting.
pub fn actions(log: &Log) -> usize {
    log.events
        .iter()
        .filter(|e| matches!(e.payload, Payload::Decision { .. }))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::self_play;

    #[test]
    fn a_corpus_refuses_a_game_from_another_configuration() {
        let full = self_play(1, TradeMode::Full);
        let off = self_play(1, TradeMode::Disabled);
        let mut c = Corpus::new(Config::of(&full));
        assert!(c.add(&full, 0));
        assert!(!c.add(&off, 0), "configurations were silently pooled");
        assert_eq!(c.games, 1);
    }

    #[test]
    fn segmenting_keeps_configurations_apart() {
        let logs: Vec<_> = (0..6)
            .map(|i| {
                self_play(
                    i,
                    if i % 2 == 0 {
                        TradeMode::Full
                    } else {
                        TradeMode::Disabled
                    },
                )
            })
            .collect();
        let corpora = segment(logs.iter(), 0);
        assert_eq!(corpora.len(), 2);
        assert!(corpora.iter().all(|c| c.games == 3));
    }

    #[test]
    fn the_pooled_audit_clears_the_engines_own_dice() {
        // The real question §10.1b asks, asked of the real generator across a
        // real corpus rather than of a synthetic sequence.
        let mut c = Corpus::new(Config {
            trade_mode: TradeMode::Disabled,
            rules_version: carranta_record::RULES_VERSION,
        });
        for seed in 0..400 {
            c.add(&self_play(seed, TradeMode::Disabled), 0);
        }
        let a = c.dice_audit();
        assert!(a.rolls > 20_000, "only {} rolls pooled", a.rolls);
        assert!(a.p_value > 0.001, "engine dice flagged: p = {}", a.p_value);
        assert!(
            a.max_outcome_deviation < 0.6,
            "outcome share off by {:.3} points",
            a.max_outcome_deviation
        );
        assert!(
            a.lag1_autocorrelation.abs() < 0.03,
            "serial structure: {}",
            a.lag1_autocorrelation
        );
        assert!(a.runs_p > 0.001, "runs test: {}", a.runs_p);
    }

    #[test]
    fn seat_win_rates_come_out_of_a_real_corpus() {
        let mut c = Corpus::new(Config {
            trade_mode: TradeMode::Disabled,
            rules_version: carranta_record::RULES_VERSION,
        });
        for seed in 0..200 {
            c.add(&self_play(seed, TradeMode::Disabled), 0);
        }
        let rates = c.seat_win_rate();
        assert!((rates.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        // Four equal bots: no seat should be wildly off a quarter. This is the
        // first-player-advantage check, and it is a real finding either way.
        for (p, &r) in rates.iter().enumerate() {
            assert!(
                (0.15..0.40).contains(&r),
                "seat {p} won {:.1}% of games",
                r * 100.0
            );
        }
    }

    #[test]
    fn production_predicts_points_but_does_not_determine_them() {
        let mut c = Corpus::new(Config {
            trade_mode: TradeMode::Full,
            rules_version: carranta_record::RULES_VERSION,
        });
        for seed in 0..150 {
            c.add(&self_play(seed, TradeMode::Full), 0);
        }
        let fit = c.luck_adjustment().expect("a fit");
        assert!(fit.fit.slope > 0.0, "more production, fewer points?");
        assert!(
            (0.05..0.95).contains(&fit.fit.r_squared),
            "r² = {:.3}: production either explains nothing or everything",
            fit.fit.r_squared
        );
        // Residuals are centred by construction; the useful part is the spread.
        let residuals = c.conversion_residuals();
        assert_eq!(residuals.len(), 4);
        assert!(residuals.windows(2).all(|w| w[0].1 >= w[1].1));
    }

    #[test]
    fn false_discovery_control_clears_a_fair_corpus() {
        let mut c = Corpus::new(Config {
            trade_mode: TradeMode::Disabled,
            rules_version: carranta_record::RULES_VERSION,
        });
        for seed in 0..150 {
            c.add(&self_play(seed, TradeMode::Disabled), 400);
        }
        let raw = c.dice_p_values.iter().filter(|&&p| p < 0.05).count();
        let flagged = c.dice_outliers(0.05);
        assert!(
            flagged.len() <= raw,
            "correction added discoveries: {} vs {raw}",
            flagged.len()
        );
        assert!(
            flagged.is_empty(),
            "{} games survived correction on fair dice",
            flagged.len()
        );
    }

    #[test]
    fn ratings_accumulate_across_the_corpus() {
        let mut c = Corpus::new(Config {
            trade_mode: TradeMode::Disabled,
            rules_version: carranta_record::RULES_VERSION,
        });
        for seed in 0..100 {
            c.add(&self_play(seed, TradeMode::Disabled), 0);
        }
        let key = crate::rating::PoolKey {
            trade_mode: TradeMode::Disabled,
            rules_version: carranta_record::RULES_VERSION,
        };
        let pool = c.ratings.pool(key).expect("a pool");
        assert_eq!(pool.len(), 4);
        // Identical bots: ratings should stay bunched, and σ should have
        // shrunk from the prior.
        for p in 0..4 {
            assert!(pool.games_played(p) > 90);
            assert!(pool.rating(p).sigma < 25.0 / 3.0);
        }
        let mus: Vec<f64> = (0..4).map(|p| pool.rating(p).mu).collect();
        let spread = mus.iter().cloned().fold(f64::MIN, f64::max)
            - mus.iter().cloned().fold(f64::MAX, f64::min);
        assert!(spread < 8.0, "identical bots rated {spread:.1} apart");
    }
}
