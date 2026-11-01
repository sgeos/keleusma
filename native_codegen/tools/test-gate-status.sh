#!/usr/bin/env bash
# Exercise the actual reporter in an isolated repository. No LLVM build required.
set -euo pipefail
reporter="$(cd "$(dirname "$0")" && pwd)/gate-status.sh"
fixture=$(mktemp -d "${TMPDIR:-/tmp}/keleusma-gate-status.XXXXXX")
trap 'rm -rf "$fixture"' EXIT
mkdir -p "$fixture/native_codegen/tools" "$fixture/src"
cp "$reporter" "$fixture/native_codegen/tools/"
cd "$fixture"
git init -q
git config user.name 'Gate status fixture'
git config user.email 'fixture@example.invalid'
git config commit.gpgsign false
git config core.hooksPath /dev/null
printf 'original\n' > src/input
printf 'original\n' > native_codegen/GATE_RECORD.md.source
git add .
git commit -qm 'fixture inputs'
source_commit=$(git rev-parse HEAD)
record=native_codegen/GATE_RECORD.md
write_record() {
    printf '| default features | %s | clean | frozen | PASS | 2026-09-28T00:00:00Z |\n' "$source_commit" > "$record"
    printf '| narrow-float-32 | %s | clean | frozen | PASS | 2026-09-28T00:00:00Z |\n' "$source_commit" >> "$record"
}
cases=0
check() {
    local label=$1 expected=$2 needle=$3 actual=0
    shift 3
    bash native_codegen/tools/gate-status.sh "$@" > "$fixture/output" 2>&1 || actual=$?
    if [ "$actual" -ne "$expected" ] || ! grep -Fq "$needle" "$fixture/output"; then
        printf 'FAIL %s, expected exit %s, got %s\n' "$label" "$expected" "$actual" >&2
        cat "$fixture/output" >&2
        exit 1
    fi
    cases=$((cases + 1))
}
write_record
check fresh 0 'PASS on a clean worktree, frozen phases'
check display_fresh 0 'still speaks to HEAD' --exit-zero
check unknown_option 2 Usage --unknown
check excess_arguments 2 Usage --exit-zero extra
# A record-only commit and a dirty record do not alter measured inputs.
git add "$record"
git commit -qm 'fixture record'
check record_only_commit 0 'still speaks to HEAD'
printf '\nrecord annotation\n' >> "$record"
check dirty_record_only 0 'still speaks to HEAD'
rm "$record"
check absent 1 UNVERIFIED
check display_absent 0 UNVERIFIED --exit-zero
printf 'no rows\n' > "$record"
check empty 1 UNVERIFIED
write_record
sed -n '1p' "$record" > "$fixture/one-row"
cp "$fixture/one-row" "$record"
check missing_configuration 1 UNVERIFIED
write_record
cat "$fixture/one-row" >> "$record"
check duplicate_configuration 1 UNVERIFIED
for substitution in 's/clean/dirty(1)/' 's/frozen/MOVED(1)/' 's/ frozen |//' 's/2026-09-28T00:00:00Z/yesterday/' 's/ |$/ | extra |/'; do
    write_record
    sed "$substitution" "$record" > "$fixture/bad-row"
    cp "$fixture/bad-row" "$record"
    check "$substitution" 1 UNVERIFIED
done
# A recorded failure is valid evidence of a failed run, not malformed input.
write_record
sed '1s/PASS/FAIL/' "$record" > "$fixture/failed-row"
cp "$fixture/failed-row" "$record"
check recorded_failure 1 'FAILED: default features recorded FAIL'
check display_recorded_failure 0 'FAILED: default features recorded FAIL' --exit-zero
if grep -Fq 'invalid gate record schema' "$fixture/output"; then
    echo 'FAIL: a recorded gate failure was misreported as invalid schema' >&2
    exit 1
fi
write_record
sed '1s/PASS/UNKNOWN/' "$record" > "$fixture/invalid-row"
cp "$fixture/invalid-row" "$record"
check invalid_verdict 1 'UNVERIFIED: invalid gate record schema'
if grep -Fq 'FAILED:' "$fixture/output"; then
    echo 'FAIL: malformed evidence was misreported as a recorded failure' >&2
    exit 1
fi
write_record
sed "s/$source_commit/0000000000000000000000000000000000000000/" "$record" > "$fixture/bad-row"
cp "$fixture/bad-row" "$record"
check unknown_commit 1 UNVERIFIED
write_record
printf 'modified\n' > src/input
check unstaged 1 UNCOMMITTED
check display_unstaged 0 UNVERIFIED --exit-zero
git add src/input
check staged 1 UNCOMMITTED
git restore --source=HEAD --staged --worktree src/input
printf 'untracked\n' > native_codegen/new-input
check untracked 1 UNCOMMITTED
rm native_codegen/new-input
printf 'modified\n' > native_codegen/GATE_RECORD.md.source
check record_name_substring 1 UNCOMMITTED
git restore native_codegen/GATE_RECORD.md.source
printf 'changed\n' > src/input
git add src/input
git commit -qm 'fixture changed input'
check committed_change 1 'inputs changed'
check display_committed_change 0 UNVERIFIED --exit-zero
# Git errors must not turn into an empty diff and hence a false success.
mkdir "$fixture/fake-bin"
git_binary=$(command -v git)
# Literal shell variables belong to the generated Git wrapper.
# shellcheck disable=SC2016
printf '#!/usr/bin/env bash\nif [ "$1" = "%s" ]; then exit 128; fi\nexec "%s" "$@"\n' diff "$git_binary" > "$fixture/fake-bin/git"
chmod +x "$fixture/fake-bin/git"
PATH="$fixture/fake-bin:$PATH" check diff_error 1 'cannot compare'
# Literal shell variables belong to the generated Git wrapper.
# shellcheck disable=SC2016
printf '#!/usr/bin/env bash\nif [ "$1" = "%s" ]; then exit 128; fi\nexec "%s" "$@"\n' status "$git_binary" > "$fixture/fake-bin/git"
PATH="$fixture/fake-bin:$PATH" check status_error 1 'cannot inspect'
printf '#!/usr/bin/env bash\nexit 128\n' > "$fixture/fake-bin/git"
PATH="$fixture/fake-bin:$PATH" check head_error 1 'cannot resolve'
printf 'gate-status fixtures passed %s cases\n' "$cases"
