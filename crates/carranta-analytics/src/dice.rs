//! Dice fairness (§10.1).
//!
//! Two different questions live here and must not be confused:
//!
//! **(a) Was *this game's* dice sequence unusual?** ~60–100 rolls. A plain
//! chi-squared p-value is invalid at that size. The expected count for 2 and
//! for 12 is under 2, so the null is simulated instead. And the result is
//! presented as a percentile against recorded games, never as a significance
//! claim: across thousands of games ~5% clear p<0.05 by construction, and those
//! are precisely the games a player screenshots as proof of rigging.
//!
//! **(b) Is the *generator* fair?** Millions of pooled rolls, where chi-squared
//! is valid but the opposite problem appears: any trivial deviation becomes
//! "significant". Judge on effect size, and check independence too. A bad
//! generator can produce correct marginals with serial structure.
//!
//! Stated once, since it drives both: **small n makes p-values invalid, large n
//! makes them uninformative.** Both regimes need effect sizes.

use carranta_core::action::Resolved;
use carranta_core::rng::{Rng, Stream};
use carranta_record::Log;

use crate::stats::{
    chi_squared_p, chi_squared_stat, kl_divergence_bits, normal_two_sided, percentile_of,
};

/// Outcomes of two dice: 2 through 12.
pub const OUTCOMES: usize = 11;

/// The exact distribution over 2..=12: `(1,2,3,4,5,6,5,4,3,2,1)/36`.
pub const REFERENCE: [f64; OUTCOMES] = [
    1.0 / 36.0,
    2.0 / 36.0,
    3.0 / 36.0,
    4.0 / 36.0,
    5.0 / 36.0,
    6.0 / 36.0,
    5.0 / 36.0,
    4.0 / 36.0,
    3.0 / 36.0,
    2.0 / 36.0,
    1.0 / 36.0,
];

/// Ways to make each total, out of 36.
pub const PIPS: [u32; OUTCOMES] = [1, 2, 3, 4, 5, 6, 5, 4, 3, 2, 1];

/// Every roll in a recorded game, in order.
pub fn rolls(log: &Log) -> Vec<u8> {
    log.decisions()
        .filter_map(|(_, _, r)| match r {
            Resolved::Dice(a, b) => Some(a + b),
            _ => None,
        })
        .collect()
}

/// Tally a roll sequence into counts indexed by `total - 2`.
pub fn histogram(rolls: &[u8]) -> [u32; OUTCOMES] {
    let mut counts = [0u32; OUTCOMES];
    for &r in rolls {
        if (2..=12).contains(&r) {
            counts[r as usize - 2] += 1;
        }
    }
    counts
}

/// How unusual one game's dice were.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameDice {
    pub rolls: u32,
    pub counts: [u32; OUTCOMES],
    /// Sevens, tracked separately: 16.7% of rolls, and the whole robber
    /// economy runs on them.
    pub sevens: u32,
    /// Effect size, in bits. Comparable across games of different length,
    /// which is what makes it the right thing to rank a corpus by.
    pub kl_bits: f64,
    /// Pearson's statistic. Reported for provenance; its *asymptotic* p-value
    /// is not valid at this sample size, which is why the next field exists.
    pub chi_squared: f64,
    /// Monte Carlo p-value: the share of simulated games under a fair die that
    /// deviated at least this much.
    ///
    /// Exact by construction rather than asymptotic. **Do not present this as
    /// a fairness verdict for one game**, use [`Corpus::deviation_percentile`]
    /// (§10.1). It is here for corpus-level analysis, where it belongs behind
    /// [`crate::stats::benjamini_hochberg`].
    pub p_value: f64,
}

/// Simulations behind a per-game p-value. 10 000 resolves to 1e-4, finer than
/// the quantity deserves to be read at.
pub const DEFAULT_SIMS: u32 = 10_000;

/// Analyse one game's rolls, simulating the null to get an exact p-value.
pub fn analyse_game(rolls: &[u8], sims: u32, seed: u64) -> GameDice {
    let counts = histogram(rolls);
    let n: u32 = counts.iter().sum();
    let expected: [f64; OUTCOMES] = core::array::from_fn(|i| REFERENCE[i] * n as f64);
    let observed_stat = chi_squared_stat(&counts, &expected);

    // The null: draw `n` fair rolls and see how often they deviate at least as
    // much. Adding one to both parts is the standard correction. A p-value of
    // exactly zero is never warranted by a finite simulation.
    let mut rng = Rng::new(seed);
    let mut at_least = 0u32;
    for _ in 0..sims {
        let mut sim = [0u32; OUTCOMES];
        for _ in 0..n {
            let total = (rng.die() + rng.die()) as usize - 2;
            sim[total] += 1;
        }
        if chi_squared_stat(&sim, &expected) >= observed_stat {
            at_least += 1;
        }
    }

    GameDice {
        rolls: n,
        counts,
        sevens: counts[5],
        kl_bits: kl_divergence_bits(&counts, &REFERENCE),
        chi_squared: observed_stat,
        p_value: (at_least + 1) as f64 / (sims + 1) as f64,
    }
}

/// Recorded games to compare one game against.
///
/// The presentation §10.1 asks for: "the dice in this game deviated more than
/// 87% of recorded games". Same information as a p-value, no significance
/// claim, and no multiple-comparisons trap.
#[derive(Clone, Debug, Default)]
pub struct Corpus {
    deviations: Vec<f64>,
}

impl Corpus {
    /// Build from the per-game effect sizes of a body of games.
    pub fn from_games(games: impl IntoIterator<Item = f64>) -> Self {
        let mut deviations: Vec<f64> = games.into_iter().collect();
        deviations.sort_by(f64::total_cmp);
        Corpus { deviations }
    }

    pub fn games(&self) -> usize {
        self.deviations.len()
    }

    /// The share of recorded games whose dice deviated *less* than this one.
    pub fn deviation_percentile(&self, kl_bits: f64) -> f64 {
        percentile_of(kl_bits, &self.deviations)
    }
}

/// The generator audit (§10.1b): pooled rolls, effect sizes, independence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Audit {
    pub rolls: u64,
    pub counts: [u64; OUTCOMES],
    pub chi_squared: f64,
    /// Valid at this sample size, and, at this sample size, near-guaranteed
    /// to be small for any real deviation. Judge on the effect sizes below.
    pub p_value: f64,
    pub kl_bits: f64,
    /// Largest gap between an observed and expected outcome share, in
    /// percentage points.
    pub max_outcome_deviation: f64,
    /// Observed share of sevens against the expected 1/6.
    pub seven_share: f64,
    /// Correlation between consecutive rolls. Marginals can be perfect while
    /// this is not.
    pub lag1_autocorrelation: f64,
    /// Wald–Wolfowitz runs test over rolls above and below 7, two-sided.
    pub runs_p: f64,
}

/// Audit a pooled roll sequence. Order matters: the independence checks read
/// it as a sequence, not a bag.
pub fn audit(rolls: &[u8]) -> Audit {
    let mut counts = [0u64; OUTCOMES];
    for &r in rolls {
        if (2..=12).contains(&r) {
            counts[r as usize - 2] += 1;
        }
    }
    let n: u64 = counts.iter().sum();
    let nf = n as f64;

    let counts32: [u32; OUTCOMES] = core::array::from_fn(|i| counts[i] as u32);
    let expected: [f64; OUTCOMES] = core::array::from_fn(|i| REFERENCE[i] * nf);
    let chi_squared = if n == 0 {
        0.0
    } else {
        counts
            .iter()
            .zip(expected)
            .map(|(&o, e)| {
                let d = o as f64 - e;
                d * d / e
            })
            .sum()
    };

    let max_outcome_deviation = if n == 0 {
        0.0
    } else {
        (0..OUTCOMES)
            .map(|i| (counts[i] as f64 / nf - REFERENCE[i]).abs() * 100.0)
            .fold(0.0, f64::max)
    };

    Audit {
        rolls: n,
        counts,
        chi_squared,
        // 11 outcomes, no fitted parameters: 10 degrees of freedom.
        p_value: chi_squared_p(chi_squared, OUTCOMES as u32 - 1),
        kl_bits: kl_divergence_bits(&counts32, &REFERENCE),
        max_outcome_deviation,
        seven_share: if n == 0 { 0.0 } else { counts[5] as f64 / nf },
        lag1_autocorrelation: lag1(rolls),
        runs_p: runs_test(rolls),
    }
}

/// Pearson correlation between consecutive rolls.
fn lag1(rolls: &[u8]) -> f64 {
    if rolls.len() < 3 {
        return 0.0;
    }
    let n = (rolls.len() - 1) as f64;
    let a = &rolls[..rolls.len() - 1];
    let b = &rolls[1..];
    let ma = a.iter().map(|&x| x as f64).sum::<f64>() / n;
    let mb = b.iter().map(|&x| x as f64).sum::<f64>() / n;
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        let dx = x as f64 - ma;
        let dy = y as f64 - mb;
        sab += dx * dy;
        saa += dx * dx;
        sbb += dy * dy;
    }
    if saa == 0.0 || sbb == 0.0 {
        return 0.0;
    }
    sab / (saa * sbb).sqrt()
}

/// Wald–Wolfowitz runs test on rolls above versus below 7.
///
/// Sevens are dropped rather than assigned a side: they are the median, and
/// forcing them either way manufactures structure that is not there.
fn runs_test(rolls: &[u8]) -> f64 {
    let sides: Vec<bool> = rolls
        .iter()
        .filter(|&&r| r != 7 && (2..=12).contains(&r))
        .map(|&r| r > 7)
        .collect();
    let n1 = sides.iter().filter(|&&s| s).count() as f64;
    let n2 = sides.len() as f64 - n1;
    if n1 < 2.0 || n2 < 2.0 {
        return 1.0;
    }
    let runs = 1 + sides.windows(2).filter(|w| w[0] != w[1]).count();

    let n = n1 + n2;
    let expected = 2.0 * n1 * n2 / n + 1.0;
    let variance = 2.0 * n1 * n2 * (2.0 * n1 * n2 - n) / (n * n * (n - 1.0));
    if variance <= 0.0 {
        return 1.0;
    }
    normal_two_sided((runs as f64 - expected) / variance.sqrt())
}

/// Roll a fair pair. Exposed so a caller can generate a reference sequence to
/// test an audit against.
pub fn fair_rolls(n: usize, seed: u64) -> Vec<u8> {
    let mut rng = Rng::new(seed);
    (0..n).map(|_| rng.die() + rng.die()).collect()
}

/// A deliberately biased sequence, for checking that the audit notices.
#[doc(hidden)]
pub fn biased_rolls(n: usize, seed: u64, extra_sevens: f64) -> Vec<u8> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            if (rng.below(Stream::Dice, 10_000) as f64) < extra_sevens * 10_000.0 {
                7
            } else {
                rng.die() + rng.die()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reference_distribution_is_a_distribution() {
        let total: f64 = REFERENCE.iter().sum();
        assert!((total - 1.0).abs() < 1e-15);
        assert_eq!(PIPS.iter().sum::<u32>(), 36);
        // The seven is a sixth of all rolls, which is why it gets its own line
        // in every report.
        assert!((REFERENCE[5] - 1.0 / 6.0).abs() < 1e-15);
    }

    #[test]
    fn fair_dice_produce_uniform_p_values() {
        // The property that makes a p-value a p-value: under the null it is
        // uniform. If ~5% of fair games did not clear 0.05, the test would be
        // miscalibrated, and §10.1's whole warning rests on this being true.
        let mut below = 0;
        let games = 400;
        for g in 0..games {
            let r = fair_rolls(70, 900 + g);
            if analyse_game(&r, 500, g).p_value < 0.05 {
                below += 1;
            }
        }
        let rate = below as f64 / games as f64;
        assert!(
            (0.01..0.11).contains(&rate),
            "{rate:.3} of fair games flagged at p<0.05, expected about 0.05"
        );
    }

    #[test]
    fn a_loaded_die_is_caught_at_game_length() {
        // Every roll a seven: as extreme as 70 rolls can be.
        let rigged = vec![7u8; 70];
        let d = analyse_game(&rigged, 2_000, 1);
        assert!(d.p_value < 0.001, "p = {}", d.p_value);
        // KL against the reference: log2(1 / (6/36)) = log2(6).
        assert!((d.kl_bits - 6.0f64.log2()).abs() < 1e-9);
    }

    #[test]
    fn the_audit_clears_a_fair_generator() {
        let r = fair_rolls(500_000, 4242);
        let a = audit(&r);
        assert_eq!(a.rolls, 500_000);
        assert!(
            a.p_value > 0.001,
            "fair generator flagged, p = {}",
            a.p_value
        );
        assert!(
            a.max_outcome_deviation < 0.5,
            "outcome share off by {:.3} points",
            a.max_outcome_deviation
        );
        assert!((a.seven_share - 1.0 / 6.0).abs() < 0.005);
        assert!(a.lag1_autocorrelation.abs() < 0.01);
        assert!(a.runs_p > 0.001);
        assert!(a.kl_bits < 1e-3, "kl = {}", a.kl_bits);
    }

    #[test]
    fn the_audit_catches_a_small_bias_the_effect_size_calls_small() {
        // 2% extra sevens: overwhelming at this n, but a small effect. That is
        // exactly the large-n regime §10.1 warns about. The p-value alone
        // would say "rigged" for a deviation worth about a quarter of a
        // percentage point per outcome.
        let r = biased_rolls(500_000, 77, 0.02);
        let a = audit(&r);
        assert!(a.p_value < 1e-6, "bias missed, p = {}", a.p_value);
        assert!(a.seven_share > 1.0 / 6.0 + 0.01);
        assert!(
            a.max_outcome_deviation > 1.0,
            "effect size {:.3} did not register",
            a.max_outcome_deviation
        );
    }

    #[test]
    fn the_audit_catches_serial_structure_behind_correct_marginals() {
        // A sorted sequence has a perfect marginal distribution and is
        // obviously not independent. Marginal tests alone would pass it.
        let mut r = fair_rolls(60_000, 5);
        r.sort_unstable();
        let a = audit(&r);
        assert!(a.max_outcome_deviation < 0.5, "marginals should look fine");
        assert!(
            a.lag1_autocorrelation > 0.9,
            "serial structure missed: {}",
            a.lag1_autocorrelation
        );
        assert!(a.runs_p < 1e-6, "runs test missed it: {}", a.runs_p);
    }

    #[test]
    fn a_corpus_reports_a_percentile_not_a_verdict() {
        let games: Vec<f64> = (0..200)
            .map(|g| analyse_game(&fair_rolls(70, 5_000 + g), 200, g).kl_bits)
            .collect();
        let corpus = Corpus::from_games(games);
        assert_eq!(corpus.games(), 200);

        // A wildly deviant game sits at the top of the corpus.
        let rigged = analyse_game(&[7u8; 70], 200, 1);
        assert!(corpus.deviation_percentile(rigged.kl_bits) > 0.99);
        // A perfectly typical one does not.
        assert!(corpus.deviation_percentile(0.0) < 0.02);
    }

    #[test]
    fn an_empty_game_does_not_panic() {
        let d = analyse_game(&[], 10, 1);
        assert_eq!(d.rolls, 0);
        assert_eq!(d.kl_bits, 0.0);
        let a = audit(&[]);
        assert_eq!(a.rolls, 0);
        assert_eq!(a.p_value, 1.0);
    }
}
