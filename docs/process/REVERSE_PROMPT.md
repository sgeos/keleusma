# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: keleusma-arena pre-publication polish.
**Status**: Complete. Crate is publication-ready pending the human pilot's decision to invoke `cargo publish` against the live registry.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --tests --all-targets -- -D warnings
cd keleusma-arena && cargo publish --dry-run --allow-dirty
cd keleusma-arena && cargo +stable miri test
```

**Results**:

- Workspace tests: 278 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit. All pass.
- Clippy: zero warnings.
- Format: clean.
- `cargo publish --dry-run`: packaged 12 files, 54.5 KiB total, 15.1 KiB compressed. No errors.
- miri: 21 of 22 tests pass under stacked borrows. The single ignored test (`arena_from_static_buffer`) deliberately leaks a `Vec` to synthesize a `'static` buffer, which miri flags as a memory leak. The pattern is sound for real `'static` data and is preserved as a behavioral test under stable Rust.

## Summary

Pre-publication polish for `keleusma-arena` v0.1.0. Six items addressed.

1. **crates.io name availability**. `keleusma-arena` is unclaimed.
2. **Drop impl audit comment**. The `Storage` enum carries an extended doc comment explaining why the arena holds a raw pointer with raw provenance rather than an owning `Box<[u8]>`. Allocations through `BottomHandle` and `TopHandle` derive write pointers from a shared `&Arena`, which would be aliasing-unsound under both stacked borrows and tree borrows if the buffer's provenance came through a unique-reference ancestor.
3. **MSRV verification**. `rust-version = "1.85"` matches edition 2024 minimum and is recorded in `keleusma-arena/Cargo.toml`.
4. **miri compliance**. Storage migrated from `Box<[u8]>` to `alloc::alloc::alloc_zeroed` with an explicit 16-byte aligned `Layout`, paired with a matching `dealloc` in the explicit `Drop` impl. Two tests that exercise externally provided buffers (`arena_from_buffer_unchecked` and `arena_from_static_buffer`) are now alignment-tolerant against arbitrary base alignment. The `arena_from_static_buffer` test is annotated with `#[cfg_attr(miri, ignore)]` because it intentionally leaks a `Vec` to obtain a `'static mut [u8]`, and miri's leak detector cannot distinguish that synthetic pattern from a real-world bug.
5. **CHANGELOG.md**. New file at `keleusma-arena/CHANGELOG.md`. Follows Keep a Changelog 1.1.0 conventions. Initial 0.1.0 entry records the public API surface, test coverage, and license.
6. **Non-global-allocator note**. Added to the `Arena` type-level documentation. Explains that the arena is not the program's `#[global_allocator]` and is not intended to be one. References the existing thread safety section.
7. **Mixed allocator example**. New file at `keleusma-arena/examples/mixed_allocator.rs`. Demonstrates the per-frame reset pattern with persistent global-allocator-backed `Vec` and transient arena-backed `ArenaVec` in the same scope.

## Changes Made

### keleusma-arena Crate

- **keleusma-arena/Cargo.toml**: No change beyond what was already in place from the prior commit, which already carried `homepage`, `repository`, `readme`, `rust-version = "1.85"`, and the `0BSD` license.
- **keleusma-arena/src/lib.rs**: Storage migration from `Box<[u8]>` to `alloc_zeroed` with explicit 16-byte alignment. Explicit `Drop` impl with matching `dealloc`. Storage enum reduced to `External` and `Owned` with audit comment. `from_buffer_unchecked` and `from_static_buffer` tests made alignment-tolerant. `arena_from_static_buffer` annotated with `#[cfg_attr(miri, ignore)]`. Non-global-allocator note added to the Arena type-level doc and to the crate-level Thread Safety section.
- **keleusma-arena/CHANGELOG.md**: New file. Initial 0.1.0 entry.
- **keleusma-arena/examples/mixed_allocator.rs**: New file. Demonstrates the arena alongside the global allocator with a per-iteration reset pattern.

### Knowledge Graph

- **docs/process/TASKLOG.md**: Task table extended with V0.0-M6-T12. History row added for the pre-publication polish.
- **docs/process/REVERSE_PROMPT.md**: This file.

## Unaddressed Concerns

1. **Live publication step has not been performed.** The dry-run succeeds. The human pilot should invoke `cargo publish` against crates.io when ready. A version bump beyond `0.1.0` is not warranted at this stage; the crate has not been previously published.

2. **miri leak test.** The `arena_from_static_buffer` test is ignored under miri because of the deliberate `Vec` leak. The behavior under genuine `'static` storage is sound. A future test could use `Box::leak` on a `[u8; N]` array to avoid the slice-pointer indirection that miri's leak detector tracks; this would let the test run unconditionally.

3. **Tree borrows experimental model.** The crate has been validated under miri's stacked borrows, the default. Validation under `MIRIFLAGS=-Zmiri-tree-borrows` has not been performed in this session. The raw-pointer derivation pattern is intended to be sound under both, and the prior session's analysis verified that, but a fresh tree-borrows run would be prudent before publishing.

4. **Rust version policy.** MSRV is set to 1.85 to align with edition 2024. The crate uses no features beyond stable Rust and `allocator-api2`. Bumping MSRV is not anticipated unless a future API change requires it.

5. **Concurrency contract.** The crate is single-threaded. The crate-level documentation states this. Hosts that want a thread-safe wrapper must build one. The current crate intentionally does not provide one.

## Intended Next Step

Three paths.

A. Invoke `cargo publish` for `keleusma-arena` to push v0.1.0 to crates.io. Optionally run `MIRIFLAGS=-Zmiri-tree-borrows cargo +nightly miri test` first.

B. V0.0-M7 implementing P7 follow-on items, namely operand stack and `DynStr` arena migration in the keleusma runtime. The published arena crate would serve as the substrate.

C. Pivot to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

Recommend A because the pre-publication polish work is complete and the crate sits in a publishable state. B and C remain reasonable options if publication is to be deferred.

Await human prompt before proceeding.

## Session Context

This session began with the V0.0-M5 and V0.0-M6 work already complete and the arena extracted into a workspace crate. Subsequent work resolved P8 (call-graph WCMU integration with auto-arena sizing) and P9 (strict-mode bounded-iteration loop analysis). The session concludes with the publication-readiness pass for `keleusma-arena` v0.1.0.
