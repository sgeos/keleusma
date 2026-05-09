# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M2-T2. P10 Phase 2 step 1. Aligned-input zero-copy validation.
**Status**: Complete. Step 2 (execution against ArchivedModule) remains open.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 344 tests pass workspace-wide. 299 keleusma unit including 19 bytecode tests, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests. Three new bytecode tests for the view path.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

Phase 2 of P10 is split into two steps. Step 1 lands the in-place validation API and the no-copy load path. Step 2 lands the actual zero-copy execution loop and is deferred.

### Step 1 deliverables

`Module::access_bytes(&'a [u8]) -> Result<&'a ArchivedModule, LoadError>` validates the framing (magic, length, CRC residue, version, word size, address size) and runs `rkyv::access` on the body to return a borrowed archived view without copying. Requires the body to be 8-byte aligned within the slice. Because the header is 16 bytes, the body is 8-byte aligned when the slice base itself is 8-byte aligned.

`Module::view_bytes(&[u8]) -> Result<Module, LoadError>` validates through `access_bytes` and deserializes the archived form to an owned `Module`. Compared to `from_bytes`, this skips the `AlignedVec` copy step. The deserialization itself still allocates an owned `Module`.

`Vm::view_bytes(&[u8]) -> Result<Self, VmError>` and `unsafe Vm::view_bytes_unchecked(&[u8]) -> Result<Self, VmError>` compose `Module::view_bytes` with the safe and unchecked `Vm` constructors.

The execution loop continues to operate on the deserialized owned `Module`. The view path is the validation half of true zero-copy. The execution half is step 2.

### Step 2 outline (deferred)

The remaining work for true zero-copy execution.

1. `Vm` gains a lifetime parameter. Storage shifts from `Module` to bytes (owned `AlignedVec` or borrowed `&[u8]`). Module access flows through `&ArchivedModule`.
2. The execution loop is rewritten to read from `&ArchivedModule`. Match arms over `Op` become match arms over `ArchivedOp` with `to_native()` conversions for endian-explicit types. Vector accesses go through `ArchivedVec`. String accesses through `ArchivedString`.
3. A converter from `ArchivedValue` to `Value` is added because the operand stack continues to use owned `Value`. Constants loaded from the bytecode are cloned into `Value` when pushed.
4. The verifier either gains an `ArchivedModule` variant or zero-copy execution is restricted to the unchecked path that skips verification.
5. New tests for execution against borrowed buffers including from `&'static [u8]` placed in `.rodata`.

## Changes Made

### Source

- **`src/bytecode.rs`**: New `Module::access_bytes` returns `&'a ArchivedModule` after framing validation. New `Module::view_bytes` calls `access_bytes` and deserializes. Both require 8-byte alignment of the body. The alignment check uses `is_multiple_of(8)`.
- **`src/vm.rs`**: New `Vm::view_bytes` and `unsafe Vm::view_bytes_unchecked` constructors. Three new tests: `bytecode_view_bytes_runs_aligned_input`, `bytecode_view_bytes_rejects_unaligned_input`, `bytecode_access_bytes_returns_archived_view`.

### Knowledge Graph

- **`docs/decisions/RESOLVED.md`**: R39 updated to describe the view path alongside `from_bytes`. The execution loop limitation and the next-iteration scope are documented.
- **`docs/decisions/PRIORITY.md`**: P10 split into step 1 (complete) and step 2 (open). Step 2 scope listed.
- **`docs/architecture/EXECUTION_MODEL.md`**: Bytecode Loading section gains the new methods in the API table.
- **`docs/process/TASKLOG.md`**: V0.1-M2-T2 row added marking step 1 complete. V0.1-M2-T3 row added for step 2 (open). New history row added.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The view path skips the body copy when the host supplies an aligned slice. For hosts that load bytecode from a file into a `Vec<u8>`, the alignment is not guaranteed and `view_bytes` rejects with a clear error. Hosts that need the alignment can wrap their input in `rkyv::util::AlignedVec`. Hosts that store bytecode in a static buffer with `#[repr(align(8))]` or in a linker section that aligns to at least 8 bytes also satisfy the requirement.

The deserialization step still allocates an owned `Module`. The actual zero-copy execution against the archived form is deferred. The architectural foundation is in place: `Module::access_bytes` exposes the archived view to callers who want to inspect bytecode without deserializing.

The view path is honest about its scope. The naming distinguishes `view_bytes` (in-place validation, no body copy, deserialize for execution) from `load_bytes` (copy to AlignedVec, deserialize for execution). Hosts that benefit from the no-copy validation path use `view_bytes`. Others use `load_bytes`.

## Unaddressed Concerns

1. **Execution path is not yet zero-copy.** Step 2 of Phase 2 is the remaining work. The owned `Module` allocation is still paid at load time. The `Vm` lifetime cascade and execution loop rewrite are required. This is a multi-session refactor properly done.

2. **Alignment requirement is host responsibility.** The view path fails with a clear error on unaligned input. Hosts that want to use the view path must arrange alignment through `AlignedVec`, `#[repr(align(8))]`, or linker placement. The from_bytes path remains available for arbitrary unaligned slices.

3. **The verifier still requires owned Module.** When step 2 lands and zero-copy execution is desired with verification, either the verifier is rewritten to accept `&ArchivedModule` or zero-copy is restricted to the unchecked path. The decision is open.

4. **Float width is not yet parameterized.** Same concern as before. `Value::Float` is always `f64`. A future iteration may add a separate float-size header field for targets that use `f32` natively.

## Intended Next Step

Three paths.

A. Continue P10 Phase 2 step 2. The actual zero-copy execution loop. Multi-session refactor.

B. Pause P10 and pivot to a different priority such as P1 (type checker), P3 (error recovery), or P7 follow-on (operand stack and DynStr arena migration).

C. Publish keleusma main crate to crates.io now that the wire format and load API are stable (subject to the user's go-or-no-go decision).

Recommend A only if the precompiled-distribution story is the priority. The current step 1 already provides validation without copy, which covers many embedded use cases where bytecode comes from a known-aligned location. Full zero-copy execution adds value for the most constrained targets.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, completed three pre-publication audit and polish passes on `keleusma-arena`, published v0.1.0 to crates.io, switched the keleusma main crate to consume the registry version, completed V0.1-M1 implementing precompiled bytecode loading and trust-based verification skip, hardened the wire format with a CRC-32 algebraic self-inclusion trailer, extended the header with length, word size, and address size, pinned a golden-bytes test, shifted word and address fields to base-2 exponent encoding with relaxed acceptance and integer masking, switched the body format from postcard to rkyv as Phase 1 of P10, and now landed Phase 2 step 1 with in-place validation and the no-copy load path.
