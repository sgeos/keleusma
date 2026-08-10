#!/usr/bin/env bash
# gate-status.sh — report every gate this repository knows about, compactly.
#
# WHY THIS EXISTS
#
# A full gate runs 2 to 3.5 hours in a detached worktree, and with two sessions
# on one machine there can be more than one. Answering "is it still going, and
# where is it" was being done by ad-hoc `pgrep` and `grep` at the prompt, and
# that produced three separate defects in a single day:
#
#   1. `pgrep -f "release-gate.sh"` MATCHES ITS OWN SHELL. Any command line
#      containing that literal string matches, so a waiter loop written as
#      `until ! pgrep -f "release-gate.sh"` never exits. That deadlocked a
#      sibling session for hours and blocked gating for both. **This script
#      therefore never greps for the script name.**
#   2. A step-header regex bounded to 70 characters silently skipped the
#      71-character thirteenth step, so progress read "11 of 12" forever.
#      **The header pattern here is unbounded.**
#   3. `cargo test ... | tail` in a background command reports TAIL's exit
#      status, not cargo's. Not this script's problem directly, but the same
#      family: a convenience that quietly answers a different question.
#
# HOW LIVENESS IS DECIDED, AND WHY NOT BY PROCESS
#
# By the LOG's modification time, plus the presence of a verdict line. A running
# gate writes continuously, so an untouched log means finished or stalled, and
# the verdict line distinguishes them. This needs no process lookup at all,
# which is what makes defect 1 unreachable by construction rather than by
# remembering.
#
# It is also cheap enough for a status line: a stat and a tail per log, no git,
# no cargo, no process table.
#
# USAGE
#   scripts/gate-status.sh              # one block per gate
#   scripts/gate-status.sh --oneline    # a single line, for a status line
set -uo pipefail

# Seconds without a write before a running-looking gate is called STALLED.
STALE_AFTER=${KEL_GATE_STALE_AFTER:-240}

REPO_ROOT="$(cd "$(dirname "$(git rev-parse --git-common-dir 2>/dev/null || echo .)")" && pwd -P)"
TREES_DIR="${KEL_WORKTREES_DIR:-"$REPO_ROOT/../keleusma-worktrees"}"
[ -d "$TREES_DIR" ] || { [ "${1:-}" = "--oneline" ] || echo "gate-status: no worktrees dir"; exit 0; }
TREES_DIR="$(cd "$TREES_DIR" && pwd -P)"

now=$(date +%s)
oneline=0
[ "${1:-}" = "--oneline" ] && oneline=1

# Newest log per gate NAME. A name may have many logs, one per gated commit;
# only the newest is the current run.
declare -a seen_names=()
rows=()

while IFS= read -r log; do
  [ -n "$log" ] || continue
  base=$(basename "$log" .log)
  name=${base%-*}
  commit=${base##*-}

  already=0
  for n in ${seen_names[@]+"${seen_names[@]}"}; do [ "$n" = "$name" ] && already=1; done
  [ "$already" -eq 1 ] && continue
  seen_names+=("$name")

  mtime=$(stat -f %m "$log" 2>/dev/null || stat -c %Y "$log" 2>/dev/null) || continue
  age=$(( now - mtime ))

  # UNANCHORED and UNBOUNDED. The header is wrapped in ANSI escapes, so an
  # anchored pattern misses it entirely — which this script did on its first
  # run, reporting steps=0 for a gate that had run thirteen. Same family as the
  # 70-character cap in note 2 above: a pattern that is too specific fails
  # silently and reads as "nothing happened".
  # The VERDICT line is also wrapped in `=== ... ===`, so counting headers
  # naively over-reports by one on a finished gate. I reported "13 steps" for a
  # 12-step gate before this script made the off-by-one visible.
  step=$(grep -aoE '=== [^=]+ ===' "$log" 2>/dev/null | grep -av 'release gate:' \
         | tail -1 | sed -E 's/^=== //; s/ ===$//' | cut -c1-46)
  steps=$(grep -aoE '=== [^=]+ ===' "$log" 2>/dev/null | grep -avc 'release gate:')
  fails=$(grep -acE 'test result: FAILED|panicked at' "$log" 2>/dev/null)

  if grep -aq 'release gate: GREEN' "$log" 2>/dev/null; then
    state=GREEN
  elif grep -aq 'release gate: RED' "$log" 2>/dev/null; then
    state=RED
  elif [ "$age" -le "$STALE_AFTER" ]; then
    state=RUNNING
  else
    state=STALLED
  fi

  rows+=("$name|$commit|$state|$steps|$fails|$age|$step")
done < <(ls -t "$TREES_DIR"/*.log 2>/dev/null)

[ ${#rows[@]} -eq 0 ] && { [ "$oneline" -eq 1 ] || echo "gate-status: no gate logs"; exit 0; }

# Per-step weights, as PERCENTAGES OF MEASURED TEST TIME, taken from a completed
# 12-step run (`wire-corpus-11c5d9d`, 12,594 s of test time).
#
# WHY WEIGHTED AND NOT UNIFORM. Four steps carry 91% of the wall clock and eight
# carry about 9%. A uniform bar would read ~50% within three minutes and then
# crawl for three hours, which is worse than no bar: it would invite exactly the
# wrong estimate of time remaining.
#
# Ordinal, not name-keyed, because the names are long and change wording more
# readily than the order changes. If the gate grows or loses a step the table no
# longer matches, so `bar_for` falls back to uniform rather than silently
# mis-weighting — the failure this file exists to avoid, applied to itself.
WEIGHTS=(1 1 8 1 23 32 23 1 1 1 1 13)

# Weighted completion bar. A finished gate is full; otherwise completed steps
# count in full and the CURRENT step counts a half, since intra-step progress is
# not observable from the log. Half is a midpoint estimator, deliberately never
# reaching 100 before the verdict does.
bar_for() {
  local steps=$1 state=$2 cells=${3:-10} pct=0 i total=0 done_w=0
  if [ "$state" = "GREEN" ] || [ "$state" = "RED" ]; then
    pct=100
  elif [ "$steps" -le 0 ]; then
    pct=0
  elif [ "$steps" -gt "${#WEIGHTS[@]}" ]; then
    # Step table is stale; degrade to uniform rather than mis-weight.
    pct=$(( 100 * steps / (steps + 1) ))
  else
    for ((i=0; i<${#WEIGHTS[@]}; i++)); do total=$(( total + WEIGHTS[i] )); done
    for ((i=0; i<steps-1; i++)); do done_w=$(( done_w + WEIGHTS[i] )); done
    # ×2 throughout so the current step's half-weight stays integral.
    pct=$(( (2 * done_w + WEIGHTS[steps-1]) * 100 / (2 * total) ))
  fi
  local filled=$(( (pct * cells + 50) / 100 ))
  [ "$filled" -gt "$cells" ] && filled=$cells
  [ "$filled" -lt 0 ] && filled=0
  local bar="" j
  for ((j=0; j<cells; j++)); do
    if [ "$j" -lt "$filled" ]; then bar="${bar}▰"; else bar="${bar}▱"; fi
  done
  printf '%s %d%%' "$bar" "$pct"
}

fmt_age() { local a=$1; if [ "$a" -lt 60 ]; then echo "${a}s"; elif [ "$a" -lt 3600 ]; then echo "$((a/60))m"; else echo "$((a/3600))h$(( (a%3600)/60 ))m"; fi; }

if [ "$oneline" -eq 1 ]; then
  out=""
  for r in "${rows[@]}"; do
    IFS='|' read -r name commit state steps fails age step <<<"$r"
    # A finished gate is not news after a while; only surface live ones and
    # anything that failed or stopped writing.
    if [ "$state" = "GREEN" ] && [ "$age" -gt 3600 ]; then continue; fi
    case "$state" in
      RUNNING) mark="▸";; GREEN) mark="✓";; RED) mark="✗";; STALLED) mark="⏸";;
    esac
    seg="$mark $name@$commit $(bar_for "$steps" "$state" 6)"
    [ "$fails" -gt 0 ] && seg="$seg !$fails"
    [ "$state" = "RUNNING" ] && seg="$seg $(fmt_age "$age")"
    [ "$state" = "STALLED" ] && seg="$seg quiet $(fmt_age "$age")"
    out="${out:+$out  }$seg"
  done
  [ -n "$out" ] && printf '%s' "$out"
  exit 0
fi

for r in "${rows[@]}"; do
  IFS='|' read -r name commit state steps fails age step <<<"$r"
  printf '%-14s %-8s %-8s %-16s steps=%-3s failures=%-3s last write %s ago\n' \
    "$name" "$commit" "$state" "$(bar_for "$steps" "$state" 10)" "$steps" "$fails" "$(fmt_age "$age")"
  [ -n "$step" ] && printf '    step: %s\n' "$step"
done
