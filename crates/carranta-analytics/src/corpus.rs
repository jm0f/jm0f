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

use std::collections::BTreeMap;

use carranta_core::state::{MAX_PLAYERS, TradeMode};
use carranta_record::{Log, Payload};

use crate::dice::{self, Audit};
use crate::game;
use crate::production;
use crate::rating::{LuckAdjustment, Ratings};
use crate::stats::{benjamini_hochberg, clustered_share};

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

/// Who was in a seat, as the cross-game slicing sees them (§10.3).
///
/// Two kinds on purpose, and the keys are different on purpose. An agent is
/// its name and build: every copy of `trained@378` at a table is the same
/// player, however many chairs it holds, because two copies of one program
/// have nothing to tell apart. A person is their durable id and nothing else:
/// the log is pseudonymous by construction (H-8), so this key can be grouped
/// on and never read.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Who {
    Agent { name: String, version: u32 },
    Human { player: u64 },
}

impl Who {
    fn of(seat: &carranta_record::SeatId) -> Who {
        match &seat.agent {
            Some(a) => Who::Agent {
                name: a.name.clone(),
                version: a.version,
            },
            None => Who::Human {
                player: seat.player,
            },
        }
    }
}

/// Everything accumulated for one actor across the corpus.
#[derive(Clone, Debug, Default)]
pub struct ActorStats {
    /// Games this actor appeared in at all.
    pub games: u32,
    /// Seats it held across them, which self-play makes more than `games`.
    pub seats: u32,
    pub wins: u32,
    /// Victory points summed over every seat held.
    pub vp: u64,
    pub seat_games: [u32; MAX_PLAYERS],
    /// Per game: seats won and seats held, the clusters the win rate's
    /// interval is built from (§10.6.3).
    pub shares: Vec<(u32, u32)>,
    /// Per seat held: total production and final victory points, for the
    /// §10.4 residual.
    pub conversion: Vec<(f64, f64)>,
}

/// One line of the per-actor table, ready to print.
#[derive(Clone, Debug)]
pub struct ActorRow {
    pub who: Who,
    pub games: u32,
    pub seats: u32,
    pub wins: u32,
    /// Win share over seats held, with a 95% half-width where two or more
    /// games make one computable. The interval is clustered by game, never
    /// per seat: seats inside one game are not independent draws.
    pub share: f64,
    pub half_width: Option<f64>,
    pub mean_vp: f64,
    /// Mean §10.4 conversion residual, when the corpus can fit the curve.
    pub residual: Option<f64>,
}

/// A per-turn aggregate that cannot be read without its `n` (§10.6.2).
///
/// Games end when somebody reaches ten points, so "mean VP at turn 25" is a
/// mean over the games that lasted that long, and quietly biased toward the
/// slow ones. The pitfall list says to report n per turn explicitly; this type
/// is that rule made structural, since [`PerTurn::rows`] is the only reader
/// and every row it returns carries the count it was computed over.
#[derive(Clone, Debug, Default)]
pub struct PerTurn {
    sums: Vec<f64>,
    sqs: Vec<f64>,
    ns: Vec<u32>,
}

impl PerTurn {
    fn fold(&mut self, curve: &[[u32; MAX_PLAYERS]], players: usize) {
        if players == 0 {
            return;
        }
        if self.sums.len() < curve.len() {
            self.sums.resize(curve.len(), 0.0);
            self.sqs.resize(curve.len(), 0.0);
            self.ns.resize(curve.len(), 0);
        }
        for (t, vp) in curve.iter().enumerate() {
            let total: u32 = vp[..players].iter().sum();
            let mean = f64::from(total) / players as f64;
            self.sums[t] += mean;
            self.sqs[t] += mean * mean;
            self.ns[t] += 1;
        }
    }

    /// `(turn, mean VP per seat, games that reached the turn)`, turn 1 first.
    pub fn rows(&self) -> Vec<(usize, f64, u32)> {
        self.spread_rows()
            .into_iter()
            .map(|(t, mean, _, n)| (t, mean, n))
            .collect()
    }

    /// The same rows with the spread beside the mean: `(turn, mean, standard
    /// deviation across the games that reached the turn, games)`. One game has
    /// no spread to measure, so its deviation is zero rather than a claim.
    pub fn spread_rows(&self) -> Vec<(usize, f64, f64, u32)> {
        self.sums
            .iter()
            .zip(&self.sqs)
            .zip(&self.ns)
            .enumerate()
            .filter(|(_, ((_, _), n))| **n > 0)
            .map(|(t, ((sum, sq), n))| {
                let n_f = f64::from(*n);
                let mean = sum / n_f;
                let sd = (sq / n_f - mean * mean).max(0.0).sqrt();
                (t + 1, mean, sd, *n)
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.ns.iter().all(|n| *n == 0)
    }
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
    /// Everything sliceable by who was playing, keyed so iteration is stable.
    pub actors: BTreeMap<Who, ActorStats>,
    /// Mean VP over the turns, with its n (§10.6.2).
    pub vp_turns: PerTurn,
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
            actors: BTreeMap::new(),
            vp_turns: PerTurn::default(),
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
            let rolls: u32 = counts.iter().sum();
            let kl_bits = crate::stats::kl_divergence_bits(&counts, &dice::REFERENCE);
            let expected: [f64; dice::OUTCOMES] =
                core::array::from_fn(|i| dice::REFERENCE[i] * f64::from(rolls));
            dice::GameDice {
                rolls,
                counts,
                sevens: counts[5],
                kl_bits,
                kl_fair: dice::fair_bits(kl_bits, rolls),
                misplaced: dice::misplaced_rolls(&counts, &expected),
                chi_squared: 0.0,
                p_value: 1.0,
            }
        };
        // The bias-corrected figure, because this list is ranked across games of
        // different lengths and the raw one would rank the short ones first.
        self.dice_deviations.push(d.kl_fair);
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

        // The actor slicing. Grouped within the game first, because the win
        // rate's interval needs each game as one cluster, and because in
        // self-play one actor holds several of the seats at once. A seat the
        // log does not name is left out rather than guessed at: no identity,
        // no row.
        let mut in_game: BTreeMap<Who, (u32, u32)> = BTreeMap::new();
        for p in 0..seats {
            let Some(seat) = log.created.seats.get(p) else {
                continue;
            };
            let who = Who::of(seat);
            let won = u32::from(summary.winner == Some(p as u8));
            let e = in_game.entry(who.clone()).or_insert((0, 0));
            e.0 += won;
            e.1 += 1;
            let a = self.actors.entry(who).or_default();
            a.seats += 1;
            a.wins += won;
            a.vp += u64::from(summary.vp[p]);
            a.seat_games[p] += 1;
            a.conversion
                .push((prod.total_actual(p) as f64, f64::from(summary.vp[p])));
        }
        for (who, (wins, held)) in in_game {
            let a = self.actors.entry(who).or_default();
            a.games += 1;
            a.shares.push((wins, held));
        }
        self.vp_turns.fold(&summary.vp_curve, seats);
        true
    }

    /// Win rate by seat, the first-player-advantage question (A-4).
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
    /// their games, the "were you good or lucky" number.
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

    /// The per-actor table, most-played first.
    ///
    /// Win shares come with a clustered interval or none at all, never a
    /// naive one, and the residual column is filled only when the corpus can
    /// fit the §10.4 curve in the first place.
    pub fn actor_rows(&self) -> Vec<ActorRow> {
        let fit = self.luck_adjustment();
        let mut rows: Vec<ActorRow> = self
            .actors
            .iter()
            .map(|(who, a)| {
                let (share, half_width) = clustered_share(&a.shares).unwrap_or((0.0, None));
                let residual = fit.as_ref().map(|f| {
                    let sum: f64 = a
                        .conversion
                        .iter()
                        .map(|&(prod, vp)| f.residual(prod, vp))
                        .sum();
                    sum / a.conversion.len().max(1) as f64
                });
                ActorRow {
                    who: who.clone(),
                    games: a.games,
                    seats: a.seats,
                    wins: a.wins,
                    share,
                    half_width,
                    mean_vp: a.vp as f64 / f64::from(a.seats.max(1)),
                    residual,
                }
            })
            .collect();
        rows.sort_by(|a, b| b.games.cmp(&a.games).then_with(|| a.who.cmp(&b.who)));
        rows
    }
}

/// Whether anybody at this table was a person.
///
/// The segmentation §10.6.5 demands: self-play corpora are orders of
/// magnitude larger than human ones, so they are never pooled, and this is
/// the bit the split is made on.
pub fn has_human(log: &Log) -> bool {
    log.created.seats.iter().any(|s| s.agent.is_none())
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
    use crate::testing::{self_play, self_play_rotated};

    #[test]
    fn every_copy_of_an_agent_is_one_actor() {
        // Four seats, all played by heuristic@1: one row, four seats a game,
        // and the row's games count games rather than chairs.
        let mut c = Corpus::new(Config::of(&self_play(1, TradeMode::Full)));
        for seed in 1..=3 {
            assert!(c.add(&self_play(seed, TradeMode::Full), 0));
        }
        let rows = c.actor_rows();
        assert_eq!(rows.len(), 1, "four copies pooled into one actor");
        let row = &rows[0];
        assert_eq!(
            row.who,
            Who::Agent {
                name: "heuristic".to_string(),
                version: 1
            }
        );
        assert_eq!(row.games, 3);
        assert_eq!(row.seats, 12);
        // Every finished game has one winner, and this actor holds every
        // seat, so its share is exactly the finish rate and its clustered
        // interval is exact zero width when every game finished.
        if c.finished == c.games {
            assert!((row.share - 0.25).abs() < 1e-12);
            assert_eq!(row.half_width, Some(0.0));
        }
    }

    #[test]
    fn two_people_are_two_actors_and_never_pooled() {
        // Hand-build a log where seat identities are two humans and two
        // copies of one agent: three rows, keyed apart.
        let mut log = self_play(7, TradeMode::Full);
        log.created.seats[0] = carranta_record::SeatId::human(11);
        log.created.seats[1] = carranta_record::SeatId::human(22);
        let mut c = Corpus::new(Config::of(&log));
        assert!(c.add(&log, 0));
        let rows = c.actor_rows();
        assert_eq!(rows.len(), 3);
        let humans = rows
            .iter()
            .filter(|r| matches!(r.who, Who::Human { .. }))
            .count();
        assert_eq!(humans, 2);
        let agent = rows
            .iter()
            .find(|r| matches!(r.who, Who::Agent { .. }))
            .expect("the agent row");
        assert_eq!(agent.seats, 2, "two copies, one row");
        assert!(has_human(&log));
        assert!(!has_human(&self_play(7, TradeMode::Full)));
    }

    #[test]
    fn the_win_interval_is_clustered_by_game_not_by_seat() {
        // An actor that holds all four seats wins every game it finishes:
        // per seat that is 25% with binomial scatter, per game it is a
        // certainty. The clustered interval knows the difference.
        let mut c = Corpus::new(Config::of(&self_play(1, TradeMode::Full)));
        let mut finished = 0;
        for seed in 10..20 {
            let log = self_play(seed, TradeMode::Full);
            if game::analyse(&log).expect("replayable").winner.is_some() {
                finished += 1;
            }
            c.add(&log, 0);
        }
        if finished == c.games && finished >= 2 {
            let rows = c.actor_rows();
            assert_eq!(
                rows[0].half_width,
                Some(0.0),
                "every cluster agrees, so the width is exactly zero"
            );
        }
    }

    #[test]
    fn per_turn_means_carry_their_n_and_the_n_shrinks() {
        let mut c = Corpus::new(Config::of(&self_play(1, TradeMode::Full)));
        for seed in 1..=4 {
            c.add(&self_play(seed, TradeMode::Full), 0);
        }
        let rows = c.vp_turns.rows();
        assert!(!rows.is_empty());
        // Turn one was reached by every game; the last row by at least one;
        // and n never grows with the turn, which is the truncation the type
        // exists to keep visible.
        assert_eq!(rows[0].2, c.games);
        for pair in rows.windows(2) {
            assert!(pair[0].2 >= pair[1].2, "n grew as games died off");
            assert!(pair[0].0 + 1 == pair[1].0);
        }
        // And the means are believable: nobody has VP above ten.
        for (_, mean, n) in &rows {
            assert!(*mean >= 0.0 && *mean <= 10.0);
            assert!(*n >= 1);
        }
    }

    #[test]
    fn rotation_still_lands_in_one_row_per_agent() {
        // The A-4 rotation moves agents around the table; the actor table
        // must see the agent, not the seat it happened to hold.
        let mut c = Corpus::new(Config::of(&self_play(1, TradeMode::Full)));
        for r in 0..4 {
            c.add(&self_play_rotated(5, TradeMode::Full, r), 0);
        }
        let rows = c.actor_rows();
        assert_eq!(rows.len(), 1);
        // The seat histogram spreads across chairs rather than piling on one.
        let key = &rows[0].who;
        let spread = c.actors[key].seat_games.iter().filter(|n| **n > 0).count();
        assert_eq!(spread, 4);
    }

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
