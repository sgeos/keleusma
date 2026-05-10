# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T48 Pre-publication polish.
**Status**: Complete. Five quality items addressed; the one hard publication blocker (`keleusma-macros` not yet on crates.io) is left for manual operator action.

## Verification

**Commands**:

```bash
cargo doc --no-deps                                 # zero warnings
cargo test --workspace                              # 519 pass
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo build -p keleusma --target thumbv7em-none-eabihf   # no_std verified
cargo build --release --example piano_roll --features sdl3-example
(printf '\n') | ./target/release/examples/piano_roll
```

**Results**:

- Rustdoc clean. The four prior warnings are gone.
- 519 workspace tests pass.
- Clippy clean across the workspace and the feature-gated example.
- Format clean.
- `keleusma` builds for `thumbv7em-none-eabihf` (verified locally; CI now exercises this on every push).
- The piano-roll example builds and smoke-tests cleanly after the `Module` re-export.

## Summary

The user observed that the SDL3 example surfaced an awkward `keleusma::bytecode::Module` import path and asked whether `keleusma-macros` makes sense outside `keleusma`. Combined with the publication-readiness review from the prior turn, this session closed five items.

### keleusma-macros stance

`keleusma-macros` is the proc-macro backend for the `KeleusmaType` derive. Its expansion produces impl blocks that reference the `KeleusmaType` trait defined in `keleusma::marshall`. Standalone, the macro generates code that does not compile, so the crate has no use independent of `keleusma`. Publication is required only because Cargo demands proc-macro crates to be separate libraries; the same shape is established practice with `serde` + `serde_derive` and `tokio` + `tokio-macros`. The session marked this explicitly through Cargo metadata and a new README that tells users to depend on `keleusma` and treat `keleusma-macros` as an implementation detail.

### Rustdoc warnings

Four warnings cleared at the source.

1. `bytecode::Module::access_bytes` doc referenced private `HEADER_LEN` through a `[` `]` link. Rewritten as prose ("the header is sixteen bytes") since the constant is internal.
2. `typecheck` module doc referenced private `Ctx::fresh`. Rewritten as prose ("the internal context") since the type is module-private.
3. `verify::verify_resource_bounds_with_cost_model` referenced bare `CostModel`, which rustdoc could not resolve from the verify module's namespace. Replaced with the absolute path `[`crate::bytecode::CostModel`]`.
4. The `Vm` struct doc referenced `Vm::reset_arena`, which does not exist. The actual method is `Vm::reset_after_error`. Updated the link to the correct method.

The first two were latent doc bugs visible only to docs.rs readers. The third and fourth would have produced unresolved links on docs.rs; the fourth in particular was actively misleading.

### Module re-export

`pub use bytecode::Module` was added to `lib.rs`. The piano-roll example was updated to import as `use keleusma::Module`. Both forms continue to work because `Module` remains accessible at `keleusma::bytecode::Module`. This closes the small ergonomic wart the example surfaced.

### CHANGELOG.md

A new file at the workspace root in Keep a Changelog format. The 0.1.0 entry documents the V0.1.0 public surface across seven sections: Language, Runtime, Verification, Host Interface, Tooling, Examples, and Documentation. Closing notes call out the 0.x stability expectation, the workspace member relationships, and the implementation-detail status of `keleusma-macros`.

### CI workflow

The single `msrv` job split into two: `msrv-arena` pinning to 1.85 (the arena's MSRV) and `msrv-keleusma` pinning to 1.87 (the main crate's actual MSRV). The previous CI did not pin the main crate's MSRV at all, so MSRV drift went undetected.

A new `no-std` job builds `keleusma` against `thumbv7em-none-eabihf`, an embedded ARM target without `std`. The crate's `#![no_std]` attribute and `extern crate alloc;` declarations are now machine-verified per push, not just stated.

### keleusma-macros metadata

Cargo.toml gained homepage, repository, documentation, readme, rust-version, keywords, and categories. A new README.md tells users to depend on `keleusma` rather than this crate directly, mirrors the serde + serde_derive shape, and documents the version-coupling expectation.

## Trade-offs and Properties

The decision to use prose rather than `[Type]` links for `HEADER_LEN` and `Ctx::fresh` reflects that those identifiers are private. Two alternatives existed: make them `pub(crate)` and link to them, or make them `pub` and link to them. The first does not satisfy rustdoc's link resolver (which scans only public items). The second over-exposes implementation detail. Prose was the right answer; the doc no longer makes a promise about a specific identifier name that could shift.

The decision to split MSRV into per-crate jobs reflects that the workspace contains crates with different MSRVs (1.85 for arena, 1.87 for keleusma), and the current MSRV declarations should be verified separately. A unified MSRV at 1.87 would be simpler but would force arena downstream consumers off 1.85; a unified MSRV at 1.85 would prevent keleusma from using 1.87-only constructs (which it does, hence the discrepancy). Two separate jobs preserve both the arena's wider compatibility and keleusma's freedom to use newer language features.

The decision to verify no_std against `thumbv7em-none-eabihf` rather than `aarch64-unknown-none` or `wasm32-unknown-unknown` reflects the embedded-scripting target audience. ARM Cortex-M is the canonical embedded target the crate's `embedded` and `no-std` Cargo categories speak to. Verifying against one such target is sufficient to catch most no_std regressions; future tasks could broaden the matrix.

The CHANGELOG entry is more verbose than the arena's. The arena was a focused allocator; keleusma is a language with seven distinct surfaces. Verbose is appropriate here because the changelog is a user's first survey of the V0.1.0 capabilities and is the document linked from crates.io.

## Files Touched

- **`src/bytecode.rs`**. Doc comment on `Module::access_bytes` reworded.
- **`src/typecheck.rs`**. Module-level doc comment reworded.
- **`src/verify.rs`**. Doc comment on `verify_resource_bounds_with_cost_model` uses absolute path for `CostModel`.
- **`src/vm.rs`**. Doc comment on `Vm` struct corrected to reference `Vm::reset_after_error`.
- **`src/lib.rs`**. `Module` added to the bytecode re-exports.
- **`examples/piano_roll.rs`**. Import simplified to `use keleusma::{Arena, Module, Value}`. Format pass applied.
- **`CHANGELOG.md`** (new). Keep a Changelog format. V0.1.0 entry covering the seven public surfaces.
- **`.github/workflows/ci.yml`**. `msrv` job split into `msrv-arena` and `msrv-keleusma`. New `no-std` job.
- **`keleusma-macros/Cargo.toml`**. Metadata enriched.
- **`keleusma-macros/README.md`** (new). Implementation-detail framing.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T48 plus history entry.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The remaining hard blocker before crates.io publication is `keleusma-macros` not yet existing on crates.io. The publication order is `keleusma-arena` (already published), `keleusma-macros` (next), then `keleusma` (final). Publication is the operator's decision, not the agent's; the dry-run command is `cargo publish -p keleusma-macros --dry-run` followed by `cargo publish -p keleusma-macros`. Once the macro crate is on crates.io and propagated through the registry, `cargo publish -p keleusma --dry-run` should succeed and the main crate can publish.

Items deferred from prior tasks remain. None are blockers.

## Intended Next Step

Await human prompt before proceeding. The natural next step the agent cannot take is `cargo publish` on `keleusma-macros` and `keleusma`.

## Session Context

This session put the codebase in publishable shape modulo the manual `cargo publish` step. The crates.io rendering will pick up an accurate description, a Keep a Changelog file, complete metadata, and a README. The docs.rs rendering will be free of warnings and unresolved links. The CI workflow will verify both the language-level MSRV claim and the no_std + alloc target claim on every push. The `keleusma-macros` crate is now framed as an implementation detail rather than a peer crate, which is the truthful relationship and matches established Rust ecosystem precedent.
