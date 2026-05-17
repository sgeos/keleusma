# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-16
**Status**: Reviewer's final ten-item list addressed. Six items are now hard-rejection at the appropriate boundary (lex, parse, compile, or `Vm::new`), one item adds a dedicated `VmError` variant for API misuse, one item threads source spans through compile-time structural errors, and the remaining two items (loop-calls-loop lexicality, integer wrap) are documented as intentional design with their rationale in the FAQ.

## Completed in this session round

Reviewer's ten-item list. All items are addressed; the cosmetic `Value::StaticStr` naming nit is intentionally skipped.

| Item | Resolution |
|------|------------|
| Integer literal overflow silently truncates to `Value::Int(0)` | `LexError` at the offending literal's span. Decimal, hexadecimal, binary, and float paths all wrapped. |
| Untyped parameters compile while missing return type rejected | `ParseError` at the parameter's span. Symmetry restored with the existing return-type rejection. |
| Duplicate `fn main` silently accepted (first wins) | `CompileError` at the second definition. Detected by `pattern_shape_eq` across heads. |
| Same-literal pattern heads silently accepted as dead code | Covered by the same `pattern_shape_eq` check. Reported as `function head is dead code`. |
| Empty source / no entry point surfaces at `vm.call()` | `VmError::VerifyError("module has no entry point")` raised at `Vm::new` and `Vm::new_unchecked`. The compile-time check from an earlier draft was reverted because three lib tests legitimately compile entry-point-less modules; `Vm::new` is the right boundary. |
| Premature resume returns generic `InvalidBytecode` | New `VmError::NotSuspended` variant. The string `"cannot resume: VM not suspended"` is removed; the variant is self-describing. |
| Loop-calls-loop rejected because productivity rule is lexical | Documented in FAQ "Loop-calls-loop is rejected by lexical productivity" with the rationale (the structural pass is sound only when local) and the recommended workaround (keep `yield` at the top level of the `loop` body). |
| Integer arithmetic wraps silently | Documented in FAQ "Integer arithmetic wraps to the target word width" with the rationale (fixed step count for WCET) and the host-side checked-arithmetic native pattern. |
| Structural-verification errors carry `Span { 0, 0, 0, 0 }` | The compiler now builds a name-to-span lookup from `program.functions` (including hoisted closure synth defs) and threads the originating span into each `CompileError`. Verified by `recursive_closure_compile_error_carries_source_span`. |
| `Value::StaticStr(String)` naming nit | Skipped. Cosmetic only; renaming would churn every text-handling code path with no behavioural change. |

## Verification matrix

```bash
cargo test --lib --features text                                              # 510 pass (was 506; +4 new tests)
cargo test --workspace --features text,shell                                  # all pass
cargo test -p keleusma --no-default-features                                  # 478 pass
cargo clippy --workspace --tests --features text,shell -- -D warnings         # clean
cargo clippy --no-default-features --tests -- -D warnings                     # clean
```

New tests added this round:

- `vm::tests::new_rejects_module_without_entry_point`
- `vm::tests::premature_resume_returns_not_suspended`
- `vm::tests::resume_after_finished_returns_not_suspended`
- `compiler::tests::recursive_closure_compile_error_carries_source_span`

## Wire format

No wire-format change this round. The `param_types` field added in the previous round (`BYTECODE_VERSION 2`) is unchanged.

## Intended Next Step

Awaiting operator prompt. The reviewer's three documented lists (parser stack overflow, call/resume boundary, the final ten-item list) are now all addressed. Candidate next directions, ordered by likely value:

1. **Cut a V0.2.0 release candidate** against the branch. The reviewer-flagged hard-rejection items are all in place; the remaining behaviour is documented design rather than bug surface.
2. **Lift `String` into the `KeleusmaType` marshalling family**, optionally adding tuple return types so the shell `run` native can register through the typed marshalling entry point.
3. **Target-scaled `Fixed` fraction bits** for sub-64-bit targets. Thread `Target` through `FuncCompiler` so 16-bit and 32-bit targets emit `Op::FixedMul(8)` and `Op::FixedMul(16)`.
