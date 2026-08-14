# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-13 (session 43)

## Where things stand

| | |
|---|---|
| `v0.2.3` | `33103cab`, pushed, tree clean, in sync with origin |
| PRs merged this session | **#54**, 22 of 22 green, merged at the commit CI ran |
| Open PRs of this line | **none** |
| Boundary counts | **79 Ok / 4 Gap / 1 RefRejects, 84 cases** — recounted, matches |
| `selfhost_wire` | **151 tests**, up two |

## The ranked item was already done, and reading first is what found that

The handoff ranked "a second stage through the whole-artifact capstone under the new encoding"
first. **It had already landed** in `45a8870f`, inside the run-length-encoding pull request, which
updated the corpus to three stages and lowered the size-span control from 4x to 2x in the same
change. Confirming that cost two commands. Starting the work would have cost an increment and
produced a diff that reverted nothing and added nothing.

What was actually open is the thing the test says about itself.

## A corpus that cannot erode

**The capstone's qualifying corpus has shrunk three times, never from attrition.** Every encoding
improvement takes another real stage under the 65,536-byte window, and a stage whose body fits one
window exercises nothing about composition. Six became four, then three.

**A test whose corpus is destroyed by its own project's success will be weakened to keep it green**,
and the pressure arrives while landing an improvement, which is exactly when lowering a threshold
looks reasonable. The fourth case is synthetic and **sized against the encoder's measured output**,
so an encoding win makes it emit more functions rather than pushing it under the window. It sits
beside the real stages, is excluded from the size-span figures, and **the 2x threshold is
unchanged**.

Measured: 384 functions, 143,320 bytes, 2.19x the window, eleven regions, five batched.

## Two guards that would have shipped unexercised

**Every assertion in the assembler other than the byte comparison is a count** — regions placed,
batches run, calls returning success. A batch written to the wrong offset changes none of them, so
without a planted defect the capstone's passing is consistent with an assembler that places bytes
anywhere. The defect is planted through the real assembler rather than a copy.

**The growth loop never runs today.** The first attempt already clears twice the window, so the one
mechanism the increment exists to install would first execute on the day a future encoding win made
it necessary. A separate case asks for a target the first attempt cannot meet.

## Three corrections to my own work, caught before merge

**A control that fires is not yet a control that fired for the right reason.** The must-fire case
passed the moment it was written, by catching a panic. But the assembler's own guard, which reports
that the sabotage could not be planted, panics too and arrives as the same `Err`. Read naively it
would report the detector working at the moment nothing had been broken. It now asserts which panic
fired.

**A bound on a loop is not a bound on the damage.** The growth cap was first twelve doublings, which
terminates and is useless: doubling makes the last attempt the expensive one, so attempt twelve
compiles 786,432 functions and a broken assumption becomes an hours-long hang rather than a legible
failure. Six allows a 32x collapse in bytes per function and keeps the worst source near three
megabytes.

**`std::panic::set_hook` is global to the process.** The must-fire case used it to silence its own
expected panic. `cargo test` runs a binary's tests as threads in one process, so any other test that
panicked in that window would have its message swallowed while still being recorded as failed. That
trades a failing test's evidence for tidier output on a passing one. **nextest would never have shown
this**, because it gives each test its own process, and CI's `Test` job runs nextest; the hazard is
live under `cargo test`, which `scripts/release-gate.sh` runs.

## Concerns raised, not acted on

- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.** Unchanged and still yours.
- **`CHANGELOG.md:340` states the checked-arithmetic push order wrongly** and describes a published
  release. `TASKLOG.md:320,331` likewise.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **`v0.2.3-prerebase-backup`**, local only, a deliberate pre-rebase safety copy.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Next intended step

1. **The Order-1 type checker**, scoped in
   [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md) at about
   fifteen rejection shapes. **The oracle is verdict agreement, not message agreement.**
2. **Load-time ECC policy**, now only "should a host scrub, and when". The order is fixed and
   recorded; only scheduling is open.

## Parallel development

`v0.3.0` carries native code generation. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries. **Tell the two lines
apart by BASE BRANCH, not by author.** Their prune notes are answered; nothing of theirs is
outstanding on my side.

## Method rules this session paid for

- **Read the test and the history before starting the increment they describe.** The ranked item was
  already merged.
- **Assert WHICH failure fired**, not merely that one did. Two different panics meant opposite
  things.
- **A bound on a loop is not a bound on the damage.** Doubling puts the cost in the last attempt.
- **A global hook in a test is a hazard to every other test in the process**, and the runner that
  hides it is not the runner that gates the release.
