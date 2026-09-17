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
#      2026-09-06 before the cause was understood. **The whole script now exceeds
#      that ceiling too** -- five steps, roughly thirteen minutes -- so run it
#      DETACHED and read the log, rather than in the foreground.
#
#      ⚠ **UNDER `narrow-float-32` THE CORPUS PHASE CAN EXCEED IT, AND THE WORD
#      IS `CAN`.** It was killed at the ten-minute foreground ceiling once on
#      2026-09-17 under load, and had to be split by test name --
#      `how_deep_does_the_undetected_set_go` separately from the other nine. **The
#      same phase completed in 415s in this script later the same day**, well
#      inside the ceiling. So it is load-dependent, not structural.
#
#      The first version of this note said it EXCEEDS the ceiling, full stop. That
#      generalised one observation under load into a property, which is a smaller
#      version of the mistake this file exists to prevent. **Run the script
#      detached and the question does not arise**; split by test name only if a
#      foreground run is killed.
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
echo "-- suite, part 1 of 3 (everything but corpus_differential)"
tools/frozen-run.sh cargo nextest run --no-fail-fast "${feat[@]}" \
    -E 'not binary(corpus_differential)' || fail=1

echo "-- suite, part 2 of 3 (corpus_differential)"
tools/frozen-run.sh cargo nextest run --no-fail-fast "${feat[@]}" \
    -E 'binary(corpus_differential)' || fail=1

# 4. THE MIDDLE END, ON THE HALF WHERE IT IS FREE.
#
# `-O0` is a CODEGEN setting, not a pass pipeline: `mem2reg` and the rest do not
# run from it, so undefined behaviour in emitted IR is invisible above and appears
# here. `KEL_OPTIMIZE` turns the shipping middle end on for every lowering.
#
# **It existed, was documented, and no script set it** until 2026-09-17 -- so the
# corpus had been optimised and VALIDATED, and never optimised and RUN. A run
# nobody performs proves nothing, which is why this is a phase and not a note.
#
# **Measured on 2026-09-17**, and the cost is why only half of it is here:
#
#   this phase (everything but the corpus)   155s optimised vs ~175s at -O0
#   the corpus differential                  420s optimised vs ~380-420s at -O0
#
# The first is free. The second would DOUBLE the gate's worst phase -- the one
# that already had to be split to fit the harness ceiling under `narrow-float-32`
# -- so **the corpus half stays a manual sweep**:
#
#   KEL_OPTIMIZE=1 tools/frozen-run.sh cargo nextest run -E 'binary(corpus_differential)'
#
# This phase covers every hand-written differential and all the generated scalar,
# byte, composite and nesting programs, which are the densest subjects here.
#
# ⚠ The variable is scoped to THIS COMMAND, so no other phase's meaning changes.
echo "-- suite, part 3 of 3 (everything but corpus_differential, UNDER THE MIDDLE END)"
KEL_OPTIMIZE=1 tools/frozen-run.sh cargo nextest run --no-fail-fast "${feat[@]}" \
    -E 'not binary(corpus_differential)' || fail=1

echo "================ BACKEND GATE — $label: $([ $fail -eq 0 ] && echo PASS || echo FAIL)"
exit $fail
