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
