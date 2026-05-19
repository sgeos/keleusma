# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: V0.2 design-decision pass items 3, 4, 6, 7 plus flash savings B, C, I complete. Item 5 (CallIndirect flow analysis) stays deferred. Item 8 (per-op decode cache or JIT) closed as superseded by B11. The microkernel's STM32N6570-DK bare-metal `.text` drops to 149 KB trust-load (was 192) and 169 KB precompile-plus-verify (was 211), leaving 471-491 KB of FLASH headroom for user code and NPU weights. 622 lib tests pass workspace-wide.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Item 3. Bare `Option::None` typecheck. | The `EnumVariant` site for `Option::None` previously returned `Option<Unknown>`, which could not unify through `Option`'s recursive arm because the unifier does not narrow `Unknown`. The fix splits `Some` and `None`: `Some(t)` uses the payload's inferred type; `None` uses a fresh type variable. Programs of the form `fn f() -> Option<T> { Option::None }` are now admitted. The previously-blocked positive test for the per-yield arena dataflow refinement is re-enabled. |
| Item 7. `VmError::category`. | New `VmErrorCategory` enum (`Halt`, `SoftScript`, `SoftHost`). Method returns the coarse retry-or-halt policy without storing per-error bytes. The split lets hosts make a single policy decision; concrete variant matching remains available for finer policy. |
| Item 6. Schema hash for hot swap. | New `Module::schema_hash: u32` field (CRC-32 of `(slot_name, visibility)` per slot in declaration order). `Vm::replace_module` now rejects schema-mismatched swaps strictly; new `Vm::replace_module_unchecked` keeps the legacy permissive behaviour for hosts that intend cross-layout swaps. Bytecode body grew by 4 bytes; golden test regenerated, `examples/zero_copy_demo.kel.bin` regenerated, `BYTECODE_LEN` updated in `zero_copy_include_bytes.rs`. |
| Item 4. Native function signature declarations. | New surface form `use host::name(T1, T2, ...) -> R` declares parameter and return types at the type-checker level. `ast::NativeSignature`, `UseDecl::signature: Option<NativeSignature>`, parser extension in `parse_use_decl`, `Ctx::native_signatures` populated during pass 1c0, `check_native_call_with_signature` helper. Bare `use host::name` form remains permissive. Microkernel `scripts/prelude.kel` declares signatures for all 17 host natives, catching arity and type errors at compile time. (Removing `Type::Unknown` entirely remains a backlog item; the foundation it requires is now in place.) |
| Flash savings B + C + I (~43 KB). | Microkernel kernel-error path was rewritten: `format!("{:?}", vmerror)` replaced with `Platform::log_event(category_code, data)` where the category code comes from `VmError::category`. This removed every transitive Debug-fmt reference, killing the float-formatter chain (`flt2dec::dragon`, `flt2dec::grisu`, `CACHED_POW10`, `__divdf3`, `__adddf3`, char `escape_debug_ext`) which was ~32 KB on its own. Release profile gained `panic = "abort"` (~1-2 KB from unwinding tables). New kernel event discriminants (`EV_KERNEL_VM_ERROR`, `EV_KERNEL_UNKNOWN_YIELD`, `EV_KERNEL_TASK_FINISHED`, `EV_KERNEL_UNEXPECTED_STATE`) with per-platform format-string arms preserve diagnostic visibility. |

Item 5 stays deferred to V0.3 backlog as recommended. Item 8 (per-op decode cache or JIT) is closed as superseded by B11 which already shipped the cache.

## Verification matrix

```bash
cargo test --workspace --features text                                         # 622 lib + 17+17+3+53+37+6+7 integration tests pass
cargo clippy --workspace --tests --features text -- -D warnings                # clean
cargo fmt --all                                                                # idempotent

# Microkernel std demonstrator (host).
(cd examples/rtos && cargo run --release --bin three-task-std)                 # heartbeat / led / sensor events visible

# Microkernel bare-metal flash size (STM32N6570-DK).
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform)                    # 149 KB .text (was 180)
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform,keleusma-verify)    # 169 KB .text (was 199)
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf \
    --release --bin three-task-n6 \
    --no-default-features --features stm32n6570dk-platform,keleusma-compile,keleusma-verify)  # 621 KB .text
```

## Notes

- The 622 lib-test figure is the runtime crate's lib test count: prior 614 from the V0.2 deferred-items pass, +1 for the re-enabled `ephemeral_bit_set_when_declared_text_return_never_produced` (Item 3), +1 for `vm_error_category_three_way_split` (Item 7), +5 for native-signature tests (Item 4), +2 for the new strict-hot-swap tests (Item 6), with 3 prior hot-swap tests rewritten in place under the new strict policy.
- The flash savings are concentrated in the embedded production modes (trust-load and precompile-plus-verify). The full-pipeline mode at 621 KB is essentially unchanged (was 614 KB) because the source compiler (lexer, parser, type checker, monomorphizer) dominates that image; pulling the lower-level dead-code wins in compose with that compile pipeline takes a larger refactor.
- The strict schema check on hot swap is a behaviour change for V0.2. Hosts that explicitly want incompatible-schema swaps now need to call `Vm::replace_module_unchecked`. The migration is mechanical and the new method name documents the safety opt-out.
- Native signatures use `-> ()` for unit return (the surface unit literal). `-> Unit` would parse as `TypeExpr::Named("Unit", ...)` rather than `TypeExpr::Unit(_)`, which produces `Type::Opaque("Unit")` rather than `Type::Unit` and would fail to unify against `()` arms in `match` expressions.

## Intended Next Step

Awaiting operator prompt. The agent-driven design-decision pass is complete except for Item 5 (deferred by user agreement) and Item 8 (closed as superseded).

1. **Operator action**: hardware verification on STM32N6570-DK. The N6 binary builds in all three feature modes; a probe-rs flash run would confirm that the new event codes (`EV_KERNEL_VM_ERROR` and siblings) print correctly through defmt RTT alongside the existing heartbeat / GPIO / sensor events.
2. **Operator action**: V0.2 release tag. With items 3, 4, 6, 7 closed and flash items B, C, I delivered, V0.2 is in releasable shape. Operator decides timing.
3. **Backlog**: Item 5 (CallIndirect flow analysis), B1 follow-up (remove `Type::Unknown` entirely now that native signatures are in place), and the remaining items previously deferred (target-scaled `Fixed` for sub-64-bit, smaller embassy feature trimming).
