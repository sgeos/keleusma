# The gate is green on this branch, and it has a hole one warning wide

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Run 2026-09-02** on the tree after absorption 46.
Brief: [`OPAQUE_ABSORPTION_BRIEF.md`](./OPAQUE_ABSORPTION_BRIEF.md).

---

## GREEN. Fourteen steps, exit 0.

| step | result |
|---|---|
| default features | 117 binaries, 2739 tests |
| `keleusma` no default features | 99 binaries, 297 tests |
| `keleusma` `signatures` | 99 binaries, 2347 tests |
| `keleusma` `signatures,shell` | 99 binaries, 2364 tests |
| `keleusma` `self-host` | 99 binaries, 2518 tests |
| `keleusma-wire` all features / none | 5 binaries, 57 tests / 5 binaries, 20 tests |
| docs `-D warnings`, Markdown links | ran, no tests to report |
| detached `compiler/` | 9 binaries, 86 tests |
| **detached `native_codegen/`** | **88 binaries, 459 tests** |

**A total across steps is not a test count.** The same suite runs under several feature sets.

**This is the first full gate run on this line**, and it closes the last verification gap before the
back-merge. The two steps that had never run here — workspace `clippy --all-targets -D warnings` and
`cargo doc` at the docs.rs feature sets — both passed.

**The native step ran rather than skipping**, at 88 binaries and 459 tests, so the backend this line
exists to build was actually compiled and tested by the release gate rather than merely by its own
package suite.

## ⚠ THE GATE PRINTED A WARNING AND STAYED GREEN, AND THE REASON IS A HOLE BETWEEN TWO INSTRUMENTS

```text
warning: unused import: `alloc::vec`
 --> src/vm.rs:8:5
```

Emitted during **`Tests — keleusma no default features`**. Measured, forcing a fresh lint each time:

| invocation | warnings |
|---|---|
| `clippy -p keleusma --no-default-features` (lib only) | **0** |
| `clippy -p keleusma --no-default-features --all-targets` | **1** |
| `clippy -p keleusma --all-targets` (default features) | **0** |

So the warning lives in exactly one cell: **no-default-features × test targets.**

**The gate has two instruments and neither covers that cell.** Its lint step is
`cargo clippy --workspace --all-targets -- -D warnings`, which **denies** warnings but only under
**default features**. Its no-default step is `cargo test`, which **reaches** that feature set but does
**not deny** warnings — it prints them and passes.

**Each instrument is correct within its own scope, and the union of the scopes has a hole.** That is
the session's recurring shape applied to the gate itself: *"clippy `-D warnings` is green"* is true
**under default features**, and the sentence does not say so.

**The file is `src/vm.rs`, which this line does not own.** Reported to the `v0.2.3` line rather than
repaired here.

## What a green gate here does and does not establish

**Does**: every step of the project's own gate passes on this branch, with the corpus, the added
witnesses, and the absorbed `Opaque` change all present — the pairing that the back-merge will make
permanent.

**Does not**: cover configurations the gate itself omits. The cell above is one. `--all-features` is
another, and it is not a supported configuration.
