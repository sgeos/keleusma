#!/bin/sh
#
# Summarise a saved `release-gate.sh` log: test binaries and test cases, PER STEP.
#
# # Why this exists
#
# The gate runs the suite under several feature sets. Its log therefore contains
# several independent `test result:` sequences, one per step, and a naive
# `awk '/^test result:/{p+=$4}'` over the whole file sums ACROSS them and
# produces a number that is not a test count for anything.
#
# That is not hypothetical. On 2026-08-31 exactly that one-liner reported "179
# binaries and 2904 tests" for a default-features pass that is 113 and 2708, and
# the wrong figure reached a commit message and a merged pull-request body before
# a comparison between two gate runs refused to reconcile and exposed it.
#
# The gate log invites the mistake -- nine sections, no summary -- so the remedy
# is one correct reader rather than a warning telling people to be careful.
#
# Usage: gate-summary.sh <path-to-gate-log>
# Exit 0 on success, 1 if the log has no recognisable structure.

set -eu

log="${1:-}"
if [ -z "$log" ] || [ ! -f "$log" ]; then
    echo "usage: $0 <path-to-gate-log>" >&2
    exit 1
fi

# The step headers carry ANSI attributes; strip them before matching.
awk '
    { line = $0; gsub(/\033\[[0-9;]*m/, "", line) }

    line ~ /^=== / {
        if (step != "") printf "  %-58s %4d binaries %6d tests\n", step, bins, cases
        step = line
        sub(/^=== /, "", step); sub(/ ===$/, "", step)
        bins = 0; cases = 0
        steps++
        next
    }

    line ~ /^test result:/ {
        # `test result: ok. N passed; M failed; ...`
        for (i = 1; i <= NF; i++) if ($i == "passed;") { cases += $(i-1); break }
        bins++
        total_bins++; total_cases++  # counted for the non-vacuity check only
        next
    }

    END {
        if (step != "") printf "  %-58s %4d binaries %6d tests\n", step, bins, cases
        if (steps == 0) { print "no step headers found; this is not a gate log" > "/dev/stderr"; exit 1 }
        if (total_bins == 0) { print "no test results found in any step" > "/dev/stderr"; exit 1 }
        printf "\n  %d step(s). PER-STEP figures above are the quotable ones.\n", steps
        print  "  A total across steps is not a test count: the same suite runs under several"
        print  "  feature sets, so summing them counts most tests more than once."
    }
' "$log"
