# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-29 (session 35)

## Current state — IN-PROGRESS increment on a feature branch (checkpoint)

- **Third-level struct-nesting equality (operator's second fork selection: the general bounded-depth
  stack) is IN PROGRESS on `feat/selfhost-3level-struct-eq` (cut from `v0.2.3` at `4037174`).** Two of
  three stages are DONE and verified green; stage 3 (the hardest) and the depth-3 wiring remain. The
  branch is a clean, resumable checkpoint. Design and precise continuation in
  [`docs/decisions/STRUCT_3LEVEL_PLAN.md`](../decisions/STRUCT_3LEVEL_PLAN.md).
  - Stage 1 (parse.kel, `13b922f`): fixed depth-2 `se_l2*` -> general `se_stk_*` frame stack +
    `se_pop_cascade`. Depth-1/2 records byte-identical. Boundary unchanged; parse.kel self-compiles.
  - Stage 2 (reconstruct.kel, `c667875`): `se_nsub_mode` -> general `se_nstk_*` frame stack +
    `se_nsub_pop`; recursive `seb` grammar. Depth-1/2 `seb` byte-identical. Boundary unchanged;
    reconstruct.kel self-compiles.
  - Stage 3 (codegen.kel): NOT STARTED. `push_struct_eq_subfields`'s inlined depth-2 case must become
    an explicit-stack reverse-DFS emitter (R4 forbids the reference's recursion). Highest byte-identity
    risk. Then stage 4 wires the `eq/3level_struct` boundary case (52 -> 53 Ok). Full design in the plan.
  - NOTE: correction to the earlier fork framing — the two boundary Gaps are `float_arith` and
    `generic_fn` (permanently out of scope). This increment ADDS a new `SOk` case (52 -> 53 Ok); it does
    NOT flip an existing Gap.
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

## Next step — RESUME STAGE 3 of the 3-level increment (branch `feat/selfhost-3level-struct-eq`)

The increment is mid-flight at a clean, green checkpoint (stages 1+2 committed; branch NOT merged). To
resume:
1. `git checkout feat/selfhost-3level-struct-eq`; confirm HEAD is the stage-2 commit `c667875` (or the
   later doc-checkpoint commit) and the working tree is clean.
2. Implement **Stage 3** per `docs/decisions/STRUCT_3LEVEL_PLAN.md` "Stage 3 design": generalize
   `push_struct_eq_subfields` (codegen.kel ~1968) from its inlined depth-2 case to an explicit-stack
   reverse-DFS emitter; verify the depth-2 fixture stays byte-identical FIRST (a regression is a stop),
   then generalize the slot-count and (if needed) intern passes.
3. **Stage 4**: add `eq/3level_struct` to `self_hosted_construct_support_boundary` as `SOk` (52 -> 53 Ok)
   and `self_host_compiles_3level_struct_equality`; run the full self-compile suite and the FULL
   `scripts/release-gate.sh`; on green, no-ff merge into `v0.2.3`, push, confirm CI.

Inner-loop verification: `scripts/fast-check.sh 'test(self_host_compiles_2level_struct_equality)'` for
the regression, then the boundary and codegen self-compile. Each stage self-compile / boundary run is
~90-215s; background and poll to avoid the 600s watchdog.

The seven-day rate-limit window is the binding budget; stage 3 is the most expensive, byte-identity-
sensitive piece, so it was checkpointed here rather than rushed. Pace accordingly.
