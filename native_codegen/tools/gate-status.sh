#!/usr/bin/env bash
# Report what the backend gate has on record, and whether it still speaks to HEAD.
#
# THE QUESTION THIS ANSWERS, AND THE ONE IT REFUSES TO
#
# It does NOT ask "does the recorded commit equal HEAD?". That comparison is
# uninformative and would be false almost always: committing a record produces a
# commit whose parent is the verified one, so the record can never name the commit
# that carries it. A check built on that would be red from the moment it was
# written, and a normally-red check teaches its reader to ignore it.
#
# The useful question is RELEVANCE: have any backend sources changed since the tree
# that was verified? Zero means the recorded verdict still describes this code, even
# though the hashes differ. Nonzero names the files that have gone unverified.
#
# `GATE_RECORD.md` is excluded throughout: the record is not code under test.
#
# Usage:  tools/gate-status.sh [--exit-zero]
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

record="GATE_RECORD.md"

# EVERY PATH THE SUITE READS, not merely the package the suite lives in.
#
# Established on 2026-09-24 by grepping the tests for external paths, after the
# handoff alone was added and the same argument was noticed to apply more widely. The
# differential's SUBJECTS are `../examples/scripts/*.kel`: a corpus file could change
# and a reach computed over `native_codegen/` alone would not see it, while reporting
# that the record still spoke to HEAD. At least eight test files read these.
#
# `../src` is here for a reason found on 2026-09-24 while building the census guard:
# `stage_differential.rs` reads `../src/selfhost/kel/*.kel`, and other tests read
# `../src/bytecode.rs`, `compiler.rs`, `vm.rs` and `wire_format.rs`. Those are the
# stage differential's SUBJECTS, and the first hand-built set missed them.
#
# ⚠ **HOW OFTEN EACH MEMBER ACTUALLY FIRES, measured over the 200 most recent commits
# on `origin/v0.2.3` on 2026-09-25** -- because an earlier version of this comment
# asserted an activity ranking instead of measuring one, and the measurement refuted it:
#
#   docs/process/REVERSE_PROMPT.md  84/200     src                       19/200
#   docs/decisions                  34/200     src/selfhost/kel           7/200
#   examples                         0/200     compiler/kel               0/200
#
# A 30-commit sample gave the same ordering, and the 200-commit figures supersede it as
# the better estimate. The LOUDEST contributor is the least consequential: a
# `REVERSE_PROMPT.md` edit only feeds a documentation guard, while the stage sources --
# whose change can alter a VERDICT -- fire at 7 in 200. This is why the reporter NAMES
# the paths rather than only counting them: a reader judges `REVERSE_PROMPT.md`
# differently from `src/selfhost/kel`, and cannot if the output is a number.
#
# ✅ **AND THE SET IS VALIDATED AGAINST REAL WORK, not only by perturbation.** The four
# most recent commits to touch each high-stakes member are all flagged by this set, and
# `e90d71e4` is flagged by exactly one file, `src/selfhost/kel/parse.kel`. So a real
# change by the other line to the differential's own subjects does reach this list. That
# validates the PATH SET; it is not an end-to-end test of the reporter in a live
# absorption, which is a weaker claim and the accurate one.
#
# ⚠ `../examples` is deliberately BROAD rather than the two corpus subdirectories it
# replaced, and the reason is a blind spot in the census that watches this set. Four
# names -- `examples`, `src`, `tests`, `tools` -- exist BOTH inside this package and at
# the repository root, so a literal like `"examples/scripts"` cannot be classified
# statically: `common/mod.rs` joins exactly that against `".."`, making it
# repo-relative, while another file resolves the same spelling package-locally. The
# census therefore cannot see those literals. **Breadth is the mitigation**: a new
# corpus root added under `examples/` or `src/` is covered even though unseen. Measured
# 2026-09-24, `examples/` broad was touched 0 of the other line's 30 most recent
# commits, so the breadth costs nothing in noise.
#
# Root `tests/` and `tools/` are deliberately absent: no test reads them, so watching
# them would add warnings with no signal behind them.
#
# So the LOUDEST contributor is the least consequential one: a `REVERSE_PROMPT.md` edit
# only feeds a documentation guard, while the stage sources and corpus -- the inputs
# whose change could alter a VERDICT -- were untouched across the whole sample. This is
# why the reporter NAMES the paths rather than only counting them: a reader who sees
# `REVERSE_PROMPT.md` judges it differently from `examples/scripts`, and could not if
# the output were a number.
#
# Several of these belong to the `v0.2.3` line. That is a FEATURE: absorbing its
# commits can change a stage source, a corpus script or a decision document, and the
# record should then report itself unverified, because it is.
REACH=(
    .
    ../docs/process/handoffs/v0.3.0.md
    ../docs/process/REVERSE_PROMPT.md
    ../docs/decisions
    ../src
    ../examples
    ../compiler/kel
)
# Exit 1 means no current verified PASS. Diagnostics distinguish recorded FAIL
# from invalid, incomplete or stale evidence.
# --exit-zero is for display-only callers and does not suppress diagnostics.
exit_zero=false
case "${1:-}" in
    '') ;;
    --exit-zero) exit_zero=true; shift ;;
    *) echo "Usage: $0 [--exit-zero]" >&2; exit 2 ;;
esac
if [ "$#" -ne 0 ]; then
    echo "Usage: $0 [--exit-zero]" >&2
    exit 2
fi
finish() {
    if "$exit_zero"; then exit 0; fi
    exit "$1"
}

# Exclude only the package's record, including when it is uncommitted. A source
# with GATE_RECORD.md in its name must still invalidate the measurement.
exclude=':(top,exclude)native_codegen/GATE_RECORD.md'
if ! head_commit=$(git rev-parse --verify HEAD 2>/dev/null); then
    echo "UNVERIFIED: cannot resolve HEAD"
    finish 1
fi
echo "HEAD is $head_commit"
status=0
if ! now_dirt=$(git status --porcelain --untracked-files=all -- "${REACH[@]}" "$exclude"); then
    echo "UNVERIFIED: cannot inspect the worktree"
    finish 1
fi
if [ -n "$now_dirt" ]; then
    echo "UNVERIFIED: backend inputs are UNCOMMITTED. No record speaks to this worktree."
    printf '%s\n' "$now_dirt"
    status=1
fi

if [ ! -r "$record" ]; then
    echo "UNVERIFIED: no readable gate record exists"
    finish 1
fi

# Only six-field rows establish phase freezing. Legacy five-field records remain
# historical evidence, but cannot establish a clean/frozen/PASS result.
if ! rows=$(awk -F '|' '
    /^\| (default features|narrow-float-32) \|/ {
        if (NF != 8) { bad=1; next }
        for (i=2; i<=7; i++) gsub(/^[[:space:]]+|[[:space:]]+$/, "", $i)
        seen[$2]++
        if (seen[$2] != 1 || length($3) != 40 || $3 ~ /[^0-9a-fA-F]/ ||
            ($4 != "clean" && $4 !~ /^dirty\([1-9][0-9]*\)$/) ||
            ($5 != "frozen" && $5 !~ /^MOVED\([1-9][0-9]*\)$/) ||
            ($6 != "PASS" && $6 != "FAIL") ||
            $7 !~ /^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]Z$/ ||
            $1 !~ /^[[:space:]]*$/ || $8 !~ /^[[:space:]]*$/) bad=1
        print $2 "|" $3 "|" $4 "|" $5 "|" $6 "|" $7
    }
    END { if (bad || seen["default features"] != 1 || seen["narrow-float-32"] != 1) exit 1 }
' "$record"); then
    echo "UNVERIFIED: invalid gate record schema; require one valid six-column row per configuration"
    finish 1
fi

while IFS='|' read -r cfg commit tree phases verdict when; do
    echo ""
    echo "$cfg"
    echo "   verdict  $verdict on a $tree worktree, $phases phases, $when"
    echo "   commit   $commit"
    if ! git cat-file -e "${commit}^{commit}" 2>/dev/null; then
        echo "   UNVERIFIED: recorded commit is not in this repository"
        status=1
        continue
    fi
    row_verified=true
    if [ "$verdict" = "FAIL" ]; then
        echo "   FAILED: $cfg recorded FAIL at $commit"
        row_verified=false
        status=1
    fi
    if [ "$tree" != "clean" ]; then
        echo "   UNVERIFIED: recorded run used a $tree worktree"
        row_verified=false
        status=1
    fi
    if [ "$phases" != "frozen" ]; then
        echo "   UNVERIFIED: recorded phases were $phases"
        row_verified=false
        status=1
    fi
    if ! changed=$(git diff --name-only "$commit" HEAD -- "${REACH[@]}" "$exclude"); then
        echo "   UNVERIFIED: cannot compare recorded inputs with HEAD"
        status=1
        continue
    fi
    if [ -n "$changed" ]; then
        echo "   reach    UNVERIFIED: backend inputs changed since the recorded run"
        printf '%s\n' "$changed" | sed 's/^/              /'
        status=1
    elif [ -n "$now_dirt" ]; then
        echo "   reach    committed inputs match, but this worktree is UNVERIFIED"
    elif ! "$row_verified"; then
        echo "   reach    no committed input change; recorded run is not a verified PASS"
    else
        echo "   reach    still speaks to HEAD: no backend source changed since"
    fi
done <<< "$rows"
finish "$status"
