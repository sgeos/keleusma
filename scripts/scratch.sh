#!/usr/bin/env bash
# Keep build output on a machine-local scratch volume, linked into the tree.
#
# Build output, such as Cargo's target/ or a node_modules/ directory, can grow by
# tens of gigabytes a day across many worktrees. Placing it on a separate volume with a
# quota, such as an APFS volume or a ZFS dataset, lets it fill that volume instead of
# the system disk. This script moves each listed directory to
# $SCRATCH_BASE/<repository>/<tree>/<directory> and leaves a symbolic link in its place.
# Nothing here is committed: the link is ignored, and a machine without SCRATCH_BASE
# keeps its build output in the tree as usual.
#
# SCRATCH_BASE names the volume, such as /Volumes/Scratch, and is required. SCRATCH_DIRS
# lists the directories to manage, relative to the tree's top level, and defaults to
# target. Set both in the shell profile of a machine that uses a scratch volume.
#
# Usage:
#   scripts/scratch.sh status          # each managed directory: link, real or missing
#   scripts/scratch.sh link [DIR...]   # move DIR to scratch and link it, or relink it
#   scripts/scratch.sh unlink [DIR...] # move DIR back into the tree
#
# Failure modes. The agent sandbox resolves the link to its real path, so it must allow
# writing under SCRATCH_BASE, or every build there is refused. `cargo clean`, and any
# tool that removes the directory, deletes the link rather than the files it points to,
# and the next build recreates a real directory in the tree. This was observed with
# Cargo 1.99.1. Run `link` after a clean and before building, which relinks to the
# copy still on scratch. If a build has already recreated the directory, `link` refuses,
# because both a real directory and a scratch copy exist and merging two build outputs
# is not safe; delete the stale scratch copy, then run `link` to move the new one.
# .gitignore entries must not end in a slash, as `/target` rather than `/target/`,
# because a pattern with a trailing slash matches only a directory, not a link.

set -euo pipefail

usage() {
    sed -n '/^# Usage:/,/^#$/p' "$0" | sed 's/^# \{0,1\}//' >&2
    exit 2
}

[[ $# -ge 1 ]] || usage
command="$1"
shift

base="${SCRATCH_BASE:-}"
if [[ -z "$base" ]]; then
    printf 'scratch.sh: SCRATCH_BASE is not set, so build output stays in the tree\n' >&2
    exit 1
fi
[[ -d "$base" && -w "$base" ]] ||
    { printf 'scratch.sh: %s is not a writable directory\n' "$base" >&2; exit 1; }

root="$(git rev-parse --show-toplevel)"
# Every tree of one repository shares a parent folder on scratch, named after the main
# checkout, with one folder per tree beneath it.
common="$(cd "$root" && cd "$(git rev-parse --git-common-dir)" && pwd)"
repository="$(basename "$(dirname "$common")")"
tree="$(basename "$root")"
home="$base/$repository/$tree"

read -r -a dirs <<< "${SCRATCH_DIRS:-target}"
if [[ $# -gt 0 ]]; then
    dirs=("$@")
fi
# An empty array cannot be expanded under set -u in bash 3.2, which macOS ships.
[[ ${#dirs[@]} -gt 0 ]] ||
    { printf 'scratch.sh: no directories named and SCRATCH_DIRS is empty\n' >&2; exit 2; }

check_dir() {
    case "$1" in
        ''|/*|*..*) printf 'scratch.sh: not a relative directory inside the tree: %s\n' "$1" >&2
                    exit 2 ;;
    esac
}

case "$command" in
    status)
        for dir in "${dirs[@]}"; do
            check_dir "$dir"
            local_path="$root/$dir"
            if [[ -L "$local_path" ]]; then
                state="link to $(readlink "$local_path")"
                [[ -d "$local_path" ]] || state="$state, which is missing"
            elif [[ -d "$local_path" ]]; then
                state="real directory in the tree"
            else
                state="missing"
            fi
            printf '%s: %s\n' "$dir" "$state"
        done
        ;;
    link)
        for dir in "${dirs[@]}"; do
            check_dir "$dir"
            local_path="$root/$dir"
            scratch="$home/$dir"
            if [[ -L "$local_path" ]]; then
                printf '%s: already a link\n' "$dir"
                continue
            fi
            if [[ -d "$local_path" && -e "$scratch" ]]; then
                printf 'scratch.sh: both %s and %s exist; delete one, then rerun\n' \
                    "$local_path" "$scratch" >&2
                exit 1
            fi
            mkdir -p "$(dirname "$scratch")"
            if [[ -d "$local_path" ]]; then
                mv "$local_path" "$scratch"
                printf '%s: moved to %s\n' "$dir" "$scratch"
            else
                mkdir -p "$scratch"
            fi
            mkdir -p "$(dirname "$local_path")"
            ln -s "$scratch" "$local_path"
            printf '%s: linked to %s\n' "$dir" "$scratch"
        done
        ;;
    unlink)
        for dir in "${dirs[@]}"; do
            check_dir "$dir"
            local_path="$root/$dir"
            scratch="$home/$dir"
            if [[ ! -L "$local_path" ]]; then
                printf '%s: not a link, left as it is\n' "$dir"
                continue
            fi
            rm "$local_path"
            if [[ -d "$scratch" ]]; then
                mv "$scratch" "$local_path"
                printf '%s: moved back into the tree\n' "$dir"
            else
                printf '%s: link removed; %s did not exist\n' "$dir" "$scratch"
            fi
        done
        ;;
    *)
        usage
        ;;
esac
