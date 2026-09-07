#!/usr/bin/env bash
# The backend's own gate: formatting, lints, and the full suite, FROZEN-CHECKED.
#
# WHY THIS EXISTS
#
# `native_codegen` is a detached package. The workspace gate does not build it and
# continuous integration does not run it, so the local run is the ONLY gate this
# backend has -- and until now it was a set of commands retyped from memory each
# time, with three facts living only in a session's scrollback:
#
#   1. THE SUITE MUST BE SPLIT. `corpus_differential` alone takes ~390s and the
#      rest ~140s. Run together they exceed the harness's ten-minute background
#      ceiling and the run is killed mid-flight, which happened twice on
#      2026-09-06 before the cause was understood.
#   2. THERE ARE TWO FLOAT CONFIGURATIONS. `narrow-float-32` is selectable only
#      because `native_codegen/Cargo.toml` forwards it; without that it is
#      reachable by hand-editing the manifest, which is why it went unmeasured
#      until 2026-08-31, when it was found RED.
#   3. EVERY RUN SHOULD BE FROZEN-CHECKED. Four measurements across three
#      sessions were taken while their inputs moved.
#
# **POINT 3 IS WHY THIS IS A SCRIPT AND NOT A NOTE.** `frozen-run.sh` made the
# tree check structural for runs that REMEMBER to use it, and the handoff's
# instruction to "wrap long runs in it" is a rule -- the form that has already
# failed four times. Wrapping happens here instead, so the common case cannot
# forget.
#
# WHAT IT IS NOT
#
# **Not the release gate.** `scripts/release-gate.sh` covers the workspace and is
# mandatory before a merge to the release line; this covers the detached backend
# only, and neither substitutes for the other.
#
# **Not a continuous-integration job.** Adding one is a per-push cost, and the
# `v0.2.3` line has recorded that such costs are the operator's call.
#
# Usage:  tools/backend-gate.sh [--narrow]      (default: default features)
set -uo pipefail
cd "$(dirname "$0")/.."

feat=()
label="default features"
if [ "${1:-}" = "--narrow" ]; then
    feat=(--features narrow-float-32)
    label="narrow-float-32"
fi

echo "================ BACKEND GATE — $label"
fail=0

echo "-- fmt"
cargo fmt --all -- --check || fail=1

echo "-- clippy"
cargo clippy --all-targets "${feat[@]}" -- -D warnings || fail=1

# Split at `corpus_differential`; see note 1 above.
echo "-- suite, part 1 of 2 (everything else)"
tools/frozen-run.sh cargo nextest run --no-fail-fast "${feat[@]}" \
    -E 'not binary(corpus_differential)' || fail=1

echo "-- suite, part 2 of 2 (corpus_differential)"
tools/frozen-run.sh cargo nextest run --no-fail-fast "${feat[@]}" \
    -E 'binary(corpus_differential)' || fail=1

echo "================ BACKEND GATE — $label: $([ $fail -eq 0 ] && echo PASS || echo FAIL)"
exit $fail
