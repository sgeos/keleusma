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
# Parallel to `seen_names`: that gate's PREVIOUS completed step count, 0 if it
# has no history. Parallel indexed arrays rather than one associative array,
# because macOS ships bash 3.2 and `declare -A` is a bash 4 feature.
declare -a prev_steps=()
# Parallel again: the predecessor's commit, and whether it reached a verdict.
# A run with no verdict line was ABANDONED — replaced by a newer run on another
# commit rather than finished — and the newest-per-name rule below would
# otherwise erase every trace of it.
declare -a prev_commit=()
declare -a prev_done=()
# Whether the IMMEDIATE predecessor has been captured yet. Separate from
# `prev_steps`, which deliberately skips incomplete runs — see below.
declare -a prev_seen=()
# Where the immediate predecessor stopped, when it was abandoned.
declare -a prev_abandoned_at=()
rows=()

while IFS= read -r log; do
  [ -n "$log" ] || continue
  base=$(basename "$log" .log)
  name=${base%-*}
  commit=${base##*-}

  # HOW MANY STEPS WILL THIS GATE HAVE? Derived from the SAME GATE'S PREVIOUS
  # COMPLETED RUN, because it cannot be known any other way while a run is in
  # progress.
  #
  # Not from the script: `release-gate.sh` has fifteen `step` call sites for a
  # twelve- or thirteen-step run, because the `native_codegen/` step sits inside
  # a conditional with two mutually exclusive calls. Counting them was the first
  # thing tried and it is simply wrong.
  #
  # Not from the gate NAME either, which would work and is refused: that is the
  # by-name enumeration this repository has been bitten by five times.
  #
  # The previous run of the same gate is the honest source. `ls -t` gives newest
  # first, so the second log for a name is the run before this one; a gate that
  # gained or lost a step self-corrects after one completed run, and a gate with
  # no history falls back to the table length.
  idx=-1 i=0
  for n in ${seen_names[@]+"${seen_names[@]}"}; do [ "$n" = "$name" ] && idx=$i; i=$((i+1)); done
  if [ "$idx" -ge 0 ]; then
    finished=0
    grep -aqE 'release gate: (GREEN|RED)' "$log" 2>/dev/null && finished=1
    # THE EXPECTED STEP COUNT MUST COME FROM A COMPLETED RUN. An abandoned one
    # stopped wherever it stopped, so taking its count would scale every later
    # bar to that number — a gate killed during step 3 would peg the bar at 3.
    # Keep looking down the list until a finished run turns up. Caught the
    # moment the abandoned-run report below was added: it printed "ABANDONED at
    # step 13" and the same 13 was silently feeding `bar_for`.
    if [ "${prev_steps[$idx]}" -eq 0 ] 2>/dev/null && [ "$finished" -eq 1 ]; then
      prev_steps[$idx]=$(grep -aoE '=== [^=]+ ===' "$log" 2>/dev/null | grep -avc 'release gate:')
    fi
    # The abandoned report is about the IMMEDIATE predecessor only, so it is
    # captured once and not overwritten by older runs.
    if [ "${prev_seen[$idx]}" -eq 0 ] 2>/dev/null; then
      prev_seen[$idx]=1
      prev_commit[$idx]=$commit
      prev_done[$idx]=$finished
      [ "$finished" -eq 0 ] && prev_abandoned_at[$idx]=$(grep -aoE '=== [^=]+ ===' "$log" 2>/dev/null | grep -avc 'release gate:')
    fi
    continue
  fi
  seen_names+=("$name")
  prev_steps+=(0)
  prev_commit+=("")
  prev_done+=(1)
  prev_seen+=(0)
  prev_abandoned_at+=(0)

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

  # The name index rides along so the render pass can look up `prev_steps`,
  # which a LATER iteration fills in — the older log for this name has not been
  # read yet at this point.
  rows+=("$name|$commit|$state|$steps|$fails|$age|$(( ${#seen_names[@]} - 1 ))|$step")
done < <(ls -t "$TREES_DIR"/*.log 2>/dev/null)

[ ${#rows[@]} -eq 0 ] && { [ "$oneline" -eq 1 ] || echo "gate-status: no gate logs"; exit 0; }

# Per-step weights, as PERCENTAGES OF MEASURED TEST TIME, over the MEDIAN of
# three completed 12-step runs (`wire-corpus` at `9eb623d`, `c2a833d`, `3ad895e`
# — 5132 s, 5824 s and 4644 s of test time).
#
# RECALIBRATED 2026-08-11, AND THE REASON IS THE POINT. The first table was taken
# from a SINGLE run, `wire-corpus-11c5d9d` at 12,594 s, and that run turns out to
# be a 2.4x outlier — it executed under concurrent gate load. Its per-step shape
# is correspondingly distorted:
#
#   step        3    5    6    7   12
#   11c5d9d    8%  23%  32%  23%  13%     <- the old table's source
#   9eb623d   20%  21%  22%  20%  18%
#   c2a833d   18%  24%  26%  17%  16%
#   3ad895e   21%  20%  20%  19%  19%
#
# The three recent runs agree within a few points; the outlier understates step 3
# by roughly two and a half times. The visible symptom was a bar that sat
# pessimistically low through the early heavy steps and then jumped.
#
# ONE SAMPLE IS NOT A CALIBRATION. That is the whole lesson here, and it applies
# to whoever extends this next: take the median of several runs on a quiet
# machine, and say which runs, so the next person can tell an outlier from a
# change in the work.
#
# WHY WEIGHTED AND NOT UNIFORM. Four steps carry about 80% of the test time and
# eight carry almost none. A uniform bar would read ~50% within three minutes and
# then crawl for hours, which is worse than no bar: it would invite exactly the
# wrong estimate of time remaining.
#
# Ordinal, not name-keyed, because the names are long and change wording more
# readily than the order changes. If the gate grows or loses a step the table no
# longer matches, so `bar_for` falls back to uniform rather than silently
# mis-weighting — the failure this file exists to avoid, applied to itself.
#
# NO 13-STEP TABLE, DELIBERATELY. The `v0.3.0` gate carries an extra
# `native_codegen/` step and asked for a thirteenth entry. Two reasons it is not
# here: only ONE completed 13-step run exists, and the paragraph above is about
# exactly that mistake — a table from a single sample.
#
# Such a gate is instead drawn UNIFORM OVER ITS OWN KNOWN LENGTH, via the
# `expected` count `bar_for` receives. Coarse, but never confidently wrong.
# That count comes from the same gate's previous completed run, because it is
# not recoverable from the script: the `native_codegen/` step sits inside a
# conditional with two mutually exclusive `step` calls, so there are 15 call
# sites for a 12- or 13-step run. Keying on the gate NAME would also work and is
# refused — that is the by-name enumeration this repository has been bitten by
# five times.
#
# When several quiet 13-step runs exist, the right change is a SECOND table
# selected by `expected`, not a thirteenth entry appended to this one, which
# would mis-weight the 12-step gate.
WEIGHTS=(1 1 20 1 21 22 19 1 1 1 1 18)

# Weighted completion bar. A finished gate is full; otherwise completed steps
# count in full and the CURRENT step counts a half, since intra-step progress is
# not observable from the log. Half is a midpoint estimator, deliberately never
# reaching 100 before the verdict does.
bar_for() {
  local steps=$1 state=$2 cells=${3:-10} expected=${4:-0} pct=0 i total=0 done_w=0
  if [ "$state" = "GREEN" ] || [ "$state" = "RED" ]; then
    pct=100
  elif [ "$steps" -le 0 ]; then
    pct=0
  elif [ "$expected" -gt 0 ] && [ "$expected" -ne "${#WEIGHTS[@]}" ]; then
    # THIS GATE IS NOT THE ONE THE TABLE WAS MEASURED ON. Uniform over its own
    # known length, which is coarse but never confidently wrong — and far better
    # than applying a 12-step shape to a 13-step gate for its first twelve steps,
    # which is what happened before the expected count was derived.
    [ "$steps" -gt "$expected" ] && steps=$expected
    pct=$(( (2 * steps - 1) * 100 / (2 * expected) ))
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
    IFS='|' read -r name commit state steps fails age idx step <<<"$r"
    expected=${prev_steps[$idx]:-0}
    # A finished gate is not news after a while; only surface live ones and
    # anything that failed or stopped writing.
    if [ "$state" = "GREEN" ] && [ "$age" -gt 3600 ]; then continue; fi
    case "$state" in
      RUNNING) mark="▸";; GREEN) mark="✓";; RED) mark="✗";; STALLED) mark="⏸";;
    esac
    seg="$mark $name@$commit $(bar_for "$steps" "$state" 6 "$expected")"
    [ "$fails" -gt 0 ] && seg="$seg !$fails"
    [ "$state" = "RUNNING" ] && seg="$seg $(fmt_age "$age")"
    [ "$state" = "STALLED" ] && seg="$seg quiet $(fmt_age "$age")"
    out="${out:+$out  }$seg"
  done
  [ -n "$out" ] && printf '%s' "$out"
  exit 0
fi

for r in "${rows[@]}"; do
  IFS='|' read -r name commit state steps fails age idx step <<<"$r"
  expected=${prev_steps[$idx]:-0}
  printf '%-14s %-8s %-8s %-16s steps=%-3s failures=%-3s last write %s ago\n' \
    "$name" "$commit" "$state" "$(bar_for "$steps" "$state" 10 "$expected")" "$steps" "$fails" "$(fmt_age "$age")"
  [ -n "$step" ] && printf '    step: %s\n' "$step"
  # AN ABANDONED PREDECESSOR IS NEWS, and without this line it is invisible.
  # The newest-per-name rule exists so a gate shows one row, but it also erases
  # a run that was replaced mid-flight. That cost real time: a run stopped at
  # step 12 with no verdict, vanished from this display the moment its
  # replacement started, and I read "not running" as "finished" — armed a waiter
  # on a verdict that could never arrive, and ran a test suite into the new
  # run's reopened canary window. Neither mistake was possible to notice from
  # the output above.
  if [ "${prev_done[$idx]:-1}" -eq 0 ]; then
    printf '    previous: %s ABANDONED at step %s (no verdict) — a wait on it will never end\n' \
      "${prev_commit[$idx]}" "${prev_abandoned_at[$idx]}"
  fi
done
