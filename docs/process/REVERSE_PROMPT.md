# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-29 (session 35)

## Current state — 3-level increment COMPLETE (branch green, pending full gate + merge)

- **Third-level (and deeper) struct-nesting equality is COMPLETE on `feat/selfhost-3level-struct-eq`
  (cut from `v0.2.3` at `4037174`).** Arbitrary-depth nested struct-in-struct equality self-compiles
  byte-identically; the construct-support boundary moved **52 -> 53 Ok** (2 Gap / 1 RefRejects). All four
  stages landed and verified via the differential oracle; the branch tip is green on the targeted
  self-host suite. NEXT: FULL `scripts/release-gate.sh`, then no-ff merge into `v0.2.3`, push, confirm CI.
  - Stage 1 (parse.kel, `13b922f`): `se_l2*` -> `se_stk_*` frame stack + `se_pop_cascade`.
  - Stage 2 (reconstruct.kel, `c667875`): `se_nsub_mode` -> `se_nstk_*` stack + `se_nsub_pop`; recursive
    `seb` grammar.
  - Stages 3+4 (`4aefcf2`): codegen `push_struct_eq_subfields` -> explicit-stack reverse-DFS emitter
    (`es_*`) + `struct_forest_end`/`nested_end`/`es_compute_sfoff`; and the ADMISSION fix — the fourth,
    unanticipated depth-2 assumption: `struct_eq_kind` only descended two levels so `D==D` fell back to a
    primitive compare. Generalized with `struct_subtree_pure`. `EXPECTED_SELF_COMPILE` 72 -> 75.
  - No opcode/record/node/`BYTECODE_VERSION` change. The two remaining Gaps (`float_arith`, `generic_fn`)
    are permanently out of scope; this ADDED a new `SOk` case.
- **Previously this session: the `--compiler self-hosted` backend error surface was HARDENED. COMPLETE,
  full gate GREEN, CI GREEN, merged to `v0.2.3` (`cf24f12`).** The operator selected "harden the CLI
  backend" at the first post-compaction fork.
- One of the two candidate sub-items was NOT pursued because it is a hard boundary rather than an
  oversight. Threading the CLI preamble through self-hosted mode is impossible in this fork. The
  self-hosted codegen emits no native-call opcode. Its emitted wire set is `decode_op` tags 1..=63,
  which carry `Op::Call` but neither `CallExternalNative` nor `CallVerifiedNative`, and the CLI preamble
  is entirely native `use` signatures. Native-call emission is a language-surface increment under a
  different fork.
- Delivered, the self-contained sub-item, richer subset-rejection errors, plus a correctness fix.
  `SelfHostError` gained `ReferenceRejected { detail }` for a genuine source error the reference compiler
  also rejects, distinct from a self-hosted-subset `Unsupported`. `rust_backend_would_help(&self)` is
  false only for `ReferenceRejected`. The CLI now appends the `retry with --compiler rust` hint only when
  it would help, and reports a genuine source error plainly. A new `describe_divergence` helper names the
  first diverging chunk and the specific dimension, so the float case now reads "chunk `main`: op 1
  diverges (Return vs reference Const(1))" rather than an opaque "diverges from the reference".
- No opcode, record, node kind, or `BYTECODE_VERSION` change, no `.kel` change. The construct-support
  boundary is UNCHANGED at **52 Ok / 2 Gap / 1 RefRejects**.

## Verification

- Three new `tests/self_hosted_backend.rs` tests pin the behavior. `ReferenceRejected` classification for
  an undefined identifier with `rust_backend_would_help() == false`, the `Unsupported` divergence detail
  naming `chunk `main``, and the `Unsupported` hint policy. All five backend tests pass.
- CLI end to end confirmed. Out-of-subset float prints the chunk-naming detail plus the retry hint at
  exit 1, a genuine source error prints a plain compile error with no retry hint at exit 1, and an
  in-subset program compiles at exit 0.
- The FULL `scripts/release-gate.sh` is GREEN.

## Next step — FULL GATE + MERGE, then OPERATOR-DECISION FORK

1. Run the FULL `scripts/release-gate.sh` on `feat/selfhost-3level-struct-eq`. On green, no-ff merge into
   `v0.2.3`, push (pre-push gate; keepalive), confirm CI green.
2. Then the loop is again at an operator fork (no bounded same-context task remains). Candidate
   directions to surface (do not auto-select):
   - **New self-host language surface** — pick another construct family the `.kel` stages still defer.
   - **Native-call support in the self-hosted pipeline** — larger; would make the CLI preamble meaningful.
   - **Deeper nesting for the OTHER composites** — the arbitrary-depth generalization landed only for
     struct-in-struct; nested tuple/array/enum still cap at their existing depths (the `es_*`/`se_stk_*`
     machinery is now in place to extend them similarly if desired).
   - **A different workstream** — release cadence (a V0.2.3 cut), backlog (B32/B33/B34).

The seven-day rate-limit window is the binding budget under heavy agent work; pace accordingly.
