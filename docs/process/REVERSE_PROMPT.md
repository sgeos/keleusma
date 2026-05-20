# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: Refined-newtype saturation contracts implemented end to end. The previously deferred Item 2 of the V0.2 gap list (`saturate_max` / `saturate_min` resolving from refinement contracts) is now resolved through a bidirectional type-checking pass and a context-driven AST mutation. 637 lib tests pass. Grammar, language-design doc, CHANGELOG, and TASKLOG updated.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Item 2. Saturate values from refinement contracts. | New surface syntax `newtype Name = T where pred with saturate_max = N, saturate_min = M` parses through `NewtypeDef.saturate_max` and `NewtypeDef.saturate_min`. Parser admits signed integer literals (including a leading minus). The type checker carries `Ctx::newtype_saturate_max` and `Ctx::newtype_saturate_min` populated in pass 1a' from the AST, and an `Ctx::expected_type_stack` pushed by annotated `let` bindings (Stmt::Let with a declared type) and by function return types (`check_function`). At the `Expr::SaturateMax` / `Expr::SaturateMin` site, the type checker peeks the top of the expected-type stack, strips any information-flow labels, and if the result is a `Type::Newtype` whose name has a recorded saturate value, mutates the AST node in place to `Expr::Call { name, args: [Literal::Int(value)], span }` and returns `Type::Newtype(name, underlying)`. The refinement predicate is verified at runtime on the literal exactly as for any other constructor invocation. Cascading `&mut` propagation through `type_of_expr`, `type_of_block`, `check_stmt`, `check_function`, and `check_native_call_with_signature` enables the mutation. Three new VM-level tests cover function-return context, annotated-let context, and the fall-back-to-`Word::MAX`/`MIN` path. `docs/design/GRAMMAR.md` Section 7.5 EBNF and `docs/architecture/LANGUAGE_DESIGN.md` Section "Surface Extensions Added in V0.2" updated. CHANGELOG entry added under `[Unreleased]` / Added. |

## Verification matrix

```bash
cargo build --quiet                                                            # clean
cargo test --lib --quiet                                                       # 637 lib tests pass
cargo clippy --tests --quiet -- -D warnings                                    # clean
```

The new behaviour is exercised by three tests in `src/vm.rs`:

- `saturate_keywords_resolve_to_newtype_contract_via_function_return`: function returning the refined newtype drives resolution of `saturate_max` to the declared value (100), wrapped by the newtype constructor; the runtime `nonneg` predicate accepts 100.
- `saturate_keywords_resolve_to_newtype_contract_via_let_annotation`: `let y: Limited = m - 2 { ... }` drives resolution of `saturate_min` to 0, with explicit `as Word` extraction in the function return.
- `saturate_keywords_fall_back_to_word_extrema_without_newtype_context`: a `fn main() -> Word` with no newtype context preserves the legacy `Word::MAX` semantics.

## Notes

- The expected-type stack is consulted only by `Expr::SaturateMax` and `Expr::SaturateMin` for now. Other surface positions that could plausibly push expected types (struct-field assignment, match-arm position) are not yet wired and would need a separate pass if future features want to use them.
- Refinement-driven cast paths and refinement-type compile-time elision (Item 4 of the original V0.2 gap list) remain on the backlog. Those require range analysis on the underlying type, which is a larger investment than bidirectional checking.
- `Type::Unknown` removal stays on the backlog. The foundation (native signatures and the expected-type stack) is now in place but a full removal still touches 26 call sites with risk of inference regressions.

## Intended Next Step

Awaiting operator prompt.

1. **Operator action**: hardware verification on STM32N6570-DK. The full command set was provided in the prior turn and covers host smoke test, bare-metal library compile check, the three-mode size check, flashing under all three feature combinations, and the per-mode pass criteria. The new saturate-contract feature does not affect the microkernel images because none of the demonstrator scripts adopt refined newtypes; the verification confirms the V0.2 closing pass remains intact.
2. **Operator action**: V0.2 release tag. With Items 2, 3, 4, 6, 7 of the V0.2 gap list now closed and flash items B, C, I delivered, V0.2 is in releasable shape. Item 5 (CallIndirect flow analysis) stays deferred to V0.3 as previously agreed. Operator decides timing.
3. **Backlog**: B1 follow-up (remove `Type::Unknown` entirely), Item 4 follow-up (compile-time elision of refinement predicates on provably-in-range arguments), target-scaled `Fixed` for sub-64-bit native runtimes, and the remaining embassy feature trimming.
