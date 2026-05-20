# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Consolidation B follow-up complete on the `V0.2.0-isa` branch. `Int` arithmetic now routes through `CheckedAdd` / `CheckedSub` / `CheckedMul` / `CheckedNeg` followed by `PopN(2)`; `Op::Add`, `Op::Sub`, `Op::Mul`, and `Op::Neg` narrowed to `Byte` / `Fixed` / `Float` operand types. The opcodes remain in the enum because the runtime still needs entry points for those non-`Int` types; the audit's aspirational target of dropping them entirely is not reached in this pass.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Move forward with Consolidation B follow-up. | Compiler emits `CheckedXxx; PopN(2)` for `Word` operands at every `BinOp::Add` / `BinOp::Sub` / `BinOp::Mul` / `UnaryOp::Neg` emission site plus the compiler-internal sites for array-indexing stride and offset arithmetic and for-loop counter increments. `infer_expr_type` extended with `Expr::TupleIndex` and `Expr::TupleLiteral` arms; `struct_name_of` extended with a recursive `Expr::FieldAccess` arm so nested struct or data-block field paths resolve. `compile_let_pattern_typed` decomposes tuple types so inner-pattern binds carry element types. `compile_checked` types the `ok` / `overflow` / `underflow` arm variable bindings as `Word`. Inference defaults to `Word` when the partial `infer_expr_type` returns `None` so host-native call results and chained data-segment accesses route through the checked family. VM dispatch for `Op::Add` and `Op::Neg` drops the `Int` arm; the `binary_arith` helper used by `Op::Sub` and `Op::Mul` likewise drops its `Int` arm. Narrow-bytecode-on-wide-runtime preserved through a new `truncate_int_to_declared_width` helper applied to the `low` half of every `CheckedXxx` dispatch; the `flag` and `high` halves remain relative to the runtime word width pending a follow-up narrow-width overflow-detection pass. |

## Verification matrix

```bash
cargo test --workspace                                                          # 750 lib + 53 rogue-script + 17 marshall tests, all green
cargo clippy --tests --all-targets -- -D warnings                               # clean
cargo build --examples                                                          # clean
cargo fmt --all                                                                 # idempotent

# Bare-metal STM32N6570-DK build, full pipeline.
(cd examples/rtos && cargo build --release --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # clean
```

## Open concerns

| Item | Note |
|------|------|
| Narrow-bytecode-on-wide-runtime: `flag` and `high` from `CheckedXxx` are relative to the runtime word width, not the bytecode's declared word width. | The `low` half is correctly sign-extended truncated through `truncate_int_to_declared_width`, so the wrapping-arithmetic synthesis (`CheckedXxx; PopN(2)`) honors the declared width. Narrow-width overflow detection through `flag` and the `high` half is deferred to a follow-up task. The single existing test (`bytecode_masking_truncates_to_declared_width`) inspects only `low` and passes. |
| `Op::Add`, `Op::Sub`, `Op::Mul`, `Op::Neg` remain in the Op enum. | The audit's aspirational target of dropping the four unchecked arithmetic opcodes is unreached. The opcodes serve `Byte`, `Fixed`, and `Float` operand types; fully dropping them requires either adding type-specific opcodes (`ByteAdd`, `FloatAdd`, etc.) or removing script-level `Byte` and `Fixed` arithmetic. Both alternatives are out of scope for Consolidation B and are tracked in B20's phase plan. |
| Op count is 74 (was 71 pre-V0.2.0; audit target was 65). | Phase 4 (closure opcode drop, `CallIndirect` / `PushFunc` / `MakeClosure` / `MakeRecursiveClosure`) will bring the count to 70. Reaching 65 would require revisiting the `Op::Add` family removal under the alternatives above. |

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Deferred to V0.3 |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B complete; Phases 4–8 pending) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 4: closure opcode removal (`CallIndirect`, `PushFunc`, `MakeClosure`, `MakeRecursiveClosure`).
- B20 Phase 5: native ABI split refinement (verified-versus-external semantics; source-level `use external` keyword).
- A narrow-width overflow-detection follow-up that brings `CheckedXxx` flag and high-half reporting in line with the bytecode's declared word width.
- Operator selection of a different directive.
