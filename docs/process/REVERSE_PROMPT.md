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

- **Autonomy loop RUNNING. Increment 5 (struct-of-array-of-struct equality) is COMPLETE, full gate
  GREEN, merged to `v0.2.3`.** `a == b` where a struct field is an array-of-struct
  (`struct Q { ps: [P; 2] }`) now self-compiles byte-identically. **This closes the LAST
  nested-composite-equality gap.**
- **Construct-support boundary: 52 Ok / 2 Gap / 1 RefRejects** (was 51 / 3 / 1).
- No opcode, record/node kind, or `BYTECODE_VERSION` change (reuses op 48 GetFieldNested and
  `getindexnested` FlatNested Struct). `EXPECTED_SELF_COMPILE` 71 → 72 (a factored
  `push_arr_of_struct_inner`).

## Verification

- Byte-identity oracle green: `self_host_compiles_struct_arrayofstruct_equality` (`[P;2]`, `[P;3]`,
  multi-field element, `!=`, array-of-struct beside a scalar top field), all five whole-stage
  self-compiles, the full nested-eq blast-radius suite, `validate_module_via_kel`, the codegen count
  (72), and the boundary.
- **Full `scripts/release-gate.sh` GREEN** (feature matrix, docs `-D warnings`, the detached
  `compiler/` subproject). Merged only after this.

## Capacity note (important for future parse.kel growth)

Two host-side capacity walls were raised this increment (no ISA/wire impact): (1) parse.kel outgrew the
lexer `src.bytes` source buffer, so it was raised **245760 → 393216** across all lockstep offset
constants (lexer.kel, `compiler/src/main.rs`, `compiler/src/selfhost.rs`, `tests/selfhost_codegen.rs`,
`tests/selfhost_pipeline.rs`); (2) the larger buffer expands the lexer's per-element shared-slot layout,
so `verify_datalayout.kel`'s working arena in `dl_reject_module_via_kel` was raised to 4 MB. LESSON:
resizing a shared byte-array buffer expands the per-element data layout and can cascade into
layout-verifier arena limits — bump both together. parse.kel now has headroom to ~393 KB.

## Next step — OPERATOR-DECISION FORK: the same-context frontier is exhausted of bounded work

The nested-composite-equality family is fully self-hosted (tuple-of-struct, enum-in-struct,
enum-with-struct-payload, 2-level struct nesting, struct-of-array-of-struct — increments 1–5). The two
remaining Gaps are NOT bounded roadmap increments:

- **Third-level struct nesting** — `eq/2level_struct` handles exactly depth-2 via a fixed extra phase.
  Depth-3 needs a GENERAL depth stack in the drain (the total-language verifier forbids the recursion
  that would make arbitrary depth trivial). That is a design decision, not a copy-the-pattern increment
  — a genuine STOP.
- **Floats and generics** — out of scope for the self-hosted subset by policy.

So the loop has reached a real fork: continue by **switching workstreams** (the highest-leverage
residual is wiring the self-hosted stages into the shipping binary — Workstream A), or take on the
third-level-nesting depth-stack design, or pause. This is an operator call per the loop's stop
conditions (no remaining same-context candidate is a bounded roadmap task). The next session should
surface these options rather than autonomously starting a design-decision or workstream switch.

The seven-day rate-limit window remains the binding budget under heavy agent work; pace accordingly.
