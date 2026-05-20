# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: Pattern-matched checked-arithmetic arms with `(h, l)` bindings, match-arm guards, and i128 intermediate runtime computation. Breaking syntax change committed (V0.2 unreleased; narrow-adoption rationale extended). 642 lib tests pass. Grammar, language-design doc, MANUAL.md Section 5.5, microkernel heartbeat script, CHANGELOG, and TASKLOG all updated.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Match-arm guards. | `MatchArm` gains `guard: Option<Expr>`. Parser admits `pattern when expr => body`. Type checker enforces `Bool` on the guard expression and treats guarded arms as non-catch-all in exhaustiveness analysis (wildcard / variable catch-all, Bool true/false coverage, enum-variant coverage, Unit-literal coverage, Option Some/None coverage all skip guarded arms). Compiler emits the guard as another `Op::If` fail-jump that participates in the existing pattern-test fail-jump list. Three new VM tests. |
| Pattern-matched checked-arithmetic arms. | `CheckedArmKind` rewritten: `Ok(Pattern)`, `Overflow(Pattern, Pattern)`, `Underflow(Pattern, Pattern)`. `CheckedArm` gains `guard: Option<Expr>` and drops the pipe-combined `kinds` Vec in favour of a single `kind`. Patterns are admitted from a restricted subset (wildcard, variable, signed integer literal) by a new `parse_checked_arm_pattern` helper. Type-check exhaustiveness shifts from "exactly one of each outcome" to "each outcome's last covering arm is an unguarded catch-all (bare identifier or wildcard in every position)". A `bind_checked_pattern` helper binds variables into the arm scope. |
| i128 intermediate runtime and (h, l, flag) push. | `Op::CheckedAdd`, `Op::CheckedSub`, `Op::CheckedMul`, `Op::CheckedNeg` now compute the true result in `i128` and push `(high, low, flag)`. Flag derivation uses the i128 range relative to `i64::MIN`/`i64::MAX` rather than the i64 wrap pattern (the prior implementation produced incorrect flags for `i64::MAX + 1`). Bytecode stack-effect entries updated: binary growth `1`, unary growth `2`, shrink `0` for all four. Division and modulo continue to use the stamped-zero-flag pattern (high = 0, flag = 0). |
| Compiler dispatch rewrite. | `compile_checked` rewritten as a virtual loop over arms. For each arm: emit a class-flag equality test (`flag == 0` / `1` / `2`), then literal-pattern equality tests against the high/low slots, then variable-pattern binding into fresh locals, then guard evaluation, then arm body, then `Break`. Failure jumps from any of the tests fall through to the next arm. Defensive `Op::Trap` after the last arm covers the unreachable no-match case. |
| Refined-newtype saturate contracts. | Confirmed unchanged behaviour under the new arm shape; the expected-type-stack push remains driven by annotated `let` bindings and function return types, and the `Expr::SaturateMax` / `Expr::SaturateMin` resolution path still consults `ctx.expected_type()` after stripping information-flow labels. All three pre-existing saturate-contract tests pass under the new arm shape. |
| Migration of existing call sites. | Six VM-level `checked_*` tests rewritten to the new arm syntax. Six typechecker-level `checked_overflow_*` tests rewritten with the updated error-message expectations (`non-exhaustive on ok|overflow|underflow`). The microkernel heartbeat script (`examples/rtos/scripts/heartbeat.kel`) updated in place. `examples/rtos/MANUAL.md` Section 5.5 updated. `docs/design/GRAMMAR.md` EBNF and example updated. `docs/architecture/LANGUAGE_DESIGN.md` Surface Extensions section updated. The pipe-combined `overflow|underflow => body` test (`checked_overflow_combined_arm_via_pipe`) is removed; that form is no longer admitted. |

## Verification matrix

```bash
cargo build --quiet                                                            # clean
cargo test --lib --quiet                                                       # 642 lib tests pass
cargo test --workspace --quiet                                                 # all workspace + doctest crates clean
cargo clippy --tests --quiet -- -D warnings                                    # clean
cargo fmt --all                                                                # idempotent
```

Tests of interest:

- `match_arm_guard_dispatches_on_runtime_predicate`, `match_arm_guard_falls_through_to_next_arm_when_false`, `match_arm_guarded_pattern_is_not_a_catchall`.
- `checked_mul_overflow_exposes_high_half`: `m * m` for `m = 2^32` produces `i128 = 2^64 = (high=1, low=0)`; the body returns the high half, demonstrating that the new shape exposes the load-bearing big-number-multiplication value.
- `checked_overflow_arm_pattern_matches_literal_high`: `i64::MAX + i64::MAX` produces `(high=0, low=-2 wrapped)`; the `overflow(0, l)` arm fires before the catch-all.
- `checked_overflow_arm_guard_falls_through`: the first overflow arm's pattern matches but its guard returns false; dispatch falls through to the catch-all.
- All three `saturate_keywords_*` tests pass unchanged under the new arm shape.

## Notes

- The pipe-combined `overflow|underflow => body` form is removed. The migration is mechanical: rewrite as two arms with the same body. The microkernel heartbeat script never used this form; the only consumer was a single VM test which has been deleted.
- Bytecode wire format `BYTECODE_VERSION` stays at 1. The narrow-adoption rationale extends to this change: V0.2 is unreleased, the existing `Op::CheckedAdd`/`Sub`/`Mul`/`Neg` discriminants are reused with changed stack effects, and no shipping consumer holds bytecode in the old (`result, flag`) shape. Future consumers should pin against the V0.2.0 commit if they need an authoritative wire-format reference for this label.
- Division and modulo overflow still stamp `(high=0, low=result, flag=0)` because the only true overflow case (`i64::MIN / -1`) is left to the existing arithmetic. A dedicated `Op::CheckedDiv` / `Op::CheckedMod` family is deferred until a real consumer needs the corner-case detection.
- Arm-pattern shapes that don't satisfy the restricted subset (struct patterns, enum-variant patterns, tuples) are rejected at parse time by `parse_checked_arm_pattern` with a span-localized diagnostic. Type checker fallback for unknown shapes binds nothing rather than panicking so any escaped case surfaces as a missing-identifier error in the body.

## Intended Next Step

Awaiting operator prompt.

1. **Operator action**: hardware verification on STM32N6570-DK. The full command set was provided previously and remains current. The microkernel heartbeat script rebuilds correctly under the new arm shape; an N6 flash run confirms the kernel-construction and dispatch timeline is unchanged.
2. **Operator action**: V0.2 release tag. With the checked-arithmetic and match-guard work landed, V0.2 carries a meaningful generalization of the construct that closes Item 2 of the gap list with broader semantics than the original ask.
3. **Backlog**: `Op::CheckedDiv` / `Op::CheckedMod` with proper `(h, l, flag)` for the `i64::MIN / -1` corner, `Type::Unknown` removal (B1 follow-up), refinement-type compile-time elision through range analysis (Item 4 follow-up), and the remaining target-scaled `Fixed` and embassy feature trimming items.
