#!/usr/bin/env bash
# Build the policy into a native object, link a C host against it, and run it.
#
# Run from the native_codegen directory:  examples/motor_policy/run.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd -P)"
out="${1:-/tmp/kel-motor-policy}"
cc="${CC:-cc}"

command -v "$cc" >/dev/null || {
  echo "no C compiler found as '$cc'; set CC or install one." >&2
  exit 1
}

mkdir -p "$out"
cargo run --quiet --example emit_object -- "$here/policy.kel" "$out"
"$cc" -I "$out" -O2 -o "$out/motor_host" "$here/host.c" "$out/policy.o"
echo
"$out/motor_host"
