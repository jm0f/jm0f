//! What is being optimised.
//!
//! Phase one of E-1: the fifteen weights the heuristic already has. They are
//! hand-set, never tuned, and an evolution strategy over them is certain to
//! produce a better bot — which is the point of doing it before topology
//! search. A NEAT genome slots in behind the same [`Arena`] interface later.
//!
//! [`Arena`]: crate::arena::Arena

use carranta_bot::Weights;
use carranta_core::rng::{Rng, Stream};

/// Weights in the genome.
pub const GENES: usize = 15;

/// Per-gene mutation scale.
///
/// Weights span three orders of magnitude — points at 1000, pieces at 2 — so a
/// single additive step is either meaningless at the top or catastrophic at the
/// bottom. Each gene mutates on its own scale, taken from the hand-set value it
/// starts at.
const SCALE: [f64; GENES] = [
    100.0, // vp
    3.0,   // pips
    6.0,   // diversity
    4.0,   // port
    3.0,   // road
    5.0,   // militia
    2.0,   // hand
    3.0,   // over_limit
    3.0,   // dev
    2.0,   // pieces
    2.0,   // build_progress
    8.0,   // buy_dev
    3.0,   // steal
    10.0,  // offer_discount
    3.0,   // offer_cost
];

/// Names, for reporting and for checkpoints.
pub const NAMES: [&str; GENES] = [
    "vp",
    "pips",
    "diversity",
    "port",
    "road",
    "militia",
    "hand",
    "over_limit",
    "dev",
    "pieces",
    "build_progress",
    "buy_dev",
    "steal",
    "offer_discount",
    "offer_cost",
];

/// A candidate agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Genome {
    pub genes: [i32; GENES],
}

impl Default for Genome {
    /// The hand-set weights: generation zero, and the thing to beat.
    fn default() -> Self {
        Genome::from_weights(Weights::default())
    }
}

impl Genome {
    pub fn from_weights(w: Weights) -> Self {
        Genome {
            genes: [
                w.vp,
                w.pips,
                w.diversity,
                w.port,
                w.road,
                w.militia,
                w.hand,
                w.over_limit,
                w.dev,
                w.pieces,
                w.build_progress,
                w.buy_dev,
                w.steal,
                w.offer_discount,
                w.offer_cost,
            ],
        }
    }

    pub fn weights(&self) -> Weights {
        let g = self.genes;
        Weights {
            vp: g[0],
            pips: g[1],
            diversity: g[2],
            port: g[3],
            road: g[4],
            militia: g[5],
            hand: g[6],
            over_limit: g[7],
            dev: g[8],
            pieces: g[9],
            build_progress: g[10],
            buy_dev: g[11],
            steal: g[12],
            offer_discount: g[13],
            offer_cost: g[14],
        }
    }

    /// Perturb every gene by a Gaussian step on its own scale.
    ///
    /// `strength` multiplies [`SCALE`]; 1.0 is a step of roughly the size that
    /// the feasibility measurement showed to be resolvable in a few hundred
    /// paired trials.
    pub fn mutate(&self, rng: &mut Rng, strength: f64) -> Genome {
        let mut out = *self;
        for (gene, scale) in out.genes.iter_mut().zip(SCALE) {
            let step = gaussian(rng) * scale * strength;
            *gene = clamp(*gene + step.round() as i32);
        }
        out
    }

    /// Uniform crossover: each gene comes from one parent or the other.
    pub fn cross(a: &Genome, b: &Genome, rng: &mut Rng) -> Genome {
        let mut out = *a;
        for (gene, &theirs) in out.genes.iter_mut().zip(&b.genes) {
            if rng.below(Stream::Board, 2) == 1 {
                *gene = theirs;
            }
        }
        out
    }

    /// Largest per-gene distance, in mutation-scale units.
    ///
    /// A population whose spread has collapsed has converged, whatever its
    /// fitness says.
    pub fn distance(&self, other: &Genome) -> f64 {
        self.genes
            .iter()
            .zip(&other.genes)
            .zip(SCALE)
            .map(|((a, b), scale)| (a - b).abs() as f64 / scale)
            .fold(0.0, f64::max)
    }

    /// One line of a checkpoint.
    pub fn encode(&self) -> String {
        self.genes
            .iter()
            .map(|g| g.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Read a line written by [`Genome::encode`].
    pub fn decode(line: &str) -> Option<Genome> {
        let mut genes = [0i32; GENES];
        let mut fields = line.split_whitespace();
        for slot in genes.iter_mut() {
            *slot = fields.next()?.parse().ok()?;
        }
        if fields.next().is_some() {
            return None; // trailing junk: not a genome line
        }
        Some(Genome { genes })
    }
}

/// Keep weights inside a range where the scorer's `i32` arithmetic cannot
/// overflow, whatever evolution proposes.
fn clamp(v: i32) -> i32 {
    v.clamp(-100_000, 100_000)
}

/// A standard normal draw, Box–Muller.
///
/// The engine's generator gives uniforms; nothing here needs a distribution
/// library for the one transform that is missing.
pub fn gaussian(rng: &mut Rng) -> f64 {
    // Never exactly zero, or the logarithm diverges.
    let u1 = (rng.below(Stream::Board, 1 << 24) as f64 + 0.5) / (1u32 << 24) as f64;
    let u2 = rng.below(Stream::Board, 1 << 24) as f64 / (1u32 << 24) as f64;
    (-2.0 * u1.ln()).sqrt() * (core::f64::consts::TAU * u2).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_survive_a_round_trip() {
        let w = Weights::default();
        assert_eq!(Genome::from_weights(w).weights(), w);
        // And every gene is actually carried: change each in turn.
        for (i, name) in NAMES.iter().enumerate() {
            let mut g = Genome::default();
            g.genes[i] += 7;
            let back = Genome::from_weights(g.weights());
            assert_eq!(back, g, "gene {i} ({name}) was dropped");
        }
    }

    #[test]
    fn a_checkpoint_line_round_trips() {
        let g = Genome::default().mutate(&mut Rng::new(4), 2.0);
        assert_eq!(Genome::decode(&g.encode()), Some(g));
        assert_eq!(Genome::decode("1 2 3"), None);
        assert_eq!(Genome::decode(&format!("{} 99", g.encode())), None);
    }

    #[test]
    fn mutation_moves_every_gene_on_its_own_scale() {
        let base = Genome::default();
        let mut rng = Rng::new(1);
        let mut moved = [0u32; GENES];
        let runs = 400;
        for _ in 0..runs {
            let m = base.mutate(&mut rng, 1.0);
            for ((count, &now), &was) in moved.iter_mut().zip(&m.genes).zip(&base.genes) {
                *count += (now != was) as u32;
            }
        }
        for (name, &n) in NAMES.iter().zip(&moved) {
            assert!(n > runs / 2, "gene {name} barely moves: {n}/{runs}");
        }
    }

    #[test]
    fn mutation_is_centred_and_scaled() {
        let base = Genome::default();
        let mut rng = Rng::new(2);
        let runs = 4_000;
        let mut sum = [0.0; GENES];
        let mut sumsq = [0.0; GENES];
        for _ in 0..runs {
            let m = base.mutate(&mut rng, 1.0);
            for (((s, sq), (&now, &was)), scale) in sum
                .iter_mut()
                .zip(sumsq.iter_mut())
                .zip(m.genes.iter().zip(&base.genes))
                .zip(SCALE)
            {
                let step = (now - was) as f64 / scale;
                *s += step;
                *sq += step * step;
            }
        }
        for (i, name) in NAMES.iter().enumerate() {
            let mean = sum[i] / runs as f64;
            let sd = (sumsq[i] / runs as f64 - mean * mean).sqrt();
            assert!(mean.abs() < 0.15, "gene {name} drifts: {mean:.3}");
            assert!(
                (0.7..1.4).contains(&sd),
                "gene {name} scale is {sd:.3}, expected near 1"
            );
        }
    }

    #[test]
    fn crossover_takes_from_both_parents() {
        let a = Genome::default();
        let b = a.mutate(&mut Rng::new(3), 5.0);
        let mut rng = Rng::new(9);
        let mut saw_a = false;
        let mut saw_b = false;
        for _ in 0..50 {
            let c = Genome::cross(&a, &b, &mut rng);
            for ((&got, &ga), &gb) in c.genes.iter().zip(&a.genes).zip(&b.genes) {
                saw_a |= got == ga && ga != gb;
                saw_b |= got == gb && ga != gb;
            }
        }
        assert!(saw_a && saw_b);
    }

    #[test]
    fn weights_stay_inside_the_safe_range() {
        let mut g = Genome::default();
        let mut rng = Rng::new(5);
        for _ in 0..2_000 {
            g = g.mutate(&mut rng, 50.0);
        }
        for (i, &v) in g.genes.iter().enumerate() {
            assert!(v.abs() <= 100_000, "gene {} escaped: {v}", NAMES[i]);
        }
    }

    #[test]
    fn distance_reads_zero_only_for_identical_genomes() {
        let a = Genome::default();
        assert_eq!(a.distance(&a), 0.0);
        let b = a.mutate(&mut Rng::new(11), 1.0);
        assert!(a.distance(&b) > 0.0);
    }
}
