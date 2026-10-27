# Order 6 completion audit

> **Navigation** [Full-language plan](./ORDER6_FULL_LANGUAGE_PLAN.md) | [Order 5 audit](./ORDER5_COMPLETION_AUDIT.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

## Scope and disposition

This audit records the state of Order 6, full-language support for the self-hosted
toolchain, on 2026-10-27. Its scope is the gate of
[the full-language plan](./ORDER6_FULL_LANGUAGE_PLAN.md). **That gate is this line's reading
of the roadmap gate and has not been ruled on by the operator.**

**Disposition.** Order 6 is not complete. Every gate condition is met except for one family
of forms, and that family waits on an operator ruling rather than on work this line can do
alone. The forms are the information-flow labels, `classify` and `declassify`, and the
newtype with its refinement predicate. Operator question 10 in
[the reverse prompt](../process/REVERSE_PROMPT.md) asks whether the self-hosted type stage
should check label flow and nominal newtype identity, or whether the reference cross-check
is enough. They stay refused until it is answered, and one repository source,
`examples/scripts/07_refinement.kel`, is among them. `BYTECODE_VERSION` remains 2, the
instruction set remains 66 opcodes, the wire format is unchanged, and nothing was published.
Stage capacities were raised under the operator's ruling of 2026-10-26 that limits may be
raised, each with its cost recorded in the plan.

## Gate evidence

All tests named below are in `tests/selfhost_full_language.rs` unless another file is
named. The figures were measured on 2026-10-27 over the tree of pull request 576 and the
gate 5 branch, with `cargo test --release --features self-host --test
selfhost_full_language`.

| Gate condition | Evidence | Disposition |
|---|---|---|
| 1. Every repository source the reference compiles is Identical, and every source it rejects is rejected by both | `every_source_has_its_pinned_self_hosted_outcome` pins 75 sources, 69 Identical, 5 rejected by the reference and 1 refused. The five are RTOS scripts the reference rejects standalone because their natives are declared in the RTOS prelude, and their prelude-joined forms are Identical in `rtos_scripts_joined_with_the_prelude_have_their_pinned_outcomes`. The refused source is `07_refinement.kel` | Met except `07_refinement.kel`, which awaits question 10 |
| 2. A construct corpus with a program for every reference AST variant is byte-identical | `the_census_names_every_ast_variant` finds a row for every variant of the six AST enums. `the_ast_census_has_its_pinned_outcomes` reads 66 Identical, 5 Unsupported and 3 RefRejects of 74 rows. The five Unsupported rows are `Classify`, `Declassify`, `Labelled`, `NegativeLabelled` and `Newtype` | Met except the five forms awaiting question 10 |
| 3. The self-hosted validator gives the reference verdict on every compiled source and on a rejection corpus, with no capacity refusal | `the_self_hosted_validator_agrees_with_the_reference_on_every_compiled_source` finds no disagreement and an empty capacity list. `the_self_hosted_validator_agrees_with_the_reference_on_a_mutated_corpus` finds no disagreement over 350 mutated modules in seven families | Met |
| 4. The trap-free analysis agrees with the reference on the widened corpus | `the_trap_analysis_agrees_with_the_reference_on_every_compiled_source` compares 70 sources, 46 MayTrap, 23 TrapFree and 1 refused by both analyses, `scripts/check-md-links.kel`, for its boxed tuple read | Met, with the one shared refusal |
| 5. The shipping command line uses the widened toolchain | `keleusma-cli/tests/self_hosted_full_language.rs` runs `keleusma compile --compiler self-hosted` on every Identical census row, read from the census table, and finds each artifact equal to the reference serialization | Met for the census rows |

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

## Residual limits

These are refused, not miscompiled, and each is pinned by a test, so a change is a test
failure rather than a silent move.

- The five forms awaiting question 10, above.
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

## Independent review

No independent review of the Order 6 work has been performed. The evidence above is this
line's own measurement, and the differential cross-check against the reference is the
correctness signal. A divergence shows that the two compilers disagree, not which is wrong,
and agreement shows the same of agreement.

## Verification receipts

Measured on 2026-10-27 over the tree of pull request 576 with the gate 5 test added.

| Command | Result |
|---|---|
| `cargo nextest run --cargo-profile release --features self-host --no-fail-fast` | 3301 run, 3300 passed before the roadmap count text was updated, then the remaining guard passed |
| `cargo tnq` | 2684 of 2684 passed |
| `cd compiler && cargo test --release` | Passed |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` | Clean |
| `cargo test --release -p keleusma-cli --test self_hosted_full_language` | Passed |

Re-measure before describing a later tree.
