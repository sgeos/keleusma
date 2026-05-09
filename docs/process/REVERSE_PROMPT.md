# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M2-T1. P10 Phase 1. Switch body format from postcard to rkyv.
**Status**: Complete. Phase 2 (zero-copy execution) deferred.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 341 tests pass workspace-wide. 296 keleusma unit including 16 bytecode tests, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

Switched the bytecode body format from postcard to rkyv 0.8 in preparation for zero-copy execution. The serde and postcard dependencies are removed. The new dependency is `rkyv` with the `alloc` and `bytecheck` features.

`BYTECODE_VERSION` bumped from three to four. Header padded to sixteen bytes with four reserved bytes that align the body to an eight-byte-aligned offset for `rkyv::access` compatibility. Minimum framing size is now twenty bytes.

The recursive `Value` type carries `#[rkyv(omit_bounds)]` on its self-referential fields (`Tuple`, `Array`, `Struct.fields`, `Enum.fields`) and explicit `serialize_bounds`, `deserialize_bounds`, and `bytecheck(bounds(...))` attributes to break the type-level recursion in the macro expansion.

`Module::from_bytes` copies the body to `rkyv::util::AlignedVec<8>` before calling `rkyv::from_bytes`. The copy ensures alignment regardless of the host slice's alignment. The runtime continues to execute against the owned `Module` form for now. Phase 2 will add a borrowed-buffer path that requires the host to supply an aligned slice and adds a lifetime parameter to the `Vm`.

## Phase 2 Status

Phase 2 is deferred. The work cascades through the entire codebase.

- `Vm` gains a lifetime parameter `Vm<'a>` or a parallel `BorrowedVm<'a>` is added.
- The execution loop is rewritten to read from `&ArchivedModule` instead of `&Module`. Match arms over `Op` become match arms over `ArchivedOp`. Vector accesses go through `ArchivedVec`. String accesses through `ArchivedString`.
- A new constructor `Vm::view_bytes(&'a [u8])` validates framing and obtains `&'a ArchivedModule` through `rkyv::access`.
- The lifetime cascades through every public method that touches the VM and every test.

This is properly its own milestone. Splitting it from Phase 1 lets the wire format change land cleanly and lets Phase 2 begin from a known-good baseline.

## Changes Made

### Source

- **`Cargo.toml`**: Removed `serde` and `postcard`. Added `rkyv 0.8` with `default-features = false` and `alloc` plus `bytecheck` features.
- **`src/bytecode.rs`**: Replaced `serde::{Serialize, Deserialize}` derives with `rkyv::{Archive, Serialize, Deserialize}` derives across `Value`, `BlockType`, `Op`, `StructTemplate`, `DataSlot`, `DataLayout`, `Chunk`, and `Module`. The `Value` enum gains `#[rkyv(serialize_bounds, deserialize_bounds, bytecheck(bounds))]` and `#[rkyv(omit_bounds)]` per recursive field. `BYTECODE_VERSION` bumped to four. `HEADER_LEN` raised to sixteen with four reserved bytes. The `Module::word_bits_log2` and `Module::addr_bits_log2` fields are now part of the rkyv body (no `#[serde(skip)]`). `Module::to_bytes` uses `rkyv::to_bytes`. `Module::from_bytes` copies the body to `AlignedVec<8>` and calls `rkyv::from_bytes`.
- **`src/vm.rs`**: Tests updated for the new minimum framing size (twenty bytes) and the new golden bytes (one-hundred-forty-four bytes for `fn main() -> i64 { 1 }`). The `bytecode_rejects_oversized_length_field`, `bytecode_rejects_undersized_length_field`, and `bytecode_load_via_vm_propagates_load_error` tests updated for the new header length.

### Knowledge Graph

- **`docs/decisions/RESOLVED.md`**: R39 updated. Wire format description includes the rkyv body, the alignment padding, the AlignedVec copy on deserialization, and the deferred Phase 2 path.
- **`docs/decisions/PRIORITY.md`**: P10 updated. Phase 1 marked complete. Phase 2 scope laid out.
- **`docs/architecture/EXECUTION_MODEL.md`**: Bytecode Loading section updated for the new header layout and the rkyv body.
- **`docs/process/TASKLOG.md`**: V0.1-M2-T1 row added marking Phase 1 complete. V0.1-M2-T2 row added for Phase 2 (open). New history row added.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The rkyv encoding is larger than postcard. The serialization of `fn main() -> i64 { 1 }` grew from thirty-seven bytes to one-hundred-forty-four bytes. The growth is due to rkyv's relative pointer and alignment padding overhead, which is the cost paid for in-place addressability. For embedded distribution this is still small in absolute terms.

The `AlignedVec` copy in `from_bytes` adds a memory allocation and copy at load time. The copy is bounded by the body length (a few KB to MB). Phase 2 will add an alignment-required path that skips the copy.

The `bytecheck` feature is enabled to make `from_bytes` safe. Without it, `rkyv::from_bytes` is gated and only `unsafe` access is available. The runtime overhead of bytecheck is modest for the deserialization path.

## Unaddressed Concerns

1. **Wire format size growth.** Rkyv adds substantial padding and pointers. For very small bytecode the overhead dominates. For typical embedded use this is acceptable. A future iteration could explore a more compact format if size matters more than zero-copy access.

2. **Phase 2 lifetime cascade.** `Vm<'a>` propagates through the public API. Some users may prefer the current owned form. A separate `BorrowedVm<'a>` type would coexist with `Vm` at the cost of some duplication.

3. **Bytecheck overhead.** The bytecheck feature adds runtime validation on deserialization. For trusted bytecode this is wasted work. A `from_bytes_unchecked` path that skips validation could be added if it matters.

4. **Endian portability test coverage unchanged.** The golden-bytes test still pins the exact byte sequence. The new format is endian-portable through rkyv's specification, which states all multi-byte integer types are stored in little-endian. The test catches drift the same way it did under postcard.

## Intended Next Step

A. Continue P10 Phase 2. Lifetime-parameterize `Vm`. Rewrite the execution loop to read from `&ArchivedModule`. New `Vm::view_bytes` constructor. Tests for execution against borrowed buffers including from `&'static [u8]` placed in `.rodata`.

B. Pause P10 and pivot to a different priority such as P1 (type checker), P3 (error recovery), or P7 follow-on (operand stack and DynStr arena migration).

C. Publish keleusma main crate to crates.io now that the wire format is settled (subject to the user's go-or-no-go decision).

Recommend A. Phase 2 is the actual user-visible delivery of P10. Phase 1 was foundation. Splitting them is sensible, but Phase 2 should follow promptly so the precompiled-distribution story closes.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, completed three pre-publication audit and polish passes on `keleusma-arena`, published v0.1.0 to crates.io, switched the keleusma main crate to consume the registry version, completed V0.1-M1 implementing precompiled bytecode loading and trust-based verification skip, hardened the wire format with a CRC-32 algebraic self-inclusion trailer, extended the header with length, word size, and address size, pinned a golden-bytes test, shifted word and address fields to base-2 exponent encoding with relaxed acceptance and integer masking, and now switched the body format from postcard to rkyv as Phase 1 of P10.
