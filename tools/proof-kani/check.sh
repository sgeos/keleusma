#!/usr/bin/env bash
# Runs every Kani harness in this package and fails unless each one gives its
# expected result.
#
# Expected to verify. Every harness in PASSING must report VERIFICATION
# SUCCESSFUL with no unsatisfiable cover statement, and the number that do
# must equal the length of PASSING.
#
# Expected to fail. Every harness in FAILING must report VERIFICATION FAILED
# with each of the failure descriptions listed for it. These are the finding
# harnesses and the negative control described in README.md. They are
# excluded from the pass count.
#
# The two lists together must name every `#[kani::proof]` in src/, so that a
# harness added without being listed fails this script instead of being
# silently skipped.
#
# Usage: tools/proof-kani/check.sh
# Environment: KANI_JOBS sets how many harnesses run at once, default 1.
# One harness can hold about eight gigabytes of memory while the model checker
# runs, and the development machine is shared, so serial is the safe default.
# Each harness starts only when `gate_machine` below allows it. Where the
# `machine-status` monitor is absent, KANI_SWAP_LIMIT_MB, default 10240, makes
# each harness wait to start while more swap than that is in use.
# KANI_LOG_DIR sets where the per-harness logs go, default a fresh temporary
# directory.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

# The global compiler cache wrapper is refused inside the sandbox this package
# is developed in, and Kani does not need it.
export RUSTC_WRAPPER=

# Waits until the machine can take another harness, and returns nonzero when
# it must not start one at all.
#
# Where the `machine-status` monitor is installed, it decides. Its exit status
# is 0 for ok, 1 for elevated, 2 for critical and 3 for stale. A harness starts
# only at ok. At elevated this waits and polls again every 60 seconds. At
# critical or stale it refuses, so that the run reports the condition instead
# of adding load. Where the monitor is absent, the swap figure from
# `sysctl vm.swapusage` decides, and where neither can be read the harness
# starts ungated.
gate_machine() {
    local level
    if command -v machine-status > /dev/null 2>&1; then
        while true; do
            if machine-status --level > /dev/null 2>&1; then
                return 0
            else
                level=$?
            fi
            if [ "$level" -eq 1 ]; then
                echo "note: machine-status reports elevated, waiting" >&2
                sleep 60
            else
                echo "note: machine-status exit $level, critical or stale, not starting" >&2
                return 1
            fi
        done
    fi
    wait_for_swap
}

# Waits while more than KANI_SWAP_LIMIT_MB of swap is in use. Where the swap
# figure cannot be read, as inside some sandboxes, it says so and goes on.
wait_for_swap() {
    local limit="${KANI_SWAP_LIMIT_MB:-10240}"
    local line used
    while true; do
        if ! line="$(sysctl vm.swapusage 2> /dev/null)"; then
            echo "note: swap usage is unreadable here, starting without the check" >&2
            return 0
        fi
        used="$(echo "$line" | sed -E 's/.*used = ([0-9]+)(\.[0-9]+)?M.*/\1/')"
        if [ "$used" -le "$limit" ]; then
            return 0
        fi
        echo "note: ${used}M of swap in use, above ${limit}M, waiting" >&2
        sleep 30
    done
}

# Internal mode. Runs one harness, writes its log and a one-line verdict file
# holding SUCCESSFUL, VACUOUS, FAILED, UNKNOWN or BLOCKED followed by the
# seconds taken. BLOCKED means the machine monitor refused the start.
if [ "${1:-}" = "--one" ]; then
    name="$2"
    log_dir="$3"
    log="$log_dir/${name##*::}.log"
    if ! gate_machine; then
        echo "BLOCKED 0" > "$log_dir/${name##*::}.verdict"
        printf '%-60s %-10s\n' "$name" "BLOCKED"
        exit 0
    fi
    start="$(date +%s)"
    cargo kani --output-format terse --exact --harness "$name" > "$log" 2>&1 || true
    end="$(date +%s)"
    verdict="UNKNOWN"
    if grep -q '^VERIFICATION:- SUCCESSFUL' "$log"; then
        verdict="SUCCESSFUL"
        # A cover statement that cannot be satisfied means the harness never
        # reached the case it claims to exercise, so the success is vacuous.
        if grep -q 'UNSATISFIABLE' "$log"; then
            verdict="VACUOUS"
        fi
    elif grep -q '^VERIFICATION:- FAILED' "$log"; then
        verdict="FAILED"
    fi
    echo "$verdict $(( end - start ))" > "$log_dir/${name##*::}.verdict"
    printf '%-60s %-10s %5ss\n' "$name" "$verdict" "$(( end - start ))"
    exit 0
fi

PASSING=(
    verifier_harnesses::op_tables_are_total_and_agree
    verifier_harnesses::wcet_stream_single_op
    verifier_harnesses::wcet_stream_if_region
    verifier_harnesses::wcet_stream_loop_region
    verifier_harnesses::wcmu_stream_single_op
    verifier_harnesses::wcmu_stream_if_region
    verifier_harnesses::wcmu_stream_loop_region
    verifier_harnesses::wcet_whole_chunk_walk_does_not_panic
    verifier_harnesses::wcet_straight_line_is_exact
    verifier_harnesses::wcet_is_monotone_in_the_cost_model
    verifier_harnesses::module_call_depth_is_bounded
    verifier_harnesses::module_wcmu_does_not_panic
    verifier_harnesses::verify_does_not_panic
    wire_harnesses::maj3_is_bitwise_majority
    wire_harnesses::crc32_detects_bit_errors_in_four_bytes
    wire_harnesses::secded_corrects_single_bit_errors
    wire_harnesses::secded_detects_double_bit_errors
)

# Each entry is a harness name followed by the failure descriptions its log
# must contain, separated by a vertical bar. Requiring the descriptions, and
# not merely a failed verdict, keeps an expected failure from passing for an
# unrelated reason such as an unwinding bound that is too small.
FAILING=(
    "verifier_harnesses::finding_f1_wcet_overhead_overflows|attempt to add with overflow|in keleusma::verify::wcet_stream_iteration_with_cost_model"
    "verifier_harnesses::finding_f2_wcmu_slot_width_unvalidated|attempt to divide by zero|in keleusma::verify::wcmu_region_at|attempt to multiply with overflow|in keleusma::verify::wcmu_stream_iteration_with_value_slot_bytes"
    "verifier_harnesses::negative_control_false_wcet_bound|in verifier_harnesses::negative_control_false_wcet_bound::{closure#0}"
)

log_dir="${KANI_LOG_DIR:-$(mktemp -d)}"
mkdir -p "$log_dir"
jobs="${KANI_JOBS:-1}"

declared="$(grep -c '#\[kani::proof\]' src/*.rs | awk -F: '{ s += $2 } END { print s }')"
listed=$(( ${#PASSING[@]} + ${#FAILING[@]} ))
if [ "$declared" -ne "$listed" ]; then
    echo "error: src/ declares $declared harnesses but this script lists $listed" >&2
    exit 1
fi

# Build once, so that the parallel runs below find the build up to date.
if ! cargo kani --only-codegen > "$log_dir/build.log" 2>&1; then
    echo "error: build failed, see $log_dir/build.log" >&2
    exit 1
fi

names=("${PASSING[@]}")
for entry in "${FAILING[@]}"; do
    names+=("${entry%%|*}")
done

start="$(date +%s)"
printf '%s\n' "${names[@]}" | xargs -P "$jobs" -I '{}' "$here/check.sh" --one '{}' "$log_dir"
end="$(date +%s)"

passed=0
problems=0

verdict_of() {
    local file="$log_dir/${1##*::}.verdict"
    if [ -f "$file" ]; then
        cut -d' ' -f1 "$file"
    else
        echo "MISSING"
    fi
}

for name in "${PASSING[@]}"; do
    verdict="$(verdict_of "$name")"
    if [ "$verdict" = "SUCCESSFUL" ]; then
        passed=$(( passed + 1 ))
    else
        echo "error: $name gave $verdict where SUCCESSFUL was expected, see $log_dir/${name##*::}.log" >&2
        problems=$(( problems + 1 ))
    fi
done

for entry in "${FAILING[@]}"; do
    IFS='|' read -r -a parts <<< "$entry"
    name="${parts[0]}"
    log="$log_dir/${name##*::}.log"
    verdict="$(verdict_of "$name")"
    if [ "$verdict" != "FAILED" ]; then
        echo "error: $name gave $verdict where FAILED was expected, see $log" >&2
        problems=$(( problems + 1 ))
        continue
    fi
    for reason in "${parts[@]:1}"; do
        if ! grep -qF "$reason" "$log"; then
            echo "error: $name failed without the expected '$reason', see $log" >&2
            problems=$(( problems + 1 ))
        fi
    done
done

echo "verified $passed of ${#PASSING[@]} expected, checked ${#FAILING[@]} expected failures, $(( end - start ))s wall time, logs in $log_dir"

if [ "$passed" -ne "${#PASSING[@]}" ]; then
    echo "error: pass count $passed differs from the expected ${#PASSING[@]}" >&2
    exit 1
fi
if [ "$problems" -ne 0 ]; then
    exit 1
fi
