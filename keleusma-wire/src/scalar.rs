//! Little-endian scalar readers and writers.
//!
//! Every reader is **total**: an out-of-range offset returns `None` rather than
//! panicking, so a hostile or truncated buffer cannot fault the decoder.
//!
//! # Why these are written with literal place values
//!
//! The obvious Rust here is `u32::from_le_bytes(b[at..at + 4].try_into().ok()?)`.
//! These use explicit place-value arithmetic instead, and that is deliberate:
//! this codec is intended to be transliterated into Keleusma, where the unrolled
//! place-value form is the one that needs no loop and no accumulator state, and
//! into hardware, where each byte of a word is a fixed slice — wiring rather than
//! arithmetic. Keeping one shape across all three targets is worth more than
//! idiomatic brevity in one of them.
//!
//! The generated code is identical; this is a source-level choice, not a
//! performance one. Please do not "simplify" these to `from_le_bytes`.

/// True when `n` bytes starting at `at` lie inside `b`.
///
/// Written as a subtraction on the length rather than `at + n <= b.len()`
/// because the addition overflows for `at` near [`usize::MAX`], which would
/// panic in a debug build — a totality hole in the one place that exists to
/// guarantee totality. The subtraction cannot overflow.
#[inline]
fn has(b: &[u8], at: usize, n: usize) -> bool {
    match b.len().checked_sub(at) {
        Some(remaining) => remaining >= n,
        None => false,
    }
}

/// Reads a `u8`. Returns `None` if `at` is out of range.
#[inline]
pub fn u8_at(b: &[u8], at: usize) -> Option<u8> {
    if !has(b, at, 1) {
        return None;
    }
    Some(b[at])
}

/// Reads a little-endian `u16`. Returns `None` unless both bytes are in range.
#[inline]
pub fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    if !has(b, at, 2) {
        return None;
    }
    Some(b[at] as u16 + (b[at + 1] as u16) * 256)
}

/// Reads a little-endian `u32`. Returns `None` unless all four bytes are in range.
#[inline]
pub fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    if !has(b, at, 4) {
        return None;
    }
    Some(
        b[at] as u32
            + (b[at + 1] as u32) * 256
            + (b[at + 2] as u32) * 65536
            + (b[at + 3] as u32) * 16_777_216,
    )
}

/// Reads a little-endian `u64`. Returns `None` unless all eight bytes are in range.
#[inline]
pub fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    if !has(b, at, 8) {
        return None;
    }
    Some(
        b[at] as u64
            + (b[at + 1] as u64) * 256
            + (b[at + 2] as u64) * 65536
            + (b[at + 3] as u64) * 16_777_216
            + (b[at + 4] as u64) * 4_294_967_296
            + (b[at + 5] as u64) * 1_099_511_627_776
            + (b[at + 6] as u64) * 281_474_976_710_656
            + (b[at + 7] as u64) * 72_057_594_037_927_936,
    )
}

/// Writes a `u8`.
///
/// Trivial, and present for symmetry: the derive names a writer for every
/// permitted scalar width, and a gap at one byte would mean special-casing the
/// narrowest field in generated code.
#[inline]
pub fn u8_bytes(v: u8) -> [u8; 1] {
    [v]
}

/// Writes a little-endian `u16`.
#[inline]
pub fn u16_bytes(v: u16) -> [u8; 2] {
    [(v % 256) as u8, ((v / 256) % 256) as u8]
}

/// Writes a little-endian `u32`.
#[inline]
pub fn u32_bytes(v: u32) -> [u8; 4] {
    [
        (v % 256) as u8,
        ((v / 256) % 256) as u8,
        ((v / 65536) % 256) as u8,
        ((v / 16_777_216) % 256) as u8,
    ]
}

/// Writes a little-endian `u64`.
#[inline]
pub fn u64_bytes(v: u64) -> [u8; 8] {
    [
        (v % 256) as u8,
        ((v / 256) % 256) as u8,
        ((v / 65536) % 256) as u8,
        ((v / 16_777_216) % 256) as u8,
        ((v / 4_294_967_296) % 256) as u8,
        ((v / 1_099_511_627_776) % 256) as u8,
        ((v / 281_474_976_710_656) % 256) as u8,
        ((v / 72_057_594_037_927_936) % 256) as u8,
    ]
}

/// Bitwise majority of three bytes. One gate per bit in hardware.
#[inline]
pub fn maj3(a: u8, b: u8, c: u8) -> u8 {
    (a & b) | (a & c) | (b & c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_round_trip_against_the_standard_library() {
        // The place-value form must agree with `from_le_bytes` exactly. If a
        // transcription error ever creeps into a place value, this catches it --
        // which is the failure the prototype hit once, in a magic constant.
        let cases: [u64; 7] = [0, 1, 255, 256, 0xDEAD_BEEF, u32::MAX as u64, u64::MAX];
        for v in cases {
            let b = u64_bytes(v);
            assert_eq!(u64_at(&b, 0), Some(v));
            assert_eq!(u64_at(&b, 0), Some(u64::from_le_bytes(b)));

            let w = v as u32;
            let b4 = u32_bytes(w);
            assert_eq!(u32_at(&b4, 0), Some(w));
            assert_eq!(u32_at(&b4, 0), Some(u32::from_le_bytes(b4)));

            let h = v as u16;
            let b2 = u16_bytes(h);
            assert_eq!(u16_at(&b2, 0), Some(h));
            assert_eq!(u16_at(&b2, 0), Some(u16::from_le_bytes(b2)));
        }
    }

    #[test]
    fn readers_are_total_at_every_truncation() {
        // Reading one byte past the end must return None, never panic, at every
        // width and every offset.
        let full = [1u8, 2, 3, 4, 5, 6, 7, 8];
        for cut in 0..=full.len() {
            let b = &full[..cut];
            for at in 0..12usize {
                assert_eq!(u8_at(b, at).is_some(), at < cut);
                assert_eq!(u16_at(b, at).is_some(), at + 2 <= cut);
                assert_eq!(u32_at(b, at).is_some(), at + 4 <= cut);
                assert_eq!(u64_at(b, at).is_some(), at + 8 <= cut);
            }
        }
    }

    #[test]
    fn readers_do_not_wrap_on_an_offset_near_usize_max() {
        // The first version of these readers bounds-checked with `at + n`, which
        // overflows here and panics in a debug build -- a totality hole in the
        // very functions whose contract is totality. Every width is checked at
        // the extreme offset, not just the widest.
        let b = [0u8; 8];
        for at in [usize::MAX, usize::MAX - 1, usize::MAX - 3, usize::MAX - 7] {
            assert_eq!(u8_at(&b, at), None);
            assert_eq!(u16_at(&b, at), None);
            assert_eq!(u32_at(&b, at), None);
            assert_eq!(u64_at(&b, at), None);
        }
    }

    #[test]
    fn majority_of_three_outvotes_a_single_corrupt_copy() {
        assert_eq!(maj3(0xAA, 0xAA, 0xAA), 0xAA);
        assert_eq!(maj3(0xAB, 0xAA, 0xAA), 0xAA);
        assert_eq!(maj3(0xAA, 0xAB, 0xAA), 0xAA);
        assert_eq!(maj3(0xAA, 0xAA, 0xAB), 0xAA);
        // Two faults in the same bit defeat the vote. Documented, not defended
        // against -- majority-of-three corrects one fault by construction.
        assert_eq!(maj3(0xAB, 0xAB, 0xAA), 0xAB);
    }
}
