# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: Embedded WCET infrastructure for the STM32N6570-DK lands. `keleusma-bench` lib is now no_std + alloc-compatible behind a `std` feature; new `BenchConfig` parametrises chunk size so embedded targets stay within RAM; new `DwtCycCnt` counter for Cortex-M reads DWT_CYCCNT at CPU clock; new `examples/rtos/src/bin/bench_n6.rs` boots embassy, enables DWT, runs the bench suite, and emits each measurement through defmt RTT. The host-side `keleusma-bench --from-log` parser converts a captured defmt log into a target fragment. Cross-compile to thumbv8m.main-none-eabihf is clean; the run-on-hardware step is deferred to the next session when the N6 is connected.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Generate WCET/WCMU tables for the N6-DK (Path A: build infrastructure now, run on hardware in follow-on) | Done. Three pieces of infrastructure. (1) `keleusma-bench` lib refactored to no_std + alloc. The `std` cargo feature gates the CLI bin and the `KELEUSMA_BENCH_CPU_HZ` env-var override; the measurement primitives are portable. `libm::ceil` replaces `f64::ceil` so the math works in both targets. (2) New `BenchConfig` struct with `embedded_default()` returning 1,000 pattern repetitions to keep the constructed chunk inside the N6's 384 KB RAM budget. New `benchmark_spec_with_config` and `measure_one_with_config` consume the config. (3) New `DwtCycCnt` counter in `keleusma-bench/src/counter.rs` reads DWT_CYCCNT via volatile MMIO at 0xE000_1004; `cpu_cycles_per_count` returns 1.0 because DWT_CYCCNT ticks at CPU clock. New `bench_n6.rs` binary in `examples/rtos/src/bin/` boots embassy, enables DWT via direct register pokes on DEMCR/DWT_CTRL, runs the spec suite, emits each `Measurement` as a `BENCH idx=I/N name=N bits=B per_op=C` defmt line, signals completion with `BENCH_DONE cpu_hz=H counter_hz=H`. Cross-compiles to thumbv8m.main-none-eabihf cleanly: text 128 KB, bss 132 KB. New host-side `keleusma-bench --from-log <path>` parses the captured defmt log and emits a fragment with the same shape as the host-bench path. Verified end-to-end against a synthetic 17-line log. Documentation in `keleusma-bench/README.md` and `keleusma-bench/measured_cost_models/README.md` covers the embedded path and the N6 capture workflow. |
| WCMU for the N6 | Documented as a parametric-Vm-width choice rather than a measurement workflow. The runtime computes WCMU at compile time from `value_slot_bytes` and per-op stack/heap effects. For the default 64-bit runtime (which `examples/rtos` currently uses) `value_slot_bytes = 32`, identical to host. Operators that select a narrower parametric Vm for the N6 (sensible for a 32-bit Cortex-M55) get a smaller `value_slot_bytes` per the parametric type's choice. There is no per-host WCMU measurement to capture; the bench measures WCET only. |

## Verification matrix

```bash
# Host build and tests
cargo build --release -p keleusma-bench                                # clean
cargo test --release -p keleusma-bench                                 # 6 passed

# no_std build of bench lib alone
cargo build --release -p keleusma-bench --no-default-features          # clean

# Cross-compile bench_n6 for the N6 target
cargo build --release --manifest-path examples/rtos/Cargo.toml \
    --bin bench_n6 --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform             # clean
# ELF: text 128 KB, bss 132 KB, fits in 384 KB RAM

# Host bench still produces the expected aarch64-darwin fragment
./target/release/keleusma-bench --output /tmp/probe.rs                 # CPU-cycle values
                                                                       # consistent with
                                                                       # prior runs

# --from-log parser against a synthetic 17-line log
./target/release/keleusma-bench --from-log /tmp/synthetic.log \
    --output /tmp/synthetic_fragment.rs                                # 17 parsed,
                                                                       # fragment shape
                                                                       # matches host-bench
                                                                       # output, scale 1.000,
                                                                       # function-call falls
                                                                       # back to scaled
                                                                       # nominal (870 cycles)
```

## Open concerns

1. **Hardware run on the N6-DK is deferred.** The infrastructure is in place and the binary cross-compiles cleanly. The next session step is: connect the N6-DK via probe-rs, run `cargo run --release --manifest-path examples/rtos/Cargo.toml --bin bench_n6 --target thumbv8m.main-none-eabihf --no-default-features --features stm32n6570dk-platform 2>&1 | tee /tmp/bench_n6.log`, wait for the `BENCH_DONE` marker, then run `cargo run --release -p keleusma-bench -- --from-log /tmp/bench_n6.log --output keleusma-bench/measured_cost_models/thumbv8m_main_none_eabihf.rs`. The committed fragment then ships in the same shape as the dev-host fragment.

2. **N6 CPU clock assumption is hardcoded at 800 MHz.** The `bench_n6.rs` binary constructs the counter with `DwtCycCnt::new(800_000_000)` matching the N6's nominal CPU clock after the bootloader configures the PLL. If the actual clock differs (the operator selects a different power profile, or thermal conditions alter the boost behaviour), the cycle counts are off proportionally. The current value matches the documented nominal; a future revision could read the clock from the RCC peripheral at runtime to remove the assumption.

3. **Embedded `BenchConfig` uses 1,000 repetitions versus host's 100,000.** Resolution at the N6's CPU-clock counter is fine because DWT_CYCCNT ticks at 800 MHz, so 1,000 repetitions of a few-cycle op still produce thousands of counter ticks. The chunk-size constraint is the binding limit (RAM, not resolution). Operators on devices with more RAM can lift the repetition count by passing a custom `BenchConfig`.

4. **The `Yield` and `Call` opcodes still fall back to scaled nominal on both host and N6.** This is unchanged from the prior session and is documented in `measured_cost_models/README.md`. A future bench harness with multi-chunk and Stream-chunk spec types can replace the fallback with measurement; for both host and N6 the fallback is consistent.

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
| (candidate) | `keleusma-bench` multi-chunk and Stream-chunk specs to remove `Yield` and `Call` nominal fallback | Deferred |
| (candidate) | Read N6 CPU clock from RCC at runtime instead of hardcoding 800 MHz | Deferred |

## Intended Next Step

The infrastructure for embedded WCET measurement is in place and cross-compiles cleanly. The natural next step is to run the bench on a connected N6-DK and commit the resulting `thumbv8m_main_none_eabihf.rs` fragment. The procedure is documented in `keleusma-bench/measured_cost_models/README.md` under "N6 capture workflow."

Alternatives:

- Resume V0.3.0 self-hosting implementation (Lexer migration first per the incremental ordering).
- B15 follow-on: remove `Type::Unknown` entirely.
- Generate cost-model fragments for other supported host architectures (x86_64-unknown-linux-gnu) before tackling N6 hardware run.
- Operator selection of a different directive.
