# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: V0.2.0 pre-publish polish items Q1 and D1 closed. Per-crate `CHANGELOG.md` files added to `keleusma-bench` and `keleusma-cli`. `actions/checkout@v4` bumped to `actions/checkout@v5` across all 15 use sites to resolve the Node.js 20 deprecation notice. CI run prior to this round was fully green (15/15). The branch is publish-ready.

## Completed in this session round

| Item | Resolution |
|------|------------|
| Q1 — per-crate CHANGELOGs | `keleusma-bench/CHANGELOG.md` and `keleusma-cli/CHANGELOG.md` created in Keep a Changelog 1.1.0 format matching the existing `keleusma-arena` and `keleusma-macros` style. Both V0.2.0 entries describe the crate's first-publicly-released surface. `cargo package --list` confirms both ship in the published tarballs. |
| D1 — Node.js 20 deprecation | `actions/checkout@v4` bumped to `actions/checkout@v5` (Node.js 24) across all 15 use sites in `.github/workflows/ci.yml`. The `dtolnay/rust-toolchain` actions did not draw the deprecation notice and were left unchanged. |

## V0.4.0 clarification (operator question)

The strategy docs in `docs/process/` are:

- `V0_3_0_SELF_HOSTING.md` — V0.3.0 self-hosting compiler bootstrap.
- `V0_4_0_NATIVE_CODEGEN.md` — V0.4.0 native code generation.
- `V0_5_0_KELEUSMA_HOST.md` — V0.5.0 Keleusma host for the compiler (sub-coroutines depend on this milestone).

The prior pre-publish list described V0.5.0 work but omitted V0.4.0. This was an oversight in the enumeration, not a gap in the strategy. V0.4.0 (native codegen) is documented and tracked in its own strategy file.

## What the operator still owns

- **Publish in dependency order.** `keleusma-macros 0.2.0` → `keleusma 0.2.0` → `keleusma-bench 0.2.0` + `keleusma-cli 0.2.0`. `keleusma-arena 0.3.0` is already on crates.io and matches the local source bit-identically.
- **Tag the release.** `git tag v0.2.0 && git push --tags`.
- **Decide B15.** Backlog "remove `Type::Unknown` entirely" remains under consideration. Recommendation: defer to V0.2.x or V0.3.0.
- **Optional: CI required-status-checks rename.** The `test-all-features` job was renamed to `test-broad-features` in a prior round. If GitHub branch protection required the old name as a status check, update the rule to require `Test (broad features)`.

## Verification matrix

```bash
# Tests (CI mirrors these invocations)
cargo test --workspace                                          # all pass
cargo test -p keleusma --no-default-features                    # all pass
cargo test -p keleusma --features signatures                    # all pass
cargo test -p keleusma --features signatures,shell              # all pass (CI broad-features job)
cargo test -p keleusma-bench                                    # all pass

# Format and clippy
cargo fmt --all -- --check                                      # clean
cargo clippy --workspace --all-targets -- -D warnings           # clean

# Doc (per-crate, CI mirrors)
RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links" \
  cargo doc -p keleusma --no-deps --features signatures,shell   # clean
  cargo doc -p keleusma-arena --no-deps --all-features          # clean
  cargo doc -p keleusma-macros --no-deps                        # clean
  cargo doc -p keleusma-bench --no-deps                         # clean
  cargo doc -p keleusma-cli --no-deps                           # clean

# Package contents
cargo package --list -p keleusma-bench --allow-dirty            # includes CHANGELOG.md
cargo package --list -p keleusma-cli --allow-dirty              # includes CHANGELOG.md
```

## Open concerns

1. **SDL3 CI job cost.** The Examples (SDL3 feature) job builds SDL3 from source through cmake. The full dependency install lands the job at roughly 1m25s on the standard Ubuntu runner; not the 5-10 minutes initially expected. The job remains valuable because it catches SDL3-gated regressions.
2. **arena 0.3.0 already on crates.io.** No action needed. Source matches.

## Backlog summary

Unchanged from prior session.

## Intended Next Step

V0.2.0 publish. The operator runs `cargo publish` in dependency order. After publish: `git tag v0.2.0 && git push --tags`. The publish is operator-owned; the AI agent's pre-publish work is complete.

Alternatives:
- Take up B15 (`Type::Unknown` removal) before publish. Recommendation: defer.
- Defer publish; operator selection of a different directive.
