# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T51 Publication-readiness verification for `keleusma-arena` and `keleusma-macros`.
**Status**: Complete. Both crates are publication-ready. Two minor gaps in `keleusma-macros` (missing LICENSE file, missing CHANGELOG) closed.

## Verification

**Commands**:

```bash
cargo publish -p keleusma-arena   --dry-run --allow-dirty
cargo publish -p keleusma-macros  --dry-run --allow-dirty
cargo package --list -p keleusma-arena
cargo package --list -p keleusma-macros --allow-dirty
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo doc --no-deps -p keleusma-arena
cargo doc --no-deps -p keleusma-macros
cargo build -p keleusma-arena --target thumbv7em-none-eabihf
cargo build -p keleusma-arena --target thumbv7em-none-eabihf --no-default-features
rustup run 1.85 cargo check -p keleusma-arena
rustup run 1.85 cargo check -p keleusma-arena --no-default-features
```

**Results**:

- `keleusma-arena 0.2.0` dry-run: 13 files, 77.1 KiB packaged, 20.9 KiB compressed. Verification compiles cleanly. No "already exists" warning.
- `keleusma-macros 0.1.0` dry-run after fixes: 8 files, 17.2 KiB packaged, 4.8 KiB compressed. Verification compiles cleanly.
- Workspace tests: 520 pass.
- Clippy clean across the workspace.
- Format clean.
- Rustdoc clean for both crates.
- Arena builds against `thumbv7em-none-eabihf` with both default features (alloc + heap arena) and no-default-features (core-only).
- Arena MSRV (Rust 1.85) verified for default features and no-default-features.

## Summary

The user asked for publication-readiness verification on `keleusma-arena` and `keleusma-macros`. The check ran the full publication-relevant test matrix: dry-run packaging, package contents, MSRV pin, no-std build, cross-architecture build, full test suite, clippy, format, rustdoc.

### keleusma-arena 0.2.0

The crate is publication-ready as-is. Package contents:

```
.cargo_vcs_info.json
CHANGELOG.md
Cargo.lock
Cargo.toml
Cargo.toml.orig
LICENSE
README.md
examples/basic.rs
examples/budget_check.rs
examples/epoch_handle.rs
examples/frame_loop.rs
examples/mixed_allocator.rs
src/lib.rs
```

The five examples are included in the package. CHANGELOG and README are present and accurate following the V0.1-M3-T49/T50 work. License is BSD Zero Clause License; the LICENSE file is included. The 0.2.0 surface is `ArenaHandle<T>`, `from_raw_parts`, the safe `Arena::reset`, `EpochSaturated`, `Stale`, plus the preserved 0.1.0 surface (handles, budget, marks, rewinds, peaks, byte allocators).

### keleusma-macros 0.1.0

Two gaps were found and fixed in this task.

**Missing per-crate LICENSE file.** The workspace root has a `LICENSE` file but the `keleusma-macros` directory did not. Cargo.toml declared `license = "0BSD"`, which is the SPDX identifier; crates.io would have accepted publication with the SPDX identifier alone, but consumers downloading the crate's source would receive a tarball without the actual license text. Fix: `cp LICENSE keleusma-macros/LICENSE`. The license file is identical to the workspace root's because both cover the same project under the same terms.

**Missing CHANGELOG.md.** `keleusma-arena` has one and crates.io users expect to find one alongside any nontrivial published crate. Fix: new `keleusma-macros/CHANGELOG.md` in Keep a Changelog format with a 0.1.0 entry that documents the `KeleusmaType` derive macro, the supported Rust input shapes (named-field structs, all enum variant kinds), and the implementation-detail framing. The framing matches the existing README and Cargo metadata: depend on `keleusma`, not on this crate directly.

After both fixes, the package contents are:

```
.cargo_vcs_info.json
CHANGELOG.md
Cargo.lock
Cargo.toml
Cargo.toml.orig
LICENSE
README.md
src/lib.rs
```

The 8-file package at 17.2 KiB / 4.8 KiB compressed is appropriate for an implementation-detail proc-macro crate.

## Trade-offs and Properties

The decision to copy the workspace `LICENSE` rather than create a different file reflects that `keleusma-macros` is part of the same project under the same license. A symbolic link would also work but is less portable across operating systems and across crates.io's tarball processing; a regular file copy is simpler and unambiguous.

The decision to add `CHANGELOG.md` even for an implementation-detail crate reflects two concerns. First, the version coupling between `keleusma` and `keleusma-macros` may not always be one-to-one across all future versions; a changelog gives the macro crate space to record its own breaking changes when they occur. Second, crates.io users routinely look for a CHANGELOG and its absence raises a small but real question of whether the maintainer is paying attention.

The decision not to bump `keleusma-macros` to 0.2.0 alongside `keleusma-arena` reflects that the macro crate's surface has not changed. The version coupling is at the major-minor of `keleusma` (when that crate releases), not at the major-minor of `keleusma-arena`. The macro crate ships at 0.1.0 because its public API (the derive) is at version 0.1.0 of the `keleusma` API contract.

The decision to verify MSRV for `keleusma-arena` against both default-features and no-default-features reflects that the crate's main embedded use case is core-only (no `alloc`), which is a different code path through the lib (the `with_capacity` constructor is gated behind the `alloc` feature). Verifying both prevents regressions where MSRV-affecting language features sneak into one path but not the other.

The decision not to verify MSRV for `keleusma-macros` reflects that its declared MSRV (1.85) is purely a compatibility statement; the macro crate uses only stable proc-macro APIs (`syn`, `quote`, `proc-macro2`) that have wide MSRV compatibility. Pinning the macros crate to 1.85 in CI would be belt-and-braces but is not currently in the workflow.

## Files Touched

- **`keleusma-macros/LICENSE`** (new). Copy of the workspace `LICENSE`.
- **`keleusma-macros/CHANGELOG.md`** (new). Keep a Changelog 0.1.0 entry.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T51 plus history entry.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The only remaining publication blocker is the manual `cargo publish` step. The order is:

1. `cargo publish -p keleusma-arena` (currently 0.2.0 in source; 0.1.0 already on crates.io).
2. Wait for the index to propagate (typically a few seconds, occasionally longer).
3. `cargo publish -p keleusma-macros` (0.1.0).
4. Wait for the index to propagate.
5. `cargo publish -p keleusma`. The main crate's `Cargo.toml` references `keleusma-arena = "0.2"` and `keleusma-macros = "0.1.0"`; both must be on crates.io before this step succeeds.

The agent does not perform `cargo publish`; the operator does. Once the publication chain completes, the workspace's `cargo publish --dry-run` checks will close out the publication-readiness story.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session ran a thorough publication-readiness check on the two crates that need to publish before the main `keleusma` crate can. `keleusma-arena 0.2.0` was found ready as-is. `keleusma-macros 0.1.0` had two minor packaging gaps (missing per-crate LICENSE, missing CHANGELOG) that have now been closed. Both crates' `cargo publish --dry-run` invocations succeed cleanly. No outstanding correctness, documentation, or metadata issues remain in either crate. The path to crates.io publication is now a sequence of `cargo publish` invocations awaiting operator action.
