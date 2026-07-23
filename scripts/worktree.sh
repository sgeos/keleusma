#!/usr/bin/env bash
# worktree.sh — manage isolated git worktrees for parallel-agent development.
#
# Each concurrent agent (or human) gets its OWN working directory on its OWN
# feature branch, so parallel work never shares one dirty tree. Worktrees are
# created as siblings of the repo under ../keleusma-worktrees/<branch-leaf>,
# each branched off the active trunk (default: v0.2.3; override with KEL_TRUNK).
#
# This is the mechanical enabler for the protocol in
# docs/process/PARALLEL_DEVELOPMENT.md — read that for the merge-serialization
# and gate-discipline rules. This script only creates/lists/removes trees.
#
# Usage:
#   scripts/worktree.sh new  feat/some-thing   # branch off the trunk, new tree
#   scripts/worktree.sh list                   # show all worktrees
#   scripts/worktree.sh rm   feat/some-thing   # remove tree + delete its branch
#   scripts/worktree.sh path feat/some-thing   # print the tree path (for cd)
#
# Note: a scope/desc branch name like feat/some-thing maps to a directory
# named "some-thing" (the leaf after the last slash) to keep paths flat.
set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT="$(pwd)"
TRUNK="${KEL_TRUNK:-v0.2.3}"
TREES_DIR="${KEL_WORKTREES_DIR:-"$REPO_ROOT/../keleusma-worktrees"}"

die() { printf 'worktree: %s\n' "$1" >&2; exit 1; }

leaf() { printf '%s' "${1##*/}"; }

cmd_new() {
  local branch="${1:-}"
  [ -n "$branch" ] || die "usage: worktree.sh new <scope>/<short-description>"
  case "$branch" in
    feat/*|fix/*|docs/*|refactor/*|test/*|chore/*) ;;
    *) die "branch must start with a supported scope (feat/ fix/ docs/ refactor/ test/ chore/); got '$branch'" ;;
  esac
  git show-ref --verify --quiet "refs/heads/$branch" && die "branch '$branch' already exists"
  local dir="$TREES_DIR/$(leaf "$branch")"
  [ -e "$dir" ] && die "path '$dir' already exists"

  # Base the new branch on the freshest trunk tip we can see.
  git fetch --quiet origin "$TRUNK" 2>/dev/null || true
  local base="$TRUNK"
  git show-ref --verify --quiet "refs/remotes/origin/$TRUNK" && base="origin/$TRUNK"

  mkdir -p "$TREES_DIR"
  git worktree add -b "$branch" "$dir" "$base"
  printf '\nworktree ready:\n  branch: %s (off %s)\n  path:   %s\n\nnext: cd "%s"  &&  scripts/fast-check.sh ...\n' \
    "$branch" "$base" "$dir" "$dir"
}

cmd_list() {
  git worktree list
}

cmd_rm() {
  local branch="${1:-}"
  [ -n "$branch" ] || die "usage: worktree.sh rm <scope>/<short-description>"
  local dir="$TREES_DIR/$(leaf "$branch")"
  if [ -d "$dir" ]; then
    git worktree remove "$dir" || die "tree '$dir' is dirty; commit/stash or use: git worktree remove --force '$dir'"
  else
    printf 'worktree: no tree at %s (already gone); pruning refs\n' "$dir"
    git worktree prune
  fi
  if git show-ref --verify --quiet "refs/heads/$branch"; then
    git branch -d "$branch" 2>/dev/null \
      || die "branch '$branch' is not fully merged; delete with: git branch -D '$branch'"
  fi
  printf 'worktree: removed %s and branch %s\n' "$dir" "$branch"
}

cmd_path() {
  local branch="${1:-}"
  [ -n "$branch" ] || die "usage: worktree.sh path <scope>/<short-description>"
  printf '%s\n' "$TREES_DIR/$(leaf "$branch")"
}

sub="${1:-}"; shift || true
case "$sub" in
  new)  cmd_new  "$@" ;;
  list) cmd_list "$@" ;;
  rm)   cmd_rm   "$@" ;;
  path) cmd_path "$@" ;;
  *) die "usage: worktree.sh {new|list|rm|path} [branch]  (trunk=$TRUNK)" ;;
esac
