#!/usr/bin/env bash
# Build the self-hosted lexer as a static library, link a Rust host against it,
# and lex a one-line Keleusma source.
#
# Run from the native_codegen directory:  examples/lexer_host/run.sh [out-dir]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
out="${1:-${TMPDIR:-/tmp}/kel-lexer-host}"
rustc="${RUSTC:-rustc}"
ar="${AR:-ar}"

for tool in "$rustc" "$ar"; do
  command -v "$tool" >/dev/null || {
    echo "'$tool' was not found; set RUSTC or AR." >&2
    exit 1
  }
done

mkdir -p "$out"
cargo run --quiet --example emit_static_lib -- \
  "$here/../../../src/selfhost/kel/lexer.kel" kel_lexer "$out"
rm -f "$out/libkel_lexer.a"
"$ar" rcs "$out/libkel_lexer.a" "$out/kel_lexer.o"
KEL_LIBRARY_BINDINGS="$out/kel_lexer.rs" "$rustc" --edition 2024 -D warnings \
  -L "native=$out" -o "$out/lexer_host" "$here/host.rs"
printf 'loop main(t: Word) -> Word { yield t + 1 }\n' > "$out/source.kel"
# Into a file, not a pipe: `head` closing a pipe under pipefail exits 141.
"$out/lexer_host" "$out" 40 "$out/source.kel" > "$out/yields.txt"
sed -n 1,12p "$out/yields.txt"
echo "all $(grep -c '^Y' "$out/yields.txt") yields are in $out/yields.txt"
echo "deepest machine stack measured, in bytes: $(sed -n 's/^S //p' "$out/yields.txt")"
echo "shared buffer written to $out/shared_0.bin"
