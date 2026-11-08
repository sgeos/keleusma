#!/usr/bin/env bash
# Check the instantiation of increment B1's theorems by increment B2's region core.
#
# What it establishes, in order, failing on the first violation.
#   1. No source, of this directory or of the dependencies it compiles, holds an
#      unfinished or assumed proof, by a scan for Admitted, admit, Admit
#      Obligations, Abort, Axiom, Parameter, Conjecture, their plurals,
#      bypass_check, Unset Guard, Positivity or Universe checking, Set Warnings
#      and the warnings attribute. The scan reads each source with its comments
#      removed and its whitespace collapsed. Statements.v and Witnesses.v open no
#      section and declare no hypothesis, variable or context.
#   2. Every source compiles, the dependencies first from their own directories
#      under their logical names RegionReuse and RegionCore, then this
#      directory under the logical name Instance, with no output at all.
#   3. The standalone checker rocqchk re-verifies every compiled module, and its
#      context summary reports no axiom, no unsafe fixpoint, no type-in-type and
#      no assumed positivity for any constant of any module.
#   4. Every result named below depends on no axiom, by Print Assumptions.
#   5. The number of results reported closed equals the number named.
#   6. Statements.v holds the expected number of type pins. Each pin fixes the
#      exact type of a headline theorem, so an added premise or a changed
#      conclusion fails to compile. A change to a definition a statement
#      mentions does not; see README.md.
#
# The trusted base is the Rocq kernel, rocqchk, the Stdlib modules List, Arith,
# Bool and Lia, the definitions of increments B1 and B2 that the statements
# mention, the definitions of this directory, and the statements as written.
#
# Usage: docs/proofs/rocq/instance/check.sh
# Requires rocq and rocqchk on PATH, for example after `eval $(opam env --switch=rocq)`.
# Build output goes to ROCQ_OUT, default <repository>/target/rocq/instance.

set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$src" rev-parse --show-toplevel)"
out="${ROCQ_OUT:-$root/target/rocq/instance}"
b1_src="$src/../region_reuse"
b2_src="$src/../region_core"
b1_modules=(Trace)
b2_modules=(Core Shape Sim Reuse Controls)
modules=(Encode Causal Memory Machine Safety Instance Witnesses Statements)
pins_expected=10
results=(
    Encode.orun_extends Encode.ocycles_extends Encode.upto_app Encode.upto_prefix
    Encode.upto_nth Encode.read_before
    Causal.causal Causal.causal_cycles
    Memory.deref_read Memory.mem_sim Memory.cycles_sim Memory.core_is_oracle
    Machine.answer_at Machine.agree_prefix Machine.next_follows Machine.gen_is_run Machine.replay
    Safety.alloc_unique Safety.good_snoc Safety.slot_safe_firstn Safety.safe_sim
    Safety.checked_slot_safe
    Instance.instance_sound Instance.core_log_is_B1_obs Instance.reuse_sound_via_B1
    Witnesses.loop_ok_events Witnesses.loop_bad_unsound_in_B1 Witnesses.loop_bad_plan_injective
    Witnesses.shared_slot_unsound_in_B1 Witnesses.shared_slot_premise_fails
    Statements.pin_instance_sound Statements.pin_checked_slot_safe Statements.pin_gen_is_run
    Statements.pin_core_is_oracle Statements.pin_core_log_is_B1_obs Statements.pin_replay
    Statements.pin_reuse_sound_via_B1 Statements.pin_same_as_B2
    Statements.pin_loop_bad_unsound_in_B1 Statements.pin_shared_slot_unsound_in_B1
    Statements.sel0_pinned Statements.baseline_pinned
)

command -v rocq >/dev/null || { echo "check.sh: rocq is not on PATH" >&2; exit 1; }
command -v rocqchk >/dev/null || { echo "check.sh: rocqchk is not on PATH" >&2; exit 1; }
echo "check.sh: $(rocq --version | head -1)"

# 1. Forbidden constructs, counted per file so an empty scan is visible. This is a
#    first line only. Step 3's summary is what covers every constant.
forbidden='\<(Admitted|admit|Admit|Abort|Axioms?|Parameters?|Conjectures?|bypass_check)\>|Unset[[:space:]]+(Guard|Positivity|Universe)|Set[[:space:]]+Warnings|warnings?[[:space:]]*=|\<Redirect\>'
flat() {
    perl -0777 -pe 's/\(\*.*?\*\)/ /gs' "$1" | tr -s '[:space:]' ' '
}
sources=()
for m in "${b1_modules[@]}"; do sources+=("$b1_src/$m.v"); done
for m in "${b2_modules[@]}"; do sources+=("$b2_src/$m.v"); done
for m in "${modules[@]}"; do sources+=("$src/$m.v"); done
hits=0
for f in "${sources[@]}"; do
    n=$(flat "$f" | { grep -oE "$forbidden" || true; } | wc -l | tr -d ' ')
    if [ "$n" -ne 0 ]; then
        flat "$f" | grep -oE "$forbidden" | sed "s|^|$(basename "$f"): |" >&2
        hits=$((hits + n))
    fi
done
for m in Statements Witnesses; do
    n=$(flat "$src/$m.v" | { grep -oE '\<(Section|Hypothes[ie]s|Hypothesis|Variables?|Context)\>' || true; } | wc -l | tr -d ' ')
    if [ "$n" -ne 0 ]; then
        echo "check.sh: $m.v opens a section or declares a hypothesis, variable or context" >&2
        hits=$((hits + n))
    fi
done
if [ "$hits" -ne 0 ]; then
    echo "check.sh: $hits forbidden construct(s)" >&2
    exit 1
fi
echo "check.sh: scanned ${#sources[@]} sources, no forbidden construct"

pins=$(grep -cE '^(Definition|Lemma) pin_' "$src/Statements.v" || true)
if [ "$pins" -ne "$pins_expected" ]; then
    echo "check.sh: Statements.v holds $pins type pins, expected $pins_expected" >&2
    exit 1
fi
echo "check.sh: Statements.v holds the $pins expected type pins"

# 2. Compile out of tree. The output directory is deleted only when this script
#    made it, shown by its marker.
marker=".instance-check-output"
if [ -e "$out" ]; then
    if [ ! -e "$out/$marker" ]; then
        echo "check.sh: $out exists without $marker, refusing to delete it" >&2
        exit 1
    fi
    rm -rf "$out"
fi
mkdir -p "$out/RegionReuse" "$out/RegionCore" "$out/Instance"
touch "$out/$marker"
for m in "${b1_modules[@]}"; do cp "$b1_src/$m.v" "$out/RegionReuse/"; done
for m in "${b2_modules[@]}"; do cp "$b2_src/$m.v" "$out/RegionCore/"; done
for m in "${modules[@]}"; do cp "$src/$m.v" "$out/Instance/"; done
cd "$out"
flags=(-Q RegionReuse RegionReuse -Q RegionCore RegionCore -Q Instance Instance)
compile() {
    local dir="$1" m="$2"
    if ! rocq compile "${flags[@]}" "$dir/$m.v" > "$dir/$m.log" 2>&1; then
        cat "$dir/$m.log" >&2
        echo "check.sh: $dir/$m.v does not compile" >&2
        exit 1
    fi
    if [ -s "$dir/$m.log" ]; then
        cat "$dir/$m.log" >&2
        echo "check.sh: $dir/$m.v compiles with output, treated as a warning" >&2
        exit 1
    fi
}
for m in "${b1_modules[@]}"; do compile RegionReuse "$m"; done
for m in "${b2_modules[@]}"; do compile RegionCore "$m"; done
for m in "${modules[@]}"; do compile Instance "$m"; done
total=$(( ${#b1_modules[@]} + ${#b2_modules[@]} + ${#modules[@]} ))
echo "check.sh: compiled $total modules with no warning"

# 3. Independent recheck of every compiled module, dependencies included.
qualified=()
for m in "${b1_modules[@]}"; do qualified+=("RegionReuse.$m"); done
for m in "${b2_modules[@]}"; do qualified+=("RegionCore.$m"); done
for m in "${modules[@]}"; do qualified+=("Instance.$m"); done
rocqchk -silent -o "${flags[@]}" -norec "${qualified[@]}" > rocqchk.log 2>&1
required=(
    '* Axioms: <none>'
    '* Constants/Inductives relying on type-in-type: <none>'
    '* Constants/Inductives relying on unsafe (co)fixpoints: <none>'
    '* Inductives whose positivity is assumed: <none>'
)
for line in "${required[@]}"; do
    if [ "$(grep -cxF "$line" rocqchk.log || true)" -ne 1 ]; then
        cat rocqchk.log >&2
        echo "check.sh: rocqchk summary lacks: $line" >&2
        exit 1
    fi
done
echo "check.sh: rocqchk re-verified $total modules, no axiom, no unsafe fixpoint, no type-in-type, no assumed positivity"

# 4 and 5. Per-result trusted base.
{
    echo "From Instance Require ${modules[*]}."
    for r in "${results[@]}"; do
        echo "Print Assumptions Instance.$r."
    done
} > Assumptions.v
rocq compile "${flags[@]}" Assumptions.v > Assumptions.log 2>&1
closed=$(grep -c '^Closed under the global context$' Assumptions.log || true)
if [ "$closed" -ne "${#results[@]}" ]; then
    cat Assumptions.log >&2
    echo "check.sh: $closed of ${#results[@]} results closed under the global context" >&2
    exit 1
fi
echo "check.sh: all $closed of ${#results[@]} named results are closed under the global context"
