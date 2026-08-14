# Wire Format

> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)

This document specifies the bytecode wire format. The format pairs a fixed-size 64-byte framing header with a section-partitioned body. The body partitions into a fixed-size opcode stream, a separately addressed operand pool for compound operands, and an auxiliary body carrying chunk metadata, the constant pool, struct templates, native names, the data layout, the enum-layout table, and the typed-verifier signature and native-return descriptor tables.

V0.2.0 introduced the format and its `version` field reset to `1`, signalling that V0.1.x runtimes cannot read V0.2.0 bytecode. The execution loop reads the opcode stream and the operand pool directly through fixed-size records.

**Version 2 is the current format.** The framing-header `version` field is `2`. Two changes accumulated under that number, and they are separate matters that are easy to confuse.

The first is an operand widening. The shared-data byte offset, the unified data-slot index, and the indexed-array length operands of `GetData`, `SetData`, `GetDataIndexed`, and `SetDataIndexed` widened from sixteen to twenty-four bits, raising the shared-segment ceiling from 64 KB to 16 MB. The widening reuses the three inline operand bytes for `GetData` and `SetData`, and the six-byte operand-pool payload for the indexed pair under the `(u24, u24)` tag `0x04`, so the four-byte opcode record and the eight-byte pool entry are unchanged and bytecode does not grow. `SharedSlotLayout::offset` becomes a `u32`. This change alone did **not** carry a version bump. It landed while the number was held at `1` under the no-public-adoption policy, and an earlier revision of this document wrongly attributed version 2 to it.

The second is the change that did carry the bump. **The auxiliary body is no longer an rkyv archive.** It is now a self-describing container specified in [The auxiliary body container](#the-auxiliary-body-container) below and implemented by the standalone `keleusma-wire` crate, with Keleusma's schema layered on top in `src/wire_schema.rs`. The substrate itself changed, which is the ground on which the version moved to `2`.

Version 1 bytecode is rejected at load on the version check rather than accepted and then misread.

## Status

V0.2.0 Phase 7a publishes this specification and the wire-format types in `src/wire_format.rs`. The opcode encoder and decoder are implemented and exercised by round-trip tests covering every `Op` variant.

V0.2.0 Phase 7b adds `wire_format::module_to_wire_bytes(&Module)` and `wire_format::module_from_wire_bytes(&[u8])` that round-trip an entire `Module` through the wire format: 64-byte framing header, opcode stream, operand pool, auxiliary body, CRC trailer.

V0.2.0 Phase 7c cuts the default `Module::to_bytes` and `Module::from_bytes` over to the wire format. The VM's zero-copy path reads opcodes via the opcode stream section and accesses the auxiliary body through the wire-format header offsets. The rkyv archive of the full `Module` is no longer used at runtime. Programs that previously round-tripped through the legacy rkyv-only framing must be recompiled.

**The Phase 7c follow-on is complete.** The auxiliary body migrated from the rkyv archive to the container specified below, and no part of the runtime reads an rkyv archive. The rkyv dependency itself remains, used only for `rkyv::util::AlignedVec` as a buffer-alignment helper, which has nothing to do with the archive. `Module::access_bytes` became `Module::validate_bytes`, because the old signature returned `&ArchivedWireAuxBody`, a type that no longer describes anything on the wire.

The container is byte-addressed and imposes no alignment requirement, so the eight-byte-aligned scratch copy the rkyv path needed is gone, and with it the class of fault that copy existed to prevent, namely an unaligned decode on a 32-bit target.

The golden bytes fixture pins the byte sequence for `fn main() -> Word { 1 }` at 988 bytes: a 64-byte framing header, an 8-byte opcode stream, an empty operand pool, a 912-byte auxiliary body at offset 72, and the 4-byte CRC trailer. That a minimal program is dominated by framing is expected and is discussed under [Cost profile](#cost-profile).

## Design rationale

The wire format is shaped by three concerns.

First, decoder simplicity. Fixed-size opcode records remove the variable-length operand decoding step that the archived enum representation they replaced required. A decoder advances exactly four bytes per record without consulting a length field or a discriminator table. This shape suits a hardware decoder that pipelines record fetch and operand expansion in lockstep.

Second, integrity at the record level. Each opcode record and each operand pool entry carry a parity bit covering the rest of the payload. Single bit flips are detected at the consumer site before the record reaches the dispatch table. The parity is cheap to compute and does not require a separate CRC pass.

Third, separation of code and data. The opcode stream is contiguous and the operand pool is addressed separately. Compound operands that exceed three inline bytes (the addressable space within a four-byte record) reference an entry in the operand pool by index. Pool entries are eight-byte aligned, which matches a natural cache-line boundary and lets a host that streams the pool into a separately mapped region do so without realignment.

The audit considered an alternative variable-length encoding that placed compound operands inline. The fixed-size record won on decoder simplicity and on the observation that pool-referencing operands cover only a few opcodes: three always (`GetDataIndexed`, `SetDataIndexed`, `IsEnum`) plus `NewComposite` in its boxed or large-count form. The pool indirection cost is paid only for those.

## Framing header

The framing header is at least sixty-four bytes for unsigned modules and grows to accommodate an optional signature-extension block for signed modules. Multiples of eight preserve alignment for the eight-byte operand pool entries that follow when the body starts at a header-aligned offset. The header carries the magic, version, total length, target widths, flags, declared WCET and WCMU, the data segment sizes, section offsets and lengths for the opcode stream, operand pool, and auxiliary body, and (when present) the cryptographic signature.

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
| 56     | 4     | Auxiliary arena bytes (u32 little-endian). Backed by `Module::aux_arena_bytes`. |
| 60     | 4     | Persistent composite bytes (u32 little-endian). Backed by `Module::persistent_composite_bytes`. |

The words at offsets 56 and 60 were reserved before B28 and are now live. Offset 56 carries the runtime auxiliary-arena byte size, the per-instance bookkeeping memory (the opaque registry and boxed-composite backing) that the runtime pre-sizes once inside the arena's top ephemeral region after each RESET rather than growing during an iteration; a host reads it to pre-size those lists. Offset 60 carries the persistent composite body pool byte size, the total storage that private `.data` slots holding flat composites require in the arena's persistent region so those bodies survive RESET in place; a host adds it to the arena's persistent capacity. A `0` value in either word leaves the header bytes identical to the prior reserved zero-fill, so a module needing neither figure is byte-unchanged. Any word that remains reserved for a future section addition holds zero, and a V0.2.x runtime that encounters a non-zero value in a still-reserved word rejects the bytecode as `LoadError::Codec` to preserve forward-compatibility against future producers that adopt the same magic and version.

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

Adding encryption required no version bump: the combination of `FLAG_ENCRYPTED` and the extended header length identifies an encrypted artefact unambiguously, and at the time a V0.2.0 runtime meeting a V0.2.1 encrypted artefact rejected it on the `header_length` check (V0.2.0 expects either 64 or 136; encrypted artefacts carry 224).

**That was the situation between V0.2.0 and V0.2.1, and it is no longer the operative mechanism.** The field is now **2**, per the version story at the top of this document, and the load path checks the version *before* it reads `header_length`. An older runtime meeting a current artefact therefore rejects it on the version check, whatever its header length. The `FLAG_ENCRYPTED` disambiguation still distinguishes encrypted from cleartext artefacts *within* a version; it is no longer what protects an older runtime.

The encryption work is feature-gated on the `encryption` Cargo feature, off by default. Hosts that do not need encrypted delivery pay no binary-size cost from the encryption crypto stack. Encrypted artefacts produced on a host with the feature on do not load on a host with the feature off; the loader returns a clear diagnostic.

See `R50` in [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) for the design rationale, `R49` for the companion CLI policy gate, and `book/src/SECURITY_POLICY.md` for the operator-facing guide.

## Opcode records

Each opcode is a four-byte record. The record carries the opcode identifier in the low seven bits of byte zero and a parity bit in the high bit. Bytes one through three carry the operand inline when it fits in twenty-four bits and carry a pool index otherwise.

| Offset | Width | Field |
|--------|-------|-------|
| 0      | 1     | Bit 7: parity. Bits 0..6: opcode identifier. |
| 1      | 1     | Operand byte 0 (low). |
| 2      | 1     | Operand byte 1. |
| 3      | 1     | Operand byte 2 (high). |

The parity bit is the XOR of the other thirty-one bits in the record. A consumer reads byte zero, computes the parity over the seven low bits of byte zero and all bits of bytes one through three, compares against the high bit of byte zero, and rejects the record on mismatch. The parity covers the entire record so single bit flips anywhere are detected at the consumer site.

The opcode identifier is the index of the `Op` variant in the canonical wire listing. The table was fixed as of version 1 of the wire format and is unchanged in version 2. The mapping is stable across the V0.2.x series. The B28 consolidation retired the four V0.2.0 construct opcodes (`NewStruct`, `NewEnum`, `NewArray`, `NewTuple`, ids 34-37) and introduced `NewComposite` at id 69, so the live ISA has sixty-six variants with a maximum identifier of 69 and four reserved-and-unused ids. The identifier fits in seven bits; future ISA additions that exceed one hundred and twenty-eight variants would require a version bump.

The operand semantics depend on the opcode variant. Inline operands cover these shapes:

- **No operand.** Bytes one through three are zero. Thirty-six variants.
- **`u8`.** Byte one carries the value; bytes two and three are zero. Eight variants.
- **`u16`.** Bytes one through two carry the value little-endian; byte three is zero. Thirteen variants.
- **`u24` (V2).** Bytes one through three carry the value little-endian. Two variants: `GetData`, `SetData`. A data-slot index above 65535 (a shared segment beyond 64 KB) is representable because the index uses all three inline bytes.
- **`(u16, u8)`.** Bytes one through two carry the `u16` little-endian; byte three carries the `u8`. Three variants.
- **`NewComposite`, flat form.** Bytes one through two carry the composite's flat byte size little-endian. Byte three packs the composite kind in its high two bits and the operand-stack pop count (zero through sixty-two) in its low six bits. A low-six-bit value of `0x3F` is the sentinel that redirects to the pool form below.

The pool-referencing forms place their payload in the operand pool because it does not fit in three bytes:

- **`(u24, u24)` (V2).** Pool entry tag `0x04`. Two variants: `GetDataIndexed`, `SetDataIndexed` (the array base slot and length, each up to twenty-four bits). The inline operand bytes carry a twenty-four-bit pool index little-endian. The two twenty-four-bit values fill the entry's six payload bytes exactly.
- **`(u16, u16)`.** Pool entry tag `0x01`. Serves the `FlatNested` composite-access records described below (before V2 it also served `GetDataIndexed`/`SetDataIndexed`, which moved to the `(u24, u24)` tag). The inline operand bytes carry a twenty-four-bit pool index little-endian.
- **`(u16, u16, u8)`.** Pool entry tag `0x02`. One variant: `NewComposite`, used for the boxed form or when the flat field count exceeds sixty-two. Operand byte three holds the composite kind in its high two bits and the sentinel `0x3F` in its low six bits, so operand bytes one through two carry a sixteen-bit pool index rather than the twenty-four-bit index used by the `(u16, u16)` opcodes. The referenced entry carries `(count, byte_size-or-meta, boxed_flag)`.
- **`(u16, u16, u16)`.** Pool entry tag `0x03`. One variant: `IsEnum`, which carries three constant indices, the enum name, the variant name, and the discriminant value. The inline operand bytes carry a twenty-four-bit pool index little-endian. The referenced entry carries the three `u16` values across bytes two through seven.

A pool of up to 16,777,216 entries (no observed program approaches one tenth of this) covers the foreseeable case for the twenty-four-bit forms. A producer that exceeds the applicable limit emits a `CompileError`.

### Baked composite-access records

The four baked field and element access opcodes, `GetField` (id 38), `GetIndex` (id 39), `GetTupleField` (id 40), and `GetEnumField` (id 41), share one operand encoding family (B28 P2). Byte three of the record is a discriminator that selects one of three forms.

- **Flat scalar access.** Byte three holds a scalar-kind tag in the range `0..=7`. For the three field forms, bytes one and two hold the little-endian flat byte offset of the field, and byte three names the scalar kind of the accessed value. For `GetIndex`, the homogeneous element kind is a scalar and byte one holds its tag directly.
- **Boxed access.** Byte three holds `0xFF` (`TUPLE_FIELD_BOXED_SENTINEL`), which is distinguishable from a scalar-kind tag because scalar tags never exceed `7`. This marks an access against a boxed rather than a flat body. For `GetField`, bytes one and two hold the field-name constant index; for `GetTupleField` and `GetEnumField`, byte one holds the positional index. For `GetIndex`, the boxed form places `0xFF` in byte one with byte three zero.
- **Nested-composite access.** Byte three holds a value in `0xF0..=0xF3` (`FLAT_NESTED_SENTINEL_BASE` with the low two bits carrying a `CompositeKind` tag). A nested composite cannot fit its `(offset, size)` in the two remaining operand bytes, so it spills to a tag `0x01` operand-pool entry. Bytes one and two hold the little-endian `u16` pool index; the pool entry holds `(offset, size)` for the three field forms and `(size, 0)` for `GetIndex` (a homogeneous array has no per-element offset because the element offset is `index * size`). A module whose nested access would reference a pool index beyond `u16::MAX` is rejected at encode time.

The three sentinel spaces do not collide: scalar-kind tags occupy `0..=7`, the nested sentinels occupy `0xF0..=0xF3`, and the boxed sentinel is `0xFF`. A decoder reads byte three, tests for the boxed sentinel first, then for a nested sentinel through the `0xFC` mask, and otherwise decodes a scalar-kind tag; an unrecognised value surfaces as a decode error.

## Operand pool

The operand pool is a contiguous sequence of eight-byte entries. Each entry is self-describing through a type tag and integrity-checked through a parity byte.

| Offset | Width | Field |
|--------|-------|-------|
| 0      | 1     | Type tag (`0x01` for `(u16, u16)`, `0x02` for `(u16, u16, u8)`, `0x03` for `(u16, u16, u16)`, `0x04` for `(u24, u24)`). |
| 1      | 1     | Parity (XOR of bytes 0 and 2 through 7). |
| 2      | 2     | First `u16` little-endian (tags `0x01`/`0x02`/`0x03`). For tag `0x04`, bytes 2 through 4 are the first `u24` little-endian. |
| 4      | 2     | Second `u16` little-endian (tags `0x01`/`0x02`/`0x03`). For tag `0x04`, bytes 5 through 7 are the second `u24` little-endian. |
| 6      | 1     | For tag `0x02`, the `u8`. For tags `0x01`, zero. For tag `0x03`, the low byte of the third `u16`. For tag `0x04`, part of the second `u24`. |
| 7      | 1     | For tag `0x03`, the high byte of the third `u16`. For tag `0x04`, the high byte of the second `u24`. Otherwise reserved (zero). |

The pool offset declared in the framing header is eight-byte aligned within the bytecode buffer. A consumer reading a pool entry validates the type tag against the expected tag for the consuming opcode and validates the parity against the rest of the entry. Tag and parity mismatches surface as `LoadError::CorruptOperandPool`.

Byte seven is the high byte of the third `u16` for a tag `0x03` entry, the high byte of the second `u24` for a tag `0x04` entry, and otherwise reserved zero so each entry occupies a full cache line within an eight-byte aligned region. The entry width is fixed at eight bytes regardless of the tag so a producer can compute pool offsets through `index * 8` arithmetic without consulting per-entry metadata. The `(u24, u24)` tag reuses the same eight-byte entry as the narrower shapes, so widening the indexed operands costs no bytecode size.

## Section-partitioned body

The body of the bytecode partitions into three sections after the framing header:

1. **Opcode stream.** Concatenated four-byte records for every chunk in declaration order. Per-chunk boundaries live in the auxiliary body's chunk table.
2. **Operand pool.** Concatenated eight-byte entries indexed by the inline pool index in the opcode records that reference them.
3. **Auxiliary body.** Constant pool, struct templates, chunk table (name, op offset, op count, local count, parameter types, and an optional per-chunk debug metadata section), native names, data layout, entry point index, a `schema_hash` (u32), the per-enum-type layout table (`enum_layouts`, added under B37 with the variant discriminants and padded-body sizes), and the typed-verifier descriptor tables (Annex A.2.1) that seed the typed operand-stack pass: a per-chunk signature table (`signatures`, the flat shape of each parameter, the return, and the Stream resume), and a per-native return-shape table (`native_return_shapes`, parallel to the native names). The verifier tables are additive and carry no `BYTECODE_VERSION` change; an empty table reproduces the unseeded behaviour. The auxiliary body is encoded in the container specified under [The auxiliary body container](#the-auxiliary-body-container). Each item named above occupies one or more regions of that container, listed in the region table there.

   The `schema_hash` is a CRC-32 of the data-segment layout, computed from a canonical serialisation of each slot's name and visibility in declaration order (`Module::schema_hash`). The runtime uses it to gate hot-swap compatibility: `Vm::replace_module` rejects a swap against an incompatible schema before any data is loaded. A module with no data layout reports zero.

   The data layout itself carries four parts (`DataLayout` in `src/bytecode.rs`). The first is `slots`, the named slots in declaration order, whose index corresponds to the `GetData`/`SetData` operand. The second is `shared_layout: Vec<SharedSlotLayout>`, one entry per shared slot in declaration order, each carrying a byte `offset` (u32 since V2, validated below `2^24` so the shared segment may reach 16 MB) into the host buffer, a `kind` (u8) that is a scalar-kind tag when the `SHARED_SLOT_COMPOSITE_FLAG` high bit is clear or a composite-kind tag in the low bits when set, and a `len` (u16) that is the flat composite body length for a composite slot and zero for a scalar slot (a single composite body still fits sixteen bits). This table is empty when there are no shared slots. The third is `private_composite_layout: Vec<PrivateCompositeSlot>`, one entry per private slot that holds a flat composite body (single composite fields and array-of-composite element slots alike), each carrying the unified data-slot index `slot` (u16) and the byte `offset` (u32) of the body within the persistent composite pool, sorted ascending by `slot` so the runtime resolves a slot by binary search. This table is empty for a module with no private composite slots, so the wire form of such a module is unchanged. The fourth is `private_init: Vec<ConstValue>`, the load-time initial value of each private slot in private-slot order (parallel to the private-slot suffix of `slots`), the `.data`-section model: a scalar slot carries its `= literal` initializer or the type's zero, and a composite or `Text` slot carries `ConstValue::Unit`. The runtime writes these into the persistent region when the VM is constructed; they persist across RESET and are not re-applied. This table is empty for a module with no private data, so the wire form of such a module is unchanged.

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

## The auxiliary body container

The auxiliary body is a self-describing container. The container itself assigns **no meaning** to any region; it locates payloads and nothing more. Region numbering, record strides, and field offsets all belong to the schema layer above it. That separation is what lets the container ship as the standalone `keleusma-wire` crate, reusable by a consumer whose content has nothing to do with Keleusma. Keleusma's schema lives in `src/wire_schema.rs`.

Every unit is a 64-bit word, and every region and record occupies a whole number of words. Element *i* of any table therefore sits at `base + i * stride` with `stride` a power of two, so addressing is a shift rather than a multiply.

### Artifact layout

```text
prologue  ×3        offsets 0, 16, 32          fixed 16 bytes each
directory ×3        offset 48                  region_count × 16 bytes each
regions                                        word-aligned payloads
```

The prologue and the directory are each stored in **three identical copies** and read by bitwise majority-of-three vote, so a single corrupted copy is both outvoted and reported.

### Prologue, 16 bytes

| Offset | Width | Field |
|---|---|---|
| 0 | u32 | magic, `0x4B41_5558`, appearing as the bytes `X`, `U`, `A`, `K` in file order |
| 4 | u16 | byte-order marker, `0xFEFF`. A reader seeing `0xFFFE` knows the artifact is opposite-endian without consulting any external document |
| 6 | u16 | format version, currently `2` |
| 8 | u16 | region count |
| 10 | u16 | flags |
| 12 | u32 | CRC-32 of bytes 0 through 11 |

### Directory entry, 16 bytes

| Offset | Width | Field |
|---|---|---|
| 0 | u16 | region kind, opaque to the container |
| 2 | u16 | region flags |
| 4 | u32 | payload offset, **in words** from the start of the artifact |
| 8 | u32 | payload length, in words |
| 12 | u16 | `covers`, the kind of the region a parity plane protects. Zero and unused otherwise |
| 14 | u16 | reserved |

Region flags are `FLAG_ENCRYPTED` (bit 0, the payload cannot be read in place), `FLAG_ECC_PRESENT` (bit 1, a companion parity region covers this one), `FLAG_OPTIONAL` (bit 2, an unrecognised region may be skipped), and `FLAG_IS_ECC` (bit 3, this region is itself a parity plane and `covers` names its subject).

The region count is bounded at 1024. The bound is not a schema limit but a totality one: a corrupted count must not drive an unbounded walk, and a reader's work must be statically bounded.

### Why the prologue is separate from the directory

This split resolves a bootstrapping problem, and it is the one part of the layout a reimplementation is most likely to get wrong.

Voting the header requires locating copies one and two, which requires the block stride, which depends on `region_count`. Were the directory inside the voted block, `region_count` would itself sit inside the block being voted, and a single bit flip in it would desynchronise the search for the very copies that exist to repair it. The field the vote most needs to protect would be the field the vote cannot be performed without.

A fixed-size prologue at fixed offsets is votable with no prior knowledge. The voted `region_count` then yields the directory stride, and the three directory copies are votable in turn.

### Parity plane, optional

A region may be protected by a companion region holding a (72,64) SECDED code: eight check bits per 64-bit data word, held in a **parallel** plane rather than interleaved with the data, so the protected payload stays readable in place. The plane corrects any single-bit error in a word and detects any double-bit error.

The shipping encoder emits planes on request through `encode_aux_body_with_ecc`, and `WireView::verify_all` scans every protected region against its plane. **Planes are off by default**, because they change an artifact's bytes and byte identity against this encoder is the oracle the self-hosted compiler is verified with. They are purely additive otherwise, since every reader resolves regions by kind instead of enumerating the directory, so an artifact carrying planes decodes identically through the ordinary path and no `BYTECODE_VERSION` change is implied.

**The overhead is one check byte per eight payload bytes asymptotically, and more on a small artifact.** Each plane is a region and is padded to a whole word, so the rounding is paid once per protected region. Measured across real compiler output: 12.5 percent at 303,472 payload bytes and 20.0 percent at 680, where nineteen regions each round up.

**The corrector is not an authority on the outcome, and a reader must not treat a clean scan as an integrity check.** A (72,64) code has minimum distance four, so it makes no claim beyond two errors. Enumerated over one word: all 64 single-bit patterns repair exactly, all 2,016 double-bit patterns are detected, **23,364 of 41,664 triple-bit patterns are reported as a successful repair while producing the wrong word**, and **5,133 of 635,376 quadruple-bit patterns are reported clean** because the error pattern is itself a codeword. A cryptographic signature remains the only authority on integrity, and any repair must be followed by a fresh verification because a signature check describes the bytes at the moment it ran.

Correction returns a **value** and never writes to the caller's buffer. An in-place corrector would require a mutable borrow, and the read path is allocation-free and immutable by construction.

### Region kinds used by Keleusma's schema

| Kind | Region | Kind | Region |
|---|---|---|---|
| `0x0010` | `STRING_POOL` | `0x001A` | `DATA_SLOTS` |
| `0x0011` | `NAMES` | `0x001B` | `SHARED_LAYOUT` |
| `0x0012` | `CONSTS` | `0x001C` | `PRIVATE_COMPOSITE` |
| `0x0013` | `STRUCT_AUX` | `0x001D` | `DATA_INIT` |
| `0x0014` | `ENUM_AUX` | `0x001E` | `PARAM_TYPES` |
| `0x0015` | `SHAPES` | `0x001F` | `CHUNKS` |
| `0x0016` | `SIGNATURES` | `0x0020` | `NATIVES` |
| `0x0017` | `STRUCT_TEMPLATES` | `0x0021` | `HEADER` |
| `0x0018` | `ENUM_VARIANTS` | `0x0022` | `DEBUG_POOL` |
| `0x0019` | `ENUM_LAYOUTS` | `0x0023` | `NATIVE_RETURNS` |

A region that is absent carries meaning. An absent `DATA_SLOTS` region denotes `None`, whereas an empty one denotes `Some` with no slots: a module with no `data` block and a module whose data block is empty are different programs. By contrast an absent `STRUCT_TEMPLATES` region simply denotes no templates, since that has only one reading. `u32::MAX` is the optional-index sentinel, used for the entry point, a native's return shape, and a chunk's debug pool.

### The forward-ordering invariant

A composite constant references a range of child constants that must lie **strictly after** the composite itself. Roots occupy the constant table's prefix; children are numbered after all roots.

This is load-bearing rather than incidental. It is what allows the table to be materialised bottom-up by a single reverse linear sweep with no stack and no recursion, with the trip count bounded by the table length. A decoder **re-validates** the ordering rather than trusting the encoder that produced its input, because a violation yields a wrong answer rather than a fault.

Two consequences bind any reader. First, a reader fetching one constant must not sweep the whole table; it walks the single constant's reachable set, which the ordering makes terminating because the worklist only ever advances. Second, **only a composite record carries a range.** A scalar record overlays its payload on those same bytes, so "does this record have children" is a question about the tag and never about the range fields. Asking it the other way round reads an integer constant's value as a list of child indices.

### Cost profile

The per-region directory cost is fixed in the number of regions rather than the data volume, and it is paid three times over. For the minimal program `fn main() -> Word { 1 }` the auxiliary body is 912 bytes, of which the 48-byte prologue block and the 720-byte triplicated directory account for roughly 84 percent, leaving 144 bytes of payload.

That ratio is a property of the format rather than a defect, and a minimal program is its worst case. A module with one chunk and one constant pays the directory cost in full, while a real module amortises the same fixed cost across thousands of records.

## Wire format types

The V0.2.0 Phase 7a release ships the following types in `src/wire_format.rs`:

- `WireFormatHeader` mirrors the sixty-four-byte framing header layout. Fields are `pub` for direct access; helpers encode and decode against `[u8; 64]`.
- `OpcodeId` is a `u8` newtype carrying the seven-bit opcode identifier. The mapping table converts to and from the `Op` enum.
- `OpcodeRecord` is a `[u8; 4]` newtype with constructors that take an `OpcodeId` and either inline operand bytes or a pool index, and that compute the parity bit before returning the record.
- `OperandPoolEntry` is a `[u8; 8]` newtype with constructors for the `(u16, u16)`, `(u16, u16, u8)`, and `(u16, u16, u16)` tag variants and a decoder that returns the typed operand on parity success.

The encoder accepts an `Op` and emits an `OpcodeRecord`, queueing pool entries through a `&mut Vec<OperandPoolEntry>` accumulator. The decoder accepts an `OpcodeRecord` and an `&[OperandPoolEntry]` and reconstructs the `Op`. Round-trip tests cover every variant.

## Migration

V0.1.x bytecode artefacts cannot be loaded by V0.2.0 runtimes. Hosts that have V0.1.x bytecode in flight at publication time recompile against the V0.2.0 toolchain. The framing-header `version` field resets to `1` to signal the discontinuity; V0.2.0 runtimes reject V0.1.x bytecode at the framing-level check.

Within the V0.2.0 series, the Phase 7a release shipped the wire-format types and tests but did not route the execution loop through them. Phase 7b switched the producer to emit the section-partitioned body and the consumer to read the opcode stream and operand pool through the new types, with the auxiliary body still an rkyv archive. Phase 7c migrated the auxiliary body to the container specified above and removed the rkyv archive from the execution loop. All three phases are complete. The CRC trailer and the magic remained stable across them.

Version 1 bytecode is rejected at load on the version check. The hazard the no-public-adoption policy had accepted, in which an older-format artifact sharing a version number is accepted and then misread, does not apply across the version 1 to version 2 boundary.

**Not yet done.** The format is emitted and consumed only by the Rust implementation. A Keleusma-language encoder and decoder for this container are specified work that has not been written; the self-hosted compiler stages produce chunk bodies, and the Rust driver assembles the container around them.
