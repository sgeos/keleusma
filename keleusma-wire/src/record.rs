//! Typed fixed-size records.
//!
//! The container itself never learns what a record means: [`WireRecord`] is
//! implemented by the *caller's* type, and the container only ever asks for a
//! stride and hands back bytes. The optional derive writes the offsets so they do
//! not have to be counted by hand, which is where they get miscounted.
//!
//! # This does not weaken the in-place read property
//!
//! [`WireRecord::read_record`] returns an owned value, which sounds like the owned
//! decode the reader's documentation warns against. It is not the same thing. A
//! record is a fixed, small set of scalars — copying sixteen bytes into registers
//! is what field-by-field access already did. The property that matters is that
//! **variable-length** data is not materialised, and it still is not: a string
//! stays a subslice of its pool, and nothing here allocates.
//!
//! If even the scalar copy is unwanted, the derive also emits `OFFSET_*`
//! constants, so a caller can read one field in place and ignore the rest.

/// A fixed-size record with a known stride and total codec.
///
/// Implement by hand, or derive with `#[derive(WireRecord)]` under the `derive`
/// feature.
///
/// # Contract
///
/// - `STRIDE` must be a whole number of [`crate::layout::WORD`] bytes, so that
///   element *i* of a table is at a power-of-two stride.
/// - `read_record` must be **total**: any slice, including a short one, yields
///   `None` rather than panicking.
/// - `write_record` must write exactly the bytes `read_record` would consume.
pub trait WireRecord: Sized {
    /// Bytes per record, padded to a whole word.
    const STRIDE: usize;

    /// Decodes one record. Returns `None` if `bytes` is shorter than `STRIDE`.
    fn read_record(bytes: &[u8]) -> Option<Self>;

    /// Encodes into `out`. Returns `None` if `out` is shorter than `STRIDE`.
    fn write_record(&self, out: &mut [u8]) -> Option<()>;
}
