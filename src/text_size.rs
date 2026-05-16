//! Abstract interpretation lattice for tracking text-size bounds
//! through Keleusma bytecode.
//!
//! The WCMU pass needs to bound the worst-case bytes that
//! text-producing opcodes allocate from the arena's top region during
//! a single Stream-to-Reset iteration. `Op::Add` on text operands and
//! the bundled `to_string`, `concat`, and `slice` natives all
//! allocate a `KString` whose length depends on the operand lengths
//! at runtime. The verifier cannot inspect the runtime values, so it
//! tracks an upper bound through abstract interpretation over a
//! per-slot lattice.
//!
//! ## Lattice
//!
//! The lattice has two elements:
//!
//! - `TextSize::Known(n)`: the slot carries a text value whose UTF-8
//!   byte length is at most `n`.
//! - `TextSize::Unbounded`: the slot carries a text value whose
//!   length the analysis cannot bound, or carries a non-text value
//!   for which size tracking is meaningless. Both interpretations
//!   collapse to the conservative top of the lattice.
//!
//! The lattice has `Known(0)` as its bottom and `Unbounded` as its
//! top. Join is the maximum of two `Known` values, saturating to
//! `Unbounded` if either argument is `Unbounded`. Addition is the
//! saturating sum, propagating `Unbounded` if either argument is
//! `Unbounded` or if the integer sum would exceed `u32::MAX`.
//!
//! ## Scope of the present implementation
//!
//! V0.2.0 ships the lattice type and the arithmetic primitives.
//! Integration with `verify::compute_chunk_wcmu` is staged for a
//! follow-up commit. Until the integration lands, the heap bound for
//! `Op::Add` on text operands continues to be reported as zero by
//! `CostModel::heap_alloc_bytes` (the fixed view); the runtime
//! exhaustion path through `VmError::OutOfArena` provides the
//! graceful-failure guarantee.

use crate::bytecode::OpCostContext;

/// Upper bound on the UTF-8 byte length of a text value carried in
/// an operand-stack slot or local variable.
///
/// See the module-level documentation for the lattice semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    /// The slot carries a text value whose length is at most `n`
    /// bytes.
    Known(u32),
    /// The slot carries a text value whose length the analysis
    /// cannot bound, or carries a non-text value.
    Unbounded,
}

impl TextSize {
    /// The bottom of the lattice. Equivalent to `Known(0)`.
    pub const ZERO: TextSize = TextSize::Known(0);

    /// The top of the lattice.
    pub const UNBOUNDED: TextSize = TextSize::Unbounded;

    /// Saturating addition of two text-size bounds. Returns
    /// `Unbounded` if either argument is `Unbounded` or if the
    /// integer sum exceeds `u32::MAX`.
    pub fn saturating_add(self, other: TextSize) -> TextSize {
        match (self, other) {
            (TextSize::Unbounded, _) | (_, TextSize::Unbounded) => TextSize::Unbounded,
            (TextSize::Known(a), TextSize::Known(b)) => match a.checked_add(b) {
                Some(sum) if sum < u32::MAX => TextSize::Known(sum),
                _ => TextSize::Unbounded,
            },
        }
    }

    /// Saturating join (maximum) of two text-size bounds. Returns
    /// `Unbounded` if either argument is `Unbounded`.
    pub fn join(self, other: TextSize) -> TextSize {
        match (self, other) {
            (TextSize::Unbounded, _) | (_, TextSize::Unbounded) => TextSize::Unbounded,
            (TextSize::Known(a), TextSize::Known(b)) => TextSize::Known(a.max(b)),
        }
    }

    /// Project the lattice value to a `u32` for use as an operand
    /// length in an [`OpCostContext`]. `Unbounded` projects to
    /// `u32::MAX`, the saturation sentinel that downstream cost
    /// evaluators interpret as the unbounded case.
    pub fn as_u32(self) -> u32 {
        match self {
            TextSize::Known(n) => n,
            TextSize::Unbounded => u32::MAX,
        }
    }
}

/// Build an [`OpCostContext`] from a pair of [`TextSize`] operand
/// bounds. Convenience helper for the integration point where the
/// abstract interpretation pass evaluates an `OpCost::Dynamic` cost.
pub fn op_cost_context(lhs: TextSize, rhs: TextSize) -> OpCostContext {
    OpCostContext {
        lhs_text_len: lhs.as_u32(),
        rhs_text_len: rhs.as_u32(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_known_values() {
        assert_eq!(
            TextSize::Known(3).saturating_add(TextSize::Known(5)),
            TextSize::Known(8)
        );
        assert_eq!(TextSize::ZERO.saturating_add(TextSize::ZERO), TextSize::ZERO);
    }

    #[test]
    fn add_saturates_to_unbounded_on_overflow() {
        assert_eq!(
            TextSize::Known(u32::MAX - 1).saturating_add(TextSize::Known(1)),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Known(u32::MAX / 2 + 1).saturating_add(TextSize::Known(u32::MAX / 2 + 1)),
            TextSize::Unbounded
        );
    }

    #[test]
    fn add_propagates_unbounded() {
        assert_eq!(
            TextSize::Unbounded.saturating_add(TextSize::Known(5)),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Known(5).saturating_add(TextSize::Unbounded),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Unbounded.saturating_add(TextSize::Unbounded),
            TextSize::Unbounded
        );
    }

    #[test]
    fn join_known_takes_max() {
        assert_eq!(
            TextSize::Known(3).join(TextSize::Known(5)),
            TextSize::Known(5)
        );
        assert_eq!(
            TextSize::Known(7).join(TextSize::Known(2)),
            TextSize::Known(7)
        );
    }

    #[test]
    fn join_propagates_unbounded() {
        assert_eq!(
            TextSize::Unbounded.join(TextSize::Known(5)),
            TextSize::Unbounded
        );
        assert_eq!(
            TextSize::Known(5).join(TextSize::Unbounded),
            TextSize::Unbounded
        );
    }

    #[test]
    fn as_u32_projects_unbounded_to_max() {
        assert_eq!(TextSize::Known(42).as_u32(), 42);
        assert_eq!(TextSize::Unbounded.as_u32(), u32::MAX);
        assert_eq!(TextSize::ZERO.as_u32(), 0);
    }

    #[test]
    fn op_cost_context_carries_lattice_values() {
        let ctx = op_cost_context(TextSize::Known(10), TextSize::Known(20));
        assert_eq!(ctx.lhs_text_len, 10);
        assert_eq!(ctx.rhs_text_len, 20);
        let ctx_unbounded = op_cost_context(TextSize::Unbounded, TextSize::Known(5));
        assert_eq!(ctx_unbounded.lhs_text_len, u32::MAX);
        assert_eq!(ctx_unbounded.rhs_text_len, 5);
    }

    #[test]
    fn doubling_pattern_saturates_after_thirty_two_iterations() {
        // The FAQ exponential-string-concat example doubles a 1-byte
        // string sixty times. Tracking the size lattice through the
        // doubling chain reaches `Unbounded` once the size exceeds
        // u32::MAX, which happens at the thirty-second doubling
        // (2^31 = 2_147_483_648, 2^32 - 1 = u32::MAX).
        let mut size = TextSize::Known(1);
        for _ in 0..31 {
            size = size.saturating_add(size);
        }
        assert_eq!(size, TextSize::Known(1u32 << 31));
        // The next doubling crosses the u32::MAX boundary.
        size = size.saturating_add(size);
        assert_eq!(size, TextSize::Unbounded);
        // Further doublings stay at Unbounded.
        for _ in 0..30 {
            size = size.saturating_add(size);
            assert_eq!(size, TextSize::Unbounded);
        }
    }
}
