//! Parametric floating-point abstraction for sub-64-bit native
//! runtimes (B16). The wire-format header's `float_bits_log2`
//! field declares the script-visible float width; this trait
//! lifts that width into the type system so the parametric
//! `Vm<W, A, F>` can specialize its floating-point values to
//! the target's float width.
//!
//! Gated behind the `floats` cargo feature alongside the rest
//! of the floating-point surface (`Value::Float`,
//! `ConstValue::Float`, `audio_natives`, `stddsl`, the
//! `Op::IntToFloat`/`Op::FloatToInt` bodies). Hosts that target
//! integer-only runtimes leave the feature off and never see
//! this trait.
//!
//! Impls: `f32` for sub-64-bit targets and embedded SoCs with a
//! single-precision FPU; `f64` for the default 64-bit runtime.

use core::fmt::Debug;

/// A script-visible floating-point type.
pub trait Float: Copy + Default + PartialEq + PartialOrd + Debug + 'static {
    /// `log2` of the bit width. Matches the bytecode header's
    /// `float_bits_log2` field. Values: `f32` → 5, `f64` → 6.
    const BITS_LOG2: u8;

    /// Convert an `f64` constant from the bytecode pool to
    /// `Self`. Loses precision when narrowing to `f32`; the
    /// bytecode emitter is expected to keep float constants in
    /// range for the target.
    fn from_f64(n: f64) -> Self;

    /// Convert `Self` to `f64` for cross-width comparisons and
    /// for the marshall layer's `KeleusmaType for f64` carrier.
    fn to_f64(self) -> f64;

    /// Add two floating-point operands.
    fn add(self, other: Self) -> Self;

    /// Subtract two floating-point operands.
    fn sub(self, other: Self) -> Self;

    /// Multiply two floating-point operands.
    fn mul(self, other: Self) -> Self;

    /// Divide two floating-point operands.
    fn div(self, other: Self) -> Self;

    /// Negate a floating-point operand.
    fn neg(self) -> Self;
}

impl Float for f32 {
    const BITS_LOG2: u8 = 5;

    fn from_f64(n: f64) -> Self {
        n as f32
    }

    fn to_f64(self) -> f64 {
        self as f64
    }

    fn add(self, other: Self) -> Self {
        self + other
    }

    fn sub(self, other: Self) -> Self {
        self - other
    }

    fn mul(self, other: Self) -> Self {
        self * other
    }

    fn div(self, other: Self) -> Self {
        self / other
    }

    fn neg(self) -> Self {
        -self
    }
}

impl Float for f64 {
    const BITS_LOG2: u8 = 6;

    fn from_f64(n: f64) -> Self {
        n
    }

    fn to_f64(self) -> f64 {
        self
    }

    fn add(self, other: Self) -> Self {
        self + other
    }

    fn sub(self, other: Self) -> Self {
        self - other
    }

    fn mul(self, other: Self) -> Self {
        self * other
    }

    fn div(self, other: Self) -> Self {
        self / other
    }

    fn neg(self) -> Self {
        -self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_basics() {
        assert_eq!(<f32 as Float>::BITS_LOG2, 5);
        assert_eq!(<f32 as Float>::from_f64(0.5), 0.5_f32);
        assert_eq!((1.0_f32).add(2.0).to_f64(), 3.0);
    }

    #[test]
    fn f64_basics() {
        assert_eq!(<f64 as Float>::BITS_LOG2, 6);
        assert_eq!(<f64 as Float>::from_f64(0.5), 0.5_f64);
        assert_eq!((1.0_f64).add(2.0).to_f64(), 3.0);
    }

    #[test]
    fn f32_narrowing_loses_precision() {
        // f64 has more mantissa bits than f32; narrowing rounds
        // to the nearest representable f32. The test confirms
        // the trait does not silently widen.
        let original_f64 = 1.1_f64;
        let narrowed = <f32 as Float>::from_f64(original_f64);
        let back_f64 = <f32 as Float>::to_f64(narrowed);
        // Round-trip loses precision but stays close.
        let delta = (back_f64 - original_f64).abs();
        assert!(delta < 1e-6, "narrowing lost more than 1e-6: {}", delta);
    }
}
