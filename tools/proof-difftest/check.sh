#!/usr/bin/env bash
# The differential test between increment A's Rocq model and the shipping Rust verifier.
#
# What it does, in order, failing on the first violation.
#   1. Extracts the interpreter and checker of docs/proofs/rocq/core_bounds to OCaml.
#      Core.v and Check.v are copied unchanged from that directory and compiled with
#      Extract.v, and every compilation must print nothing.
#   2. Compiles the extracted code with the driver model/model_main.ml into the model
#      binary, again printing nothing.
#   3. Builds the Rust harness, a detached package that depends on keleusma by path.
#   4. Runs CASES generated modules from SEED. The harness exits nonzero on any
#      unexplained disagreement and when a stage collapses below its floor.
#   5. Runs three negative controls, each a deliberate defect on one side, and requires
#      that each is reported as a disagreement under the relation it targets.
#
# The relations, the generator's coverage, the results and the limits are in README.md.
#
# Run it as tools/proof-difftest/check.sh from any directory.
# The environment variables SEED, CASES, MIN_BOTH and CONTROL_CASES override the defaults below.
# Requires rocq, ocamlfind and cargo on PATH, for example after
# `eval $(opam env --switch=rocq)`. Cargo runs with RUSTC_WRAPPER empty, because a
# compiler wrapper such as sccache may be refused by a sandbox.

set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$here" rev-parse --show-toplevel)"
core="$root/docs/proofs/rocq/core_bounds"
out="$here/target/check"
seed="${SEED:-20261108}"
cases="${CASES:-20000}"
min_both="${MIN_BOTH:-5000}"
control_cases="${CONTROL_CASES:-2000}"

for tool in rocq ocamlfind cargo; do
    command -v "$tool" >/dev/null || { echo "check.sh: $tool is not on PATH" >&2; exit 1; }
done
echo "check.sh: $(rocq --version | head -1), OCaml $(ocamlfind ocamlopt -version)"

# 1. Extraction, out of tree. The directory is deleted only when this script made it.
marker=".proof-difftest-check-output"
if [ -e "$out" ]; then
    if [ ! -e "$out/$marker" ]; then
        echo "check.sh: $out exists without $marker, refusing to delete it" >&2
        exit 1
    fi
    rm -rf "$out"
fi
mkdir -p "$out/extract"
touch "$out/$marker"
cp "$core/Core.v" "$core/Check.v" "$here/extract/Extract.v" "$out/extract/"
(
    cd "$out/extract"
    for m in Core Check Extract; do
        if ! rocq compile -Q . CoreBounds -output-directory . "$m.v" > "$m.log" 2>&1; then
            cat "$m.log" >&2
            echo "check.sh: $m.v does not compile" >&2
            exit 1
        fi
        if [ -s "$m.log" ]; then
            cat "$m.log" >&2
            echo "check.sh: $m.v compiles with output, treated as a warning" >&2
            exit 1
        fi
    done
    [ -s corebounds.ml ] && [ -s corebounds.mli ] || { echo "check.sh: extraction produced no corebounds.ml" >&2; exit 1; }
)
echo "check.sh: extracted run_b, cycle, cycles, stream_segs, check, wcet and wcmu from $(basename "$core")"

# 2. The model binary.
cp "$here/model/model_main.ml" "$out/extract/"
(
    cd "$out/extract"
    if ! ocamlfind ocamlopt -w +a-4-9-40-41-42-44-45-70 -o model \
        corebounds.mli corebounds.ml model_main.ml > ocaml.log 2>&1 || [ -s ocaml.log ]; then
        cat ocaml.log >&2
        echo "check.sh: the model does not compile cleanly" >&2
        exit 1
    fi
)
model="$out/extract/model"
echo "check.sh: built the model binary"

# 3. The Rust harness.
RUSTC_WRAPPER='' cargo build --quiet --release --manifest-path "$here/Cargo.toml"
harness="$here/target/release/keleusma-proof-difftest"
echo "check.sh: built the Rust harness"

# 4. The differential run.
start=$(date +%s)
if ! "$harness" --model "$model" --out "$out/run" --seed "$seed" --cases "$cases" \
    --min-both "$min_both" | tee "$out/run.log"; then
    echo "check.sh: the differential run failed, details in $out/run/disagreements.txt" >&2
    exit 1
fi
echo "check.sh: differential run passed in $(( $(date +%s) - start )) seconds"

# 5. Negative controls. Each must fail, and its targeted relation must report at least
#    one unexplained disagreement.
controls=(
    "accept-recursion:R1 acceptance"
    "rust-cost:R3 shipping-bound"
    "swap-branches:R4 lockstep"
)
for c in "${controls[@]}"; do
    mode="${c%%:*}"
    relation="${c#*:}"
    status=0
    "$harness" --model "$model" --out "$out/control-$mode" --seed "$seed" \
        --cases "$control_cases" --min-both 1 --break "$mode" > "$out/control-$mode.log" || status=$?
    if [ "$status" -ne 1 ]; then
        cat "$out/control-$mode.log" >&2
        echo "check.sh: negative control $mode exited with $status, expected 1" >&2
        exit 1
    fi
    n=$(sed -n "s/^check .*  $relation, unexplained \([0-9]*\)$/\1/p" "$out/control-$mode.log")
    if [ -z "$n" ] || [ "$n" -eq 0 ]; then
        cat "$out/control-$mode.log" >&2
        echo "check.sh: negative control $mode was not reported under $relation" >&2
        exit 1
    fi
    echo "check.sh: negative control $mode reported $n disagreement(s) under $relation"
done
echo "check.sh: PASS"
