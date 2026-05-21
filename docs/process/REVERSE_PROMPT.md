# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: Measured cost-model fragments are now consumed by code, not just generated. Three closures land in one round: a cookbook recipe in `docs/guide/COOKBOOK.md`, a standalone example at `examples/measured_wcet.rs`, and per-task WCET reporting at boot in both RTOS demonstrator binaries (`three-task-std` on host, `three-task-n6` on the N6 hardware). The stale `GRAMMAR.md` note about AArch64 calibration was rewritten to reflect the resolved state. The std demonstrator on the dev host shows realistic 80-85x measured-vs-nominal ratios per task, consistent with the M1 Max measured cost model.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Option A: documentation patch | Three doc updates. (1) New cookbook section "Calibrated WCET with a measured cost model" walks through the three-step wiring: obtain a fragment via `keleusma-bench`, include it under `cfg(target_arch = ...)`, pass the model to a `_with_cost_model` API variant. (2) Stale `docs/spec/GRAMMAR.md` § 17.2 note about "AArch64 produces degenerate one-cycle output" was rewritten to "resolved" with the current scaling described inline. (3) New `Calibrated WCET in CPU cycles` subsection in `docs/guide/EMBEDDING.md` cross-references the cookbook recipe, the standalone example, and the measured-model fragments. |
| Option B: standalone example | New `examples/measured_wcet.rs` (registered in workspace `Cargo.toml`) compiles a small Stream-classified Keleusma program, computes per-iteration WCET under both `NOMINAL_COST_MODEL` and the included `MEASURED_COST_MODEL`, and prints the comparison. On the dev host: `NOMINAL 25 cycles, MEASURED 2145 cycles, ratio 85.80x`. |
| Headline example wiring (rtos) | New `examples/rtos/src/cost_model.rs` exposes a target-dispatched `MEASURED_COST_MODEL`: M1 Max fragment for `aarch64-apple-darwin`, Cortex-M55 fragment for `thumbv8m.main-none-eabihf`, `NOMINAL_COST_MODEL` fallback elsewhere. The module gates on `feature = "keleusma-verify"` because the report logic depends on `keleusma::verify::wcet_stream_iteration_with_cost_model`. Two helper functions: `report_measured_wcet(bytecode)` for the precompiled path used by the N6 binary, and `report_measured_wcet_from_source(source)` for the source-compile path used by the std demonstrator. Both demonstrator binaries call into the helper under their respective feature gates and emit a per-task WCET report at boot. `setup::PRELUDE` promoted from private to `pub` so the binaries can prepend it when compiling task sources off-line. |

## Verification matrix

```bash
# Workspace build clean
cargo build --release --workspace

# Bench unit tests
cargo test --release -p keleusma-bench                                # 6 passed

# Standalone example
cargo run --release --example measured_wcet                           # prints
                                                                       # NOMINAL 25 cycles
                                                                       # MEASURED 2145 cycles
                                                                       # ratio 85.80x

# RTOS std demonstrator (with keleusma-verify default)
cargo run --release --manifest-path examples/rtos/Cargo.toml \
    --bin three-task-std                                              # prints WCET report
                                                                       # per task at boot:
                                                                       # led 6214/74, sensor
                                                                       # 5528/66, heartbeat
                                                                       # 5006/60,
                                                                       # event_listener
                                                                       # 3102/38, faulty
                                                                       # 5624/70

# RTOS N6 demonstrator (build with keleusma-verify)
cargo build --release --manifest-path examples/rtos/Cargo.toml \
    --bin three-task-n6 --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform,keleusma-verify
                                                                       # clean

# RTOS N6 demonstrator without verify still builds
cargo build --release --manifest-path examples/rtos/Cargo.toml \
    --bin three-task-n6 --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform           # clean (cost_model
                                                                       # module is gated
                                                                       # out)
```

## Open concerns

1. **N6 hardware run with the WCET report is not captured yet.** The build with `--features stm32n6570dk-platform,keleusma-verify` is clean, but the runtime defmt output has not been captured against the connected board in this session. The defmt log format matches the std demonstrator's println; per-task lines would appear after the existing "Tasks:" banner.

2. **Cost-model selection is per-arch, not per-host-CPU-model.** The `cfg(target_arch = "aarch64")` arm uses the M1 Max fragment for all aarch64-apple-darwin builds. Operators on different Apple Silicon variants (M2, M3, M4) consuming the rtos example get dev-host estimates rather than calibrated values. The committed fragment header records the CPU clock assumption (3.228 GHz); operators who care regenerate the fragment per host or use the `--cpu-hz` override at bench time.

3. **The std demonstrator compiles task sources at boot to report WCET.** Compilation overhead is in milliseconds per task, paid once at startup. The N6 demonstrator uses the precompiled-bytecode path so the runtime image is unchanged.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed |
| B21 | Value-side IFC negative labels via product lattice | Deferred |
| B22 | Sub-coroutines as callable ephemeral loops | Now load-bearing for V0.5.0 |
| (candidate) | Multi-chunk and Stream-chunk bench specs to remove `Yield` and `Call` nominal fallback | Deferred |
| (candidate) | Read N6 CPU clock from RCC at runtime | Deferred |
| (candidate) | Slab or bump allocator on the N6 to restore larger bench repetition counts | Deferred |
| (candidate) | Capture N6 hardware run with WCET report appearing in defmt output | Deferred |

## Intended Next Step

The measured cost-model artefacts are now integrated end-to-end: generation, documentation, standalone example, and headline-example wiring. The natural next step is one of:

- Capture N6 hardware run of the three-task demonstrator with the WCET boot report appearing in defmt RTT output.
- Resume V0.3.0 self-hosting implementation (Lexer migration first per the incremental ordering).
- B15 follow-on: remove `Type::Unknown` entirely.
- Generate cost-model fragments for x86_64-unknown-linux-gnu or other host architectures.
- Operator selection of a different directive.
