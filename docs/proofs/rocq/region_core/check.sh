#!/usr/bin/env bash
# Check increment B2 of the proof line, the region core and its reuse-soundness theorem.
#
# What it establishes, in order, failing on the first violation.
#   1. No source holds an unfinished or assumed proof, by a scan for Admitted,
#      admit, Admit Obligations, Abort, Axiom, Parameter, Conjecture, their
#      plurals, bypass_check, and Unset Guard, Positivity or Universe checking.
#   2. Every source compiles, in dependency order, with no warning at all.
#   3. The standalone checker rocqchk re-verifies every compiled module, and its
#      context summary reports no axiom, no unsafe fixpoint, no type-in-type and
#      no assumed positivity for any constant of any module.
#   The scan of step 1 also refuses Set Warnings and the warnings attribute,
#   either of which could silence step 2.
#   4. Every result named below depends on no axiom: Print Assumptions reports
#      it closed under the global context. Section hypotheses are not axioms;
#      they become explicit premises of each closed theorem.
#   5. The number of results reported closed equals the number named, which
#      catches a named result that is not closed. It does not catch a name
#      removed from the list; the context summary of step 3 covers every
#      constant for axioms regardless.
#   6. Statements.v pins the exact type of each headline theorem and proves the
#      definitions of injective and baseline by equations, so an added premise or a
#      changed conclusion fails to compile. A change to another definition a
#      statement mentions does not; see README.md. The script requires the
#      expected number of type pins.
#
# The trusted base is the Rocq kernel, rocqchk, the Stdlib modules imported by
# the sources, List, Arith, Bool and Lia, and the statements as written. Whether the
# region core models Keleusma is not checked here; see README.md.
#
# Usage: docs/proofs/rocq/region_core/check.sh
# Requires rocq and rocqchk on PATH, for example after `eval $(opam env --switch=rocq)`.
# Build output goes to ROCQ_OUT, default <repository>/target/rocq/region_core.

set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$src" rev-parse --show-toplevel)"
out="${ROCQ_OUT:-$root/target/rocq/region_core}"
modules=(Core Shape Sim Reuse Controls Statements)
pins_expected=5
results=(
    Sim.pop_spec Sim.pop_rel Sim.in_handle_site Sim.ts_no_handle Sim.cells_differ Sim.no_regions
    Sim.step_pop Sim.simulation
    Reuse.shape_extend Reuse.check_funcs_inv Reuse.region_check_wf Reuse.reuse_sound
    Controls.loop_ok_accepted Controls.loop_ok_agrees Controls.loop_ok_one_slot Controls.loop_ok_sound
    Controls.loop_bad_rejected Controls.loop_bad_diverges Controls.two_sites_accepted
    Controls.shared_slot_diverges Controls.shared_slot_not_injective
    Controls.reject_yield_handle Controls.reject_store_handle Controls.reject_branch_on_handle
    Controls.reject_arith_on_handle Controls.reject_get_scalar Controls.reject_new_in_function
    Controls.reject_arms_differ Controls.reject_second_live_allocation
    Controls.accept_consumed_then_reallocated Controls.loop_dup_agrees
    Controls.sep_only_rejected_at_allocation Controls.sep_only_diverges Controls.accept_copies_consumed
    Statements.pin_loop_bad_diverges Statements.pin_sep_only_diverges
    Statements.injective_pinned Statements.baseline_pinned
)

command -v rocq >/dev/null || { echo "check.sh: rocq is not on PATH" >&2; exit 1; }
command -v rocqchk >/dev/null || { echo "check.sh: rocqchk is not on PATH" >&2; exit 1; }
echo "check.sh: $(rocq --version | head -1)"

# 1. Forbidden constructs, counted per file so an empty scan is visible. This is a
#    first line only. Step 3's summary is what covers every constant.
forbidden='\<(Admitted|admit|Admit|Abort|Axioms?|Parameters?|Conjectures?|bypass_check)\>|Unset[[:space:]]+(Guard|Positivity|Universe)|Set[[:space:]]+Warnings|warnings?[[:space:]]*=|\<Redirect\>'
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

# The pinned statements. Statements.v must compile, which step 2 checks, and must
# hold exactly the expected number of type pins, so a deleted pin is visible.
pins=$(grep -cE '^(Definition|Lemma) pin_' "$src/Statements.v" || true)
if [ "$pins" -ne "$pins_expected" ]; then
    echo "check.sh: Statements.v holds $pins type pins, expected $pins_expected" >&2
    exit 1
fi
echo "check.sh: Statements.v holds the $pins expected type pins"

# 2. Compile out of tree, so no build output lands beside the sources.
# The output directory is deleted only when this script made it, shown by its marker.
marker=".region-core-check-output"
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
    if ! rocq compile -Q . RegionCore "$m.v" > "$m.log" 2>&1; then
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
    qualified+=("RegionCore.$m")
done
rocqchk -silent -o -Q . RegionCore -norec "${qualified[@]}" > rocqchk.log 2>&1
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
    echo "From RegionCore Require ${modules[*]}."
    for r in "${results[@]}"; do
        echo "Print Assumptions RegionCore.$r."
    done
} > Assumptions.v
rocq compile -Q . RegionCore Assumptions.v > Assumptions.log 2>&1
closed=$(grep -c '^Closed under the global context$' Assumptions.log || true)
if [ "$closed" -ne "${#results[@]}" ]; then
    cat Assumptions.log >&2
    echo "check.sh: $closed of ${#results[@]} results closed under the global context" >&2
    exit 1
fi
echo "check.sh: all $closed of ${#results[@]} named results are closed under the global context"
