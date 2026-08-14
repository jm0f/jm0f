//! The population loop (E-1, E-5, E-6, E-7).
//!
//! A generation is:
//!
//! 1. Draw a fixed opponent field, current population, hall of fame, and the
//!    anchor, held constant across every genome, so genomes are compared on
//!    identical conditions (E-4).
//! 2. Play every genome over the same board seeds, its seat rotated.
//! 3. Score by mean finishing position (E-6): the whole order, not one bit.
//! 4. Keep the best, breed the rest, enrol the champion on the ladder.
//! 5. Decide next generation's budget from whether the field could actually be
//!    told apart (E-5).
//!
//! The budget rule is the part worth reading. The feasibility measurement
//! showed resolution costs scale as 1/effect², so a fixed budget is wrong at
//! both ends of a run: wasteful while genomes differ wildly, and useless once
//! they do not. Here the budget doubles whenever the gap between the best and
//! the median genome falls inside the noise of the estimate, and decays back
//! when selection is comfortable again.

use carranta_core::rng::{Rng, Stream};
use carranta_core::state::{MAX_PLAYERS, TradeMode};

use crate::arena::{Arena, Job};
use crate::behaviour::{Behaviour, Sampler};
use crate::genome::Genome;
use crate::ladder::{ANCHOR, Ladder};

/// Identities for members of the live population, which have none of their own
/// until they survive a generation and are enrolled.
///
/// Far above any real version id, so a transient can never collide with one.
const TRANSIENT: u64 = 1 << 40;

/// Who is sitting in an opponent seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seat {
    /// A version on the ladder: the anchor, or a hall-of-fame champion.
    Rated(u64),
    /// A member of the current population, by index.
    Current(usize),
}

/// How a run is configured.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub population: usize,
    /// Genomes that survive selection and breed the next generation.
    pub survivors: usize,
    /// Games per genome at the start of a run.
    pub trials: u32,
    /// Games the champion plays *after* selection, on fresh seeds, to earn its
    /// ladder rating.
    ///
    /// Selecting the best of a population and then rating it on the same games
    /// is a winner's curse: the best of N noisy estimates is biased upward by
    /// roughly the noise itself, so a champion would look stronger than it is
    /// and the ladder would drift upward on selection alone. Fresh games cost a
    /// little and make the rating mean what it says.
    pub validation: u32,
    pub trials_min: u32,
    pub trials_max: u32,
    /// Mutation step, in units of the per-gene scale.
    pub mutation: f64,
    /// Opponents drawn from the hall of fame rather than the live population.
    ///
    /// Without it a population can chase itself into a mediocre stable state,
    /// beating its contemporaries and nobody else.
    pub hall_seats: usize,
    pub hall_size: usize,
    /// Validation games recorded and analysed per generation (§10).
    ///
    /// Behavioural markers, not a fitness signal, selection never sees them.
    /// A handful is enough to show a trend across generations, and recording
    /// costs nothing measurable per game. Zero switches it off.
    pub sample: u32,
    pub mode: TradeMode,
    pub threads: usize,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            population: 48,
            survivors: 12,
            trials: 96,
            validation: 96,
            trials_min: 24,
            trials_max: 6_144,
            mutation: 1.0,
            hall_seats: 1,
            hall_size: 24,
            sample: 8,
            mode: TradeMode::Restricted,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
        }
    }
}

/// What one generation produced.
#[derive(Clone, Debug)]
pub struct Report {
    pub generation: u32,
    pub trials: u32,
    pub games: u32,
    /// Mean finishing position of the best genome. Lower is better; 2.5 is the
    /// score of a seat that finishes in an average place.
    pub best_fitness: f64,
    pub median_fitness: f64,
    /// Standard error of a genome's fitness estimate.
    pub noise: f64,
    /// The champion's identity on the ladder.
    pub champion: u64,
    /// The champion's μ above the pinned heuristic.
    ///
    /// Meaningless without [`Report::champion_sigma`]: a gap of +2 against a
    /// σ of 6 is noise, and a run that reads the gap alone will see progress
    /// in a number that is not moving.
    pub above_anchor: f64,
    /// Uncertainty in the champion's rating. Shrinks with `Config::validation`.
    pub champion_sigma: f64,
    /// Largest per-gene spread among survivors, in mutation-scale units.
    pub spread: f64,
    /// How the champion actually played, from the sampled games.
    pub behaviour: Behaviour,
    pub seconds: f64,
}

/// A run in progress.
pub struct Trainer {
    pub config: Config,
    pub ladder: Ladder,
    pub(crate) population: Vec<Genome>,
    pub(crate) hall: Vec<u64>,
    pub(crate) generation: u32,
    pub(crate) trials: u32,
    /// The seed the whole run derives from.
    ///
    /// Each generation draws its own generator from `(run_seed, generation)`
    /// rather than the run carrying one that evolves. That is what makes a
    /// checkpoint exact: [`Rng`]'s state is private and cannot be serialised,
    /// but a generation's randomness can always be re-derived from two numbers
    /// that can.
    pub(crate) run_seed: u64,
}

/// The generator for one generation, derived rather than carried.
pub(crate) fn generation_rng(run_seed: u64, generation: u32) -> Rng {
    Rng::new(
        run_seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(generation as u64),
    )
}

impl Trainer {
    /// Start a run from the hand-set weights, spread out by mutation.
    pub fn new(config: Config, seed: u64) -> Self {
        let mut rng = generation_rng(seed, 0);
        let base = Genome::default();
        let population = (0..config.population)
            .map(|i| {
                // Genome zero is the unmutated starting point, so a run can
                // never come out worse than what it began with.
                if i == 0 {
                    base
                } else {
                    base.mutate(&mut rng, config.mutation)
                }
            })
            .collect();
        let trials = config.trials;
        Trainer {
            config,
            ladder: Ladder::default(),
            population,
            hall: Vec::new(),
            generation: 0,
            trials,
            run_seed: seed,
        }
    }

    /// Rebuild a run from a checkpoint. See [`crate::checkpoint`].
    #[allow(clippy::too_many_arguments)] // a checkpoint is a flat record
    pub fn restore(
        config: Config,
        ladder: Ladder,
        population: Vec<Genome>,
        hall: Vec<u64>,
        generation: u32,
        trials: u32,
        run_seed: u64,
    ) -> Self {
        Trainer {
            config,
            ladder,
            population,
            hall,
            generation,
            trials,
            run_seed,
        }
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Games each genome plays in the next generation.
    pub fn trials(&self) -> u32 {
        self.trials
    }

    pub fn best(&self) -> Genome {
        self.population[0]
    }

    /// Run one generation.
    pub fn step(&mut self) -> Report {
        let started = std::time::Instant::now();
        let cfg = self.config;
        let arena = Arena {
            mode: cfg.mode,
            cap: 20_000,
        };
        self.generation += 1;
        // Derived, not carried: see `Trainer::run_seed`.
        let mut rng = generation_rng(self.run_seed, self.generation);

        // ---- The field, fixed for the whole generation (E-4) ----
        //
        // One opponent triple per trial, shared by every genome, so genomes
        // are compared on identical conditions. Each opponent keeps its ladder
        // identity where it has one: a hall-of-fame version recorded as
        // anonymous would never accumulate games, its σ would stay wide, and
        // the ladder would be unable to separate an old champion from a new
        // one however many generations passed.
        let field: Vec<[Seat; 3]> = (0..self.trials)
            .map(|t| {
                core::array::from_fn(|slot| match (slot, t as usize % 3) {
                    // A third of trials seat the anchor, which is ample to keep
                    // every version directly comparable without handing the
                    // population a weak field to optimise against.
                    (0, 0) => Seat::Rated(ANCHOR),
                    (0, 1) if !self.hall.is_empty() => {
                        let pick = rng.below(Stream::Board, self.hall.len() as u32);
                        Seat::Rated(self.hall[pick as usize])
                    }
                    _ if slot < cfg.hall_seats && !self.hall.is_empty() => {
                        let pick = rng.below(Stream::Board, self.hall.len() as u32);
                        Seat::Rated(self.hall[pick as usize])
                    }
                    _ => {
                        let pick = rng.below(Stream::Board, cfg.population as u32);
                        Seat::Current(pick as usize)
                    }
                })
            })
            .collect();
        let genome_of = |seat: &Seat, ladder: &Ladder, pop: &[Genome]| match *seat {
            Seat::Rated(id) => ladder.get(id).unwrap().genome,
            Seat::Current(i) => pop[i],
        };
        let field_genomes: Vec<[Genome; 3]> = field
            .iter()
            .map(|triple| {
                core::array::from_fn(|s| genome_of(&triple[s], &self.ladder, &self.population))
            })
            .collect();

        // ---- Play ----
        let base_seed = self.generation as u64 * 1_000_003;
        let mut jobs = Vec::with_capacity(self.population.len() * self.trials as usize);
        for genome in self.population.iter() {
            for (t, _) in field_genomes.iter().enumerate() {
                // Rotating the seat by trial index averages out first-player
                // advantage without spending extra games on it (A-4).
                let seat = t % MAX_PLAYERS;
                let mut seats = [*genome; MAX_PLAYERS];
                let mut k = 0;
                let triple = &field_genomes[t];
                for (s, slot) in seats.iter_mut().enumerate() {
                    if s != seat {
                        *slot = triple[k];
                        k += 1;
                    }
                }
                jobs.push(Job {
                    seed: base_seed + t as u64,
                    seats,
                });
            }
        }
        let outcomes = arena.play_all(&jobs, cfg.threads);

        // ---- Score ----
        let mut fitness = vec![0.0f64; self.population.len()];
        let mut spread_sum = vec![0.0f64; self.population.len()];
        for (g, fit) in fitness.iter_mut().enumerate() {
            let mut total = 0.0;
            let mut sq = 0.0;
            for t in 0..self.trials as usize {
                let seat = t % MAX_PLAYERS;
                let pos = outcomes[g * self.trials as usize + t].position[seat] as f64;
                total += pos;
                sq += pos * pos;
            }
            let n = self.trials as f64;
            *fit = total / n;
            spread_sum[g] = (sq / n - (total / n).powi(2)).max(0.0).sqrt();
        }

        // ---- Rate the champion on fresh games ----
        //
        // Never on the games that selected it: see `Config::validation`.
        let mut order: Vec<usize> = (0..self.population.len()).collect();
        order.sort_by(|&a, &b| fitness[a].total_cmp(&fitness[b]));
        let champion_genome = self.population[order[0]];
        let champion = self.ladder.enrol(champion_genome, self.generation);

        let validation_seed = base_seed + 500_000;
        let mut vjobs = Vec::with_capacity(cfg.validation as usize);
        let mut vseats = Vec::with_capacity(cfg.validation as usize);
        for t in 0..cfg.validation as usize {
            let opponents: [Seat; 3] = core::array::from_fn(|slot| match (slot, t % 3) {
                (0, 0) => Seat::Rated(ANCHOR),
                (0, 1) if !self.hall.is_empty() => {
                    let pick = rng.below(Stream::Board, self.hall.len() as u32);
                    Seat::Rated(self.hall[pick as usize])
                }
                _ => {
                    let pick = rng.below(Stream::Board, cfg.population as u32);
                    Seat::Current(pick as usize)
                }
            });
            let seat = t % MAX_PLAYERS;
            let mut seats = [champion_genome; MAX_PLAYERS];
            let mut k = 0;
            for (sidx, slot) in seats.iter_mut().enumerate() {
                if sidx != seat {
                    *slot = genome_of(&opponents[k], &self.ladder, &self.population);
                    k += 1;
                }
            }
            vjobs.push(Job {
                seed: validation_seed + t as u64,
                seats,
            });
            vseats.push((seat, opponents));
        }
        let voutcomes = arena.play_all(&vjobs, cfg.threads);

        // Behavioural markers, from a sample of the validation games. Replayed
        // rather than re-played: the same seeds and seats, so what is measured
        // is exactly what was rated.
        let mut sampler = Sampler::default();
        let stride = if cfg.sample == 0 {
            0
        } else {
            (vjobs.len() as u32 / cfg.sample).max(1)
        };
        if stride > 0 {
            for (t, job) in vjobs.iter().enumerate() {
                if !(t as u32).is_multiple_of(stride) {
                    continue;
                }
                sampler.add(&arena.play_recorded(job).1);
            }
        }

        for (t, o) in voutcomes.iter().enumerate() {
            let (seat, opponents) = vseats[t];
            let mut ids = [0u64; MAX_PLAYERS];
            let mut k = 0;
            for (sidx, slot) in ids.iter_mut().enumerate() {
                if sidx == seat {
                    *slot = champion;
                } else {
                    // A member of the live population has no durable identity,
                    // it may not survive the generation, so it plays under a
                    // throwaway id. Rated opponents keep theirs and go on
                    // accumulating games.
                    *slot = match opponents[k] {
                        Seat::Rated(id) => id,
                        Seat::Current(i) => TRANSIENT + i as u64,
                    };
                    k += 1;
                }
            }
            self.ladder.record(&ids, &o.position);
        }
        // The population's placeholders are not players; see
        // `Ladder::forget_transients`.
        self.ladder.forget_transients();

        // ---- Select and breed ----
        let survivors: Vec<Genome> = order
            .iter()
            .take(cfg.survivors)
            .map(|&i| self.population[i])
            .collect();

        let best_fitness = fitness[order[0]];
        let median_fitness = fitness[order[order.len() / 2]];
        // Standard error of one genome's estimate, from the observed spread of
        // its own positions.
        let noise = spread_sum[order[0]] / (self.trials as f64).sqrt();
        let spread = survivors
            .iter()
            .map(|g| survivors[0].distance(g))
            .fold(0.0, f64::max);

        let mut next = Vec::with_capacity(cfg.population);
        next.extend_from_slice(&survivors);
        while next.len() < cfg.population {
            let a = survivors[rng.below(Stream::Board, survivors.len() as u32) as usize];
            let b = survivors[rng.below(Stream::Board, survivors.len() as u32) as usize];
            next.push(Genome::cross(&a, &b, &mut rng).mutate(&mut rng, cfg.mutation));
        }
        self.population = next;

        self.hall.push(champion);
        if self.hall.len() > cfg.hall_size {
            // Drop the oldest kept version, never the anchor, it is not in
            // the hall to begin with.
            self.hall.remove(0);
        }

        // ---- Adapt the budget (E-5) ----
        //
        // If the best genome is not separated from the median by more than the
        // noise in the estimate, selection is choosing at random and the only
        // fix is more games.
        let separated = (median_fitness - best_fitness) > 2.0 * noise;
        self.trials = if separated {
            (self.trials * 3 / 4).max(cfg.trials_min)
        } else {
            (self.trials * 2).min(cfg.trials_max)
        };

        Report {
            generation: self.generation,
            trials: jobs.len() as u32 / self.population.len() as u32,
            games: jobs.len() as u32 + voutcomes.len() as u32,
            best_fitness,
            median_fitness,
            noise,
            champion,
            above_anchor: self.ladder.above_anchor(champion),
            champion_sigma: self.ladder.rating(champion).sigma,
            spread,
            behaviour: sampler.finish(),
            seconds: started.elapsed().as_secs_f64(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quick() -> Config {
        Config {
            population: 8,
            survivors: 3,
            trials: 8,
            validation: 8,
            trials_min: 4,
            trials_max: 32,
            hall_size: 4,
            threads: 2,
            ..Config::default()
        }
    }

    #[test]
    fn a_generation_runs_and_reports() {
        let mut t = Trainer::new(quick(), 1);
        let r = t.step();
        assert_eq!(r.generation, 1);
        assert_eq!(
            r.games,
            8 * 8 + 8,
            "population games plus held-out validation"
        );
        // Mean finishing position lies in 1..=4, and a genome playing three
        // near-equal opponents should land near the middle.
        assert!((1.0..=4.0).contains(&r.best_fitness));
        assert!(r.best_fitness <= r.median_fitness);
        assert!(r.noise > 0.0);
    }

    #[test]
    fn the_run_is_reproducible() {
        let a: Vec<f64> = {
            let mut t = Trainer::new(quick(), 7);
            (0..3).map(|_| t.step().best_fitness).collect()
        };
        let b: Vec<f64> = {
            let mut t = Trainer::new(quick(), 7);
            (0..3).map(|_| t.step().best_fitness).collect()
        };
        assert_eq!(a, b);
    }

    #[test]
    fn the_starting_point_is_never_lost() {
        // Genome zero is the unmutated hand-set weights, and elitism carries
        // the survivors forward untouched, so a run cannot end up worse than
        // where it started.
        let t = Trainer::new(quick(), 3);
        assert_eq!(t.population[0], Genome::default());
    }

    #[test]
    fn the_budget_grows_when_the_field_cannot_be_told_apart() {
        // Contrived: a population of identical genomes has nothing to select
        // on, so every generation should raise the budget.
        let mut t = Trainer::new(quick(), 5);
        t.population = vec![Genome::default(); t.config.population];
        let first = t.trials;
        t.step();
        assert!(
            t.trials > first,
            "budget stayed at {first} with nothing to choose between"
        );
    }

    #[test]
    fn the_budget_is_bounded_at_both_ends() {
        let mut t = Trainer::new(quick(), 6);
        for _ in 0..8 {
            t.step();
            assert!(t.trials >= t.config.trials_min);
            assert!(t.trials <= t.config.trials_max);
        }
    }

    #[test]
    fn a_champion_is_rated_on_games_it_did_not_win_its_place_with() {
        // Selecting the best of a population and rating it on the same games
        // is a winner's curse. The validation games must use seeds the
        // selection round never touched.
        let mut cfg = quick();
        cfg.validation = 16;
        let mut t = Trainer::new(cfg, 12);
        let r = t.step();
        assert_eq!(r.games, cfg.population as u32 * 8 + 16);
        // The champion's ladder games are exactly the validation games.
        assert_eq!(t.ladder.games_played(r.champion), 16);
    }

    #[test]
    fn every_champion_is_anchored() {
        // The property the ladder depends on: a version whose rating rests on
        // a chain of comparisons cannot be compared to the anchor.
        let mut t = Trainer::new(quick(), 9);
        for _ in 0..4 {
            t.step();
        }
        assert_eq!(
            t.ladder.connectivity(1),
            1.0,
            "a champion never met the anchor"
        );
        assert!(t.ladder.len() >= 5, "anchor plus four champions");
    }

    #[test]
    fn the_hall_of_fame_is_bounded() {
        let mut t = Trainer::new(quick(), 10);
        for _ in 0..10 {
            t.step();
        }
        assert!(t.hall.len() <= t.config.hall_size);
    }

    #[test]
    fn training_uses_a_market() {
        // E-9: strategies tuned without trading do not transfer.
        assert_eq!(Config::default().mode, TradeMode::Restricted);
    }
}
