#!/usr/bin/env bash
# release-gate.sh — the local pre-release verification gate.
#
# Runs the checks the CI workflow (.github/workflows/ci.yml) enforces PLUS the
# detached compiler/ subproject that CI never sees, so a green run here means CI
# will be green AND the self-hosted compiler subproject is sound. It is a superset
# of the everyday `cargo test && cargo clippy` and, critically, includes `cargo doc`
# under
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

# Reap orphaned test binaries before timing anything.
#
# When a gate is interrupted -- a closed laptop, a dropped session, a Ctrl-C --
# cargo dies but the test binary it spawned is reparented to PID 1 and keeps
# running at full tilt. On 2026-08-08 one had been burning four cores for ten
# hours and was halving the machine; the gate running beside it doubled in speed
# the moment it was reaped. These accumulate silently, one per interrupted run.
#
# This matters beyond wall-clock: the performance canary below reads elapsed
# time, and a machine quietly running at half capacity is exactly how a canary
# produces a false alarm and then gets its ceiling raised for the wrong reason.
if pgrep -f "$(pwd)/target/debug/deps" >/dev/null 2>&1; then
  echo "note: reaping orphaned test binaries from an earlier interrupted run" >&2
  pkill -f "$(pwd)/target/debug/deps" || true
  sleep 1
fi

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

step "Tests — keleusma self-host (the self-hosted compile backend)"
cargo test -p keleusma --features self-host

# keleusma-wire ships an off-by-default `derive` feature, and its READER is meant
# to work with no allocator at all. `cargo test --workspace` above runs DEFAULT
# features only, so neither configuration would be exercised -- the same shape of
# hole that let broken intra-doc links in src/selfhost/ survive four releases.
step "Tests — keleusma-wire all features (derive)"
cargo test -p keleusma-wire --all-features

step "Tests — keleusma-wire no default features (allocator-free reader)"
cargo test -p keleusma-wire --no-default-features

# The Doc gate: mirror the CI Doc job exactly. -D warnings turns a broken or
# private intra-doc link into an error. Each crate is documented at the same
# feature set docs.rs uses so the signal matches the published docs.
step "Docs (-D warnings) — the check that catches broken intra-doc links"
export RUSTDOCFLAGS="-D warnings -A rustdoc::redundant-explicit-links"
cargo doc -p keleusma       --no-deps --features signatures,encryption,shell
# The docs.rs set above excludes `self-host`, so `src/selfhost/` was never
# documented here and its broken intra-doc links survived four releases. The CLI
# enables that feature, so the published CLI docs do reach it. Document it
# explicitly rather than relying on the docs.rs set.
cargo doc -p keleusma       --no-deps --features signatures,encryption,shell,self-host
cargo doc -p keleusma-arena --no-deps --all-features
cargo doc -p keleusma-macros --no-deps
cargo doc -p keleusma-bench  --no-deps
cargo doc -p keleusma-cli    --no-deps
cargo doc -p keleusma-wire   --no-deps --all-features
cargo doc -p keleusma-wire-derive --no-deps
unset RUSTDOCFLAGS

step "Relative Markdown links (check-md-links.kel)"
cargo run -q -p keleusma-cli -- run scripts/check-md-links.kel

# The self-hosted compiler at compiler/ is a DETACHED workspace: excluded from the
# crate tarball, run by neither the pre-push hook nor CI. It reads the shared
# kel/*.kel stage sources, so a change to those or to its Rust driver can break it
# silently. Including it here is where that is caught before a merge to the release
# line, and it makes this gate a SUPERSET of CI (which never sees the subproject). A
# stale decoder here shipped `unknown op tag 62` into v0.2.3 undetected; the
# `decoder_drift_guard` unit test in compiler/src/selfhost.rs is the fast standing
# regression, and this step is the full check. (Process-audit 2026-07-22, item 4.)
step "Detached compiler/ subproject (fmt, clippy, tests — gated nowhere else)"
( cd compiler && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test )

# The LLVM backend at native_codegen/ is a DETACHED workspace for the same
# reasons as compiler/, plus one it does not have: it needs an LLVM 22.1
# development install. As a workspace member it would make LLVM a hard build
# dependency of the entire repository, for every developer and for CI.
#
# So this step SKIPS when LLVM is absent -- but it skips LOUDLY. A silent skip
# is the same shape of hole that let four broken intra-doc links in
# src/selfhost/ survive four releases: a step that quietly does nothing reads
# as a step that passed. Anyone whose gate prints the skip notice has been told,
# in terms, that the native lowering was not verified by this run.
KEL_LLVM_PREFIX_DEFAULT=/opt/local/libexec/llvm-22
if [ -n "${LLVM_SYS_221_PREFIX:-}" ] || [ -d "$KEL_LLVM_PREFIX_DEFAULT" ]; then
  step "Detached native_codegen/ subproject (fmt, clippy, tests — gated nowhere else)"
  # `cargo doc` belongs here for the reason the comment above this step
  # gives about the workspace Doc job: this package declares its own
  # `[workspace]` and is absent from the parent's `members`, so
  # `cargo doc --workspace` NEVER sees it. Without this line a broken
  # intra-doc link in `native_codegen/` is caught nowhere, by anything —
  # the same hole, one directory over, from the one the step above closed.
  ( cd native_codegen && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test \
    && RUSTDOCFLAGS="-D warnings" cargo doc --no-deps )
else
  step "Detached native_codegen/ subproject — SKIPPED"
  printf '  \033[1;33mNO LLVM 22.1 DEVELOPMENT INSTALL FOUND. THIS STEP DID NOT RUN.\033[0m\n'
  printf '  The native lowering is UNVERIFIED by this gate. To run it, install\n'
  printf '  LLVM 22.1 (MacPorts: sudo port install llvm-22) or set\n'
  printf '  LLVM_SYS_221_PREFIX to an existing install. See native_codegen/README.md.\n'
fi

if [ "$RUN_MIRI" -eq 1 ]; then
  step "Miri — Tree Borrows (memory-safety regressions)"
  MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test -p keleusma-arena
  MIRIFLAGS="-Zmiri-tree-borrows" cargo +nightly miri test -p keleusma \
    --test marshall c1_null_text_pointer_marshals_to_empty_string_not_ub
fi

# Incidental sccache metrics capture (guarded; no-op without sccache).
"$(dirname "$0")/sccache-metrics.sh" "release-gate" >/dev/null 2>&1 || true

printf '\n\033[1;32m=== release gate: GREEN ===\033[0m\n'
