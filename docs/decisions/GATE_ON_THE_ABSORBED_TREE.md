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

**The file is `src/vm.rs`, which this line does not own.** Reported to the `v0.2.3` line, which fixed
it: the import was gated on `test` while its only consumer was gated on `all(test, compile, verify)`,
so under no-default-features the module vanished and the import did not.

### ⚠ THE OBVIOUS REMEDY DOES NOT WORK, AND THIS RECORD IMPLIED IT WOULD

The natural fix is to add `--no-default-features` to the gate's lint step. **The `v0.2.3` line
measured it, found it does nothing, and said so before proposing it. Reproduced independently here,
forcing a fresh lint each time:**

| invocation | unused-import warnings |
|---|---|
| `clippy --workspace --all-targets` | 0 |
| `clippy --workspace --all-targets --no-default-features` | **0 — the flag is accepted and has no effect** |
| `clippy -p keleusma --all-targets` | 0 |
| `clippy -p keleusma --all-targets --no-default-features` | **1** |

**Workspace feature unification defeats the flag.** `keleusma-cli` declares `keleusma` with
`features = ["shell", "signatures", "encryption", "self-host"]`, so a workspace build turns them back
on whatever is passed at the workspace level.

**So the hole is narrower and more specific than this record first stated.** Not *"the gate does not
lint the no-default configuration"* — that phrasing suggests a flag would close it. Their statement:

> **The gate's lint is workspace-scoped and its tests are package-scoped, and no flag on a
> workspace-scoped command can reach a package-scoped configuration.**

Closing it requires `-p keleusma` lint steps mirroring the test steps' scoping. **That is not done**,
and their reason is the one this line accepted from them an hour earlier, now pointing the other way:
package-scoped linting under default features already reports five warnings on unrelated items, so
adding the steps today would turn the gate red on a separate matter — **folding a behaviour change
into a hole-closing fix.** It gets its own increment.

Their counts, recorded with their configurations attached rather than quoted bare: **nine warnings
package-scoped under no-default, five under default, zero under `self-host`.**

## What a green gate here does and does not establish

**Does**: every step of the project's own gate passes on this branch, with the corpus, the added
witnesses, and the absorbed `Opaque` change all present — the pairing that the back-merge will make
permanent.

**Does not**: cover configurations the gate itself omits. The cell above is one. `--all-features` is
another, and it is not a supported configuration.
