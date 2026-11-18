#!/usr/bin/env bash
# Check the memory accounting of increment B2's region core, Theorem C at the
# core level: the cells a reuse run uses, epoch by epoch.
#
# What it establishes, in order, failing on the first violation.
#   1. No source holds an unfinished or assumed proof, by a scan for Admitted,
#      admit, Admit Obligations, Abort, Axiom, Parameter, Conjecture, their
#      plurals, bypass_check, and Unset Guard, Positivity or Universe checking.
#      The scan also refuses Set Warnings, the warnings attribute and Redirect,
#      any of which could silence step 2.
#   2. Every source compiles, in dependency order, with no warning at all.
#   3. The standalone checker rocqchk re-verifies every compiled module, and its
#      context summary reports no axiom, no unsafe fixpoint, no type-in-type and
#      no assumed positivity for any constant of any module.
#   4. Every result named below depends on no axiom: Print Assumptions reports
#      it closed under the global context.
#   5. The number of results reported closed equals the number named.
#   6. Statements.v pins the exact type of each headline result, for injective
#      plans and for plans the join checker of ../shared_slots accepts, and of the
#      controls, and fixes the definitions the conclusions rest on by proved
#      equations. The script requires the expected number of type pins.
#
# The trusted base is the Rocq kernel, rocqchk, the Stdlib modules imported by
# the sources, List, Arith and Lia, the definitions of increment B2 and of its
# shared-slot extension, and the
# statements as written. Whether the region core models Keleusma is not checked
# here; see README.md.
#
# Usage: docs/proofs/rocq/accounting/check.sh
# Requires rocq and rocqchk on PATH, for example after `eval $(opam env --switch=rocq)`.
# Build output goes to ROCQ_OUT, default <repository>/target/rocq/accounting.

set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$src" rev-parse --show-toplevel)"
out="${ROCQ_OUT:-$root/target/rocq/accounting}"
deps=(Core Shape Sim Reuse Controls)
modules=(Accounting Footprint Controls ArmFootprint ArmControls Claims Statements)
ss_deps=(SharedShape SharedSim Shared ArmShape ArmSim Arm Controls)
pins_expected=23
results=(
    Accounting.indexed_from_fst Accounting.indexed_app Accounting.nodup_length_le
    Accounting.NoDup_allocs_at Accounting.baseline_footprint Accounting.footprint_le_baseline
    Accounting.NoDup_below Accounting.footprint_slot_bound
    Accounting.indexed_from_app Accounting.NoDup_fst_filter Accounting.NoDup_map_bump
    Footprint.within_same Footprint.pop_keeps Footprint.new_within Footprint.run_within
    Footprint.mem_within Footprint.same_allocations Footprint.reuse_footprint_bounded
    Controls.loop_ok_footprints Controls.loop_ok_bound_is_tight Controls.slots_alone_do_not_bound
    Controls.P0_injective Controls.loop_ok_footprint_bounded
    ArmFootprint.same_allocations_arm ArmFootprint.reuse_footprint_bounded_arm
    ArmControls.sequential_accepted_by_join ArmControls.sequential_shares_one_cell
    ArmControls.sequential_footprint_bounded
    Claims.footprint_at_pinned Claims.cell_at_pinned Claims.allocs_at_pinned
    Claims.indexed_pinned Claims.unplanned_at_pinned Claims.within_pinned
    Claims.run_of_pinned Claims.fp_pinned Claims.seq_run_pinned
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
for m in Statements Claims Controls; do
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
# Counted with comments removed, as the scan reads the sources, so a pin
# commented out is not counted.
pins=$(flat "$src/Statements.v" | { grep -oE '\<(Definition|Lemma) pin_' || true; } | wc -l | tr -d ' ')
if [ "$pins" -ne "$pins_expected" ]; then
    echo "check.sh: Statements.v holds $pins type pins, expected $pins_expected" >&2
    exit 1
fi
echo "check.sh: Statements.v holds the $pins expected type pins"

# 2. Compile out of tree, so no build output lands beside the sources.
# The output directory is deleted only when this script made it, shown by its marker.
marker=".accounting-check-output"
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
# The second dependency, the shared-slot extension, is built and rechecked from its own sources.
mkdir -p "$out/ss"
for m in "${ss_deps[@]}"; do
    cp "$src/../shared_slots/$m.v" "$out/ss/"
done
(
    cd "$out/ss"
    for m in "${ss_deps[@]}"; do
        if ! rocq compile -Q ../dep RegionCore -Q . SharedSlots "$m.v" > "$m.log" 2>&1 || [ -s "$m.log" ]; then
            cat "$m.log" >&2
            echo "check.sh: dependency $m.v does not compile cleanly" >&2
            exit 1
        fi
    done
)
cd "$out/own"
for m in "${modules[@]}"; do
    if ! rocq compile -Q ../dep RegionCore -Q ../ss SharedSlots -Q . Accounting "$m.v" > "$m.log" 2>&1; then
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
    qualified+=("Accounting.$m")
done
rocqchk -silent -o -Q ../dep RegionCore -Q ../ss SharedSlots -Q . Accounting -norec "${qualified[@]}" > rocqchk.log 2>&1
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
    echo "From Accounting Require ${modules[*]}."
    for r in "${results[@]}"; do
        echo "Print Assumptions Accounting.$r."
    done
} > Assumptions.v
rocq compile -Q ../dep RegionCore -Q ../ss SharedSlots -Q . Accounting Assumptions.v > Assumptions.log 2>&1
closed=$(grep -c '^Closed under the global context$' Assumptions.log || true)
if [ "$closed" -ne "${#results[@]}" ]; then
    cat Assumptions.log >&2
    echo "check.sh: $closed of ${#results[@]} results closed under the global context" >&2
    exit 1
fi
echo "check.sh: all $closed of ${#results[@]} named results are closed under the global context"
