#!/usr/bin/env bash
# merge-to-trunk.sh — serialize a feature-branch merge into the active trunk.
#
# Implements the merge protocol in docs/process/PARALLEL_DEVELOPMENT.md section 4
# so parallel agents cannot race the release line: rebase onto the freshest trunk,
# run the mandatory full gate, RE-CHECK that the trunk tip did not move while the
# gate ran, then fast-forward merge and push. The re-check is the guard against a
# gate result that silently raced another agent's merge — a gate is only valid for
# the exact trunk tip it ran against.
#
# Run it FROM the feature branch (or its worktree). Dry-run by default; it will
# rebase and gate but stop before the merge/push and print the commands. Pass
# --execute to actually merge and push.
#
#   scripts/merge-to-trunk.sh              # rebase + gate + dry-run (no push)
#   scripts/merge-to-trunk.sh --execute    # rebase + gate + merge + push
#   scripts/merge-to-trunk.sh --execute --skip-gate   # docs/code-identical ONLY
#
# --skip-gate is an escape hatch for a merge provably code-identical to the last
# green trunk tip (docs/scripts only); it prints a loud warning and you own the
# claim. Never use it for a change that touches src/, tests/, or compiler/.
set -euo pipefail

cd "$(dirname "$0")/.."
TRUNK="${KEL_TRUNK:-v0.2.3}"
EXECUTE=0
SKIP_GATE=0
for a in "$@"; do
  case "$a" in
    --execute) EXECUTE=1 ;;
    --skip-gate) SKIP_GATE=1 ;;
    *) echo "merge-to-trunk: unknown arg '$a'" >&2; exit 2 ;;
  esac
done

die() { printf '\033[1;31mmerge-to-trunk: %s\033[0m\n' "$1" >&2; exit 1; }
say() { printf '\033[1mmerge-to-trunk: %s\033[0m\n' "$1"; }

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
[ "$BRANCH" = "$TRUNK" ] && die "you are on the trunk ($TRUNK); run this from the feature branch"
[ "$BRANCH" = "main" ] && die "refusing to operate from main"
[ -z "$(git status --porcelain)" ] || die "working tree is dirty; commit or stash first"

say "fetching origin/$TRUNK"
git fetch --quiet origin "$TRUNK"
PRE="$(git rev-parse "origin/$TRUNK")"
say "trunk tip before gate: ${PRE:0:12}"

say "rebasing $BRANCH onto origin/$TRUNK"
git rebase "origin/$TRUNK"

if [ "$SKIP_GATE" -eq 1 ]; then
  printf '\033[1;33mmerge-to-trunk: WARNING — skipping the mandatory gate. Only valid if this\n'
  printf 'branch is provably code-identical to the green trunk (docs/scripts only).\033[0m\n'
else
  say "running the mandatory pre-merge gate (scripts/release-gate.sh)"
  scripts/release-gate.sh
fi

# The race guard: re-fetch and confirm the trunk did not advance under us.
say "re-checking trunk tip (race guard)"
git fetch --quiet origin "$TRUNK"
POST="$(git rev-parse "origin/$TRUNK")"
[ "$PRE" = "$POST" ] || die "origin/$TRUNK advanced ${PRE:0:12} -> ${POST:0:12} during the gate; another merge won the race. Re-run this script (it will rebase onto the new tip and re-gate)."
say "trunk tip unchanged (${POST:0:12}); safe to merge"

if [ "$EXECUTE" -eq 0 ]; then
  cat <<EOF

--- DRY RUN complete (rebased + gated, trunk unmoved). To finish, run:
    scripts/merge-to-trunk.sh --execute
  or do it by hand:
    git checkout $TRUNK && git merge --ff-only $BRANCH && \\
      GIT_SSH_COMMAND="ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=8 -o TCPKeepAlive=yes" \\
      git push origin $TRUNK
EOF
  exit 0
fi

say "merging $BRANCH into $TRUNK (fast-forward) and pushing"
git checkout "$TRUNK"
git merge --ff-only "$BRANCH"
GIT_SSH_COMMAND="ssh -o ServerAliveInterval=15 -o ServerAliveCountMax=8 -o TCPKeepAlive=yes" \
  git push origin "$TRUNK"
say "merged $BRANCH -> $TRUNK and pushed. Verify: git ls-remote origin $TRUNK"
say "clean up the branch/worktree with: scripts/worktree.sh rm $BRANCH"
