//! Parametric integer-word abstraction for sub-64-bit native
//! runtimes (B16). The bundled `Vm` defaults to `Word = i64`, but
//! hosts targeting narrower runtimes (16-bit `Vm<i16, ...>` for
//! retro-class hardware, 8-bit `Vm<i8, ...>` for the smallest
//! microcontrollers) construct a `Vm` with the appropriate
//! integer width.
//!
//! Three traits compose:
//!
//! - [`Word`]: the script-visible signed integer type. Carries
//!   the arithmetic operations the VM needs (`wrapping_add`,
//!   `checked_div`, comparison) and an associated [`Word::Wide`]
//!   type for the widened multiplication intermediate used by
//!   the checked-arithmetic opcodes. The widened type is twice
//!   the bit width of `Self`: `i8::Wide = i16`, `i16::Wide =
//!   i32`, `i32::Wide = i64`, `i64::Wide = i128`. The associated
//!   type is auto-detected from the impl; users do not need to
//!   pass a separate parameter for it.
//!
//! - `WideWord`: the twice-as-wide signed integer used as the
//!   multiplication intermediate. Implemented for `i16`, `i32`,
//!   `i64`, and `i128`.
//!
//! Wire-format alignment. The bytecode header's `word_bits_log2`
//! field encodes the script's word width as a power-of-two
//! exponent. The `Word` trait carries the corresponding
//! [`Word::BITS_LOG2`] constant so a runtime can validate that
//! its compile-time `W` matches the bytecode's declared width
//! at load time.

use core::fmt::Debug;

/// A script-visible signed integer type for the parametric VM.
/// Implemented for `i8`, `i16`, `i32`, and `i64`. The associated
/// `Wide` type is the next-larger signed integer, used as the
/// multiplication intermediate by the checked-arithmetic
/// opcodes.
pub trait Word:
    Copy
    + Default
    + Eq
    + Ord
    + Debug
    + 'static
    + core::ops::BitAnd<Output = Self>
    + core::ops::BitOr<Output = Self>
    + core::ops::BitXor<Output = Self>
    + core::ops::Shr<u32, Output = Self>
    + core::ops::Shl<u32, Output = Self>
{
    /// The widened signed integer for multiplication and the
    /// `i128`-style intermediates in the checked-arithmetic
    /// opcodes. Equal to twice the bit width of `Self`.
    type Wide: WideWord;

    /// `log2` of the bit width. The bytecode header encodes the
    /// same value, so the runtime can validate that its
    /// compile-time `W` matches the bytecode's declared width at
    /// load time. Values: `i8` → 3, `i16` → 4, `i32` → 5, `i64` → 6.
    const BITS_LOG2: u8;

    /// The minimum value representable by `Self`.
    const MIN: Self;

    /// The maximum value representable by `Self`.
    const MAX: Self;

    /// Convert an `i64` constant from the bytecode pool to
    /// `Self`. The high bits are truncated when the constant
    /// does not fit; this is the same wrap-on-load discipline
    /// the existing 64-bit runtime applies to narrower
    /// bytecode through `truncate_int`.
    fn from_i64_wrap(n: i64) -> Self;

    /// Convert `Self` to `i64` for the marshall layer's
    /// `Value::Int` carrier and for comparisons against literal
    /// constants in the constant pool. Sign-extends narrower
    /// widths.
    fn to_i64(self) -> i64;

    /// Convert `Self` to `usize` for indexing into Rust-side
    /// containers. Returns `None` for negative values (since
    /// `usize` is unsigned) and for values that exceed
    /// `usize::MAX` on the host (relevant only for `i64` on a
    /// 32-bit host). Mirrors [`crate::address::Address::to_usize_checked`].
    fn to_usize_checked(self) -> Option<usize> {
        usize::try_from(self.to_i64()).ok()
    }

    /// Widen to the multiplication intermediate.
    fn widen(self) -> Self::Wide;

    /// Truncate the widened type back to `Self`, wrapping on
    /// overflow. Used after multiplication and checked-
    /// arithmetic intermediates to recover the low-half result.
    fn from_wide_wrap(w: Self::Wide) -> Self;

    /// Wrapping addition.
    fn wrapping_add(self, other: Self) -> Self;

    /// Wrapping subtraction.
    fn wrapping_sub(self, other: Self) -> Self;

    /// Wrapping multiplication. The result is the low half of
    /// the widened product.
    fn wrapping_mul(self, other: Self) -> Self;

    /// Wrapping division. Panics on division by zero; the VM's
    /// dispatch loop checks for zero before calling this.
    fn wrapping_div(self, other: Self) -> Self;

    /// Wrapping remainder. Panics on division by zero.
    fn wrapping_rem(self, other: Self) -> Self;

    /// Wrapping negation.
    fn wrapping_neg(self) -> Self;
}

/// The widened multiplication intermediate. Implemented for the
/// `Wide` type of each [`Word`] impl. Carries the arithmetic
/// and range-check operations the VM needs to derive the
/// `(high, low, flag)` shape of the checked-arithmetic opcodes.
pub trait WideWord:
    Copy
    + Default
    + Eq
    + Ord
    + Debug
    + 'static
    + core::ops::Add<Output = Self>
    + core::ops::Sub<Output = Self>
    + core::ops::Mul<Output = Self>
    + core::ops::Div<Output = Self>
    + core::ops::Rem<Output = Self>
    + core::ops::Neg<Output = Self>
    + core::ops::Shr<u32, Output = Self>
    + core::ops::Shl<u32, Output = Self>
{
    /// Add two widened operands.
    fn wide_add(self, other: Self) -> Self;

    /// Subtract two widened operands.
    fn wide_sub(self, other: Self) -> Self;

    /// Multiply two widened operands. Used after both operands
    /// have been widened via [`Word::widen`].
    fn wide_mul(self, other: Self) -> Self;

    /// Negate a widened operand.
    fn wide_neg(self) -> Self;

    /// Right-shift by half the widened width (i.e. by the
    /// `Word`'s bit width). Returns the high half of the
    /// widened value as a value still in the widened type.
    /// Callers narrow to the `Word` type with `from_wide_wrap`.
    fn high_half(self) -> Self;
}

macro_rules! impl_word_pair {
    ($word:ty, $wide:ty, $bits_log2:expr, $word_bits:expr) => {
        impl Word for $word {
            type Wide = $wide;
            const BITS_LOG2: u8 = $bits_log2;
            const MIN: Self = <$word>::MIN;
            const MAX: Self = <$word>::MAX;

            fn from_i64_wrap(n: i64) -> Self {
                n as $word
            }

            fn to_i64(self) -> i64 {
                self as i64
            }

            fn widen(self) -> Self::Wide {
                self as $wide
            }

            fn from_wide_wrap(w: Self::Wide) -> Self {
                w as $word
            }

            fn wrapping_add(self, other: Self) -> Self {
                <$word>::wrapping_add(self, other)
            }

            fn wrapping_sub(self, other: Self) -> Self {
                <$word>::wrapping_sub(self, other)
            }

            fn wrapping_mul(self, other: Self) -> Self {
                <$word>::wrapping_mul(self, other)
            }

            fn wrapping_div(self, other: Self) -> Self {
                <$word>::wrapping_div(self, other)
            }

            fn wrapping_rem(self, other: Self) -> Self {
                <$word>::wrapping_rem(self, other)
            }

            fn wrapping_neg(self) -> Self {
                <$word>::wrapping_neg(self)
            }
        }

        impl WideWord for $wide {
            fn wide_add(self, other: Self) -> Self {
                <$wide>::wrapping_add(self, other)
            }

            fn wide_sub(self, other: Self) -> Self {
                <$wide>::wrapping_sub(self, other)
            }

            fn wide_mul(self, other: Self) -> Self {
                <$wide>::wrapping_mul(self, other)
            }

            fn wide_neg(self) -> Self {
                <$wide>::wrapping_neg(self)
            }

            fn high_half(self) -> Self {
                self >> $word_bits
            }
        }
    };
}

impl_word_pair!(i8, i16, 3, 8);
impl_word_pair!(i16, i32, 4, 16);
impl_word_pair!(i32, i64, 5, 32);
impl_word_pair!(i64, i128, 6, 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i8_word_basics() {
        assert_eq!(<i8 as Word>::BITS_LOG2, 3);
        assert_eq!(<i8 as Word>::MIN, i8::MIN);
        assert_eq!(<i8 as Word>::MAX, i8::MAX);
        assert_eq!(<i8 as Word>::from_i64_wrap(0x1234), 0x34_u8 as i8);
        assert_eq!((100_i8).wrapping_add(50), -106_i8);
    }

    #[test]
    fn i16_word_basics() {
        assert_eq!(<i16 as Word>::BITS_LOG2, 4);
        assert_eq!(<i16 as Word>::from_i64_wrap(0x12345), 0x2345_i16);
        assert_eq!((30000_i16).wrapping_add(10000), -25536_i16);
    }

    #[test]
    fn i32_word_basics() {
        assert_eq!(<i32 as Word>::BITS_LOG2, 5);
        assert_eq!(<i32 as Word>::from_i64_wrap(0x1_0000_0001), 1_i32);
    }

    #[test]
    fn i64_word_basics() {
        assert_eq!(<i64 as Word>::BITS_LOG2, 6);
        assert_eq!(<i64 as Word>::from_i64_wrap(0x1234), 0x1234_i64);
    }

    #[test]
    fn widening_multiplication_high_low_split_i8() {
        // 100 * 50 = 5000. In i8: low = 5000 % 256 = 136 - 256 = -120.
        // Wide product is 5000 = 0x1388. High half = 0x13 = 19.
        let a = 100_i8;
        let b = 50_i8;
        let wide = a.widen().wide_mul(b.widen());
        assert_eq!(wide, 5000_i16);
        assert_eq!(<i16 as WideWord>::high_half(wide), 19_i16);
        assert_eq!(<i8 as Word>::from_wide_wrap(wide), -120_i8);
    }

    #[test]
    fn widening_multiplication_high_low_split_i64() {
        // i64::MAX * 2 overflows; wide product is 2 * (2^63 - 1) =
        // 2^64 - 2 = 0xFFFFFFFFFFFFFFFE as i128 (positive).
        let a = i64::MAX;
        let b = 2_i64;
        let wide = a.widen().wide_mul(b.widen());
        assert_eq!(wide, (i64::MAX as i128) * 2);
        // High half: (2^64 - 2) >> 64 = 0.
        assert_eq!(<i128 as WideWord>::high_half(wide), 0_i128);
        // Low half wraps to -2 in i64.
        assert_eq!(<i64 as Word>::from_wide_wrap(wide), -2_i64);
    }

    #[test]
    fn wrapping_arithmetic_at_extreme_values() {
        // i8::MIN + 1 wraps: -128 + 1 = -127 (no wrap).
        assert_eq!(i8::MIN.wrapping_add(1), -127_i8);
        // i8::MAX + 1 wraps: 127 + 1 = -128.
        assert_eq!(i8::MAX.wrapping_add(1), i8::MIN);
        // i8::MIN.wrapping_neg() saturates to i8::MIN.
        assert_eq!(i8::MIN.wrapping_neg(), i8::MIN);
    }

    #[test]
    fn to_usize_checked_succeeds_for_non_negative() {
        assert_eq!(<i8 as Word>::to_usize_checked(0_i8), Some(0_usize));
        assert_eq!(<i8 as Word>::to_usize_checked(127_i8), Some(127_usize));
        assert_eq!(
            <i16 as Word>::to_usize_checked(30_000_i16),
            Some(30_000_usize)
        );
        assert_eq!(
            <i64 as Word>::to_usize_checked(1_000_000_i64),
            Some(1_000_000_usize)
        );
    }

    #[test]
    fn to_usize_checked_rejects_negative() {
        assert_eq!(<i8 as Word>::to_usize_checked(-1_i8), None);
        assert_eq!(<i16 as Word>::to_usize_checked(i16::MIN), None);
        assert_eq!(<i64 as Word>::to_usize_checked(-42_i64), None);
    }

    #[test]
    fn bits_log2_matches_runtime_constant() {
        // The bytecode header's word_bits_log2 field should
        // round-trip through Word::BITS_LOG2.
        assert_eq!(<i8 as Word>::BITS_LOG2, 3);
        assert_eq!(<i16 as Word>::BITS_LOG2, 4);
        assert_eq!(<i32 as Word>::BITS_LOG2, 5);
        assert_eq!(<i64 as Word>::BITS_LOG2, 6);
    }
}
