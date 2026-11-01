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
#
# Build output on a scratch volume. When SCRATCH_BASE is set, `new` links the new
# tree's build output to $SCRATCH_BASE/<repository>/<leaf>/ through scripts/scratch.sh,
# vendored unchanged from sgeos/project-template v1.0.8, commit f397f03, so that it
# stays easy to compare. The directories are the `target` of every package in
# SCRATCH_PACKAGES whose Cargo.toml exists in the new tree. `new` refuses a leaf whose
# scratch folder already exists, or that another worktree already uses, because
# scratch.sh would link the new tree to that output. `rm` deletes the tree's scratch
# folder once the tree is gone, and `list` reports scratch folders that no tree uses
# and package targets of live trees that are not links, deleting none. Without SCRATCH_BASE nothing changes. The operator approved this on
# 2026-10-31, in docs/process/kaizen/2026-10-31-scratch-volume-proposal.md.
set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT="$(pwd)"
TRUNK="${KEL_TRUNK:-v0.2.3}"
TREES_DIR="${KEL_WORKTREES_DIR:-"$REPO_ROOT/../keleusma-worktrees"}"

# The packages whose `target` goes to scratch, relative to the tree. `.` is the root.
SCRATCH_PACKAGES=". compiler fuzz examples/rtos keleusma-lsp keleusma-wasm native_codegen"
# scratch.sh names the repository folder after the main checkout, found through the
# common git directory, so this script does the same.
REPO_NAME="$(basename "$(dirname "$(cd "$(git rev-parse --git-common-dir)" && pwd)")")"

die() { printf 'worktree: %s\n' "$1" >&2; exit 1; }

leaf() { printf '%s' "${1##*/}"; }

# The scratch folder of a tree, or nothing when SCRATCH_BASE is unset. The leaf is
# checked so that a deletion of this path cannot widen to the repository's folder.
scratch_home() {
  local name="$1"
  [ -n "${SCRATCH_BASE:-}" ] || return 0
  case "$name" in
    ''|.|..|*/*) die "not a tree name for the scratch volume: '$name'" ;;
  esac
  printf '%s/%s/%s' "$SCRATCH_BASE" "$REPO_NAME" "$name"
}

# The base name of every current worktree, one per line. The list is captured before
# grep reads it, because `grep -q` in a pipeline under pipefail can report a match as
# a failure when it closes the pipe early.
tree_names() {
  local paths
  paths="$(git worktree list --porcelain | sed -n 's/^worktree //p')"
  while IFS= read -r path; do
    [ -n "$path" ] && basename "$path"
  done <<< "$paths"
}

# The `target` directories of the packages that exist in a tree.
package_targets() {
  local tree="$1" pkg
  for pkg in $SCRATCH_PACKAGES; do
    if [ -f "$tree/$pkg/Cargo.toml" ]; then
      if [ "$pkg" = . ]; then printf 'target\n'; else printf '%s/target\n' "$pkg"; fi
    fi
  done
}

# Delete a removed tree's scratch folder, which holds build output only. A live tree
# with the same leaf, such as one under .worktrees/, still owns the folder, so it stays.
remove_scratch() {
  local home
  home="$(scratch_home "$1")"
  [ -n "$home" ] && [ -d "$home" ] || return 0
  if grep -qxF "$1" <<< "$(tree_names)"; then
    printf 'worktree: kept %s, which a live tree named %s still uses\n' "$home" "$1"
    return 0
  fi
  rm -rf -- "$home"
  printf 'worktree: removed build output in %s\n' "$home"
}

cmd_new() {
  local branch="${1:-}"
  [ -n "$branch" ] || die "usage: worktree.sh new <scope>/<short-description>"
  case "$branch" in
    feat/*|fix/*|docs/*|refactor/*|test/*|chore/*) ;;
    *) die "branch must start with a supported scope (feat/ fix/ docs/ refactor/ test/ chore/); got '$branch'" ;;
  esac
  git show-ref --verify --quiet "refs/heads/$branch" && die "branch '$branch' already exists"
  local dir name home
  name="$(leaf "$branch")"
  dir="$TREES_DIR/$name"
  [ -e "$dir" ] && die "path '$dir' already exists"
  home="$(scratch_home "$name")"
  if [ -n "$home" ]; then
    [ -e "$home" ] && die "scratch folder '$home' already exists; a new tree would share its build output"
    if grep -qxF "$name" <<< "$(tree_names)"; then
      die "a worktree named '$name' already uses that scratch folder name"
    fi
  fi

  # Base the new branch on the freshest trunk tip we can see.
  git fetch --quiet origin "$TRUNK" 2>/dev/null || true
  local base="$TRUNK"
  git show-ref --verify --quiet "refs/remotes/origin/$TRUNK" && base="origin/$TRUNK"

  mkdir -p "$TREES_DIR"
  git worktree add -b "$branch" "$dir" "$base"
  printf '\nworktree ready:\n  branch: %s (off %s)\n  path:   %s\n\nnext: cd "%s"  &&  scripts/fast-check.sh ...\n' \
    "$branch" "$base" "$dir" "$dir"
  if [ -n "$home" ]; then
    local targets=()
    while IFS= read -r t; do targets+=("$t"); done < <(package_targets "$dir")
    if [ "${#targets[@]}" -gt 0 ]; then
      (cd "$dir" && scripts/scratch.sh link "${targets[@]}") \
        || die "the tree exists, but its build output was not linked and stays in the tree"
    fi
  fi
}

# Report each package `target` of a live tree that is a directory rather than a link,
# which is build output on the tree's own volume. Trees made without this script, such
# as Claude Code's agent trees under .claude/worktrees/, are the expected case.
report_unlinked() {
  local paths tree target
  paths="$(git worktree list --porcelain | sed -n 's/^worktree //p')"
  while IFS= read -r tree; do
    if [ -z "$tree" ] || [ ! -d "$tree" ]; then continue; fi
    while IFS= read -r target; do
      if [ -n "$target" ] && [ -d "$tree/$target" ] && [ ! -L "$tree/$target" ]; then
        printf 'build output not on the scratch volume: %s (%s)\n' \
          "$tree/$target" "$(du -sh "$tree/$target" | cut -f1)"
      fi
    done <<< "$(package_targets "$tree")"
  done <<< "$paths"
}

cmd_list() {
  git worktree list
  [ -n "${SCRATCH_BASE:-}" ] || return 0
  # Report, never delete or move. The operator decides what happens to the output.
  report_unlinked
  [ -d "$SCRATCH_BASE/$REPO_NAME" ] || return 0
  local used folder
  used="$(tree_names)"
  for folder in "$SCRATCH_BASE/$REPO_NAME"/*/; do
    [ -d "$folder" ] || continue
    folder="${folder%/}"
    if ! grep -qxF "$(basename "$folder")" <<< "$used"; then
      printf 'scratch folder used by no tree: %s (%s)\n' "$folder" "$(du -sh "$folder" | cut -f1)"
    fi
  done
}

cmd_rm() {
  local branch="${1:-}"
  [ -n "$branch" ] || die "usage: worktree.sh rm <scope>/<short-description>"
  local dir
  dir="$TREES_DIR/$(leaf "$branch")"
  if [ -d "$dir" ]; then
    git worktree remove "$dir" || die "tree '$dir' is dirty; commit/stash or use: git worktree remove --force '$dir'"
  else
    printf 'worktree: no tree at %s (already gone); pruning refs\n' "$dir"
    git worktree prune
  fi
  # The tree is gone in both branches, so its build output is no longer used.
  remove_scratch "$(leaf "$branch")"
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
