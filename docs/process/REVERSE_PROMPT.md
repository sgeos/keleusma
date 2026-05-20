# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: STM32N6570-DK hardware verification passed under the V0.2 image after the AXISRAM2 rebalance. LED toggling observed on the board. Six new backlog items recorded as B13 through B18, capturing the remaining V0.2 design-pass deferred work and the newly-opened items from the pattern-arm refactor.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Memory rebalance to fit the full-pipeline image. | The pattern-matched checked-arithmetic refactor (commit `68e7cb5`) expanded the compiler emission and pulled in `compiler_builtins` 128-bit helpers, growing the full-pipeline image from ~622 KB to ~663 KB and overflowing the 640 KB FLASH region by ~22 KB. Rebalanced AXISRAM2: FLASH `640 KB → 704 KB`, RAM `384 KB → 320 KB`, `HEAP_SIZE` `320 KB → 256 KB`. All three feature modes now link clean (trust-load 142 KB, verifier-only 165 KB, full pipeline 663 KB). MANUAL.md updated. |
| STM32N6570-DK hardware verification. | Operator-confirmed pass. LED toggling observed on the board; defmt RTT logs render through the new event-code path. The hardware-verification command set in the prior turn covered host smoke test, bare-metal library compile check, three-mode size check, flash-and-observe under each feature combination, and the pass criteria from MANUAL.md Section 10. |
| Queue remaining V0.2 design-pass items and newly-opened items. | Six new BACKLOG entries added: B13 refinement-type compile-time elision through range analysis (Item 4 of the V0.2 gap list), B14 CallIndirect flow analysis (Item 5, deferred to V0.3), B15 remove `Type::Unknown` entirely (B1 follow-up), B16 target-scaled `Fixed` defaults for sub-64-bit native runtimes, B17 embassy feature trimming, B18 big-number arithmetic worked example using the pattern-arm form. Each entry carries scope, out-of-scope items, and a deferral rationale. |

## Verification matrix

```bash
cargo build --quiet                                                            # clean
cargo test --lib --quiet                                                       # 642 lib tests pass
cargo test --workspace --quiet                                                 # all workspace + doctest crates clean
cargo clippy --tests --quiet -- -D warnings                                    # clean
cargo fmt --all                                                                # idempotent

# Bare-metal STM32N6570-DK builds, all three modes.
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf --release \
    --bin three-task-n6 --no-default-features --features stm32n6570dk-platform)
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf --release \
    --bin three-task-n6 --no-default-features --features stm32n6570dk-platform,keleusma-verify)
(cd examples/rtos && cargo build --target thumbv8m.main-none-eabihf --release \
    --bin three-task-n6 --no-default-features --features stm32n6570dk-platform,keleusma-compile,keleusma-verify)
```

Hardware: STM32N6570-DK, operator-verified 2026-05-19.

## Backlog summary

| ID | Title | Status |
|----|-------|--------|
| B13 | Refinement-type compile-time elision through range analysis | Deferred (needs interval-arithmetic infrastructure) |
| B14 | CallIndirect flow analysis for non-recursive closures | Deferred to V0.3 |
| B15 | Remove `Type::Unknown` entirely (B1 follow-up) | Foundation in place; refactor pending |
| B16 | Target-scaled `Fixed` defaults for sub-64-bit native runtimes | Deferred until host demand |
| B17 | Embassy feature trimming | Deferred until measured size pressure |
| B18 | Big-number arithmetic worked example | Deferred until adoption demand |

Also tracked as newly-opened from the checked-arithmetic refactor (not yet in BACKLOG.md as standalone entries):

- `Op::CheckedDiv` / `Op::CheckedMod` with proper `(h, l, flag)` for the `i64::MIN / -1` corner. Small change; deferred until a real consumer hits it. Cross-references B18 (prerequisite for big-number division and modulo).

## Notes

- Four branch commits on `v0.2.0` ahead of `origin/v0.2.0` (`46a649f`, `771e2b1`, `68e7cb5`, `1c9e3a8`). Push remains operator-driven.
- With hardware verification passed, V0.2 is in releasable shape from the agent side. The remaining release-tag action is operator-driven.
- The newly-opened `Op::CheckedDiv` / `Op::CheckedMod` item could be added as a standalone B19 entry if it warrants tracking outside the checked-arithmetic refactor's commit; left as a note for now.

## Intended Next Step

Awaiting operator prompt.

1. **Operator action**: V0.2 release tag. All design-pass items closed or deferred; hardware verification passed; backlog recorded.
2. **Operator action**: optional `Op::CheckedDiv` / `Op::CheckedMod` request if the `i64::MIN / -1` corner case matters for an upcoming consumer.
3. **Backlog**: B13 through B18 stand ready for selection when a future development phase opens.
