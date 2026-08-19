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

Every run below is on the merged tree. Each was captured whole and its process exit
status read outside any pipe, because this line has five recorded instances of a
constructed status and four of them came from a pipe.

### All seven predictions HELD

| | outcome | evidence |
|---|---|---|
| P1 | **held** | merge exited 0 with no conflict; `git grep` for merge markers exits 1 over 0 files |
| P2 | **held** | `native_codegen` builds unchanged; `git diff origin/v0.3.0 HEAD -- native_codegen/` is 0 files |
| P3 | **held** | 216 passed, 0 failed, 39 test binaries, process exit 0, 0 compiler diagnostics |
| P4 | **held** | 44 / 1 / 19, and the exempt membership is identical entry for entry |
| P5 | **held** | `wire.kel` exempt with the byte-identical reason `IndexOutOfBounds(1570808, 65536)` |
| P6 | **held** | 0 chunks reach negative operand depth, now over 1027 chunks where it was 971 |
| P7 | **held in both halves** | see below |

**Four independent signals were required to agree before P3 was called held**, rather
than one summary line. The process exit status, the summed `test result` totals, the
count of `FAILED` result lines, and the number of test binaries actually run. The
other line was nearly fooled by a green-looking summary while eighteen binaries never
executed, and the tell was the shape of the run rather than any status it printed.

### P7, the anti-prediction, in detail

**Unchanged in count. Changed in message only. NOT closed.**

The survey reports `parse_functions PANICKED on: 02_struct_field.kel,
08_method_dispatch.kel, 09_big_numbers.kel, 10_multbyte.kel` — four sources, and the
same four this line reported.

Both halves are measured rather than inferred:

- **Still an abort.** One `cur.as_mut()` site remains in the driver and it is
  `unwrap_or_else(|| panic!(...))`. The six bare `unwrap()` calls are gone; the
  process still dies.
- **The message genuinely changed.** The other line's own test
  `an_unrecognised_declaration_is_named_rather_than_unwrapped` requires the refusal to
  name both the missing declaration and `struct`, and to *not* contain
  `Option::unwrap`. It passes on this merged tree, run directly.

**This line's hypothesis is now confirmed by their measurement.** The reported cause
was offered here as an unconfirmed guess that the four sources declare a composite
before any function; a top-level `struct` is the measured cause and `parse.kel` has no
struct handling at all.

**The tempting summary is that the other line fixed this, and it is wrong.** Whether a
top-level `struct` should be supported or explicitly refused is an open question the
other line records as undecided, and a public API under `self-host` still aborts the
process on ordinary shipped source. **The report stands.**

### A new interaction this merge created, not predicted because it was not foreseen

The other line's guard `no_other_file_restates_the_shared_layout` walks the whole
repository from the crate manifest directory, skipping only `target` and
dot-directories. **After this merge it reaches `native_codegen/` for the first time**,
43 source files that were never in the tree it walks before.

It passes. **A pass here would look identical if the walk did not reach this package
at all**, so the pass was made a real result by planting a slot-layout copy in
`native_codegen/tests/probe_unsupported.rs` and re-running. The guard failed and named
that file and line. The plant was then removed and the tree verified clean.

**Nothing under `native_codegen/` restates a shared-slot layout today**, and that is
now a measured fact rather than a structural argument. A future harness here that
copies one would break the other line's test rather than this line's.

### Claims re-established over the grown corpus

| | before | after |
|---|---|---|
| modules compiled | 64 | **64** |
| chunks walked, nesting instrument | 985 | **1032** |
| chunks walked, peak-model instrument | 971 | **1027** |
| deepest nesting observed | 19, `parse.kel::body_step` | **19, `parse.kel::body_step`** |
| loops carrying a break | 386 | **390** |
| loops whose breaks disagree | 0 | **0** |
| chunks reaching negative depth | 0 | **0** |

**`parse.kel` grew by 477 lines and the deepest nesting did not move.** It is still 19
and still in `body_step`. The warning that accompanied that figure is unchanged and is
repeated here because it is the load-bearing part: **19 is what the corpus contains,
not a bound on what the language admits**, and it must never be offered as one.

The break-depth zero is guarded by a must-fire control, which fired in this run,
reporting one disagreeing synthetic loop with depths 1 and 3.

## Examined and left UNRESOLVED

- **The two instruments disagree on how many chunks the corpus has.** The nesting walk
  reports 1032 and the peak-model walk 1027. **This is pre-existing**, not introduced
  here — the same pair read 985 and 971 before the merge. **What is unknown is which
  chunks each population includes and why the gap narrowed from 14 to 5.** Neither
  figure is corrected in favour of the other, because nothing measured here says which
  is right. This line already has a recorded instance of a second walker inventing its
  own handling and reporting a confidently wrong number, so the resolution is to read
  both walkers rather than to write a third.
- **`wire.kel` remains exempt.** Nothing in this absorption touched the per-module
  resume convention whose absence causes it, and that trade is still deliberately
  unmade.
- **The mutation census PART C denominators still cite the 54-module corpus.** This
  absorption did not change the module count, which stays 64, so their staleness is
  neither worsened nor repaired here.
- **`verify_datalayout.kel` is still the one vacuous module**, blocked by design.

---

# Absorption 1 and earlier

Recorded in `docs/process/handoffs/v0.3.0.md` rather than here, this file having
been created at absorption 2. The prior syncs were `sync/v023-into-v030`,
`sync/v023-into-v030-2`, `sync/v023-operand-model`, and `sync/v023-coroutine-entry`.
The last of those is the one whose registered prediction held in two thirds and
failed in the third, costing this line `wire.kel`, and it is the precedent for
registering these in advance.
