//! (72,64) Hsiao SECDED: single-error correction, double-error detection.
//!
//! Eight check bits per 64-bit word, held in a **parallel plane** — a separate
//! region carrying one parity byte per data word — rather than interleaved with
//! the data.
//!
//! # Why a parallel plane rather than interleaved parity
//!
//! Interleaving would put nine bytes on the wire for every eight of payload, which
//! breaks contiguity: a string could no longer be a direct subslice, and the whole
//! allocation-free read path would collapse into copy-and-reassemble. A parallel
//! plane keeps payload bytes contiguous and in place, lets a hardware
//! implementation fetch data and syndrome concurrently, and is purely additive —
//! an artifact without the plane is simply unprotected, and a reader that ignores
//! it still works.
//!
//! # The construction
//!
//! Every column of the parity-check matrix is **distinct** and of **odd weight**,
//! which is what makes the syndrome self-classifying with no separate overall
//! parity bit:
//!
//! | Syndrome | Meaning |
//! |---|---|
//! | zero | no error |
//! | odd weight, matches a column | single error, located by that column |
//! | even and non-zero | double error: detected, not correctable |
//! | odd weight, matches nothing | three or more errors: reported uncorrectable |
//!
//! Data columns are the 56 weight-3 vectors followed by the first 8 weight-5
//! vectors, ascending; check columns are the 8 unit vectors.
//!
//! The columns are **generated from that rule at compile time**, not transcribed
//! from a table. A hand-copied matrix is exactly the kind of thing that acquires a
//! single wrong digit and then passes every test that uses the same wrong digit on
//! both sides.
//!
//! # What this does and does not buy
//!
//! It corrects **one** flipped bit per 72-bit codeword and detects **two**. Three
//! or more may be mis-corrected or missed; that is inherent to SECDED, not an
//! implementation limit. For accumulating corruption the defence is scrubbing —
//! read, correct, rewrite — before a second fault lands in a word that already
//! carries one, which is why [`EccPlane::scan`] reports corrected words rather
//! than silently fixing them.

use crate::scalar::{u8_at, u64_at};

/// Number of check bits per word.
pub const CHECK_BITS: usize = 8;

/// High bit marking a region kind as the parity plane for the kind below it.
///
/// A plane needs its own region kind, and choosing one per protected kind by
/// hand is a by-name enumeration waiting to drift. The convention instead
/// derives it: the plane for kind `k` is `k | ECC_KIND_BIT`. Every schema kind
/// this project defines sits well below `0x8000`, so the mapping is injective
/// over the kinds in use and a plane's own kind can never collide with a
/// payload kind.
///
/// The reader does not depend on this. [`crate::WireView::ecc_for`] matches on
/// a plane's `covers` field, so the convention is the ENCODER's business alone
/// and an artifact numbering its planes differently still reads correctly.
pub const ECC_KIND_BIT: u16 = 0x8000;

/// The parity-plane kind protecting `kind`, by the [`ECC_KIND_BIT`] convention.
#[inline]
pub const fn plane_kind_for(kind: u16) -> u16 {
    kind | ECC_KIND_BIT
}

/// Parity columns: 64 data columns then 8 check columns.
///
/// Generated from the construction rule so the matrix cannot drift from its
/// specification.
const COLUMNS: [u8; 72] = build_columns();

const fn build_columns() -> [u8; 72] {
    let mut cols = [0u8; 72];
    let mut n = 0;

    // The 56 weight-3 vectors, ascending.
    let mut v = 0u16;
    while v < 256 {
        if (v as u8).count_ones() == 3 {
            cols[n] = v as u8;
            n += 1;
        }
        v += 1;
    }

    // The first 8 weight-5 vectors, ascending. Together these give 64 distinct
    // odd-weight data columns.
    let mut taken = 0;
    let mut v = 0u16;
    while v < 256 && taken < 8 {
        if (v as u8).count_ones() == 5 {
            cols[n] = v as u8;
            n += 1;
            taken += 1;
        }
        v += 1;
    }

    // Check columns are the unit vectors, so a flipped check bit yields its own
    // syndrome and is distinguishable from any data-bit fault.
    let mut i = 0;
    while i < CHECK_BITS {
        cols[64 + i] = 1 << i;
        i += 1;
    }

    cols
}

/// Computes the eight check bits for one data word.
pub fn check_byte(data: u64) -> u8 {
    let mut s = 0u8;
    let mut i = 0;
    while i < 64 {
        if (data >> i) & 1 == 1 {
            s ^= COLUMNS[i];
        }
        i += 1;
    }
    s
}

/// Outcome of decoding one word against its check byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordStatus {
    /// No fault detected. The value is the word as stored.
    Clean(u64),
    /// A single-bit fault was found and corrected. The value is the repaired
    /// word; the artifact itself is unchanged, since the read path does not
    /// mutate the caller's buffer.
    Corrected(u64),
    /// Two or more faults. The word cannot be trusted and no value is offered —
    /// returning a "best effort" value here is how silent corruption spreads.
    Uncorrectable,
}

/// Decodes one word against its check byte.
pub fn decode_word(data: u64, check: u8) -> WordStatus {
    let syndrome = check_byte(data) ^ check;
    if syndrome == 0 {
        return WordStatus::Clean(data);
    }
    if syndrome.count_ones() % 2 == 1 {
        let mut i = 0;
        while i < COLUMNS.len() {
            if COLUMNS[i] == syndrome {
                return if i < 64 {
                    WordStatus::Corrected(data ^ (1u64 << i))
                } else {
                    // A check bit took the hit; the data was never wrong.
                    WordStatus::Corrected(data)
                };
            }
            i += 1;
        }
        // Odd weight but matching no column: three or more faults. Reported
        // rather than mis-corrected.
        return WordStatus::Uncorrectable;
    }
    WordStatus::Uncorrectable
}

/// A parity plane covering a data region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EccPlane<'a> {
    parity: &'a [u8],
}

/// Result of scanning a whole region against its plane.
///
/// `#[non_exhaustive]`: a future scan may report more (words skipped as
/// encrypted, say) and adding a counter should not break callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct EccReport {
    /// Words examined.
    pub words: usize,
    /// Words that carried a corrected single-bit fault.
    pub corrected: usize,
    /// Words with two or more faults.
    pub uncorrectable: usize,
}

impl EccReport {
    /// True when nothing was wrong.
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.corrected == 0 && self.uncorrectable == 0
    }

    /// True when the region carries repairable damage.
    ///
    /// Rewriting it restores the margin that lets the next fault also be
    /// survivable; leaving it lets damage accumulate until a second fault lands
    /// in a word that already carries one.
    #[inline]
    pub fn needs_scrub(&self) -> bool {
        self.corrected > 0
    }
}

impl<'a> EccPlane<'a> {
    /// Wraps a parity region.
    #[inline]
    pub fn new(parity: &'a [u8]) -> Self {
        Self { parity }
    }

    /// The parity bytes.
    #[inline]
    pub fn bytes(&self) -> &'a [u8] {
        self.parity
    }

    /// Decodes word `index` of `data`.
    ///
    /// Returns `None` when the index lies outside either the data region or the
    /// plane, so a truncated or mismatched plane cannot fault the reader.
    pub fn word(&self, data: &[u8], index: usize) -> Option<WordStatus> {
        let at = index.checked_mul(8)?;
        let value = u64_at(data, at)?;
        let check = u8_at(self.parity, index)?;
        Some(decode_word(value, check))
    }

    /// Scans every word of `data` against the plane.
    ///
    /// This is the scrub pass. It reports rather than repairs, because the read
    /// path borrows the caller's buffer and must not mutate it; a caller that
    /// wants to repair copies the data out and applies [`Self::word`] per index.
    pub fn scan(&self, data: &[u8]) -> EccReport {
        let mut report = EccReport::default();
        let words = core::cmp::min(data.len() / 8, self.parity.len());
        for index in 0..words {
            report.words += 1;
            match self.word(data, index) {
                Some(WordStatus::Clean(_)) | None => {}
                Some(WordStatus::Corrected(_)) => report.corrected += 1,
                Some(WordStatus::Uncorrectable) => report.uncorrectable += 1,
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_distinct_and_odd_weight() {
        // The two properties the whole construction rests on. Without odd weight
        // the syndrome is not self-classifying; without distinctness a single
        // error cannot be located.
        for (i, &c) in COLUMNS.iter().enumerate() {
            assert_eq!(c.count_ones() % 2, 1, "column {i} has even weight");
            for (j, &d) in COLUMNS.iter().enumerate() {
                if i != j {
                    assert_ne!(c, d, "columns {i} and {j} collide");
                }
            }
        }
    }

    #[test]
    fn column_layout_matches_the_reference_construction() {
        // 56 weight-3 columns, then 8 weight-5, then 8 unit vectors.
        for c in COLUMNS.iter().take(56) {
            assert_eq!(c.count_ones(), 3);
        }
        for c in COLUMNS.iter().take(64).skip(56) {
            assert_eq!(c.count_ones(), 5);
        }
        for (i, c) in COLUMNS.iter().enumerate().skip(64) {
            assert_eq!(*c, 1 << (i - 64));
        }
        // First and last weight-3 columns, ascending order pinned.
        assert_eq!(COLUMNS[0], 0b0000_0111);
        assert_eq!(COLUMNS[55], 0b1110_0000);
    }

    /// The same patterns the independently written reference model uses.
    const PATTERNS: [u64; 6] = [
        0x0000_0000_0000_0000,
        0xFFFF_FFFF_FFFF_FFFF,
        0xA5A5_A5A5_A5A5_A5A5,
        0x0123_4567_89AB_CDEF,
        0x8000_0000_0000_0001,
        0xDEAD_BEEF_CAFE_F00D,
    ];

    #[test]
    fn check_bytes_match_an_independently_written_reference() {
        // Numerical agreement with a separate implementation of the same
        // construction, not merely the same pass/fail counts. Counts agreeing
        // would only show both sides classify faults the same way; these pin the
        // actual matrix, which is where a transcription error would hide.
        //
        // Vectors produced by the reference model for the patterns below.
        const REFERENCE: [u8; 6] = [0x00, 0xD8, 0x11, 0x42, 0x50, 0xD2];
        for (d, want) in PATTERNS.iter().zip(REFERENCE.iter()) {
            assert_eq!(check_byte(*d), *want, "check byte for {d:#018x}");
        }

        // And the matrix itself at four sampled positions, including both ends of
        // the weight-3 run and the weight-3/weight-5 boundary.
        assert_eq!(COLUMNS[0], 0x07);
        assert_eq!(COLUMNS[55], 0xE0);
        assert_eq!(COLUMNS[56], 0x1F);
        assert_eq!(COLUMNS[63], 0x57);
    }

    #[test]
    fn a_clean_word_decodes_clean() {
        for d in PATTERNS {
            assert_eq!(decode_word(d, check_byte(d)), WordStatus::Clean(d));
        }
    }

    #[test]
    fn every_single_bit_fault_in_the_codeword_is_corrected() {
        // Exhaustive over all 72 bit positions, data and check alike -- 432
        // cases, matching the reference model's count exactly.
        let mut cases = 0;
        for d in PATTERNS {
            let c = check_byte(d);
            for bit in 0..72 {
                let (dd, cc) = if bit < 64 {
                    (d ^ (1u64 << bit), c)
                } else {
                    (d, c ^ (1u8 << (bit - 64)))
                };
                assert_eq!(
                    decode_word(dd, cc),
                    WordStatus::Corrected(d),
                    "bit {bit} of {d:#018x} not corrected"
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 432);
    }

    #[test]
    fn every_double_bit_fault_is_detected_never_miscorrected() {
        // The dangerous failure is SILENT mis-correction, so this asserts the
        // status is Uncorrectable rather than merely that the value differs.
        // Exhaustive over all 2556 pairs per pattern: 15336 cases, matching the
        // reference model.
        let mut cases = 0;
        for d in PATTERNS {
            let c = check_byte(d);
            for b1 in 0..72 {
                for b2 in (b1 + 1)..72 {
                    let mut dd = d;
                    let mut cc = c;
                    for b in [b1, b2] {
                        if b < 64 {
                            dd ^= 1u64 << b;
                        } else {
                            cc ^= 1u8 << (b - 64);
                        }
                    }
                    assert_eq!(
                        decode_word(dd, cc),
                        WordStatus::Uncorrectable,
                        "double fault {b1},{b2} of {d:#018x} not detected"
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 15336);
    }

    #[test]
    fn a_plane_shorter_than_its_region_does_not_fault_the_reader() {
        let data = [0u8; 32];
        let plane = EccPlane::new(&[0u8; 1]);
        assert!(plane.word(&data, 0).is_some());
        assert!(plane.word(&data, 1).is_none());
        assert!(plane.word(&data, usize::MAX).is_none());
        // The scan stops at the shorter of the two rather than running off.
        assert_eq!(plane.scan(&data).words, 1);
    }
}
