//! The error type. One enum, no allocation, no source chaining.

use core::fmt;

/// Why an artifact could not be read or built.
///
/// Deliberately coarse. A decoder that distinguishes twenty failure modes tempts
/// a caller into recovering from some of them; for a container format the only
/// safe response to a malformed artifact is to reject it, so the variants exist
/// to aid diagnosis rather than control flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WireError {
    /// The buffer is shorter than the structure it claims to contain.
    Truncated,
    /// No prologue copy carried the expected magic, even after voting.
    BadMagic,
    /// The byte-order marker indicates an artifact this build cannot read
    /// in place.
    ForeignEndian,
    /// The format version is not one this crate understands.
    UnsupportedVersion {
        /// The version found in the artifact.
        found: u16,
    },
    /// The prologue's CRC did not match, and voting did not repair it.
    BadPrologueChecksum,
    /// `region_count` exceeds [`crate::layout::MAX_REGIONS`].
    TooManyRegions {
        /// The count found in the artifact.
        found: u32,
    },
    /// A region's payload lies outside the buffer.
    RegionOutOfBounds {
        /// Index of the offending directory entry.
        index: u16,
    },
    /// Two regions of the same kind appear in one directory.
    DuplicateRegion {
        /// The repeated kind.
        kind: u16,
    },
    /// A record table's length is not a whole multiple of its stride, or the
    /// stride is zero.
    BadRecordStride,
    /// An arithmetic step would overflow the address space.
    Overflow,
    /// The artifact would exceed the addressable size while being built.
    TooLarge,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "buffer is shorter than the structure it declares"),
            Self::BadMagic => write!(f, "container magic not found"),
            Self::ForeignEndian => write!(f, "artifact byte order does not match this build"),
            Self::UnsupportedVersion { found } => {
                write!(f, "unsupported container version {found}")
            }
            Self::BadPrologueChecksum => write!(f, "prologue checksum mismatch"),
            Self::TooManyRegions { found } => write!(f, "region count {found} exceeds the maximum"),
            Self::RegionOutOfBounds { index } => {
                write!(f, "region {index} payload lies outside the buffer")
            }
            Self::DuplicateRegion { kind } => write!(f, "duplicate region kind {kind}"),
            Self::BadRecordStride => write!(f, "record region length is not a multiple of stride"),
            Self::Overflow => write!(f, "offset arithmetic overflowed"),
            Self::TooLarge => write!(f, "artifact exceeds the addressable size"),
        }
    }
}

// `core::error::Error` rather than `std::error::Error`, so the impl is available
// to `no_std` consumers too and needs no feature gate at all.
impl core::error::Error for WireError {}
