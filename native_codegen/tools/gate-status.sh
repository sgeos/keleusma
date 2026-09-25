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
# Usage:  tools/gate-status.sh
set -uo pipefail
cd "$(dirname "$0")/.."

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
# ⚠ **HOW OFTEN EACH MEMBER ACTUALLY FIRES, measured over the 30 most recent commits
# on `origin/v0.2.3` on 2026-09-24** -- because the first version of this comment
# asserted an activity ranking instead of measuring one, and the measurement refuted it:
#
#   docs/process/REVERSE_PROMPT.md  14/30      src                        3/30
#   docs/decisions                   7/30      src/selfhost/kel           0/30
#   examples/scripts                 0/30      examples/rtos/scripts      0/30
#   compiler/kel                     0/30      native_codegen             0/30
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
head_commit=$(git rev-parse HEAD 2>/dev/null || echo UNKNOWN)
echo "HEAD is $head_commit"

# The reach figures below diff COMMIT to COMMIT, so uncommitted work is invisible
# to them. Saying "still speaks to HEAD" while backend sources sit unstaged would
# be the reporter telling its own version of the lie it exists to prevent.
now_dirt=$(git status --porcelain -- "${REACH[@]}" 2>/dev/null | grep -cv 'GATE_RECORD.md') || true
if [ "${now_dirt:-0}" -gt 0 ]; then
    echo "⚠ ${now_dirt} backend file(s) are UNCOMMITTED right now. No record can speak"
    echo "  to them: the reach figures below compare commits and cannot see them."
fi

if [ ! -f "$record" ]; then
    echo "no gate record exists -- the backend's only instrument has left no trace"
    exit 0
fi

rows=$(grep -E '^\| (default features|narrow-float-32) \|' "$record") || true
if [ -z "$rows" ]; then
    echo "gate record present but carries no configuration rows"
    exit 0
fi

while IFS='|' read -r _ cfg commit tree verdict when _rest; do
    cfg=$(echo "$cfg" | xargs); commit=$(echo "$commit" | xargs)
    tree=$(echo "$tree" | xargs); verdict=$(echo "$verdict" | xargs)
    when=$(echo "$when" | xargs)
    echo ""
    echo "$cfg"
    echo "   verdict  $verdict on a $tree worktree, $when"
    echo "   commit   $commit"
    if ! git cat-file -e "${commit}^{commit}" 2>/dev/null; then
        echo "   ⚠ that commit is NOT in this repository -- the row cannot be checked"
        continue
    fi
    # Diffed over every READ path, not the package alone. Counting only the package
    # would have this reporter answer "still speaks to HEAD" while a test's input --
    # a corpus script, a handoff figure, a decision document -- had moved underneath
    # it. That is the reporter telling its own version of the lie it exists to stop.
    changed=$(git diff --name-only "${commit}..HEAD" -- "${REACH[@]}" 2>/dev/null \
              | grep -v "$record")
    n=$(printf '%s' "$changed" | grep -c . ) || true
    if [ "${n:-0}" -eq 0 ]; then
        echo "   reach    still speaks to HEAD: no backend source changed since"
    else
        echo "   reach    ⚠ ${n} backend source(s) changed since, so UNVERIFIED:"
        printf '%s\n' "$changed" | sed 's/^/              /'
    fi
    if [ "$tree" != "clean" ]; then
        echo "   ⚠ run on a modified worktree: this verdict is not reproducible from history"
    fi
done <<< "$rows"
