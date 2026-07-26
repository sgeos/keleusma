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

- **Autonomy loop RUNNING. Increment 4 (2-level struct-nesting equality) is COMPLETE, full gate GREEN,
  merged to `v0.2.3`.** `a == b` for `struct O { m: M }`, `struct M { i: I }`, `struct I { v: Word }`
  (two levels of struct nesting) now self-compiles byte-identically.
- **Construct-support boundary: 51 Ok / 3 Gap / 1 RefRejects** (was 50 / 4 / 1). The 2-level struct gap
  is closed.
- No opcode, record/node kind, or `BYTECODE_VERSION` change (reuses op 48 GetFieldNested, records
  55/57/58, node 59, and the increment-3 sentinel-kind + packing-record streaming convention one level
  deeper). `EXPECTED_SELF_COMPILE` 70 → 71 (a factored `push_struct_eq_subfields`).

## Verification

- Byte-identity oracle green: `self_host_compiles_2level_struct_equality` (==/!=, 2-level beside a
  scalar top field, multi-field deepest struct, middle struct with an extra scalar field), all five
  whole-stage self-compiles, the full nested-eq blast-radius suite, `validate_module_via_kel`, the
  codegen self-compile count (71), and `self_hosted_construct_support_boundary`.
- **Full `scripts/release-gate.sh` GREEN**: fmt, clippy `-D warnings`, the feature matrix, docs
  `-D warnings`, markdown links, and the detached `compiler/` subproject. Merged to `v0.2.3` only after
  this.

## Implementation notes (for the next increment)

- The load-bearing constraint on every DEPTH increase is the verifier's **no-recursion rule** (R4): a
  `.kel` stage function may not self-recurse, so each extra nesting level is an explicit extra
  phase/stack, not a copy-recurse. This is why the journal rated 2-level "extreme"; the ISA is
  untouched. Increment 4 used a FIXED depth-2 extension (a `se_l2phase` in parse, `se_nsub_mode` in
  reconstruct, `push_struct_eq_subfields` in codegen); a third struct level currently defers.
- Byte-identity hinges on the slot-order: temps allocate depth-first, r2 before l2, +2 per level,
  matching the reference's monotonic `next_slot` (never rewound by `end_scope`).
- Interning stayed EAGER (push_struct_eq_nested is fully eager) and needed no change — pure struct
  nesting adds no new constant values, so deeper false/true dedup into the existing bool indices.

## Next step — struct-of-array-of-struct: SCOUTED TO A COMPLETE BLUEPRINT, ready to implement

The remaining nested-equality gap `eq/struct_arrayofstruct__GAP` (`struct Q { ps: [P; 2] }`, a struct
field that is an array-of-struct) is confirmed **BOUNDED** (no new opcode/record/node kind; the
element-struct index is already tracked via `sd_farraylen` + `sd_fstruct`; `push_array_of_struct_eq` is
the exact per-element codegen template), and the full edit-level plan is persisted at
[`docs/decisions/STRUCT_ARRAYOFSTRUCT_PLAN.md`](../decisions/STRUCT_ARRAYOFSTRUCT_PLAN.md) — a four-stage
blueprint (parse admit + array-of-struct sub-drain, reconstruct variant-1 packing, codegen inline
per-element loop, test + boundary flip to 52 Ok / 2 Gap). Implement directly from that document. Key
sharp edge: emit the per-element loop INLINE (not factored) so the eager interning keeps the
element-index constants before false/true.

**Why this is a checkpoint, not a stop.** The increment is bounded and blueprinted, but this session has
been heavy on the seven-day rate-limit budget (increments 3 and 4 merged, two full release gates, and
two large implementation forks totalling roughly 900k tokens — the second fork deliberately stopped at a
blueprint rather than risk a byte-identity commit from a saturated context). This is a clean,
fully-merged boundary with a VALID handoff, so the loop pauses here to bank budget and resumes with
`STRUCT_ARRAYOFSTRUCT_PLAN.md` next.

After struct-of-array-of-struct, the same-context frontier is the deferred tail: a third struct level
(needs a general depth stack rather than a fixed extra phase — likely a design-decision stop) and
floats/generics (out of scope for the subset). At that point re-weigh a workstream switch (for example
wiring the self-hosted stages into the shipping binary) and surface the choice.

The seven-day rate-limit window remains the binding budget under heavy agent work; pace accordingly.
