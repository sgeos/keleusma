# Wire Format

> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)

This document specifies the V0.2.0 bytecode wire format. The format pairs a fixed-size 64-byte framing header with a section-partitioned body. The body partitions into a fixed-size opcode stream, a separately addressed operand pool for compound operands, and the in-place archived auxiliary data (chunk metadata, constant pool, struct templates, native names, data layout) that the existing rkyv path produces.

V0.2.0 introduces the format. V0.1.x runtimes cannot read V0.2.0 bytecode. The framing-header `version` field resets to `1` to signal the discontinuity. The rkyv-archived encoding survives as the internal representation for the auxiliary body and as a cross-process transport mechanism, but the execution loop reads the opcode stream and the operand pool directly through the new fixed-size records.

## Status

V0.2.0 Phase 7a publishes this specification and the wire-format types in `src/wire_format.rs`. The opcode encoder and decoder are implemented and exercised by round-trip tests covering every `Op` variant.

V0.2.0 Phase 7b adds `wire_format::module_to_wire_bytes(&Module)` and `wire_format::module_from_wire_bytes(&[u8])` that round-trip an entire `Module` through the V0.2.0 wire format: 64-byte framing header, opcode stream, operand pool, rkyv-archived auxiliary body, CRC trailer.

V0.2.0 Phase 7c cuts the default `Module::to_bytes` / `Module::from_bytes` / `Module::access_bytes` over to the wire format. `Module::to_bytes` delegates to `module_to_wire_bytes`; `Module::from_bytes` delegates to `module_from_wire_bytes`; `Module::access_bytes` now returns `&ArchivedWireAuxBody` and validates the wire format. The VM's zero-copy path reads opcodes via the opcode stream section and accesses the auxiliary body through the wire-format header offsets. The rkyv archive of the full `Module` is no longer used at runtime; the rkyv-archived `WireAuxBody` is the only on-the-wire archived form. Programs that previously round-tripped through the legacy rkyv-only framing must be recompiled against the V0.2.0 toolchain.

The phased cutover preserves the existing test surface where possible. Tests that hand-patched legacy header byte offsets are retargeted at the new wire format layout, and the golden bytes fixture is refreshed to pin the V0.2.0 byte sequence for `fn main() -> Word { 1 }` (216 bytes total, with an 8-byte opcode stream and an empty operand pool).

## Design rationale

The wire format is shaped by three concerns.

First, decoder simplicity. Fixed-size opcode records remove the variable-length operand decoding step that the rkyv-archived enum representation requires. A decoder advances exactly four bytes per record without consulting a length field or a discriminator table. This shape suits a hardware decoder that pipelines record fetch and operand expansion in lockstep.

Second, integrity at the record level. Each opcode record and each operand pool entry carry a parity bit covering the rest of the payload. Single bit flips are detected at the consumer site before the record reaches the dispatch table. The parity is cheap to compute and does not require a separate CRC pass.

Third, separation of code and data. The opcode stream is contiguous and the operand pool is addressed separately. Compound operands that exceed three inline bytes (the addressable space within a four-byte record) reference an entry in the operand pool by index. Pool entries are eight-byte aligned, which matches a natural cache-line boundary and lets a host that streams the pool into a separately mapped region do so without realignment.

The audit considered an alternative variable-length encoding that placed compound operands inline. The fixed-size record won on decoder simplicity and on the observation that pool-referencing operands cover only a few opcodes: three always (`GetDataIndexed`, `SetDataIndexed`, `IsEnum`) plus `NewComposite` in its boxed or large-count form. The pool indirection cost is paid only for those.

## Framing header

The framing header is at least sixty-four bytes for unsigned modules and grows to accommodate an optional signature-extension block for signed modules. Multiples of eight preserve alignment for the eight-byte operand pool entries that follow when the body starts at a header-aligned offset. The header carries the magic, version, total length, target widths, flags, declared WCET and WCMU, the data segment sizes, section offsets and lengths for the opcode stream, operand pool, and rkyv-archived auxiliary body, and (when present) the cryptographic signature.

| Offset | Width | Field |
|--------|-------|-------|
| 0      | 4     | Magic `b"KELE"` |
| 4      | 2     | Version (u16 little-endian) |
| 6      | 2     | Header length (u16 little-endian; 64 for unsigned, 64 + 8 + signature_length + padding-to-8 for signed) |
| 8      | 4     | Total length (u32 little-endian, includes header, sections, CRC trailer) |
| 12     | 1     | Target word bits log2 (u8) |
| 13     | 1     | Target address bits log2 (u8) |
| 14     | 1     | Target float bits log2 (u8) |
| 15     | 1     | Flags (u8). Bit 0 = `FLAG_EPHEMERAL`. Bit 1 = `FLAG_REQUIRES_SIGNATURE`. Bit 2 = `FLAG_ENCRYPTED`. Other bits reserved. |
| 16     | 4     | Declared WCET cycles (u32 little-endian) |
| 20     | 4     | Declared WCMU bytes (u32 little-endian) |
| 24     | 4     | Shared data bytes (u32 little-endian) |
| 28     | 4     | Private data bytes (u32 little-endian) |
| 32     | 4     | Opcode stream offset (u32 little-endian, relative to start of file) |
| 36     | 4     | Opcode stream length (u32 little-endian, multiple of 4) |
| 40     | 4     | Operand pool offset (u32 little-endian, relative to start of file) |
| 44     | 4     | Operand pool length (u32 little-endian, multiple of 8) |
| 48     | 4     | Auxiliary body offset (u32 little-endian, relative to start of file) |
| 52     | 4     | Auxiliary body length (u32 little-endian) |
| 56     | 4     | Reserved (u32, zero) |
| 60     | 4     | Reserved (u32, zero) |

The reserved fields cover future section additions. A V0.2.x runtime that encounters non-zero reserved fields rejects the bytecode as `LoadError::Codec` to preserve forward-compatibility against future producers that adopt the same magic and version.

### Signature extension (optional)

Signed modules append an eight-byte metadata block followed by the raw signature payload immediately after byte 64. The `header_length` field at bytes 6..8 encodes the total header size including the extension; section offsets later in the header point past the extension into the body.

| Offset | Width | Field |
|--------|-------|-------|
| 64     | 1     | Scheme id (u8). `1` = Ed25519. `0` and other values reserved. |
| 65     | 1     | Reserved (u8, zero) |
| 66     | 2     | Signature length (u16 little-endian). For Ed25519: 64. |
| 68     | 4     | Reserved (u32, zero) |
| 72     | n     | Signature payload (n bytes, scheme-dependent) |
| 72+n   | pad   | Zero padding to the next 8-byte boundary |

For Ed25519, the signature payload is 64 bytes; total `header_length` = 64 + 8 + 64 = 136 bytes (already 8-aligned, no padding).

A `FLAG_REQUIRES_SIGNATURE` bit in the flags byte indicates whether the loader must verify the signature. The decoder rejects inconsistent combinations: flag set without an extension, or extension present without the flag. V0.2.0 does not admit optional or audit-only signatures.

The cryptographic message that the signature covers is the entire framed buffer with the signature payload bytes and the CRC trailer bytes zeroed. Both signer and verifier zero those two regions before computing the cryptographic operation. The CRC trailer covers everything including the real signature bytes, so the CRC catches corruption regardless of whether the signature itself was modified in transit.

See `R42` in [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) for the design rationale.

### Encryption extension (optional, V0.2.1)

Encrypted modules append an 88-byte encryption-metadata block after the signature extension. Encryption requires signing; the wire format does not admit unsigned encrypted modules because the signature is what authenticates the encrypted payload's origin.

| Offset | Width | Field |
|--------|-------|-------|
| 136    | 1     | Encryption scheme id (u8). `1` = X25519 + AES-256-GCM + HKDF-SHA-256. Other values reserved. |
| 137    | 1     | Reserved (u8, zero) |
| 138    | 2     | Encryption metadata length (u16 little-endian; 88 for the V0.2.1 scheme) |
| 140    | 32    | Ephemeral X25519 public key (32 bytes). The compiler's per-module ephemeral public key. The recipient combines this with its own private key to reconstruct the shared secret. |
| 172    | 32    | recipient_key_id (32 bytes). SHA-256 fingerprint of the destination runtime's X25519 public key. The runtime checks this matches the SHA-256 of its own public key before attempting decryption. |
| 204    | 12    | AES-GCM nonce (12 bytes). Included in the artefact so the recipient can verify the HKDF-derived nonce matches. |
| 216    | 8     | Reserved (u64, zero, for 8-byte alignment) |

The block is 88 bytes total. For Ed25519 + X25519 + AES-256-GCM, `header_length` = 64 + 8 + 64 + 88 = 224 bytes (already 8-aligned).

The encrypted body replaces the cleartext body. The body region carries AES-256-GCM ciphertext immediately followed by the 16-byte authentication tag. The on-disk total length is `header_length + ciphertext_length + tag_length + 4` where `ciphertext_length` equals the plaintext body length and the trailing 4 bytes are the CRC.

The signature covers the entire on-disk buffer (including the encryption metadata and the encrypted body) with the signature payload bytes and the CRC trailer bytes zeroed. This means the signature authenticates both the encryption metadata and the ciphertext; an adversary cannot strip the encryption layer and substitute cleartext bytecode while preserving signature validity.

The runtime workflow for an encrypted artefact:

1. Read the header. Confirm `FLAG_REQUIRES_SIGNATURE` and `FLAG_ENCRYPTED` are both set.
2. Verify the Ed25519 signature against the encrypted form.
3. Parse the encryption metadata block. Confirm `recipient_key_id` matches the SHA-256 of the local X25519 public key.
4. Compute the X25519 shared secret from the metadata's ephemeral public key and the local X25519 private key.
5. Derive the AES-256 key through HKDF-SHA-256 with the info string `"keleusma-v1-aes256-gcm-key"`. Derive the AES-GCM nonce with the info string `"keleusma-v1-aes256-gcm-nonce"`. Cross-check the derived nonce against the metadata.
6. Decrypt the body with AES-256-GCM. The crate verifies the authentication tag; a failure indicates either tampering or wrong key.
7. Run structural verification on the decrypted plaintext, then construct the VM.

The `BYTECODE_VERSION` field remains 1. V0.2.0 runtimes reject V0.2.1 encrypted artefacts cleanly because the `header_length` check fails (V0.2.0 expects either 64 or 136; encrypted artefacts carry 224). The combination of `FLAG_ENCRYPTED` and the extended header length unambiguously identifies encrypted artefacts.

The encryption work is feature-gated on the `encryption` Cargo feature, off by default. Hosts that do not need encrypted delivery pay no binary-size cost from the encryption crypto stack. Encrypted artefacts produced on a host with the feature on do not load on a host with the feature off; the loader returns a clear diagnostic.

See `R50` in [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) for the design rationale, `R49` for the companion CLI policy gate, and `docs/guide/SECURITY_POLICY.md` for the operator-facing guide.

## Opcode records

Each opcode is a four-byte record. The record carries the opcode identifier in the low seven bits of byte zero and a parity bit in the high bit. Bytes one through three carry the operand inline when it fits in twenty-four bits and carry a pool index otherwise.

| Offset | Width | Field |
|--------|-------|-------|
| 0      | 1     | Bit 7: parity. Bits 0..6: opcode identifier. |
| 1      | 1     | Operand byte 0 (low). |
| 2      | 1     | Operand byte 1. |
| 3      | 1     | Operand byte 2 (high). |

The parity bit is the XOR of the other thirty-one bits in the record. A consumer reads byte zero, computes the parity over the seven low bits of byte zero and all bits of bytes one through three, compares against the high bit of byte zero, and rejects the record on mismatch. The parity covers the entire record so single bit flips anywhere are detected at the consumer site.

The opcode identifier is the index of the `Op` variant in the canonical wire listing. The table is fixed at version 1 of the wire format. The mapping is stable across the V0.2.x series. The B28 consolidation retired the four V0.2.0 construct opcodes (`NewStruct`, `NewEnum`, `NewArray`, `NewTuple`, ids 34-37) and introduced `NewComposite` at id 69, so the live ISA has sixty-six variants with a maximum identifier of 69 and four reserved-and-unused ids. The identifier fits in seven bits; future ISA additions that exceed one hundred and twenty-eight variants would require a version bump.

The operand semantics depend on the opcode variant. Inline operands cover these shapes:

- **No operand.** Bytes one through three are zero. Thirty-six variants.
- **`u8`.** Byte one carries the value; bytes two and three are zero. Eight variants.
- **`u16`.** Bytes one through two carry the value little-endian; byte three is zero. Fifteen variants.
- **`(u16, u8)`.** Bytes one through two carry the `u16` little-endian; byte three carries the `u8`. Three variants.
- **`NewComposite`, flat form.** Bytes one through two carry the composite's flat byte size little-endian. Byte three packs the composite kind in its high two bits and the operand-stack pop count (zero through sixty-two) in its low six bits. A low-six-bit value of `0x3F` is the sentinel that redirects to the pool form below.

The pool-referencing forms place their payload in the operand pool because it does not fit in three bytes:

- **`(u16, u16)`.** Pool entry tag `0x01`. Three variants: `GetDataIndexed`, `SetDataIndexed`, `IsEnum`. The inline operand bytes carry a twenty-four-bit pool index little-endian.
- **`(u16, u16, u8)`.** Pool entry tag `0x02`. One variant: `NewComposite`, used for the boxed form or when the flat field count exceeds sixty-two. Operand byte three holds the composite kind in its high two bits and the sentinel `0x3F` in its low six bits, so operand bytes one through two carry a sixteen-bit pool index rather than the twenty-four-bit index used by the `(u16, u16)` opcodes. The referenced entry carries `(count, byte_size-or-meta, boxed_flag)`.

A pool of up to 16,777,216 entries (no observed program approaches one tenth of this) covers the foreseeable case for the twenty-four-bit forms. A producer that exceeds the applicable limit emits a `CompileError`.

## Operand pool

The operand pool is a contiguous sequence of eight-byte entries. Each entry is self-describing through a type tag and integrity-checked through a parity byte.

| Offset | Width | Field |
|--------|-------|-------|
| 0      | 1     | Type tag (`0x01` for `(u16, u16)`, `0x02` for `(u16, u16, u8)`). |
| 1      | 1     | Parity (XOR of bytes 0 and 2 through 7). |
| 2      | 2     | First `u16` little-endian. |
| 4      | 2     | Second `u16` little-endian. |
| 6      | 1     | `u8` (for tag `0x02`) or zero (for tag `0x01`). |
| 7      | 1     | Reserved (zero). |

The pool offset declared in the framing header is eight-byte aligned within the bytecode buffer. A consumer reading a pool entry validates the type tag against the expected tag for the consuming opcode and validates the parity against the rest of the entry. Tag and parity mismatches surface as `LoadError::CorruptOperandPool`.

The reserved byte at offset seven is included so each entry occupies a full cache line within an eight-byte aligned region. The entry width is fixed at eight bytes regardless of the tag so a producer can compute pool offsets through `index * 8` arithmetic without consulting per-entry metadata.

## Section-partitioned body

The body of the bytecode partitions into three sections after the framing header:

1. **Opcode stream.** Concatenated four-byte records for every chunk in declaration order. Per-chunk boundaries live in the auxiliary body's chunk table.
2. **Operand pool.** Concatenated eight-byte entries indexed by the inline pool index in the opcode records that reference them.
3. **Auxiliary body.** Constant pool, struct templates, chunk table (name, op offset, op count, local count, parameter types, and an optional per-chunk debug metadata section), native names, data layout, and entry point index. The auxiliary body uses the existing rkyv archived encoding through V0.2.x and migrates to a custom encoding under a Phase 7c follow-on.

The CRC-32 trailer covers the header and all three sections. The trailer's algebraic self-inclusion property holds: a consumer computing the CRC over the bytes from offset zero through the four-byte trailer obtains the residue constant `0x2144DF1C`. This property survives the section-partitioned body unchanged.

### Debug metadata (optional, B29)

Each entry in the chunk table carries an optional `debug_pool_bytes` field: the canonical byte encoding of a strippable debug-metadata section, or absent for a release build or a stripped artefact. The metadata lives only in the auxiliary body and never in the opcode stream, so the opcode stream is byte-identical between a debug build and a release build, and stripping the metadata removes the field rather than transforming the program.

The field holds the bytes produced by `debug_meta::DebugPool::encode`, using the same little-endian, `u32`-length-prefixed convention as the rest of the wire format. The layout is four sub-pools in fixed order.

| Sub-pool | Encoding |
|----------|----------|
| String pool | `u32` count, then each entry as a `u32` byte length and UTF-8 bytes |
| Span pool | `u32` count, then each entry as `(u16 file_string_index, u32 byte_offset, u32 byte_length)` |
| Type pool | `u32` count, then each entry as a `u32` byte length and opaque bytes |
| Record pool | `u32` count, then each record as `(u32 op_index, u8 kind, u16 operand_count, operand_count × u16)` |

A record annotates the op-stream position named by its `op_index` and carries `u16` operand indices into the sub-pools, with the operand meaning fixed per record kind. The record pool is emitted in canonical `(op_index, kind, operands)` order, so the encoding is byte-deterministic for a given logical pool. Dropping the field reproduces a release artefact byte-for-byte, and re-encoding a decoded pool reproduces the same bytes.

The metadata never affects execution. Strippable annotations neither push nor pop operand-stack values nor alter control flow, so the verifier's stack-effect and control-flow analyses are identical with or without the field, and the worst-case memory pass treats it as zero runtime cost.

The compiler emits the field when invoked with debug enabled (`compiler::compile_with_options` with `emit_debug`, surfaced as `keleusma compile --debug`); the `keleusma strip` subcommand removes it. The encoded bytes are the canonical byte form of the chunk's debug pool. The record catalogue, the per-kind operand encodings, the byte layout, the read and query interface, and the runtime fault-localization path are specified in [DEBUG_METADATA.md](./DEBUG_METADATA.md); all twelve record kinds emit. The field was added within the V0.2.x line without a `BYTECODE_VERSION` bump, consistent with the project's no-production-traction stance; a runtime built before B29 does not know the optional section.

## Wire format types

The V0.2.0 Phase 7a release ships the following types in `src/wire_format.rs`:

- `WireFormatHeader` mirrors the sixty-four-byte framing header layout. Fields are `pub` for direct access; helpers encode and decode against `[u8; 64]`.
- `OpcodeId` is a `u8` newtype carrying the seven-bit opcode identifier. The mapping table converts to and from the `Op` enum.
- `OpcodeRecord` is a `[u8; 4]` newtype with constructors that take an `OpcodeId` and either inline operand bytes or a pool index, and that compute the parity bit before returning the record.
- `OperandPoolEntry` is a `[u8; 8]` newtype with constructors for the `(u16, u16)` and `(u16, u16, u8)` tag variants and a decoder that returns the typed operand on parity success.

The encoder accepts an `Op` and emits an `OpcodeRecord`, queueing pool entries through a `&mut Vec<OperandPoolEntry>` accumulator. The decoder accepts an `OpcodeRecord` and an `&[OperandPoolEntry]` and reconstructs the `Op`. Round-trip tests cover every variant.

## Migration

V0.1.x bytecode artefacts cannot be loaded by V0.2.0 runtimes. Hosts that have V0.1.x bytecode in flight at publication time recompile against the V0.2.0 toolchain. The framing-header `version` field resets to `1` to signal the discontinuity; V0.2.0 runtimes reject V0.1.x bytecode at the framing-level check.

Within the V0.2.0 series, the Phase 7a release ships the wire-format types and tests but does not yet route the execution loop through them. `Module::to_bytes` and `Module::from_bytes` continue to produce and consume the V0.1.x-style framing plus rkyv body. Phase 7b switches the producer to emit the section-partitioned body and the consumer to read the opcode stream and operand pool through the new types; the auxiliary body remains rkyv. Phase 7c migrates the auxiliary body to a custom encoding and removes the rkyv dependency from the execution loop. The CRC trailer and the magic remain stable across all three phases.
