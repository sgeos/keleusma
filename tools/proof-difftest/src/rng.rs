//! A seeded generator of pseudo-random numbers, SplitMix64.
//!
//! The harness carries its own generator rather than a dependency so that a seed names the
//! same cases on every machine and every toolchain. It is not cryptographic and needs not be.

/// The SplitMix64 state.
pub struct Rng(u64);

impl Rng {
    /// A generator started from `seed`.
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value in `0..n`. The modulo bias is irrelevant for test generation.
    ///
    /// # Panics
    /// When `n` is zero, which is a caller defect.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "Rng::below requires a positive bound");
        self.next_u64() % n
    }

    /// True with probability `percent` in one hundred.
    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// An index chosen with the given nonnegative weights, of which at least one is positive.
    ///
    /// # Panics
    /// When every weight is zero, which is a caller defect.
    pub fn weighted(&mut self, weights: &[u64]) -> usize {
        let total: u64 = weights.iter().sum();
        assert!(total > 0, "Rng::weighted requires a positive weight");
        let mut pick = self.below(total);
        for (i, &w) in weights.iter().enumerate() {
            if pick < w {
                return i;
            }
            pick -= w;
        }
        unreachable!("the pick is below the total of the weights")
    }
}
