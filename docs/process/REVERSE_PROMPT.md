# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: V0.2.0 ISA Phase 4 complete on the `V0.2.0-isa` branch. The closure family (`Op::PushFunc`, `Op::MakeClosure`, `Op::MakeRecursiveClosure`, `Op::CallIndirect`) and the `Value::Func` runtime variant are removed. Closures are rejected at the type-checker stage. Opcode count is 70 (was 74 after Consolidation B, was 71 in V0.1.x).

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Proceed with Phase 4. Removal is acceptable if feasible upon further inspection. | Removal proved feasible. Type checker rejects `Expr::Closure` with a clear diagnostic. Closure-hoisting compiler pass retired entirely. Compiler rejects first-class function references and call-a-local invocations. The four closure opcodes and `Value::Func` are removed from `bytecode.rs`, `vm.rs`, `verify.rs`, `compiler.rs`, `typecheck.rs`, and `keleusma-bench/src/lib.rs`. Eight closure-typecheck tests, one compiler closure-span test, and two verifier closure-rejection tests retargeted at the typecheck-stage rejection path. Golden-bytes test updated to reflect the smaller archived-op tag. |
| Note that `Op::Add`/Sub/Mul/Neg might reasonably serve as Float ops while Checked* serve as Word ops. | The current state matches that framing: after Consolidation B and Phase 4, `Op::Add` / `Sub` / `Mul` / `Neg` serve `Byte`, `Fixed`, and `Float` operand types (Float being the primary script-visible use case); the `CheckedXxx` family is the canonical `Int` arithmetic. Narrowing `Op::Add` further to Float-only would require adding `Op::ByteAdd` / `Op::FixedAdd` (and equivalents for Sub/Mul/Neg) or removing script-level `Byte` and `Fixed` arithmetic; either alternative grows the opcode count or restricts the surface language and is out of scope for V0.2.0. |

## Verification matrix

```bash
cargo test --workspace                                                          # 747 lib + 53 rogue-script + 17 marshall tests, all green
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
| `Expr::Closure` and `Expr::ClosureRef` AST variants survive. | The parser still produces `Expr::Closure` from source so the type checker can emit a precise diagnostic at the closure expression site. `Expr::ClosureRef` is now dead in practice because the hoisting pass is gone; the compiler treats it as a compiler-internal error. Removing the variants entirely would be a parser-side change with no functional gain. |
| Narrow-bytecode-on-wide-runtime `flag` and `high` halves from `CheckedXxx` are relative to the runtime word width, not the bytecode's declared width. | Inherited from Consolidation B; deferred. The `low` half is correctly sign-extended truncated through `truncate_int_to_declared_width`. |

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | In progress (Phases 1, 2, 3, 3.5, Consolidation B, 4 complete; Phases 5–8 pending) |

## Intended Next Step

Awaiting operator prompt. The next development action is one of:

- B20 Phase 5: native ABI split refinement (verified-versus-external semantics; source-level `use external` keyword).
- B20 Phase 6: control-flow operand narrowing `u32` → `u16` with 80% soft warning.
- B20 Phase 7: wire format with fixed-size opcode records and operand pool.
- B20 Phase 8: documentation alignment and `BYTECODE_VERSION` reset to 1.
- A narrow-width overflow-detection follow-up that brings `CheckedXxx` flag and high-half reporting in line with the bytecode's declared word width.
- Operator selection of a different directive.
