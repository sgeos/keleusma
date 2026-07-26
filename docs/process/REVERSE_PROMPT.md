# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-25 (session 33)

## Current state

- **Autonomy loop RUNNING. Increment 2 (enum-in-struct equality) is COMPLETE, full gate GREEN,
  merged to `v0.2.3`.** The loop selected it without an operator prompt (the context-switching-first
  task-ordering policy). `s1 == s2` where a struct field is an enum now self-compiles byte-identically.
- **Construct-support boundary: 49 Ok / 5 Gap / 1 RefRejects** (was 48 / 6 / 1). The tuple-of-struct
  and enum-in-struct gaps are both closed.
- No opcode, record/node kind, or `BYTECODE_VERSION` change (nested variant tag 3 rides record 56's
  existing 2-bit field). `EXPECTED_SELF_COMPILE` 68 -> 69 (a factored `push_nested_enum_loop`).

## Verification

- Byte-identity oracle green: `self_host_compiles_enum_in_struct_equality` (unit/payload variants,
  `==`/`!=`, nonzero enum-field offset, enum beside a nested struct), all five whole-stage
  self-compiles (lexer/parse/reconstruct/codegen/analyze), the full nested-equality blast-radius
  suite, `validate_module_via_kel`, and `self_hosted_construct_support_boundary`.
- **Full `scripts/release-gate.sh` GREEN**: fmt, clippy `-D warnings`, the feature matrix (default,
  no-default, signatures, signatures+shell), docs `-D warnings`, markdown links, and the detached
  `compiler/` subproject. Merged to `v0.2.3` only after this.

## Implementation notes (for the next increment)

- Two capacity gotchas recurred (the tuple-of-struct failure mode, caught by the FULL gate, not a
  spot check): `push_struct_eq_nested` grew past the 1536 op cap (fixed by factoring
  `push_nested_enum_loop`), and two per-chunk-op scan loops in `analyze.kel` were still bounded at
  1024 (the tuple-of-struct sweep raised the op-table arrays but missed these — raised to 1536).
  A future increment that grows a codegen chunk should check BOTH the op-table cap AND the analyze
  scan-loop bounds.
- The nested enum field interns its pool EAGERLY (replaying `push_enum_eq`'s order), which works only
  because `s1 == s2` has no literal operand. A construct WITH a literal composite operand would need
  the deferred path.

## Next step

Continue the loop. The remaining nested-equality gaps are the harder tail (per the boundary test and
the DESIGN_JOURNAL re-scout): 2-level nesting (`struct O { m: M }` where M has a composite field —
needs the streaming machine to recurse, rated extreme), struct-of-array-of-struct (an intentional
`struct_eq_kind` defer), and enum-with-struct-payload. Per the task-ordering policy, the loop should
next weigh a same-context nested-equality gap against switching workstreams (for example wiring the
self-hosted stages into the shipping binary, Workstream A's highest-leverage residual). If every
remaining same-context gap needs a genuine design decision (not merely deep work), that is a Stop —
surface the options to the operator rather than pick unilaterally.
