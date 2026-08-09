#!/usr/bin/env bash
# gate-in-worktree.sh — run the full pre-merge gate against an IMMUTABLE commit
# in its own worktree, so development continues in the main tree meanwhile.
#
# WHY THIS EXISTS
#
# `scripts/release-gate.sh` reads the working tree. Running it directly freezes
# development for its whole duration (~2h33m), because any edit mid-run makes the
# result correspond to no commit at all. That serial dead time was the single
# largest calendar-time cost in the loop.
#
# Running the gate in a detached worktree pinned to a commit fixes both halves:
#
#   - The main tree is free the entire time. Slice N+1 is developed while slice N
#     gates.
#   - The gate result is pinned to an immutable commit BY CONSTRUCTION. The rule
#     "a gate result is valid only for the tip it ran against" stops being a
#     discipline anyone has to remember and becomes a property of the mechanism.
#     That is the shape PROCESS_STRATEGY.md argues for.
#
# ON THE PERFORMANCE CANARY, which is the obvious objection
#
# `tests/perf_canary.rs` wants a quiet machine, and this deliberately introduces
# concurrent load. That is acceptable, and the reason is directional rather than
# a judgement call: **load can only make the canary slower.** So it can produce a
# FALSE POSITIVE, which costs one re-run, and it cannot produce a false negative,
# which is the failure that would actually matter. A regression stays visible
# under load; only a clean run can be wrongly accused.
#
# If it fires: re-run alone before profiling, exactly as the existing failure
# message says.
#
# USAGE
#   scripts/gate-in-worktree.sh                 # gate HEAD
#   scripts/gate-in-worktree.sh <commit-ish>    # gate a specific commit
#   scripts/gate-in-worktree.sh --miri          # pass flags through to the gate
#
# The log path and the resolved commit are printed at the start so a background
# run can be monitored.
set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT="$(pwd)"

# First non-flag argument is the commit-ish; everything else passes through.
COMMITISH="HEAD"
SETUP_ONLY=0
PASSTHROUGH=()
for arg in "$@"; do
  case "$arg" in
    # Prepare and verify the worktree, then stop without running the gate.
    # Exists so the setup path — which is the part that could silently gate the
    # wrong commit — can be exercised without paying for a 2.5-hour run.
    --setup-only) SETUP_ONLY=1 ;;
    -*) PASSTHROUGH+=("$arg") ;;
    *)  COMMITISH="$arg" ;;
  esac
done

COMMIT="$(git rev-parse --verify "${COMMITISH}^{commit}")"
SHORT="$(git rev-parse --short "$COMMIT")"

TREES_DIR="${KEL_WORKTREES_DIR:-"$REPO_ROOT/../keleusma-worktrees"}"
mkdir -p "$TREES_DIR"
# Normalise before deriving any path from it. `git worktree list` reports
# RESOLVED paths, so an unnormalised `<repo>/../keleusma-worktrees/gate` never
# matches the reuse check and the script tries to re-create an existing tree.
# That failed on the second invocation, which is the common case, and was caught
# only because `--setup-only` made a second invocation cheap to try.
TREES_DIR="$(cd "$TREES_DIR" && pwd -P)"
GATE_DIR="$TREES_DIR/gate"
# A target directory OUTSIDE the worktree, and stable across runs, so the build
# cache survives and only changed crates recompile. Inside the worktree it would
# be discarded with the tree; shared with the main tree it would thrash, because
# the two are usually on different commits.
GATE_TARGET="${KEL_GATE_TARGET:-"$TREES_DIR/.gate-target"}"

# Reuse the worktree across runs; just move it to the commit under test. A stale
# tree from an interrupted run is reset rather than refused, since the whole
# point is that its contents are disposable.
if git worktree list --porcelain | grep -qx "worktree $GATE_DIR"; then
  git -C "$GATE_DIR" reset --hard --quiet
  git -C "$GATE_DIR" clean -fdq
  git -C "$GATE_DIR" checkout --detach --quiet "$COMMIT"
else
  git worktree add --detach --quiet "$GATE_DIR" "$COMMIT"
fi

# Confirm the tree really is at the requested commit and carries no local edits.
# Gating something other than what was asked for is precisely the failure this
# script exists to make impossible, so it is checked rather than assumed.
ACTUAL="$(git -C "$GATE_DIR" rev-parse HEAD)"
[ "$ACTUAL" = "$COMMIT" ] || { echo "gate-worktree: tree is at $ACTUAL, expected $COMMIT" >&2; exit 1; }
if [ -n "$(git -C "$GATE_DIR" status --porcelain)" ]; then
  echo "gate-worktree: worktree is dirty after checkout; refusing" >&2
  git -C "$GATE_DIR" status --short >&2
  exit 1
fi

LOG="${KEL_GATE_LOG:-"$TREES_DIR/gate-$SHORT.log"}"

cat <<EOF
gate-worktree: commit  $SHORT ($(git log -1 --format=%s "$COMMIT" | cut -c1-60))
gate-worktree: tree    $GATE_DIR
gate-worktree: target  $GATE_TARGET
gate-worktree: log     $LOG
gate-worktree: the main tree is free; develop there while this runs.
EOF

if [ "$SETUP_ONLY" -eq 1 ]; then
  echo "gate-worktree: --setup-only, stopping before the gate."
  exit 0
fi

cd "$GATE_DIR"
CARGO_TARGET_DIR="$GATE_TARGET" scripts/release-gate.sh "${PASSTHROUGH[@]+"${PASSTHROUGH[@]}"}" 2>&1 | tee "$LOG"
status=${PIPESTATUS[0]}

if [ "$status" -eq 0 ]; then
  echo "gate-worktree: GREEN for $SHORT"
else
  echo "gate-worktree: RED for $SHORT (exit $status) — see $LOG" >&2
fi
exit "$status"
