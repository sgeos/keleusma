//! Parametric address-value abstraction for sub-64-bit native
//! runtimes (B16). The wire-format header's `addr_bits_log2`
//! field declares the script-visible address width; this trait
//! lifts that width into the type system so the parametric
//! `Vm<W, A, F>` can specialize its address values to the
//! target's pointer width.
//!
//! Address values are *script-visible* indices — slot indices
//! in the data segment, byte offsets for arena handles, and
//! similar quantities the script can observe through native
//! functions or direct opcode arguments. Internal VM
//! bookkeeping (the instruction pointer, the chunk count, the
//! operand-stack offset) stays `usize`; the parametric `Address`
//! type is only the surface-visible width.
//!
//! Impls are provided for `u8` (8-bit script address space),
//! `u16` (16-bit, e.g. retro-class hardware), `u32` (32-bit
//! microcontrollers and embedded SoCs), and `u64` (modern hosts,
//! the default for `Vm`).

use core::fmt::Debug;

/// A script-visible unsigned address value.
pub trait Address: Copy + Default + Eq + Ord + Debug + 'static {
    /// `log2` of the bit width. Matches the bytecode header's
    /// `addr_bits_log2` field. Values: `u8` → 3, `u16` → 4,
    /// `u32` → 5, `u64` → 6.
    const BITS_LOG2: u8;

    /// The minimum value representable (always `0` for the
    /// unsigned address types). Exposed as a constant so the
    /// trait surface mirrors [`crate::word::Word`].
    const MIN: Self;

    /// The maximum value representable. Determines the upper
    /// bound for slot indices and other surface-visible address
    /// quantities.
    const MAX: Self;

    /// Convert a `u64` constant from the bytecode pool to
    /// `Self`. The high bits are truncated when the constant
    /// does not fit; the bytecode emitter is expected to keep
    /// address values in range, so truncation indicates a load
    /// of bytecode produced for a wider target.
    fn from_u64_wrap(n: u64) -> Self;

    /// Convert `Self` to `u64` for cross-width comparisons and
    /// for resolving against host-side capacity bounds.
    fn to_u64(self) -> u64;

    /// Convert `Self` to `usize` for indexing into Rust-side
    /// containers. Returns `None` when the address exceeds
    /// `usize::MAX` on the host (relevant only for 64-bit
    /// `Address` on a 32-bit host).
    fn to_usize_checked(self) -> Option<usize>;
}

macro_rules! impl_address {
    ($ty:ty, $bits_log2:expr) => {
        impl Address for $ty {
            const BITS_LOG2: u8 = $bits_log2;
            const MIN: Self = <$ty>::MIN;
            const MAX: Self = <$ty>::MAX;

            fn from_u64_wrap(n: u64) -> Self {
                n as $ty
            }

            fn to_u64(self) -> u64 {
                self as u64
            }

            fn to_usize_checked(self) -> Option<usize> {
                usize::try_from(self as u64).ok()
            }
        }
    };
}

impl_address!(u8, 3);
impl_address!(u16, 4);
impl_address!(u32, 5);
impl_address!(u64, 6);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u8_address_basics() {
        assert_eq!(<u8 as Address>::BITS_LOG2, 3);
        assert_eq!(<u8 as Address>::MAX, u8::MAX);
        assert_eq!(<u8 as Address>::from_u64_wrap(0x1234), 0x34_u8);
        assert_eq!(<u8 as Address>::to_u64(42_u8), 42_u64);
    }

    #[test]
    fn u16_address_basics() {
        assert_eq!(<u16 as Address>::BITS_LOG2, 4);
        assert_eq!(<u16 as Address>::from_u64_wrap(0x12345), 0x2345_u16);
    }

    #[test]
    fn u32_address_basics() {
        assert_eq!(<u32 as Address>::BITS_LOG2, 5);
        assert_eq!(<u32 as Address>::from_u64_wrap(0x1_0000_0001), 1_u32);
    }

    #[test]
    fn u64_address_basics() {
        assert_eq!(<u64 as Address>::BITS_LOG2, 6);
        assert_eq!(<u64 as Address>::from_u64_wrap(0x1234), 0x1234_u64);
    }

    #[test]
    fn to_usize_checked_succeeds_within_host_bounds() {
        // On a 64-bit host this round-trips without loss for
        // any u32 address. The test is robust on a 32-bit host
        // because `u32::MAX` is exactly `usize::MAX` there.
        assert_eq!((42_u32).to_usize_checked(), Some(42_usize));
    }

    #[test]
    fn bits_log2_matches_runtime_constant() {
        assert_eq!(<u8 as Address>::BITS_LOG2, 3);
        assert_eq!(<u16 as Address>::BITS_LOG2, 4);
        assert_eq!(<u32 as Address>::BITS_LOG2, 5);
        assert_eq!(<u64 as Address>::BITS_LOG2, 6);
    }
}
