# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `9e00e82`
- **Written**: 2026-08-06
- **Tree at write**: clean, in sync with origin, no feature branch open.
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** It defines what must not
  appear in this repository. **Hard constraint, not a preference.**

**Validity check — run on resume, before trusting this handoff.** Compare the **Parent commit** above
to `git rev-parse HEAD~1`.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** Report the mismatch, familiarize from `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, and wait for instruction.

## On resume — do these first

1. **Run the validity check above.** Mismatch → stop and report.
2. **Read `secret/notes/APPENDIX_B.md` before writing ANY tracked file**, commit message, or code
   comment for this work.
3. **Re-read the three channels fresh** — [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md),
   [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).** Its step 1a
   (**probe before planning**) falsified something in **every increment** of this arc — including
   claims written one increment earlier by the same agent, and limitations in already-merged code.
5. **In-flight verification: NONE.**

## THE LOOP IS STOPPED ON AN OPERATOR DECISION

**The next increment requires `BYTECODE_VERSION` to change from 1 to 2. Do not proceed without an
explicit decision.**

What is being asked:

- The cutover replaces the `Archived*` read surface in `vm.rs`/`bytecode.rs` with `AuxView`, deletes
  the rkyv aux path, and drops the dependency. The aux encoding changes completely.
- **Consequence of staying at 1**: a version-1 artifact read by the new runtime is **accepted and
  mis-read** rather than cleanly rejected. That is the hazard `CLAUDE.md` already documents and
  accepts under the no-public-adoption policy.
- **The precedent runs both ways.** A bump was authorised for this work on 2026-08-03 and then rolled
  back to 1; a version-2 bump was likewise rolled back during V0.2.0. The argument for staying at 1 is
  that a version number is a compatibility commitment to consumers, and there are none.

Everything up to this point is complete, merged, gated, and validated against real compiler output.

## What is DONE (steps 1, 2, 4, and step 5 increment 1)

- **`keleusma-wire` + `keleusma-wire-derive`** — schema-free container: triplicated prologue and
  directory, fixed-stride records, pools, CRC-32, (72,64) SECDED parity plane, `#[derive(WireRecord)]`.
  Both `#![forbid(unsafe_code)]`, reader allocation-free, builds for `wasm32v1-none`.
- **`src/wire_schema.rs`** — Keleusma's schema on that container. Every field of `WireAuxBody` and
  `WireChunk` encodes. `SchemaBuilder` owns the shared state (name interner, shape table, constant
  table); `encode_aux_body`/`decode_aux_body` round-trip the whole thing.
- **`AuxView`** — the runtime's single-parse read surface, chunk-relative indices.
- **Validation**: 85 schema tests, a corpus differential over all ten self-hosted stages (287 chunks,
  2192 constants), and randomised input testing.

## The narrow requirement that governs the cutover (probed, not assumed)

Exactly **one** accessor must return image-aliasing bytes: `AuxView::chunk_const_str_bytes`, for a
**non-empty top-level** `StaticStr`. An empty string is deliberately not aliased (so the runtime need
not rest on a non-null guarantee for a zero-length pointer), and a composite's string leaves are
**already copied today**. Do not over-constrain the accessor into a borrow-everything design, and do
not lose that one property. It is asserted **by address**, with a control proving the predicate
rejects a copy.

## Also outstanding, neither claimed nor done

- **Publication is HELD.** Neither crate is published. The standing decision is to publish only after a
  real consumer exercises the API — the cutover is that consumer. **Publishing is irreversible and
  outward-facing: confirm first.**
- **MSRV 1.85 is declared but never verified.** No build against that toolchain has been run.
- **The corpus emits zero struct templates**, so that table is covered only by hand-built cases. The
  corpus test asserts the zero so the caveat cannot drift.

## Method rules this arc actually validated

- **Probe before planning.** It falsified a claim in every increment, including ones written by me a
  single increment earlier.
- **Ask what a test would still pass with.** Three tests succeeded emptily before being caught: the
  `ConstValue::PartialEq` blindness (it ignores the enum discriminant), ECC counts agreeing rather than
  values, and a fuzz suite where 0/2000 inputs reached the readers.
- **Run `cargo clippy --all-features`, the `-D warnings` doc build, and `cargo test
  --no-default-features` BEFORE the gate.** Both genuine gate reds this arc were in those two blind
  spots, which targeted tests structurally cannot see.
- **Measure, do not theorise.** Three wrong guesses about a performance cliff cost ~30 minutes of
  timeouts; per-stage instrumentation found it in one run. Wall-clock (`etime`, `date`) includes
  laptop suspend and is not a work measurement.
- **Do not touch the tree while a gate runs.** Four gates were lost to this.

**Boundary counts** — **79 Ok / 4 Gap / 1 RefRejects**, unchanged (no `.kel` stage work this arc).
Recount with a grep rather than trusting the number.

**Git position**: `v0.2.3` = `9e00e82` plus this handoff commit, in sync with origin, tree clean.
Local branches: `main`, `v0.2.3`, `v0.2.3-prerebase-backup`. **Do NOT delete `v0.2.3-prerebase-backup`.**

**Guardrails**: no new opcode and no `BYTECODE_VERSION` bump without operator authorization; run the
FULL gate before claiming complete; confirm before any irreversible or outward-facing action; never
bypass the pre-push gate.
