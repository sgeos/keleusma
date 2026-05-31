//! Strippable debug metadata (backlog item B29).
//!
//! Debug information in Keleusma lives entirely in an optional,
//! chunk-local section and never in the op stream. This module defines
//! that section, [`crate::debug_meta::DebugPool`], and its canonical
//! byte encoding. The
//! design and rationale are recorded in B29 of
//! `docs/decisions/BACKLOG.md`; the short version is that keeping debug
//! markers out of the op stream makes the op stream byte-identical
//! between a debug build and a release build, so stripping debug
//! information is the removal of a separable section rather than a
//! transform of the program.
//!
//! # Status
//!
//! This is the foundational data model and serialization for B29. It is
//! parallel infrastructure: defined and tested in isolation, not yet
//! attached to [`crate::bytecode::Chunk`] nor emitted by the compiler.
//! Subsequent increments wire the optional section into the chunk wire
//! format, emit records from the compiler, and add the `keleusma strip`
//! tool. The module mirrors the established pattern of
//! [`crate::flat_value`], [`crate::value_layout`], and
//! [`crate::zero_value`], which are likewise staged ahead of their
//! consumers.
//!
//! # Encoding
//!
//! The encoding matches the runtime wire format's conventions:
//! little-endian integers and `u32` length prefixes. Strings are a
//! `u32` byte length followed by UTF-8 bytes. The four sub-pools are
//! emitted in a fixed order (strings, spans, types, records).
//!
//! # Determinism
//!
//! [`DebugPool::encode`](crate::debug_meta::DebugPool::encode) sorts
//! the record pool into a canonical order
//! (by op index, then kind, then operands) before emission, so the same
//! logical set of records produces byte-identical output regardless of
//! the order in which the producer appended them. The three data
//! sub-pools are emitted in their stored order because records reference
//! their entries by index; a producer that wants end-to-end
//! byte-determinism must build those sub-pools deterministically.
//! Sorting the records does not disturb sub-pool indices, since each
//! record carries those indices in its operands.

use alloc::string::String;
use alloc::vec::Vec;

/// The kind of a [`DebugRecord`]. Serialized as a single byte. The
/// discriminants are stable wire values and must not be renumbered;
/// new kinds append at the end. The catalogue corresponds to the
/// candidate record kinds in B29.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum DebugRecordKind {
    /// Per-call-site source position for stack traces.
    CallSite = 0,
    /// Per-op source position for fault highlighting.
    SourceSpan = 1,
    /// Per-op source line, coarser and cheaper than a span.
    LineNumber = 2,
    /// Per-local-slot human-readable name.
    VariableName = 3,
    /// Per-stack-position type for debugger introspection.
    TypeAnnotation = 4,
    /// Structured context for an assertion failure.
    AssertionContext = 5,
    /// A source-level position at which a breakpoint may be set.
    BreakpointCandidate = 6,
    /// Which monomorphisation a chunk was generated from.
    GenericInstantiation = 7,
    /// Per-position information-flow label audit information.
    IfcLabelAnnotation = 8,
    /// Per-block declared worst-case-execution-time annotation.
    WcetMarker = 9,
    /// Which optimisations the compiler applied to a region.
    OptimisationMarker = 10,
    /// Audit-grade trace of a verifier acceptance.
    VerifierWitness = 11,
}

impl DebugRecordKind {
    /// The wire byte for this kind.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Lift a wire byte into a kind, returning [`None`] for an
    /// unrecognized value.
    pub fn from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            0 => Self::CallSite,
            1 => Self::SourceSpan,
            2 => Self::LineNumber,
            3 => Self::VariableName,
            4 => Self::TypeAnnotation,
            5 => Self::AssertionContext,
            6 => Self::BreakpointCandidate,
            7 => Self::GenericInstantiation,
            8 => Self::IfcLabelAnnotation,
            9 => Self::WcetMarker,
            10 => Self::OptimisationMarker,
            11 => Self::VerifierWitness,
            _ => return None,
        })
    }
}

/// A source span resolved through the string sub-pool:
/// `(file_string_index, byte_offset, byte_length)`.
pub type Span = (u16, u32, u32);

/// A single op-index-keyed debug annotation. The record names the
/// op-stream position it annotates and carries `u16` indices into the
/// [`DebugPool`] sub-pools through `operands`; the meaning of each
/// operand is fixed per [`DebugRecordKind`] and is interpreted by the
/// consuming tool, not by this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugRecord {
    /// The op-stream position this record annotates.
    pub op_index: u32,
    /// The kind of annotation.
    pub kind: DebugRecordKind,
    /// Operand indices into the [`DebugPool`] sub-pools, interpreted
    /// per `kind`.
    pub operands: Vec<u16>,
}

/// The chunk-local debug metadata section: three data sub-pools holding
/// variable-length payloads, plus the record pool of op-index-keyed
/// markers that reference them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugPool {
    /// Interned UTF-8 strings (file names, variable names, ...).
    pub string_pool: Vec<String>,
    /// Source spans referenced by records.
    pub span_pool: Vec<Span>,
    /// Compact type representations referenced by `TypeAnnotation`
    /// records. Held as opaque length-prefixed byte blobs in this
    /// foundational increment; the concrete `TypeRepr` encoding is
    /// defined when `TypeAnnotation` emission lands.
    pub type_pool: Vec<Vec<u8>>,
    /// The op-index-keyed annotation records.
    pub records: Vec<DebugRecord>,
}

/// A source location resolved from a debug record through the pool's
/// sub-pools: the file name (when the string pool carries one) and the
/// byte range in that file. Returned by the read API on [`DebugPool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation<'a> {
    /// The source file name, or `None` when the referenced string-pool
    /// entry is absent. The compiler emits an empty placeholder string
    /// for the file name (it does not know the source path), so this is
    /// commonly `Some("")` until a host rewrites it.
    pub file: Option<&'a str>,
    /// Byte offset of the location's start in the file.
    pub byte_offset: u32,
    /// Byte length of the location's span.
    pub byte_length: u32,
}

/// Failure decoding a [`DebugPool`] from bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugMetaError {
    /// The input ended before a field that the format requires.
    Truncated,
    /// A record carried a kind byte outside the known catalogue.
    UnknownRecordKind(u8),
    /// A string sub-pool entry was not valid UTF-8.
    InvalidUtf8,
}

// Little-endian primitive writers. The encoded form matches the
// runtime wire format's little-endian, u32-length-prefixed convention.
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

// Cursor-based readers over a byte slice.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DebugMetaError> {
        let end = self.pos.checked_add(n).ok_or(DebugMetaError::Truncated)?;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or(DebugMetaError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, DebugMetaError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DebugMetaError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, DebugMetaError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

impl DebugPool {
    /// True when the pool carries no data and no records. An absent
    /// pool and an empty pool are distinct at the chunk level (an
    /// absent pool omits the section entirely); this helper lets a
    /// producer collapse an empty pool to absent.
    pub fn is_empty(&self) -> bool {
        self.string_pool.is_empty()
            && self.span_pool.is_empty()
            && self.type_pool.is_empty()
            && self.records.is_empty()
    }

    /// Encode the pool to its canonical byte form. Records are sorted
    /// into canonical order (op index, then kind, then operands) so the
    /// output is byte-deterministic for a given logical pool regardless
    /// of record insertion order. The data sub-pools are emitted in
    /// their stored order; see the module-level determinism note.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();

        put_u32(&mut out, self.string_pool.len() as u32);
        for s in &self.string_pool {
            put_u32(&mut out, s.len() as u32);
            out.extend_from_slice(s.as_bytes());
        }

        put_u32(&mut out, self.span_pool.len() as u32);
        for &(file_idx, offset, length) in &self.span_pool {
            put_u16(&mut out, file_idx);
            put_u32(&mut out, offset);
            put_u32(&mut out, length);
        }

        put_u32(&mut out, self.type_pool.len() as u32);
        for blob in &self.type_pool {
            put_u32(&mut out, blob.len() as u32);
            out.extend_from_slice(blob);
        }

        // Canonical record order. Operands are compared lexically as a
        // final tiebreak so two records that differ only in operands
        // still order deterministically.
        let mut records: Vec<&DebugRecord> = self.records.iter().collect();
        records.sort_by(|a, b| {
            a.op_index
                .cmp(&b.op_index)
                .then(a.kind.as_u8().cmp(&b.kind.as_u8()))
                .then(a.operands.cmp(&b.operands))
        });

        put_u32(&mut out, records.len() as u32);
        for record in records {
            put_u32(&mut out, record.op_index);
            out.push(record.kind.as_u8());
            put_u16(&mut out, record.operands.len() as u16);
            for &operand in &record.operands {
                put_u16(&mut out, operand);
            }
        }

        out
    }

    /// Decode a pool from its canonical byte form. The returned pool's
    /// records are in the canonical order [`encode`](Self::encode)
    /// emits, so `decode(encode(p))` round-trips and re-encodes to the
    /// identical bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, DebugMetaError> {
        let mut r = Reader::new(bytes);

        let string_count = r.u32()? as usize;
        let mut string_pool = Vec::with_capacity(string_count);
        for _ in 0..string_count {
            let len = r.u32()? as usize;
            let raw = r.take(len)?;
            let s = core::str::from_utf8(raw).map_err(|_| DebugMetaError::InvalidUtf8)?;
            string_pool.push(String::from(s));
        }

        let span_count = r.u32()? as usize;
        let mut span_pool = Vec::with_capacity(span_count);
        for _ in 0..span_count {
            let file_idx = r.u16()?;
            let offset = r.u32()?;
            let length = r.u32()?;
            span_pool.push((file_idx, offset, length));
        }

        let type_count = r.u32()? as usize;
        let mut type_pool = Vec::with_capacity(type_count);
        for _ in 0..type_count {
            let len = r.u32()? as usize;
            type_pool.push(r.take(len)?.to_vec());
        }

        let record_count = r.u32()? as usize;
        let mut records = Vec::with_capacity(record_count);
        for _ in 0..record_count {
            let op_index = r.u32()?;
            let kind_byte = r.u8()?;
            let kind = DebugRecordKind::from_u8(kind_byte)
                .ok_or(DebugMetaError::UnknownRecordKind(kind_byte))?;
            let operand_count = r.u16()? as usize;
            let mut operands = Vec::with_capacity(operand_count);
            for _ in 0..operand_count {
                operands.push(r.u16()?);
            }
            records.push(DebugRecord {
                op_index,
                kind,
                operands,
            });
        }

        Ok(DebugPool {
            string_pool,
            span_pool,
            type_pool,
            records,
        })
    }

    // --- Read path ---
    //
    // The query API a consumer uses to resolve op-stream positions to
    // debug information: a debugger, a stack-trace formatter, or a
    // runtime error decorator that maps a faulting op back to source.
    // The records are stored in canonical `(op_index, kind)` order, so
    // a given op's records are contiguous.

    /// The records annotating op-stream position `op_index`, in
    /// canonical order. Empty when the position carries no debug
    /// information.
    pub fn records_at(&self, op_index: u32) -> impl Iterator<Item = &DebugRecord> {
        self.records.iter().filter(move |r| r.op_index == op_index)
    }

    /// The string-pool entry at `index`, or `None` when out of range.
    pub fn string(&self, index: u16) -> Option<&str> {
        self.string_pool.get(index as usize).map(|s| s.as_str())
    }

    /// The span-pool entry at `index`, or `None` when out of range.
    pub fn span(&self, index: u16) -> Option<Span> {
        self.span_pool.get(index as usize).copied()
    }

    /// The type sub-pool blob at `index`, or `None` when out of range.
    /// A `TypeAnnotation` record's second operand indexes this pool.
    /// The blob is the opaque `TypeRepr`; the current compiler stores a
    /// UTF-8 string-form rendering.
    pub fn type_blob(&self, index: u16) -> Option<&[u8]> {
        self.type_pool.get(index as usize).map(|v| v.as_slice())
    }

    /// Resolve a record that references a span to a [`SourceLocation`].
    /// Applies to record kinds whose first operand indexes the span
    /// pool (`CallSite`, `SourceSpan`, `AssertionContext`,
    /// `BreakpointCandidate`). Returns `None` for other kinds, for a
    /// record with no operands, or when an operand index dangles.
    pub fn source_location(&self, record: &DebugRecord) -> Option<SourceLocation<'_>> {
        match record.kind {
            DebugRecordKind::CallSite
            | DebugRecordKind::SourceSpan
            | DebugRecordKind::AssertionContext
            | DebugRecordKind::BreakpointCandidate => {}
            _ => return None,
        }
        let span_idx = *record.operands.first()?;
        let (file_idx, byte_offset, byte_length) = self.span(span_idx)?;
        Some(SourceLocation {
            file: self.string(file_idx),
            byte_offset,
            byte_length,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn sample_pool() -> DebugPool {
        DebugPool {
            string_pool: vec!["main.kel".to_string(), "count".to_string()],
            span_pool: vec![(0, 10, 4), (0, 20, 5)],
            type_pool: vec![vec![0x01, 0x02], vec![0x03]],
            records: vec![
                DebugRecord {
                    op_index: 7,
                    kind: DebugRecordKind::CallSite,
                    operands: vec![0, 0],
                },
                DebugRecord {
                    op_index: 3,
                    kind: DebugRecordKind::VariableName,
                    operands: vec![1],
                },
            ],
        }
    }

    #[test]
    fn records_at_returns_records_for_a_position() {
        let pool = sample_pool();
        let at7: alloc::vec::Vec<_> = pool.records_at(7).collect();
        assert_eq!(at7.len(), 1);
        assert_eq!(at7[0].kind, DebugRecordKind::CallSite);
        assert!(pool.records_at(99).next().is_none());
    }

    #[test]
    fn source_location_resolves_call_site_span() {
        let pool = sample_pool();
        let rec = pool.records_at(7).next().unwrap();
        let loc = pool.source_location(rec).expect("call site resolves");
        assert_eq!(loc.file, Some("main.kel"));
        assert_eq!(loc.byte_offset, 10);
        assert_eq!(loc.byte_length, 4);
    }

    #[test]
    fn source_location_is_none_for_non_span_kinds() {
        let pool = sample_pool();
        let var = pool.records_at(3).next().unwrap();
        assert_eq!(var.kind, DebugRecordKind::VariableName);
        assert!(pool.source_location(var).is_none());
    }

    #[test]
    fn source_location_is_none_on_dangling_span_index() {
        let mut pool = DebugPool::default();
        pool.records.push(DebugRecord {
            op_index: 0,
            kind: DebugRecordKind::CallSite,
            operands: vec![5], // no span-pool entries exist
        });
        let rec = &pool.records[0];
        assert!(pool.source_location(rec).is_none());
    }

    #[test]
    fn all_kinds_round_trip_through_u8() {
        let kinds = [
            DebugRecordKind::CallSite,
            DebugRecordKind::SourceSpan,
            DebugRecordKind::LineNumber,
            DebugRecordKind::VariableName,
            DebugRecordKind::TypeAnnotation,
            DebugRecordKind::AssertionContext,
            DebugRecordKind::BreakpointCandidate,
            DebugRecordKind::GenericInstantiation,
            DebugRecordKind::IfcLabelAnnotation,
            DebugRecordKind::WcetMarker,
            DebugRecordKind::OptimisationMarker,
            DebugRecordKind::VerifierWitness,
        ];
        for k in kinds {
            assert_eq!(DebugRecordKind::from_u8(k.as_u8()), Some(k));
        }
        // The catalogue has twelve kinds (0..=11); 12 is unknown.
        assert_eq!(DebugRecordKind::from_u8(12), None);
    }

    #[test]
    fn empty_pool_round_trips() {
        let pool = DebugPool::default();
        assert!(pool.is_empty());
        let bytes = pool.encode();
        let decoded = DebugPool::decode(&bytes).expect("decode");
        assert_eq!(decoded, pool);
    }

    #[test]
    fn populated_pool_round_trips() {
        let pool = sample_pool();
        let decoded = DebugPool::decode(&pool.encode()).expect("decode");
        // The records come back in canonical order (op_index 3 before
        // 7), so compare the decoded pool to the canonically-ordered
        // expectation rather than the insertion order.
        assert_eq!(decoded.string_pool, pool.string_pool);
        assert_eq!(decoded.span_pool, pool.span_pool);
        assert_eq!(decoded.type_pool, pool.type_pool);
        assert_eq!(decoded.records.len(), 2);
        assert_eq!(decoded.records[0].op_index, 3);
        assert_eq!(decoded.records[1].op_index, 7);
    }

    #[test]
    fn encode_is_deterministic_regardless_of_record_order() {
        let pool_a = sample_pool();
        // Same logical records, appended in the opposite order.
        let mut pool_b = sample_pool();
        pool_b.records.reverse();
        assert_eq!(
            pool_a.encode(),
            pool_b.encode(),
            "record insertion order must not affect encoded bytes"
        );
    }

    #[test]
    fn decode_then_encode_is_byte_identical() {
        let bytes = sample_pool().encode();
        let reencoded = DebugPool::decode(&bytes).expect("decode").encode();
        assert_eq!(bytes, reencoded);
    }

    #[test]
    fn decode_rejects_truncated_input() {
        let bytes = sample_pool().encode();
        // Drop the final byte; decoding must fail rather than panic.
        let err = DebugPool::decode(&bytes[..bytes.len() - 1]).unwrap_err();
        assert_eq!(err, DebugMetaError::Truncated);
    }

    #[test]
    fn decode_rejects_unknown_record_kind() {
        let mut pool = DebugPool::default();
        pool.records.push(DebugRecord {
            op_index: 0,
            kind: DebugRecordKind::CallSite,
            operands: vec![],
        });
        let mut bytes = pool.encode();
        // The single record's kind byte sits after the four sub-pool
        // count headers (4 * u32 = 16 bytes) plus the record's op_index
        // (4 bytes): offset 20. Corrupt it to an unknown kind.
        bytes[20] = 200;
        let err = DebugPool::decode(&bytes).unwrap_err();
        assert_eq!(err, DebugMetaError::UnknownRecordKind(200));
    }

    #[test]
    fn decode_rejects_non_utf8_string() {
        // One string entry whose bytes are invalid UTF-8.
        let mut bytes = Vec::new();
        put_u32(&mut bytes, 1); // string_pool length
        put_u32(&mut bytes, 1); // first string byte-length
        bytes.push(0xFF); // invalid UTF-8 lead byte
        put_u32(&mut bytes, 0); // span_pool length
        put_u32(&mut bytes, 0); // type_pool length
        put_u32(&mut bytes, 0); // record_pool length
        let err = DebugPool::decode(&bytes).unwrap_err();
        assert_eq!(err, DebugMetaError::InvalidUtf8);
    }
}
