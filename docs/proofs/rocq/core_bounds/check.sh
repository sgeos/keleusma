#!/usr/bin/env bash
# Check increment A of the proof line, the Rocq core semantics and its bound theorem.
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
#      definitions of good, wcet and wcmu by equations, so an added premise or a
#      changed conclusion fails to compile. A change to another definition a
#      statement mentions does not; see README.md. The script requires the
#      expected number of type pins.
#
# The trusted base is the Rocq kernel, rocqchk, the Stdlib modules imported by
# the sources, List, Arith and Lia, and the statements as written. Whether the
# core models Keleusma's verifier is not checked here; see README.md.
#
# Usage: docs/proofs/rocq/core_bounds/check.sh
# Requires rocq and rocqchk on PATH, for example after `eval $(opam env --switch=rocq)`.
# Build output goes to ROCQ_OUT, default <repository>/target/rocq/core_bounds.

set -euo pipefail

src="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$src" rev-parse --show-toplevel)"
out="${ROCQ_OUT:-$root/target/rocq/core_bounds}"
modules=(Core Check Algebra Soundness Controls Statements)
pins_expected=4
results=(
    Algebra.summary_join Algebra.le_seq Algebra.le_join_l Algebra.le_join_r Algebra.summary_elements
    Soundness.runs_within_bounds Soundness.fuel_suffices Soundness.abs_extend Soundness.check_funcs_inv
    Soundness.function_bounded Soundness.fn_never_yields Soundness.cycle_bounded Soundness.stream_bounded
    Controls.ex_accepted Controls.ex_figures Controls.ex_two_cycles Controls.ex_two_cycles_host0
    Controls.ex_bounds_attained Controls.ex_stream_bounded
    Controls.reject_recursion Controls.reject_forward_call Controls.reject_yielding_fn
    Controls.reject_fn_calling_yield Controls.reject_unproductive_path Controls.reject_unproductive_main
    Controls.reject_zero_iteration_yield Controls.accept_one_iteration_yield
    Controls.junction_alone_attains Controls.loop_iterations_counted
    Statements.good_pinned Statements.wcet_pinned Statements.wcmu_pinned
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
marker=".core-bounds-check-output"
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
    if ! rocq compile -Q . CoreBounds "$m.v" > "$m.log" 2>&1; then
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
    qualified+=("CoreBounds.$m")
done
rocqchk -silent -o -Q . CoreBounds -norec "${qualified[@]}" > rocqchk.log 2>&1
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
    echo "From CoreBounds Require ${modules[*]}."
    for r in "${results[@]}"; do
        echo "Print Assumptions CoreBounds.$r."
    done
} > Assumptions.v
rocq compile -Q . CoreBounds Assumptions.v > Assumptions.log 2>&1
closed=$(grep -c '^Closed under the global context$' Assumptions.log || true)
if [ "$closed" -ne "${#results[@]}" ]; then
    cat Assumptions.log >&2
    echo "check.sh: $closed of ${#results[@]} results closed under the global context" >&2
    exit 1
fi
echo "check.sh: all $closed of ${#results[@]} named results are closed under the global context"
