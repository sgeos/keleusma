# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `5ad58ab`
- **Written**: 2026-08-09
- **Tree at write**: clean. Everything merged and pushed. **No unmerged work anywhere.**
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

**Validity check — run on resume.** Check **both**:

1. `git rev-parse --abbrev-ref HEAD` is `v0.2.3` (or a feature branch cut from it).
2. `git rev-parse HEAD~1` equals the **Parent commit** above.

The branch check is not redundant. `v0.3.0` exists for parallel native-codegen work and is rebased
onto `v0.2.3`, so it can satisfy the commit check while this document describes a different
workstream. **A handoff that validates on the wrong branch is worse than a stale one.** If you are on
`v0.3.0` or a branch cut from it, this is not yours — read
`docs/process/handoffs/v0.3.0.md` instead.

- **Both match → VALID.**
- **Commit mismatch → INVALID and STALE.** Report it and familiarize from the live channels.
- **Branch mismatch → NOT YOURS.**

## On resume — do these first

1. **Run the validity check.**
2. **Read `secret/notes/APPENDIX_B.md` before writing ANY tracked file**, commit message, or comment.
3. **Read the other session's mailbox**, which is the parallel-development protocol, not a courtesy:
   `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`
4. **Re-read the three channels** — [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md),
   [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest first), [`TASKLOG.md`](./TASKLOG.md) — and this
   branch's mailbox, [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md).
5. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).** Step 1a
   (probe before planning) has falsified a recorded claim in every increment of this arc, including a
   published specification and a plan document that said the current work was blocked.
6. **In-flight verification: NONE.**

## THE STATE: clean, green, nothing outstanding

`v0.2.3` = `5ad58ab`, in sync with origin, CI green. Working branch
`feat/selfhost-wire-crc32` is cut from it with **no commits of its own yet**.

`BYTECODE_VERSION` is **2**. The auxiliary body is the wire format v2 container, not an rkyv
archive, and [`../spec/WIRE_FORMAT.md`](../spec/WIRE_FORMAT.md) accurately describes it.

The six-step wire-format programme: **1 to 5 done and merged; 6 is the active work.**

## THE ACTIVE WORK: step 6 slice 1 — CRC-32 in Keleusma

Read [`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md) **from
the top**, where it is re-scoped into seven slices. Its 2026-08-03 body says the work is blocked;
that was true only while the auxiliary body was rkyv, and is superseded.

**Slice 1 is CRC-32**, oracle-checked against `crate::bytecode::crc32` (`src/bytecode.rs:3697`,
`CRC32_POLY = 0xEDB88320`, init `0xFFFFFFFF`, bitwise and table-free, final xor). It is small on
purpose: its job is to establish the byte-emission harness every later slice needs.

### What is already probed, so do not re-probe it

- **A `.kel` stage addresses a byte buffer as a `shared data` byte array.** `lexer.kel` already does
  this, taking source from `src.bytes`. That is shipping code, not the `secret/` prototype.
- The `secret/kel-format-probe/` prototype **predates format lock-in** — 12-byte directory entries
  where the shipped entry is 16, and no triplicated prologue. Feasibility evidence only.

### What to probe before writing, reported by the `v0.3.0` session and unverified by me

- **Locals are immutable.** `s = s + b` in a loop is rejected with "assignment is only supported for
  data block fields", so a running CRC accumulator must live in a data block. This makes the function
  stateful, so the harness has to reset between vectors.
- **The verifier admits less than the parser.** A data-dependent `break` compiles and then fails
  `verify_resource_bounds`. A CRC over a runtime-length buffer wants the `for … limit <const>` form.

Both match what is on file (no `let mut`; a `for` needs `limit`). **Confirm them against the
reference anyway** — that is the rule neither session gets an exemption from.

### A design point neither session has settled

Keleusma's `Word` is signed `i64`; CRC-32 needs `u32` semantics. The shift must be logical (`lsr`,
not `asr`) and the accumulator masked to 32 bits after each step. B19 supplies `lsr`, `bxor`, `band`.

### The test must carry BOTH directions

A differential against a known-good reference is exactly where a too-strict check hides. Required:

- a **must-fire case** — inputs that discriminate, and a demonstration that perturbing one input byte
  changes the answer;
- a **must-not-fire case** — the check stays quiet when it should.

An assertion that never fires is indistinguishable from one that always succeeds.

## Parallel development is ACTIVE — read this before touching anything shared

`v0.3.0` carries native code generation in a separate session and worktree, rebased onto `v0.2.3`.
The protocol is [`PARALLEL_DEVELOPMENT.md`](./PARALLEL_DEVELOPMENT.md) **section 0a**.

- **Mailboxes are the channel.** One per version branch, on that branch, read with `git show`. The
  operator is for decisions, not transport.
- **Poll at increment boundaries.** The mailbox has no wake. Read theirs before starting an increment
  and after finishing one.
- **Do not touch `v0.3.0` or anything under `keleusma-worktrees/`.** There is one shared `.git`, so
  their branches are visible here; visibility is not ownership.
- **`scripts/release-gate.sh` is the one genuinely shared file.** My edits sit above the first
  `step()` call; theirs at the end.
- **They read `src/wire_schema.rs` (`AuxView`, `AuxOffsets`) and `src/bytecode.rs`.** Announce changes
  to that surface in the mailbox before making them. `AuxResolved` in `src/vm.rs` is private and
  outside it. Step 6 is additive `.kel` work and does not disturb them.
- **Never run two full gates at once**, and reap orphans **path-scoped**:
  `pkill -f "$PWD/target/debug/deps"`. Unscoped kills a sibling session's live run.

## Verification

Full gate before every **merge**, not after every **change**. See
[`PROCESS_STRATEGY.md`](./PROCESS_STRATEGY.md#tiered-verification).

- **Tier 0**, per edit: `scripts/fast-check.sh 'test(<filter>)'`.
- **Tier 1**, per increment: `clippy --workspace --all-targets -D warnings`,
  `cargo test -p keleusma --no-default-features`, and the `-D warnings` doc build.
- **Tier 2**, per merge: `scripts/release-gate.sh`, **batching three or four increments**.

The gate is **~2h33m** and is repetition-bound, not contention-bound: `selfhost_codegen` is 1008 s
standalone at 4.85 of 10 cores and runs about four times across the feature matrix. Raising test
parallelism buys nothing. Run it in the background and monitor the log.

**CI is now a strict superset of the local gate**, which is what makes it safe as the authority — it
is a mechanism, not a procedure. Do not create a strong gate and a weak gate for a human to choose
between.

`tests/perf_canary.rs` is a tripwire, not a benchmark. If it fires: rule out concurrent load, re-run
alone, and profile before touching the ceiling.

## Method rules this arc paid for

- **A control executes a test's precondition** — that the predicate measures what you think it does.
  It runs in **one direction only**, so a **must-fire** and a **must-not-fire** case are both needed.
  Every control run on 2026-08-08 was must-fire, and all three were recorded as if they closed the
  question.
- **Executed beats reasoned**, and an executed claim is only as good as its **unexecuted
  preconditions**. A measured 3.7× speedup this arc came from a build with a live bug in it.
- **Rehearse a history rewrite on throwaway refs.** Two consecutive wrong rebase ranges were caught
  that way.
- **Prefer a pattern to an enumeration.** A by-name list has now produced three coverage holes.
- **A recorded status claim is a lead, not a fact** — including one in this document.

## Open, and held by the operator

- **Trimming the gate's feature matrix**, worth roughly 34 minutes. Measured, not guessed. Not done
  unilaterally because it weakens verification.
- **Publication is HELD.** Nothing is published. Irreversible and outward-facing — confirm first.
- **MSRV 1.85 declared, never verified.**
- **Fifteen `self.aux()` sites remain, audited, none hot** — a real follow-up, not a blocker.

**Boundary counts** — recount with a grep on `self_hosted_construct_support_boundary` in
`tests/selfhost_codegen.rs` rather than trusting a number here.

**Git**: `v0.2.3` = `5ad58ab` plus this handoff commit. Local branches include `main`, `v0.2.3`,
`v0.3.0` (**theirs**), `feat/selfhost-wire-crc32` (mine, empty), `v0.2.3-prerebase-backup`, and the
other session's `tmp/*` rehearsal refs. **Do NOT delete `v0.2.3-prerebase-backup`** or anything you
do not own.

**Guardrails**: no new opcode or `BYTECODE_VERSION` bump without authorization; full gate before any
merge; confirm before any irreversible or outward-facing action; never bypass the pre-push gate.
