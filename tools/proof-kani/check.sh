#!/usr/bin/env bash
# Runs every Kani harness in this package and fails unless each one gives its
# expected result.
#
# Expected to verify. Every selected harness of LIGHT or HEAVY must report
# VERIFICATION SUCCESSFUL with no unsatisfiable cover statement, and the
# number that do must equal the number selected.
#
# Expected to fail. Every harness in FAILING must report VERIFICATION FAILED
# with each of the failure descriptions listed for it. This is the negative
# control described in README.md, excluded from the pass count. The finding
# harnesses for F1 and F2 were listed here until the V0.2.X line fixed both,
# and now verify as regressions in LIGHT.
#
# LIGHT, HEAVY and FAILING together must name every `#[kani::proof]` in src/, so that a
# harness added without being listed fails this script instead of being
# silently skipped.
#
# Usage: tools/proof-kani/check.sh [--tier light|heavy|all] [--jobs N] [--only NAME]
#   --tier light  the default. The harnesses measured to finish quickly with a
#                 small footprint, and every expected failure.
#   --tier heavy  the instruction-walk and module harnesses, whose footprint is
#                 large or not yet measured. One of them held about thirty
#                 gigabytes for more than thirty minutes without finishing.
#   --tier all    both tiers.
#   --jobs N      how many harnesses run at once, overriding KANI_JOBS.
#   --only NAME   runs the one named harness, from whichever list holds it, so
#                 a heavy harness can be measured alone.
# Environment: KANI_JOBS sets how many harnesses run at once, default 1.
# One harness can hold about eight gigabytes of memory while the model checker
# runs, and the development machine is shared, so serial is the safe default.
# Each harness starts only when `gate_machine` below allows it. Where the
# `machine-status` monitor is absent, KANI_SWAP_LIMIT_MB, default 10240, makes
# each harness wait to start while more swap than that is in use.
# KANI_LOG_DIR sets where the per-harness logs go, default a fresh temporary
# directory.
#
# Self-pausing. While a harness runs, `run_watched` below polls the machine
# monitor every KANI_POLL seconds, default 15. At critical it stops the
# harness's own process group with SIGSTOP, and at ok it resumes the group with
# SIGCONT. At elevated or stale it changes nothing, so a paused harness stays
# paused until the machine is ok again. The stopped model checker keeps its
# memory, which the system may swap out, but it stops growing and stops
# competing for the processor. Only process groups this script started are
# signalled. An interrupt or termination resumes a paused group and then
# terminates it, so no stopped process outlives the run. The seconds a harness
# spent paused are recorded beside its verdict. The operator ruled on
# 2026-11-11 that heavy runs are allowed when guarded, and that self-pausing at
# critical is the guard wanted.
# KANI_WALK_SEL_LO, KANI_WALK_SEL_HI, KANI_CLOSE_SEL_LO and KANI_CLOSE_SEL_HI
# narrow the instruction selectors of the generators at compile time, as
# src/generators.rs describes, so that one piece of a heavy harness can be
# measured alone. A run with any of them set exits 3 even when every selected
# harness verified, because it verified only a piece.
# KANI_STATUS_CMD replaces `machine-status --level` as the monitor command, so
# the pausing can be exercised without a real critical level. It must exit 0
# for ok, 1 for elevated, 2 for critical and 3 for stale.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

# The global compiler cache wrapper is refused inside the sandbox this package
# is developed in, and Kani does not need it.
export RUSTC_WRAPPER=

status_cmd="${KANI_STATUS_CMD:-machine-status --level}"

# Succeeds when the monitor command can be found.
have_monitor() {
    local first
    read -r first _ <<< "$status_cmd"
    command -v "$first" > /dev/null 2>&1
}

# Runs the monitor once and returns its level as the exit status. The command
# is split on spaces on purpose, so that it may carry arguments.
monitor_level() {
    # shellcheck disable=SC2086
    $status_cmd > /dev/null 2>&1
}

# Runs a command in a process group of its own, with standard output to $1 and
# standard error to $2, pausing and resuming the group as the header describes.
# Sets `paused_total` to the seconds spent paused. Always returns 0, because the
# verdict is read from the log and not from the exit status.
paused_total=0
watched_pgid=""
watched_paused=0
run_watched() {
    local out="$1" err="$2"
    shift 2
    local poll="${KANI_POLL:-15}"
    case "$poll" in
        ''|*[!0-9]*|0) echo "error: KANI_POLL must be a positive whole number" >&2; exit 2 ;;
    esac
    local monitored=0
    if have_monitor; then monitored=1; fi
    # With job control on, a background job is the leader of a new process
    # group whose id is its process id, so `kill -- -id` reaches the model
    # checker and every other descendant along with the job itself.
    set -m
    "$@" > "$out" 2> "$err" &
    watched_pgid=$!
    set +m
    watched_paused=0
    paused_total=0
    trap 'release_watched; exit 130' INT TERM
    trap 'release_watched' EXIT
    local level paused_at=0 waited stale_noted=0
    while kill -0 "$watched_pgid" 2> /dev/null; do
        if [ "$monitored" -eq 1 ]; then
            if monitor_level; then level=0; else level=$?; fi
            if [ "$level" -eq 2 ] && [ "$watched_paused" -eq 0 ]; then
                if kill -STOP -- "-$watched_pgid" 2> /dev/null; then
                    watched_paused=1
                    paused_at="$(date +%s)"
                    echo "note: machine-status reports critical, paused process group $watched_pgid" >&2
                else
                    echo "warning: could not pause process group $watched_pgid" >&2
                fi
            elif [ "$level" -eq 0 ] && [ "$watched_paused" -eq 1 ]; then
                kill -CONT -- "-$watched_pgid" 2> /dev/null || true
                watched_paused=0
                paused_total=$(( paused_total + $(date +%s) - paused_at ))
                echo "note: machine-status reports ok, resumed process group $watched_pgid" >&2
            elif [ "$level" -eq 3 ] && [ "$stale_noted" -eq 0 ]; then
                echo "note: machine-status is stale, leaving process group $watched_pgid as it is" >&2
                stale_noted=1
            fi
        fi
        # Sleep in one-second steps, so that the end of the harness is seen
        # promptly while the monitor is asked only every `poll` seconds.
        waited=0
        while [ "$waited" -lt "$poll" ] && kill -0 "$watched_pgid" 2> /dev/null; do
            sleep 1
            waited=$(( waited + 1 ))
        done
    done
    wait "$watched_pgid" 2> /dev/null || true
    if [ "$watched_paused" -eq 1 ]; then
        paused_total=$(( paused_total + $(date +%s) - paused_at ))
        watched_paused=0
    fi
    watched_pgid=""
    trap - INT TERM EXIT
    return 0
}

# Resumes and then terminates a watched group that is still running, so that
# an interrupted run leaves no stopped process behind. The group is resumed
# first because a stopped process does not act on SIGTERM until it continues.
release_watched() {
    if [ -n "$watched_pgid" ] && kill -0 "$watched_pgid" 2> /dev/null; then
        kill -CONT -- "-$watched_pgid" 2> /dev/null || true
        kill -TERM -- "-$watched_pgid" 2> /dev/null || true
        echo "note: interrupted, resumed and terminated process group $watched_pgid" >&2
    fi
    watched_pgid=""
}

# Waits until the machine can take another harness, and returns nonzero when
# it must not start one at all.
#
# Where the `machine-status` monitor is installed, it decides. Its exit status
# is 0 for ok, 1 for elevated, 2 for critical and 3 for stale. A harness starts
# only at ok. At elevated or critical this waits and polls again every 60
# seconds, which is the self-pausing of a harness not yet started. At stale it
# refuses, because a monitor that has stopped reporting may never report ok,
# and the run then reports the condition instead of waiting without end. A
# light-tier run of 2026-11-11 refused five harnesses at a critical level after
# waiting eighteen hours at elevated, which is why critical now waits. Where the monitor is absent, the swap figure from
# `sysctl vm.swapusage` decides, and where neither can be read the harness
# starts ungated.
gate_machine() {
    local level waited=0 last_level=""
    if have_monitor; then
        while true; do
            if monitor_level; then
                if [ "$waited" -gt 0 ]; then
                    echo "note: machine-status reports ok after ${waited} minutes waiting, starting" >&2
                fi
                return 0
            else
                level=$?
            fi
            if [ "$level" -eq 1 ] || [ "$level" -eq 2 ]; then
                # Said when the level changes, and otherwise every hour, so a
                # long wait does not flood the log.
                if [ $(( waited % 60 )) -eq 0 ] || [ "$level" != "${last_level:-}" ]; then
                    echo "note: machine-status exit $level, 1 elevated or 2 critical, waiting, ${waited} minutes so far" >&2
                fi
                last_level="$level"
                waited=$(( waited + 1 ))
                sleep 60
            else
                echo "note: machine-status exit $level, stale or unknown, not starting" >&2
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
# seconds taken and the peak memory footprint in bytes, or `unknown`. BLOCKED
# means the machine monitor refused the start. Each peak is also appended to
# peaks.txt in the log directory.
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
    # Peak memory, where `/usr/bin/time -l` works. Inside some sandboxes it
    # fails on a refused sysctl, and the peak is then recorded as unknown,
    # never as zero.
    peak="unknown"
    if /usr/bin/time -l true > /dev/null 2>&1; then
        run_watched "$log" "$log.time" \
            /usr/bin/time -l cargo kani --output-format terse --exact --harness "$name"
        # The maximum resident set size comes first because it covers the
        # model checker, a grandchild of this process. The peak memory
        # footprint covers only the direct child, `cargo`, which measured
        # under two megabytes beside a model checker of hundreds.
        peak="$(awk '/maximum resident set size/ { print $1 }' "$log.time")"
        if [ -z "$peak" ]; then
            peak="$(awk '/peak memory footprint/ { print $1 }' "$log.time")"
        fi
        if [ -z "$peak" ]; then
            peak="unknown"
        fi
        # Kani's own diagnostics went to the timing file, so keep them in the log.
        cat "$log.time" >> "$log"
    else
        run_watched "$log" "$log.err" \
            cargo kani --output-format terse --exact --harness "$name"
        cat "$log.err" >> "$log"
        rm -f "$log.err"
    fi
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
    echo "$verdict $(( end - start )) $peak $paused_total" > "$log_dir/${name##*::}.verdict"
    echo "$name $peak" >> "$log_dir/peaks.txt"
    printf '%-60s %-10s %5ss  peak %s bytes  paused %ss\n' \
        "$name" "$verdict" "$(( end - start ))" "$peak" "$paused_total"
    exit 0
fi

# Expected to verify, measured light. Each finished in under three minutes,
# the largest measured peak being 336 megabytes. The two regressions for F1
# and F2 run the chunks their finding harnesses ran, which took 13 and 11
# seconds as expected failures.
LIGHT=(
    verifier_harnesses::op_tables_are_total_and_agree
    wire_harnesses::maj3_is_bitwise_majority
    wire_harnesses::crc32_detects_bit_errors_in_four_bytes
    wire_harnesses::secded_corrects_single_bit_errors
    wire_harnesses::secded_detects_double_bit_errors
    verifier_harnesses::f1_wcet_overhead_saturates
    verifier_harnesses::f2_wcmu_slot_width_validated
)

# Expected to verify, heavy or not yet measured. None has a recorded green run
# in its committed form, so a run of this tier establishes their standing.
HEAVY=(
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
)

# Each entry is a harness name followed by the failure descriptions its log
# must contain, separated by a vertical bar. Requiring the descriptions, and
# not merely a failed verdict, keeps an expected failure from passing for an
# unrelated reason such as an unwinding bound that is too small.
FAILING=(
    "verifier_harnesses::negative_control_false_wcet_bound|in verifier_harnesses::negative_control_false_wcet_bound::{closure#0}"
)

tier="light"
jobs="${KANI_JOBS:-1}"
only=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --tier) tier="${2:?--tier needs light, heavy or all}"; shift 2 ;;
        --jobs) jobs="${2:?--jobs needs a number}"; shift 2 ;;
        --only) only="${2:?--only needs a harness name}"; shift 2 ;;
        -h|--help) awk 'NR > 1 && /^#/ { print; next } NR > 1 { exit }' "$0"; exit 0 ;;
        *) echo "error: unknown option $1, see --help" >&2; exit 2 ;;
    esac
done
case "$jobs" in
    ''|*[!0-9]*|0) echo "error: --jobs needs a positive whole number" >&2; exit 2 ;;
esac

declared="$(grep -c '#\[kani::proof\]' src/*.rs | awk -F: '{ s += $2 } END { print s }')"
listed=$(( ${#LIGHT[@]} + ${#HEAVY[@]} + ${#FAILING[@]} ))
if [ "$declared" -ne "$listed" ]; then
    echo "error: src/ declares $declared harnesses but this script lists $listed" >&2
    exit 1
fi

# The selection. PASSING holds the expected successes to run, and FAILING
# the expected failures to run.
all_failing=("${FAILING[@]}")
case "$tier" in
    light) PASSING=("${LIGHT[@]}") ;;
    heavy) PASSING=("${HEAVY[@]}"); FAILING=() ;;
    all) PASSING=("${LIGHT[@]}" "${HEAVY[@]}") ;;
    *) echo "error: --tier must be light, heavy or all" >&2; exit 2 ;;
esac
if [ -n "$only" ]; then
    PASSING=()
    FAILING=()
    for name in "${LIGHT[@]}" "${HEAVY[@]}"; do
        if [ "$name" = "$only" ]; then PASSING=("$name"); fi
    done
    for entry in "${all_failing[@]}"; do
        if [ "${entry%%|*}" = "$only" ]; then FAILING=("$entry"); fi
    done
    if [ $(( ${#PASSING[@]} + ${#FAILING[@]} )) -ne 1 ]; then
        echo "error: no harness named $only" >&2
        exit 2
    fi
    tier="only"
fi
if [ "$tier" = "heavy" ] || [ "$tier" = "all" ]; then
    echo "note: the heavy tier can hold tens of gigabytes per harness, running $jobs at a time" >&2
fi

# The selector narrowing that src/generators.rs reads at compile time. A run
# with any of it set is a measurement, never a pass.
narrowed=""
for var in KANI_WALK_SEL_LO KANI_WALK_SEL_HI KANI_CLOSE_SEL_LO KANI_CLOSE_SEL_HI; do
    if [ -n "${!var+set}" ]; then
        narrowed="$narrowed $var=${!var}"
    fi
done
if [ -n "$narrowed" ]; then
    echo "note: selectors narrowed by$narrowed, a measurement run that cannot pass" >&2
fi

log_dir="${KANI_LOG_DIR:-$(mktemp -d)}"
mkdir -p "$log_dir"
if [ -n "$narrowed" ]; then
    echo "narrowed:$narrowed" > "$log_dir/narrowed.txt"
fi

# Build once, so that the parallel runs below find the build up to date. The
# build is gated like a harness, since a fresh one compiles the whole crate.
if ! gate_machine; then
    echo "error: the machine monitor refused the build, see machine-status" >&2
    exit 1
fi
if ! cargo kani --only-codegen > "$log_dir/build.log" 2>&1; then
    echo "error: build failed, see $log_dir/build.log" >&2
    exit 1
fi

names=()
if [ "${#PASSING[@]}" -gt 0 ]; then names=("${PASSING[@]}"); fi
for entry in "${FAILING[@]+"${FAILING[@]}"}"; do
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

for name in "${PASSING[@]+"${PASSING[@]}"}"; do
    verdict="$(verdict_of "$name")"
    if [ "$verdict" = "SUCCESSFUL" ]; then
        passed=$(( passed + 1 ))
    else
        echo "error: $name gave $verdict where SUCCESSFUL was expected, see $log_dir/${name##*::}.log" >&2
        problems=$(( problems + 1 ))
    fi
done

for entry in "${FAILING[@]+"${FAILING[@]}"}"; do
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

echo "tier $tier, $jobs at a time: verified $passed of ${#PASSING[@]} expected, checked ${#FAILING[@]} expected failures, $(( end - start ))s wall time, logs in $log_dir"

if [ "$passed" -ne "${#PASSING[@]}" ]; then
    echo "error: pass count $passed differs from the expected ${#PASSING[@]}" >&2
    exit 1
fi
if [ "$problems" -ne 0 ]; then
    exit 1
fi
if [ -n "$narrowed" ]; then
    echo "error: narrowed by$narrowed, so this run measured pieces of its harnesses and verified none whole" >&2
    exit 3
fi
