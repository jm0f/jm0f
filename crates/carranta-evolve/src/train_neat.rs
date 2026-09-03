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

use carranta_core::rng::{Rng, Stream};
use carranta_core::state::{MAX_PLAYERS, OfferShapes, TradeMode};

use crate::arena::{Arena, Brain, NetJob};
use crate::behaviour::{Behaviour, Sampler};
use crate::ladder::{ANCHOR, Ladder};
use crate::mapelites::{Archive, Descriptor, Placed};
use crate::neat::{Innovations, NeatGenome, Params, Species, speciate};
use crate::train::generation_rng;

/// How eagerly a simplifying phase sheds (E-35), how long it tolerates no
/// progress before giving way, and how much room the next complexifying
/// phase is given above the floor it reached.
/// Age layers (E-36): how many, how many generations of age each spans, and
/// how many of the bottom layer's seats are refilled with fresh genomes every
/// generation. The point of the scheme is that a young lineage never has to
/// out-breed a champion three hundred generations older than it.
const ALPS_LAYERS: usize = 5;
const ALPS_AGE_GAP: u32 = 20;
/// The nursery is a share of the population rather than a count, so the
/// scheme behaves the same at any size.
fn alps_nursery(population: usize) -> usize {
    (population / ALPS_LAYERS / 2).max(1)
}

const SIMPLIFY_DELETE_P: f64 = 0.35;
const SIMPLIFY_PATIENCE: u32 = 10;
const COMPLEXITY_MARGIN: f64 = 50.0;

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
    /// A payoff per finishing position, highest place first, replacing the
    /// position-minus-bonus score when set (E-19). Fitness becomes the
    /// negated payoff, so lower still means better and everything reading
    /// fitness is untouched. An unwon first place pays the second place
    /// rate: the table's top entry is the price of a *win*, and a game
    /// nobody won has none to sell.
    pub payoff: Option<[f64; 4]>,
    pub hall_seats: usize,
    pub hall_size: usize,
    pub sample: u32,
    pub threads: usize,
    /// Cards a generated offer may give; `None` is bounded by the hand alone.
    pub give_cap: Option<u8>,
    /// Proposals generated per seat per turn (E-15). Three by default: with a
    /// minute's turn clock and paced answers, three asks are what actually
    /// fit at a table people sit at, and scarcity is what gives an ask an
    /// opportunity cost the fitness can feel.
    pub ask_cap: u8,
    /// Cards a generated offer may ask.
    pub want_cap: u8,
    /// What a win is worth beyond first place (E-17), subtracted from the
    /// finishing position because lower is better. At the default of 1.0 a
    /// won game scores 0 rather than 1, so first place is two steps clear of
    /// second where every other step is one. Zero is E-6's pure position
    /// fitness, which selects policies that avoid fourth rather than reach
    /// first.
    pub win_bonus: f64,
    /// Actions before a game is abandoned. Small in tests, generous in runs.
    pub cap: usize,
    pub mode: TradeMode,
    pub params: Params,
    /// Select on victory-point margin (own VP less the mean of the other
    /// three) instead of finishing position (E-20). Position is a four-valued
    /// observation; the margin carries most of what the game produced at the
    /// same cost. The ship gate stays rank-based either way, so the objective
    /// cannot drift toward point-farming unpunished.
    pub margin: bool,
    /// Evaluate in halving rounds (E-21): everyone plays a cheap first round,
    /// and each halving of the field doubles the deals for those still in.
    /// The budget concentrates where the ordering is actually decided, which
    /// is among the best few, not between the best and the median.
    pub halving: bool,
    /// Sample hall opponents by how often they still beat the population
    /// (E-22), rather than uniformly. A hall member every genome beats
    /// carries no gradient, and a uniformly sampled archive spends most of
    /// its games on exactly those members.
    pub pfsp: bool,
    /// Play every deal once per seat (E-23): the same board and dice with the
    /// genome in each of the four chairs, so seat and board luck cancel
    /// exactly instead of averaging out slowly.
    pub rotate: bool,
    /// Keep the anchor out of the training field (E-24). It is the measuring
    /// stick for the gap column, and a stick the population trains against
    /// is one third training set.
    pub held_out_anchor: bool,
    /// Evaluate the finalists a ply deep (E-28): in the last two halving
    /// rounds every network at the table, candidate and field alike, plays
    /// inside the beamed search. The Baldwinian form: search improves the
    /// measurement, the genome stays a network, and selection breeds
    /// evaluators that are good to search with, which is the condition the
    /// table now deploys them under. The early rounds stay shallow, because
    /// ranking a hundred and fifty juniors does not need the depth.
    pub deep_eval: bool,
    /// Alternate complexifying and simplifying phases (E-35).
    ///
    /// Additive mutation alone makes genome size a ratchet: a plateau removes
    /// the only downward pressure there ever was, so the search keeps adding
    /// dimensions exactly when it is already struggling to find a good one.
    /// A simplifying phase turns the additive operators off, turns deletion
    /// on, and suspends crossover, because otherwise the genes one parent
    /// sheds are handed straight back by the other.
    pub phased: bool,
    /// Breed in age layers (E-36), refilling the youngest continuously.
    ///
    /// Without it a population converges on whatever holds the crown and
    /// every newcomer is measured against it immediately, which is how a
    /// three-hundred-generation incumbent survives two league surgeries. A
    /// layer is a band of genotype age; competition and breeding happen
    /// inside one, so a lineage gets time to become good before it has to be
    /// better than the best thing in the run.
    pub alps: bool,
    /// Breed from a quality-diversity archive rather than from species
    /// (E-37), keeping the best player found at each *style* of play.
    ///
    /// The answer to a converged run. Species and age layers both slow
    /// convergence down; neither stops it, because every genome is still
    /// compared against every other on one number. An archive changes what a
    /// genome competes with: only the others that play the same way. A
    /// weak-but-strange lineage then survives on its strangeness, long enough
    /// to become strong. This replaces speciation and the layers both, so a
    /// run is one or the other and never both.
    pub qd: bool,
    /// Recorded games per genome per generation, for reading its descriptors.
    ///
    /// Descriptors need replayed logs and logs are not free, so this is small
    /// and the noise it leaves is real: a genome's cell can wobble between
    /// generations. That costs less than it looks, because a genome that
    /// lands in the wrong cell is compared against the wrong neighbours for
    /// one generation rather than culled.
    pub qd_games: u32,
}

impl Default for NeatConfig {
    fn default() -> Self {
        NeatConfig {
            population: 96,
            trials: 48,
            validation: 96,
            trials_min: 16,
            trials_max: 8_192,
            payoff: None,
            hall_seats: 1,
            hall_size: 24,
            sample: 8,
            threads: std::thread::available_parallelism().map_or(1, |n| n.get()),
            give_cap: Some(2),
            ask_cap: 3,
            want_cap: 2,
            win_bonus: 1.0,
            cap: 20_000,
            mode: TradeMode::Full,
            params: Params::default(),
            margin: false,
            halving: false,
            pfsp: false,
            rotate: false,
            held_out_anchor: false,
            deep_eval: false,
            phased: false,
            alps: false,
            qd: false,
            qd_games: 2,
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
    /// The champion's mean-position gap to the anchor in the paired match
    /// (E-16), negative when the champion is ahead, and the 95% half-width
    /// around it. A gap inside its interval has not been shown to exist.
    pub gap: f64,
    pub gap_ci: f64,
    /// The champion's share of the same paired games' wins (E-17), where a
    /// half is even, and the 95% half-width around it. Position and wins can
    /// disagree, and a run selecting on the blend should be watched by both.
    pub wins: f64,
    pub wins_ci: f64,
    /// How the population is structured: species alive, and the champion's
    /// size, which is the number a NEAT run is watched by.
    pub species: usize,
    pub champion_nodes: usize,
    pub champion_genes: usize,
    /// How many of the observation's senses the champion actually listens
    /// to: distinct inputs with at least one enabled outgoing gene. Under
    /// the sparse genesis (E-34) this starts near the spine and grows as
    /// crossover wakes dormant senses that earn their keep, which makes the
    /// annealing visible rather than assumed.
    pub champion_ears: usize,
    /// Mean enabled genes across the population it just bred, and whether it
    /// bred under the simplifying rules (E-35).
    pub mpc: f64,
    pub simplifying: bool,
    /// Mean genotype age of the population just bred (E-36). Under the layers
    /// it should sit well below the incumbent's, which is the whole point:
    /// most of the run is younger than whatever is winning.
    pub mean_age: f64,
    /// The quality-diversity archive (E-37): cells holding an elite, the mean
    /// fitness across them, and how many cells this generation reached for the
    /// first time. Coverage alone can rise while the archive fills with poor
    /// players, so the mean is printed beside it; a healthy run moves both.
    pub archive_filled: usize,
    pub archive_mean: f64,
    pub archive_found: usize,
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
    /// Ladder ids that stay in the opponent field for ever (E-26): enrolled
    /// outsiders such as a shipped baseline or an exploiter. The hall is a
    /// rolling window of recent champions and evicts; these do not, because
    /// they are in the field to hold a standard, not to represent recency.
    pub(crate) pinned: Vec<u64>,
    pub(crate) generation: u32,
    pub(crate) trials: u32,
    pub(crate) run_seed: u64,
    /// The reigning champion, for export between generations.
    pub(crate) champion: u64,
    /// How often each hall member still beat the population last generation,
    /// as a sampling weight (E-22). Checkpointed, because a resume that
    /// sampled the hall uniformly for a generation would diverge from the
    /// run it claims to continue.
    pub(crate) hall_weight: std::collections::HashMap<u64, f64>,
    /// Whether this generation breeds under the simplifying rules (E-35).
    pub(crate) simplifying: bool,
    /// Mean enabled genes above which a complexifying phase gives way to a
    /// simplifying one.
    pub(crate) ceiling: f64,
    /// The lowest mean complexity this simplifying phase has reached, and how
    /// many generations it has failed to beat: the phase ends when
    /// simplification stops paying, not after a fixed count.
    pub(crate) mpc_floor: f64,
    pub(crate) stalled: u32,
    /// Each genome's age in generations, for the layers (E-36). Parallel to
    /// `population`, and kept beside it rather than inside the genome so a
    /// champion's file stays what it always was.
    pub(crate) ages: Vec<u32>,
    /// The quality-diversity archive (E-37), empty and unused unless the run
    /// asked for it. It outlives any one generation, which is the point: a
    /// cell holds its elite until something plays that way and plays better.
    pub(crate) archive: Archive,
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
            pinned: Vec::new(),
            generation: 0,
            run_seed: seed,
            champion: ANCHOR,
            hall_weight: std::collections::HashMap::new(),
            simplifying: false,
            ceiling: 0.0,
            mpc_floor: f64::INFINITY,
            stalled: 0,
            ages: Vec::new(),
            archive: Archive::new(),
        }
    }

    /// Enrol an outside network into the permanent field (E-25, E-26): into
    /// the ladder at the generation its file names, and into the pinned list,
    /// which the hall's eviction never touches. A fresh run's baseline and a
    /// mid-run exploiter both come through here: each is in the field to hold
    /// a standard the population must answer, not to represent recency.
    pub fn seed_baseline(&mut self, genome: NeatGenome, generation: u32) -> u64 {
        let id = self.ladder.enrol(genome, generation);
        self.pinned.push(id);
        id
    }

    /// Take every pinned member of the named generation out of the field.
    ///
    /// The DELPHI lesson (E-26): the field should hold opponents that make
    /// distinctions between learners, and a pinned member dominated by
    /// another teaches lessons the population has already learned while
    /// diluting the seats of the one it has not. The ladder keeps the
    /// member's record; only the field forgets it.
    pub fn unpin(&mut self, generation: u32) -> usize {
        let before = self.pinned.len();
        let ladder = &self.ladder;
        self.pinned
            .retain(|&id| ladder.get(id).is_none_or(|v| v.generation != generation));
        before - self.pinned.len()
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
            pinned: Vec::new(),
            generation,
            trials,
            run_seed,
            champion,
            hall_weight: std::collections::HashMap::new(),
            simplifying: false,
            ceiling: 0.0,
            mpc_floor: f64::INFINITY,
            stalled: 0,
            ages: Vec::new(),
            archive: Archive::new(),
        }
    }

    /// Put a checkpointed archive back (E-37).
    pub fn restore_archive(&mut self, archive: Archive) {
        self.archive = archive;
    }

    /// The archive, for reading a run's coverage and for seeding one.
    pub fn archive(&self) -> &Archive {
        &self.archive
    }

    /// Offer a genome to the archive from outside the generation loop.
    ///
    /// Seeding (E-37). A run resumed into quality diversity starts with an
    /// empty grid, and an empty grid breeds from whatever batch happens to be
    /// loaded, which is a converged population: the archive would fill with a
    /// hundred and forty-four variations on one player. Seeding it from the
    /// run's own champion catalogue instead hands it every distinct body the
    /// run ever produced, which is the diversity reservoir it already owns.
    pub fn seed_archive(
        &mut self,
        genome: NeatGenome,
        fitness: f64,
        descriptor: Descriptor,
        generation: u32,
    ) -> Placed {
        self.archive.insert(genome, fitness, descriptor, generation)
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Begin a simplifying phase now (E-35).
    ///
    /// What `--phased` does when it is switched on mid-run: the reason to
    /// reach for it is a genome already too big, so the run starts by
    /// shedding rather than by waiting to cross a ceiling it is past.
    pub fn begin_simplifying(&mut self) {
        self.simplifying = true;
        self.mpc_floor = f64::INFINITY;
        self.stalled = 0;
    }

    /// The phase, its ceiling, the floor it is chasing and how long it has
    /// failed to beat it: state a resume must carry, or a run comes back
    /// mid-phase with no memory of why.
    pub fn phase_state(&self) -> (bool, f64, f64, u32) {
        (self.simplifying, self.ceiling, self.mpc_floor, self.stalled)
    }

    pub fn restore_phase(&mut self, simplifying: bool, ceiling: f64, floor: f64, stalled: u32) {
        self.simplifying = simplifying;
        self.ceiling = ceiling;
        self.mpc_floor = floor;
        self.stalled = stalled;
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

    /// Every champion this run has enrolled, best rated first, as
    /// `(label, generation, mu, sigma, games)`.
    ///
    /// The exported `champion.net` is overwritten every generation, so on disk
    /// only the newest survives. The ladder keeps all of them, which is what
    /// makes a past champion recoverable at all: this is the list you choose
    /// from. The pinned anchor is left out, being the heuristic rather than a
    /// network, and having no genome anybody could export.
    pub fn roster(&self) -> Vec<(String, u32, f64, f64, u32)> {
        self.ladder
            .standings(0)
            .into_iter()
            .filter(|(v, _, _)| v.id != ANCHOR)
            .map(|(v, r, games)| (v.label.clone(), v.generation, r.mu, r.sigma, games))
            .collect()
    }

    /// One past champion as a network file, chosen by label (`g042-0017`), by
    /// generation number (`42`), or by `best`.
    ///
    /// Answers the text of the file and the label it came from. A generation
    /// picks the champion enrolled *at* that generation, which is the number
    /// on the row of `history.csv` you were reading, and `best` picks by
    /// conservative rating rather than by recency, because the newest champion
    /// is not reliably the strongest one.
    pub fn export(&self, which: &str, run: &str) -> Option<(String, String)> {
        let which = which.trim();
        let rows = self.ladder.standings(0);
        let mut mine = rows.iter().filter(|(v, _, _)| v.id != ANCHOR);
        // `standings` is sorted best first, so `best` is simply the first.
        let (v, _, _) = if which == "best" {
            mine.next()?
        } else {
            mine.find(|(v, _, _)| {
                v.label == which
                    || which
                        .parse::<u32>()
                        .is_ok_and(|generation| v.generation == generation)
            })?
        };
        Some((
            v.label.clone(),
            v.genome.compile().show_from(v.generation, run),
        ))
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
            asks: cfg.ask_cap,
            cap: cfg.cap,
        };
        self.generation += 1;
        let mut rng = generation_rng(self.run_seed, self.generation);

        // ---- The roster: every brain compiled once ----
        //
        // Slot 0 is the anchor, played by the heuristic itself. Slots
        // 1..=population are the live genomes. Hall-of-fame versions follow,
        // in the order their ids appear in the hall, compiled from the ladder.
        // The opponent pool: the pinned members first, then the rolling hall.
        let pool: Vec<u64> = self
            .pinned
            .iter()
            .chain(self.hall.iter())
            .copied()
            .collect();
        let mut roster: Vec<Brain> = Vec::with_capacity(1 + cfg.population + pool.len());
        roster.push(Brain::Anchor);
        for g in &self.population {
            roster.push(Brain::Net(g.compile()));
        }
        let mut rated_slot = std::collections::HashMap::new();
        for &id in &pool {
            if id == ANCHOR || rated_slot.contains_key(&id) {
                continue;
            }
            let genome = &self.ladder.get(id).expect("hall ids are enrolled").genome;
            rated_slot.insert(id, roster.len() as u32);
            roster.push(Brain::Net(genome.compile()));
        }
        // The finalists' roster (E-28): the same brains a ply deep, used in
        // the last halving rounds when deep evaluation is on. The anchor
        // stays itself; it is a heuristic, not a net, and held out besides.
        let roster_deep: Vec<Brain> = if cfg.deep_eval {
            roster
                .iter()
                .map(|b| match b {
                    Brain::Net(n) => Brain::Deep(n.clone()),
                    other => other.clone(),
                })
                .collect()
        } else {
            Vec::new()
        };
        let slot_of = |seat: &Seat| -> u32 {
            match *seat {
                Seat::Rated(ANCHOR) => 0,
                Seat::Rated(id) => rated_slot[&id],
                Seat::Current(i) => 1 + i as u32,
            }
        };

        // ---- The rounds ----
        //
        // Classic evaluation is one round: every genome, `trials` deals.
        // Halving (E-21) is a schedule: everyone plays a cheap opening round,
        // and each halving of the field doubles the deals for those still in,
        // so the budget concentrates where the ordering is decided, among the
        // best few rather than between the best and the median. With rotation
        // (E-23) a deal costs four games, one per seat.
        let seats_per_deal = if cfg.rotate { MAX_PLAYERS as u32 } else { 1 };
        let base_deals = (self.trials / seats_per_deal).max(1);
        let rounds: Vec<(usize, u32)> = if cfg.halving {
            let mut rounds = Vec::new();
            let mut keep = cfg.population;
            let mut deals = (base_deals / 4).max(1);
            loop {
                rounds.push((keep, deals));
                if keep <= 6 {
                    break;
                }
                keep = keep.div_ceil(2);
                deals *= 2;
            }
            rounds
        } else {
            vec![(cfg.population, base_deals)]
        };

        // Hall sampling weights (E-22): last generation's meetings, squared
        // shortfall, floored so no member ever leaves the field entirely.
        // Uniform when PFSP is off, and uniform on the first generation after
        // a resume, which relearns the weights in one generation.
        let hall_cum: Vec<f64> = {
            let mut total = 0.0;
            pool.iter()
                .map(|id| {
                    total += if cfg.pfsp {
                        self.hall_weight.get(id).copied().unwrap_or(1.0)
                    } else {
                        1.0
                    };
                    total
                })
                .collect()
        };
        let pick_hall = |rng: &mut Rng, hall: &[u64], cum: &[f64]| -> u64 {
            let scale = cum.last().copied().unwrap_or(0.0);
            let x = rng.below(Stream::Board, 1 << 20) as f64 / (1u64 << 20) as f64 * scale;
            hall[cum.partition_point(|&c| c <= x).min(hall.len() - 1)]
        };

        // ---- Play, in rounds ----
        //
        // Within a round every surviving genome plays the same deals (E-4):
        // the same board, the same dice, the same opponents, so every
        // comparison the survivors face is paired rather than independent.
        let base_seed = self.generation as u64 * 1_000_003;
        let mut alive: Vec<usize> = (0..cfg.population).collect();
        let mut sum = vec![0.0f64; cfg.population];
        let mut sq = vec![0.0f64; cfg.population];
        let mut played = vec![0u32; cfg.population];
        let mut total_jobs = 0u32;
        // Per hall member: how often the population finished ahead of it, and
        // how often they met (E-22).
        let mut met: std::collections::HashMap<u64, (f64, f64)> = std::collections::HashMap::new();
        let mut deal_no = 0u64;
        let mut finalists: Vec<usize> = alive.clone();
        for (round, &(keep, deals)) in rounds.iter().enumerate() {
            if round > 0 {
                alive.sort_by(|&a, &b| {
                    let fa = sum[a] / played[a].max(1) as f64;
                    let fb = sum[b] / played[b].max(1) as f64;
                    fa.total_cmp(&fb).then(a.cmp(&b))
                });
                alive.truncate(keep);
            }
            let dealset: Vec<(u64, [Seat; 3])> = (0..deals)
                .map(|_| {
                    let d = deal_no;
                    deal_no += 1;
                    let triple: [Seat; 3] = core::array::from_fn(|slot| {
                        match (slot, d as usize % 3) {
                            // The anchor's arm, unless it is held out (E-24):
                            // the measuring stick for the gap column should
                            // not also be one third of the training set.
                            (0, 0) if !cfg.held_out_anchor => Seat::Rated(ANCHOR),
                            // Held out, the arm belongs to the pinned members
                            // (E-26). Guaranteed rather than weighted, because
                            // the sampling weights are averaged over the whole
                            // population, and to the average junior every
                            // strong opponent is equally unbeatable: the one
                            // opponent the run was given to answer measured no
                            // more games than the scar tissue beside it. An
                            // exploiter is in the field to be met, and this
                            // arm is a third of the featured seats meeting it.
                            (0, 0) if !self.pinned.is_empty() => {
                                let pick = rng.below(Stream::Board, self.pinned.len() as u32);
                                Seat::Rated(self.pinned[pick as usize])
                            }
                            _ if !pool.is_empty()
                                && (slot < cfg.hall_seats
                                    || (slot == 0 && d as usize % 3 <= 1)) =>
                            {
                                Seat::Rated(pick_hall(&mut rng, &pool, &hall_cum))
                            }
                            _ => {
                                let pick = rng.below(Stream::Board, cfg.population as u32);
                                Seat::Current(pick as usize)
                            }
                        }
                    });
                    (base_seed + d, triple)
                })
                .collect();
            let games_each = dealset.len() * seats_per_deal as usize;
            let mut jobs = Vec::with_capacity(alive.len() * games_each);
            for &gi in &alive {
                for (di, (seed, triple)) in dealset.iter().enumerate() {
                    let first = if cfg.rotate { 0 } else { di % MAX_PLAYERS };
                    for r in 0..seats_per_deal as usize {
                        let seat = (first + r) % MAX_PLAYERS;
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
                        jobs.push(NetJob { seed: *seed, seats });
                    }
                }
            }
            // The last two rounds are the finalists' (E-28): with deep
            // evaluation on, those games are played inside the search.
            let deep_round = cfg.deep_eval && round + 2 >= rounds.len();
            let table = if deep_round { &roster_deep } else { &roster };
            let outcomes = arena.play_net_all(table, &jobs, cfg.threads);
            total_jobs += jobs.len() as u32;

            // ---- Score (E-6, E-17, E-20) ----
            //
            // Position, less a bonus for the games actually won: pure
            // position is dense but prices first one step above second, so it
            // selects policies that trade wins for safe middles, and the
            // bonus buys the difference back. The margin signal (E-20) keeps
            // the same shape in victory points: the mean of the other three
            // hands less your own, lower better, several times the
            // information per game at the same cost.
            for (ai, &gi) in alive.iter().enumerate() {
                for j in 0..games_each {
                    let o = &outcomes[ai * games_each + j];
                    let di = j / seats_per_deal as usize;
                    let first = if cfg.rotate { 0 } else { di % MAX_PLAYERS };
                    let seat = (first + j % seats_per_deal as usize) % MAX_PLAYERS;
                    let won = o.winner == Some(seat as u8);
                    let score = if cfg.margin {
                        let own = o.vp[seat] as f64;
                        let others = (0..MAX_PLAYERS)
                            .filter(|&s| s != seat)
                            .map(|s| o.vp[s] as f64)
                            .sum::<f64>()
                            / (MAX_PLAYERS - 1) as f64;
                        (others - own) - if won { cfg.win_bonus } else { 0.0 }
                    } else {
                        match cfg.payoff {
                            Some(pay) => {
                                let mut place = o.position[seat] as usize - 1;
                                if place == 0 && !won {
                                    place = 1;
                                }
                                -pay[place]
                            }
                            None => o.position[seat] as f64 - if won { cfg.win_bonus } else { 0.0 },
                        }
                    };
                    sum[gi] += score;
                    sq[gi] += score * score;
                    played[gi] += 1;
                    // Who the genome met from the hall, and whether it
                    // finished ahead (E-22). The triple fills every seat that
                    // is not the genome's, in order.
                    let mut k = 0;
                    for s in 0..MAX_PLAYERS {
                        if s == seat {
                            continue;
                        }
                        if let Seat::Rated(id) = dealset[di].1[k]
                            && id != ANCHOR
                        {
                            let e = met.entry(id).or_insert((0.0, 0.0));
                            e.1 += 1.0;
                            if o.position[seat] < o.position[s] {
                                e.0 += 1.0;
                            } else if o.position[seat] == o.position[s] {
                                e.0 += 0.5;
                            }
                        }
                        k += 1;
                    }
                }
            }
            finalists = alive.clone();
        }
        let fitness: Vec<f64> = (0..cfg.population)
            .map(|g| sum[g] / played[g].max(1) as f64)
            .collect();
        let spread_sum: Vec<f64> = (0..cfg.population)
            .map(|g| {
                let n = played[g].max(1) as f64;
                (sq[g] / n - (sum[g] / n).powi(2)).max(0.0).sqrt()
            })
            .collect();

        // ---- Read each genome's style, and offer it to the archive (E-37) ----
        //
        // Only on a quality-diversity run, because it costs recorded games and
        // a run that does not breed from the archive has nothing to spend them
        // on. Every genome sits at seat 0 against three drawn from the field,
        // so the descriptors describe the genome rather than the table it
        // happened to be dealt into, and so two genomes are read under
        // comparable conditions rather than one being flattered by its
        // company.
        let mut archive_found = 0usize;
        if cfg.qd && cfg.qd_games > 0 {
            let style_seed = base_seed + 700_001;
            let mut style_rng = generation_rng(self.run_seed, self.generation + 1);
            // One set of tables for the whole population, so the comparison
            // between genomes is paired the way every other comparison in this
            // loop is (E-4).
            let tables: Vec<[u32; 3]> = (0..cfg.qd_games)
                .map(|_| {
                    core::array::from_fn(|_| {
                        if !pool.is_empty() && style_rng.below(Stream::Board, 3) == 0 {
                            let id = pick_hall(&mut style_rng, &pool, &hall_cum);
                            rated_slot.get(&id).copied().unwrap_or(0)
                        } else {
                            1 + style_rng.below(Stream::Board, cfg.population as u32)
                        }
                    })
                })
                .collect();
            let mut style_jobs = Vec::with_capacity(cfg.population * cfg.qd_games as usize);
            for gi in 0..cfg.population {
                for (t, table) in tables.iter().enumerate() {
                    style_jobs.push(NetJob {
                        seed: style_seed + t as u64,
                        seats: [1 + gi as u32, table[0], table[1], table[2]],
                    });
                }
            }
            // Threaded, because this is a few hundred recorded games and
            // everything else in the loop already uses the whole machine.
            let played = arena.play_net_all_recorded(&roster, &style_jobs, cfg.threads);
            for gi in 0..cfg.population {
                let mut style = Sampler::default();
                for t in 0..cfg.qd_games as usize {
                    style.add_seat(&played[gi * cfg.qd_games as usize + t].1, 0);
                }
                let descriptor = Descriptor::of(&style.finish());
                if self.archive.insert(
                    self.population[gi].clone(),
                    fitness[gi],
                    descriptor,
                    self.generation,
                ) == Placed::Discovered
                {
                    archive_found += 1;
                }
            }
        }

        // ---- Rate the champion on fresh games (E-10) ----
        let mut order: Vec<usize> = (0..self.population.len()).collect();
        order.sort_by(|&a, &b| fitness[a].total_cmp(&fitness[b]).then(a.cmp(&b)));
        let champion_genome = self.population[order[0]].clone();
        let champion = self.ladder.enrol(champion_genome.clone(), self.generation);
        self.champion = champion;
        let champion_slot = roster.len() as u32;
        // Validated the way it is deployed (E-28): a deep-eval run breeds
        // evaluators for the search, and reading one shallow misstates it by
        // about a position. On such a run the gap column is a deep ruler,
        // comparable within the run and deliberately not with shallow ones.
        roster.push(if cfg.deep_eval {
            Brain::Deep(champion_genome.compile())
        } else {
            Brain::Net(champion_genome.compile())
        });

        // ---- Validate the champion (E-16) ----
        //
        // A paired anchor-only match, the `versus` method. The first long run
        // validated against a field that was mostly random siblings, two of
        // three games seating no anchor at all, and printed +20 to +35 while
        // a paired match had the champion *behind* the anchor: the number
        // mostly said "better than a random genome of my own generation",
        // which every champion is. E-10's lesson one level up: held-out games
        // must be held out against the opponent the claim is about.
        //
        // Every board seed is played in all six ways two champions can sit
        // among four seats, so champion and anchor meet on identical boards
        // with identical dice and the seat advantage cancels exactly. The six
        // seatings of one board are strongly correlated, so a seed is one
        // observation: counting them as six would claim about sqrt(6) more
        // certainty than the experiment has.
        const PAIRINGS: [[bool; MAX_PLAYERS]; 6] = [
            [true, true, false, false],
            [true, false, true, false],
            [true, false, false, true],
            [false, true, true, false],
            [false, true, false, true],
            [false, false, true, true],
        ];
        let validation_seed = base_seed + 500_000;
        let vseeds = (cfg.validation as usize / PAIRINGS.len()).max(1);
        let mut vjobs = Vec::with_capacity(vseeds * PAIRINGS.len());
        for s in 0..vseeds {
            for mask in PAIRINGS {
                let seats: [u32; MAX_PLAYERS] =
                    core::array::from_fn(|i| if mask[i] { champion_slot } else { 0 });
                vjobs.push(NetJob {
                    seed: validation_seed + s as u64,
                    seats,
                });
            }
        }
        let voutcomes = arena.play_net_all(&roster, &vjobs, cfg.threads);

        // One gap per seed: the champion's mean position less the anchor's,
        // averaged over the six seatings. Negative is the champion ahead.
        let mut gaps = Vec::with_capacity(vseeds);
        // And the same seeds' win share (E-17). Two champions sit against two
        // anchors in every seating, so a half is even, and this is the column
        // that says whether the win bonus is buying what it was added for.
        let mut shares = Vec::with_capacity(vseeds);
        for chunk in voutcomes.chunks(PAIRINGS.len()) {
            let mut gap = 0.0;
            let (mut won, mut decided) = (0.0, 0.0);
            for (o, mask) in chunk.iter().zip(PAIRINGS.iter()) {
                let (mut mine, mut theirs) = (0.0, 0.0);
                for (seat, &is_champion) in mask.iter().enumerate() {
                    let p = o.position[seat] as f64 / 2.0;
                    if is_champion {
                        mine += p;
                    } else {
                        theirs += p;
                    }
                }
                gap += (mine - theirs) / PAIRINGS.len() as f64;
                if let Some(w) = o.winner {
                    decided += 1.0;
                    if mask[w as usize] {
                        won += 1.0;
                    }
                }
            }
            gaps.push(gap);
            // A board nobody won says nothing either way, so it counts as
            // even rather than as a loss.
            shares.push(if decided > 0.0 { won / decided } else { 0.5 });
        }
        // One seed is one observation for both columns, so both intervals are
        // taken the same way over the same seeds.
        //
        // One seed has no spread to measure. Zero would claim certainty; an
        // infinite half-width says the interval is the whole line, which is
        // the truth.
        let interval = |xs: &[f64], mean: f64| {
            let n = xs.len() as f64;
            if xs.len() > 1 {
                let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
                1.96 * (var / n).sqrt()
            } else {
                f64::INFINITY
            }
        };
        let n = gaps.len() as f64;
        let gap = gaps.iter().sum::<f64>() / n;
        let gap_ci = interval(&gaps, gap);
        let wins = shares.iter().sum::<f64>() / n;
        let wins_ci = interval(&shares, wins);

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

        // The same games feed the ladder, which makes its ratings
        // anchor-grounded too: every durable game a champion has is against
        // the anchor, so the standings and the printed gap can no longer
        // tell two different stories.
        for (t, o) in voutcomes.iter().enumerate() {
            let mask = PAIRINGS[t % PAIRINGS.len()];
            let ids: [u64; MAX_PLAYERS] =
                core::array::from_fn(|i| if mask[i] { champion } else { ANCHOR });
            self.ladder.record(&ids, &o.position);
        }

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
        // What mutation may do this generation (E-35). A simplifying phase
        // turns the additive operators off and deletion on; a complexifying
        // one is the run as it always was.
        let params = if cfg.phased && self.simplifying {
            Params {
                add_node_p: 0.0,
                add_conn_p: 0.0,
                del_conn_p: SIMPLIFY_DELETE_P,
                ..params
            }
        } else {
            Params {
                del_conn_p: 0.0,
                ..params
            }
        };
        // Ages, for the layers (E-36), spread across the bands rather than
        // started together. A population bred from one ancestor ages in
        // lockstep: everybody crosses a band boundary in the same
        // generation, the youngest band empties, and its seats go to fresh
        // genomes, which is a partial wipe every twenty generations. It
        // showed as mean complexity sawing between 160 and 78 with the mean
        // age resetting on the same rows. Staggering the start breaks the
        // cohort, and from then on the ages spread on their own.
        if self.ages.len() != self.population.len() {
            let span = (ALPS_LAYERS as u32 * ALPS_AGE_GAP).max(1);
            self.ages = (0..self.population.len())
                .map(|i| (i as u32 * span) / self.population.len().max(1) as u32)
                .collect();
        }
        let mut next: Vec<NeatGenome> = Vec::with_capacity(cfg.population);
        let mut next_ages: Vec<u32> = Vec::with_capacity(cfg.population);
        let layer_of = |age: u32| (age / ALPS_AGE_GAP).min(ALPS_LAYERS as u32 - 1) as usize;

        if cfg.qd {
            // Quality diversity (E-37). The archive is the population that
            // persists; what `self.population` holds between generations is
            // only the batch being measured. Every seat is a fresh mutation of
            // a parent drawn uniformly from the filled cells, which is what
            // makes a lone genome in a strange cell as likely to breed as the
            // best player in the run.
            //
            // Crossover is deliberately absent. Two elites drawn from distant
            // cells are good at different things, and their child is reliably
            // good at neither; the archive already supplies the diversity that
            // crossover is usually there to preserve.
            //
            // The first generation has an empty archive, because nothing has
            // been measured yet. It breeds from the batch it just scored, and
            // from the generation after that the archive is never empty again.
            for _ in 0..cfg.population {
                let parent = match self.archive.pick(&mut rng) {
                    Some(e) => e.genome.clone(),
                    None => {
                        let pick = rng.below(Stream::Board, cfg.population as u32) as usize;
                        self.population[pick].clone()
                    }
                };
                next.push(parent.mutate(&mut rng, &params, &mut self.inn));
                next_ages.push(0);
            }
        } else if cfg.alps {
            // One band of age at a time. Everything inside a band competes
            // and breeds only with itself, so a lineage twenty generations
            // old is measured against its own kind rather than against the
            // best thing the run has ever made.
            let per_layer = cfg.population / ALPS_LAYERS;
            // Oldest band first, and every seat an empty band cannot use
            // goes to the youngest, not to the next band down. Early on the
            // upper bands are empty because nothing is old enough for them,
            // and handing their seats to the eldest band that does exist
            // would make age the thing the scheme rewards.
            let mut carry = 0usize;
            for layer in (0..ALPS_LAYERS).rev() {
                // The species still do the sharing; the layer only says who
                // is eligible, so a young species is protected twice over.
                // Stagnation does not cull here, and applying it as well
                // nearly emptied the run the first time this was switched
                // on: after a long drought most species are stale, so every
                // band came up empty and its seats went to fresh genomes,
                // which replaced the population and left the champion
                // scoring against near-random opponents. Age is this
                // scheme's own turnover, and the oldest band's fixed size
                // already caps how much of the run one old lineage may hold.
                let groups: Vec<Vec<usize>> = species
                    .iter()
                    .map(|sp| {
                        sp.members
                            .iter()
                            .copied()
                            .filter(|&m| layer_of(self.ages[m]) == layer)
                            .collect::<Vec<usize>>()
                    })
                    .filter(|g| !g.is_empty())
                    .collect();

                let mut slots = if layer == 0 {
                    per_layer + carry
                } else {
                    per_layer
                };
                if layer == 0 {
                    // The nursery, refilled every generation. This is the
                    // part that guarantees the run never fully converges.
                    let fresh = alps_nursery(cfg.population).min(slots);
                    for _ in 0..fresh {
                        next.push(NeatGenome::minimal(&mut rng));
                        next_ages.push(0);
                    }
                    slots -= fresh;
                }
                if groups.is_empty() {
                    // Nobody of this age yet. The seats go to the youngest
                    // band, which is where a run with room to grow wants
                    // them.
                    if layer == 0 {
                        for _ in 0..slots {
                            next.push(NeatGenome::minimal(&mut rng));
                            next_ages.push(0);
                        }
                    } else {
                        carry += slots;
                    }
                    continue;
                }

                // Shared fitness, inside the band.
                let scores: Vec<f64> = groups
                    .iter()
                    .map(|g| g.iter().map(|&m| score(fitness[m])).sum::<f64>() / g.len() as f64)
                    .collect();
                let sum: f64 = scores.iter().sum();
                let mut share: Vec<usize> = scores
                    .iter()
                    .map(|&sc| {
                        if sum > 0.0 {
                            ((sc / sum) * slots as f64).floor() as usize
                        } else {
                            0
                        }
                    })
                    .collect();
                let mut left = slots.saturating_sub(share.iter().sum());
                let mut frac: Vec<(usize, f64)> = scores
                    .iter()
                    .enumerate()
                    .map(|(gi, &sc)| {
                        let exact = if sum > 0.0 {
                            (sc / sum) * slots as f64
                        } else {
                            0.0
                        };
                        (gi, exact - exact.floor())
                    })
                    .collect();
                frac.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                for (gi, _) in frac {
                    if left == 0 {
                        break;
                    }
                    share[gi] += 1;
                    left -= 1;
                }

                for (gi, g) in groups.iter().enumerate() {
                    let n = share[gi];
                    if n == 0 {
                        continue;
                    }
                    let mut members = g.clone();
                    members.sort_by(|&a, &b| fitness[a].total_cmp(&fitness[b]).then(a.cmp(&b)));
                    // The band's own best carries over untouched, and ages.
                    next.push(self.population[members[0]].clone());
                    next_ages.push(self.ages[members[0]] + 1);
                    let parents = &members[..members.len().div_ceil(2)];
                    for _ in 1..n {
                        let pa = parents[rng.below(Stream::Board, parents.len() as u32) as usize];
                        let pb = parents[rng.below(Stream::Board, parents.len() as u32) as usize];
                        let (fit, other) = if fitness[pa] <= fitness[pb] {
                            (pa, pb)
                        } else {
                            (pb, pa)
                        };
                        let child = if cfg.phased && self.simplifying {
                            self.population[fit].clone()
                        } else {
                            NeatGenome::cross(
                                &self.population[fit],
                                &self.population[other],
                                &mut rng,
                                &params,
                            )
                        };
                        next.push(child.mutate(&mut rng, &params, &mut self.inn));
                        // A child is as old as its oldest parent: age tracks
                        // the lineage, not the individual, or every child
                        // would be new and the layers would mean nothing.
                        next_ages.push(self.ages[fit].max(self.ages[other]) + 1);
                    }
                }
            }
            // Short of a full house, the youngest blood fills the gap: under
            // this scheme the answer to a shortfall is never more of the
            // incumbent.
            while next.len() < cfg.population {
                next.push(NeatGenome::minimal(&mut rng));
                next_ages.push(0);
            }
            next.truncate(cfg.population);
            next_ages.truncate(cfg.population);
        } else {
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
                    // Crossover is suspended while simplifying (E-35): what one
                    // parent sheds, the other hands straight back, and the phase
                    // would never finish.
                    let child = if cfg.phased && self.simplifying {
                        self.population[fit].clone()
                    } else {
                        NeatGenome::cross(
                            &self.population[fit],
                            &self.population[other],
                            &mut rng,
                            &params,
                        )
                    };
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
            next_ages = vec![0; next.len()];
        }

        let best_fitness = fitness[order[0]];
        let median_fitness = fitness[order[order.len() / 2]];
        let noise = spread_sum[order[0]] / (played[order[0]].max(1) as f64).sqrt();
        let species_alive = species
            .iter()
            .enumerate()
            .filter(|(si, _)| quota[*si] > 0)
            .count();

        // Where the phase stands now that the children exist (E-35). Mean
        // enabled genes is the measure, because a sleeping gene costs the
        // network nothing and is not the dimension a mutation must search
        // around.
        let mpc = next.iter().map(|g| g.enabled_len() as f64).sum::<f64>() / next.len() as f64;
        if cfg.phased {
            if self.simplifying {
                if mpc < self.mpc_floor - 0.5 {
                    self.mpc_floor = mpc;
                    self.stalled = 0;
                } else {
                    self.stalled += 1;
                }
                // The phase ends when shedding stops paying, not on a count.
                if self.stalled >= SIMPLIFY_PATIENCE {
                    self.simplifying = false;
                    self.ceiling = mpc + COMPLEXITY_MARGIN;
                    self.stalled = 0;
                }
            } else {
                if self.ceiling <= 0.0 {
                    self.ceiling = mpc + COMPLEXITY_MARGIN;
                }
                if mpc > self.ceiling {
                    self.begin_simplifying();
                }
            }
        }

        self.population = next;
        self.ages = next_ages;
        self.species = species;

        self.hall.push(champion);
        if self.hall.len() > cfg.hall_size {
            self.hall.remove(0);
        }

        // Refresh the sampling weights from this generation's meetings
        // (E-22): the more of the population a member still beats, the more
        // games it earns next generation.
        self.hall_weight = met
            .iter()
            .map(|(&id, &(ahead, games))| (id, (1.0 - ahead / games).powi(2).max(0.02)))
            .collect();

        // ---- Adapt the budget (E-5) ----
        //
        // Classic separation asks whether the best stands clear of the
        // median, which is trivially easy and lets the budget sit at the
        // floor for ever. Under halving the question is asked where selection
        // actually happens: does the best stand clear of the median finalist?
        let separated = if cfg.halving {
            let mut f: Vec<f64> = finalists.iter().map(|&g| fitness[g]).collect();
            f.sort_by(|a, b| a.total_cmp(b));
            (f[f.len() / 2] - best_fitness) > 2.0 * noise
        } else {
            (median_fitness - best_fitness) > 2.0 * noise
        };
        self.trials = if separated {
            (self.trials * 3 / 4).max(cfg.trials_min)
        } else {
            (self.trials * 2).min(cfg.trials_max)
        };

        NeatReport {
            archive_filled: self.archive.filled(),
            archive_mean: self.archive.mean_fitness(),
            archive_found,
            generation: self.generation,
            trials: total_jobs / cfg.population as u32,
            games: total_jobs + voutcomes.len() as u32,
            best_fitness,
            median_fitness,
            noise,
            champion,
            gap,
            gap_ci,
            wins,
            wins_ci,
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
            mpc,
            simplifying: cfg.phased && self.simplifying,
            mean_age: if self.ages.is_empty() {
                0.0
            } else {
                self.ages.iter().map(|&a| a as f64).sum::<f64>() / self.ages.len() as f64
            },
            champion_ears: {
                let mut ears: Vec<u32> = champion_genome
                    .genes
                    .iter()
                    .filter(|g| g.enabled && (g.from as usize) < crate::neat::INPUTS)
                    .map(|g| g.from)
                    .collect();
                ears.sort_unstable();
                ears.dedup();
                ears.len()
            },
            behaviour: sampler.finish(),
            seconds: started.elapsed().as_secs_f64(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use carranta_bot::net::Net;

    #[test]
    fn a_simplifying_phase_sheds_and_gives_way_when_shedding_stops_paying() {
        // The ratchet and its release (E-35). A run that only ever adds must
        // grow; the phased controller must actually shrink it, and must not
        // shrink for ever, or the population dissolves into nothing.
        let mut cfg = quick();
        cfg.phased = true;
        cfg.params.add_conn_p = 0.9; // grow fast, so there is something to shed
        let mut t = NeatTrainer::new(cfg, 11);
        // Sparse genesis (E-34) starts most genes asleep, so the baseline is
        // what this run itself began with, not the width of the observation.
        let born = t.step().mpc;
        for _ in 0..6 {
            t.step();
        }
        let grown = t.step().mpc;
        assert!(
            grown > born,
            "the complexifying phase added something to shed: {grown} against {born}"
        );

        // Switch on the shedding, as a resume with --phased does.
        t.begin_simplifying();
        let mut smallest = f64::INFINITY;
        let mut simplified_for = 0;
        let mut gave_way = false;
        for _ in 0..40 {
            let r = t.step();
            if r.simplifying {
                simplified_for += 1;
                smallest = smallest.min(r.mpc);
            } else {
                gave_way = true;
                break;
            }
        }
        assert!(simplified_for > 0, "it simplified at all");
        assert!(
            smallest < grown,
            "simplifying shed nothing: {smallest} against {grown}"
        );
        assert!(
            gave_way,
            "the phase never ended; it would shrink the population away"
        );
        // And what it hands back is a ceiling above where it landed, so the
        // next complexifying phase has somewhere to go.
        let (simplifying, ceiling, _, _) = t.phase_state();
        assert!(!simplifying);
        assert!(ceiling > smallest, "the ceiling leaves room: {ceiling}");
    }

    #[test]
    fn age_layers_keep_the_population_young_and_the_nursery_full() {
        // The scheme's promise (E-36): however long one lineage holds the
        // crown, most of the run is younger than it, because the youngest
        // band is refilled every generation and nobody breeds across bands.
        let mut cfg = quick();
        cfg.alps = true;
        cfg.population = 30;
        let mut t = NeatTrainer::new(cfg, 5);
        let mut oldest_seen = 0u32;
        for _ in 0..40 {
            let r = t.step();
            oldest_seen = oldest_seen.max(t.ages.iter().copied().max().unwrap_or(0));
            assert_eq!(t.ages.len(), t.population.len(), "an age for every genome");
            // Somebody is always brand new: that is the nursery.
            assert!(
                t.ages.iter().any(|&a| a == 0),
                "the youngest band was not refilled"
            );
            assert!(
                r.mean_age <= oldest_seen as f64,
                "the mean cannot exceed the oldest"
            );
        }
        // The run keeps its accumulated structure: the layers are turnover,
        // not a restart. Switching them on once wiped the population, because
        // stagnation culled the bands empty and the seats went to fresh
        // genomes, and a population of near-random genomes is also the
        // opponent field, so the fitness signal went with it.
        let mean_genes = t
            .population
            .iter()
            .map(|g| g.enabled_len() as f64)
            .sum::<f64>()
            / t.population.len() as f64;
        assert!(
            mean_genes > crate::neat::GENESIS_SPINE as f64 * 1.2,
            "the population collapsed to minimal genomes: mean {mean_genes}"
        );
        // Lineages do accumulate age, or the layers would be decoration.
        assert!(oldest_seen > 5, "nothing ever grew old: {oldest_seen}");
        // And every band is occupied, which is what stops the run aging as
        // one cohort and emptying a band all at once.
        let bands = t
            .ages
            .iter()
            .map(|&a| (a / ALPS_AGE_GAP).min(ALPS_LAYERS as u32 - 1))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            bands.len() >= 2,
            "the whole run sat in one band: {:?}",
            t.ages
        );
        // And a real share of the run is always in the youngest band, which
        // is what stops an incumbent crowding it out.
        let young = t.ages.iter().filter(|&&a| a < ALPS_AGE_GAP).count();
        assert!(
            young * 4 >= t.ages.len(),
            "only {young} of {} genomes were young: {:?}",
            t.ages.len(),
            t.ages
        );
    }

    #[test]
    fn an_unlayered_run_keeps_breeding_as_it_always_did() {
        // The scheme is opt-in, so without it nothing about breeding moves.
        let mut t = NeatTrainer::new(quick(), 5);
        for _ in 0..5 {
            let r = t.step();
            assert_eq!(r.mean_age, 0.0, "no ages accrue without the layers");
        }
    }

    #[test]
    fn an_unphased_run_never_deletes() {
        // The controller is opt-in: without it the run breeds exactly as it
        // always did, which is what makes the comparison honest.
        let mut t = NeatTrainer::new(quick(), 11);
        for _ in 0..5 {
            let r = t.step();
            assert!(!r.simplifying, "no phase without being asked");
        }
        let (simplifying, ceiling, _, _) = t.phase_state();
        assert!(!simplifying && ceiling == 0.0, "no phase state accrued");
    }

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
    fn a_quality_diversity_run_fills_an_archive_and_breeds_from_it() {
        // The claim E-37 rests on: a run that keeps an archive discovers
        // cells, keeps what it discovers across generations, and does not
        // simply reproduce its population count.
        let mut t = NeatTrainer::new(
            NeatConfig {
                qd: true,
                qd_games: 1,
                ..quick()
            },
            5,
        );
        let first = t.step();
        assert!(
            first.archive_filled > 0,
            "nothing reached a cell in the first generation"
        );
        assert_eq!(
            first.archive_found, first.archive_filled,
            "every cell in an empty archive is a discovery"
        );

        let mut widest = first.archive_filled;
        for _ in 0..6 {
            let r = t.step();
            assert!(
                r.archive_filled >= widest,
                "coverage went backwards, {} after {widest}: a cell must \
                 hold its elite until something better plays the same way",
                r.archive_filled
            );
            widest = r.archive_filled;
        }
        assert!(
            widest <= crate::mapelites::CELLS * crate::mapelites::CELLS,
            "more cells filled than the grid has"
        );
        // The population is still the configured size: the archive is what
        // persists, not what is measured.
        assert_eq!(t.population.len(), 6);
    }

    #[test]
    fn quality_diversity_keeps_a_weak_genome_that_plays_unlike_the_rest() {
        // The mechanism, stated as a test on the archive the run actually
        // builds: the best player is not the only one that survives, because
        // survival is per cell. If this ever collapses to one entry the run
        // has become plain selection wearing a grid.
        let mut t = NeatTrainer::new(
            NeatConfig {
                qd: true,
                qd_games: 1,
                population: 12,
                ..quick()
            },
            9,
        );
        for _ in 0..4 {
            t.step();
        }
        let archive = t.archive();
        assert!(archive.filled() > 1, "the archive collapsed to one cell");
        let best = archive.best().expect("something is in there").0.fitness;
        assert!(
            archive.iter().any(|e| e.fitness > best),
            "every survivor is the best one, which is not an archive"
        );
    }

    #[test]
    fn a_quality_diversity_run_does_not_also_run_the_layers() {
        // They are alternatives, and a run doing both would be breeding twice
        // from two schemes that disagree about what a population is.
        let cfg = NeatConfig {
            qd: true,
            qd_games: 1,
            alps: true,
            ..quick()
        };
        let mut t = NeatTrainer::new(cfg, 3);
        t.step();
        // The archive branch runs first and fills every seat, so the layers
        // never see the population: every genome comes back newly bred.
        assert!(
            t.ages.iter().all(|&a| a == 0),
            "the layers aged a population the archive had already bred"
        );
    }

    #[test]
    fn a_generation_runs_and_reports() {
        let mut t = NeatTrainer::new(quick(), 1);
        let r = t.step();
        assert_eq!(r.generation, 1);
        // Trials games plus the paired validation: 4 asked-for validation
        // games round up to one seed, played in all six seatings.
        assert_eq!(r.games, 6 * 4 + 6);
        // A won game scores its position less the bonus, so the floor is one
        // less than first place rather than first place itself (E-17).
        let floor = 1.0 - t.config.win_bonus;
        assert!((floor..=4.0).contains(&r.best_fitness));
        assert!(r.best_fitness <= r.median_fitness);
        assert!((0.0..=1.0).contains(&r.wins), "a share of the paired wins");
        assert!(r.species >= 1);
        assert!(r.champion_genes >= crate::neat::INPUTS);
        assert_eq!(t.population.len(), 6, "the population count is preserved");
    }

    #[test]
    fn the_win_bonus_prices_first_place_without_changing_the_games() {
        // E-17. The bonus is a scoring choice, not a different experiment:
        // one seed plays one set of games either way, and every genome's
        // score falls by the bonus times the share of them it won. So the
        // field can only move down, never up, and the run costs the same.
        let plain = NeatTrainer::new(
            NeatConfig {
                win_bonus: 0.0,
                ..quick()
            },
            5,
        )
        .step();
        let priced = NeatTrainer::new(
            NeatConfig {
                win_bonus: 1.0,
                ..quick()
            },
            5,
        )
        .step();
        assert_eq!(plain.games, priced.games, "the same games are played");
        assert!(priced.best_fitness <= plain.best_fitness);
        assert!(priced.median_fitness <= plain.median_fitness);
        // And pure position is still reachable, which is what a run started
        // before the bonus resumes as.
        assert!(plain.best_fitness >= 1.0, "position alone floors at first");
    }

    #[test]
    fn any_past_champion_can_be_taken_back_out_of_the_run() {
        // The run overwrites one `champion.net`, so a champion from an earlier
        // generation exists only in the ladder. Getting it back has to work
        // from there or those generations are gone the moment they pass.
        let mut t = NeatTrainer::new(quick(), 5);
        for _ in 0..3 {
            t.step();
        }
        let roster = t.roster();
        assert_eq!(roster.len(), 3, "one champion enrolled per generation");
        // Best rated first, conservatively (mu less three sigma, the order
        // the ladder actually stands on), and the anchor is not in the list:
        // it is the heuristic, and has no network to write down.
        for pair in roster.windows(2) {
            assert!(
                pair[0].2 - 3.0 * pair[0].3 >= pair[1].2 - 3.0 * pair[1].3,
                "sorted by conservative rating"
            );
        }

        // By generation, by label, and by rating, all naming a real network.
        for which in ["2", &roster[0].0.clone(), "best"] {
            let (label, text) = t.export(which, "neat-test").expect("a champion by {which}");
            assert!(roster.iter().any(|(l, ..)| *l == label));
            let (net, generation) = Net::parse(&text).expect("a readable network");
            assert_eq!(net.inputs(), crate::neat::INPUTS);
            assert!((1..=3).contains(&generation), "stamped with its own age");
        }
        // A generation picks that generation, not merely something.
        let (label, text) = t.export("2", "neat-test").expect("generation two");
        assert!(label.starts_with("g002"));
        assert_eq!(Net::parse(&text).expect("readable").1, 2);
        // And asking for what is not there answers nothing rather than
        // something near it, because a champion silently substituted is a
        // rating attached to the wrong player.
        assert!(t.export("99", "neat-test").is_none());
        assert!(t.export("g999-0001", "neat-test").is_none());
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
