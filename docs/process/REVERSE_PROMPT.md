# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: V0.1-M1 precompiled bytecode loading and trust-based verification skip.
**Status**: Complete.

## Verification

**Commands**:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

**Results**:

- Workspace tests pass. 286 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests. 331 tests total. Up from 323 by seven new precompiled-bytecode tests in `src/vm.rs::tests`.
- Clippy with `--workspace --all-targets`. Zero warnings.
- Format. Clean.
- New dependencies. `serde 1.x` with `derive` and `alloc` features. `postcard 1.x` with `alloc` feature. Both `no_std` plus `alloc` compatible.

## Summary

The Keleusma scripting language now runs precompiled code from any addressable byte slice. The implementation is path A as defined in R39. The Module type is serialized and deserialized through `postcard` with a self-describing magic-and-version header. The deserialized form holds owned heap data and does not borrow from the input slice. The bytecode buffer can persist in `.rodata`, in a file, in `Vec<u8>`, or anywhere else accessible as `&[u8]`. Section placement is the host's responsibility because the runtime crate stays `no_std` plus `alloc`.

A trust-based verification skip is now exposed through `unsafe fn Vm::new_unchecked` and `unsafe fn Vm::load_bytes_unchecked`. Both run structural verification because the VM execution loop relies on those invariants for memory safety. Both skip the WCET and WCMU resource bounds checks. The unsafe marker captures the trust contract. The host attests that the bytecode was previously verified or originates from a trusted compiler. Exceeding the bound at runtime produces an arena allocation failure rather than memory unsafety, so the unsafe marker captures the loss of contract rather than a memory-safety risk.

True zero-copy execution from `.rodata`, where the runtime Module borrows directly from the input buffer with no heap allocation for the parsed form, is recorded as P10 and deferred. The current implementation covers the user's full request for runtime loading from any source, including `.rodata` and other host-binary sections, with the caveat that the parsed form is heap-allocated.

## Changes Made

### Source

- **`src/bytecode.rs`**: `#[derive(Serialize, Deserialize)]` on Value, BlockType, Op, StructTemplate, DataSlot, DataLayout, Chunk, Module. New `BYTECODE_MAGIC` and `BYTECODE_VERSION` constants. New `LoadError` enum with `BadMagic`, `Truncated`, `UnsupportedVersion`, and `Codec` variants. New `Module::to_bytes` and `Module::from_bytes` methods. `core::error::Error` and `Display` impls on `LoadError`.
- **`src/vm.rs`**: New `VmError::LoadError(String)` variant. `From<bytecode::LoadError> for VmError` impl. New `unsafe fn Vm::new_unchecked` and `unsafe fn Vm::new_unchecked_with_arena_capacity`. New `Vm::load_bytes` and `unsafe fn Vm::load_bytes_unchecked` convenience constructors. Internal `Vm::construct` helper deduplicates field initialization between the verifying and unchecked paths. Seven new tests covering roundtrip, header rejection paths, error propagation through `Vm::load_bytes`, and unchecked admission of a module that fails resource bounds verification.
- **`Cargo.toml`**: `serde 1` and `postcard 1` added with `default-features = false` and the appropriate feature flags for `no_std` plus `alloc`.

### Knowledge Graph

- **`docs/decisions/RESOLVED.md`**: New R39 entry recording the wire format, the trust-skip API, the unsafe contract, and the postcard choice.
- **`docs/decisions/PRIORITY.md`**: New P10 entry recording zero-copy execution as the deferred path B.
- **`docs/decisions/BACKLOG.md`**: B9 and B10 cross-reference R39 for the addressed portions.
- **`docs/architecture/EXECUTION_MODEL.md`**: New `## Bytecode Loading` section between Memory Model and Hot Code Swapping. Wire format description, loading API table, trust contract.
- **`docs/process/TASKLOG.md`**: Phase advances to V0.1. Active milestone updated to V0.1-M1. Four task rows added under V0.1-M1. History row added.
- **`docs/process/REVERSE_PROMPT.md`**: This file.

## Unaddressed Concerns

1. **Path B remains deferred.** P10 captures the zero-copy execution requirement. Lifetime-parameterizing Module is a substantial refactor that cascades through Chunk, Op, Value, and the Vm struct. The current implementation supports the user's runtime-loading request with the parsed Module heap-allocated. Path B is admissible as a future milestone once the format and API have settled.

2. **Bytecode versioning has only one version.** A future change to any serialized type bumps `BYTECODE_VERSION`. The crate does not yet provide a migration path between versions. For V0.1, mismatched versions are rejected with `LoadError::UnsupportedVersion`. A future iteration can add explicit migration shims if needed.

3. **The `serde` dependency widens the dependency surface.** Two new transitive dependencies arrive with this change. Both are well-tested and `no_std` compatible. The trade-off favors ergonomics over a custom binary layout. A custom format remains admissible if path B in P10 motivates a representation amenable to in-place execution.

4. **No file-loading helper in the crate.** Per the design choice in this milestone, file I/O is the host's responsibility. Hosts on `std`-bearing platforms call `std::fs::read` and pass the result to `Vm::load_bytes`. Hosts on bare-metal targets place bytecode in `.rodata` or a flash region accessible as `&'static [u8]`.

5. **Native function name resolution at load time is unchanged.** The deserialized Module carries `native_names: Vec<String>`. The host registers natives by name through `Vm::register_fn` and `Vm::register_native` after construction. Future work could explore index-based native resolution baked into bytecode, but this requires a stable native registry shared between compile time and load time.

6. **Trust contract and unsafe API.** The unsafe marker on `Vm::new_unchecked` and `Vm::load_bytes_unchecked` is conservative. Skipping resource bounds checks does not produce memory unsafety in Rust's strict sense, but does weaken the documented bounded-memory and bounded-step guarantees. The decision to require `unsafe` reflects the user's choice in the design discussion. A future iteration could provide a safe `_unchecked` variant if the community settles on the convention that contract violations of this kind do not warrant `unsafe`.

## Intended Next Step

Three paths.

A. V0.1-M2 implementing path B from P10. Lifetime-parameterize Module. Eliminate String fields in favor of byte-offset references. Adopt either a custom binary layout or `rkyv`. This delivers true zero-copy execution from `.rodata` for hosts that want to avoid the heap-allocation cost of the deserialized form.

B. V0.1-M2 returning to P7 follow-on items, namely operand stack and `DynStr` arena migration in the keleusma runtime. The published `keleusma-arena` crate and the new precompiled-loading API both inform the design.

C. V0.1-M2 advancing to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

Recommend A if the precompiled-code use case is the priority. Recommend B if memory-discipline tightening is the priority. Recommend C if language-feature breadth is the priority. The three paths are largely independent. The user's recent direction emphasized embedded use and precompiled distribution, which favors A.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, completed three pre-publication audit and polish passes on `keleusma-arena`, published v0.1.0 to crates.io, switched the keleusma main crate to consume the registry version, and now completes V0.1-M1 implementing precompiled bytecode loading and trust-based verification skip. The phase has advanced to V0.1.
