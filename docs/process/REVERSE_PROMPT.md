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

## Next step — CONTINUE: struct-of-array-of-struct (bounded, same context)

The remaining nested-equality gap `eq/struct_arrayofstruct__GAP` (`struct Q { ps: [P; 2] }`, a struct
field that is an array-of-struct) is the next same-context candidate. The frontier scout judged it
BOUNDED: it reuses the depth scaffolding this increment established PLUS an array-element-is-composite
sub-drain (the current array sub-drain, `se_subisarray`, only handles scalar elements). It composes the
per-element unroll of `push_array_of_struct_eq` underneath a struct-field extraction. No ISA change is
expected. The next session should scout the exact reference lowering (the `emit_composite_fieldwise_eq`
recursion for an array-of-struct field), confirm no new record/node kind, then implement it the same
way (parse sub-drain admit + stream, reconstruct seb layout, codegen inner per-element loop). If it
turns out to need a new record/node kind or a general stack, STOP and surface.

After that, the same-context frontier is the deferred tail: a third struct level (needs a general depth
stack, likely a design-decision stop) and floats/generics (out of scope for the subset). At that point
re-weigh a workstream switch (for example wiring the self-hosted stages into the shipping binary).

The seven-day rate-limit window remains the binding budget under heavy agent work; pace accordingly.
