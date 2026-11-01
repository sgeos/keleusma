//! CRC-32/ISO-HDLC, the check used by the prologue and by region integrity.
//!
//! Computed bitwise rather than from a lookup table. A table would be 1 KiB of
//! static data and a data-dependent load per byte; the bitwise form is eight
//! fixed iterations per byte with no table, which is what makes it expressible
//! in a bounded-loop language and cheap in hardware (a shift register with taps).
//!
//! Same polynomial the surrounding project already uses, so an artifact's check
//! is verifiable by existing tooling.

/// Reflected CRC-32/ISO-HDLC polynomial.
const POLY: u32 = 0xEDB8_8320;

/// Computes CRC-32/ISO-HDLC over `bytes`.
///
/// Loop bounds are static: eight iterations per byte, one pass over the input.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in bytes {
        crc ^= byte as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (POLY & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_published_check_value() {
        // The standard CRC-32/ISO-HDLC check: "123456789" -> 0xCBF43926. This is
        // the published vector for the algorithm, so agreement pins the
        // polynomial, the reflection, and both the initial and final XOR at once.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn empty_input_is_the_identity_case() {
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn a_single_bit_flip_changes_the_check() {
        // The property the field exists for. Checked over every bit of a short
        // message rather than one sampled bit.
        let base = [0x12u8, 0x34, 0x56, 0x78];
        let want = crc32(&base);
        for byte in 0..base.len() {
            for bit in 0..8 {
                let mut m = base;
                m[byte] ^= 1 << bit;
                assert_ne!(crc32(&m), want, "flip at byte {byte} bit {bit} undetected");
            }
        }
    }

    #[test]
    fn length_changes_are_detected() {
        // A trailing zero byte must not produce the same check, or truncation
        // and zero-padding would be indistinguishable.
        assert_ne!(crc32(b"\x00"), crc32(b"\x00\x00"));
        assert_ne!(crc32(b""), crc32(b"\x00"));
    }
}
