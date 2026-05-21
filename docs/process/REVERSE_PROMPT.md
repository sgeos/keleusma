# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: `keleusma-bench` counter-to-cycle scale fix landed. The bench was reporting raw CNTVCT_EL0 counter ticks as if they were CPU cycles; on Apple Silicon at 24 MHz counter rate and 3.228 GHz CPU clock, one tick is approximately 134 CPU cycles, so the values were understated by roughly two orders of magnitude. The bench now scales counter ticks to CPU cycles using a documented assumed CPU clock that the operator can override per host via the `KELEUSMA_BENCH_CPU_HZ` environment variable. The committed dev-host fragment now shows realistic VM-dispatch costs (data movement 87 cycles, arithmetic 164 cycles, composite construction 338 cycles, function call 870 cycles). All 6 bench unit tests pass.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Operator flagged that VM opcodes reporting one pipelined cycle is implausible, and identified a scale mismatch between the profiling function and the WCET arithmetic | Diagnosis confirmed. On AArch64 the bench reads CNTVCT_EL0 which ticks at the architectural virtual counter frequency (24 MHz on Apple Silicon, confirmed by reading CNTFRQ_EL0 directly: 24,000,000 Hz). The CPU clock on the dev host (Apple M1 Max P-core nominal) is 3.228 GHz. One counter tick is therefore approximately 134 CPU cycles. The bench was reporting raw ticks as cycles, understating by approximately 2 orders of magnitude. Fix in three parts. (1) `CycleCounter` trait gained `cpu_cycles_per_count` and `frequency_hz` methods. `Rdtsc` returns 1.0 (invariant TSC counts CPU cycles directly). `CntvctEl0` reads CNTFRQ_EL0 at runtime and returns `assumed_cpu_hz / counter_hz`. `InstantFallback` returns `assumed_cpu_hz / 1_000_000_000`. (2) `benchmark_spec` multiplies the raw counter delta by `cpu_cycles_per_count` before dividing across pattern repetitions. (3) `DEFAULT_ASSUMED_CPU_HZ = 3.228e9` (M1 Max P-core nominal) with `KELEUSMA_BENCH_CPU_HZ` env var override. The emitted fragment header records the counter name, tick frequency, assumed CPU clock, and scale factor for transparency. Secondary fix: the nominal fallback for unmeasured categories (Yield, Call) was in nominal relative-weight units (1, 10) while measured categories were in CPU cycles (hundreds), producing an incoherent mixed-unit model. Fallback now scales the nominal value by the maximum measured-to-nominal ratio across measured categories (~87 on the dev host), keeping units consistent. Regenerated fragment committed at `keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs`. |

## Verification matrix

```bash
# Bench unit tests
cargo test --release -p keleusma-bench                                # 6 passed, 0 failed

# Counter frequency read independently from a probe matches CNTVCT_EL0
sysctl hw.tbfrequency                                                 # 24000000 Hz
sysctl machdep.cpu.brand_string                                       # Apple M1 Max

# Bench tool reports the scale factor and produces CPU-cycle values
./target/release/keleusma-bench --output /tmp/probe.rs                # scale 134.500;
                                                                      # per-op values
                                                                      # 40-870 cycles

# Generated fragment compiles as include! target and returns expected values
probe project including aarch64_apple_darwin.rs                       # Const 87, Dup 87,
                                                                      # CheckedAdd 164,
                                                                      # Div 140,
                                                                      # NewArray 338,
                                                                      # Call 870,
                                                                      # Yield 87
```

## Open concerns

1. **CPU clock assumption is per-host.** The `DEFAULT_ASSUMED_CPU_HZ` is set to Apple M1 Max P-core nominal (3.228 GHz). Operators on other Apple Silicon variants (M2, M3, M4) should override via `KELEUSMA_BENCH_CPU_HZ` before regenerating to obtain accurate CPU-cycle values for their host. Operators on x86_64 hosts where invariant TSC tracks the nominal frequency get correct values without override (`Rdtsc::cpu_cycles_per_count` returns 1.0).

2. **Yield and Call still use scaled nominal fallback.** The bench harness cannot exercise these opcodes in isolation. The fallback (~87× nominal, derived from the maximum measured-to-nominal ratio across measured categories on the dev host) is conservative for WCET but is an extrapolation rather than a direct measurement. Future work can add Stream-chunk and multi-chunk spec types to replace the fallback with real measurement.

3. **Frequency assumption does not track thermal throttling, P-core vs E-core differences, or frequency scaling.** The CPU-cycle values are calibrated against the assumed nominal clock; the actual instantaneous frequency during execution can vary. Operators who need wall-clock-time bounds should divide the reported cycle counts by the measured CPU clock under their workload's actual thermal conditions, not the nominal clock.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed |
| B21 | Value-side IFC negative labels via product lattice | Deferred |
| B22 | Sub-coroutines as callable ephemeral loops | Now load-bearing for V0.5.0; spec lives under `docs/architecture/SUB_COROUTINES.md` |
| (new candidate) | `keleusma-bench` multi-chunk and Stream-chunk spec types so `Call` and `Yield` measure in isolation | Deferred; acknowledged in `measured_cost_models/README.md` |

## Intended Next Step

The development host's measured cost model is committed in CPU-cycle units calibrated for the M1 Max P-core nominal frequency. The runtime defaults to `NOMINAL_COST_MODEL`; switching is an explicit host-side `CostModel` construction. The natural next step is one of:

- Generate cost-model fragments for other supported host architectures, with operators setting `KELEUSMA_BENCH_CPU_HZ` per host.
- Extend the bench harness with multi-chunk and Stream-chunk spec types so `Yield` and `Call` measure in isolation.
- Resume V0.3.0 self-hosting implementation (Lexer migration first per the incremental ordering).
- B15 follow-on: remove `Type::Unknown` entirely.
- Operator selection of a different directive.
