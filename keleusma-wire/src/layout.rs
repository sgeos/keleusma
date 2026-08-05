//! Container layout constants and the region-directory record.
//!
//! # The artifact
//!
//! ```text
//! prologue  ×3        offsets 0, 16, 32          fixed 16 bytes each
//! directory ×3        offset 48                  region_count × 16 bytes each
//! regions                                        word-aligned payloads
//! ```
//!
//! ## Prologue (16 bytes, 2 words)
//!
//! | Offset | Width | Field |
//! |---|---|---|
//! | 0 | u32 | magic |
//! | 4 | u16 | byte-order marker |
//! | 6 | u16 | format version |
//! | 8 | u16 | region count |
//! | 10 | u16 | flags |
//! | 12 | u32 | CRC-32 of bytes 0..12 |
//!
//! ## Directory entry (16 bytes, 2 words)
//!
//! | Offset | Width | Field |
//! |---|---|---|
//! | 0 | u16 | kind |
//! | 2 | u16 | flags |
//! | 4 | u32 | word offset |
//! | 8 | u32 | word length |
//! | 12 | u16 | covers (ECC planes only) |
//! | 14 | u16 | reserved |
//!
//! # Why the prologue is separate from the directory
//!
//! This split is not cosmetic, and it was **not** in the design sketch. Writing a
//! real reader exposed a bootstrapping problem the prototype had dodged by
//! hardcoding its block size.
//!
//! The header and directory are triplicated so a corrupt copy can be outvoted.
//! Voting requires locating copies 1 and 2, which requires knowing the block
//! stride, which — if the directory sits inside the block — depends on
//! `region_count`, which is *itself inside the block being voted*. A single bit
//! flip in `region_count` would therefore desynchronise the search for the copies
//! that exist to repair it. The one field the vote most needs to protect was the
//! one field the vote could not be performed without.
//!
//! Splitting a **fixed-size** prologue out resolves it: the three prologue copies
//! sit at offsets 0, 16 and 32 by definition, so they are votable with no prior
//! knowledge. The voted `region_count` then gives the directory stride, and the
//! three directory copies are votable in turn.
//!
//! Every record is a whole number of 64-bit words, so element *i* of any table is
//! at `base + i * stride` with `stride` a power of two — a shift, not a multiply.

/// Bytes per word. Every region and record is a whole number of these.
pub const WORD: usize = 8;

/// Container magic, `"KAUX"` read little-endian.
pub const MAGIC: u32 = 0x4B41_5558;

/// Byte-order marker. A reader seeing `0xFFFE` knows the artifact is
/// opposite-endian without consulting any external document.
pub const BOM: u16 = 0xFEFF;

/// Format version this crate reads and writes.
pub const FORMAT_VERSION: u16 = 2;

/// Size of the fixed prologue record.
pub const PROLOGUE_BYTES: usize = 16;

/// Size of one region-directory entry.
pub const DIR_ENTRY_BYTES: usize = 16;

/// Number of redundant copies of the prologue and of the directory.
pub const COPIES: usize = 3;

/// Byte offset of the first directory copy.
pub const DIRECTORY_OFFSET: usize = PROLOGUE_BYTES * COPIES;

/// Upper bound on regions in one artifact.
///
/// Bounded so a corrupt `region_count` cannot drive an unbounded walk, and so a
/// reader's work is statically bounded. 1024 regions is far beyond any plausible
/// schema while keeping the directory under 48 KiB across all three copies.
pub const MAX_REGIONS: usize = 1024;

/// Region flag: the payload is encrypted and cannot be read in place.
pub const FLAG_ENCRYPTED: u16 = 1 << 0;

/// Region flag: a companion ECC region covers this one.
pub const FLAG_ECC_PRESENT: u16 = 1 << 1;

/// Region flag: a reader that does not recognise this region may skip it.
pub const FLAG_OPTIONAL: u16 = 1 << 2;

/// Region flag: this region **is** a parity plane, and [`Region::covers`] names
/// the region kind it protects.
pub const FLAG_IS_ECC: u16 = 1 << 3;

/// One region-directory entry.
///
/// This crate assigns no meaning to `kind`. The schema layer above chooses the
/// numbering; the container only locates payloads. That is what keeps the format
/// reusable by a project with entirely different content.
/// Marked `#[non_exhaustive]` deliberately: `covers` was added after the first
/// draft, and post-1.0 that would have been a breaking change for anyone
/// constructing a `Region` literal. Callers read these; the container produces
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct Region {
    /// Schema-defined region kind. Opaque to this crate.
    pub kind: u16,
    /// Region flags. See the `FLAG_*` constants.
    pub flags: u16,
    /// Payload start, in words from the beginning of the artifact.
    pub word_offset: u32,
    /// Payload length, in words.
    pub word_length: u32,
    /// When [`FLAG_IS_ECC`] is set, the kind of the region this plane protects.
    /// Otherwise zero and unused.
    ///
    /// This is the one relationship the container itself understands. It is still
    /// mechanism rather than schema: which regions exist and what they mean stays
    /// the caller's business, but "this parity plane covers that payload" is a
    /// property of the container, and leaving it to the schema would mean every
    /// user reinvented it.
    pub covers: u16,
}

impl Region {
    /// Payload start in bytes, or `None` on overflow.
    #[inline]
    pub fn byte_offset(&self) -> Option<usize> {
        (self.word_offset as usize).checked_mul(WORD)
    }

    /// Payload length in bytes, or `None` on overflow.
    #[inline]
    pub fn byte_length(&self) -> Option<usize> {
        (self.word_length as usize).checked_mul(WORD)
    }

    /// True when this region's payload is encrypted, and therefore must not be
    /// read in place.
    #[inline]
    pub fn is_encrypted(&self) -> bool {
        self.flags & FLAG_ENCRYPTED != 0
    }

    /// True when this region is a parity plane rather than payload.
    #[inline]
    pub fn is_ecc_plane(&self) -> bool {
        self.flags & FLAG_IS_ECC != 0
    }

    /// True when a parity plane elsewhere in the artifact protects this region.
    #[inline]
    pub fn has_ecc(&self) -> bool {
        self.flags & FLAG_ECC_PRESENT != 0
    }
}

/// Rounds a byte count up to a whole number of words.
#[inline]
pub fn words_for(bytes: usize) -> Option<usize> {
    bytes.checked_add(WORD - 1).map(|n| n / WORD)
}

/// Byte length of the whole header area for `region_count` regions.
#[inline]
pub fn header_bytes(region_count: usize) -> Option<usize> {
    region_count
        .checked_mul(DIR_ENTRY_BYTES)?
        .checked_mul(COPIES)?
        .checked_add(DIRECTORY_OFFSET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_record_is_a_whole_number_of_words() {
        // The design rule that a 12-byte directory entry violated. Pinned so it
        // cannot regress silently.
        assert_eq!(PROLOGUE_BYTES % WORD, 0);
        assert_eq!(DIR_ENTRY_BYTES % WORD, 0);
        assert_eq!(DIRECTORY_OFFSET % WORD, 0);
    }

    #[test]
    fn strides_are_powers_of_two_so_addressing_is_a_shift() {
        assert!(DIR_ENTRY_BYTES.is_power_of_two());
        assert!(WORD.is_power_of_two());
    }

    #[test]
    fn header_size_arithmetic_saturates_rather_than_wrapping() {
        assert_eq!(header_bytes(0), Some(DIRECTORY_OFFSET));
        assert_eq!(header_bytes(1), Some(DIRECTORY_OFFSET + 48));
        assert_eq!(header_bytes(usize::MAX), None);
        assert_eq!(words_for(usize::MAX), None);
        assert_eq!(words_for(0), Some(0));
        assert_eq!(words_for(1), Some(1));
        assert_eq!(words_for(8), Some(1));
        assert_eq!(words_for(9), Some(2));
    }

    #[test]
    fn region_byte_arithmetic_is_checked() {
        let r = Region {
            kind: 1,
            flags: 0,
            word_offset: u32::MAX,
            word_length: u32::MAX,
            covers: 0,
        };
        // On a 64-bit host these fit; the point is that they are computed with
        // checked arithmetic rather than a bare multiply.
        assert_eq!(r.byte_offset(), (u32::MAX as usize).checked_mul(WORD));
        assert!(!r.is_encrypted());

        let e = Region {
            kind: 1,
            flags: FLAG_ENCRYPTED,
            word_offset: 0,
            word_length: 0,
            covers: 0,
        };
        assert!(e.is_encrypted());
    }
}
