#!/usr/bin/env bash
# workspace-coverage.sh — is the recorded workspace-suite result still about THIS tree?
#
# WHY THIS EXISTS
#   Every absorption prediction on the V0.3.X line names only `native_codegen`
#   figures. A figure that ranges over one package cannot express staleness in a
#   population it never covered, so the workspace check is missed BY CONSTRUCTION
#   rather than by oversight. Prose recording that fact has already failed once:
#   the same defect appeared as the absorption-46 build clause and recurred here.
#   A prose note is the same species of artifact that drifted before, so this one
#   is executable.
#
# WHY THIS IS A SCRIPT AND NOT A TEST
#   A test asserting the stamp is current would be RED from the moment of every
#   absorption until someone spends ninety minutes on a workspace run. A guard
#   that is red by default gets suppressed, not obeyed. This answers on demand.
#
# DEFAULT-DENY
#   A path this script does not recognise is treated as COMPILED, the strongest
#   class. The guard can therefore OVER-report staleness but never under-report
#   it. Adding a path to an inert list is a claim requiring evidence.
#
# VERDICTS
#   CURRENT           nothing the workspace suite compiles or reads has moved
#   STALE-READ-ONLY   only documentation moved; the docs-reading tests are at risk
#   STALE-COMPILED    source the workspace suite builds has moved; re-run required

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
stamp="$repo/native_codegen/WORKSPACE_COVERAGE.stamp"

# Paths the workspace suite provably cannot see.
# `native_codegen/` is a detached package: not a workspace member, not built by
# `cargo test --workspace`, and no workspace test reads it from disk. Verified by
# grepping tests/ for disk reads; the single textual hit is a doc comment.
is_inert() { [[ "$1" == native_codegen/* ]]; }

# Paths read from disk by workspace tests but not compiled by them.
is_read_only() { [[ "$1" == docs/* ]]; }

usage() {
  cat >&2 <<'USAGE'
usage:
  workspace-coverage.sh check [--since <commit>] [--at <commit>]
  workspace-coverage.sh stamp <commit> <figures...>
USAGE
  exit 64
}

cmd="${1:-}"; shift || usage

case "$cmd" in
  stamp)
    [[ $# -ge 1 ]] || usage
    commit="$1"; shift
    resolved="$(git -C "$repo" rev-parse "$commit")"
    { echo "commit: $resolved"
      echo "stamped: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
      echo "figures: $*"
    } > "$stamp"
    echo "stamped workspace coverage at $resolved"
    ;;

  check)
    since=""; at="HEAD"
    while [[ $# -gt 0 ]]; do
      case "$1" in
        --since) since="${2:-}"; shift 2 ;;
        # --at exists so the guard's own reach can be demonstrated over a
        # historical range WITHOUT a checkout. Several tests in this repository
        # read source text from disk, so mutating the working tree to test a
        # guard would corrupt any suite running beside it.
        --at)    at="${2:-}";    shift 2 ;;
        *) usage ;;
      esac
    done
    if [[ -z "$since" ]]; then
      [[ -f "$stamp" ]] || { echo "no stamp at $stamp; run 'stamp' after a workspace run" >&2; exit 3; }
      since="$(awk '/^commit:/{print $2}' "$stamp")"
    fi
    git -C "$repo" cat-file -e "${since}^{commit}" 2>/dev/null || {
      echo "stamped commit $since is not in this repository" >&2; exit 3; }

    head="$(git -C "$repo" rev-parse "$at")"
    compiled=(); readonly_=()
    while IFS= read -r p; do
      [[ -n "$p" ]] || continue
      if   is_inert     "$p"; then continue
      elif is_read_only "$p"; then readonly_+=("$p")
      else                         compiled+=("$p")
      fi
    done < <(git -C "$repo" diff --name-only "$since" "$head")

    echo "workspace coverage stamped at: $since"
    echo "tree is at:                    $head"
    echo "changed since, compiled by the workspace suite: ${#compiled[@]}"
    echo "changed since, read by workspace tests only:    ${#readonly_[@]}"

    if (( ${#compiled[@]} > 0 )); then
      printf '  %s\n' "${compiled[@]}" | head -20
      if (( ${#compiled[@]} > 20 )); then echo "  ... $(( ${#compiled[@]} - 20 )) more"; fi
      echo "VERDICT: STALE-COMPILED"
      exit 1
    elif (( ${#readonly_[@]} > 0 )); then
      echo "VERDICT: STALE-READ-ONLY"
      exit 2
    fi
    echo "VERDICT: CURRENT"
    ;;

  *) usage ;;
esac
