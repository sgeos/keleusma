#!/usr/bin/env bash
# Run a command and say whether the tree moved underneath it.
#
# WHY THIS EXISTS, AND WHY IT IS NOT A CONVENTION
#
# Four times across three sessions, a measurement on this project was taken
# while its own inputs were being edited, and each time the rule against it had
# already been written down:
#
#   * absorption 40 -- increment edits landed mid-run, and this suite contains
#     tests that READ SOURCE FROM DISK, so the attribution had to be argued.
#   * 2026-09-02 -- a workspace run had a documentation commit land at 18:11
#     while it was still going.
#   * 2026-09-06 -- documents were edited under a running backend suite; that
#     run was discarded rather than quoted.
#   * 2026-09-06 -- a commit landed during a push, so the push's own
#     local-vs-remote check printed a mismatch that was not one.
#
# A RESOLUTION TO BE CAREFUL HAS NOW FAILED FOUR TIMES. The tree's own lesson,
# from the twelve-tests-one-witness fault, is that such a repair has to be
# structural. So the verdict is attached to the OUTPUT, where a reader meets it,
# rather than living in a rule someone has to remember.
#
# WHAT IT COVERS, AND WHAT IT DOES NOT
#
# It hashes the TRACKED content of the worktree -- `git status --porcelain` plus
# `HEAD` -- before and after. That catches an edit, a commit, a merge, a stash,
# or a checkout landing mid-run.
#
# It does NOT catch: untracked files the run reads, environment changes, machine
# load, another process writing outside the worktree, or anything about whether
# the measurement was correct in the first place. **A CLEAN VERDICT HERE IS NOT
# A VALID MEASUREMENT.** It rules out one specific way of being wrong, which is
# the way this project keeps being wrong.
#
# IT DOES NOT BLOCK. A run that moves still reports its own exit status, and a
# legitimate edit to an unrelated file is not an error. The point is that the
# result cannot be quoted as frozen when it was not.
#
# Usage:  tools/frozen-run.sh <command> [args...]
set -uo pipefail

repo_root="$(git rev-parse --show-toplevel)"

stamp() {
    {
        git -C "$repo_root" rev-parse HEAD
        git -C "$repo_root" status --porcelain
    } | shasum -a 256 | cut -d' ' -f1
}

before="$(stamp)"
head_before="$(git -C "$repo_root" rev-parse --short HEAD)"
start="$(date +%s)"

# **STREAM AND CAPTURE, NEVER BUFFER.** The output goes to the terminal AS IT
# HAPPENS and to a file at the same time. Buffering it until exit is a recorded
# failure on this project: a 12h51m sweep was piped through `tail`, so progress
# was invisible and the continue-or-kill decision was blind for hours.
_fr_out="$(mktemp -t frozen-run)"
"$@" 2>&1 | tee "$_fr_out"
rc=${PIPESTATUS[0]}

end="$(date +%s)"
after="$(stamp)"
head_after="$(git -C "$repo_root" rev-parse --short HEAD)"

echo
echo "================ FROZEN-TREE VERDICT"
echo "  command   : $*"
echo "  exit      : $rc"
echo "  wall      : $((end - start))s"
echo "  HEAD      : $head_before -> $head_after"
# **WHAT THE RUN MEASURED, BESIDE WHETHER THE TREE MOVED.**
#
# A record saying "frozen" without saying what was found is half a record: it
# tells a later reader the tree was still and nothing about the result. Keeping
# the summary ADJACENT to the verdict means a truncated log fragment carries both
# or neither -- and a caller who pipes this through `tail` (twice in ten minutes,
# on the day this was written) still gets the number.
if [ -s "$_fr_out" ]; then
    echo "  ---- last lines of the wrapped command ----"
    tail -n 4 "$_fr_out" | sed 's/^/  | /'
fi
if [ "$before" = "$after" ]; then
    echo "  VERDICT   : FROZEN. Tracked content did not change during the run."
    echo "              This rules out ONE way of being wrong. It is not a claim"
    echo "              that the measurement is correct."
else
    echo "  VERDICT   : *** NOT FROZEN. THE TREE MOVED DURING THIS RUN. ***"
    echo "              Do not quote this result as a clean measurement. Re-run"
    echo "              it on a still tree, or report it with this caveat"
    echo "              attached -- which is what the record requires."
fi
echo "================"
rm -f "$_fr_out"
exit $rc
