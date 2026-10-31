# Order 6 completion audit

> **Navigation** [Full-language plan](./ORDER6_FULL_LANGUAGE_PLAN.md) | [Order 5 audit](./ORDER5_COMPLETION_AUDIT.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope and disposition

This audit records the state of Order 6, full-language support for the self-hosted
toolchain, on 2026-10-27. Its scope is the gate of
[the full-language plan](./ORDER6_FULL_LANGUAGE_PLAN.md). **That gate is this line's reading
of the roadmap gate and has not been ruled on by the operator.**

**Accepted by the operator first-hand on 2026-10-28.** The operator accepted this gate and
ruled on question 10, "Cross-check is enough", so the self-hosted type stage does not port
label-flow or newtype checking.

**Disposition, revised 2026-10-27.** Order 6 is complete for its first-pass scope on this
line's reading of the gate, pending the operator's acceptance. Every gate condition is met.
The information-flow labels, `classify` and `declassify`, and the newtype with its
refinement predicate, which the first version of this audit recorded as waiting on operator
question 10, now compile byte-identically by erasure, as the reference compiles them.
**Question 10 is not decided by that.** It asks whether the self-hosted type stage should
also check label flow and nominal newtype identity, and until it is answered a violation is
rejected by the reference cross-check alone, the same standing the generic and Option
arguments have. That is a residual of the type stage, listed below, and not a refusal. `BYTECODE_VERSION` remains 2, the
instruction set remains 66 opcodes, the wire format is unchanged, and nothing was published.
Stage capacities were raised under the operator's ruling of 2026-10-26 that limits may be
raised, each with its cost recorded in the plan.

## Gate evidence

All tests named below are in `tests/selfhost_full_language.rs` unless another file is
named. The census and corpus figures were measured on 2026-10-27 after the labels and
newtypes slice, with `cargo test --release --features self-host --test
selfhost_full_language`.

| Gate condition | Evidence | Disposition |
|---|---|---|
| 1. Every repository source the reference compiles is Identical, and every source it rejects is rejected by both | The per-source tests in module `every_source_has_its_pinned_self_hosted_outcome`, one function until 2026-10-30, pin 75 sources, 70 Identical and 5 rejected by the reference. The five are RTOS scripts the reference rejects standalone because their natives are declared in the RTOS prelude, and their prelude-joined forms are Identical in `rtos_scripts_joined_with_the_prelude_have_their_pinned_outcomes` | Met |
| 2. A construct corpus with a program for every reference AST variant is byte-identical | `the_census_names_every_ast_variant` finds a row for every variant of the six AST enums. `the_ast_census_has_its_pinned_outcomes` reads 71 Identical and 3 RefRejects of 74 rows, the three being forms the reference itself rejects | Met |
| 3. The self-hosted validator gives the reference verdict on every compiled source and on a rejection corpus, with no capacity refusal | `the_self_hosted_validator_agrees_with_the_reference_on_every_compiled_source` finds no disagreement and an empty capacity list. `the_self_hosted_validator_agrees_with_the_reference_on_a_mutated_corpus` finds no disagreement over 350 mutated modules in seven families | Met |
| 4. The trap-free analysis agrees with the reference on the widened corpus | `the_trap_analysis_agrees_with_the_reference_on_every_compiled_source` compares 70 sources, 46 MayTrap, 23 TrapFree and 1 refused by both analyses, `scripts/check-md-links.kel`, for its boxed tuple read | Met, with the one shared refusal |
| 5. The shipping command line uses the widened toolchain | `keleusma-cli/tests/self_hosted_full_language.rs` runs `keleusma compile --compiler self-hosted` on every Identical census row, 71, read from the census table, and finds each artifact equal to the reference serialization | Met for the census rows |

The construct-support boundary in `tests/selfhost_codegen.rs` reads 113 SOk, 2 Refuses,
2 Diverges and 1 RefRejects over 118 cases, and the roadmap states the same counts.

## Defects found and repaired

Each was found by this line's own checks during Order 6 and repaired before or in the slice
that exposed it. They are listed because a reader weighing the evidence above should know
which classes of defect the instruments have already caught.

1. A `break` in a `for` without `limit` was lowered as if the loop had one, writing into
   the first parameter's slot. The cross-check had refused every such program.
2. `analyze.kel` overestimated the execution bound of an enum match in a stream, and
   `verify_depth.kel` accepted a fall-through root that the reference rejects.
3. The first form of the Option payload register went stale across bindings, so the
   self-hosted type stage rejected sources the reference accepts. That was a false
   rejection, which is worse than a refusal, and the regressions are pinned.
4. The generic request record first reused tag 116, a body record, and every loop-outcome
   program was refused until it moved to tag 140.
5. The plan recorded gate 1 as met on the whole corpus when `07_refinement.kel` was
   refused. The record is corrected in place with a dated correction.
6. A packed parser record carries its tag modulo 64, so the refinement check's tag 85
   first decoded as 21 until it moved to the two-word form.
7. The first newtype erasure read a newtype name as its base in a struct field too, where
   the reference boxes the struct, and the result diverged. It is now refused there.

## Residual limits

These are refused, not miscompiled, and each is pinned by a test, so a change is a test
failure rather than a silent move.

- Information-flow labels and newtype identity are not checked by the self-hosted type
  stage, which is question 10, so a violation is rejected by the reference cross-check
  alone. A newtype as a struct field, data field, payload or native signature type is
  refused.
- An `Option` over a struct, and Option values whose payload is not `Word`, `Byte` or
  `bool`. A binding of `Option::None`, an annotated Option binding and an Option-returning
  native are refused.
- The self-hosted type stage does not compare generic arguments or an Option payload with
  its declared `T`, so such a mismatch is rejected by the reference cross-check alone.
- Generic functions over a type argument other than `Word`, `Byte` or `bool`, an
  uninstantiated template, a piped call to a template, and a template returning a composite
  such as `Option<T>`.
- A match of more than 64 arms, and an enum match of more than about 51. No repository
  source reaches either.
- The header records 113 and 114 share their numbers with body records that reconstruct
  reads, and are correct only because the host routes them in its declaration dispatch.

## Instrument limits found

`the_stage_is_measured_against_the_real_stage_sources` builds the type stage's inputs from
the reference parser's AST, and that channel conflates same-named `let` bindings in
different functions, which made it report a false rejection of `parse.kel`. The shipping
pipeline is not affected, measured on three user programs. The binding was renamed, and the
limit is recorded in the plan.

## Independent review

No independent review of the Order 6 work has been performed. The evidence above is this
line's own measurement, and the differential cross-check against the reference is the
correctness signal. A divergence shows that the two compilers disagree, not which is wrong,
and agreement shows the same of agreement.

## Verification receipts

Measured on 2026-10-27 over the tree of the labels and newtypes slice, on branch
`feat/v02-order6-ifc-newtype`.

| Command | Result |
|---|---|
| `cargo nextest run --cargo-profile release --features self-host --no-fail-fast` | 3302 of 3302 passed |
| `cargo tnq` | 2685 of 2685 passed |
| `cd compiler && cargo test --release` | Passed |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` | Clean |
| `cargo test --release -p keleusma-cli --test self_hosted_full_language` | Passed |

The gate figures for the mutation corpus and the trap analysis, 350 modules and 70 sources,
were measured earlier the same day on the tree of pull request 576, before this slice, and
both tests pass on this tree. Re-measure before describing a later tree.
