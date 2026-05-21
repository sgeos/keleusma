# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-21
**Status**: V0.2.0 pre-publish polish: top-level `README.md` and every file in `docs/guide/` audited and corrected. All identified inaccuracies and gaps closed. Verification: `cargo fmt --all -- --check` clean; the README quick-start compiles and runs end to end emitting `result: Int(42)`. The branch is publish-ready.

## Completed in this session round

### Top-level `README.md`

| Fix | Detail |
|-----|--------|
| Broken pattern-matching example | The `describe` function used a non-existent `format` native. Rewrote it to match an `enum Message { Body(Text), Code(Word) }` exhaustively without text-composition natives. |
| Cargo feature table omission | Added the `signatures` row (Ed25519 signing surface introduced in V0.2.0) and the `sdl3-example` row. |
| Narrow-runtime selectors | Added a one-line note pointing at the seven mutually-exclusive `narrow-word-*` / `narrow-address-*` / `narrow-float-32` parametric features. |
| BACKLOG B10 reference | Reframed to acknowledge that the portability foundation is in place; added a forward pointer to the `narrow-*` cargo features. |
| Examples section | Added a pointer to the new `examples/README.md` overview. |
| Quick Start | Verified end to end; emits `result: Int(42)`. No change needed. |

### `docs/guide/` files

| File | Fix |
|------|-----|
| `README.md` | Removed `,text` from piano-roll and rogue command lines. Reframed the FAQ row from V0.1.x to V0.2.0. |
| `GETTING_STARTED.md` | Bumped the embedding `Cargo.toml` snippet to `keleusma = "0.2"` and `keleusma-arena = "0.3"`. Stripped `text` from the piano-roll Next Steps command. |
| `EMBEDDING.md` | Corrected "four bundled libraries" to "three" (the V0.1.x `stddsl::Text` bundle was retired). Replaced the `set_native_bounds` invocations that used invalid Rust named-parameter syntax with positional `(name, wcet, wcmu_bytes)`. |
| `FAQ.md` | Rewrote the "Opaque types compile but cannot cross the native boundary" section to reflect the V0.2.0 `HostOpaque` first-class support. Removed the stale "Bytecode 0.1.0 was yanked" entry that no longer applies to V0.2.0 readers. |
| `BIG_NUMBERS.md` | Replaced the "Division and modulo route to a stamped-zero-flag path" caveat with the V0.2.0 reality: dedicated `Op::CheckedDiv` and `Op::CheckedMod` with the `(h, l, flag)` shape; both `i64::MIN / -1` and `i64::MIN % -1` corners flag through the overflow arm. |
| `PIANO_ROLL.md` | Dropped the `text` feature from the build instruction. Added a sentence noting that static string literals are unconditional in V0.2.0. |
| `ROGUE.md` | Same `text`-feature removal. |
| `WHY_REJECTED.md` | Audited; no changes needed. |
| `COOKBOOK.md` | Audited; no changes needed. The `text::*` host-registered natives use a `text::` namespace prefix that collides historically with the retired V0.1.x `stddsl::Text` bundle name, but the prose correctly distinguishes them. |

### `docs/README.md`

| Fix | Detail |
|-----|--------|
| FAQ Quick Reference row | Reframed from V0.1.x to V0.2.0. |

## What the operator still owns

- **Publish in dependency order.** `keleusma-macros 0.2.0` → `keleusma 0.2.0` → `keleusma-bench 0.2.0` + `keleusma-cli 0.2.0`. The arena 0.3.0 is already on crates.io and matches the local source.
- **Tag the release.** `git tag v0.2.0 && git push --tags`.
- **Decide B15.** Backlog "remove `Type::Unknown` entirely" remains under consideration. Recommendation: defer.

## Verification matrix

```bash
# README quick-start runs end to end
( cd /tmp/keleusma_quickstart_test && cargo run )
# -> result: Int(42)

# All workspace tests
cargo test --workspace                                          # all pass (from prior round)

# Per-crate cargo doc under CI flags
RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links" \
  cargo doc -p keleusma --no-deps --features signatures,shell   # clean

# Format and clippy
cargo fmt --all -- --check                                      # clean
cargo clippy --workspace --all-targets -- -D warnings           # clean (from prior round)
```

## Open concerns

None blocking publish. Remaining items are either operator-owned (publish, tag, branch protection) or deferred (B15).

## Intended Next Step

V0.2.0 publish. The operator runs `cargo publish` in dependency order. After publish: `git tag v0.2.0 && git push --tags`.
