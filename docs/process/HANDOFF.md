# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale.

## Validity

- **Branch**: `v0.2.3`, or a branch cut from it.
- **Parent commit**: `ec90e8a`
- **Written**: 2026-08-09
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

Check **both**: `git rev-parse --abbrev-ref HEAD` is `v0.2.3` or a branch off it, and
`git rev-parse HEAD~1` equals the parent above. `v0.3.0` exists for parallel native-codegen work and
can satisfy the commit check while describing a different workstream, so the branch half is not
redundant. If you are on `v0.3.0`, read `docs/process/handoffs/v0.3.0.md` and **do not overwrite
this file** — that session has asked.

- **Both match → VALID.** **Commit mismatch → INVALID and STALE**; report it, orient from the live
  channels. **Branch mismatch → NOT YOURS.**

## FIRST

1. **Read `secret/notes/APPENDIX_B.md`** before writing any tracked file, commit message, or comment.
2. **Read the other session's mailbox**: `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.
   Protocol, not courtesy.
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md),
   [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).** Its
   probe-before-planning step has falsified a recorded claim in nearly every increment of this arc,
   including one of mine today that would have put a false statement into a normative spec.

## THE GIT STATE — the easiest thing to get wrong right now

**TWO BRANCHES STACK, AND ONLY THE FIRST HAS BEEN GATED.**

| Ref | Commits | State |
|---|---|---|
| `v0.2.3` | — | **2 unpushed** (mailbox, this handoff) |
| `feat/selfhost-wire-directory` | 7 over `v0.2.3` | tip `06dabf0`, **in the gate** when this was written |
| `feat/selfhost-wire-data` | 11 over that | slices 5d–7 and two audit fixes, Tier-1 green, **NEVER GATED** |

**Do NOT merge the second branch on the first branch's green.** It contains eleven increments the
gate never saw. The sequence is: merge `wire-directory`, then rebase `wire-data` onto `v0.2.3` and
**gate it separately** before merging.

Gate with `scripts/gate-in-worktree.sh <commit>`. It runs in a detached worktree so the main tree
stays free, refuses to start while another gate runs, and `--setup-only` verifies the setup without
a 2.5-hour run.

**Stopping a gate: PATH-SCOPED, always.** A bare `pkill -f "release-gate.sh"` killed the other
session's gate today and orphaned its test binary at 98% CPU. Both sessions made this identical
mistake within one hour, each after reading the warning. Use
`pkill -f "<gate dir>"` then `pkill -f "<gate target>/debug/deps"`; the second is not optional,
because killing the driver leaves the children reparented to PID 1.

## THE WORK: step 6 is COMPLETE

`src/selfhost/kel/wire.kel` plus `tests/selfhost_wire.rs`, **80 tests**. All seven slices: CRC-32;
container primitives, prologue and the majority-of-three vote; the region directory; record tables
and byte pools; the schema layer's twenty region kinds and seventeen record shapes; the opcode
record and operand pool; the framing header and CRC trailer.

**The next increment is WIRING, not invention.** `wire.kel` is deliberately absent from
`read_stage` and nothing drives it. Making the self-hosted path actually emit an artifact is the
remaining Order-1 work.

### Design facts that cost something to learn

- **Transcribe, then pin.** The derive packs with no implicit padding and rounds the stride to a
  word, so offsets cannot be recomputed by eye. Every constant is asserted against the derive's
  generated value **by parsing it back out of the Keleusma source**; restating it in the test would
  only prove the test agrees with itself.
- **The sentinel technique fails silently** where the value domain has no spare value. Three cases
  hit this. Split the bound from the value instead.
- **Two parity schemes**: an opcode record carries one BIT of popcount parity; a pool entry carries
  one BYTE of exclusive-or. Conflating them is the easy mistake.
- **The CRC trailer is validated by a residue**, `0x2144DF1C`, not by recomputation.
- **The parser rejects expressions nested deeper than 24**, so a flat `if/else if` dispatch caps at
  about two dozen arms. The dispatch is nine chains, with a test that no command falls through.

## Order-1: the roadmap now understates the progress

Two blockers were measured and shrank; the third is done.

- **Monomorphizer: EMPTY** for the first pass. Identity on all ten stage sources, pinned by
  `tests/selfhost_monomorphize_identity.rs` with a must-fire control.
- **Type checker: REJECTION ALONE.** Clearing `program.fn_expr_types` leaves every stage module
  byte-identical. Three controls; see
  [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md).
- **Wire-format serialization: DONE.**

**The roadmap's Order-1 gate row should be restated** — it implies three comparable blockers. Not
done yet because that file was inside the running gate; do it after the merge.

## Method rules this session paid for

- **Check `$?` explicitly; never read success off output.** A `| tail` hid a red gate. Appending
  `; echo` or `nohup … &` to a background command reports the wrapper's status. Three occurrences.
- **An implausibly fast pass is the signal.** A five-minute "green" on a 2.5-hour gate.
- **A probe needs its own control.** Six constructs looked language-rejected when the real cause was
  an arena with zero persistent capacity.
- **A set difference is not a finding until the scope is established.** Two false alarms in the
  documentation audit, both rejected by reading the document's stated scope first.
- **Hold an unverified claim out of a spec.** The `CheckedArithNoArm` finding was not merely
  unverified, it was false; execution refuted it.
- **Prefer a mechanism to a longer list.** The stage-directory corpus guard is the fourth instance
  of the by-name-enumeration family and the first closed mechanically.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **Trimming the gate's feature matrix.** Now argued against by evidence: the non-`--all-features`
  clippy caught lints in five separate increments today, and `--no-default-features` caught a stray
  `examples/` file. The matrix is finding defects at a steady rate.
- **MSRV 1.85 declared, never verified.**

**Guardrails**: no new opcode or `BYTECODE_VERSION` bump without authorization; full gate before any
merge; confirm before anything irreversible or outward-facing; never bypass the pre-push gate.
