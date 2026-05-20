# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-20
**Status**: B16 step 11 lands the four post-audit follow-ups together. The WCMU verifier now threads a per-runtime `value_slot_bytes` through the chunk-WCMU computation, so narrow `GenericVm<W, A, F>` instances are no longer subject to the conservative 32-byte default. Three additional integration tests (Audio bundle on narrow runtime, two `view_bytes_zero_copy` regressions, two `Vm<i8>` smoke tests) round out the coverage matrix. B16 is closed across runtime, marshall, library bundles, verifier, knowledge-graph documentation, and end-to-end integration tests.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Verifier `value_slot_bytes` threading (item 1). | The internal `wcmu_region`, `wcmu_subregion`, and `compute_chunk_wcmu` functions now take a `value_slot_bytes: u32` parameter. New public variants `module_wcmu_with_value_slot_bytes`, `wcmu_stream_iteration_with_value_slot_bytes`, and `verify_resource_bounds_with_natives_and_value_slot_bytes` expose the parameter. The `verify_resource_bounds_with_cost_model` entry point previously ignored its `_cost_model` argument; it now honors `cost_model.value_slot_bytes` through the plumbing. `Vm::new_with_options` and `replace_module_inner` pass `core::mem::size_of::<GenericValue<W, F>>() as u32` so the WCMU bound matches the runtime's actual slot footprint. Existing public API (`module_wcmu`, `wcmu_stream_iteration`, `verify_resource_bounds_with_natives`, `verify_resource_bounds`) keeps the 32-byte default for back compat. |
| Audio bundle narrow-runtime test (item 6). | New `narrow_runtime_can_register_audio_library_via_lifted_impl` registers `stddsl::Audio` on `GenericVm<i16, u16, f64>` and confirms `audio::midi_to_freq(69) = 440.0_f64`. Belt-and-suspenders coverage of the lift code path. |
| `view_bytes_zero_copy` regression tests (item 7). | Two new tests. `narrow_runtime_view_bytes_zero_copy_runs_embedded_16_bytecode` runs a narrow runtime against precompiled narrow bytes through the zero-copy entry point. `narrow_runtime_view_bytes_zero_copy_rejects_wider_bytecode` confirms the load-time width check fires on the zero-copy path as well as `Vm::new`. |
| `Vm<i8>` end-to-end smoke tests (item 8). | Two new tests against `Target::embedded_8()` bytecode. `i8_narrow_runtime_runs_embedded_8_bytecode` confirms `100 + 27 = 127_i8` (boundary case). `i8_narrow_runtime_wraps_at_i8_boundary` confirms `100 + 28 = -128_i8` (wraps via `Word::wrapping_add`). |

## Comment on items 2, 3, 4, and 5

The user asked for commentary on the four standing items rather than action. Each item is presented with its current status and recommended course of action.

**Item 2: `truncate_int` workaround in `Op::Add` and `binary_arith`.** The workaround applies sign-extending truncation when a wide-Vm runs narrow bytecode (for example, default `Vm<i64>` running `Target::embedded_16()` bytecode). The path is load-bearing for the supported direction and intentionally retained. *Recommended action: leave in place.* The load-time width check (step 8) means truncate_int now only fires in the supported direction; the opposite direction is rejected at construction. Removing it would break the documented "wide Vm admits narrow bytecode" contract.

**Item 3: `Address` parameter `A` participates only in load-time width validation.** No opcode dispatches against `A::MAX` or otherwise consumes the address type at runtime. The `_phantom_a: PhantomData<A>` field encodes the present status. *Recommended action: leave as is.* Adding semantic weight requires a concrete opcode-level use case (a host-side `A::MAX` bound check in a pointer-shaped opcode). Without such a use case, the parameter contributes only at load-time validation and the additional design surface would be speculative. The parametric infrastructure is ready when the use case lands.

**Item 4: No 128-bit Word impl, no 16-bit Float impl.** Extensions would require widening the wire-format width encoding (the byte-10/11/12 fields are u8, currently encoding exponents 3-6; an i128 would need exponent 7) and adding `Word for i128`, `Float for f16`, `WideWord for i256` (or removing the wide-multiplication assumption for i128). *Recommended action: leave out of scope until a concrete consumer.* Adding these prematurely accumulates dead-code monomorphization weight on every host binary. A future host that genuinely needs i128 arithmetic or f16 precision can drive the extension with concrete requirements.

**Item 5: `RUNTIME_*_BITS_LOG2` global constants remain at 6.** These set the binary build's framing-level upper bound on bytecode widths. Reducing them on a binary that only ships narrow runtimes would tighten the framing-level rejection earlier (before the per-Vm check). *Recommended action: expose as a build-time configuration if a host requests it.* The change is mechanically simple (cargo feature or const-evaluated build flag) but has no concrete consumer yet. Hosts can already achieve the same effect by validating bytecode out-of-band before passing it to `Vm::load_bytes`.

## Verification matrix

```bash
cargo test -p keleusma --lib                                                    # 736 lib tests pass
cargo test --workspace --features text                                          # all workspace tests pass
cargo test -p keleusma --no-default-features --features compile,verify --lib    # 644 lib tests pass (floats off)
cargo test --test narrow_vm --features text                                     # 16 tests pass (was 11; +5)
cargo check --features shell                                                    # clean
cargo clippy --tests --all-targets --features text -- -D warnings               # clean
cargo fmt --all                                                                 # idempotent
cargo run --example narrow_runtime                                              # prints expected output

# Bare-metal STM32N6570-DK build, full pipeline.
(cd examples/rtos && cargo check --bin three-task-n6 \
    --target thumbv8m.main-none-eabihf --no-default-features \
    --features stm32n6570dk-platform)                                          # clean
```

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Deferred to V0.3 |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved (eleven steps complete) |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |

## Intended Next Step

Awaiting operator prompt. B16 is closed end-to-end. Standing items 2, 3, 4, 5 are documented design properties; no action is recommended without a concrete driver. The next development action belongs to the operator's selection from B13, B14, B15, or a new directive.
