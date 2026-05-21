# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: `keleusma-bench` gains a `--cpu-hz <Hz>` CLI flag. The flag takes precedence over `KELEUSMA_BENCH_CPU_HZ` and works on both the host-bench path (scaling counter ticks to CPU cycles) and the `--from-log` path (overriding the `BENCH_DONE`-reported value in the emitted fragment header).

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Add a command-line parameter to specify CPU speed | New `--cpu-hz <Hz>` flag in `keleusma-bench` CLI. Validates the value is a positive finite f64, then sets the `KELEUSMA_BENCH_CPU_HZ` environment variable for the rest of the process (via `unsafe { env::set_var(...) }` because the 2024 edition marks `set_var` unsafe; the bench main is single-threaded at this point so the unsafe block is justified). Counter implementations read `assumed_cpu_hz()` lazily inside `cpu_cycles_per_count`, so the override applies uniformly to host-bench measurements without further plumbing. In `--from-log` mode the override is threaded through `run_from_log` and replaces the `BENCH_DONE`-reported `cpu_hz` value in the emitted fragment header; this matters for embedded captures where DWT_CYCCNT cycle counts are correct regardless of CPU clock but the documentation should reflect the operator's actual hardware. Banner now reports the source of the assumption (`--cpu-hz override`, `KELEUSMA_BENCH_CPU_HZ env var`, or `DEFAULT_ASSUMED_CPU_HZ`). READMEs updated. End-to-end test confirms both paths honor the flag: host-bench scale changes from 134.5 to 125.0 when overriding 3.228 GHz to 3.0 GHz; from-log fragment header shows 400 MHz instead of 800 MHz when overriding the N6 log. |

## Verification matrix

```bash
# Host bench with --cpu-hz override
./target/release/keleusma-bench --cpu-hz 3000000000
# Output: assumed CPU clock: 3000000000 Hz (3.000 GHz) (source: --cpu-hz override)
#         scale (CPU cycles per counter tick): 125.000

# from-log with --cpu-hz override
./target/release/keleusma-bench --from-log /tmp/bench_n6.log \
    --cpu-hz 400000000 --output /tmp/test_override.rs
# Output: cpu_hz: 400000000 Hz (source: --cpu-hz override)
# Fragment header: Assumed CPU clock: 400000000 Hz (0.400 GHz)

# Precedence: --cpu-hz wins over env var
KELEUSMA_BENCH_CPU_HZ=5000000000 ./target/release/keleusma-bench --cpu-hz 4000000000
# Output: assumed CPU clock: 4000000000 Hz (4.000 GHz) (source: --cpu-hz override)

# All 6 unit tests pass
cargo test --release -p keleusma-bench
```

## Open concerns

1. **The embedded `bench_n6.rs` still hardcodes 800 MHz** in `N6_CPU_HZ`. The override applies at the `--from-log` parse step, so the operator can correct the documentation after capture, but the embedded binary itself prints 800 MHz in its boot banner and `BENCH_DONE` marker. A future revision could read the constant from `option_env!("KELEUSMA_BENCH_CPU_HZ")` at compile time, or read the actual CPU clock from the RCC peripheral at runtime. The latter is the proper fix and is recorded as a backlog candidate.

2. **`env::set_var` is unsafe in the 2024 edition** because of thread-safety concerns. The bench main is single-threaded at the point of the call, well before any counter or VM construction, so the unsafe block is justified and documented inline. A future refactor could thread an explicit `cpu_hz` value through the counter constructors instead, removing the env-var trampoline.

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
| (candidate) | Embedded bench reads `KELEUSMA_BENCH_CPU_HZ` via `option_env!` at compile time | Deferred |
| (candidate) | Thread explicit `cpu_hz` through counter constructors so `env::set_var` is no longer required | Deferred |

## Intended Next Step

The bench tooling is now fully configurable for arbitrary host CPU clocks. The natural next step is one of:

- Generate cost-model fragments for additional host architectures.
- Resume V0.3.0 self-hosting implementation (Lexer migration first per the incremental ordering).
- Read the N6 CPU clock from RCC at runtime to remove the hardcoded assumption.
- B15 follow-on: remove `Type::Unknown` entirely.
- Operator selection of a different directive.
