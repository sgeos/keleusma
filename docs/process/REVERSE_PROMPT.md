# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: WCET table generated on real STM32N6570-DK hardware. The `bench_n6` binary flashed to the connected board via probe-rs, enabled DWT_CYCCNT, ran all 17 OPCODE_SPECS, and emitted measurements via defmt RTT. Captured log parsed through `keleusma-bench --from-log`; fragment committed at `keleusma-bench/measured_cost_models/thumbv8m_main_none_eabihf.rs`. The bench had to be re-configured for embedded memory budgets (200 repetitions and 8 KB arena rather than 1000 repetitions and 64 KB arena) after the first run triggered a heap fragmentation panic at the 6th spec.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Generate the N6-DK WCET table by running the bench on hardware | Done. The N6 was reachable via probe-rs (`probe-rs run --chip STM32N657`). First run with the original `BenchConfig::embedded_default` (1,000 reps, 64 KB arena) reached spec 6 then panicked: the linked-list allocator could not satisfy a fresh 64 KB arena allocation after five iterations of allocate-then-free cycles. Diagnosis: the bench's per-spec memory footprint (~100 KB) was too large relative to the 128 KB heap, and the `ZeroSizeOk` allocator wrapper does not defragment between iterations. Fix: added an `arena_capacity` field to `BenchConfig` and lowered both embedded defaults to 200 repetitions and 8 KB arena. The bench's runtime working set is tiny (patterns leave the operand stack near empty); 8 KB is comfortable. At DWT_CYCCNT's single-CPU-cycle resolution and 800 MHz clock, 200 repetitions of patterns costing 3,000 to 13,000 cycles each produce hundreds of thousands of cycles per measurement pass with ample resolution. Second run completed cleanly: all 17 measurements in 8.14 seconds wall time. The captured log was parsed through `keleusma-bench --from-log` into the committed fragment. |

## N6 measured per-category cost

| Category | M1 Max (host) | N6 (Cortex-M55) | Ratio |
|---|---|---|---|
| Data movement | 87 | 6070 | 70x |
| Control marker (scaled nominal) | 87 | 6070 | 70x |
| Arithmetic, comparison, bitwise, casts | 164 | 10079 | 61x |
| Division, field lookup, type checks | 140 | 9164 | 65x |
| Composite construction | 338 | 13540 | 40x |
| Function call (scaled nominal) | 870 | 60700 | 70x |

The ratios are consistent with the architectural difference between an out-of-order superscalar with deep caches running at 3.228 GHz (M1 Max) and an in-order Cortex-M55 running from flash at 800 MHz. The N6's per-CPU-cycle cost is dominated by VM dispatch (no branch prediction to speak of, simpler instruction-cache hierarchy); the M1 Max's measured cost is dominated by data-cache and branch-prediction effects that have already absorbed most of the dispatch overhead.

## Verification matrix

```bash
# Cross-compile and flash via probe-rs
cd examples/rtos
cargo run --release --bin bench_n6 \
    --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform \
    > /tmp/bench_n6.log 2>&1                                       # ran for 8.14 s,
                                                                   # all 17 BENCH lines
                                                                   # plus BENCH_DONE
                                                                   # captured

# Generate fragment from captured log
cargo run --release -p keleusma-bench -- \
    --from-log /tmp/bench_n6.log \
    --output keleusma-bench/measured_cost_models/thumbv8m_main_none_eabihf.rs
                                                                   # 17 measurements
                                                                   # parsed, fragment
                                                                   # written

# Fragment compiles as include! target
probe project including thumbv8m_main_none_eabihf.rs               # builds clean;
                                                                   # per-category
                                                                   # values 6070,
                                                                   # 10079, 9164,
                                                                   # 13540, 60700,
                                                                   # 6070
```

## Open concerns

1. **`Yield` and `Call` still use scaled nominal fallback on the N6.** Same limitation as the dev-host fragment: the bench harness cannot exercise these opcodes in isolation (Yield needs a Stream chunk; Call needs a multi-chunk module). The fallback at the N6 scale factor (~70x) yields a Call cost of 60700 cycles, which is consistent with an embedded VM dispatch into a callee, but is an extrapolation not a measurement. Future work would add multi-chunk and Stream-chunk spec types.

2. **800 MHz CPU clock is hardcoded in `bench_n6.rs`.** The N6's actual instantaneous clock depends on the bootloader's PLL configuration and any runtime power-management decisions. The current value matches the documented nominal P-core clock. If the actual clock differs, cycle counts are off proportionally. A future revision could read the clock from the RCC peripheral at runtime.

3. **Linked-list allocator fragmentation forced a smaller embedded config.** The bench now uses 200 repetitions and an 8 KB arena per spec. Resolution remains good because DWT_CYCCNT counts CPU cycles directly, but the smaller working set means each measurement covers fewer total cycles than the host equivalent. If a future use case demands tighter measurements, switching to a slab or bump allocator on the N6 would allow restoring larger repetition counts.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed |
| B21 | Value-side IFC negative labels via product lattice | Deferred |
| B22 | Sub-coroutines as callable ephemeral loops | Now load-bearing for V0.5.0 |
| (candidate) | Multi-chunk and Stream-chunk bench specs to remove `Yield` and `Call` nominal fallback | Deferred |
| (candidate) | Read N6 CPU clock from RCC at runtime instead of hardcoding 800 MHz | Deferred |
| (candidate) | Slab or bump allocator on the N6 to restore larger bench repetition counts | Deferred |

## Intended Next Step

Both supported host architectures now ship measured cost-model fragments in `keleusma-bench/measured_cost_models/`. The natural next step is one of:

- Generate cost-model fragments for additional host architectures (x86_64-unknown-linux-gnu would be the most common third target).
- Resume V0.3.0 self-hosting implementation (Lexer migration first per the incremental ordering).
- B15 follow-on: remove `Type::Unknown` entirely.
- Operator selection of a different directive.
