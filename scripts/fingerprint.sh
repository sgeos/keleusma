#!/usr/bin/env bash
#
# Report or regenerate the format fingerprint.
#
# BYTECODE_VERSION is frozen at 2 across releases, so it cannot distinguish two
# releases that both declare 2. FORMAT_FINGERPRINT can, because every release
# gets a fresh random value. This script is how it is read and rolled.
#
#   scripts/fingerprint.sh            value in the working tree
#   scripts/fingerprint.sh <commit>   value that commit uses
#   scripts/fingerprint.sh --new      roll a new one and rewrite the constant
#
# Rolling is a RELEASE step. Doing it mid-cycle is harmless but pointless: it
# makes bytecode from earlier in the same cycle refuse to load.

set -euo pipefail

SRC="src/bytecode.rs"
CONST="FORMAT_FINGERPRINT"

# Match only the definition, never a mention of the name in prose.
# Prints the value, or a clear absence. Silence would read as success and this
# is meant to answer "what does commit X use", including "X predates it".
extract() {
  local out
  out=$(grep -oE "pub const ${CONST}: u32 = 0x[0-9A-Fa-f_]+;" | grep -oE '0x[0-9A-Fa-f_]+' || true)
  if [ -z "$out" ]; then
    echo "(no ${CONST} at this revision)"
    return 1
  fi
  echo "$out"
}

case "${1:---show}" in
  --new)
    old=$(extract < "$SRC")
    # Reject 0 and all-ones: zero is what a pre-fingerprint module carries, and
    # all-ones is the sort of value a wiped or padded field lands on, so neither
    # should ever be a live fingerprint.
    new=$(python3 - <<'PY'
import secrets
v = 0
while v in (0, 0xFFFFFFFF):
    v = secrets.randbits(32)
print(f"0x{v >> 16:04X}_{v & 0xFFFF:04X}")
PY
)
    # Anchored to the definition so a mention in a doc comment is untouched.
    python3 - "$SRC" "$CONST" "$new" <<'PY'
import pathlib, re, sys
path, const, new = sys.argv[1], sys.argv[2], sys.argv[3]
p = pathlib.Path(path)
t = p.read_text()
pat = re.compile(rf"(pub const {re.escape(const)}: u32 = )0x[0-9A-Fa-f_]+(;)")
t, n = pat.subn(rf"\g<1>{new}\g<2>", t)
assert n == 1, f"expected exactly one definition, rewrote {n}"
p.write_text(t)
PY
    echo "fingerprint: ${old} -> ${new}"
    echo
    echo "Now: update the pinned value in the tests that assert it, and the golden"
    echo "wire bytes, then run scripts/release-gate.sh. Both are SUPPOSED to move."
    ;;
  --show)
    extract < "$SRC"
    ;;
  -h|--help)
    sed -n '3,17p' "$0" | sed 's/^# \{0,1\}//'
    ;;
  *)
    # Any commit-ish. Reads that commit's source, not the working tree.
    git show "$1:$SRC" | extract
    ;;
esac
