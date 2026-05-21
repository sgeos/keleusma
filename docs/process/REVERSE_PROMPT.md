# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: `keleusma-bench` repaired and first measured cost-model fragment generated for the development host (aarch64-apple-darwin). The bench tool had two bugs that prevented useful WCET tables: arithmetic specs used Int operands on opcodes V0.2.0 Consolidation B had narrowed to non-Int types, and the cost-emit pipeline divided per-pattern measurements by `ops_per_pattern` before rounding, collapsing every category to a single cycle on counters that run below CPU clock speed. Both are fixed. The generated fragment lives at `keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs` and compiles cleanly as an `include!` target.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Generate WCET tables for the development architecture; resolve the recalled bug | Found two bugs. (1) Stale `OPCODE_SPECS`: `Op::Add` / `Sub` / `Mul` / `Neg` patterns used `Int` constants, which V0.2.0 Consolidation B narrowed to `Byte` / `Fixed` / `Float` operand types only; integer arithmetic now flows through `Op::CheckedAdd` / `CheckedSub` / `CheckedMul` / `CheckedNeg`. Updated the four specs to the checked opcodes plus `PopN(3)` (the checked variants push low, high, flag). Removed the retired `Op::MakeClosure` spec. (2) Methodology: the cost-emit pipeline divided `cycles_per_pattern` by `ops_per_pattern` and ceiled, which collapsed every category to 1 cycle because the AArch64 CNTVCT_EL0 counter runs at 24 MHz on Apple Silicon (far below CPU clock) and per-op fractional values fall below one tick. Switched to `ceil(cycles_per_pattern)` directly, preserving relative ordering between categories at the cost of overstating per-op absolute cost (conservative for WCET). Diagnostic improvement: warmup-pass errors now surface to stderr. Unmeasured categories (`Yield`, `Call`) fall back to `nominal_op_cycles` rather than to misleading placeholder push-and-pop measurements. Emit logic now covers every V0.2.0 ISA opcode across the six categories. Final measured ratios versus nominal for the dev host: data movement 1 versus 1, control marker 1 versus 1, arithmetic 2 versus 2, division 2 versus 3, composite construction 3 versus 5, function call 10 versus 10 (nominal fallback). Generated fragment committed at `keleusma-bench/measured_cost_models/aarch64_apple_darwin.rs`. New `measured_cost_models/README.md` documents the fragment, the counter, the calibration caveats, and the regeneration procedure. Main `keleusma-bench/README.md` updated with methodology notes and a pre-generated fragments cross-reference. |

## Verification matrix

```bash
# Bench unit tests
cargo test --release -p keleusma-bench                                # 6 passed, 0 failed

# Bench tool runs to completion and produces a populated fragment
./target/release/keleusma-bench --output /tmp/probe.rs                # 17 specs, all non-zero

# Generated fragment compiles as include! target in a host crate
(probe project including aarch64_apple_darwin.rs)                     # builds clean; per-op
                                                                      # lookups return 1, 2, 3,
                                                                      # and 10 for the right
                                                                      # categories

# Stack-balance invariant on the new PopN(3) patterns
test opcode_specs_have_balanced_stack_patterns                        # ok
```

## Open concerns

Two known limitations of the bench methodology, documented in the README and not blocking the immediate goal. Both are candidates for follow-on work.

1. **`Yield` and `Call` cannot be measured in isolation by the current harness.** `Yield` is rejected by Func chunks; `Call` requires a multi-chunk module the harness does not construct. Both categories fall back to nominal values for now. A future revision can add Stream-chunk and multi-chunk spec types to remove the fallback.

2. **CNTVCT_EL0 counter resolution on Apple Silicon.** The architectural virtual counter runs at 24 MHz (one tick is approximately 125 CPU cycles). Relative ordering between opcodes is preserved at counter resolution, but absolute wall-clock conversion needs multiplying by the host's CNTFRQ_EL0 reciprocal and the host's clock frequency. The generated fragment header records the counter so downstream consumers can apply the calibration.

The committed fragment is not a certified hard-real-time bound. It is a best-effort relative-ordering estimate suitable for soft-real-time analysis and order-of-magnitude WCET. Certified bounds require either the bundled `NOMINAL_COST_MODEL` on a verified-cost VM, or a third-party static-analysis toolchain (aiT, Bound-T) on the native lowering.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred |
| B14 | CallIndirect flow analysis for non-recursive closures | Closed as not-applicable (closures retired in Phase 4) |
| B15 | Remove `Type::Unknown` entirely | Foundation in place; refactor pending |
| B16 | Parametric `Vm<W, A, F>` for sub-64-bit native runtimes | Resolved |
| B17 | Embassy feature trimming | Resolved as not actionable |
| B18 | Big-number arithmetic worked example | Resolved |
| B20 | V0.2.0 ISA and wire format implementation | Closed (Phases 1, 2, 3, 3.5, Consolidation B, 4, 5, 6, 7a, 7b, 7c, 8 complete) |
| B21 | Value-side IFC negative labels via product lattice | Deferred (forward-looking; admitted when forcing case appears) |
| B22 | Sub-coroutines as callable ephemeral loops | Now load-bearing for V0.5.0; specification under `docs/architecture/SUB_COROUTINES.md` |
| (new candidate) | `keleusma-bench` multi-chunk and Stream-chunk spec types so `Call` and `Yield` measure in isolation | Not yet filed; deferral acknowledged in `measured_cost_models/README.md` |

## Intended Next Step

The development host's measured cost model is committed and ready for use by hosts that want to opt out of the bundled `NOMINAL_COST_MODEL`. The runtime continues to default to nominal; switching to measured is an explicit host-side `CostModel` construction. The natural next step is one of:

- Generate cost-model fragments for other supported host architectures (x86_64-unknown-linux-gnu being the most likely second target).
- Extend the bench harness with multi-chunk and Stream-chunk spec types so `Yield` and `Call` measure in isolation, removing the nominal-fallback for those categories.
- Resume the V0.3.0 self-hosting implementation. The Lexer migration is the recommended first step per the incremental migration ordering.
- B15 follow-on: remove `Type::Unknown` entirely.
- Operator selection of a different directive.
