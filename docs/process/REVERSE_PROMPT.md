# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: V0.2.0 pre-publish polish items P1 through P5 closed and CI restored to green-ready. Crates.io, docs.rs, license, and CI badges added to all five publishable crates' READMEs. CHANGELOG V0.2.0 section reviewed and judged complete at the headline level. `cargo doc` clean across all five crates under the CI flags. Full workspace `cargo test` passes. CI workflow rewritten to replace the failing `--all-features` invocations with explicit feature sets and to install the full SDL3 build dependencies on the dedicated SDL3 examples job. The branch is ready for `cargo publish` in dependency order.

## Completed in this session round

| Item | Resolution |
|------|------------|
| P1 — top-level README badges | Crates.io, Docs.rs, License (0BSD), CI badges added to `README.md`. |
| P2 — child-crate README badges | Crates.io, Docs.rs, License badges added to `keleusma-arena/README.md`, `keleusma-macros/README.md`, `keleusma-bench/README.md`, `keleusma-cli/README.md`. The arena badge uses an absolute OSI URL for the license link to avoid a broken intra-doc-link warning (the arena lib includes its README through `#![doc = include_str!("../README.md")]`). |
| P3 — CHANGELOG V0.2.0 verification | The V0.2.0 section has 148 lines and covers the headline additions: cryptographic module signing (R42), ISA reset, wire-format reset (BYTECODE_VERSION 1), refinement-newtype saturation contracts, big-number arithmetic worked example, pattern-matched checked-arithmetic arms with guards, IFC label propagation including negative labels, ephemeral data partitioning (shared/private/const), the RTOS microkernel example, B13/B15/B18 closures, the `compile`/`verify`/`floats`/`text`/`shell`/`signatures` cargo features, the `keleusma-bench` crate and calibrated WCET cost models, the docs/spec/ reorganization. The recent session work on items 1-13 and items A-G is sub-release polish and does not need explicit changelog entries. |
| P4 — workspace cargo doc | `cargo doc -p keleusma --no-deps --features signatures,shell`, `cargo doc -p keleusma-arena --no-deps --all-features`, `cargo doc -p keleusma-macros --no-deps`, `cargo doc -p keleusma-bench --no-deps`, `cargo doc -p keleusma-cli --no-deps` all clean under `RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links"`. |
| P5 — workspace cargo test | `cargo test --workspace` passes end to end. Doctests pass. |
| CI repair | `.github/workflows/ci.yml` was failing on three jobs (Test (all features), Doc, Examples (SDL3 feature)) because `--all-features` cascades the mutually-exclusive narrow-* selectors into the narrowest configuration AND pulls in `sdl3-example`, which cmake-builds SDL3 from source. The SDL3 build needs X11, Wayland, and audio development headers that the Ubuntu runner does not have by default; the previous install installed `libsdl2-dev` (SDL2, wrong library). Fix: replace `--all-features` in the Test and Doc jobs with the docs.rs feature set (`signatures,shell` on top of the defaults); install the full SDL3 development dependency list on the Examples (SDL3 feature) job. The Test job is renamed to "Test (broad features)" to be honest about what it tests. The Doc job now exercises the same feature set docs.rs renders, so the CI signal matches what the published documentation will look like. |

## What the operator still owns

- **Publish in dependency order.** `keleusma-macros 0.2.0` → `keleusma 0.2.0` → `keleusma-bench 0.2.0` + `keleusma-cli 0.2.0`. The `keleusma-arena 0.3.0` is already on crates.io and matches the local source bit-identically.
- **Tag the release.** `git tag v0.2.0 && git push --tags`.
- **Decide B15.** Backlog "remove `Type::Unknown` entirely" remains under consideration. Recommendation: defer to V0.2.x or V0.3.0.
- **Optional: CI required-status-checks rename.** The `test-all-features` job was renamed to `test-broad-features`. If the GitHub branch protection rules required `Test (all features)` as a status check, the rule must be updated to require `Test (broad features)`.

## Verification matrix

```bash
# Tests (CI mirrors these invocations)
cargo test --workspace                                          # all pass
cargo test -p keleusma --no-default-features                    # all pass
cargo test -p keleusma --features signatures                    # all pass
cargo test -p keleusma --features signatures,shell              # all pass (new broad CI job)
cargo test -p keleusma-bench                                    # all pass

# Format and clippy
cargo fmt --all -- --check                                      # clean
cargo clippy --workspace --all-targets -- -D warnings           # clean

# Doc (per-crate, mirrors new CI)
RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links" \
  cargo doc -p keleusma --no-deps --features signatures,shell   # clean
  cargo doc -p keleusma-arena --no-deps --all-features          # clean
  cargo doc -p keleusma-macros --no-deps                        # clean
  cargo doc -p keleusma-bench --no-deps                         # clean
  cargo doc -p keleusma-cli --no-deps                           # clean
```

## Open concerns

1. **Node.js 20 deprecation notice.** GitHub Actions emits a notice that `actions/checkout@v4` runs on Node.js 20, which will be forced to Node.js 24 by default in June 2026 and removed in September 2026. Not blocking for V0.2.0 publish. A separate follow-up pass should bump checkout actions to a Node.js 24 compatible major when the upstream release lands.
2. **SDL3 CI job cost.** The Examples (SDL3 feature) job builds SDL3 from source through cmake. With the full dependency install, the job will take five to ten minutes per push. The job remains valuable because it catches SDL3-gated regressions; if CI cost becomes a concern, the job can be moved to a label-gated trigger or a weekly schedule rather than every push.
3. **arena 0.3.0 already on crates.io.** No action needed. Source matches.

## Backlog summary

Unchanged from prior session.

## Intended Next Step

V0.2.0 publish. The operator runs `cargo publish` in dependency order. After publish: `git tag v0.2.0 && git push --tags`. The publish is operator-owned; the AI agent's pre-publish work is complete.

Alternatives:
- Take up B15 (`Type::Unknown` removal) before publish. Recommendation: defer.
- Defer publish and bump checkout actions to a Node.js 24 compatible major.
- Operator selection of a different directive.
