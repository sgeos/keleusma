#!/usr/bin/env bash
#
# scripts/verify.sh -- comprehensive local verification beyond the pre-push gate.
#
# The pre-push hook (.git/hooks/pre-push) runs only the DEFAULT-feature workspace
# checks and never the detached compiler/ subproject. CI additionally exercises
# the --no-default-features and signatures/broad feature sets, and the subproject
# is gated nowhere. That gap is how a --no-default-features compile break and
# subproject regressions reach CI without the pre-push gate noticing. Run this
# before pushing changes that touch feature-gated code or the compiler/ subproject.
#
# This mirrors CI's runnable feature matrix. It deliberately does NOT use
# --all-features: that config is unsuitable for this workspace because it selects
# mutually-degenerate narrow-word widths, exactly as the CI "broad features" job
# documents; the broad config CI (and this script) use is `signatures,shell`.
#
# It does not reproduce the toolchain/target-specific CI jobs (Miri, cross-builds
# for no_std/RTOS, WASM, SDL3 examples, VS Code); the no_std build and the MSRV
# check are attempted only when the required target/toolchain is installed, and
# skipped with a note otherwise.
#
# Every check runs even if an earlier one fails, so a single run reports all
# failures; the script exits non-zero if any check failed.

set -uo pipefail
cd "$(dirname "$0")/.."

FAILED=()
section() { printf '\n\033[1m========== %s ==========\033[0m\n' "$1"; }
run() { # run <label> <command string>
  local label="$1"
  shift
  printf '\n--- %s ---\n' "$label"
  if bash -c "$*"; then
    printf '  \033[32mPASS\033[0m %s\n' "$label"
  else
    printf '  \033[31mFAIL\033[0m %s\n' "$label"
    FAILED+=("$label")
  fi
}

# Match CI's parallel test runner when available; fall back to `cargo test`.
if cargo nextest --version >/dev/null 2>&1; then
  RUN="cargo nextest run"
else
  RUN="cargo test"
  echo "note: cargo-nextest not installed; using 'cargo test' (install: cargo install cargo-nextest --locked)"
fi

section "Format and lint (default features)"
run "fmt --check" "cargo fmt --all -- --check"
run "clippy --workspace --all-targets -D warnings" "cargo clippy --workspace --all-targets -- -D warnings"

section "Feature-config test matrix (the gap the pre-push gate misses)"
run "test: default (workspace)" "$RUN --workspace && cargo test --workspace --doc"
run "test: --no-default-features" \
  "$RUN -p keleusma --no-default-features && cargo test -p keleusma --no-default-features --doc"
run "test: --features signatures" \
  "$RUN -p keleusma --features signatures && cargo test -p keleusma --features signatures --doc"
run "test: --features signatures,shell (broad)" \
  "$RUN -p keleusma --features signatures,shell && cargo test -p keleusma --features signatures,shell --doc"
run "build: keleusma-bench --no-default-features" "cargo build -p keleusma-bench --no-default-features"

section "Docs and markdown links"
run "doc: keleusma (docs.rs feature set, CI flags)" \
  "RUSTDOCFLAGS='-D warnings -A rustdoc::redundant-explicit-links' cargo doc -p keleusma --no-deps --features signatures,encryption,shell"
run "markdown link check" "cargo run -q -p keleusma-cli -- run scripts/check-md-links.kel"

section "Detached compiler/ subproject (gated by neither the pre-push hook nor CI)"
run "compiler: fmt --check" "cd compiler && cargo fmt --all -- --check"
run "compiler: clippy --all-targets -D warnings" "cd compiler && cargo clippy --all-targets -- -D warnings"
run "compiler: test" "cd compiler && cargo test"

section "Optional cross-config checks (skipped when the toolchain/target is absent)"
if rustup target list --installed 2>/dev/null | grep -qx 'thumbv7em-none-eabihf'; then
  run "no_std build (thumbv7em-none-eabihf)" "cargo build -p keleusma --target thumbv7em-none-eabihf"
else
  echo "  SKIP no_std build -- target not installed (rustup target add thumbv7em-none-eabihf)"
fi
if rustup toolchain list 2>/dev/null | grep -q '^1\.88'; then
  run "MSRV check (1.88)" \
    "cargo +1.88 check -p keleusma && cargo +1.88 check -p keleusma --no-default-features && cargo +1.88 check -p keleusma --features signatures && cargo +1.88 check -p keleusma --tests"
else
  echo "  SKIP MSRV 1.88 check -- toolchain not installed (rustup toolchain install 1.88)"
fi

printf '\n\033[1m========== SUMMARY ==========\033[0m\n'
if [ ${#FAILED[@]} -eq 0 ]; then
  printf '\033[32mAll checks passed.\033[0m\n'
  exit 0
else
  printf '\033[31m%d check(s) FAILED:\033[0m\n' "${#FAILED[@]}"
  printf '  - %s\n' "${FAILED[@]}"
  exit 1
fi
