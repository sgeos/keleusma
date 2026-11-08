#!/usr/bin/env bash
# Check the shared-slot extension of increment B2, reuse under plans that share slots.
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
#      statements of the controls, so an added premise or a
#      changed conclusion fails to compile. A change to another definition a
#      statement mentions does not; see README.md. The script requires the
#      expected number of type pins.
#
# The trusted base is the Rocq kernel, rocqchk, the Stdlib modules imported by
# the sources, List, Arith, Bool and Lia, and the statements as written. Whether the
# region core models Keleusma is not checked here; see README.md.
#
# Usage: docs/proofs/rocq/shared_slots/check.sh
# Requires rocq and rocqchk on PATH, for example after `eval $(opam env --switch=rocq)`.
# Build output goes to ROCQ_OUT, default <repository>/target/rocq/shared_slots.

set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$src" rev-parse --show-toplevel)"
out="${ROCQ_OUT:-$root/target/rocq/shared_slots}"
deps=(Core Shape Sim Reuse Controls)
modules=(SharedShape SharedSim Shared ArmShape ArmSim Arm Controls Statements)
pins_expected=7
results=(
    SharedSim.shape_false_eq SharedSim.existsb_false_all SharedSim.cells_differ_shared
    SharedSim.simulation_shared
    Shared.region_checkP_wf Shared.reuse_sound_shared Shared.conflict_injective
    Shared.existsb_conflict Shared.shape_injective_eq Shared.shared_generalises
    Controls.Pshare_not_injective Controls.arms_accepted Controls.arms_agree Controls.arms_one_slot
    Controls.arms_sound Controls.sequential_accepted_and_agrees Controls.co_live_rejected
    Controls.co_live_diverges Controls.choose_needs_join Controls.choose_agrees Controls.choose_sound
    Controls.co_live_rejected_by_join
    ArmSim.ajoin_sound_l ArmSim.ajoin_sound_r ArmSim.step_popA ArmSim.simulation_arm
    Arm.region_checkA_wf Arm.reuse_sound_arm Arm.shape_embed Arm.arm_generalises Arm.arm_contains_b2
    Statements.pin_arm_group_covered Statements.pin_co_live_rejected Statements.pin_choose_covered
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
marker=".shared-slots-check-output"
if [ -e "$out" ]; then
    if [ ! -e "$out/$marker" ]; then
        echo "check.sh: $out exists without $marker, refusing to delete it" >&2
        exit 1
    fi
    rm -rf "$out"
fi
mkdir -p "$out"
touch "$out/$marker"
mkdir -p "$out/own"
for m in "${modules[@]}"; do
    cp "$src/$m.v" "$out/own/"
done
# The dependency, increment B2, is built and rechecked from its own sources.
mkdir -p "$out/dep"
for m in "${deps[@]}"; do
    cp "$src/../region_core/$m.v" "$out/dep/"
done
(
    cd "$out/dep"
    for m in "${deps[@]}"; do
        if ! rocq compile -Q . RegionCore "$m.v" > "$m.log" 2>&1 || [ -s "$m.log" ]; then
            cat "$m.log" >&2
            echo "check.sh: dependency $m.v does not compile cleanly" >&2
            exit 1
        fi
    done
)
cd "$out/own"
for m in "${modules[@]}"; do
    if ! rocq compile -Q ../dep RegionCore -Q . SharedSlots "$m.v" > "$m.log" 2>&1; then
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
    qualified+=("SharedSlots.$m")
done
rocqchk -silent -o -Q ../dep RegionCore -Q . SharedSlots -norec "${qualified[@]}" > rocqchk.log 2>&1
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
    echo "From SharedSlots Require ${modules[*]}."
    for r in "${results[@]}"; do
        echo "Print Assumptions SharedSlots.$r."
    done
} > Assumptions.v
rocq compile -Q ../dep RegionCore -Q . SharedSlots Assumptions.v > Assumptions.log 2>&1
closed=$(grep -c '^Closed under the global context$' Assumptions.log || true)
if [ "$closed" -ne "${#results[@]}" ]; then
    cat Assumptions.log >&2
    echo "check.sh: $closed of ${#results[@]} results closed under the global context" >&2
    exit 1
fi
echo "check.sh: all $closed of ${#results[@]} named results are closed under the global context"
