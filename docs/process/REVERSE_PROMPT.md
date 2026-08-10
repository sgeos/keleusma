# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

Prior sessions' blocks were removed here rather than allowed to accrete, which is what this
file's spec asks for and why it once reached 362 KB. Nothing was discarded: the reasoning is
in the design journal, and the two items it did not already hold were carried into its newest
entry before this overwrite.

---

## Last Updated

**Date**: 2026-08-09 (session 40, continued)

## STEP 6 IS COMPLETE — the wire format is expressible in Keleusma end to end

All seven slices. `src/selfhost/kel/wire.kel` plus `tests/selfhost_wire.rs`, **80 tests**.

| Slice | Content |
|---|---|
| 1 | CRC-32/ISO-HDLC, oracle the published check value |
| 2 | Container primitives, prologue, majority-of-three vote |
| 3 | Region directory, with the prologue-to-directory bootstrap |
| 4 | Record tables and byte pools |
| 5a–5e | The schema layer: 20 region kinds, 17 record shapes |
| 6a–6b | Opcode records and the operand pool |
| 7 | Framing header and CRC trailer |

**What remains is wiring, not invention.** `wire.kel` is deliberately absent from `read_stage`
and is not yet driven by the pipeline; it emits and reads the format, but nothing calls it to
produce a real artifact yet. That is the next increment.

## Order-1 has moved a long way, and the roadmap now understates it

The roadmap names three blockers. Two were measured and shrank; the third is now done.

- **Monomorphizer: EMPTY for the first pass.** Identity on all ten stage sources, pinned by
  `tests/selfhost_monomorphize_identity.rs` with a must-fire control.
- **Type checker: reduces to REJECTION alone.** Clearing `program.fn_expr_types` leaves every
  stage module byte-identical, so the structural fallback covers the subset. Three controls, in
  [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md).
- **Wire-format serialization: step 6 complete.**

**The roadmap's Order-1 row should be restated once this merges.** It currently implies three
comparable blockers; the accurate statement is that the remaining work is integration.

## Git state — READ THIS BEFORE RESUMING

Two branches stack, and one gate is in flight.

| Ref | Commits | State |
|---|---|---|
| `v0.2.3` | — | 1 unpushed (the gate banner) |
| `feat/selfhost-wire-directory` | 10 over `v0.2.3` | **IN THE GATE**, tip `06dabf0` |
| `feat/selfhost-wire-data` | 8 over that | slices 5d–7, Tier-1 green, **not gated** |

The gate is `scripts/gate-in-worktree.sh 06dabf0`. When it reports green: merge that branch,
then rebase `feat/selfhost-wire-data` onto `v0.2.3` and gate it before merging. **Do not merge
the second branch on the first branch's gate** — it contains eight increments the gate never saw.

## Method rules this stretch paid for

- **Transcribe, then pin.** Where a value must be copied from another language, assert it
  against the generating source rather than restating it in the test.
- **The sentinel technique fails silently** when the value domain has no spare value. Split the
  bound from the value instead: three separate cases hit this.
- **Check `$?`; never read success off the output.** A `| tail` hid a red gate earlier today.
- **An implausibly fast pass is the signal.** A five-minute "green" on a 2.5-hour gate.
- **A set difference is not a finding until the scope is established.** Two false alarms in the
  documentation audit, both rejected by reading the document's stated scope first.

## Open, and held by the operator

- **Publication remains HELD.** Nothing is published.
- **Trimming the gate's feature matrix.** Now argued against by evidence: the
  non-`--all-features` clippy caught lints in five separate increments today, and
  `--no-default-features` caught a stray `examples/` file. The matrix is finding defects at a
  steady rate.
- **MSRV 1.85 declared, never verified.**
- **Two audit findings are recorded but NOT applied**, both in the session scratchpad: naming
  `CheckedArithNoArm` in `RUNTIME_FAULTS.md`, which needs an execution check first; and the
  stage-directory corpus guard, where `tests/wire_corpus.rs` enumerates ten `.kel` files by name
  while the directory holds twelve and nothing reads the directory.

## Parallel development

`v0.3.0` carries native code generation. Their gate went green; mine is running. The mailbox is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md), theirs is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`. **A gate no longer makes the main tree
look busy** — it runs in a detached worktree, so check `pgrep -f release-gate.sh` or the banner.
