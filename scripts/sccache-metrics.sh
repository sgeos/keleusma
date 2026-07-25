#!/usr/bin/env bash
# sccache-metrics.sh — append a timestamped one-line sccache summary to a
# gitignored log, so cache effectiveness is captured incidentally as development
# proceeds. GUARDED: a no-op when sccache is absent, so it is safe in CI and for
# contributors who have not opted into sccache. Part of the 2026-07-24 sccache
# trial; if the trial is reverted, delete this script and the two guarded call
# sites in fast-check.sh and release-gate.sh.
#
#   scripts/sccache-metrics.sh [label]   # append one data point, tagged [label]
#
# The log lives at tmp/sccache-metrics.log (tmp/ is gitignored). sccache counts
# are cumulative since the server started or since the last `sccache --zero-stats`.
set -uo pipefail
command -v sccache >/dev/null 2>&1 || exit 0
cd "$(dirname "$0")/.." || exit 0
LOG="${SCCACHE_METRICS_LOG:-tmp/sccache-metrics.log}"
label="${1:-}"

stats="$(sccache --show-stats 2>/dev/null)" || exit 0
field() { printf '%s\n' "$stats" | grep -iE "^$1" | head -1 | awk '{print $NF}'; }
req="$(field 'Compile requests +[0-9]')"
exe="$(field 'Compile requests executed')"
hit="$(field 'Cache hits +[0-9]')"
mis="$(field 'Cache misses +[0-9]')"
nc="$(field 'Non-cacheable calls')"
rate="$(printf '%s\n' "$stats" | grep -iE '^Cache hits rate +[0-9]' | head -1 | awk '{print $(NF-1)}')"
ts="$(date '+%Y-%m-%dT%H:%M')"

mkdir -p "$(dirname "$LOG")"
printf '%s  requests=%s executed=%s hits=%s misses=%s noncacheable=%s hit_rate=%s%%  %s\n' \
  "$ts" "${req:-?}" "${exe:-?}" "${hit:-?}" "${mis:-?}" "${nc:-?}" "${rate:-?}" "$label" >> "$LOG"
tail -1 "$LOG"
