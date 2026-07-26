# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-26 (session 34)

## Current state

- **Autonomy loop RUNNING. Increment 3 (enum-with-struct-payload equality) is COMPLETE, full gate
  GREEN, merged to `v0.2.3`.** The loop selected it without an operator prompt (context-switching-first
  policy). `a == b` where an enum variant carries a struct payload (`struct P { x: Word }`,
  `enum E { A(P), B }`) now self-compiles byte-identically.
- **Construct-support boundary: 50 Ok / 4 Gap / 1 RefRejects** (was 49 / 5 / 1). The enum-struct-payload
  gap is closed.
- No opcode, record/node kind, or `BYTECODE_VERSION` change (reuses op 57 variant tag 2 and the tracked
  `evfstruct` index). `EXPECTED_SELF_COMPILE` 69 → 70 (a factored `push_enum_struct_payload_loop`).

## Verification

- Byte-identity oracle green: `self_host_compiles_enum_struct_payload_equality` (single/multi-field
  payload, `==`/`!=`, struct-beside-scalar, `A(Word, P)`), all five whole-stage self-compiles, the full
  enum-equality blast-radius suite (12 tests), `validate_module_via_kel`, the codegen self-compile count
  (70), and `self_hosted_construct_support_boundary`.
- **Full `scripts/release-gate.sh` GREEN**: fmt, clippy `-D warnings`, the feature matrix, docs
  `-D warnings`, markdown links, and the detached `compiler/` subproject. Merged to `v0.2.3` only after
  this.

## Implementation notes (for the next increment)

- The interning stayed DEFERRED (`push_bool`) for the inner struct loop, NOT the eager pre-pass the
  scouted plan suggested. `push_enum_eq` is uniformly deferred (unlike the fully-eager
  `push_struct_eq_nested`), so deferred bools intern in forward-emission order and dedup into the
  reference pool with no pre-pass. This is the cleaner pattern when the base emitter is deferred.
- Extract temps r2/l2 are allocated in PARSE (monotonic `slot_count`, mirroring the reference's
  `next_slot`, which `end_scope` never rewinds) and streamed to codegen. The codegen `let_count` is
  bumped +2 per struct-payload field to keep the frame size in sync.

## Next step — DECISION POINT: the same-context nested-equality frontier is exhausted of bounded work

Continue the loop, but the next task selection reaches a decision point. The two remaining
nested-equality Gaps are NOT obviously bounded roadmap tasks:

- **2-level nesting** (`struct O { m: M }` where `M` has a composite field) — needs the streaming
  drain to RECURSE (a nested composite field inside a nested composite field). Rated extreme in the
  prior frontier assessment; it likely needs a genuine design decision about how to represent
  arbitrary-depth recursion in the flat record stream (a stack, not the current fixed sub-phase). This
  is a candidate STOP, not a mechanical increment.
- **struct-of-array-of-struct** (`struct Q { ps: [P; 2] }`) — an intentional `struct_eq_kind` defer.

Per the loop's stop conditions, when no remaining same-context candidate is a bounded roadmap task the
loop should either switch workstreams (for example wiring the self-hosted stages into the shipping
binary, Workstream A's highest-leverage residual) or surface a stop for operator direction. The next
session should FIRST re-scout 2-level nesting: if a bounded approach exists (no new record/node kind, a
tractable recursion), implement it; if it requires a new streaming-recursion design or an ISA change,
STOP and surface the options rather than forcing it. Do not begin a workstream switch without weighing
it against the operator's priorities.

The seven-day rate-limit window remains the binding budget under heavy agent work; pace accordingly.
