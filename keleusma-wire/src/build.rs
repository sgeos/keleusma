//! The encoder: accumulate one buffer per region, then emit.
//!
//! # Why one buffer per region
//!
//! A producer that discovers content incrementally cannot fill globally
//! contiguous regions in a single forward pass: a stage that finds a string and a
//! record in the same unit of work would have to append to two different regions
//! at once, and they would interleave. Nor can it write a leading directory
//! first, because the directory's contents are the region offsets and lengths,
//! which are unknown until emission ends.
//!
//! Buffering per region resolves both while keeping the directory at the front,
//! which is what a hardware reader and a random-access reader both want. Append
//! remains forward-only *within* a region — no back-patching, no two-pass writes
//! — and the only join is the concatenation at [`WireBuilder::finish`].
//!
//! The alternative, a trailing directory with self-contained per-unit segments,
//! is genuinely single-pass but gives up cross-region sharing (identical strings
//! in different units can no longer be pooled) and makes every offset
//! segment-relative. It was prototyped and works; per-region buffering was chosen
//! deliberately over it, and the alternative remains reachable without changing
//! any record layout — only the directory's position would move.

extern crate alloc;

use alloc::vec::Vec;

use crate::crc::crc32;
use crate::ecc::check_byte;
use crate::error::WireError;
use crate::layout::{
    BOM, COPIES, DIR_ENTRY_BYTES, FLAG_ECC_PRESENT, FLAG_IS_ECC, FORMAT_VERSION, MAGIC,
    MAX_REGIONS, PROLOGUE_BYTES, WORD, header_bytes, words_for,
};
use crate::record::WireRecord;
use crate::scalar::{u16_bytes, u32_bytes, u64_at};

/// One region under construction.
struct RegionBuf {
    kind: u16,
    flags: u16,
    bytes: Vec<u8>,
    /// When set, a parity plane of this kind is generated at `finish`.
    ecc_kind: Option<u16>,
}

/// Computes a parity plane for a payload: one check byte per 64-bit word.
///
/// The payload is treated as zero-padded to a whole word, matching how the reader
/// sees it, so the last partial word's parity covers the padding too. Padding is
/// handled here rather than by materialising a padded copy, so protecting a
/// region does not double peak memory.
fn parity_of(bytes: &[u8]) -> Vec<u8> {
    let words = bytes.len().div_ceil(WORD);
    let mut out = Vec::with_capacity(words);
    for i in 0..words {
        let start = i * WORD;
        let end = core::cmp::min(start + WORD, bytes.len());
        let mut word = [0u8; WORD];
        word[..end - start].copy_from_slice(&bytes[start..end]);
        // `u64_at` on a fixed 8-byte array cannot fail.
        out.push(check_byte(u64_at(&word, 0).unwrap_or(0)));
    }
    out
}

/// Builds an artifact from regions.
///
/// Regions are appended to independently and in any order; the builder pads each
/// to a whole number of words and lays them out at [`WireBuilder::finish`].
#[derive(Default)]
pub struct WireBuilder {
    regions: Vec<RegionBuf>,
}

impl WireBuilder {
    /// A builder with no regions.
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Declares a region and returns its handle.
    ///
    /// # Errors
    ///
    /// [`WireError::DuplicateRegion`] if `kind` is already declared, since the
    /// reader rejects duplicates; catching it here reports the fault at the point
    /// that caused it. [`WireError::TooManyRegions`] past
    /// [`crate::layout::MAX_REGIONS`].
    pub fn region(&mut self, kind: u16, flags: u16) -> Result<RegionId, WireError> {
        if self.kind_in_use(kind) {
            return Err(WireError::DuplicateRegion { kind });
        }
        if self.emit_count() >= MAX_REGIONS {
            return Err(WireError::TooManyRegions {
                found: self.emit_count() as u32 + 1,
            });
        }
        self.regions.push(RegionBuf {
            kind,
            flags,
            bytes: Vec::new(),
            ecc_kind: None,
        });
        Ok(RegionId(self.regions.len() - 1))
    }

    /// Generates a parity plane protecting `id`, stored as a region of kind
    /// `ecc_kind`.
    ///
    /// The plane is computed at [`Self::finish`], so this may be called at any
    /// point — before or after the region's contents are appended.
    ///
    /// A protected region gains one check byte per eight payload bytes, so the
    /// cost is 12.5% of the region's size, and single-bit faults in it become
    /// correctable rather than silently wrong.
    ///
    /// # Errors
    ///
    /// [`WireError::DuplicateRegion`] if `ecc_kind` collides with a declared or
    /// pending region kind, or if `id` already has a plane.
    /// [`WireError::TooManyRegions`] if adding the plane would exceed the
    /// ceiling.
    pub fn protect(&mut self, id: RegionId, ecc_kind: u16) -> Result<(), WireError> {
        if self.kind_in_use(ecc_kind) {
            return Err(WireError::DuplicateRegion { kind: ecc_kind });
        }
        if self.regions[id.0].ecc_kind.is_some() {
            return Err(WireError::DuplicateRegion {
                kind: self.regions[id.0].kind,
            });
        }
        // The ceiling applies to what will be EMITTED, which includes planes, not
        // to the count of declared regions.
        if self.emit_count() + 1 > MAX_REGIONS {
            return Err(WireError::TooManyRegions {
                found: self.emit_count() as u32 + 1,
            });
        }
        self.regions[id.0].ecc_kind = Some(ecc_kind);
        self.regions[id.0].flags |= FLAG_ECC_PRESENT;
        Ok(())
    }

    /// Generates a parity plane for every region declared so far.
    ///
    /// Each plane's kind comes from [`crate::ecc::plane_kind_for`], so the
    /// caller does not maintain a kind-per-region table. Call this LAST, after
    /// every region exists: a region declared afterwards is simply unprotected,
    /// silently, which is why [`Self::finish`] is the natural place to precede.
    ///
    /// Idempotent per region: a region that already has a plane is skipped
    /// rather than reported, so this composes with explicit [`Self::protect`]
    /// calls for regions wanting a non-default plane kind.
    ///
    /// # Cost
    ///
    /// One check byte per eight payload bytes, so 12.5% of the protected bytes,
    /// plus one directory entry per plane. The region ceiling counts planes, so
    /// this halves the number of payload regions an artifact may carry.
    ///
    /// # Errors
    ///
    /// [`WireError::TooManyRegions`] if the planes would exceed
    /// [`crate::layout::MAX_REGIONS`]. [`WireError::DuplicateRegion`] if a
    /// derived plane kind collides with a kind already in use, which can only
    /// happen for a payload region whose own kind has the high bit set.
    pub fn protect_all(&mut self) -> Result<(), WireError> {
        for i in 0..self.regions.len() {
            if self.regions[i].ecc_kind.is_some() {
                continue;
            }
            let plane = crate::ecc::plane_kind_for(self.regions[i].kind);
            self.protect(RegionId(i), plane)?;
        }
        Ok(())
    }

    /// True when `kind` is taken, by a declared region or a pending plane.
    fn kind_in_use(&self, kind: u16) -> bool {
        self.regions
            .iter()
            .any(|r| r.kind == kind || r.ecc_kind == Some(kind))
    }

    /// Regions that will actually be emitted, planes included.
    fn emit_count(&self) -> usize {
        self.regions.len() + self.regions.iter().filter(|r| r.ecc_kind.is_some()).count()
    }

    /// Appends bytes to a region. Forward-only; nothing is ever rewritten.
    pub fn push(&mut self, id: RegionId, bytes: &[u8]) {
        self.regions[id.0].bytes.extend_from_slice(bytes);
    }

    /// Appends one typed record, padded to its stride.
    ///
    /// This is the counterpart to [`crate::RecordTable::get_as`]: together they
    /// mean a caller declares the layout once, in a struct, rather than twice, in
    /// hand-written read and write offsets that can drift apart.
    pub fn push_record<T: WireRecord>(&mut self, id: RegionId, record: &T) {
        let start = self.regions[id.0].bytes.len();
        self.regions[id.0].bytes.resize(start + T::STRIDE, 0);
        // `write_record` cannot fail here: the slice was just sized to STRIDE.
        let _ = record.write_record(&mut self.regions[id.0].bytes[start..]);
    }

    /// Current length of a region in bytes, before word padding.
    ///
    /// This is how a caller records an offset for a reference it is about to
    /// emit — the pool offset of a string, say — without back-patching.
    pub fn len_of(&self, id: RegionId) -> usize {
        self.regions[id.0].bytes.len()
    }

    /// Lays out the artifact and returns its bytes.
    ///
    /// Regions are emitted in declaration order, each padded to a whole word,
    /// with each parity plane immediately after the region it protects.
    ///
    /// # Errors
    ///
    /// [`WireError::TooLarge`] or [`WireError::Overflow`] if the artifact would
    /// exceed the addressable size.
    pub fn finish(&self) -> Result<Vec<u8>, WireError> {
        // Materialise the emit list: every declared region, each followed by its
        // parity plane if it has one. Planes are the only allocation here; the
        // payloads are borrowed.
        let mut planes: Vec<Vec<u8>> = Vec::new();
        for r in &self.regions {
            if r.ecc_kind.is_some() {
                planes.push(parity_of(&r.bytes));
            }
        }

        // (kind, flags, covers, payload)
        let mut emit: Vec<(u16, u16, u16, &[u8])> = Vec::with_capacity(self.emit_count());
        let mut next_plane = 0;
        for r in &self.regions {
            emit.push((r.kind, r.flags, 0, r.bytes.as_slice()));
            if let Some(ecc_kind) = r.ecc_kind {
                emit.push((ecc_kind, FLAG_IS_ECC, r.kind, planes[next_plane].as_slice()));
                next_plane += 1;
            }
        }

        let n = emit.len();
        let header = header_bytes(n).ok_or(WireError::Overflow)?;
        let header_words = words_for(header).ok_or(WireError::Overflow)?;

        // Offsets are computed before any byte is written, so the directory can
        // lead. This is the step that per-region buffering buys.
        let mut offsets = Vec::with_capacity(n);
        let mut cursor = header_words;
        for (_, _, _, bytes) in &emit {
            let words = words_for(bytes.len()).ok_or(WireError::Overflow)?;
            offsets.push((cursor, words));
            cursor = cursor.checked_add(words).ok_or(WireError::Overflow)?;
        }

        let total = cursor.checked_mul(WORD).ok_or(WireError::Overflow)?;
        let mut out = Vec::with_capacity(total);

        // Prologue, three copies. The CRC covers the first twelve bytes, so a
        // vote that repairs one is confirmed rather than trusted.
        let mut prologue = [0u8; PROLOGUE_BYTES];
        prologue[0..4].copy_from_slice(&u32_bytes(MAGIC));
        prologue[4..6].copy_from_slice(&u16_bytes(BOM));
        prologue[6..8].copy_from_slice(&u16_bytes(FORMAT_VERSION));
        prologue[8..10].copy_from_slice(&u16_bytes(
            u16::try_from(n).map_err(|_| WireError::TooManyRegions { found: n as u32 })?,
        ));
        prologue[10..12].copy_from_slice(&u16_bytes(0));
        let check = crc32(&prologue[..12]);
        prologue[12..16].copy_from_slice(&u32_bytes(check));
        for _ in 0..COPIES {
            out.extend_from_slice(&prologue);
        }

        // Directory, three copies.
        let mut dir = Vec::with_capacity(n * DIR_ENTRY_BYTES);
        for ((kind, flags, covers, _), (word_off, word_len)) in emit.iter().zip(offsets.iter()) {
            dir.extend_from_slice(&u16_bytes(*kind));
            dir.extend_from_slice(&u16_bytes(*flags));
            dir.extend_from_slice(&u32_bytes(
                u32::try_from(*word_off).map_err(|_| WireError::TooLarge)?,
            ));
            dir.extend_from_slice(&u32_bytes(
                u32::try_from(*word_len).map_err(|_| WireError::TooLarge)?,
            ));
            dir.extend_from_slice(&u16_bytes(*covers));
            dir.extend_from_slice(&u16_bytes(0));
        }
        for _ in 0..COPIES {
            out.extend_from_slice(&dir);
        }

        // Pad the header area out to a word boundary, then the payloads.
        while out.len() % WORD != 0 {
            out.push(0);
        }
        debug_assert_eq!(out.len(), header_words * WORD);

        for (_, _, _, bytes) in &emit {
            out.extend_from_slice(bytes);
            while out.len() % WORD != 0 {
                out.push(0);
            }
        }

        Ok(out)
    }
}

/// Handle to a declared region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionId(usize);
