#!/bin/sh
# loc.sh — count the project's lines with cloc, one block per category.
#
# Usage:
#   scripts/loc.sh                 # every category, then the total
#   scripts/loc.sh runtime docs    # only the named categories
#   scripts/loc.sh -l              # list the categories
#
# WHAT IS COUNTED. Only files git tracks, read with `git ls-files`, so build output,
# node_modules, tmp/, secret/ and nested worktrees never enter a count, whatever a
# machine holds. Within the tracked files, data that is not code or prose is left out
# of every category: lockfiles, the fuzz seed corpus, the kaizen evidence files, the
# interline and Jev checkpoints, and the gate summary fixtures. A file belongs to one
# category, and the total counts the union.
#
# KELEUSMA SOURCE. tools/cloc.lang teaches cloc the `.kel` extension. A shell alias that
# adds --read-lang-def does not reach a script, so the definition is passed here.
#
# cloc counts a file only once when several are identical, its default, so a vendored copy
# or a duplicated fixture counts once.
#
# The script is POSIX sh, so that it runs under dash as well as bash. It assumes no
# tracked path contains a newline, since cloc reads its file list one path per line.
#
# Lists of names below are split on spaces on purpose, which POSIX sh offers in place of
# arrays. `set -f` turns off filename expansion, so a pathspec such as
# `:(glob)scripts/**/*.md` reaches git unchanged and is never expanded by the shell.
# shellcheck disable=SC2086,SC2046
set -euf

cd "$(dirname "$0")/.."
LANG_DEF=tools/cloc.lang
# Directory and file names with no spaces, split on purpose below.
CRATES="keleusma-macros keleusma-arena keleusma-wire keleusma-wire-derive keleusma-cli keleusma-bench"
DETACHED="compiler native_codegen keleusma-lsp keleusma-wasm fuzz footprint"
TOOLING="scripts .github editors tools .cargo .cargo-husky .config .devcontainer flake.nix Cargo.toml book/book.toml book/ci"
CATEGORIES="runtime crates tests detached tooling examples example-docs docs package-docs process translations"

die() { printf 'loc: %s\n' "$1" >&2; exit "${2:-1}"; }

title() {
  case $1 in
    runtime) echo 'Runtime library, src/' ;;
    crates) echo 'Workspace crates' ;;
    tests) echo 'Integration tests, tests/' ;;
    detached) echo 'Detached packages: compiler, native_codegen, LSP, WASM, fuzz, footprint' ;;
    examples) echo 'Examples, examples/' ;;
    example-docs) echo 'Example documentation' ;;
    docs) echo 'Documentation: root, docs/ without process, book source, changelog' ;;
    package-docs) echo 'Package and tooling documentation: crates, detached packages, tools' ;;
    process) echo 'Process records, docs/process/' ;;
    translations) echo 'Book translations, book/po/' ;;
    tooling) echo 'Tooling and CI: scripts, workflows, editors, configuration' ;;
    total) echo 'Total of every category' ;;
    *) return 1 ;;
  esac
}

# Tracked files under the given pathspecs, without the data excluded everywhere.
tracked() {
  git -c core.quotePath=false ls-files -- "$@" \
    ':(exclude,glob)**/Cargo.lock' \
    ':(exclude,glob)**/package-lock.json' \
    ':(exclude)fuzz/corpus' \
    ':(exclude)docs/process/kaizen/evidence' \
    ':(exclude)docs/process/interline' \
    ':(exclude)docs/process/jev-shadow' \
    ':(exclude)docs/process/verification' \
    ':(exclude)scripts/gate_summary_fixtures'
}

# The same, without Markdown, for the categories that count code. Markdown in a
# package or tooling directory is counted under package-docs instead.
code() {
  tracked "$@" ':(exclude,glob)**/*.md'
}

files() {
  case $1 in
    runtime) code src ;;
    crates) code $CRATES ;;
    tests) code tests ;;
    detached) code $DETACHED ;;
    tooling) code $TOOLING ;;
    examples) code examples ;;
    example-docs) tracked ':(glob)examples/**/*.md' ;;
    docs) tracked ':(glob)*.md' llms.txt CITATION.cff docs book/src changelog.d \
      ':(exclude)docs/process' ;;
    process) tracked docs/process ;;
    translations) tracked book/po ;;
    package-docs) tracked ':(glob)src/**/*.md' ':(glob)tests/**/*.md' \
      $(for dir in $CRATES $DETACHED $TOOLING; do printf ':(glob)%s/**/*.md ' "$dir"; done) ;;
    total)
      for category in $CATEGORIES; do files "$category"; done | sort -u ;;
    *) return 1 ;;
  esac
}

count() {
  printf '\n== %s ==\n' "$(title "$1")"
  files "$1" > "$LIST"
  if [ ! -s "$LIST" ]; then
    echo 'No tracked files.'
    return 0
  fi
  # cloc's version banner is dropped. Its output goes through a file rather than a
  # pipe so that a cloc failure still fails the script, which POSIX sh cannot see
  # through a pipeline.
  cloc --quiet --hide-rate --read-lang-def="$LANG_DEF" --force-lang='Linker Script,x' \
    --list-file="$LIST" > "$OUT"
  sed '/^github\.com\/AlDanial\/cloc/d' "$OUT"
}

if [ "${1:-}" = -h ] || [ "${1:-}" = --help ]; then
  sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'
  exit 0
fi
if [ "${1:-}" = -l ]; then
  for category in $CATEGORIES total; do
    printf '%-14s %s\n' "$category" "$(title "$category")"
  done
  exit 0
fi

command -v cloc >/dev/null 2>&1 || die 'cloc is not installed. See https://github.com/AlDanial/cloc' 127
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || die 'not inside a git work tree'
[ -f "$LANG_DEF" ] || die "missing $LANG_DEF"

if [ "$#" -eq 0 ]; then
  set -- $CATEGORIES total
fi
for category in "$@"; do
  title "$category" >/dev/null || die "unknown category '$category'. Run scripts/loc.sh -l"
done

LIST=$(mktemp "${TMPDIR:-/tmp}/loc.XXXXXX")
OUT=$(mktemp "${TMPDIR:-/tmp}/loc.XXXXXX")
trap 'rm -f "$LIST" "$OUT"' EXIT
trap 'exit 130' INT TERM

for category in "$@"; do
  count "$category"
done
