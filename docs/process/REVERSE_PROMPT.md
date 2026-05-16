# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-16
**Status**: V0.2.0 text refactor sequence committed on the `v0.2.0` branch.

## Completed in this session

The V0.2.0 text refactor proceeded through six logical atomic commits on the `v0.2.0` branch.

1. `refactor(text): rename surface String type to Text` — surface keyword renamed; AST `PrimType::KString` renamed to `PrimType::Text`; parser, type checker, compiler, monomorphizer, target descriptor, verifier, and VM tests updated; all documentation and the bundled `string_ops` example use the new keyword.
2. `refactor(text): arena-resident Op::Add and remove Value::DynStr` — `Value::DynStr` variant removed; `Op::Add` text branch routes through `KString::alloc`; bundled `to_string`, `concat`, and `slice` natives now produce `Value::KStr` from the arena's top region; `register_utility_natives` is arena-aware by default; `register_utility_natives_with_ctx` retained as a deprecated alias.
3. `feat(cost-model): introduce OpCost::{Fixed, Dynamic} enum` — cost-model surface for runtime-dependent opcode cost. `CostModel::heap_alloc_cost` returns `OpCost::Dynamic` for `Op::Add` on text. The fixed-view `heap_alloc_bytes` accessor saturates dynamic costs to zero. Six new tests pin the contract.
4. `feat(text): add text cargo feature gating surface string support` — new `text` cargo feature, default off. Lexer rejects string literals with a clear feature-disabled message; parser does not recognise `Text`; the bundled string utility natives are still compiled but never reached by script-side code in the off configuration. The `keleusma-cli` crate enables the feature on its runtime dependency. CI gains a `test-no-text` job and the MSRV job covers both feature configurations.
5. `feat(vm): add Vm::new_with_options and OverflowPolicy knob` — new constructor returning `Result<(Self, Vec<VerifyWarning>), VmError>`. `OverflowPolicy::{Reject (default), Warn, Allow}` decides how the verifier responds to declared WCET or WCMU header fields that saturated to `u32::MAX`. The bare `Vm::new` continues to reject overflow because it wraps `new_with_options(VmOptions::default())`.
6. `feat(verify): add TextSize lattice for WCMU text-size tracking` — `TextSize::{Known(u32), Unbounded}` lattice with saturating addition, join, and projection. `op_cost_context` lifts a pair of lattice values into the `OpCostContext` consumed by `OpCost::Dynamic`. Integration with `verify::compute_chunk_wcmu` is staged for V0.2.x. Eight tests pin the lattice behaviour, including the doubling-pattern saturation against the FAQ exponential-string-concat example.

## Verification matrix

```bash
cargo test --workspace                                                      # 465 + ancillary, all pass
cargo test -p keleusma --no-default-features                                # 434 pass (31 text-only tests gated)
cargo clippy --workspace --tests --features text -- -D warnings             # clean
cargo clippy --workspace --tests --no-default-features -- -D warnings       # clean
cargo fmt --check                                                            # clean
cargo run --example string_ops                                              # prints "result: hello..."
cargo build --workspace --no-default-features                               # clean
```

## Outstanding work for V0.2.0 cycle

The four tasks in the V0.2.0 charter are addressed. Two items are explicitly deferred to V0.2.x with the design captured in source:

1. **Static WCMU text-size tracking through `compute_chunk_wcmu`.** The `TextSize` lattice and `OpCost::Dynamic` cost surface are in place. The integration commit will populate an abstract per-slot lattice during the existing `wcmu_region` walk, evaluate `OpCost::Dynamic` against the resulting context, and sum the dynamic heap cost into `McuResult::heap_total`. The work is mostly mechanical; the design hinge is how to surface joins and loop iteration counts cleanly without duplicating `wcmu_region`'s control-flow logic.
2. **BYTECODE_VERSION bump.** The wire format is unchanged, but the host-visible runtime semantics moved meaningfully. A version bump for V0.2.0 may still be appropriate; deferred because the cost-model and overflow-policy surface may evolve further in V0.2.x and a single bump that captures the V0.2.x deltas in one move is cheaper than two.

## Concerns

- The `register_utility_natives` API is now arena-aware by default. Hosts that pinned `keleusma = "0.1"` and migrate to `keleusma = "0.2"` will see `Value::DynStr` removed and their match arms on the result of bundled natives change from `Value::DynStr(_)` to `Value::KStr(_)`. The CHANGELOG documents the break; the migration is mechanical but unavoidable.
- The `text` feature default-off changes the surface available to embedding hosts that previously took `keleusma = "0.1"` defaults. Hosts that depended on script-side strings must add `features = ["text"]` to their `Cargo.toml`. The CHANGELOG and the new FAQ entry "Enabling text support" document the migration.
- The overflow-policy rewrite of the declared header field to `0` under `Warn` and `Allow` is the only way to bypass the load-time u32::MAX check in `Module::access_bytes` without restructuring the loader. The policy preserves the original signal through the warning vector, so hosts still observe the overflow. A future refinement could push the policy into the loader and avoid the rewrite, but the present arrangement is the minimal change.

## Intended Next Step

Awaiting operator prompt. Likely directions:

1. Land the WCMU text-size integration in `wcmu_region`, removing the gap noted above and closing the FAQ caveat that text growth is not statically tracked.
2. Cut a V0.2.0 release candidate against the branch and tag it for crates.io publication.
3. Continue to address any open reviewer comments not surfaced in the current FAQ entries.
