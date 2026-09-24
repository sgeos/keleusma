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
HANDOFF="../docs/process/handoffs/v0.3.0.md"
head_commit=$(git rev-parse HEAD 2>/dev/null || echo UNKNOWN)
echo "HEAD is $head_commit"

# The reach figures below diff COMMIT to COMMIT, so uncommitted work is invisible
# to them. Saying "still speaks to HEAD" while backend sources sit unstaged would
# be the reporter telling its own version of the lie it exists to prevent.
now_dirt=$(git status --porcelain -- . "$HANDOFF" 2>/dev/null | grep -cv 'GATE_RECORD.md') || true
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
    # The handoff is diffed alongside the package because `handoff_figures.rs` READS
    # it: a figure edited there can turn this suite red without a single file under
    # `native_codegen/` changing. Counting only the package would have this reporter
    # answer "still speaks to HEAD" while a test's input had moved underneath it.
    changed=$(git diff --name-only "${commit}..HEAD" -- . "$HANDOFF" 2>/dev/null \
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
