//! The statistics the rest of the crate is built on.
//!
//! Written out rather than pulled in: the whole workspace is dependency-free,
//! and what is needed here is four special functions and an ordinary least
//! squares fit. Each carries a test against values computable by hand or from
//! a table.

/// The error function complement, `1 − erf(x)`.
///
/// Numerical Recipes' Chebyshev fit: relative error below 1.2e-7, which is far
/// finer than any p-value reported here is meaningful to.
pub fn erfc(x: f64) -> f64 {
    let z = x.abs();
    let t = 2.0 / (2.0 + z);
    let ty = 4.0 * t - 2.0;

    const COF: [f64; 28] = [
        -1.3026537197817094,
        6.419_697_923_564_902e-1,
        1.9476473204185836e-2,
        -9.561_514_786_808_63e-3,
        -9.46595344482036e-4,
        3.66839497852761e-4,
        4.2523324806907e-5,
        -2.0278578112534e-5,
        -1.624290004647e-6,
        1.303655835580e-6,
        1.5626441722e-8,
        -8.5238095915e-8,
        6.529054439e-9,
        5.059343495e-9,
        -9.91364156e-10,
        -2.27365122e-10,
        9.6467911e-11,
        2.394038e-12,
        -6.886027e-12,
        8.94487e-13,
        3.13092e-13,
        -1.12708e-13,
        3.81e-16,
        7.106e-15,
        -1.523e-15,
        -9.4e-17,
        1.21e-16,
        -2.8e-17,
    ];

    // Clenshaw recurrence over the Chebyshev coefficients.
    let (mut d, mut dd) = (0.0, 0.0);
    for &c in COF.iter().skip(1).rev() {
        let tmp = d;
        d = ty * d - dd + c;
        dd = tmp;
    }
    let ans = t * (-z * z + 0.5 * (COF[0] + ty * d) - dd).exp();
    if x >= 0.0 { ans } else { 2.0 - ans }
}

/// The standard normal cumulative distribution.
pub fn normal_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / core::f64::consts::SQRT_2)
}

/// Two-sided normal tail probability for a z-score.
pub fn normal_two_sided(z: f64) -> f64 {
    erfc(z.abs() / core::f64::consts::SQRT_2)
}

/// The natural log of the gamma function (Lanczos, g=5, n=7).
fn ln_gamma(x: f64) -> f64 {
    const COF: [f64; 6] = [
        76.180_091_729_471_46,
        -86.505_320_329_416_77,
        24.014_098_240_830_91,
        -1.231_739_572_450_155,
        0.120_865_097_386_617_7e-2,
        -0.539_523_938_495_3e-5,
    ];
    let mut y = x;
    let tmp = x + 5.5 - (x + 0.5) * (x + 5.5).ln();
    let mut ser = 1.000_000_000_190_015;
    for &c in &COF {
        y += 1.0;
        ser += c / y;
    }
    -tmp + (2.506_628_274_631_000_5 * ser / x).ln()
}

/// The regularized lower incomplete gamma `P(a, x)`.
fn gamma_p(a: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x < a + 1.0 {
        // Series representation.
        let mut ap = a;
        let mut sum = 1.0 / a;
        let mut del = sum;
        for _ in 0..300 {
            ap += 1.0;
            del *= x / ap;
            sum += del;
            if del.abs() < sum.abs() * 1e-15 {
                break;
            }
        }
        sum * (-x + a * x.ln() - ln_gamma(a)).exp()
    } else {
        // Continued fraction for Q, then complement (Lentz).
        let tiny = 1e-300;
        let mut b = x + 1.0 - a;
        let mut c = 1.0 / tiny;
        let mut d = 1.0 / b;
        let mut h = d;
        for i in 1..300 {
            let an = -(i as f64) * (i as f64 - a);
            b += 2.0;
            d = an * d + b;
            if d.abs() < tiny {
                d = tiny;
            }
            c = b + an / c;
            if c.abs() < tiny {
                c = tiny;
            }
            d = 1.0 / d;
            let del = d * c;
            h *= del;
            if (del - 1.0).abs() < 1e-15 {
                break;
            }
        }
        1.0 - (-x + a * x.ln() - ln_gamma(a)).exp() * h
    }
}

/// Upper tail of the chi-squared distribution: `P(X > stat)` with `df`
/// degrees of freedom.
pub fn chi_squared_p(stat: f64, df: u32) -> f64 {
    if df == 0 || !stat.is_finite() || stat <= 0.0 {
        return 1.0;
    }
    (1.0 - gamma_p(df as f64 / 2.0, stat / 2.0)).clamp(0.0, 1.0)
}

/// Pearson's chi-squared statistic for observed counts against expected ones.
///
/// Valid as a *statistic* at any sample size. What needs a large sample is
/// reading its p-value off the chi-squared distribution — see [`crate::dice`],
/// which simulates the null instead when the sample is small.
pub fn chi_squared_stat(observed: &[u32], expected: &[f64]) -> f64 {
    observed
        .iter()
        .zip(expected)
        .filter(|&(_, &e)| e > 0.0)
        .map(|(&o, &e)| {
            let d = o as f64 - e;
            d * d / e
        })
        .sum()
}

/// Kullback–Leibler divergence of an empirical distribution from a reference,
/// in **bits**.
///
/// The effect size §10.1 asks for: interpretable, and comparable across games
/// of different length in a way a p-value is not. Zero exactly when the
/// empirical distribution matches.
pub fn kl_divergence_bits(observed: &[u32], reference: &[f64]) -> f64 {
    let n: u32 = observed.iter().sum();
    if n == 0 {
        return 0.0;
    }
    let n = n as f64;
    observed
        .iter()
        .zip(reference)
        .filter(|&(&o, &r)| o > 0 && r > 0.0)
        .map(|(&o, &r)| {
            let p = o as f64 / n;
            p * (p / r).log2()
        })
        .sum()
}

/// Mean and sample standard deviation.
pub fn mean_sd(xs: &[f64]) -> (f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0);
    }
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    if xs.len() < 2 {
        return (mean, 0.0);
    }
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, var.sqrt())
}

/// The share of `sample` at or below `value`, in [0, 1].
///
/// §10.1 wants a game's dice reported as "deviated more than 87% of recorded
/// games" rather than as a p-value, which is what this computes.
pub fn percentile_of(value: f64, sample: &[f64]) -> f64 {
    if sample.is_empty() {
        return 0.0;
    }
    let below = sample.iter().filter(|&&x| x <= value).count();
    below as f64 / sample.len() as f64
}

/// An ordinary least squares fit of `y` on `x`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub intercept: f64,
    pub slope: f64,
    /// Fraction of variance in `y` explained.
    pub r_squared: f64,
    pub n: usize,
}

impl Fit {
    /// The value the fit predicts at `x`.
    pub fn predict(&self, x: f64) -> f64 {
        self.intercept + self.slope * x
    }
}

/// Least squares regression of `y` on `x`, for §10.4's luck adjustment.
pub fn least_squares(x: &[f64], y: &[f64]) -> Option<Fit> {
    let n = x.len().min(y.len());
    if n < 2 {
        return None;
    }
    let (mx, _) = mean_sd(&x[..n]);
    let (my, _) = mean_sd(&y[..n]);
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    let mut syy = 0.0;
    for i in 0..n {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    if sxx == 0.0 {
        return None;
    }
    let slope = sxy / sxx;
    Some(Fit {
        intercept: my - slope * mx,
        slope,
        r_squared: if syy == 0.0 {
            0.0
        } else {
            sxy * sxy / (sxx * syy)
        },
        n,
    })
}

/// Benjamini–Hochberg false discovery rate control (§10.1).
///
/// Returns, for each input p-value, whether it is a discovery at level `q`.
/// Order is preserved. Applied when per-game dice tests are used across a
/// corpus — where, by construction, 5% of games clear p<0.05 with nothing
/// wrong with them.
pub fn benjamini_hochberg(p_values: &[f64], q: f64) -> Vec<bool> {
    let m = p_values.len();
    if m == 0 {
        return Vec::new();
    }
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&a, &b| p_values[a].total_cmp(&p_values[b]));

    // The largest k with p_(k) <= k/m * q; everything up to it is a discovery.
    let mut cutoff = 0;
    for (rank, &i) in order.iter().enumerate() {
        if p_values[i] <= (rank + 1) as f64 / m as f64 * q {
            cutoff = rank + 1;
        }
    }
    let mut out = vec![false; m];
    for &i in order.iter().take(cutoff) {
        out[i] = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "{a} vs {b}");
    }

    #[test]
    fn normal_cdf_matches_the_table() {
        close(normal_cdf(0.0), 0.5, 1e-12);
        close(normal_cdf(1.0), 0.841_344_746, 1e-8);
        close(normal_cdf(-1.96), 0.024_997_895, 1e-8);
        close(normal_cdf(2.575_829), 0.995, 1e-6);
        // Symmetry, over a wide range.
        for i in -60..=60 {
            let z = i as f64 / 10.0;
            close(normal_cdf(z) + normal_cdf(-z), 1.0, 1e-12);
        }
    }

    #[test]
    fn chi_squared_matches_the_table() {
        // Critical values at p = 0.05, from standard tables.
        close(chi_squared_p(3.841, 1), 0.05, 1e-4);
        close(chi_squared_p(5.991, 2), 0.05, 1e-4);
        close(chi_squared_p(18.307, 10), 0.05, 1e-4);
        close(chi_squared_p(31.410, 20), 0.05, 1e-4);
        // And at p = 0.01 with 10 df.
        close(chi_squared_p(23.209, 10), 0.01, 1e-4);
        // A statistic of zero is never evidence.
        close(chi_squared_p(0.0, 10), 1.0, 1e-12);
    }

    #[test]
    fn chi_squared_is_monotone_in_the_statistic() {
        let mut last = 1.0;
        for i in 1..200 {
            let p = chi_squared_p(i as f64 / 4.0, 10);
            assert!(p <= last, "p rose at {i}");
            last = p;
        }
    }

    #[test]
    fn kl_is_zero_only_on_a_perfect_match() {
        let reference = [0.25, 0.25, 0.5];
        close(kl_divergence_bits(&[25, 25, 50], &reference), 0.0, 1e-12);
        assert!(kl_divergence_bits(&[50, 25, 25], &reference) > 0.0);
        // One bit: everything lands on an outcome the reference gives half to.
        close(kl_divergence_bits(&[0, 0, 10], &reference), 1.0, 1e-12);
    }

    #[test]
    fn least_squares_recovers_a_line() {
        let x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|x| 3.0 * x - 7.0).collect();
        let fit = least_squares(&x, &y).unwrap();
        close(fit.slope, 3.0, 1e-9);
        close(fit.intercept, -7.0, 1e-9);
        close(fit.r_squared, 1.0, 1e-12);
        close(fit.predict(100.0), 293.0, 1e-6);
        // A flat y has no line to find, but is not an error.
        let flat = vec![5.0; 50];
        assert_eq!(least_squares(&x, &flat).unwrap().slope, 0.0);
        // A flat x has no fit at all.
        assert!(least_squares(&flat, &y).is_none());
    }

    #[test]
    fn benjamini_hochberg_is_stricter_than_a_flat_threshold() {
        // 100 p-values uniform on [0,1): about 5 clear 0.05 by chance, and BH
        // should reject essentially all of them.
        let ps: Vec<f64> = (0..100).map(|i| (i as f64 + 0.5) / 100.0).collect();
        let flagged = benjamini_hochberg(&ps, 0.05);
        assert_eq!(flagged.iter().filter(|&&f| f).count(), 0);
        assert_eq!(ps.iter().filter(|&&p| p < 0.05).count(), 5);

        // One genuinely tiny p-value survives.
        let mut with_signal = ps.clone();
        with_signal[0] = 1e-9;
        let flagged = benjamini_hochberg(&with_signal, 0.05);
        assert!(flagged[0]);
        assert_eq!(flagged.iter().filter(|&&f| f).count(), 1);
    }

    #[test]
    fn benjamini_hochberg_keeps_input_order() {
        let ps = [0.5, 0.001, 0.9, 0.002];
        let flagged = benjamini_hochberg(&ps, 0.05);
        assert_eq!(flagged, vec![false, true, false, true]);
    }

    #[test]
    fn percentile_reads_the_way_it_is_reported() {
        let sample: Vec<f64> = (0..100).map(|i| i as f64).collect();
        close(percentile_of(86.5, &sample), 0.87, 1e-12);
        close(percentile_of(-1.0, &sample), 0.0, 1e-12);
        close(percentile_of(1000.0, &sample), 1.0, 1e-12);
    }
}
