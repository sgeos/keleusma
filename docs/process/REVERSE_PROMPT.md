# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-30 (session 35)

## Current state — tuple-of-deep-struct + 3-level struct both COMPLETE and merged to `v0.2.3` (CI green)

- **Tuple-of-deep-struct equality is COMPLETE, merged (`67539e7`).** A tuple whose struct element nests
  arbitrarily deep is admitted, reusing the 3-level frame-stack machinery with NO new code path — only the
  admission `tuple_eq_kind` was widened to `struct_subtree_pure`. Boundary +1 (`eq/tuple_of_deep_struct`).
  Full gate GREEN.
- **Third-level (and deeper) struct-nesting equality is COMPLETE, merged (`5c93920`, CI green).**
  Arbitrary-depth nested struct-in-struct equality self-compiles byte-identically; boundary moved
  **52 -> 53 Ok**, now 54 Ok with the tuple case (2 Gap / 1 RefRejects). All four stages landed.
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

## Next step — continue the "deeper nesting" workstream (operator-chosen), or pause

The operator chose the **deeper-nesting-for-other-composites** workstream after the 3-level struct merge.
The first, smallest increment (tuple-of-deep-struct, admission-only) is DONE. Remaining gaps, roughly by
increasing effort — each needs its OWN drain generalization (unlike tuple-of-deep-struct), so each is a
real multi-stage byte-identity increment like the 3-level struct one:
- **tuple-in-tuple** (a tuple element that is itself a tuple; `tup_ekind >= 100` currently defers) — the
  emit-DFS would need a per-frame accessor/variant (Tuple vs Struct) rather than the hardcoded `getfield`.
- **deeper array nesting** (array-of-array-of-struct, array element that is a deep composite).
- **deeper enum payloads** (enum variant carrying a nested composite deeper than one level).
- **mixed subtrees** (a struct/tuple whose subtree mixes struct+tuple+array+enum; `struct_subtree_pure`
  currently defers on any non-struct in the subtree).

STRONG RECOMMENDATION given the seven-day rate-limit (the binding budget) and that THIS SESSION already
merged THREE CI-green increments (CLI-backend hardening, 3-level struct, tuple-of-deep-struct): this is a
natural stopping point. Resume the next increment (tuple-in-tuple is the next smallest) from this channel
and the 3-level plan doc, which documents the reusable pattern. Note: the self-host suite ran ~4-5x slower
than normal this session (transient host load) — budget extra wall-clock if it persists.
