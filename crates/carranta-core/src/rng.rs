//! Deterministic randomness, split into independent streams.
//!
//! §6.4 requires four sources that can be varied independently: dice, the
//! development deck shuffle, the random steal, and board generation. Holding
//! them in one generator makes a change to any of them perturb all the others,
//! which destroys paired evaluation and makes debugging miserable — vary the
//! dice while holding the board fixed and you want *only* the dice to move.
//!
//! SplitMix64: tiny, fast, and good enough for a board game. It is not
//! cryptographic and must not be used where unpredictability matters.

/// The independent random streams of a game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Board = 0,
    DevDeck = 1,
    Dice = 2,
    Steal = 3,
}

/// A seeded set of independent streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    state: [u64; 4],
}

impl Rng {
    /// Derive the four streams from one game seed.
    pub const fn new(seed: u64) -> Self {
        // Distinct odd offsets keep the streams from aligning.
        Rng {
            state: [
                seed ^ 0x9E37_79B9_7F4A_7C15,
                seed ^ 0xBF58_476D_1CE4_E5B9,
                seed ^ 0x94D0_49BB_1331_11EB,
                seed ^ 0x2545_F491_4F6C_DD1D,
            ],
        }
    }

    #[inline]
    pub fn next_u64(&mut self, stream: Stream) -> u64 {
        let s = &mut self.state[stream as usize];
        *s = s.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *s;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`. Unbiased by rejection, so it consumes a variable
    /// number of draws — which is fine, since streams are independent.
    #[inline]
    pub fn below(&mut self, stream: Stream, n: u32) -> u32 {
        debug_assert!(n > 0);
        let n = n as u64;
        let limit = u64::MAX - (u64::MAX % n);
        loop {
            let x = self.next_u64(stream);
            if x < limit {
                return (x % n) as u32;
            }
        }
    }

    /// One die.
    #[inline]
    pub fn die(&mut self) -> u8 {
        self.below(Stream::Dice, 6) as u8 + 1
    }

    /// Fisher–Yates, in place.
    pub fn shuffle<T>(&mut self, stream: Stream, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(stream, i as u32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_are_independent() {
        // Drawing from one stream must not disturb another: this is what lets
        // a run hold the board fixed while varying the dice.
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..100 {
            a.next_u64(Stream::Dice);
        }
        assert_eq!(a.next_u64(Stream::Board), b.next_u64(Stream::Board));
    }

    #[test]
    fn same_seed_gives_the_same_game() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.die(), b.die());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        let da: Vec<u8> = (0..64).map(|_| a.die()).collect();
        let db: Vec<u8> = (0..64).map(|_| b.die()).collect();
        assert_ne!(da, db);
    }

    #[test]
    fn dice_are_in_range_and_roughly_uniform() {
        let mut r = Rng::new(0xC0FFEE);
        let mut counts = [0u32; 7];
        const N: u32 = 600_000;
        for _ in 0..N {
            let d = r.die();
            assert!((1..=6).contains(&d));
            counts[d as usize] += 1;
        }
        // Each face should land near N/6; allow a generous 2% band.
        let expected = N / 6;
        for (face, &c) in counts.iter().enumerate().skip(1) {
            let delta = c.abs_diff(expected);
            assert!(delta * 50 < expected, "face {face}: {c} vs {expected}");
        }
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut r = Rng::new(9);
        let mut items: Vec<u8> = (0..25).collect();
        r.shuffle(Stream::DevDeck, &mut items);
        let mut sorted = items.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..25).collect::<Vec<u8>>());
        assert_ne!(items, sorted, "a 25-item shuffle should reorder something");
    }
}
