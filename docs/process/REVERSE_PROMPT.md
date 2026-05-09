# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: keleusma-arena pre-publication final pass.
**Status**: Complete. The crate is publication-ready. The remaining step is the human pilot's invocation of `cargo publish` against the live registry.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p keleusma-arena --doc
cargo +1.85 -C keleusma-arena check
cargo +1.85 -C keleusma-arena check --no-default-features
cd keleusma-arena && cargo +nightly miri test
cd keleusma-arena && MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test
```

**Results**:

- Workspace tests. 278 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit, 1 keleusma-arena doctest. All pass.
- Clippy with `--workspace --all-targets`. Zero warnings.
- Format. Clean.
- Doctest on `Arena::with_capacity`. Passes.
- Rust 1.85 build of keleusma-arena. Builds with default features and with `--no-default-features`.
- miri stacked borrows. 21 of 22 tests pass. The single ignore is `arena_from_static_buffer`, which deliberately leaks a `Vec` to obtain a `'static mut [u8]`.
- miri tree borrows. 21 of 22 tests pass. Same single ignore. The raw-pointer derivation pattern is sound under both aliasing models.

## Summary

Six items were addressed before publication.

1. **Tree borrows verification.** Ran `MIRIFLAGS=-Zmiri-tree-borrows cargo miri test`. Clean. The arena's allocation pattern derives write pointers through a shared reference to a raw-provenance buffer, which is sound under both stacked borrows and tree borrows.
2. **docs.rs metadata.** Added `[package.metadata.docs.rs]` block with `all-features = true` and `rustdoc-args = ["--cfg", "docsrs"]`. The `docsrs` cfg activates `#[doc(cfg(...))]` annotations on items behind the `alloc` feature. The metadata block also ensures all features are documented when the crate renders on docs.rs.
3. **CI miri job.** Added a `miri` job to `.github/workflows/ci.yml` that installs miri on nightly and runs both stacked borrows and tree borrows against `keleusma-arena`. Future regressions in unsafe code will be caught at PR time.
4. **CI MSRV job.** Added an `msrv` job pinned to `1.85`. The job runs `cargo check -p keleusma-arena` with default features and with `--no-default-features` to verify the `core`-only path. MSRV drift will be caught.
5. **CI clippy upgraded.** Changed `cargo clippy --tests -- -D warnings` to `cargo clippy --workspace --all-targets -- -D warnings`. Examples are now lint-checked, including `mixed_allocator` and the `wcmu_*` examples in the keleusma crate.
6. **Doctest.** Added a runnable example on `Arena::with_capacity` demonstrating construction, allocation through `stack_handle()` into `allocator_api2::vec::Vec::new_in`, and observability via `bottom_used()`. The doctest is wired through `cargo test --doc` and catches documentation drift on the primary entry point.

## Changes Made

### keleusma-arena Crate

- **keleusma-arena/Cargo.toml**: Added `[package.metadata.docs.rs]` block.
- **keleusma-arena/src/lib.rs**: Added `# Examples` doctest to `Arena::with_capacity`.

### CI

- **.github/workflows/ci.yml**: Test job now runs `cargo test --workspace`. Check job runs `cargo check --workspace --all-targets`. Clippy job runs `cargo clippy --workspace --all-targets`. New `msrv` job pins Rust 1.85. New `miri` job runs both stacked and tree borrows against keleusma-arena.

### Knowledge Graph

- **docs/process/TASKLOG.md**: V0.0-M6-T13 row added. History row added.
- **docs/process/REVERSE_PROMPT.md**: This file.

## Unaddressed Concerns

1. **The crate has not been published.** All preparation is complete. The human pilot should invoke `cargo publish` against crates.io when ready. The dry run succeeded.

2. **The single miri-ignored test.** `arena_from_static_buffer` constructs a `&'static mut [u8]` by leaking a `Vec`. miri's leak detector cannot distinguish that synthetic pattern from a real bug. The behavior is sound under genuine `'static` storage. Unchanged from prior session. A future test using `Box::leak` on a `[u8; N]` array would let the test run unconditionally because miri tracks the leaked allocation differently. Not blocking.

3. **CI mac and Windows coverage.** CI runs on `ubuntu-latest` only. The arena uses no platform-specific code, but downstream embedded users on Cortex-M targets will not exercise the same toolchain configuration. A `runs-on` matrix is admissible but is overhead the human pilot may decide is not worth it for v0.1.0.

4. **Crate-level documentation freshness.** Only one method has a doctest. The crate-level documentation in `lib.rs` and the `README.md` quickstart are not compile-verified. This is mitigated by the four working examples, which are compile-checked by clippy. Adding `#![doc = include_str!("../README.md")]` at the crate root would unify the README and crate-level docs at the cost of a small refactor.

5. **Concurrency contract.** Single-threaded by design. Documented. Hosts that need a thread-safe wrapper must build one. This is a stated design choice, not a deficiency.

## Intended Next Step

Three paths.

A. Push the branch. Invoke `cargo publish` for `keleusma-arena` to push v0.1.0 to crates.io.

B. V0.0-M7 implementing P7 follow-on items, namely operand stack and `DynStr` arena migration in the keleusma runtime. The published arena crate would serve as the substrate.

C. Pivot to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

Recommend A. The pre-publication audit is exhausted. Tree borrows is clean. CI guardrails are in place. The crate is ready for the registry. Following A with B preserves the natural sequence in which `keleusma-arena` first lands as a published crate, then the runtime adopts it through its published version rather than a path dependency.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 (call-graph WCMU integration with auto-arena sizing) and P9 (strict-mode bounded-iteration loop analysis), then completed two pre-publication passes on `keleusma-arena`. The crate is now publication-ready with miri verification under both aliasing models, CI guardrails for miri and MSRV, complete Cargo.toml metadata, a CHANGELOG, working examples, a doctest, and an explicit Drop and storage discipline.
