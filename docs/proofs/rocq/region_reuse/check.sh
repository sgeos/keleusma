#!/usr/bin/env bash
# Check the Rocq mechanisation of Part I of the composite-region-reuse proof.
#
# What it establishes, in order, failing on the first violation.
#   1. No source holds an unfinished or assumed proof, by a scan for Admitted,
#      admit, Admit Obligations, Abort, Axiom, Parameter, Conjecture, their
#      plurals, bypass_check, and Unset Guard, Positivity or Universe checking.
#   2. Every source compiles, in dependency order, with no warning at all.
#   3. The standalone checker rocqchk re-verifies every compiled module, and its
#      context summary reports no axiom, no unsafe fixpoint, no type-in-type and
#      no assumed positivity for any constant of any module.
#   4. Every result named below depends on no axiom: Print Assumptions reports
#      it closed under the global context. Section hypotheses are not axioms;
#      they become explicit premises of each closed theorem.
#   5. The number of results reported closed equals the number named, so a
#      result that silently dropped out of the list cannot pass.
#
# The trusted base is the Rocq kernel, rocqchk, the Stdlib modules imported by
# the sources, List, Arith and Lia, and the statements as written. Whether the
# abstract machine models Keleusma is not checked here; see README.md.
#
# Usage: docs/proofs/rocq/region_reuse/check.sh
# Requires rocq and rocqchk on PATH, for example after `eval $(opam env --switch=rocq)`.
# Build output goes to ROCQ_OUT, default <repository>/target/rocq/region_reuse.

set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$src" rev-parse --show-toplevel)"
out="${ROCQ_OUT:-$root/target/rocq/region_reuse}"
modules=(Bound Trace Clearance Theorems Controls)
results=(
    Bound.A1_branch_bound Bound.A1s_static_bound Bound.cond_one_arm Bound.C_plan_bound
    Bound.B2a_accounting Bound.footprint_below_bump Bound.footprint_above_bump
    Trace.lemma4_composition Trace.plan_follows_baseline Trace.slot_theorem
    Trace.A2_arm_overlap
    Clearance.lemma1_provenance Clearance.lemma2_confinement_clearance
    Clearance.lemma3_refined_clearance Clearance.lemma5_unconditional_clearance
    Theorems.theorem_A2 Theorems.theorem_B1 Theorems.theorem_B1r Theorems.corollary_A3
    Theorems.theorem_B2 Theorems.corollary_B2b_hybrid Theorems.lemma4_plan_sound
    Theorems.plan_runs_baseline_events Theorems.alloc_at_inj Theorems.element_safe
    Trace.memory_agrees
    Controls.reuse_without_separation_is_unsound Controls.baseline_reads_first
    Controls.plan_reads_second Controls.unsound_plan_fails_the_premises
    Controls.premises_of_B1_are_satisfiable Controls.instance_reads_each_region
)

command -v rocq >/dev/null || { echo "check.sh: rocq is not on PATH" >&2; exit 1; }
command -v rocqchk >/dev/null || { echo "check.sh: rocqchk is not on PATH" >&2; exit 1; }
echo "check.sh: $(rocq --version | head -1)"

# 1. Forbidden constructs, counted per file so an empty scan is visible. This is a
#    first line only. Step 3's summary is what covers every constant.
forbidden='\<(Admitted|admit|Admit|Abort|Axioms?|Parameters?|Conjectures?|bypass_check)\>|Unset[[:space:]]+(Guard|Positivity|Universe)|Set[[:space:]]+Warnings|warnings[[:space:]]*='
# A source with comments removed and whitespace collapsed to single spaces.
flat() {
    perl -0777 -pe 's/\(\*.*?\*\)/ /gs' "$1" | tr -s '[:space:]' ' '
}
hits=0
for m in "${modules[@]}"; do
    # The scan reads the source with its comments removed and its whitespace
    # collapsed, so a construct split across lines or by a comment is still
    # matched.
    n=$(flat "$src/$m.v" | { grep -oE "$forbidden" || true; } | wc -l | tr -d ' ')
    if [ "$n" -ne 0 ]; then
        flat "$src/$m.v" | grep -oE "$forbidden" | sed "s|^|$m.v: |" >&2
        hits=$((hits + n))
    fi
done
# The pins and the controls may not open a section or declare a hypothesis,
# variable or context, which could discharge a premise added to a pinned
# statement.
for m in Statements Controls; do
    [ -f "$src/$m.v" ] || continue
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
echo "check.sh: scanned ${#modules[@]} sources, no forbidden construct"

# 2. Compile out of tree, so no build output lands beside the sources.
# The output directory is deleted only when this script made it, shown by its marker.
marker=".region-reuse-check-output"
if [ -e "$out" ]; then
    if [ ! -e "$out/$marker" ]; then
        echo "check.sh: $out exists without $marker, refusing to delete it" >&2
        exit 1
    fi
    rm -rf "$out"
fi
mkdir -p "$out"
touch "$out/$marker"
for m in "${modules[@]}"; do
    cp "$src/$m.v" "$out/"
done
cd "$out"
for m in "${modules[@]}"; do
    if ! rocq compile -Q . RegionReuse "$m.v" > "$m.log" 2>&1; then
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
echo "check.sh: compiled ${#modules[@]} modules with no warning"

# 3. Independent recheck of the compiled proofs. rocqchk accepts an axiom or an
#    unguarded fixpoint and only reports it, so its context summary must report
#    none of either, for every constant of every module, listed or not.
qualified=()
for m in "${modules[@]}"; do
    qualified+=("RegionReuse.$m")
done
rocqchk -silent -o -Q . RegionReuse -norec "${qualified[@]}" > rocqchk.log 2>&1
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
echo "check.sh: rocqchk re-verified ${#modules[@]} modules, no axiom, no unsafe fixpoint, no type-in-type, no assumed positivity"

# 4 and 5. Per-result trusted base.
{
    echo "From RegionReuse Require ${modules[*]}."
    for r in "${results[@]}"; do
        echo "Print Assumptions RegionReuse.$r."
    done
} > Assumptions.v
rocq compile -Q . RegionReuse Assumptions.v > Assumptions.log 2>&1
closed=$(grep -c '^Closed under the global context$' Assumptions.log || true)
if [ "$closed" -ne "${#results[@]}" ]; then
    cat Assumptions.log >&2
    echo "check.sh: $closed of ${#results[@]} results closed under the global context" >&2
    exit 1
fi
echo "check.sh: all $closed of ${#results[@]} named results are closed under the global context"
