//! Expected versus actual production (§10.2).
//!
//! The analysis that separates luck from play, and the reason the log stores
//! resolved outcomes: replaying a game gives the board *at every roll*, so the
//! expectation is computed against what the player actually owned at the time
//! rather than against an average.
//!
//! **The expectation is exact, not simulated.** Production on a single roll is
//! a deterministic function of the roll, so per-roll production has a known
//! distribution over 11 outcomes. Rolls are independent, so expectations and
//! variances add, even though buildings change during the game, since each
//! roll simply contributes its own term. That gives an exact z-score where an
//! estimate would otherwise be needed.
//!
//! **A raw expected-vs-actual gap mixes four causes**, which mean completely
//! different things:
//!
//! ```text
//! Actual = E_raw − RobberCost − SupplyDenial + DiceLuck
//! ```
//!
//! `DiceLuck` is chance. `RobberCost` is *other players choosing to target
//! you*, a social outcome, not a random one. `SupplyDenial` is a rules
//! artefact (R-5.6). Reported as one number they tell a player nothing about
//! which of the three happened to them.

use carranta_core::state::{MAX_PLAYERS, State};
use carranta_core::topology::{HEX_COUNT, hex_vertices};
use carranta_core::{Action, Resolved};
use carranta_record::{Log, Payload, ReplayError};

use crate::dice::{OUTCOMES, REFERENCE};

/// Cards owed per roll outcome, per seat, per resource.
type YieldTable = [[[u32; 5]; MAX_PLAYERS]; OUTCOMES];

/// What each seat is owed on every possible roll, given the board as it
/// stands.
///
/// With `respect_robber`, the blocked hex pays nothing (R-5.8); without, it
/// pays as though the robber were not there. The difference between the two
/// is precisely `RobberCost`.
fn yields(state: &State, respect_robber: bool) -> YieldTable {
    let mut table: YieldTable = [[[0; 5]; MAX_PLAYERS]; OUTCOMES];
    for h in 0..HEX_COUNT {
        let n = state.number[h];
        if !(2..=12).contains(&n) {
            continue; // the desert carries no disc
        }
        if respect_robber && h as u8 == state.robber {
            continue;
        }
        let Some(res) = state.terrain[h].yields() else {
            continue;
        };
        let corners = hex_vertices(h as u8);
        // Indexed by seat throughout: the seat number is the thing being
        // written, into a table this loop does not iterate.
        #[allow(clippy::needless_range_loop)]
        for p in 0..state.players as usize {
            // A settlement earns one card, a city two (R-5.4, R-5.5).
            let cards = (state.settlements[p] & corners).count_ones()
                + 2 * (state.cities[p] & corners).count_ones();
            table[n as usize - 2][p][res as usize] += cards;
        }
    }
    table
}

/// Cumulative production at one point in a game, for the chart §10.2 asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// How many rolls have happened.
    pub roll: u32,
    /// Cumulative expected cards, robber respected, summed over resources.
    pub expected: [f64; MAX_PLAYERS],
    /// Cumulative cards actually received.
    pub actual: [u32; MAX_PLAYERS],
}

/// Production over one game, per seat and per resource.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub players: u8,
    pub rolls: u32,
    /// Expected production ignoring the robber and the supply.
    pub e_raw: [[f64; 5]; MAX_PLAYERS],
    /// Expected production given where the robber actually sat.
    pub e_robber: [[f64; 5]; MAX_PLAYERS],
    /// Variance of the robber-respecting expectation. Exact, summed per roll.
    pub variance: [[f64; 5]; MAX_PLAYERS],
    /// Cards the rolls owed, given the real robber positions, before the
    /// supply was checked.
    pub ideal: [[u32; 5]; MAX_PLAYERS],
    /// Cards actually received.
    pub actual: [[u32; 5]; MAX_PLAYERS],
    /// Cumulative curve, one entry per roll.
    pub timeline: Vec<Point>,
}

/// A seat's production broken into its four causes, in cards.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Decomposition {
    /// Expected production ignoring robber and supply.
    pub e_raw: f64,
    /// Expected production lost to the robber sitting on your hexes, an
    /// opponent's choice, not chance.
    pub robber_cost: f64,
    /// Production owed but not paid because a stack ran out (R-5.6).
    pub supply_denial: f64,
    /// What the dice did, given the real robber positions. The only genuinely
    /// random term.
    pub dice_luck: f64,
    /// What arrived.
    pub actual: f64,
    /// `dice_luck` in standard deviations. Exact, per the note above.
    pub luck_z: f64,
}

impl Report {
    /// Break one seat's whole-game production into its four causes.
    pub fn decompose(&self, seat: usize) -> Decomposition {
        self.decompose_resource(seat, None)
    }

    /// The same, for one resource, which is what answers "was I starved of
    /// ore specifically".
    pub fn decompose_resource(&self, seat: usize, resource: Option<usize>) -> Decomposition {
        let pick = |xs: &[f64; 5]| -> f64 {
            match resource {
                Some(r) => xs[r],
                None => xs.iter().sum(),
            }
        };
        let pick_u = |xs: &[u32; 5]| -> f64 {
            match resource {
                Some(r) => xs[r] as f64,
                None => xs.iter().map(|&x| x as f64).sum(),
            }
        };

        let e_raw = pick(&self.e_raw[seat]);
        let e_robber = pick(&self.e_robber[seat]);
        let ideal = pick_u(&self.ideal[seat]);
        let actual = pick_u(&self.actual[seat]);
        let sd = pick(&self.variance[seat]).sqrt();
        let dice_luck = ideal - e_robber;

        Decomposition {
            e_raw,
            robber_cost: e_raw - e_robber,
            supply_denial: ideal - actual,
            dice_luck,
            actual,
            luck_z: if sd > 0.0 { dice_luck / sd } else { 0.0 },
        }
    }

    /// Total cards produced by every seat, for §10.4's luck adjustment.
    pub fn total_actual(&self, seat: usize) -> u32 {
        self.actual[seat].iter().sum()
    }
}

impl Decomposition {
    /// The §10.2 identity, as a residual. Zero up to floating point.
    ///
    /// Not decoration: it is what proves the four terms are a decomposition
    /// and not four separately-computed numbers that happen to be near each
    /// other.
    pub fn residual(&self) -> f64 {
        self.e_raw - self.robber_cost - self.supply_denial + self.dice_luck - self.actual
    }
}

/// Compute production for a recorded game.
pub fn analyse(log: &Log) -> Result<Report, ReplayError> {
    let mut state = *log.created.opening;
    let mut r = Report {
        players: state.players,
        rolls: 0,
        e_raw: [[0.0; 5]; MAX_PLAYERS],
        e_robber: [[0.0; 5]; MAX_PLAYERS],
        variance: [[0.0; 5]; MAX_PLAYERS],
        ideal: [[0; 5]; MAX_PLAYERS],
        actual: [[0; 5]; MAX_PLAYERS],
        timeline: Vec::new(),
    };
    let seats = state.players as usize;

    for event in &log.events {
        let Payload::Decision { action, resolved } = event.payload else {
            continue;
        };

        // Only a roll produces, and only from the board as it stands *before*
        // the roll, so the tables are built here, not after applying.
        let rolling = matches!(action, Action::Roll);
        let (blocked, open, before) = if rolling {
            (yields(&state, true), yields(&state, false), state.hand)
        } else {
            Default::default()
        };

        let got = state
            .apply_scripted(action, resolved)
            .map_err(|why| ReplayError::Illegal {
                seq: event.seq,
                why,
            })?;
        if got != resolved {
            return Err(ReplayError::Diverged { seq: event.seq });
        }
        if !rolling {
            continue;
        }
        let Resolved::Dice(a, b) = resolved else {
            return Err(ReplayError::Diverged { seq: event.seq });
        };

        r.rolls += 1;
        for p in 0..seats {
            for res in 0..5 {
                // Exact per-roll moments over the 11 outcomes.
                let (mut mean, mut second, mut mean_open) = (0.0, 0.0, 0.0);
                for n in 0..OUTCOMES {
                    let y = blocked[n][p][res] as f64;
                    mean += REFERENCE[n] * y;
                    second += REFERENCE[n] * y * y;
                    mean_open += REFERENCE[n] * open[n][p][res] as f64;
                }
                r.e_robber[p][res] += mean;
                r.e_raw[p][res] += mean_open;
                r.variance[p][res] += second - mean * mean;

                // What the roll owed, and what arrived. A roll only ever adds
                // cards, so the hand delta is the payment.
                r.ideal[p][res] += blocked[(a + b) as usize - 2][p][res];
                r.actual[p][res] += (state.hand[p][res] - before[p][res]) as u32;
            }
        }

        r.timeline.push(Point {
            roll: r.rolls,
            expected: core::array::from_fn(|p| r.e_robber[p].iter().sum()),
            actual: core::array::from_fn(|p| r.actual[p].iter().sum()),
        });
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::self_play;
    use carranta_core::state::TradeMode;

    #[test]
    fn the_decomposition_is_an_identity() {
        for seed in 0..30 {
            let log = self_play(seed, TradeMode::Full);
            let r = analyse(&log).expect("analyse");
            for seat in 0..4 {
                let d = r.decompose(seat);
                assert!(
                    d.residual().abs() < 1e-9,
                    "seed {seed} seat {seat}: residual {}",
                    d.residual()
                );
                for res in 0..5 {
                    let d = r.decompose_resource(seat, Some(res));
                    assert!(
                        d.residual().abs() < 1e-9,
                        "seed {seed} seat {seat} res {res}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_robber_only_ever_costs() {
        for seed in 0..20 {
            let r = analyse(&self_play(seed, TradeMode::Disabled)).unwrap();
            for seat in 0..4 {
                let d = r.decompose(seat);
                assert!(d.robber_cost >= -1e-9, "seed {seed}: robber paid a bonus");
                assert!(d.supply_denial >= -1e-9, "seed {seed}: supply overpaid");
                assert!(d.e_raw >= d.e_raw - d.robber_cost - 1e-9);
            }
        }
    }

    #[test]
    fn luck_is_centred_across_a_corpus() {
        // Dice luck is the only random term, so over many games and seats its
        // z-score should sit near zero with a spread near one. A systematic
        // offset would mean the expectation is wrong, not that anyone was
        // lucky, which is the failure this catches.
        let mut zs = Vec::new();
        for seed in 0..120 {
            let r = analyse(&self_play(seed, TradeMode::Disabled)).unwrap();
            for seat in 0..4 {
                let d = r.decompose(seat);
                if d.luck_z != 0.0 {
                    zs.push(d.luck_z);
                }
            }
        }
        let (mean, sd) = crate::stats::mean_sd(&zs);
        assert!(zs.len() > 300, "only {} usable seats", zs.len());
        assert!(mean.abs() < 0.2, "luck z-scores are off-centre: {mean:.3}");
        assert!(
            (0.6..1.6).contains(&sd),
            "luck z-score spread is {sd:.3}, expected near 1"
        );
    }

    #[test]
    fn expectation_tracks_actual_production_over_a_game() {
        // Not a tautology: expectation is computed from the board, actual from
        // the hands the engine paid out. Over a whole game they should be
        // close, and the timeline should be monotone in both.
        let r = analyse(&self_play(3, TradeMode::Disabled)).unwrap();
        assert!(r.rolls > 20);
        assert_eq!(r.timeline.len(), r.rolls as usize);
        for w in r.timeline.windows(2) {
            for p in 0..4 {
                assert!(w[1].expected[p] >= w[0].expected[p]);
                assert!(w[1].actual[p] >= w[0].actual[p]);
            }
        }
        let total_expected: f64 = (0..4).map(|p| r.decompose(p).e_raw).sum();
        let total_actual: f64 = (0..4).map(|p| r.decompose(p).actual).sum();
        assert!(
            (total_actual - total_expected).abs() < total_expected * 0.35,
            "expected {total_expected:.0}, actual {total_actual:.0}"
        );
    }

    #[test]
    fn a_blocked_hex_pays_nothing() {
        let log = self_play(1, TradeMode::Disabled);
        let mut state = *log.created.opening;
        state.settlements[0] = carranta_core::topology::hex_vertices(state.robber);
        let blocked = yields(&state, true);
        let open = yields(&state, false);
        let n = state.number[state.robber as usize];
        if (2..=12).contains(&n) {
            let r = state.terrain[state.robber as usize].yields().unwrap() as usize;
            assert!(open[n as usize - 2][0][r] > blocked[n as usize - 2][0][r]);
        }
    }
}
