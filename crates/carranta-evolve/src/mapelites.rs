//! Quality diversity: an archive of the best player found at each *style* of
//! play, rather than a population all chasing the same number (E-37).
//!
//! The problem this answers is the one neat-12 ran into. A population under
//! plain selection converges: by generation 2500 it held four species with a
//! mean age of 154, and the bodies it produced could no longer beat what had
//! already shipped. Every genome was climbing the same hill, and the hill had
//! a top.
//!
//! MAP-Elites keeps a grid instead of a population. Two behavioural axes cut
//! the space of ways to play into cells, and each cell remembers only the best
//! player that has ever landed in it. A weak-but-strange genome survives
//! because nothing else is strange in the same way, and it survives long
//! enough to be the parent of something that is strong *and* strange. Those
//! are the stepping stones a converged population has thrown away.
//!
//! The two axes follow Pugh, Soros and Stanley: one *aligned* with quality and
//! one *unaligned* with it. The aligned axis spreads the population along
//! something that does correlate with winning, so the archive keeps a ladder
//! rather than a museum. The unaligned axis is the one that does the real
//! work, holding open styles that selection alone would have closed.
//!
//! Nothing here is a fitness signal. Selection inside a cell is still by
//! fitness alone; the descriptors only decide *which* comparison a genome is
//! entered into.

use crate::behaviour::Behaviour;
use crate::neat::NeatGenome;
use carranta_core::rng::{Rng, Stream};

/// Cells along each axis. Twelve squared is 144, close to the population a run
/// of this size already carries, so a full archive costs about what the
/// population cost and the comparison between the two is honest.
pub const CELLS: usize = 12;

/// The aligned axis: cards actually produced per seat per game.
///
/// Economy scale. It correlates with winning, which is the point of an aligned
/// axis: it keeps the archive from filling with equally poor players who
/// merely differ. The bounds are the range observed across neat-12's whole
/// champion catalogue with room at both ends, and a genome outside them is
/// clamped rather than dropped, because an outlier is still somebody's elite.
pub const PRODUCTION_LOW: f64 = 30.0;
pub const PRODUCTION_HIGH: f64 = 90.0;

/// The unaligned axis: what share of a seat's building went into cities rather
/// than roads.
///
/// Densify against expand, which is a real fork in this game with no settled
/// answer: a tall three-city board and a wide nine-road board can both win,
/// and they lose to different opponents. Nothing about the ratio says which is
/// better, which is exactly what an unaligned descriptor has to be.
pub const CITY_SHARE_LOW: f64 = 0.0;
pub const CITY_SHARE_HIGH: f64 = 0.6;

/// Where one genome's play falls on the two axes, before it is put to a cell.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Descriptor {
    /// Aligned: production per seat per game.
    pub production: f64,
    /// Unaligned: cities as a share of cities plus roads.
    pub city_share: f64,
}

impl Descriptor {
    /// Read the two axes off a genome's sampled play.
    ///
    /// A genome that built nothing at all reports a city share of zero rather
    /// than a division by zero. That is the honest reading: it did not densify.
    pub fn of(b: &Behaviour) -> Descriptor {
        let built = b.cities_built + b.roads_built;
        Descriptor {
            production: b.production,
            city_share: if built > 0.0 {
                b.cities_built / built
            } else {
                0.0
            },
        }
    }

    /// The cell this descriptor belongs to, clamped to the grid.
    pub fn cell(&self) -> (usize, usize) {
        (
            bucket(self.production, PRODUCTION_LOW, PRODUCTION_HIGH),
            bucket(self.city_share, CITY_SHARE_LOW, CITY_SHARE_HIGH),
        )
    }
}

/// One axis of a descriptor as a cell index, clamped at both ends.
fn bucket(v: f64, low: f64, high: f64) -> usize {
    if !v.is_finite() {
        return 0;
    }
    let t = (v - low) / (high - low);
    let i = (t * CELLS as f64).floor();
    if i < 0.0 {
        0
    } else if i >= CELLS as f64 {
        CELLS - 1
    } else {
        i as usize
    }
}

/// The best genome found so far at one style of play.
#[derive(Clone, Debug)]
pub struct Elite {
    pub genome: NeatGenome,
    /// Lower is better, the same sense as everywhere else in this crate.
    pub fitness: f64,
    pub descriptor: Descriptor,
    /// The generation this elite was placed, for reading the archive's age.
    pub generation: u32,
    /// Seeded from outside the generation loop rather than measured by it.
    ///
    /// This distinction is load-bearing. A seeded champion's fitness comes
    /// from a handful of games against the anchor; a bred genome's comes from
    /// the whole evaluation, against a field of a hundred and fifty. The
    /// second is a far harder test, so the two numbers are not comparable and
    /// treating them as one scale hands every cell to a seed for ever: the
    /// archive fills, coverage looks excellent, and nothing can ever improve.
    /// A provisional elite is a parent and a placeholder, and yields its cell
    /// to the first genome that has actually been measured.
    pub provisional: bool,
}

/// The grid: one elite per cell, most cells empty at the start.
#[derive(Clone, Debug, Default)]
pub struct Archive {
    cells: Vec<Option<Elite>>,
}

/// What one insertion did, which is what a run wants to report.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placed {
    /// Landed in a cell nobody had reached: the archive grew.
    Discovered,
    /// Beat the elite that was there: the archive got better.
    Improved,
    /// Lost to the elite that was there, and was dropped.
    Rejected,
}

impl Archive {
    pub fn new() -> Archive {
        Archive {
            cells: vec![None; CELLS * CELLS],
        }
    }

    /// Offer a genome to the archive.
    ///
    /// It takes the cell if the cell is empty, or if it is better than what is
    /// there. Ties are rejected: an incumbent that has survived is worth more
    /// than a newcomer that merely matched it, and letting ties through would
    /// churn the archive without improving it.
    pub fn insert(
        &mut self,
        genome: NeatGenome,
        fitness: f64,
        descriptor: Descriptor,
        generation: u32,
    ) -> Placed {
        self.offer(genome, fitness, descriptor, generation, false)
    }

    /// Offer a genome the loop has not measured: a seed.
    ///
    /// It takes an empty cell and never displaces anything, because its
    /// fitness was not produced by the same experiment and cannot be compared
    /// with one that was.
    pub fn seed(
        &mut self,
        genome: NeatGenome,
        fitness: f64,
        descriptor: Descriptor,
        generation: u32,
    ) -> Placed {
        self.offer(genome, fitness, descriptor, generation, true)
    }

    fn offer(
        &mut self,
        genome: NeatGenome,
        fitness: f64,
        descriptor: Descriptor,
        generation: u32,
        provisional: bool,
    ) -> Placed {
        let (a, b) = descriptor.cell();
        let slot = &mut self.cells[a * CELLS + b];
        let beaten = match slot.as_ref() {
            None => true,
            // A measured genome always takes a cell from a seed, whatever the
            // two numbers say, because they do not mean the same thing.
            Some(held) if held.provisional && !provisional => true,
            // A seed never displaces anything, measured or seeded.
            Some(_) if provisional => false,
            Some(held) => held.fitness > fitness,
        };
        if !beaten {
            return Placed::Rejected;
        }
        let was_empty = slot.is_none();
        *slot = Some(Elite {
            genome,
            fitness,
            descriptor,
            generation,
            provisional,
        });
        if was_empty {
            Placed::Discovered
        } else {
            Placed::Improved
        }
    }

    /// How many cells hold anything at all, seeds included: the reach of the
    /// breeding pool, since a seed is drawn as a parent like any other.
    pub fn filled(&self) -> usize {
        self.cells.iter().filter(|c| c.is_some()).count()
    }

    /// How many cells hold a genome this run measured itself.
    ///
    /// The honest coverage number. `filled` counts seeds too, and on a freshly
    /// seeded archive that is most of them, so reading `filled` alone would
    /// call a run successful before it had done anything.
    pub fn measured(&self) -> usize {
        self.iter().filter(|e| !e.provisional).count()
    }

    pub fn is_empty(&self) -> bool {
        self.filled() == 0
    }

    /// The best *measured* elite, and the cell holding it.
    ///
    /// Seeds are excluded. Their fitness came from a different experiment, so
    /// including them would report a champion the run never produced and, on
    /// a freshly seeded archive, would report one every time.
    pub fn best(&self) -> Option<(&Elite, usize, usize)> {
        self.cells
            .iter()
            .enumerate()
            .filter_map(|(i, c)| c.as_ref().map(|e| (e, i / CELLS, i % CELLS)))
            .filter(|(e, _, _)| !e.provisional)
            .min_by(|x, y| x.0.fitness.total_cmp(&y.0.fitness))
    }

    /// The mean fitness across filled cells: the archive's quality as a whole,
    /// which is the number that separates a wide archive of poor players from
    /// a wide archive of good ones.
    pub fn mean_fitness(&self) -> f64 {
        let held: Vec<f64> = self
            .iter()
            .filter(|e| !e.provisional)
            .map(|e| e.fitness)
            .collect();
        if held.is_empty() {
            return 0.0;
        }
        held.iter().sum::<f64>() / held.len() as f64
    }

    /// Every elite, in cell order.
    pub fn iter(&self) -> impl Iterator<Item = &Elite> {
        self.cells.iter().filter_map(|c| c.as_ref())
    }

    /// Every cell with its coordinates, empty ones included. For checkpointing
    /// and for drawing the grid.
    pub fn cells(&self) -> impl Iterator<Item = (usize, usize, Option<&Elite>)> {
        self.cells
            .iter()
            .enumerate()
            .map(|(i, c)| (i / CELLS, i % CELLS, c.as_ref()))
    }

    /// Put an elite back at a named cell, for restoring a checkpoint.
    ///
    /// Unlike `insert` this does not consult the descriptor, because a
    /// restored archive must come back exactly as it was written even if the
    /// axis bounds have since been edited. A run that changes its bounds
    /// should rebuild its archive deliberately, not have it quietly reshuffle
    /// on the next resume.
    pub fn restore_cell(&mut self, a: usize, b: usize, elite: Elite) {
        if a < CELLS && b < CELLS {
            self.cells[a * CELLS + b] = Some(elite);
        }
    }

    /// A uniformly drawn parent from the filled cells.
    ///
    /// Uniform over *cells*, not over fitness, which is the whole mechanism: a
    /// lone genome in a strange cell is drawn as often as the best player in
    /// the archive, so strange survives long enough to become good.
    pub fn pick(&self, rng: &mut Rng) -> Option<&Elite> {
        let n = self.filled();
        if n == 0 {
            return None;
        }
        let k = rng.below(Stream::Board, n as u32) as usize;
        self.iter().nth(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behaviour::Behaviour;

    fn descriptor(production: f64, city_share: f64) -> Descriptor {
        Descriptor {
            production,
            city_share,
        }
    }

    #[test]
    fn the_axes_are_read_off_play_and_a_genome_that_built_nothing_does_not_divide_by_zero() {
        let b = Behaviour {
            production: 55.0,
            cities_built: 3.0,
            roads_built: 9.0,
            ..Behaviour::default()
        };
        let d = Descriptor::of(&b);
        assert_eq!(d.production, 55.0);
        assert!((d.city_share - 0.25).abs() < 1e-9, "{}", d.city_share);

        let nothing = Descriptor::of(&Behaviour::default());
        assert_eq!(nothing.city_share, 0.0, "no builds is not a division");
        assert!(nothing.cell().0 < CELLS && nothing.cell().1 < CELLS);
    }

    #[test]
    fn a_descriptor_outside_the_bounds_is_clamped_rather_than_dropped() {
        // An outlier is still somebody's elite, and a run that silently threw
        // away its extremes would be selecting on the axis it claims not to.
        let low = descriptor(-100.0, -5.0).cell();
        let high = descriptor(1e9, 99.0).cell();
        assert_eq!(low, (0, 0));
        assert_eq!(high, (CELLS - 1, CELLS - 1));
        let nan = descriptor(f64::NAN, f64::INFINITY).cell();
        assert!(nan.0 < CELLS && nan.1 < CELLS, "a NaN must not panic");
    }

    #[test]
    fn distinct_styles_land_in_distinct_cells() {
        // The point of the grid: two players with the same economy but
        // opposite building habits must not compete with each other.
        let tall = descriptor(60.0, 0.5).cell();
        let wide = descriptor(60.0, 0.05).cell();
        assert_eq!(tall.0, wide.0, "same economy, same aligned bucket");
        assert_ne!(tall.1, wide.1, "opposite habits must not share a cell");
    }

    #[test]
    fn a_cell_keeps_the_better_player_and_reports_what_happened() {
        let mut archive = Archive::new();
        let g = NeatGenome::default();
        let d = descriptor(60.0, 0.3);

        assert_eq!(archive.insert(g.clone(), -3.0, d, 1), Placed::Discovered);
        assert_eq!(archive.filled(), 1);

        // Lower is better everywhere in this crate.
        assert_eq!(archive.insert(g.clone(), -2.0, d, 2), Placed::Rejected);
        assert_eq!(archive.best().unwrap().0.fitness, -3.0);

        assert_eq!(archive.insert(g.clone(), -4.0, d, 3), Placed::Improved);
        assert_eq!(archive.best().unwrap().0.fitness, -4.0);
        assert_eq!(archive.filled(), 1, "improving is not discovering");

        // A tie leaves the incumbent, which has survived and is worth more.
        assert_eq!(archive.insert(g, -4.0, d, 4), Placed::Rejected);
        assert_eq!(archive.best().unwrap().0.generation, 3);
    }

    #[test]
    fn a_weak_genome_in_an_empty_cell_survives_a_strong_one_elsewhere() {
        // The whole reason the archive exists. Under plain selection the weak
        // genome is gone; here it holds its cell and can be drawn as a parent.
        let mut archive = Archive::new();
        let g = NeatGenome::default();
        archive.insert(g.clone(), -5.0, descriptor(60.0, 0.3), 1);
        archive.insert(g, -1.0, descriptor(35.0, 0.05), 1);
        assert_eq!(archive.filled(), 2);
        assert!(
            archive.iter().any(|e| e.fitness == -1.0),
            "the weak one was culled, which is the failure this guards"
        );
    }

    #[test]
    fn parents_are_drawn_by_cell_and_not_by_fitness() {
        // Uniform over cells is the mechanism: over many draws the lone
        // strange genome must come up about as often as the good one, or the
        // archive is just a population with extra steps.
        let mut archive = Archive::new();
        let g = NeatGenome::default();
        archive.insert(g.clone(), -9.0, descriptor(80.0, 0.5), 1);
        archive.insert(g, -1.0, descriptor(35.0, 0.05), 1);

        let mut rng = Rng::new(7);
        let mut weak = 0;
        for _ in 0..400 {
            if archive.pick(&mut rng).unwrap().fitness == -1.0 {
                weak += 1;
            }
        }
        assert!(
            (120..=280).contains(&weak),
            "drawn {weak} of 400, which is not uniform over two cells"
        );
    }

    #[test]
    fn a_seed_yields_its_cell_to_anything_the_run_actually_measured() {
        // The bug this guards, found live: seeded fitnesses come from a few
        // games against the anchor and bred ones from the whole evaluation
        // against a field of a hundred and fifty. The seeded number looks far
        // better because the test was far easier. Compared as one scale, every
        // seeded cell is locked for ever: coverage looks excellent and nothing
        // can improve. The run had 125 of 144 cells and a "best" of -7.0 that
        // no generation had produced.
        let mut archive = Archive::new();
        let g = NeatGenome::default();
        let d = descriptor(60.0, 0.3);

        assert_eq!(archive.seed(g.clone(), -7.0, d, 758), Placed::Discovered);
        assert_eq!(archive.measured(), 0, "a seed is not a measurement");
        assert_eq!(archive.filled(), 1, "but it is a parent, so it is there");
        assert!(
            archive.best().is_none(),
            "reporting a seed as the best would name a champion the run \
             never produced"
        );

        // Far worse on paper, and it still takes the cell.
        assert_eq!(archive.insert(g.clone(), -3.0, d, 2559), Placed::Improved);
        assert_eq!(archive.measured(), 1);
        assert_eq!(archive.best().unwrap().0.fitness, -3.0);

        // And from then on the cell is an ordinary contest again.
        assert_eq!(archive.insert(g.clone(), -2.0, d, 2560), Placed::Rejected);
        assert_eq!(archive.insert(g, -4.0, d, 2561), Placed::Improved);
        assert_eq!(archive.best().unwrap().0.fitness, -4.0);
    }

    #[test]
    fn a_seed_never_displaces_and_never_moves_the_mean() {
        let mut archive = Archive::new();
        let g = NeatGenome::default();
        let d = descriptor(60.0, 0.3);
        archive.insert(g.clone(), -3.0, d, 10);
        // Even a spectacular-looking seed leaves a measured elite alone.
        assert_eq!(archive.seed(g.clone(), -9.0, d, 11), Placed::Rejected);
        assert_eq!(archive.best().unwrap().0.fitness, -3.0);

        // A seed in its own cell is a parent but not part of the quality
        // reading, or the mean would flatter the run by its seeding alone.
        archive.seed(g, -9.0, descriptor(35.0, 0.05), 12);
        assert_eq!(archive.filled(), 2);
        assert_eq!(archive.measured(), 1);
        assert_eq!(archive.mean_fitness(), -3.0);
    }

    #[test]
    fn seeds_are_still_drawn_as_parents() {
        // They must be: supplying parents is the entire reason to seed.
        let mut archive = Archive::new();
        let g = NeatGenome::default();
        archive.seed(g.clone(), -9.0, descriptor(80.0, 0.5), 1);
        archive.insert(g, -3.0, descriptor(35.0, 0.05), 2);
        let mut rng = Rng::new(4);
        let mut seeds = 0;
        for _ in 0..400 {
            if archive.pick(&mut rng).unwrap().provisional {
                seeds += 1;
            }
        }
        assert!(
            (120..=280).contains(&seeds),
            "seeds drawn {seeds} of 400, so they are not breeding"
        );
    }

    #[test]
    fn an_empty_archive_answers_rather_than_panicking() {
        let archive = Archive::new();
        let mut rng = Rng::new(1);
        assert!(archive.is_empty());
        assert!(archive.pick(&mut rng).is_none());
        assert!(archive.best().is_none());
        assert_eq!(archive.mean_fitness(), 0.0);
    }

    #[test]
    fn a_restored_cell_comes_back_where_it_was_written() {
        let mut archive = Archive::new();
        archive.restore_cell(
            3,
            4,
            Elite {
                genome: NeatGenome::default(),
                fitness: -2.5,
                descriptor: descriptor(60.0, 0.3),
                generation: 11,
                provisional: false,
            },
        );
        let (elite, a, b) = archive.best().unwrap();
        assert_eq!((a, b), (3, 4), "restore must not consult the descriptor");
        assert_eq!(elite.generation, 11);
        // Out of range is ignored rather than panicking, so a checkpoint
        // written by a build with a different grid cannot take a run down.
        archive.restore_cell(CELLS, 0, elite.clone());
        assert_eq!(archive.filled(), 1);
    }
}
