# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: V0.2.0 pre-publish pass complete on items 1 through 13 from the publication checklist. Crate versions bumped, MSRV verified, rustdoc warnings cleaned, spec docs audited, CHANGELOG promoted to `[0.2.0] - 2026-05-21`, WHY_REJECTED diagnostics confirmed to match source, READMEs refreshed, unsafe blocks audited, workspace tests pass under default features and default+signatures, CI workflow reviewed (gaps noted), all examples build, N6 hardware boot captured the WCET report, cargo publish --dry-run gated. The branch is ready for `cargo publish` in dependency order. Items 14 (migration guide) rejected per operator. Items 15 (B15 Type::Unknown removal) under operator consideration. Item 16 (tag/release process) premature.

## Completed in this session round

| # | Item | Resolution |
|---|------|------------|
| 1 | Crate versions bumped | keleusma 0.1.1 → 0.2.0, keleusma-bench 0.1.0 → 0.2.0, keleusma-cli 0.1.0 → 0.2.0, keleusma-macros 0.1.0 → 0.2.0. keleusma-arena stays at 0.3.0 (already on crates.io). Intra-workspace dep version requirements bumped. |
| 10 | MSRV review | Recent additions (env::set_var unsafe in 2024 edition, let-chains in cost-model emit, libm::ceil) all within the pinned MSRVs (1.85 for arena/macros, 1.88 for keleusma/bench/cli). |
| 11 | cargo doc clean | Seven rustdoc warnings resolved (link path corrections, private-item references rendered as prose). Remaining warning is cargo #6313 (lib vs bin name collision); not actionable here. |
| 7 | Spec docs freshness | Opcode count (69) matches Op enum; wire-format constants match; signature extension layout present; negative-IFC and signed surface present in GRAMMAR.md. |
| 2 | CHANGELOG entry | `[Unreleased]` block promoted to `[0.2.0] - 2026-05-21` with a release headline summary; fresh `[Unreleased]` inserted above. |
| 8 | WHY_REJECTED audit | Closure and first-class-function-reference diagnostic strings in WHY_REJECTED.md match the source. |
| 9 | README accuracy | Top-level README Cargo dep example bumped to "0.2"; FAQ blurb softened. All 380+ markdown cross-references resolve. |
| 13 | Unsafe block audit | 6 V0.2.0-introduced unsafe sites (RDTSC, CNTVCT_EL0, CNTFRQ_EL0, DWT_CYCCNT MMIO, env::set_var, ZeroSizeOk wrapper). All have SAFETY justifications. |
| 4 | Full workspace tests | Default features: 826 main + 53 rogue + others, all pass. Default+signatures: same. `--no-default-features`: passes after gating one bench test on the `std` feature. `--all-features` exposes 5 pre-existing test failures at unusual feature interactions, not in publish-relevant configurations. |
| 5 | CI workflow review | `.github/workflows/ci.yml` covers check, test (default + no-default + signatures), clippy strict, fmt, per-crate MSRV, thumbv7em-none-eabihf no-std, Miri stacked+tree borrows. Gaps noted (keleusma-bench, thumbv8m, cargo doc) but not publish blockers. |
| 6 | Examples build | `cargo build --workspace --examples --release` clean; same with `--features sdl3-example`. |
| 12 | N6 boot with WCET | three-task-n6 with `keleusma-verify` flashed and the WCET boot report captured: led NOMINAL 74 / MEASURED 409377, sensor NOMINAL 66 / MEASURED 362878, heartbeat NOMINAL 60 / MEASURED 326458. Kernel boots, scheduler enters loop, supervised restart fires on faulty task. |
| 3 | cargo publish --dry-run | keleusma-macros 0.2.0 dry-runs clean. keleusma-arena 0.3.0 already on crates.io; operator decides whether arena needs to bump to 0.4.0 for post-0.3.0 changes (KString move, persistent .data region). keleusma 0.2.0, keleusma-bench 0.2.0, keleusma-cli 0.2.0 fail dry-run with "candidate versions found which didn't match: 0.1.x" because the publish order requires macros 0.2.0 to land on crates.io first; this is the standard workspace publish dance, not a state issue. Final clippy strict + fmt --check pass clean. |

## What the operator still owns

- **Decide arena version.** Verify whether the currently-published `keleusma-arena 0.3.0` matches the current source. If yes, no further arena action. If the current source has changes the published 0.3.0 lacks (KString move via `969bdeb`, persistent .data region via `fe7fc5a`), bump to 0.4.0 and republish.
- **Publish in dependency order.** macros 0.2.0 → keleusma 0.2.0 → bench 0.2.0 + cli 0.2.0.
- **Tag the release.** `git tag v0.2.0 && git push --tags`. Operator-owned.
- **Decide B15.** Backlog "remove `Type::Unknown` entirely" is queued for consideration; recommendation was to defer.

## Verification matrix

```bash
# Tests (publish-relevant configurations)
cargo test --workspace --release                                   # 826 main + others
cargo test --workspace --release --no-default-features             # passes
cargo test -p keleusma --release --features signatures             # 826 main

# Clippy and fmt
cargo clippy --workspace --all-targets --tests --release -- -D warnings  # clean
cargo fmt --all -- --check                                         # clean

# Doc
cargo doc --workspace --no-deps --all-features                     # 0 warnings (excluding cargo #6313)

# Examples
cargo build --workspace --examples --release                       # clean
cargo build --workspace --examples --release --features sdl3-example  # clean

# Cross-compile
cargo build --release --manifest-path examples/rtos/Cargo.toml \
    --bin three-task-n6 --target thumbv8m.main-none-eabihf \
    --no-default-features --features stm32n6570dk-platform,keleusma-verify   # clean

# Dry-run publish
cargo publish -p keleusma-macros --dry-run --allow-dirty           # clean
cargo publish -p keleusma-arena --dry-run --allow-dirty            # "already exists" warn
cargo publish -p keleusma --dry-run --allow-dirty                  # fails on macros 0.2.0
                                                                    # (publish-order dance)
```

## Open concerns

1. **`--all-features` test failures.** Five tests fail under the unusual `--all-features` combination (`embedded_8` target tests and three checked-multiplication high-half assertions). Pre-existing; not present in publish-relevant configurations. Worth investigating post-publish.
2. **arena 0.3.0 status.** Already on crates.io, but the local source has changes since the publish (KString move, persistent .data region). Operator confirms whether the current source matches the published 0.3.0 or needs 0.4.0.
3. **CI gaps.** keleusma-bench, thumbv8m target, cargo doc, examples build matrix. Not blockers but worth a follow-on CI pass.

## Backlog summary

Unchanged from prior session.

## Intended Next Step

V0.2.0 publish. The operator runs `cargo publish` in dependency order. After publish: `git tag v0.2.0 && git push --tags`. The publish is operator-owned; the agent's pre-publish work is complete.

Alternatives:
- Address `--all-features` test failures before publish.
- Take up B15 (`Type::Unknown` removal) before publish.
- Defer publish and address CI gaps.
- Operator selection of a different directive.
