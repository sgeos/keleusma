# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-22 (session 28)

## Current state

- **`v0.2.3` (release line) at `a8ee1bb`.** Carries the fourteen-increment self-hosted
  language-surface phase (85th–98th: enum equality, the complete shift and bitwise
  operator families, array-of-{struct,enum,tuple,array} equality, eager `and`/`or`),
  the construct-support boundary test, the doc-currency corrections, and the
  `compiler/` subproject decoder fix plus the mandatory pre-merge gate.
- **`feat-selfhost-nested-eq` at `a8ee1bb`.** The tuple-of-struct feature branch; step 1
  (the `tup_estruct` prerequisite) is committed, the rest pending (recipe below).
- **`chore/process-audit-2026-07-22`.** The 2026-07-22 process-audit worklist work.
- **`main` at `7494435`** is diverged and ~268 commits behind `v0.2.3` (operator
  decision pending; see below).

## Verification

- Full pre-merge gate (`scripts/release-gate.sh`, now covering the detached `compiler/`
  subproject) is **GREEN** at `a8ee1bb`.
- The self-hosted subset boundary is pinned and CI-checked by
  `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`: **45 Ok,
  9 Gap, 1 RefRejects**. The Gaps are the nested-machinery frontier (enum-in-struct,
  tuple-of-struct, 2+-level, struct-of-array-of-struct, enum-struct-payload), floats,
  generics, and two precedence-faithfulness defects (`a xor b == c`, `a and b xor c`).

## In flight: tuple-of-struct equality (`feat-selfhost-nested-eq`)

The most contained nested-equality gap. Reference lowering is exactly
`push_struct_eq_nested` with the top-level accessor swapped `GetField` → `GetTupleField`.
Step 1 done (`tup_estruct` tracks a struct tuple-element's declaration index in
`parse.kel`; a near-no-op). Remaining, in order, each guarded by the byte-identical
nested-struct self-compiles (82nd–84th):

1. A `tuple_eq_kind` detector (composite tuple element via `tup_estruct > 0`) + `emit_op`
   routing to a tuple-container nested path.
2. Thread an `is_tuple_container` flag through `struct_eq_nested_start` /
   `structeq_nested_next` (se_phase 0 reads the container field from
   `tup_eoffset`/`tup_ekind`/`tup_estruct` instead of `sd_*`; se_phase 1 is unchanged
   since the nested struct P is always `sd_*`). Carry the flag on the StructEqNestedBuild
   record → node.
3. Codegen `push_struct_eq_nested` swaps only the top-level accessor on the flag
   (`GetField` op 47 → `GetTupleField` op 53; nested extract `getfieldnested` op 48 →
   `gettuplefield` op 53 with a nested operand form — the wire-op space 1..63 is full, so
   REUSE op 53 with a nested operand distinguished in the driver `decode_op` by operand
   magnitude, per the note in DESIGN_JOURNAL).
4. KIND-NUMBERING TRAP: a top-level tuple element uses `tup_ekind` (`scalar_kind_of`,
   Word=3); the inner struct field uses `sd_fkind` (Word=0). Do not cross them.

The next nested targets after tuple-of-struct: enum-in-struct (needs a variant-dispatch
nested-field variant + `sd_fenum` tracking) and 2+-level nesting (needs recursion).

## Process-audit worklist (`tmp/process-audit-worklist-2026-07-22.md`)

- **Item 4 (subproject gate blind spot): DONE.** Found and fixed a long-standing red
  subproject test (`unknown op tag 62`, a stale decoder) that had ridden into `v0.2.3`
  undetected; ported the missing decode arms, added a fast `decoder_drift_guard`, and
  put the `compiler/` subproject into `release-gate.sh` as the mandatory pre-merge gate.
- **Item 5 (channel discipline + currency): DONE.** Part 1: CLAUDE.md stage count /
  `BYTECODE_VERSION` 1→2 / branch framing, and TASKLOG Current Phase refreshed. Part 2:
  this REVERSE_PROMPT split into a bounded handoff plus the append-only DESIGN_JOURNAL.
- **Item 1 (nextest contention cap): PENDING — measure first.** `.config/nextest.toml`
  already caps `test-threads = 4`; the refinement is a `[test-groups]` cap on the heavy
  self-host tests. The `>960s` observation may be partly a nextest `SLOW`-warning
  artifact; time isolated vs. gated before changing anything.
- **Items 2–3 (fast inner-loop lane): PENDING.** `scripts/fast-check.sh` (fmt + clippy on
  the touched crate + the specific construct test); re-self-compile only the changed
  stage. Skip the memoization (cache-correctness risk to the oracle).
- **Item 6 (encoding-space exhaustion): PENDING — operator decision.** Four namespaces are
  full (token, record/node-kind, wire-op) and the precedence scale is coarse. Prepare a
  design brief on widening/restructuring and stop for the operator; do not land more
  workarounds.
- **Item 7 (autonomy/parallelism): operator call, blocked on item 6. Not started.**

## Concerns / operator decisions pending

- **Branching model:** `GIT_STRATEGY.md` says work merges into `main`, but practice
  merges into `v0.2.3`; `main` is diverged and ~268 commits behind, and CI triggers on
  `main` only — so the `v0.2.x` line is not CI-gated, which is why the pre-merge gate is
  load-bearing. Reconciling this (catch `main` up, or point CI at the release line) is an
  operator decision (flagged in `GIT_STRATEGY.md`).
- **The subproject red test predated this session** (the decoder fell behind in the
  B19/B28 era) and shipped to `v0.2.3` and likely earlier tags because the subproject was
  gated nowhere. Now fixed and gated.

## Next step

Continue the process-audit worklist in order: item 1 (measure nextest contention, then
the test-group cap), items 2–3 (fast-check lane), and prepare the item 6 encoding-space
design brief for the operator. Then resume tuple-of-struct on `feat-selfhost-nested-eq`.
