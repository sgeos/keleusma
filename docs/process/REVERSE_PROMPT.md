# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-16
**Status**: Operator clarifications applied. Three behaviours are confirmed in the language and compiler. Untyped parameters are inferred from context rather than rejected at parse time. Multi-headed entry points compile for all three function categories. Duplicate function heads are rejected uniformly.

## Completed in this session round

| Operator clarification | Resolution |
|------------------------|------------|
| `fn main(x) -> Word { x }` infers `x: Word` | The earlier round's parser-level rejection of untyped parameters is reverted. The type checker now writes inferred primitive types back into the AST after each function body is checked. The compiler's `type_tag_for_param` reads from the filled-in `param.type_expr`, so the chunk's `param_types` carries the inferred tag. `Vm::call(&[Value::Float(1.5)])` against the inferred `Word` parameter is rejected at the boundary, as required. |
| Multi-headed entry points for `fn`, `yield`, `loop` | `fn` and `yield` already supported it; `loop main(...)` previously returned "multiheaded stream (loop) functions are not supported". The compiler now wraps the Stream dispatch in `Op::Loop`/`Op::EndLoop`. Each matched head's body ends with `Op::Pop` and `Op::Break` so the chunk retains exactly one `Op::Stream` and exactly one `Op::Reset`. The productivity rule is satisfied via the existing `analyze_yield_coverage` Loop+Break path. |
| Duplicate function heads rejected uniformly | The pattern-shape check already rejects duplicate heads at compile time. New tests cover the entry-point and non-entry-point cases for all three categories. |

## Verification matrix

```bash
cargo test --workspace --features text,shell                                  # 520 lib + ancillary, all pass
cargo test -p keleusma --no-default-features                                  # 488 lib + ancillary, all pass
cargo clippy --workspace --tests --features text,shell -- -D warnings         # clean
cargo clippy --no-default-features --tests -- -D warnings                     # clean
cargo fmt --check                                                              # clean
```

New tests added this round (10):

- `compiler::tests::untyped_param_is_inferred_from_return_type`
- `compiler::tests::multiheaded_fn_main_dispatches`
- `compiler::tests::multiheaded_yield_main_dispatches`
- `compiler::tests::multiheaded_loop_main_dispatches`
- `compiler::tests::duplicate_fn_main_is_rejected`
- `compiler::tests::duplicate_yield_main_is_rejected`
- `compiler::tests::duplicate_loop_main_is_rejected`
- `compiler::tests::duplicate_non_entry_function_is_rejected`
- `vm::tests::untyped_param_inferred_rejects_wrong_type_at_call`
- `vm::tests::multiheaded_loop_main_executes`

## Notes

- The `pub fn check(program: &Program)` signature changed to `pub fn check(program: &mut Program)`. The mutating variant is required so the inferred types can be written back into `param.type_expr`. Both internal call sites (in `compile_with_target`) and the typecheck test helper were updated accordingly. There are no external callers of `typecheck::check` because the module is `pub` but the project surface routes through `compile`.
- The "untyped parameters are now `ParseError`" FAQ entry from the prior round was rewritten to describe inference. The CHANGELOG entry was likewise rewritten.

## Intended Next Step

Awaiting operator prompt. Candidate next directions, ordered by likely value:

1. **Cut a V0.2.0 release candidate** against the branch. The reviewer-flagged items and the operator clarifications are all addressed; the remaining behaviour is documented design rather than bug surface.
2. **Lift `String` into the `KeleusmaType` marshalling family**, optionally adding tuple return types so the shell `run` native can register through the typed marshalling entry point.
3. **Target-scaled `Fixed` fraction bits** for sub-64-bit targets. Thread `Target` through `FuncCompiler` so 16-bit and 32-bit targets emit `Op::FixedMul(8)` and `Op::FixedMul(16)`.
