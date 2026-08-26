//! What everyone at the table can work out about everyone's hand (E-33).
//!
//! Nearly every card in this game moves in public. Production is the dice and
//! the board; trades name both sides; discards name the card; development
//! purchases pay a known cost; Monopoly and Invention announce themselves.
//! The single exception is the robber's steal, where one card moves and only
//! the thief and the victim see which. A person who pays attention therefore
//! knows opponents' hands almost exactly, and the players here should not be
//! dimmer than a person who pays attention.
//!
//! This is that attention, kept as expected counts: for each seat, the
//! expected number of each resource held, from the public record alone. It is
//! the *outside observer's* belief, deliberately: the thief and the victim of
//! a steal each know a little more than the table does, and tracking one
//! shared belief means nothing here ever peeks at a hidden card. The engine
//! maintains it inside [`apply`](crate::state::State::apply), so a replayed
//! game rebuilds it move for move, and a search probing a candidate action
//! carries its beliefs forward through the probe.
//!
//! Counts are fixed-point, [`SCALE`] to the card, because the state promises
//! `Eq` and floats cannot. The bookkeeping keeps one invariant exactly: each
//! seat's row sums to its true hand size times `SCALE`. Hand sizes are
//! public, so the invariant leaks nothing, and it is what makes the numbers
//! read as a hand rather than a drift of estimates.

use crate::state::MAX_PLAYERS;

/// Fixed-point units per card.
pub const SCALE: u32 = 1024;

/// The table's shared belief about every hand, in `SCALE`ths of a card.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct Counting {
    pub expected: [[u32; 5]; MAX_PLAYERS],
}

impl Counting {
    /// Fold one seat's public hand change in: `delta[r]` cards gained or
    /// lost, as the whole table saw happen.
    ///
    /// Losses first, then gains, so a spend that reveals a card this record
    /// had elsewhere corrects against the old belief rather than against
    /// cards arriving in the same breath. A loss larger than the cell is
    /// that revelation: the seat spent a card the record believed was
    /// something else, so the shortfall comes out of the other cells,
    /// largest first, which is where the misplaced belief most likely sat.
    pub fn public(&mut self, seat: usize, delta: [i32; 5]) {
        let row = &mut self.expected[seat];
        for r in 0..5 {
            if delta[r] < 0 {
                let mut owed = (-delta[r]) as u32 * SCALE;
                let take = owed.min(row[r]);
                row[r] -= take;
                owed -= take;
                // The revelation: what this cell could not cover was
                // believed elsewhere in the row.
                while owed > 0 {
                    let big = (0..5).max_by_key(|&k| row[k]).unwrap_or(0);
                    if row[big] == 0 {
                        break; // an inconsistent caller; hold at empty
                    }
                    let take = owed.min(row[big]);
                    row[big] -= take;
                    owed -= take;
                }
            }
        }
        for r in 0..5 {
            if delta[r] > 0 {
                row[r] += delta[r] as u32 * SCALE;
            }
        }
    }

    /// Fold a steal in: one card moved from `victim` to `thief`, identity
    /// unseen, drawn uniformly from a hand of `size` cards.
    ///
    /// Expectation moves exactly as the draw does: each cell gives up its
    /// share in proportion to what it holds. Integer division truncates, so
    /// the largest cell carries the rounding remainder and exactly one
    /// card's worth of belief changes hands, keeping both rows' sums true.
    pub fn steal(&mut self, thief: usize, victim: usize, size: u32) {
        debug_assert!(size > 0, "an empty hand cannot be stolen from");
        if size == 0 {
            return;
        }
        let mut moved = [0u32; 5];
        let mut total = 0u32;
        for r in 0..5 {
            moved[r] = self.expected[victim][r] / size;
            total += moved[r];
        }
        // The truncation remainder rides on the largest cell.
        if total < SCALE {
            let big = (0..5)
                .max_by_key(|&k| self.expected[victim][k])
                .unwrap_or(0);
            moved[big] += SCALE - total;
        }
        for r in 0..5 {
            let m = moved[r].min(self.expected[victim][r]);
            self.expected[victim][r] -= m;
            self.expected[thief][r] += m;
        }
    }

    /// One seat's expected count of one resource, in cards.
    pub fn cards(&self, seat: usize, resource: usize) -> f64 {
        self.expected[seat][resource] as f64 / SCALE as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_moves_are_certain_and_steals_are_spread() {
        let mut c = Counting::default();
        // Seat 1 produces two brick and a wheat in public.
        c.public(1, [2, 0, 0, 1, 0]);
        assert_eq!(c.expected[1], [2 * SCALE, 0, 0, SCALE, 0]);

        // Seat 0 steals one of the three: belief moves in proportion.
        c.steal(0, 1, 3);
        let third = SCALE / 3;
        assert_eq!(c.expected[1][3], SCALE - third, "a third of the wheat left");
        assert!(
            c.expected[0][0] > c.expected[0][3],
            "the thief more likely took brick, having seen nothing"
        );
        // One whole card moved, no more, no less.
        assert_eq!(c.expected[0].iter().sum::<u32>(), SCALE);
        assert_eq!(c.expected[1].iter().sum::<u32>(), 2 * SCALE);
    }

    #[test]
    fn a_spend_the_record_did_not_expect_corrects_the_record() {
        let mut c = Counting::default();
        // Two cards believed to be brick and wood.
        c.public(2, [1, 1, 0, 0, 0]);
        // The seat spends an ore: it never had the phantom brick or wood in
        // that quantity. The row still sums to one card.
        c.public(2, [0, 0, 0, 0, -1]);
        assert_eq!(
            c.expected[2].iter().sum::<u32>(),
            SCALE,
            "one card remains and the row says so"
        );
        assert_eq!(c.expected[2][4], 0, "no ore is left to believe in");
    }
}
