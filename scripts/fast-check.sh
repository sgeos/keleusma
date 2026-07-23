#!/usr/bin/env bash
# fast-check.sh — the fast INNER-LOOP lane (seconds-scale), NOT a gate.
#
# Purpose: a reliable, named version of the targeted check run by hand during
# development, so the inner loop has a fast signal without paying for the full gate.
# It runs fmt, clippy on the touched crate, and a test filter you pass in — typically
# the ONE construct test or the ONE changed self-host stage in flight.
#
#   scripts/fast-check.sh 'test(self_host_compiles_word_bnot)'                 # one construct
#   scripts/fast-check.sh 'test(self_host_compiles_parse_kel_byte_identically)' # one changed stage
#   scripts/fast-check.sh 'test(self_hosted_construct_support_boundary)'        # the boundary
#
# Item 3 of the 2026-07-22 process audit — "re-self-compile only the changed stage in
# the inner loop": pass just that stage's `*_kel_byte_identically` filter here; the full
# five-stage self-compile stays in the push hook and merge gate. (Memoization of stage
# bytecode was deliberately NOT added: a stale cache could mask a real divergence, the
# worst failure for a byte-identical differential oracle.)
#
# This is NOT sufficient before a push or merge. The push runs the cargo-husky pre-push
# hook; a merge to the release line runs `scripts/release-gate.sh` (the mandatory gate,
# which also covers the detached compiler/ subproject). Reserve those for their moments.
#
# Optional second arg overrides the crate clippy targets (default: -p keleusma).
set -euo pipefail
cd "$(dirname "$0")/.."

FILTER="${1:-}"
CLIPPY_PKG="${2:--p keleusma}"

step() { printf '\n\033[1mfast-check: %s\033[0m\n' "$1"; }

step "Format (cargo fmt --check)"
cargo fmt --all -- --check

step "Clippy (${CLIPPY_PKG}, tests, -D warnings)"
# shellcheck disable=SC2086
cargo clippy ${CLIPPY_PKG} --tests --features "compile verify" -- -D warnings

if [ -n "$FILTER" ]; then
  step "Tests matching: ${FILTER}"
  if cargo nextest --version >/dev/null 2>&1; then
    cargo nextest run --features "compile verify" -E "$FILTER"
  else
    echo "note: cargo-nextest not installed; falling back to a substring filter with cargo test" >&2
    # Best-effort: strip a leading `test(...)` wrapper to a bare substring for cargo test.
    SUB="$(printf '%s' "$FILTER" | sed -E 's/^test\((.*)\)$/\1/')"
    cargo test --features "compile verify" "$SUB"
  fi
else
  printf '\n\033[1mfast-check: no test filter given.\033[0m Pass one, e.g.\n'
  printf "  scripts/fast-check.sh 'test(self_host_compiles_word_bnot)'\n"
fi

printf '\n\033[1;32mfast-check: OK\033[0m\n'
