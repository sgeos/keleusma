# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than
misleading a resuming agent.

## Validity

- **Branch**: `v0.2.3`, or a feature branch cut from it.
- **Parent commit**: `cdee459`
- **Written**: 2026-08-09
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

**Check both.** `git rev-parse --abbrev-ref HEAD` is `v0.2.3` or a branch off it, and
`git rev-parse HEAD~1` equals the parent above.

The branch half is not redundant: `v0.3.0` carries parallel native-codegen work and can satisfy the
commit check while describing a different workstream. If you are on `v0.3.0` or a branch off it,
read `docs/process/handoffs/v0.3.0.md` instead — **and do not overwrite this file**, which that
session has explicitly asked.

- **Both match → VALID.** **Commit mismatch → INVALID and STALE**; say so and orient from the live
  channels. **Branch mismatch → NOT YOURS.**

## On resume, before doing anything

1. **Read `secret/notes/APPENDIX_B.md`** before writing any tracked file, commit message, or comment.
2. **Read the other session's mailbox**: `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.
   Protocol, not courtesy.
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md),
   [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).** Its
   probe-before-planning step has falsified a recorded claim in nearly every increment of this arc,
   including two of mine on 2026-08-09 — one of which would have put a false statement into a
   normative specification.

## THE STATE: clean. Nothing is in flight.

`v0.2.3` = `cdee459`, **in sync with origin, tree clean, no gate running, no unmerged work.**
This is a genuinely quiet resume point; the previous two were not.

Local branches `feat/selfhost-wire-directory`, `docs/spec-currency` and `feat/selfhost-wire-crc32`
are fully merged and safe to delete. `feat/selfhost-wire-data` shows one commit ahead, which is the
pre-cherry-pick original of a docs change already on `v0.2.3`; also safe. **Do not delete
`v0.2.3-prerebase-backup` or anything under `keleusma-worktrees/`** — the latter includes the other
session's tree.

## WHAT WAS FINISHED: wire-format step 6, complete

The wire format is expressible in Keleusma **end to end**. `src/selfhost/kel/wire.kel` is the
implementation, `tests/selfhost_wire.rs` the differential, **80 tests**.

| Slice | Content |
|---|---|
| 1 | CRC-32/ISO-HDLC, oracle the published check value |
| 2 | Container primitives, prologue, majority-of-three vote |
| 3 | Region directory, with the prologue-to-directory bootstrap |
| 4 | Record tables and byte pools |
| 5a–5e | The schema layer: 20 region kinds, 17 record shapes |
| 6a–6b | Opcode records and the operand pool |
| 7 | Framing header and CRC trailer |

`wire.kel` is **deliberately absent from `read_stage`**. Nothing drives it; it can emit and read the
format, but no artifact is produced by the self-hosted path yet.

## THE NEXT INCREMENT: wiring, and its shape is already measured

Read the "wiring increment" section of
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md) **before
planning**. Probing reshaped it before any code existed, and the numbers are recorded there:

- **A whole-artifact-in-one-buffer emitter cannot work.** The shared ceiling is `MAX_DATA_ADDR`,
  16,777,216 bytes; `lexer.kel`'s artifact is 16,124,636 — 96.1% of it, leaving 652,580 bytes for
  the emitter's own inputs, which also live in shared data.
- **Emission must be staged**: compute region lengths, write the leading directory, emit region by
  region with the host appending. That matches the operator's chosen encoder strategy.
- **Size the working buffer from the largest single REGION**, 6,609,960 bytes (`lexer`'s
  `STRING_POOL`, 39.4% of the ceiling). About 8 MB covers every stage. `STRING_POOL` is the largest
  region for **all ten** stages, and being a byte pool it is also the easiest to chunk further.
- **`DEBUG_POOL` is the one region kind the corpus never emits**, because
  `CompileOptions::emit_debug` defaults to false. The reader side is covered; the **emitter** needs
  a hand-built case or a compile with `emit_debug` on.

Why the artifacts are so large, which is worth knowing before optimising anything: **every array
element becomes its own data slot with its own interned name.** `lexer.kel` declares a 393,216-byte
array and reports 395,784 data slots, so its auxiliary body is 99.94% of the artifact. Whether an
array should instead occupy one slot with a length is a **format and data-layout design question for
the operator**, with WCMU implications, and is outside a wiring increment.

## Order-1: integration, not invention

The roadmap's gate row was restated on 2026-08-09 and now says this; it previously implied three
comparable blockers.

- **Monomorphizer: EMPTY** for the first pass. Identity on all ten stage sources, pinned by
  `tests/selfhost_monomorphize_identity.rs` with a must-fire control.
- **Type checker: REJECTION ALONE.** Clearing `program.fn_expr_types` leaves every stage module
  byte-identical, so the emitter's structural fallback covers the subset. Three controls; see
  [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md).
- **Wire-format serialization: expressible end to end.**

## Design facts that cost real effort to learn

- **Transcribe, then pin.** `#[derive(WireRecord)]` packs with no implicit padding then rounds the
  stride to a word, so offsets cannot be recomputed by eye. Every constant in `wire.kel` is asserted
  against the derive's generated value **by parsing it back out of the Keleusma source**; restating
  it in the test would only prove the test agrees with itself.
- **The sentinel technique fails silently** where the value domain has no spare value. Three cases
  hit this — a discriminant of -1 is legal, `DATA_SLOTS` absence differs from emptiness, a debug
  pool absent differs from present-but-empty. Split the bound from the value.
- **Two parity schemes.** An opcode record carries one BIT of popcount parity; a pool entry carries
  one BYTE of exclusive-or. Conflating them is the easy mistake.
- **The CRC trailer is validated by a residue**, `0x2144DF1C`, not by recomputation.
- **The parser rejects expressions nested deeper than 24**, so a flat `if/else if` dispatch caps at
  about two dozen arms. `wire.kel`'s dispatch is nine chains, with a test that no command falls
  through to a chain default.
- **Language facts, all executed**: locals are immutable, rejected at parse; a runtime-range `for`
  needs `limit`, rejected at verify; `Byte as Word` zero-extends and `as Byte` truncates silently;
  `lsr` is logical over the full word; division by zero traps and `andalso` short-circuits.

## Gating

`scripts/gate-in-worktree.sh <commit>` runs the gate in a detached worktree pinned to that commit,
so the main tree stays free and the result is pinned by construction. `--setup-only` verifies the
setup without a 2.5-hour run. The script refuses to start while another gate runs, machine-wide.

**Two traps, both of which caught me on 2026-08-09:**

- **Gate the tip you intend to merge.** I committed after launching a gate and nearly merged a
  commit the gate never saw. Merge the *gated* commit; land anything later separately.
- **Stopping a gate is PATH-SCOPED, always.** A bare `pkill -f "release-gate.sh"` killed the other
  session's gate and orphaned its test binary at 98% CPU. Use `pkill -f "<gate dir>"` then
  `pkill -f "<gate target>/debug/deps"`; the second is not optional, because killing the driver
  leaves the children reparented to PID 1. **Both sessions made this identical mistake within one
  hour, each after reading the warning.**

**A gate no longer makes the main tree look busy.** Check `pgrep -f release-gate.sh` or the mailbox
banner, never the tree's cleanliness.

## Method rules this arc paid for

- **Check `$?` explicitly; never read success off output.** A `| tail` hid a red gate; appending
  `; echo` or `nohup … &` to a background command reports the wrapper's status. Three occurrences in
  one session.
- **An implausibly fast pass is the signal.** A five-minute "green" on a 2.5-hour gate was the only
  honest indication; the reported status was wrong.
- **A probe needs its own control.** Six constructs looked language-rejected when the cause was an
  arena with zero persistent capacity.
- **A set difference is not a finding until the scope is established.** Two false alarms in the
  documentation audit, both rejected by reading the document's stated scope first.
- **Hold an unverified claim out of a specification.** The `CheckedArithNoArm` finding was not
  merely unverified, it was false, and execution refuted it.
- **Prefer a mechanism to a longer list.** The stage-directory corpus guard is the fourth instance
  of the by-name-enumeration family and the first closed mechanically.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **Trimming the gate's feature matrix**, worth roughly 34 minutes. **Now argued against by
  evidence**: the non-`--all-features` clippy caught lints in five separate increments on
  2026-08-09, and `--no-default-features` caught a stray `examples/` file. The matrix is finding
  defects at a steady rate.
- **Per-element data slots.** One slot and one interned name per array element is why a 21 KB source
  produces a 16 MB artifact. A format and data-layout question, not a loop decision.
- **MSRV 1.85 declared, never verified.**

## Parallel development

`v0.3.0` carries native code generation in a separate session and worktree. Its gate went green;
mine has finished and **the machine is free**, which the mailbox says. Poll their mailbox at
increment boundaries — it has no wake.

**Guardrails**: no new opcode or `BYTECODE_VERSION` bump without authorization; full gate before any
merge; confirm before anything irreversible or outward-facing; never bypass the pre-push gate.
