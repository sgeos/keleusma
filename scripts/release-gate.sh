#!/usr/bin/env bash
# release-gate.sh — the local pre-release verification gate.
#
# Runs the same checks the CI workflow (.github/workflows/ci.yml) enforces, so a
# green run here means CI will be green. It is a superset of the everyday
# `cargo test && cargo clippy` and, critically, includes `cargo doc` under
# `-D warnings` — the check whose absence let a red CI Doc job ship alongside
# V0.2.1 (broken intra-doc links). Do not cut this gate down to a subset before a
# release; run it whole.
#
# Usage:
#   scripts/release-gate.sh          # fmt, clippy, tests, doc, doc-links
#   scripts/release-gate.sh --miri   # also run Miri (nightly, Tree Borrows) — slow
#
# Requires a healthy stable toolchain. If `rustc --version` errors with
# "the rustc binary ... is not applicable", repair it:
#   rustup component add rustc --toolchain stable
set -euo pipefail
cd "$(dirname "$0")/.."

RUN_MIRI=0
[ "${1:-}" = "--miri" ] && RUN_MIRI=1

# Fail fast with a repair hint if the active toolchain is broken (a recurring
# local failure mode: an interrupted rustup update leaves stable without a usable
# rustc component).
if ! rustc --version >/dev/null 2>&1; then
  echo "error: the active 'rustc' is not usable (broken toolchain). Repair with:" >&2
  echo "  rustup component add rustc --toolchain stable" >&2
  exit 1
fi

step() { printf '\n\033[1m=== %s ===\033[0m\n' "$1"; }

step "Format (cargo fmt --check)"
cargo fmt --check

step "Clippy (workspace, all targets, -D warnings)"
cargo clippy --workspace --all-targets -- -D warnings

step "Tests — default features"
cargo test --workspace

step "Tests — keleusma no default features"
cargo test -p keleusma --no-default-features

step "Tests — keleusma signatures"
cargo test -p keleusma --features signatures

step "Tests — keleusma signatures,shell (broad / docs.rs surface)"
cargo test -p keleusma --features signatures,shell

# The Doc gate: mirror the CI Doc job exactly. -D warnings turns a broken or
# private intra-doc link into an error. Each crate is documented at the same
# feature set docs.rs uses so the signal matches the published docs.
step "Docs (-D warnings) — the check that catches broken intra-doc links"
export RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links"
cargo doc -p keleusma       --no-deps --features signatures,encryption,shell
cargo doc -p keleusma-arena --no-deps --all-features
cargo doc -p keleusma-macros --no-deps
cargo doc -p keleusma-bench  --no-deps
cargo doc -p keleusma-cli    --no-deps
unset RUSTDOCFLAGS

step "Relative Markdown links (check-md-links.kel)"
cargo run -q -p keleusma-cli -- run scripts/check-md-links.kel

if [ "$RUN_MIRI" -eq 1 ]; then
  step "Miri — Tree Borrows (memory-safety regressions)"
  MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test -p keleusma-arena
  MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test -p keleusma \
    --test marshall c1_null_text_pointer_marshals_to_empty_string_not_ub
fi

printf '\n\033[1;32m=== release gate: GREEN ===\033[0m\n'
