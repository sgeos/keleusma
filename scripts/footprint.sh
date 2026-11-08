#!/bin/sh
# footprint.sh — measure what embedding the Keleusma runtime costs in a bare-metal image.
#
# Usage:
#   scripts/footprint.sh                  # measure every configuration
#   scripts/footprint.sh --check          # also fail when a configuration exceeds its ceiling
#   scripts/footprint.sh --features LIST  # measure one custom configuration as well
#
# It builds the probe in footprint/ for the Cortex-M55 target once as a baseline with no
# Keleusma code and once per configuration, and reports each image's sections from
# llvm-size. The runtime's footprint is the difference from the baseline. FLASH is text,
# which for this target includes read-only data, plus data. Static RAM is data plus bss.
# The runtime's dynamic RAM, its arena and module, comes from the heap and is bounded per
# program by the worst-case memory analysis, so it is not a property of the image.
#
# Every run overwrites footprint/target/footprint.json with the figures, the commit, the
# toolchain and whether the tree was clean, so the file always describes the build that
# produced it. footprint/budget.tsv holds the ceilings that --check enforces. Raising a
# ceiling is a deliberate edit, reviewed in the pull request that needs it.
#
# The script is POSIX sh. It needs cargo and the toolchain pinned by
# footprint/rust-toolchain.toml, with its target and llvm-tools component.
set -eu

cd "$(dirname "$0")/.."
PROBE=footprint
BUDGET=$PROBE/budget.tsv
OUT=$PROBE/target/footprint.json
TARGET=thumbv8m.main-none-eabihf
IMAGE=$PROBE/target/$TARGET/release/keleusma-footprint

# Name and probe features of each configuration. The baseline comes first.
CONFIGS="baseline:
vm-trusted:probe
vm-verify:verify
vm-verify-floats:verify,floats
vm-verify-floats-text:verify,floats,text"

die() { printf 'footprint: %s\n' "$1" >&2; exit 1; }

check=0
custom=''
while [ "$#" -gt 0 ]; do
  case $1 in
    --check) check=1 ;;
    --features)
      [ "$#" -ge 2 ] || die '--features needs a comma-separated list'
      custom=$2
      shift ;;
    -h|--help) sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument '$1'" ;;
  esac
  shift
done
if [ -n "$custom" ]; then
  case $custom in
    *[!a-z0-9,-]*) die "features may hold only lower-case letters, digits, hyphens and commas: '$custom'" ;;
  esac
  CONFIGS="$CONFIGS
custom:$custom"
fi

command -v cargo >/dev/null 2>&1 || die 'cargo is not installed'
# rustc resolves to the probe's pinned toolchain inside its directory.
host=$(cd "$PROBE" && rustc -vV | sed -n 's/^host: //p')
toolchain=$(cd "$PROBE" && rustc -V)
size_tool=$(cd "$PROBE" && rustc --print sysroot)/lib/rustlib/$host/bin/llvm-size
[ -x "$size_tool" ] || die "llvm-size is missing. Install it with: (cd $PROBE && rustup component add llvm-tools)"

commit=$(git rev-parse HEAD)
if [ -z "$(git status --porcelain)" ]; then
  clean=true
else
  clean=false
fi

rows=$(mktemp "${TMPDIR:-/tmp}/footprint.XXXXXX")
trap 'rm -f "$rows"' EXIT
trap 'exit 130' INT TERM

# Each build waits while the shared machine is elevated or critical, by the playbook
# scripts/machine-guard.sh applies. It returns at once where no monitor exists.
# The sourced file is checked on its own, as every script under scripts/ is.
# shellcheck disable=SC1091
. scripts/machine-guard.sh

# Build one configuration and append "name features text data bss" to the rows.
measure() {
  name=$1
  features=$2
  machine_guard_wait "footprint build, $name"
  if [ -n "$features" ]; then
    (cd "$PROBE" && cargo build --release --locked --quiet --features "$features")
  else
    (cd "$PROBE" && cargo build --release --locked --quiet)
  fi
  sizes=$("$size_tool" -B "$IMAGE" | awk 'NR == 2 { print $1, $2, $3 }')
  [ -n "$sizes" ] || die "llvm-size reported nothing for $name"
  printf '%s %s %s\n' "$name" "${features:--}" "$sizes" >> "$rows"
}

printf '%s\n' "$CONFIGS" | while IFS=: read -r name features; do
  [ -n "$name" ] || continue
  printf 'footprint: building %s\n' "$name" >&2
  measure "$name" "$features"
done
[ "$(wc -l < "$rows")" -ge 2 ] || die 'the configurations did not all build'

mkdir -p "$PROBE/target"
awk -v commit="$commit" -v clean="$clean" -v toolchain="$toolchain" -v target="$TARGET" '
  NR == 1 { bt = $3; bd = $4; bb = $5 }
  {
    name[NR] = $1; feat[NR] = ($2 == "-") ? "" : $2
    t[NR] = $3; d[NR] = $4; b[NR] = $5
  }
  END {
    printf "{\n  \"schema\": 1,\n  \"commit\": \"%s\",\n  \"clean_tree\": %s,\n", commit, clean
    printf "  \"toolchain\": \"%s\",\n  \"target\": \"%s\",\n", toolchain, target
    printf "  \"profile\": \"release, opt-level s, fat LTO, one codegen unit, panic abort\",\n"
    printf "  \"configurations\": [\n"
    for (i = 1; i <= NR; i++) {
      printf "    {\"name\": \"%s\", \"features\": \"%s\", ", name[i], feat[i]
      printf "\"text\": %d, \"data\": %d, \"bss\": %d, ", t[i], d[i], b[i]
      printf "\"runtime_flash\": %d, \"runtime_static_ram\": %d}%s\n", \
        (t[i] + d[i]) - (bt + bd), (d[i] + b[i]) - (bd + bb), (i < NR) ? "," : ""
    }
    printf "  ]\n}\n"
  }' "$rows" > "$OUT"

printf '\nKeleusma runtime footprint on %s, %s\n' "$TARGET" "$toolchain"
printf 'commit %s, clean tree %s\n\n' "$commit" "$clean"
awk 'NR == 1 { bt = $3; bd = $4; bb = $5
       printf "%-24s %12s %12s   %s\n", "configuration", "flash", "static RAM", "features" }
     NR > 1 { printf "%-24s %12d %12d   %s\n", $1, ($3 + $4) - (bt + bd), ($4 + $5) - (bd + bb), $2 }' "$rows"
printf '\nFigures are bytes above the baseline image. Written to %s\n' "$OUT"

[ "$check" -eq 1 ] || exit 0
[ -f "$BUDGET" ] || die "missing $BUDGET"
over=$(awk '
  FNR == NR { if ($1 !~ /^#/ && NF >= 3) { flash[$1] = $2; ram[$1] = $3; seen[$1] = 1 }; next }
  FNR == 1 { bt = $3; bd = $4; bb = $5; next }
  $1 == "custom" { next }
  {
    f = ($3 + $4) - (bt + bd); r = ($4 + $5) - (bd + bb)
    if (!($1 in seen)) { printf "%s has no ceiling in the budget\n", $1; next }
    if (f > flash[$1]) printf "%s flash %d exceeds its ceiling %d\n", $1, f, flash[$1]
    if (r > ram[$1]) printf "%s static RAM %d exceeds its ceiling %d\n", $1, r, ram[$1]
  }' "$BUDGET" "$rows")
if [ -n "$over" ]; then
  printf '%s\n' "$over" >&2
  die "a footprint exceeds its ceiling in $BUDGET"
fi
printf 'Every configuration is within its ceiling in %s\n' "$BUDGET"
