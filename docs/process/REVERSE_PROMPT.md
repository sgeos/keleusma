# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-16
**Status**: V0.2.0 text refactor sequence complete on the `v0.2.0` branch. All four charter tasks landed.

## Completed in this session

Seven logical atomic commits on the `v0.2.0` branch.

1. `refactor(text): rename surface String type to Text` — surface keyword renamed; AST `PrimType::KString` renamed to `PrimType::Text`; parser, type checker, compiler, monomorphizer, target descriptor, verifier, and VM tests updated; all documentation and the bundled `string_ops` example use the new keyword.
2. `refactor(text): arena-resident Op::Add and remove Value::DynStr` — `Value::DynStr` variant removed; `Op::Add` text branch routes through `KString::alloc`; bundled `to_string`, `concat`, and `slice` natives now produce `Value::KStr` from the arena's top region; `register_utility_natives` is arena-aware by default; `register_utility_natives_with_ctx` retained as a deprecated alias.
3. `feat(cost-model): introduce OpCost::{Fixed, Dynamic} enum` — cost-model surface for runtime-dependent opcode cost. `CostModel::heap_alloc_cost` returns `OpCost::Dynamic` for `Op::Add` on text. The fixed-view `heap_alloc_bytes` accessor saturates dynamic costs to zero. Six new tests pin the contract.
4. `feat(text): add text cargo feature gating surface string support` — new `text` cargo feature, default off. Lexer rejects string literals with a clear feature-disabled message; parser does not recognise `Text`; the bundled string utility natives are still compiled but never reached by script-side code in the off configuration. The `keleusma-cli` crate enables the feature on its runtime dependency. CI gains a `test-no-text` job and the MSRV job covers both feature configurations.
5. `feat(vm): add Vm::new_with_options and OverflowPolicy knob` — new constructor returning `Result<(Self, Vec<VerifyWarning>), VmError>`. `OverflowPolicy::{Reject (default), Warn, Allow}` decides how the verifier responds to declared WCET or WCMU header fields that saturated to `u32::MAX`. The bare `Vm::new` continues to reject overflow because it wraps `new_with_options(VmOptions::default())`.
6. `feat(verify): add TextSize lattice for WCMU text-size tracking` — `TextSize::{Known(u32), Unbounded}` lattice with saturating addition, join, and projection. `op_cost_context` lifts a pair of lattice values into the `OpCostContext` consumed by `OpCost::Dynamic`. Eight tests pin the lattice behaviour, including the doubling-pattern saturation against the FAQ exponential-string-concat example.
7. `feat(verify): WCMU text-size tracking via abstract interpretation` — completes the V0.2.0 charter. `chunk_text_heap_alloc` walks each chunk's bytecode linearly with a `TextSize::{NotText, Known(u32), Unbounded}` lattice over an abstract operand stack and locals, evaluating the dynamic heap cost of every text-producing `Op::Add` and accumulating the result. `verify::compute_chunk_wcmu` adds this to each chunk's heap WCMU. The FAQ exponential-string-concat example expressed as a Stream block is now rejected at `Vm::new` with a `VerifyError`. The pass is conservative for text inside loops, branches, and from native calls.

## Verification matrix

```bash
cargo test --features text                                                  # 473 + ancillary, all pass
cargo test -p keleusma --no-default-features                                # 441 pass (32 text-only tests gated)
cargo clippy --workspace --tests --features text -- -D warnings             # clean
cargo clippy --workspace --tests --no-default-features -- -D warnings       # clean
cargo fmt --check                                                            # clean
cargo run --example string_ops                                              # prints "result: hello..."
cargo run --example wcmu_basic                                              # prints "yielded: Int(42)"
cargo build --workspace --no-default-features                               # clean
echo 'fn main() -> Text { "hello, world" }' | keleusma                      # CLI prints "hello, world"
```

## State of the four charter tasks

All four tasks complete on the branch.

1. **Surface `String` → `Text` rename + arena-resident migration + `OpCost::{Fixed, Dynamic}` enum** — done in commits 1, 2, 3.
2. **`text` cargo feature, default off** — done in commit 4.
3. **WCMU text-size tracking via abstract interpretation** — done in commits 6 and 7.
4. **Overflow policy knob (`Vm::new_with_options`, `OverflowPolicy::{Reject, Warn, Allow}`)** — done in commit 5.

## Status of items previously flagged for V0.2.x deferral

- **WCMU text-size tracking integration through `compute_chunk_wcmu`.** Resolved in commit 7. No longer deferred.
- **`BYTECODE_VERSION` bump.** Still deferred. The wire format is unchanged across V0.2.0 commits; bumping when the format changes is more useful than bumping speculatively now.

## Concerns

- The `register_utility_natives` API is now arena-aware by default. Hosts that pinned `keleusma = "0.1"` and migrate to `keleusma = "0.2"` will see `Value::DynStr` removed and their match arms on the result of bundled natives change from `Value::DynStr(_)` to `Value::KStr(_)`. The CHANGELOG documents the break; the migration is mechanical but unavoidable.
- The `text` feature default-off changes the surface available to embedding hosts that previously took `keleusma = "0.1"` defaults. Hosts that depended on script-side strings must add `features = ["text"]` to their `Cargo.toml`. The CHANGELOG and the FAQ entry "Enabling text support" document the migration.
- The WCMU text-size pass is conservative for text operations inside loops, branches, and against native return values. Atomic-total programs (those without a Stream block) are not subject to the per-iteration WCMU bound and fall back to runtime arena exhaustion via `VmError::OutOfArena`. The FAQ enumerates these cases under the "Limitations of the V0.2.0 text-size analysis" section.
- `examples/generic_match.rs` fails at compilation with a monomorphizer bug that affects pattern matching on generic enums (`enum pattern Maybe::Just does not match scrutinee type Maybe__i64`). The bug exists since the example landed in commit cdb1943 and is unrelated to V0.2.0 text work. Opaque-type implementation may surface the same code paths; flag for investigation when that work begins.

## Intended Next Step

Awaiting operator prompt. The branch is ready for the next direction the operator chooses:

1. **Opaque type implementation.** The runtime currently supports an `Opaque(String)` placeholder in the type checker. A full implementation would let host applications register Rust types (e.g. `String`) as opaque types in scripts. This pairs naturally with the "register native Rust functions for text" pattern documented in the FAQ: a `Text` surface type for cheap arena-resident text plus host-registered `String` opaque type for heavy Rust text manipulation.
2. **Rust `String` as opaque type for registered functions example.** A worked example demonstrating the pattern above: register `String` as an opaque type, register native functions that operate on it, and have a script orchestrate the work.
3. **Cut a V0.2.0 release candidate** against the branch and tag it for crates.io publication.
