# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-09
**Task**: V0.1-M2-T4. P10 Phase 2 step 2 execution loop refactor.
**Status**: Complete. P10 is now resolved across all phases.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

**Results**:

- 347 tests pass workspace-wide. 302 keleusma unit including 22 bytecode tests and the new `vm_view_bytes_zero_copy_executes_against_borrowed_buffer` test, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests.
- Clippy clean under `--workspace --all-targets`.
- Format clean.

## Summary

True zero-copy execution against an `&'a ArchivedModule` is now in place. The Vm reads bytecode directly from a borrowed buffer with no owned `Module` materialized.

The refactor cascaded through the Vm and several callers.

- `Vm` gained the lifetime parameter `Vm<'a>`. Internal storage shifted to a `BytecodeStore<'a>` enum carrying either an `Owned` `AlignedVec<8>` (for VMs constructed from `Module` or unaligned bytes) or a `Borrowed` `&'a [u8]` (for the new zero-copy constructor).
- A private `archived()` helper returns `&ArchivedModule` from the bytecode storage via `rkyv::access_unchecked`. The bytes were validated at construction, so the unchecked access is sound.
- The execution loop now reads through per-access converter helpers. `chunk_op` returns an owned `Op` via `op_from_archived`. `chunk_const` returns an owned `Value` via `value_from_archived`. `chunk_const_str`, `struct_template`, `native_name`, `chunk_op_count`, `chunk_local_count`, and `word_bits_log2` cover the remaining access patterns.
- Cold-path methods (`verify_resources`, `auto_arena_capacity`) deserialize the bytecode to an owned `Module` on call via `module_owned()` and operate on it. The hot execution path never materializes the owned form.
- `replace_module` serializes the new module to `AlignedVec` and replaces the storage. The Vm's lifetime parameter is unchanged because the new bytes are always owned.
- `register_utility_natives`, `register_audio_natives`, and the `build_vm` helper in the marshall integration test all updated to thread the lifetime parameter through their signatures.
- A new `unsafe Vm::view_bytes_zero_copy(&'a [u8])` constructor stores the borrowed slice directly without deserialization. Validates the framing only. The execution loop runs against the buffer's `&ArchivedModule` for as long as the borrowed lifetime is valid.
- A new test `vm_view_bytes_zero_copy_executes_against_borrowed_buffer` compiles a program, serializes to an `AlignedVec`, constructs a `Vm<'_>` borrowing the slice, and confirms the program executes correctly with no owned `Module` materialized.

## API Surface

The runtime now supports five entry points spanning the design space.

| Entry point | Source | Verification | Allocation |
|---|---|---|---|
| `Vm::new(Module)` | Owned module | Full | Serializes module internally for archived access |
| `Vm::load_bytes(&[u8])` | Unaligned bytes | Full | Body copy to `AlignedVec` before deserialize |
| `Vm::view_bytes(&[u8])` | Aligned bytes | Full | Skip body copy. Deserialize for verification then store. |
| `unsafe Vm::view_bytes_unchecked(&[u8])` | Aligned bytes | Skip resource bounds | Same as `view_bytes` minus bounds check |
| `unsafe Vm::view_bytes_zero_copy(&'a [u8])` | Aligned bytes | Skip all checks | True zero-copy. Borrow the buffer. |

The zero-copy path is the strongest unsafe contract because it skips all verification including structural verification. Hosts use it when the bytecode is known good (typically because it was verified by the build pipeline).

## Changes Made

### Source

- **`src/vm.rs`**: New `BytecodeStore<'a>` enum and `archived()` helper. `Vm` struct gained lifetime parameter `Vm<'a>`. `CallFrame` derives `Copy`. New helpers `module_owned`, `chunk_op`, `chunk_const`, `chunk_op_count`, `chunk_local_count`, `chunk_const_str`, `struct_template`, `native_name`, `chunk_count`, `word_bits_log2`. `construct` returns `Result<Self, VmError>` and serializes the module to `AlignedVec`. All 24 access sites for `self.module.X` converted to use the helpers. `replace_module` re-serializes. `verify_resources` and `auto_arena_capacity` deserialize on call. New `unsafe fn view_bytes_zero_copy(bytes: &'a [u8])` constructor. New `vm_view_bytes_zero_copy_executes_against_borrowed_buffer` test.
- **`src/utility_natives.rs`**: `register_utility_natives` gained the `<'a>` lifetime parameter on its signature.
- **`src/audio_natives.rs`**: `register_audio_natives` gained the `<'a>` lifetime parameter on its signature.
- **`tests/marshall.rs`**: `build_vm` return type changed from `Vm` to `Vm<'_>`.

### Knowledge Graph

- **`docs/decisions/PRIORITY.md`**: P10 marked resolved across all phases. Strikethrough on the heading. Full design space table added.
- **`docs/process/TASKLOG.md`**: V0.1-M2-T4 row marked complete. New history row added.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Trade-offs and Properties

The hot path now does a discriminant match plus a `to_native()` conversion per op-fetch through `op_from_archived`. The cost is comparable to a clone on a small `Op` value and is dominated by the match dispatch. Constants are converted via `value_from_archived` which is more expensive for composite values because of recursive descent and heap allocations for `Vec<Value>` and `String`. For typical Keleusma programs that execute many instructions per second, the overhead is acceptable.

The zero-copy path skips all verification. This is intentional. Hosts that use this path attest the bytecode is well-formed. Adversarial bytecode could cause arbitrary VM behavior including reads or writes through invalid frame state. The unsafe marker captures this contract.

The owned-input paths (`Vm::new`, `Vm::load_bytes`) now serialize the module internally. This is a one-time cost at construction. Subsequent execution uses the archived form like the zero-copy path. The unified execution path simplifies the maintenance burden compared to alternatives such as a parallel `VmView<'a>` type.

## Unaddressed Concerns

1. **Zero-copy execution skips structural verification.** The unsafe contract requires the host to attest that block nesting, jump offsets, and the productivity rule are valid. Future work could add a verification pass that operates on `&ArchivedModule` to provide a safe zero-copy entry point. The pass would mirror `verify::verify` but read through archived accessors.

2. **Per-op converter overhead.** Every op-fetch and constant-load runs a discriminant match. Performance is acceptable for typical programs. Hot loops could benefit from caching the chunk's op slice or specializing the dispatch. Future optimization.

3. **String constants clone on access.** When the bytecode declares `Value::StaticStr("hello")`, the runtime materializes a fresh `String` for each load. The clone happens at the converter boundary. A future variant could return `Cow<'a, str>` to avoid cloning when the host buffer outlives the operand stack lifetime, at the cost of more lifetime annotations.

4. **Float width is still hardcoded.** `Value::Float` is `f64`. Targets that use `f32` natively would need a separate float-size header field. Tracked under B10.

5. **Zero-copy from `&'static [u8]` placed in `.rodata` is supported in principle but not exercised by an explicit test.** The `vm_view_bytes_zero_copy_executes_against_borrowed_buffer` test uses an `AlignedVec` rather than a static array. A test using a `static` with `#[repr(align(8))]` would close the loop. Not currently a defect.

## Intended Next Step

P10 is now resolved. The precompiled-distribution story is complete from compile through wire format through zero-copy execution.

Three paths.

A. Pivot to P1 (type checker). Currently the compiler emits bytecode without type checking. Adding a semantic analysis pass would catch type errors at compile time rather than runtime.

B. Pivot to P3 (error recovery model). A runtime error currently halts execution. Defining recovery semantics is necessary for safety-critical control systems.

C. Pivot to P7 follow-on (operand stack and DynStr arena migration). The arena is in place but the operand stack and dynamic strings still use the global allocator. Routing them through the arena completes the bounded-memory guarantee.

D. Publish keleusma main crate to crates.io now that the wire format and load API are stable.

Recommend C if the bounded-memory guarantee is the priority. Recommend A or B if the safety-critical positioning is the priority. Recommend D if external visibility is the priority.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, completed three pre-publication audit and polish passes on `keleusma-arena`, published v0.1.0 to crates.io, switched the keleusma main crate to consume the registry version, completed V0.1-M1 implementing precompiled bytecode loading and trust-based verification skip, hardened the wire format with a CRC-32 algebraic self-inclusion trailer, extended the header with length, word size, and address size, pinned a golden-bytes test, shifted word and address fields to base-2 exponent encoding with relaxed acceptance and integer masking, switched the body format from postcard to rkyv, landed the in-place validation and no-copy load path, landed the conversion helper foundations, and now completed the full Vm refactor with true zero-copy execution against borrowed bytecode buffers. P10 is resolved across all phases.
