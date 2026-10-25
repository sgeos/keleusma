#!/usr/bin/env bash
# Print `mode=certified` and `certified_by=<run>` when a full, green pull-request run of
# this repository's CI tested this exact tree with this stable toolchain within the last
# day. Print nothing otherwise, which leaves the branch run full.
#
# Read by the `changes` job of .github/workflows/ci.yml on a push to a version branch.
# A pull-request run tests GitHub's trial merge. When the base has not moved, the merge
# commit has the same tree, and no test reads git history, so a full branch run would
# repeat the pull-request run on identical input. The `certify` job of a full, green
# pull-request run uploads `ci-tree-<tree>` holding the tree and `rustc +stable -V`.
#
# Every failure is silent and leaves the run full. This script can only skip work it
# can show was done. It accepts a certificate only from a pull-request run of ci.yml in
# this repository, never a fork, that concluded success, began within the last day, and
# recorded the same tree and the same stable toolchain. What it cannot see is drift that
# leaves the toolchain unchanged, such as a newly published advisory, which the next
# full run reports.
set -u

tree=$(git rev-parse 'HEAD^{tree}') || exit 0
toolchain=$(rustc +stable -V 2>/dev/null) || exit 0
repo=${GITHUB_REPOSITORY:-}
[ -n "$repo" ] || exit 0
cutoff=$(python3 -c 'import datetime as d; print((d.datetime.now(d.timezone.utc) - d.timedelta(hours=24)).strftime("%Y-%m-%dT%H:%M:%SZ"))') || exit 0

runs=$(gh api "repos/$repo/actions/artifacts?name=ci-tree-$tree&per_page=30" \
  --jq '.artifacts[] | select(.expired == false) | .workflow_run.id') || exit 0

for run in $runs; do
  accepted=$(gh api "repos/$repo/actions/runs/$run" --jq "select(.event == \"pull_request\"
    and .conclusion == \"success\" and .path == \".github/workflows/ci.yml\"
    and .head_repository.full_name == \"$repo\" and .created_at >= \"$cutoff\") | .id") || continue
  [ "$accepted" = "$run" ] || continue
  dir=$(mktemp -d "${RUNNER_TEMP:-${TMPDIR:-/tmp}}/ci-certified.XXXXXX") || continue
  gh run download "$run" --repo "$repo" --name "ci-tree-$tree" --dir "$dir" >/dev/null 2>&1 || continue
  if [ "$(sed -n 's/^tree=//p' "$dir/certificate.txt")" = "$tree" ] &&
     [ "$(sed -n 's/^toolchain=//p' "$dir/certificate.txt")" = "$toolchain" ]; then
    echo "mode=certified"
    echo "certified_by=$run"
    exit 0
  fi
done
exit 0
