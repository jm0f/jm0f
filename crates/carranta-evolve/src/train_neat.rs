//! Phase two's generation loop: NEAT over the full mixed-offer market.
//!
//! The measurement harness is phase one's, unchanged, because it was built to
//! be shared: common random numbers with the seat rotated (E-4), fitness as
//! mean finishing position (E-6), the anchor plus a hall of fame as the
//! opponent field (E-7), champions rated on held-out games (E-10) against a
//! pinned anchor (E-11), and a budget that grows only when selection cannot
//! tell the field apart (E-5). What changes is the genome, the market, and
//! selection: species with fitness sharing instead of truncation, because a
//! fresh topology plays badly before it plays well, and truncation would
//! delete every innovation the moment it appeared.
//!
//! E-9 is deliberately overturned here: phase one trained in `Restricted`
//! because a policy that cannot see mixed offers has no use for a market full
//! of them; this bot exists to trade, so it trains in the market it will
//! play, `Full`, every affordable shape enumerated under configurable caps.

use carranta_core::rng::Stream;
use carranta_core::state::{MAX_PLAYERS, OfferShapes, TradeMode};

use crate::arena::{Arena, Brain, NetJob};
use crate::behaviour::{Behaviour, Sampler};
use crate::ladder::{ANCHOR, Ladder};
use crate::neat::{Innovations, NeatGenome, Params, Species, speciate};
use crate::train::generation_rng;

/// See `train::TRANSIENT`: identities for live population members, far above
/// any real version id.
const TRANSIENT: u64 = 1 << 40;

/// Who is sitting in an opponent seat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seat {
    Rated(u64),
    Current(usize),
}

/// How a phase-two run is configured.
#[derive(Clone, Copy, Debug)]
pub struct NeatConfig {
    pub population: usize,
    pub trials: u32,
    pub validation: u32,
    pub trials_min: u32,
    pub trials_max: u32,
    pub hall_seats: usize,
    pub hall_size: usize,
    pub sample: u32,
    pub threads: usize,
    /// Cards a generated offer may give; `None` is bounded by the hand alone.
    pub give_cap: Option<u8>,
    /// Cards a generated offer may ask.
    pub want_cap: u8,
    /// Actions before a game is abandoned. Small in tests, generous in runs.
    pub cap: usize,
    pub mode: TradeMode,
    pub params: Params,
}

impl Default for NeatConfig {
    fn default() -> Self {
        NeatConfig {
            population: 96,
            trials: 48,
            validation: 96,
            trials_min: 16,
            trials_max: 8_192,
            hall_seats: 1,
            hall_size: 24,
            sample: 8,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
            give_cap: Some(2),
            want_cap: 2,
            cap: 20_000,
            mode: TradeMode::Full,
            params: Params::default(),
        }
    }
}

/// What one generation produced.
#[derive(Clone, Debug)]
pub struct NeatReport {
    pub generation: u32,
    pub trials: u32,
    pub games: u32,
    pub best_fitness: f64,
    pub median_fitness: f64,
    pub noise: f64,
    pub champion: u64,
    pub above_anchor: f64,
    pub champion_sigma: f64,
    /// How the population is structured: species alive, and the champion's
    /// size, which is the number a NEAT run is watched by.
    pub species: usize,
    pub champion_nodes: usize,
    pub champion_genes: usize,
    pub behaviour: Behaviour,
    pub seconds: f64,
}

/// A phase-two run in progress.
pub struct NeatTrainer {
    pub config: NeatConfig,
    pub ladder: Ladder<NeatGenome>,
    pub(crate) population: Vec<NeatGenome>,
    pub(crate) species: Vec<Species>,
    pub(crate) delta: f64,
    pub(crate) inn: Innovations,
    pub(crate) hall: Vec<u64>,
    pub(crate) generation: u32,
    pub(crate) trials: u32,
    pub(crate) run_seed: u64,
    /// The reigning champion, for export between generations.
    pub(crate) champion: u64,
}

impl NeatTrainer {
    /// Start a run from minimal networks: inputs wired straight to the
    /// output, no hidden nodes, structure to be earned.
    pub fn new(config: NeatConfig, seed: u64) -> Self {
        let mut rng = generation_rng(seed, 0);
        let population = (0..config.population)
            .map(|_| NeatGenome::minimal(&mut rng))
            .collect();
        NeatTrainer {
            delta: config.params.delta_start,
            trials: config.trials,
            config,
            ladder: Ladder::with_anchor(
                carranta_analytics::rating::Model::default(),
                NeatGenome::default(),
                "heuristic-v1",
            ),
            population,
            species: Vec::new(),
            inn: Innovations::new(),
            hall: Vec::new(),
            generation: 0,
            run_seed: seed,
            champion: ANCHOR,
        }
    }

    /// Rebuild a run from a checkpoint. See [`crate::checkpoint`].
    #[allow(clippy::too_many_arguments)] // a checkpoint is a flat record
    pub fn restore(
        config: NeatConfig,
        ladder: Ladder<NeatGenome>,
        population: Vec<NeatGenome>,
        species: Vec<Species>,
        delta: f64,
        inn: Innovations,
        hall: Vec<u64>,
        generation: u32,
        trials: u32,
        run_seed: u64,
        champion: u64,
    ) -> Self {
        NeatTrainer {
            config,
            ladder,
            population,
            species,
            delta,
            inn,
            hall,
            generation,
            trials,
            run_seed,
            champion,
        }
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn trials(&self) -> u32 {
        self.trials
    }

    pub fn delta(&self) -> f64 {
        self.delta
    }

    /// The reigning champion's genome, for export.
    pub fn champion_genome(&self) -> Option<&NeatGenome> {
        self.ladder.get(self.champion).map(|v| &v.genome)
    }

    /// Run one generation.
    pub fn step(&mut self) -> NeatReport {
        let started = std::time::Instant::now();
        let cfg = self.config;
        let arena = Arena {
            mode: cfg.mode,
            shapes: OfferShapes::Mixed {
                give: cfg.give_cap,
                want: cfg.want_cap,
            },
            cap: cfg.cap,
        };
        self.generation += 1;
        let mut rng = generation_rng(self.run_seed, self.generation);

        // ---- The field, fixed for the whole generation (E-4) ----
        let field: Vec<[Seat; 3]> = (0..self.trials)
            .map(|t| {
                core::array::from_fn(|slot| match (slot, t as usize % 3) {
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

        // ---- The roster: every brain compiled once ----
        //
        // Slot 0 is the anchor, played by the heuristic itself. Slots
        // 1..=population are the live genomes. Hall-of-fame versions follow,
        // in the order their ids appear in the hall, compiled from the ladder.
        let mut roster: Vec<Brain> = Vec::with_capacity(1 + cfg.population + self.hall.len());
        roster.push(Brain::Anchor);
        for g in &self.population {
            roster.push(Brain::Net(g.compile()));
        }
        let mut rated_slot = std::collections::HashMap::new();
        for &id in &self.hall {
            if id == ANCHOR || rated_slot.contains_key(&id) {
                continue;
            }
            let genome = &self.ladder.get(id).expect("hall ids are enrolled").genome;
            rated_slot.insert(id, roster.len() as u32);
            roster.push(Brain::Net(genome.compile()));
        }
        let slot_of = |seat: &Seat| -> u32 {
            match *seat {
                Seat::Rated(ANCHOR) => 0,
                Seat::Rated(id) => rated_slot[&id],
                Seat::Current(i) => 1 + i as u32,
            }
        };

        // ---- Play ----
        let base_seed = self.generation as u64 * 1_000_003;
        let mut jobs = Vec::with_capacity(self.population.len() * self.trials as usize);
        for gi in 0..self.population.len() {
            for (t, triple) in field.iter().enumerate() {
                let seat = t % MAX_PLAYERS;
                let mut seats = [0u32; MAX_PLAYERS];
                let mut k = 0;
                for (s, slot) in seats.iter_mut().enumerate() {
                    if s == seat {
                        *slot = 1 + gi as u32;
                    } else {
                        *slot = slot_of(&triple[k]);
                        k += 1;
                    }
                }
                jobs.push(NetJob {
                    seed: base_seed + t as u64,
                    seats,
                });
            }
        }
        let outcomes = arena.play_net_all(&roster, &jobs, cfg.threads);

        // ---- Score (E-6) ----
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

        // ---- Rate the champion on fresh games (E-10) ----
        let mut order: Vec<usize> = (0..self.population.len()).collect();
        order.sort_by(|&a, &b| fitness[a].total_cmp(&fitness[b]).then(a.cmp(&b)));
        let champion_genome = self.population[order[0]].clone();
        let champion = self.ladder.enrol(champion_genome.clone(), self.generation);
        self.champion = champion;
        let champion_slot = roster.len() as u32;
        roster.push(Brain::Net(champion_genome.compile()));

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
            let mut seats = [0u32; MAX_PLAYERS];
            let mut k = 0;
            for (sidx, slot) in seats.iter_mut().enumerate() {
                if sidx == seat {
                    *slot = champion_slot;
                } else {
                    *slot = slot_of(&opponents[k]);
                    k += 1;
                }
            }
            vjobs.push(NetJob {
                seed: validation_seed + t as u64,
                seats,
            });
            vseats.push((seat, opponents));
        }
        let voutcomes = arena.play_net_all(&roster, &vjobs, cfg.threads);

        // Behavioural markers, replayed from a sample of the validation games.
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
                sampler.add(&arena.play_net_recorded(&roster, job).1);
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
                    *slot = match opponents[k] {
                        Seat::Rated(id) => id,
                        Seat::Current(i) => TRANSIENT + i as u64,
                    };
                    k += 1;
                }
            }
            self.ladder.record(&ids, &o.position);
        }
        self.ladder.forget_transients();

        // ---- Speciate, share, breed ----
        let params = cfg.params;
        let mut species = speciate(&self.population, &self.species, self.delta, &params);
        // Hold the species count near the target by moving the threshold.
        if species.len() > params.target_species {
            self.delta += params.delta_step;
        } else if species.len() < params.target_species {
            self.delta = (self.delta - params.delta_step).max(params.delta_floor);
        }

        // Stagnation: a species that has stopped improving stops breeding.
        // The species holding the generation's best is immune, so the run as
        // a whole can never cull its own frontier.
        let best_species = species
            .iter()
            .position(|s| s.members.contains(&order[0]))
            .expect("the best genome lives in some species");
        for (si, s) in species.iter_mut().enumerate() {
            let best_here = s
                .members
                .iter()
                .map(|&m| fitness[m])
                .fold(f64::INFINITY, f64::min);
            if best_here < s.best {
                s.best = best_here;
                s.stale = 0;
            } else {
                s.stale += 1;
            }
            if si == best_species {
                s.stale = 0;
            }
        }
        let stagnant: Vec<usize> = (0..species.len())
            .filter(|&si| species[si].stale > params.stagnation)
            .collect();

        // Offspring quotas by shared fitness. Positions are lower-better, so
        // they become scores first; sharing divides by species size, which is
        // the mechanism that lets a small new species live long enough to
        // matter.
        let score = |f: f64| (5.0 - f).max(0.01);
        let species_score: Vec<f64> = species
            .iter()
            .enumerate()
            .map(|(si, s)| {
                if stagnant.contains(&si) {
                    0.0
                } else {
                    s.members.iter().map(|&m| score(fitness[m])).sum::<f64>()
                        / s.members.len() as f64
                }
            })
            .collect();
        let total: f64 = species_score.iter().sum();
        let mut quota: Vec<usize> = if total > 0.0 {
            species_score
                .iter()
                .map(|&sc| ((sc / total) * cfg.population as f64).floor() as usize)
                .collect()
        } else {
            vec![0; species.len()]
        };
        // Distribute the remainder by largest fractional part, index breaking
        // ties, so the quotas are a pure function of the scores.
        let mut leftover = cfg.population.saturating_sub(quota.iter().sum());
        let mut frac: Vec<(usize, f64)> = species_score
            .iter()
            .enumerate()
            .map(|(si, &sc)| {
                let exact = if total > 0.0 {
                    (sc / total) * cfg.population as f64
                } else {
                    0.0
                };
                (si, exact - exact.floor())
            })
            .collect();
        frac.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        for (si, _) in frac {
            if leftover == 0 {
                break;
            }
            quota[si] += 1;
            leftover -= 1;
        }

        // Breed. Each species with a quota keeps its own best unchanged
        // (elitism), then fills the rest by crossover within itself, parents
        // drawn from its better half.
        self.inn.begin_generation();
        let mut next: Vec<NeatGenome> = Vec::with_capacity(cfg.population);
        for (si, s) in species.iter().enumerate() {
            let n = quota[si];
            if n == 0 {
                continue;
            }
            let mut members: Vec<usize> = s.members.clone();
            members.sort_by(|&a, &b| fitness[a].total_cmp(&fitness[b]).then(a.cmp(&b)));
            next.push(self.population[members[0]].clone());
            let parents = &members[..members.len().div_ceil(2)];
            for _ in 1..n {
                let pa = parents[rng.below(Stream::Board, parents.len() as u32) as usize];
                let pb = parents[rng.below(Stream::Board, parents.len() as u32) as usize];
                let (fit, other) = if fitness[pa] <= fitness[pb] {
                    (pa, pb)
                } else {
                    (pb, pa)
                };
                let child = NeatGenome::cross(
                    &self.population[fit],
                    &self.population[other],
                    &mut rng,
                    &params,
                );
                next.push(child.mutate(&mut rng, &params, &mut self.inn));
            }
        }
        // Rounding or culling can leave the count short; the overall best
        // genome's line fills the gap.
        while next.len() < cfg.population {
            let child = self.population[order[0]].clone();
            next.push(child.mutate(&mut rng, &params, &mut self.inn));
        }
        next.truncate(cfg.population);

        let best_fitness = fitness[order[0]];
        let median_fitness = fitness[order[order.len() / 2]];
        let noise = spread_sum[order[0]] / (self.trials as f64).sqrt();
        let species_alive = species
            .iter()
            .enumerate()
            .filter(|(si, _)| quota[*si] > 0)
            .count();

        self.population = next;
        self.species = species;

        self.hall.push(champion);
        if self.hall.len() > cfg.hall_size {
            self.hall.remove(0);
        }

        // ---- Adapt the budget (E-5) ----
        let separated = (median_fitness - best_fitness) > 2.0 * noise;
        self.trials = if separated {
            (self.trials * 3 / 4).max(cfg.trials_min)
        } else {
            (self.trials * 2).min(cfg.trials_max)
        };

        NeatReport {
            generation: self.generation,
            trials: jobs.len() as u32 / cfg.population as u32,
            games: jobs.len() as u32 + voutcomes.len() as u32,
            best_fitness,
            median_fitness,
            noise,
            champion,
            above_anchor: self.ladder.above_anchor(champion),
            champion_sigma: self.ladder.rating(champion).sigma,
            species: species_alive,
            champion_nodes: {
                let mut nodes: Vec<u32> = champion_genome
                    .genes
                    .iter()
                    .flat_map(|g| [g.from, g.to])
                    .collect();
                nodes.sort_unstable();
                nodes.dedup();
                nodes.len()
            },
            champion_genes: champion_genome.genes.len(),
            behaviour: sampler.finish(),
            seconds: started.elapsed().as_secs_f64(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny and quick: a truncated cap and no market, because these tests
    /// assert the mechanics of the loop, not the quality of play. The market
    /// integration is covered where it is cheap, in the policy's own tests.
    fn quick() -> NeatConfig {
        NeatConfig {
            population: 6,
            trials: 4,
            validation: 4,
            trials_min: 4,
            trials_max: 16,
            hall_size: 4,
            sample: 0,
            threads: 2,
            cap: 400,
            mode: TradeMode::Disabled,
            ..NeatConfig::default()
        }
    }

    #[test]
    fn a_generation_runs_and_reports() {
        let mut t = NeatTrainer::new(quick(), 1);
        let r = t.step();
        assert_eq!(r.generation, 1);
        assert_eq!(r.games, 6 * 4 + 4);
        assert!((1.0..=4.0).contains(&r.best_fitness));
        assert!(r.best_fitness <= r.median_fitness);
        assert!(r.species >= 1);
        assert!(r.champion_genes >= crate::neat::INPUTS);
        assert_eq!(t.population.len(), 6, "the population count is preserved");
    }

    #[test]
    fn the_run_is_reproducible() {
        let go = |seed| {
            let mut t = NeatTrainer::new(quick(), seed);
            (0..3).map(|_| t.step().best_fitness).collect::<Vec<f64>>()
        };
        assert_eq!(go(7), go(7));
        assert_ne!(go(7), go(8), "and the seed actually matters");
    }

    #[test]
    fn every_champion_is_anchored() {
        let mut t = NeatTrainer::new(quick(), 9);
        for _ in 0..4 {
            t.step();
        }
        assert_eq!(t.ladder.connectivity(1), 1.0);
        assert!(t.ladder.len() >= 5, "anchor plus four champions");
        assert!(t.champion_genome().is_some(), "a champion is exportable");
    }

    #[test]
    fn the_budget_is_bounded_at_both_ends() {
        let mut t = NeatTrainer::new(quick(), 6);
        for _ in 0..5 {
            t.step();
            assert!(t.trials >= t.config.trials_min);
            assert!(t.trials <= t.config.trials_max);
        }
    }

    #[test]
    fn species_form_and_the_threshold_follows_them() {
        let mut t = NeatTrainer::new(quick(), 11);
        let d0 = t.delta();
        for _ in 0..3 {
            t.step();
        }
        assert!(!t.species.is_empty());
        // The threshold moved in whatever direction the count demanded; with
        // six genomes and a target of eight it can only have fallen.
        assert!(t.delta() < d0);
        assert!(t.delta() >= t.config.params.delta_floor);
    }
}
