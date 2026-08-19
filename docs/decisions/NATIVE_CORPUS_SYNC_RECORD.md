# Corpus Sync Record — the V0.3.X native line

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

A durable record of each absorption of the `v0.2.3` line's work into `v0.3.0`, the
predictions registered **before** the measuring run, and what the run actually
produced. Append newest-first.

## Why this file exists

Every number this line publishes is measured over a corpus it does not own.
`native_codegen`'s differential walks `src/selfhost/kel/` and `examples/scripts/`,
and both belong to the other release line. When that corpus moves, every measured
claim here is describing a tree that no longer exists.

**A number produced after the fact is a story rather than a measurement.** This
line has registered a prediction before a run twice, and both times it was the only
reason the outcome meant anything. Once the prediction is registered, a result that
contradicts it is evidence; without one, the same result is a narrative written to
fit whatever happened. This file is the register.

**It also records the failures.** A prediction file that only ever holds confirmed
predictions is a file that was written afterwards.

---

# Absorption 2 — the diagnostics and streaming-emit delta (2026-08-19)

## What is being absorbed

Forty two commits on `v0.2.3` that `v0.3.0` did not carry, fifteen of them merges.
Merge base `7f186fd9`. Measured from the merge base rather than by diffing the two
tips, because a raw two-tip diff mixes both directions and this line has been
misled by exactly that before.

Incoming changes, by file, from `git diff --name-only 7f186fd9 origin/v0.2.3`:

| surface | files |
|---|---|
| corpus sources | `parse.kel`, `verify_types.kel`, `wire.kel` |
| the parse driver | `src/selfhost/mod.rs`, `src/selfhost_host.rs` |
| their harnesses | five `tests/selfhost_*.rs` |
| their detached package | `compiler/src/main.rs` |
| their documents | four process channels plus two others |

**Nothing else.** In particular the delta does not touch `src/vm.rs`,
`src/bytecode.rs`, `src/verify.rs`, `.github/`, `Cargo.toml`, or `Cargo.lock`.

**That absence is the reason this is a clean experiment.** The virtual machine is
the oracle every differential here compares against. Because the oracle is
unchanged and only the corpus moved, any measurement that shifts has exactly one
candidate cause. A sync that moved both would leave every shift ambiguous.

## The measured baseline, taken before the merge

Run on `18319c88`, the published `v0.3.0` head.

| | |
|---|---|
| `native_codegen` suite | 216 passed, 0 failed, process exit 0 |
| executed and agreeing | 44 |
| agreed but vacuous | 1, being `verify_datalayout.kel` |
| exempt | 19 |

The exempt membership, recorded in full because a count alone cannot answer which
module moved:

`prelude.kel` twice (two distinct prelude files), `faulty.kel`, `11_signed.kel`,
`piano_roll_0` through `piano_roll_9`, `rogue_ai_boss.kel`, `rogue_ai_hunter.kel`,
`rogue_ai_tracker.kel`, `rogue_dungen.kel`, `wire.kel`.

## The predictions, registered before the merge

| | prediction | basis |
|---|---|---|
| P1 | the merge raises no conflict | the two file sets since the merge base are disjoint, checked path by path |
| P2 | `native_codegen` builds with no source change on this side | all ten accessors this harness calls are present on the incoming head with unchanged arities, checked by signature |
| P3 | the suite total stays 216 passed, 0 failed | nothing in the delta reaches `native_codegen/`, which the other line does not edit |
| P4 | the triple stays 44 / 1 / 19 | corpus composition is unchanged, twelve stage files on both sides, only three sources edited |
| P5 | `wire.kel` stays exempt | its exemption is this harness feeding a tick counter where the stage expects a command, and nothing in the delta touches that convention |
| P6 | the zero-negative-operand-depth assertion still holds | the operand model lives in `src/verify.rs`, which the delta does not touch |
| **P7** | **the parser-driver abort is NOT closed, and still affects four example sources** | see below |

### P7 is the prediction worth having, and it is an ANTI-prediction

This line reported that `parse_functions` aborts the process on four of eleven
example scripts, at a bare `unwrap()` reached when a record arrives with no
declaration open, and offered struct-before-function as an unconfirmed hypothesis.

The other line has since replaced six bare `unwrap()` calls with a single named
panic that identifies an unrecognised top-level declaration form, and **confirms by
measurement that a top-level `struct` is the cause**. That settles this line's
hypothesis in its favour.

**It does not close the report.** Reading `open_decl` on the incoming head shows
`unwrap_or_else(|| panic!(...))`. The abort is still an abort. What changed is the
message, from a Rust `Option::unwrap` failure that names nothing, to text naming
the six declaration forms the stage handles and the one it does not.

So the predicted outcome is **unchanged in count, changed in message only**. This
is precisely the shape the standing rule warns about, that a count moving in the
expected direction is not evidence the expected thing moved, run in reverse: a
count that does **not** move, beside a real improvement, is the case where "the
other line fixed it" is the tempting and wrong summary.

If P7 is wrong in either direction it is the most valuable result of this
absorption, and it is the one a careless reading gets backwards.

## Measured outcomes

*Pending. This section is filled from runs on the merged tree, and every prediction
above gets a row whether it held or not.*

---

# Absorption 1 and earlier

Recorded in `docs/process/handoffs/v0.3.0.md` rather than here, this file having
been created at absorption 2. The prior syncs were `sync/v023-into-v030`,
`sync/v023-into-v030-2`, `sync/v023-operand-model`, and `sync/v023-coroutine-entry`.
The last of those is the one whose registered prediction held in two thirds and
failed in the third, costing this line `wire.kel`, and it is the precedent for
registering these in advance.
