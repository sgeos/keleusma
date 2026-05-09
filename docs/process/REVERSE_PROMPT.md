# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-08
**Task**: keleusma-arena docs polish.
**Status**: Complete. The crate is publication-ready. The remaining step is the human pilot's invocation of `cargo publish` against the live registry.

## Verification

**Commands**:

```bash
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p keleusma-arena --doc
RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc -p keleusma-arena --no-deps --all-features
```

**Results**:

- Workspace tests. 278 keleusma unit, 17 keleusma integration, 22 keleusma-arena unit, 6 keleusma-arena doctests. 323 tests pass total. All pass.
- Clippy with `--workspace --all-targets`. Zero warnings.
- Format. Clean.
- Doctests. Six pass. Five from the README and one on `Arena::with_capacity`.
- docs.rs simulation. Generated under `--cfg docsrs --all-features`. The `stab portability` marker confirming `#[doc(cfg(feature = "alloc"))]` rendering is present in the generated HTML for `Arena::with_capacity`. The published documentation will display an `Available on crate feature alloc only` badge.

## Summary

Two final docs items were addressed.

1. **Activate docs.rs feature badge.** Added `#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]` to `Arena::with_capacity`. The crate root already carried `#![cfg_attr(docsrs, feature(doc_cfg))]` and the Cargo.toml metadata sets the `docsrs` cfg on docs.rs. Without the per-item annotation, the metadata had no observable effect. The annotation now causes docs.rs to render an `Available on crate feature alloc only` badge on the method.

2. **Wire README into crate-level documentation.** Added `#![doc = include_str!("../README.md")]` to `src/lib.rs`. The README's five code blocks now run as doctests through `cargo test --doc`. The existing structured reference content is preserved as a `## API Reference` subsection following the README intro. Section headers were demoted from `#` to `##` for nesting consistency with the README's `## Quick Start`, `## Static-Buffer Use`, and similar sections.

3. **Fix the Static-Buffer Use README example for edition 2024.** The original example used `BUFFER.as_mut_ptr()` and `BUFFER.len()`, which create implicit references to a `static mut` and were tightened to a hard error in edition 2024. Replaced with `core::ptr::addr_of_mut!(BUFFER) as *mut u8` and an explicit length. The constructor function form was inlined into top-level statements to make the example a runnable doctest without an unused `fn make_arena` warning.

## Changes Made

### keleusma-arena Crate

- **keleusma-arena/src/lib.rs**: Replaced the intro paragraph of the crate-level `//!` docs with `#![doc = include_str!("../README.md")]`. Demoted the structured reference section headers from `#` to `##` and grouped them under a new `## API Reference` parent. Added `#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]` to `Arena::with_capacity`.
- **keleusma-arena/README.md**: Rewrote the `Static-Buffer Use` example to use `core::ptr::addr_of_mut!` and inline form so the code compiles cleanly under edition 2024 and runs as a doctest.

### Knowledge Graph

- **docs/process/TASKLOG.md**: V0.0-M6-T14 row added. History row added.
- **docs/process/REVERSE_PROMPT.md**: This file.

## Unaddressed Concerns

1. **The crate has not been published.** All preparation is complete. The human pilot should invoke `cargo publish` against crates.io when ready.

2. **Mixing of section headers in rendered crate docs.** When docs.rs renders the included README, the README's `# keleusma-arena` H1 produces a heading inside a page that already has the crate name as the page title. This is a common pattern across the Rust ecosystem and is generally tolerated, but it does create a visual double-H1. A future iteration could replace the README's H1 with content that flows from the crate name, but the trade-off is divergence between GitHub README rendering and docs.rs rendering. Not blocking.

3. **The single miri-ignored test.** Unchanged. `arena_from_static_buffer` deliberately leaks a `Vec` to obtain a `'static mut [u8]`. Sound under genuine `'static` storage; flagged by miri's leak detector under the synthetic test pattern.

4. **CI mac and Windows coverage.** CI runs on `ubuntu-latest` only. Not blocking for v0.1.0.

## Intended Next Step

A. Push the branch. Invoke `cargo publish` for `keleusma-arena` to push v0.1.0 to crates.io.

B. V0.0-M7 implementing P7 follow-on items, namely operand stack and `DynStr` arena migration in the keleusma runtime. The published arena crate would serve as the substrate.

C. Pivot to V0.1 candidates such as the type checker (P1), for-in over arbitrary expressions (P2), or error recovery (P3).

Recommend A. The pre-publication audit is now closed. All documented items are addressed. The crate has miri verification under both aliasing models, CI guardrails, complete Cargo.toml metadata, a CHANGELOG, working examples, six doctests, and an explicit Drop and storage discipline.

Await human prompt before proceeding.

## Session Context

This session began with V0.0-M5 and V0.0-M6 already complete and the arena extracted into a workspace crate. The session resolved P8 and P9, then completed three pre-publication passes on `keleusma-arena`. The crate is now publication-ready.
