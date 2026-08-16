#![no_std]
#![deny(missing_docs)]
// This crate parses untrusted bytes. Every bounds check is written in safe Rust
// and the compiler enforces that there is no escape hatch, so "malformed input
// cannot cause memory unsafety" is a checked property rather than a claim.
#![forbid(unsafe_code)]
//! A word-oriented binary container format: fixed-size records, in-place reads,
//! and a triplicated directory, with **no dependency on what the records mean**.
//!
//! This crate provides the container only — framing, a region directory, fixed-
//! stride record tables, byte pools, and the integrity primitives. It does not
//! know what a "chunk" or a "constant" is. A schema layers on top by choosing
//! region kinds, record strides, and field offsets. That separation is what makes
//! the format reusable by a project with entirely different content.
//!
//! # What it is for
//!
//! It suits artifacts that are **written once and read many times, possibly much
//! later, on hardware that may be small or hostile**:
//!
//! - **Reads cost no allocation.** Every accessor returns a slice aliasing the
//!   caller's buffer. A reader needs no allocator at all — the `alloc` feature
//!   gates only the encoder.
//! - **Damage is survivable and reportable, in the header AND the payload.** The
//!   prologue and directory are stored three times and read by majority vote, so a
//!   corrupted copy is outvoted rather than fatal. Any region may additionally
//!   carry a [`ecc`] parity plane — (72,64) SECDED, one check byte per word — which
//!   corrects a single-bit fault in the *data* and detects a double. Both paths
//!   report the damage ([`WireView::needs_scrub`], [`EccReport::needs_scrub`])
//!   rather than silently repairing, because unreported damage accumulates until
//!   correction is overwhelmed.
//! - **Corruption cannot destroy framing.** No record carries a length prefix, so
//!   a flipped bit corrupts one field rather than desynchronising everything
//!   after it. Records are fixed-stride and offsets are word indices, so element
//!   *i* is at a shift from the base.
//! - **Decoding is total.** Malformed input is rejected, never panicked on.
//!
//! # Layout
//!
//! See [`layout`] for the byte-level structure and for why the prologue is split
//! from the directory (a bootstrapping problem: voting the directory requires a
//! region count that would otherwise live inside the thing being voted).
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "alloc")] {
//! use keleusma_wire::{WireBuilder, WireView};
//!
//! const STRINGS: u16 = 1;
//! const RECORDS: u16 = 2;
//!
//! let mut b = WireBuilder::new();
//! let pool = b.region(STRINGS, 0)?;
//! let table = b.region(RECORDS, 0)?;
//!
//! // Record the offset before appending, so nothing is back-patched.
//! let at = b.len_of(pool) as u32;
//! b.push(pool, b"alpha");
//!
//! let mut record = [0u8; 8];
//! record[0..4].copy_from_slice(&at.to_le_bytes());
//! record[4..8].copy_from_slice(&5u32.to_le_bytes());
//! b.push(table, &record);
//!
//! let artifact = b.finish()?;
//!
//! let view = WireView::parse(&artifact)?;
//! let pool = view.pool(&view.find_region(STRINGS).unwrap())?;
//! let table = view.records(&view.find_region(RECORDS).unwrap(), 8)?;
//!
//! let r = table.get(0).unwrap();
//! let off = u32::from_le_bytes(r[0..4].try_into().unwrap());
//! let len = u32::from_le_bytes(r[4..8].try_into().unwrap());
//! assert_eq!(pool.slice(off, len), Some(&b"alpha"[..]));
//! # }
//! # Ok::<(), keleusma_wire::WireError>(())
//! ```
//!
//! # Cargo features
//!
//! - `alloc` (default on): the encoder, [`WireBuilder`]. The reader does not
//!   need it. [`WireError`] implements `core::error::Error` unconditionally, so
//!   a `no_std` consumer still gets the standard error interface.
//! - `derive` (default off): `#[derive(WireRecord)]`, which generates a record's
//!   offset constants, stride, and total codec from a struct definition instead
//!   of leaving them to be counted by hand.
//!
//! # What this crate does not do
//!
//! - **No encryption.** A region carries an `ENCRYPTED` flag so a reader knows a
//!   payload cannot be read in place, but this crate neither encrypts nor
//!   decrypts.
//! - **No encryption of the parity plane.** A plane over an encrypted region
//!   protects the ciphertext, not the plaintext.
//! - **No multi-bit correction.** SECDED corrects one fault per word and detects
//!   two; three or more may be missed. The defence against accumulation is
//!   scrubbing, which is why damage is reported rather than silently repaired.
//! - **No schema.** Record strides and field offsets are the caller's.

// The README's examples are compiled and run as doctests, so the front page
// cannot drift away from the API it advertises.
//
// Gated on the features the examples USE. Every one of them builds an artifact,
// which needs the encoder, and one derives a record -- so under
// `--no-default-features` they cannot compile, and an unconditional include made
// the allocator-free build fail. The examples are still verified, in the
// all-features run; what is gated is where they run, not whether.
#[cfg(all(doctest, feature = "alloc", feature = "derive"))]
#[doc = include_str!("../README.md")]
struct ReadmeExamples;

#[cfg(feature = "alloc")]
mod build;
mod crc;
pub mod ecc;
mod error;
pub mod layout;
mod record;
pub mod scalar;
mod view;

#[cfg(feature = "alloc")]
pub use build::{RegionId, WireBuilder};
pub use crc::crc32;
pub use ecc::{ECC_KIND_BIT, EccPlane, EccReport, WordStatus, plane_kind_for};
pub use error::WireError;
pub use layout::Region;
pub use record::WireRecord;
pub use view::{Pool, RecordTable, WireView, scrub};

#[cfg(feature = "derive")]
pub use keleusma_wire_derive::WireRecord;
