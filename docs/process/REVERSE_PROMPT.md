# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T52 Documentation polish for `keleusma-macros`.
**Status**: Complete. Four documentation additions close the gap between "minimal but adequate" and "self-explanatory on docs.rs". The crate keeps its implementation-detail framing.

## Verification

**Commands**:

```bash
cargo doc --no-deps -p keleusma-macros
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo publish -p keleusma-macros --dry-run --allow-dirty
```

**Results**:

- Rustdoc clean for `keleusma-macros`.
- 520 workspace tests pass.
- Clippy clean.
- Format clean.
- `cargo publish -p keleusma-macros --dry-run --allow-dirty`: 8 files, 20.3 KiB packaged, 5.5 KiB compressed. Verification compiles cleanly. Package size grew from 17.2 KiB to 20.3 KiB; the 3 KiB increase is the README and doc-comment expansion.

## Summary

The user observed that the `keleusma-macros` README and crate-level documentation were "adequate but minimal" and asked for four targeted additions.

### README

A new "Supported Input Shapes" section sits between the implementation-detail framing and the stability statement. The section lists what the derive accepts (named-field structs, enums with unit variants, enums with tuple variants, enums with struct-style variants) and what it rejects (tuple structs, unit structs, unions), with a one-line rationale for each rejection. The closing of the section links to the parent crate's `keleusma::KeleusmaType` trait documentation at the canonical docs.rs URL so a reader who needs the full trait contract has a one-click path.

The Stability section gained a link to the new `CHANGELOG.md` so users can find version history.

### Module-level doc comment

The `src/lib.rs` `//!` block was rewritten to mirror the README structure: a one-paragraph framing, an "Implementation detail" subsection that names the parent crate as the user-facing API, a "Supported input shapes" subsection listing the four accepted shapes with example syntax, and a "Rejected inputs" subsection listing the three rejected shapes with the rationale for each. The opening references the parent crate's trait documentation directly through a docs.rs link.

### Derive function doc comment

The `derive_keleusma_type` proc-macro doc went from two lines to a structured block. The first line names the trait. The second sentence explains what the expansion produces. An "Accepted inputs" subsection enumerates the four accepted shapes. A "Compile errors" subsection documents the three rejection paths and their respective error mechanisms (`syn::Error` for unions, `compile_error!` for tuple/unit structs). The block closes with a docs.rs link to the trait.

The minimal style is preserved where it belongs: the public API surface is one derive macro and the trait contract lives in the parent crate. The documentation now points cleanly at the right places without duplicating content from the parent crate.

## Trade-offs and Properties

The decision to enumerate the input shapes in three places (README, module doc, derive doc) reflects the three different reading paths a user takes. A README reader is browsing crates.io. A module-doc reader has clicked into the crate on docs.rs. A derive-doc reader has clicked into the specific derive macro. Each surface should answer the same question with the appropriate granularity. The README is the most user-facing and therefore most prose-oriented; the derive doc is the most reference-oriented and therefore most concise.

The decision to use `https://docs.rs/keleusma/latest/keleusma/trait.KeleusmaType.html` as the canonical trait link rather than a relative reference reflects the absence of a working intra-doc link path between independent crates on docs.rs. Until both crates are published and indexed together, the absolute URL is the only reliable target.

The decision not to expand the documentation with code examples beyond the existing `Point` example reflects that the parent crate already carries the canonical examples in its own documentation. Duplicating them here would create a maintenance hazard if the parent updates an example shape.

The decision to keep tuple-struct and unit-struct rejections explicit in the documentation, rather than treating them as obvious, reflects that Rust users frequently reach for tuple structs as compact wrappers (`pub struct Wrapper(i64);`) and the rejection error message would otherwise be the user's first signal that the shape is not supported. Documenting it in the README and the derive doc puts the constraint on the same surface as the reach.

## Files Touched

- **`keleusma-macros/README.md`**. New "Supported Input Shapes" section. Trait-docs link added. CHANGELOG link added.
- **`keleusma-macros/src/lib.rs`**. Module-level `//!` block expanded. `derive_keleusma_type` doc comment expanded with accepted-inputs and compile-errors subsections.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T52 plus history entry.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The publication chain is unchanged: the operator runs `cargo publish` for `keleusma-arena 0.2.0`, then for `keleusma-macros 0.1.0`, then for `keleusma 0.1.0`. The agent does not perform `cargo publish`.

`keleusma-macros` is now adequate for users landing on docs.rs without ever visiting the parent crate. A reader who lands on either the README or the module doc finds the implementation-detail framing, the input-shape coverage, and a link to the parent crate's trait contract.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session closed the documentation gap on `keleusma-macros`. The four additions are deliberate: enough to make the crate self-explanatory on docs.rs without duplicating the parent crate's user-facing material. The crate remains an intentional implementation detail. The publication chain is one manual step closer to executable.
