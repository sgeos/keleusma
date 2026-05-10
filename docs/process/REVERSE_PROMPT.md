# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T45 Refactor SDL3 example into a feature-gated Cargo example.
**Status**: Complete. The previously-introduced `keleusma-piano-roll` workspace member is now a Cargo example at `examples/piano_roll.rs` with SDL3 gated behind the `sdl3-example` feature on the parent crate.

## Verification

**Commands**:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --example piano_roll --features sdl3-example
cargo clippy --example piano_roll --features sdl3-example -- -D warnings
cargo run --example piano_roll                # expect actionable error
(sleep 2; echo) | ./target/release/examples/piano_roll
```

**Results**:

- `cargo build --workspace` completes in seconds without compiling `sdl3` or `sdl3-sys`. The previous workspace-member arrangement triggered a roughly sixty-second CMake build of SDL3 on every `--workspace` invocation; this is fully eliminated.
- `cargo test --workspace` passes. 519 tests across the workspace, unchanged from prior count.
- Clippy clean on the workspace.
- Clippy clean on the feature-gated example.
- `cargo run --example piano_roll` without the feature produces the expected Cargo error: `target piano_roll in package keleusma requires the features: sdl3-example`. The error is actionable.
- The example runs end to end, opens the SDL3 audio device, drives the tick loop, and exits cleanly on stdin Enter.

## Summary

The previous task introduced the SDL3 audio example as a workspace member (`keleusma-piano-roll`). The user observed that this was the wrong shape: an audio demonstration belongs in `examples/`, not as a sibling crate. The workspace-member arrangement was originally chosen to isolate SDL3's heavy build cost from `cargo --workspace` invocations, but the idiomatic Cargo way to handle that is `[[example]]` plus `required-features` plus optional dependency, not workspace structure.

This task implements the correct shape.

### Cargo wiring

The parent `keleusma` Cargo.toml now contains:

```toml
[features]
sdl3-example = ["dep:sdl3"]

[dependencies]
sdl3 = { version = "0.18", features = ["build-from-source-static"], optional = true }

[[example]]
name = "piano_roll"
required-features = ["sdl3-example"]
```

The `package.metadata.docs.rs` block dropped its prior `all-features = true` entry. Without that change, docs.rs builds would attempt to enable `sdl3-example` and trigger an SDL3 source build during documentation generation, which is undesirable.

### File moves

Three files transitioned from the removed workspace-member directory.

- `keleusma-piano-roll/src/main.rs` -> `examples/piano_roll.rs`. The internal `include_str!` path was updated from `"../song.kel"` to `"piano_roll.kel"`. The header doc comment was upgraded from line comments to a `//!` block and absorbed the architecture notes that previously lived in the workspace-member README. The "Run" instruction in the header was updated to the new invocation form.
- `keleusma-piano-roll/song.kel` -> `examples/piano_roll.kel`. No content change.
- `keleusma-piano-roll/README.md` -> deleted. Its content was distilled into the file-level doc comment on `piano_roll.rs`.

The `keleusma-piano-roll/` directory was removed entirely.

### Top-level navigation

The top-level README workspace-crate list was reduced from six entries to five. A new "Examples" section points at `examples/` and the `piano_roll` example with the explicit feature-flag invocation. The workspace member listing in the workspace `Cargo.toml` was reduced to four entries.

## Trade-offs and Properties

Putting SDL3 as an optional dependency on the parent crate, rather than scoping it tightly to the example, accepts a small surface-area expansion on `keleusma`'s declared dependency tree. The benefit is that the standard Cargo convention (optional dep + feature + required-features) does the right thing automatically: the dep does not download, compile, or appear in the dependency graph unless the feature is enabled. Downstream users of `keleusma` see no behavior change because their builds do not enable `sdl3-example`.

The decision to leave the architecture notes in the file-level doc comment on `piano_roll.rs` rather than create a separate `examples/piano_roll.md` keeps the example self-contained: a reader who opens the file sees the rationale alongside the code. The trade-off is that file-level doc comments do not render as nicely on docs.rs as standalone markdown, but examples are not the primary docs.rs surface.

The decision to use `include_str!("piano_roll.kel")` rather than `std::fs::read_to_string("examples/piano_roll.kel")` keeps the example self-contained at runtime. The compiled binary embeds the script bytes; users can run the binary from any working directory. The trade-off is that the example's script is not editable without recompilation; for a demonstration, this is the right trade.

The `sdl3-example` feature name is used per the user's directive and is consistent with the example file name. A more general name like `audio` was considered and rejected because the feature gates SDL3 specifically, not audio capability in general; future audio examples on a different backend would warrant their own feature.

## Files Touched

- **`Cargo.toml`** (workspace root). Removed `keleusma-piano-roll` from `[workspace] members`. Added `[features] sdl3-example`. Added `sdl3` as an optional dependency. Added `[[example]] name = "piano_roll" required-features = ["sdl3-example"]`. Removed `all-features = true` from `package.metadata.docs.rs`.
- **`README.md`** (top-level). Reduced workspace crate list to five. Added an "Examples" section pointing at the gated `piano_roll` invocation.
- **`examples/piano_roll.rs`** (moved from `keleusma-piano-roll/src/main.rs`). Header rewritten as a `//!` doc block absorbing the rationale from the removed workspace-member README. `include_str!` path updated.
- **`examples/piano_roll.kel`** (moved from `keleusma-piano-roll/song.kel`). Unchanged content.
- **`keleusma-piano-roll/`**. Removed.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T45.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The carry-over open priorities from V0.1-M3-T44 still apply (no ADSR envelopes, no audio-output golden test in CI, single-frequency-per-voice limitation, no explicit audio thread priority management, hot code swap not demonstrated, `set_native_bounds` not exercised). None are blocking.

The `package.metadata.docs.rs` change removed `all-features = true`. If any future feature genuinely should be on for docs.rs, an explicit `features = ["..."]` line should replace the absent setting.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session closed an architectural mismatch in the prior task. The audio demonstration is now organized as the user expected. The shape is the idiomatic Rust pattern for examples that depend on heavy optional crates: optional dependency, feature flag, `[[example]] required-features`. Workspace builds stay fast and SDL3-free; the example builds cleanly when explicitly requested.
