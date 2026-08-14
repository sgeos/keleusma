//! The reader: a **borrowed view** over an artifact.
//!
//! Nothing here allocates and nothing is copied out of the buffer. Every accessor
//! returns a slice that aliases the caller's bytes, so a payload can be used in
//! place — a string stays a direct subslice, and a record table is addressed by
//! arithmetic rather than materialised.
//!
//! That is a load-bearing property, not an optimisation. An owned decode would
//! allocate once per artifact load, which defeats a worst-case-memory bound and
//! silently removes the reason the format is shaped this way. If a future change
//! makes an accessor return owned data, the property is gone and no test that
//! merely checks values would notice — so it is stated here explicitly.

use crate::crc::crc32;
use crate::ecc::{EccPlane, EccReport};
use crate::error::WireError;
use crate::layout::{
    BOM, COPIES, DIR_ENTRY_BYTES, DIRECTORY_OFFSET, FORMAT_VERSION, MAGIC, MAX_REGIONS,
    PROLOGUE_BYTES, Region, WORD, header_bytes,
};
use crate::record::WireRecord;
use crate::scalar::{maj3, u16_at, u32_at};

/// A parsed, validated, borrowed view over an artifact.
#[derive(Debug, Clone, Copy)]
pub struct WireView<'a> {
    bytes: &'a [u8],
    region_count: u16,
    prologue_disagreed: bool,
    directory_disagreed: bool,
}

impl<'a> WireView<'a> {
    /// Parses and validates an artifact.
    ///
    /// Validation is done once, here, so that every later accessor is
    /// total by construction rather than by repeated re-checking. On success,
    /// every directory entry is known to describe a payload inside `bytes`.
    ///
    /// # Errors
    ///
    /// Returns a [`WireError`] for any malformed artifact. This function never
    /// panics, regardless of input.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, WireError> {
        if bytes.len() < PROLOGUE_BYTES * COPIES {
            return Err(WireError::Truncated);
        }

        // The prologue is votable with no prior knowledge, because its three
        // copies are at fixed offsets. See the module docs on `layout`.
        let mut prologue = [0u8; PROLOGUE_BYTES];
        let mut prologue_disagreed = false;
        for (i, slot) in prologue.iter_mut().enumerate() {
            let a = bytes[i];
            let b = bytes[PROLOGUE_BYTES + i];
            let c = bytes[2 * PROLOGUE_BYTES + i];
            let v = maj3(a, b, c);
            if a != v || b != v || c != v {
                prologue_disagreed = true;
            }
            *slot = v;
        }

        if u32_at(&prologue, 0) != Some(MAGIC) {
            return Err(WireError::BadMagic);
        }
        match u16_at(&prologue, 4) {
            Some(BOM) => {}
            Some(_) => return Err(WireError::ForeignEndian),
            None => return Err(WireError::Truncated),
        }
        let version = u16_at(&prologue, 6).ok_or(WireError::Truncated)?;
        if version != FORMAT_VERSION {
            return Err(WireError::UnsupportedVersion { found: version });
        }
        // The check covers the prologue's own fields, so a vote that repaired a
        // byte is confirmed rather than assumed correct.
        let want = u32_at(&prologue, 12).ok_or(WireError::Truncated)?;
        if crc32(&prologue[..12]) != want {
            return Err(WireError::BadPrologueChecksum);
        }

        let region_count = u16_at(&prologue, 8).ok_or(WireError::Truncated)?;
        if region_count as usize > MAX_REGIONS {
            return Err(WireError::TooManyRegions {
                found: region_count as u32,
            });
        }

        let header = header_bytes(region_count as usize).ok_or(WireError::Overflow)?;
        if bytes.len() < header {
            return Err(WireError::Truncated);
        }

        let mut view = Self {
            bytes,
            region_count,
            prologue_disagreed,
            directory_disagreed: false,
        };
        view.directory_disagreed = view.directory_has_disagreement();
        view.validate_regions()?;
        Ok(view)
    }

    /// Number of regions in the directory.
    #[inline]
    pub fn region_count(&self) -> u16 {
        self.region_count
    }

    /// The artifact's bytes.
    #[inline]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// True when at least one redundant copy disagreed with the majority.
    ///
    /// The artifact was still read correctly — that is what the vote is for — but
    /// it has taken damage, and rewriting it restores the margin that lets the
    /// next fault also be survivable. Treat this as a scrub trigger, not a
    /// failure: unreported damage accumulates silently until a second fault in
    /// the same position defeats the vote.
    #[inline]
    pub fn needs_scrub(&self) -> bool {
        self.prologue_disagreed || self.directory_disagreed
    }

    /// Reads one voted byte of directory copy-space.
    #[inline]
    fn dir_byte(&self, index: u16, offset: usize) -> u8 {
        let dir_span = self.region_count as usize * DIR_ENTRY_BYTES;
        let at = index as usize * DIR_ENTRY_BYTES + offset;
        let a = self.bytes[DIRECTORY_OFFSET + at];
        let b = self.bytes[DIRECTORY_OFFSET + dir_span + at];
        let c = self.bytes[DIRECTORY_OFFSET + 2 * dir_span + at];
        maj3(a, b, c)
    }

    fn directory_has_disagreement(&self) -> bool {
        let dir_span = self.region_count as usize * DIR_ENTRY_BYTES;
        for at in 0..dir_span {
            let a = self.bytes[DIRECTORY_OFFSET + at];
            let b = self.bytes[DIRECTORY_OFFSET + dir_span + at];
            let c = self.bytes[DIRECTORY_OFFSET + 2 * dir_span + at];
            let v = maj3(a, b, c);
            if a != v || b != v || c != v {
                return true;
            }
        }
        false
    }

    /// Returns directory entry `index`, or `None` if out of range.
    pub fn region_at(&self, index: u16) -> Option<Region> {
        if index >= self.region_count {
            return None;
        }
        let mut e = [0u8; DIR_ENTRY_BYTES];
        for (o, slot) in e.iter_mut().enumerate() {
            *slot = self.dir_byte(index, o);
        }
        Some(Region {
            kind: u16_at(&e, 0)?,
            flags: u16_at(&e, 2)?,
            word_offset: u32_at(&e, 4)?,
            word_length: u32_at(&e, 8)?,
            covers: u16_at(&e, 12)?,
        })
    }

    /// Finds the parity plane protecting `region`, if the artifact carries one.
    ///
    /// Returns `None` when the region is unprotected, which is the normal case
    /// for an artifact built without ECC — the plane is purely additive, so its
    /// absence is not an error.
    pub fn ecc_for(&self, region: &Region) -> Option<EccPlane<'a>> {
        for i in 0..self.region_count {
            let candidate = self.region_at(i)?;
            if candidate.is_ecc_plane() && candidate.covers == region.kind {
                return self.region_bytes(&candidate).ok().map(EccPlane::new);
            }
        }
        None
    }

    /// Scans a region against its parity plane.
    ///
    /// Returns `None` when the region has no plane. A clean report means every
    /// word decoded without a syndrome; see [`EccReport::needs_scrub`] for the
    /// repairable-damage signal.
    pub fn verify_region(&self, region: &Region) -> Option<EccReport> {
        let plane = self.ecc_for(region)?;
        let data = self.region_bytes(region).ok()?;
        Some(plane.scan(data))
    }

    /// Scans every protected region against its plane and sums the outcome.
    ///
    /// Returns `None` when the artifact carries no planes at all, which is the
    /// normal case and is deliberately distinguished from a clean scan of zero
    /// protected regions. A caller that treated "no ECC" as "verified" would be
    /// reporting an unprotected artifact as sound.
    ///
    /// A plane region is not itself scanned. Protecting the parity of the parity
    /// is not what SECDED buys, and a fault in a plane surfaces as a false
    /// syndrome on the region it covers rather than as silent acceptance.
    pub fn verify_all(&self) -> Option<EccReport> {
        let mut total: Option<EccReport> = None;
        for i in 0..self.region_count {
            let Some(r) = self.region_at(i) else { continue };
            if r.is_ecc_plane() || !r.has_ecc() {
                continue;
            }
            let Some(rep) = self.verify_region(&r) else {
                continue;
            };
            let acc = total.get_or_insert(EccReport {
                words: 0,
                corrected: 0,
                uncorrectable: 0,
            });
            acc.words += rep.words;
            acc.corrected += rep.corrected;
            acc.uncorrectable += rep.uncorrectable;
        }
        total
    }

    /// Finds the single region of the given kind.
    ///
    /// `parse` rejects duplicate kinds, so this is unambiguous.
    pub fn find_region(&self, kind: u16) -> Option<Region> {
        for i in 0..self.region_count {
            let r = self.region_at(i)?;
            if r.kind == kind {
                return Some(r);
            }
        }
        None
    }

    /// Checks every entry lies inside the buffer and no kind repeats.
    ///
    /// The duplicate scan is quadratic in the region count, which is bounded by
    /// [`MAX_REGIONS`], and runs once per artifact rather than per access.
    fn validate_regions(&self) -> Result<(), WireError> {
        for i in 0..self.region_count {
            let r = self
                .region_at(i)
                .ok_or(WireError::RegionOutOfBounds { index: i })?;
            let start = r.byte_offset().ok_or(WireError::Overflow)?;
            let len = r.byte_length().ok_or(WireError::Overflow)?;
            let end = start.checked_add(len).ok_or(WireError::Overflow)?;
            if end > self.bytes.len() {
                return Err(WireError::RegionOutOfBounds { index: i });
            }
            for j in (i + 1)..self.region_count {
                let other = self
                    .region_at(j)
                    .ok_or(WireError::RegionOutOfBounds { index: j })?;
                if other.kind == r.kind {
                    return Err(WireError::DuplicateRegion { kind: r.kind });
                }
            }
        }
        Ok(())
    }

    /// Borrows a region's payload. The slice aliases the artifact.
    ///
    /// # Errors
    ///
    /// [`WireError::RegionOutOfBounds`] if the region does not belong to this
    /// artifact.
    pub fn region_bytes(&self, region: &Region) -> Result<&'a [u8], WireError> {
        let start = region.byte_offset().ok_or(WireError::Overflow)?;
        let len = region.byte_length().ok_or(WireError::Overflow)?;
        let end = start.checked_add(len).ok_or(WireError::Overflow)?;
        self.bytes
            .get(start..end)
            .ok_or(WireError::RegionOutOfBounds { index: 0 })
    }

    /// Views a region as a table of fixed-size records.
    ///
    /// # Errors
    ///
    /// [`WireError::BadRecordStride`] if `stride` is zero, is not a whole number
    /// of words, or does not divide the region length.
    pub fn records(&self, region: &Region, stride: usize) -> Result<RecordTable<'a>, WireError> {
        if stride == 0 || stride % WORD != 0 {
            return Err(WireError::BadRecordStride);
        }
        let bytes = self.region_bytes(region)?;
        if bytes.len() % stride != 0 {
            return Err(WireError::BadRecordStride);
        }
        Ok(RecordTable { bytes, stride })
    }

    /// Views a region as a table of `T`, using `T`'s own stride.
    ///
    /// # Errors
    ///
    /// [`WireError::BadRecordStride`] if the region length is not a whole
    /// multiple of `T::STRIDE`.
    pub fn typed_records<T: WireRecord>(
        &self,
        region: &Region,
    ) -> Result<RecordTable<'a>, WireError> {
        self.records(region, T::STRIDE)
    }

    /// Views a region as a flat byte pool.
    ///
    /// # Errors
    ///
    /// Propagates a bounds failure from [`Self::region_bytes`].
    pub fn pool(&self, region: &Region) -> Result<Pool<'a>, WireError> {
        Ok(Pool {
            bytes: self.region_bytes(region)?,
        })
    }
}

/// A region viewed as fixed-size records. Element *i* is at `i * stride`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordTable<'a> {
    bytes: &'a [u8],
    stride: usize,
}

impl<'a> RecordTable<'a> {
    /// Views `bytes` as records of `stride`.
    ///
    /// For a caller that has already resolved a region's byte range and wants to
    /// rebuild the table without walking the directory again — validate once,
    /// then reconstruct cheaply.
    ///
    /// Returns `None` if `stride` is zero, is not a whole number of words, or
    /// does not divide `bytes.len()`; the same conditions
    /// [`WireView::records`] enforces, so a table built this way is
    /// indistinguishable from one obtained through the directory.
    #[inline]
    pub fn from_bytes(bytes: &'a [u8], stride: usize) -> Option<Self> {
        if stride == 0 || stride % WORD != 0 || bytes.len() % stride != 0 {
            return None;
        }
        Some(Self { bytes, stride })
    }

    /// Number of records.
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len() / self.stride
    }

    /// True when the table holds no records.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Bytes per record.
    #[inline]
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Borrows record `index`. The slice aliases the artifact.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&'a [u8]> {
        let start = index.checked_mul(self.stride)?;
        let end = start.checked_add(self.stride)?;
        self.bytes.get(start..end)
    }

    /// Decodes record `index` as `T`.
    ///
    /// Returns `None` if the index is out of range, if the table's stride does
    /// not match `T::STRIDE`, or if `T` rejects the bytes. The stride check is
    /// the point: it catches a table opened with the wrong record type, which
    /// would otherwise read plausible values from the wrong offsets.
    #[inline]
    pub fn get_as<T: WireRecord>(&self, index: usize) -> Option<T> {
        if self.stride != T::STRIDE {
            return None;
        }
        T::read_record(self.get(index)?)
    }

    /// Checks a composite range reference against the **forward-ordering
    /// invariant**.
    ///
    /// A record that references a range of other records in the same table must
    /// reference one that lies strictly after it. Under that ordering the whole
    /// table can be walked bottom-up by a single reverse linear sweep, with no
    /// stack and a statically bounded trip count.
    ///
    /// This is offered as a checked operation because **its violation is
    /// silent**. A range pointing backwards makes a reverse sweep read entries
    /// it has not computed yet, which yields a wrong answer rather than a fault
    /// — the failure mode that gets shipped. A schema that uses range references
    /// should call this while validating, not assume its encoder got it right.
    #[inline]
    pub fn range_is_forward(&self, at: usize, first: u32, count: u32) -> bool {
        let first = first as usize;
        let Some(end) = first.checked_add(count as usize) else {
            return false;
        };
        // A zero-length range is vacuously fine wherever it points, but an
        // out-of-table one never is.
        first > at && end <= self.len()
    }
}

/// A region viewed as a flat byte pool. A slice out of it is contiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pool<'a> {
    bytes: &'a [u8],
}

impl<'a> Pool<'a> {
    /// Views `bytes` as a byte pool.
    ///
    /// The counterpart to [`RecordTable::from_bytes`], for a caller rebuilding a
    /// view from an already-resolved region range.
    #[inline]
    pub fn from_bytes(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    /// The whole pool.
    #[inline]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// Borrows `len` bytes at `offset`. The slice aliases the artifact, so a
    /// string resolved this way costs no copy.
    #[inline]
    pub fn slice(&self, offset: u32, len: u32) -> Option<&'a [u8]> {
        let start = offset as usize;
        let end = start.checked_add(len as usize)?;
        self.bytes.get(start..end)
    }
}

/// Repairs every protected region of a wire CONTAINER in place.
///
/// `bytes` is a container as [`WireView::parse`] accepts one. A caller whose
/// container is embedded in a larger framing slices it out first; handing this
/// the outer buffer makes the parse fail on the magic and the scrub returns
/// `None` having repaired nothing.
///
/// This is the **scrub** verb, the mutating counterpart to the reporting verbs
/// [`WireView::verify_all`] and [`WireView::verify_region`]. It returns `None`
/// when the artifact carries no parity plane, exactly as `verify_all` does, so
/// a caller cannot mistake an unprotected artifact for a repaired one.
///
/// # The ordering invariant, which this signature exists to protect
///
/// **A repair must precede the check that authorises the bytes it produced, and
/// every later repair must be followed by a fresh check.** The invariant is that
/// no byte is executed which has been modified since the last successful
/// verification, and a scrub is a modification.
///
/// That is not advice. It is measured. Enumerated over one 64-bit word, a
/// (72,64) code repairs all 64 single-bit patterns exactly and detects all 2,016
/// double-bit patterns, but **reports 23,364 of 41,664 triple-bit patterns as a
/// successful repair while producing the wrong word**. This function therefore
/// hands back **counts, not an artifact**: there is nothing here for a caller to
/// load, and the repaired bytes must be re-authenticated by whatever authorised
/// them originally.
///
/// # What the signature buys on the zero-copy path
///
/// Taking `&mut [u8]` makes the unsound order **unrepresentable** wherever the
/// reader borrows the buffer: a live [`WireView`], or a virtual machine reading
/// the artifact in place, holds `&[u8]`, so `&mut [u8]` cannot be obtained while
/// either exists. Scrubbing must happen before the reader is constructed, and
/// constructing it again re-runs whatever checks it performs.
///
/// **The guarantee is weaker where the artifact is copied out.** A consumer that
/// decodes into owned structures no longer borrows the buffer, so nothing
/// prevents a scrub after the check, and the invariant above must be honoured by
/// the caller.
///
/// # Cost
///
/// One linear pass over the protected regions, no allocation, and bounded by the
/// artifact's own region count and lengths. Nothing is written unless a word
/// decodes as corrected, so a clean artifact is left byte-identical.
pub fn scrub(bytes: &mut [u8]) -> Option<EccReport> {
    // Whether any plane exists at all, decided before anything is written.
    let region_count = WireView::parse(bytes).ok()?.region_count();
    let mut any_plane = false;
    let mut total = EccReport {
        words: 0,
        corrected: 0,
        uncorrectable: 0,
    };

    for i in 0..region_count {
        // The view borrows `bytes` immutably, so its spans are copied out as
        // plain integers and the view is dropped BEFORE anything is written.
        // Holding it across the write would not compile, which is the point.
        let spans = {
            let view = WireView::parse(bytes).ok()?;
            let Some(r) = view.region_at(i) else { continue };
            if r.is_ecc_plane() || !r.has_ecc() {
                continue;
            }
            let (Some(base), Ok(data)) = (r.byte_offset(), view.region_bytes(&r)) else {
                continue;
            };
            // Locate the plane by the kind it covers, as `ecc_for` does, and
            // take its offset rather than its bytes so nothing stays borrowed.
            let mut plane: Option<(usize, usize)> = None;
            for j in 0..view.region_count() {
                let Some(c) = view.region_at(j) else { continue };
                if c.is_ecc_plane() && c.covers == r.kind {
                    if let (Some(pb), Ok(pd)) = (c.byte_offset(), view.region_bytes(&c)) {
                        plane = Some((pb, pd.len()));
                    }
                    break;
                }
            }
            plane.map(|(pb, pl)| (base, data.len(), pb, pl))
        };
        let Some((base, len, plane_base, plane_len)) = spans else {
            continue;
        };
        any_plane = true;

        for w in 0..len.div_ceil(8) {
            let at = base + w * 8;
            let Some(check) = bytes.get(plane_base + w).copied() else {
                break;
            };
            if w >= plane_len {
                break;
            }
            // The last word of a region may be partial; the encoder computed its
            // parity over the zero-padded word, so the decode must match that.
            let mut word = [0u8; 8];
            let end = core::cmp::min(at + 8, base + len);
            let Some(src) = bytes.get(at..end) else { break };
            word[..end - at].copy_from_slice(src);
            let value = u64::from_le_bytes(word);
            total.words += 1;
            match crate::ecc::decode_word(value, check) {
                crate::ecc::WordStatus::Corrected(fixed) => {
                    total.corrected += 1;
                    let le = fixed.to_le_bytes();
                    bytes[at..end].copy_from_slice(&le[..end - at]);
                }
                crate::ecc::WordStatus::Uncorrectable => total.uncorrectable += 1,
                crate::ecc::WordStatus::Clean(_) => {}
            }
        }
    }

    if any_plane { Some(total) } else { None }
}

#[cfg(all(test, feature = "alloc"))]
mod scrub_tests {
    extern crate alloc;

    use super::*;
    use crate::WireBuilder;
    use alloc::vec::Vec;

    /// An artifact with one protected region of known bytes.
    fn artifact() -> Vec<u8> {
        let mut b = WireBuilder::new();
        let id = b.region(0x0010, 0).expect("region");
        b.push(
            id,
            &[1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        );
        b.protect_all().expect("protect");
        b.finish().expect("finish")
    }

    #[test]
    fn an_unprotected_artifact_reports_none_rather_than_a_clean_scrub() {
        let mut b = WireBuilder::new();
        let id = b.region(0x0010, 0).expect("region");
        b.push(id, &[1u8, 2, 3, 4, 5, 6, 7, 8]);
        let mut bytes = b.finish().expect("finish");
        let before = bytes.clone();
        assert!(
            scrub(&mut bytes).is_none(),
            "an artifact with no plane must report None, since a caller treating a clean \
             report as verified would call an unprotected artifact sound"
        );
        assert_eq!(
            bytes, before,
            "scrubbing an unprotected artifact must not write"
        );
    }

    #[test]
    fn scrubbing_a_clean_artifact_is_the_identity() {
        let mut bytes = artifact();
        let before = bytes.clone();
        let report = scrub(&mut bytes).expect("planes present");
        assert!(
            report.is_clean(),
            "a fresh artifact reported faults: {report:?}"
        );
        assert!(
            report.words > 0,
            "the scrub examined zero words, so it measured nothing"
        );
        assert_eq!(
            bytes, before,
            "scrubbing an undamaged artifact changed it, which would break every consumer \
             that authenticates the bytes"
        );
    }

    #[test]
    fn a_single_fault_is_repaired_in_place_and_exactly() {
        let clean = artifact();
        // Every bit of the first two bytes of the protected payload.
        let base = {
            let v = WireView::parse(&clean).expect("parses");
            let r = v.region_at(0).expect("region");
            r.byte_offset().expect("offset")
        };
        for off in [0usize, 1, 8] {
            for bit in 0..8u32 {
                let mut damaged = clean.clone();
                damaged[base + off] ^= 1 << bit;
                let report = scrub(&mut damaged).expect("planes present");
                assert_eq!(
                    (report.corrected, report.uncorrectable),
                    (1, 0),
                    "byte {off} bit {bit}: expected exactly one corrected word"
                );
                assert_eq!(
                    damaged, clean,
                    "byte {off} bit {bit}: the repair did not reproduce the original bytes \
                     exactly, so no signature over the original could verify against it"
                );
            }
        }
    }

    #[test]
    fn a_double_fault_is_reported_uncorrectable_and_not_silently_repaired() {
        let clean = artifact();
        let base = {
            let v = WireView::parse(&clean).expect("parses");
            v.region_at(0)
                .expect("region")
                .byte_offset()
                .expect("offset")
        };
        for (a, b) in [(0u32, 1u32), (0, 7), (3, 4)] {
            let mut damaged = clean.clone();
            damaged[base] ^= (1 << a) | (1 << b);
            let report = scrub(&mut damaged).expect("planes present");
            assert_eq!(
                (report.corrected, report.uncorrectable),
                (0, 1),
                "bits {a},{b}: a double fault must be reported uncorrectable and NOT repaired, \
                 since a silently wrong repair is the dangerous failure"
            );
        }
    }

    /// A clean report is NOT an integrity check, and the count that proves it.
    ///
    /// Weight-four codewords exist in a distance-four code, so an error pattern
    /// that IS a codeword passes with a zero syndrome. A caller that skipped a
    /// cryptographic check because the scrub came back clean would accept this.
    #[test]
    fn a_clean_report_does_not_mean_the_artifact_is_undamaged() {
        let clean = artifact();
        let base = {
            let v = WireView::parse(&clean).expect("parses");
            v.region_at(0)
                .expect("region")
                .byte_offset()
                .expect("offset")
        };
        // Search the weight-four patterns of one byte-pair for an invisible one.
        let mut found = false;
        for i in 0..64u32 {
            for j in (i + 1)..64 {
                for k in (j + 1)..64 {
                    for l in (k + 1)..64 {
                        let e = (1u64 << i) | (1u64 << j) | (1u64 << k) | (1u64 << l);
                        if crate::ecc::check_byte(e) != 0 {
                            continue;
                        }
                        let mut damaged = clean.clone();
                        let word = u64::from_le_bytes(
                            damaged[base..base + 8].try_into().expect("8 bytes"),
                        );
                        damaged[base..base + 8].copy_from_slice(&(word ^ e).to_le_bytes());
                        let report = scrub(&mut damaged).expect("planes present");
                        assert!(
                            report.is_clean(),
                            "a weight-four codeword error must decode CLEAN, which is what makes \
                             a clean report unusable as an integrity check"
                        );
                        assert_ne!(
                            damaged, clean,
                            "the artifact is genuinely damaged and the scrub reported clean"
                        );
                        found = true;
                        break;
                    }
                    if found {
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            if found {
                break;
            }
        }
        assert!(
            found,
            "no weight-four codeword was found, so this test never exercised the case it exists \
             for and the claim that a clean report is not an integrity check is unsupported here"
        );
    }
}
