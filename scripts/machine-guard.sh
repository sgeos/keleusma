#!/bin/sh
# machine-guard.sh — apply the shared machine's playbook to Keleusma's heavy jobs.
#
# Usage, as a command:
#   scripts/machine-guard.sh level              # ok, elevated, critical, stale or unmonitored
#   scripts/machine-guard.sh wait LABEL         # return once heavy work may start
#   scripts/machine-guard.sh run LABEL CMD...   # wait, run CMD, record its peak footprint
# Usage, from another POSIX script:
#   . "$(dirname "$0")/machine-guard.sh"
#   machine_guard_wait "release gate, clippy"
#
# WHY. On 2026-11-08 one heavy job nearly exhausted the shared machine's memory. The
# operator asked for an abstract playbook in place of rules about particular
# activities, because the next shortage will come from somewhere else. The playbook,
# named on the `playbook=` line of every `machine-status` sample, says: observe with
# `machine-status`, act only on your own work, prefer reversible actions such as
# waiting, report what you saw and did, and record peak footprints. The admin line owns
# the monitor, and this line decides how the playbook applies to Keleusma's tooling.
# This helper is that decision, shared by every heavy Keleusma script rather than
# written into each one.
#
# WHAT IT DOES at each level, before heavy work starts.
#   ok           proceed.
#   elevated     wait, reporting every few minutes, until the level is ok.
#   critical     wait in the same way, and say so at once, since critical calls for a
#                report to the operator.
#   stale        treated as elevated, as the playbook says, and reported as stale.
#   unmonitored  no `machine-status` on this machine, as on a CI runner, so proceed.
# It never signals a process, its own or anyone else's. Pausing running work is a
# process-control step that some sessions' permissions refuse and that the operator
# has not ruled on, so a waiting point between steps is this helper's only lever.
#
# Environment:
#   KEL_MACHINE_GUARD=off     skip every wait. The operator's explicit choice.
#   KEL_MACHINE_GUARD_POLL    seconds between checks while waiting, default 60, the
#                             monitor's own sampling interval.
#   KEL_MACHINE_GUARD_REPORT  seconds between progress reports while waiting, default 300.
#   KEL_MACHINE_GUARD_MAX     give up after this many seconds and fail, so that a
#                             caller can report rather than wait for ever. Unset waits
#                             without limit.
#   KEL_MACHINE_GUARD_LOG     where `run` appends its peak footprints, default
#                             target/machine-guard/peaks.tsv under the current directory.
#
# Peaks are approximate. `run` reads them from `/usr/bin/time -l` on macOS, or `-v` on
# Linux, which gives the resident figure only. On macOS the utility reports
# the measured command's own peak footprint and the largest resident size among the
# processes it waited for. For a driver such as cargo that spawns compilers, the second
# figure is the closer one, and neither is the sum of what ran at once. Where a sandbox
# refuses the time utility, the peak is recorded as unknown, never as zero.
#
# The script is POSIX sh. Sourcing it defines functions and changes no shell option.

machine_guard_level() {
    if ! command -v machine-status > /dev/null 2>&1; then
        echo unmonitored
        return 0
    fi
    machine_guard_raw=$(machine-status --level 2> /dev/null) || true
    case $machine_guard_raw in
        ok|elevated|critical|stale) echo "$machine_guard_raw" ;;
        *) echo stale ;;
    esac
}

machine_guard_wait() {
    machine_guard_label=${1:-heavy work}
    [ "${KEL_MACHINE_GUARD:-on}" = off ] && return 0
    machine_guard_poll=${KEL_MACHINE_GUARD_POLL:-60}
    machine_guard_every=${KEL_MACHINE_GUARD_REPORT:-300}
    machine_guard_waited=0
    machine_guard_held=0
    machine_guard_since_report=$machine_guard_every
    while :; do
        machine_guard_now=$(machine_guard_level)
        case $machine_guard_now in
            ok|unmonitored)
                if [ "$machine_guard_held" -eq 1 ]; then
                    printf 'machine-guard: level %s after %ss, starting %s\n' \
                        "$machine_guard_now" "$machine_guard_waited" "$machine_guard_label" >&2
                fi
                return 0 ;;
        esac
        if [ -n "${KEL_MACHINE_GUARD_MAX:-}" ] && [ "$machine_guard_waited" -ge "$KEL_MACHINE_GUARD_MAX" ]; then
            printf 'machine-guard: level still %s after %ss, not starting %s\n' \
                "$machine_guard_now" "$machine_guard_waited" "$machine_guard_label" >&2
            return 75
        fi
        if [ "$machine_guard_since_report" -ge "$machine_guard_every" ] || [ "$machine_guard_now" = critical ]; then
            printf 'machine-guard: level %s, waiting before %s. Run machine-status for the figures.\n' \
                "$machine_guard_now" "$machine_guard_label" >&2
            machine_guard_since_report=0
        fi
        machine_guard_held=1
        sleep "$machine_guard_poll"
        machine_guard_waited=$(( machine_guard_waited + machine_guard_poll ))
        machine_guard_since_report=$(( machine_guard_since_report + machine_guard_poll ))
    done
}

# Wait, then run a command, recording its peak and its exit status. Returns the
# command's own status.
machine_guard_run() {
    machine_guard_label=$1
    shift
    machine_guard_wait "$machine_guard_label" || return $?
    machine_guard_log=${KEL_MACHINE_GUARD_LOG:-target/machine-guard/peaks.tsv}
    mkdir -p "$(dirname "$machine_guard_log")"
    machine_guard_time=$(mktemp "${TMPDIR:-/tmp}/machine-guard.XXXXXX")
    machine_guard_start=$(date +%s)
    if /usr/bin/time -l true > /dev/null 2>&1; then
        machine_guard_status=0
        /usr/bin/time -l -o "$machine_guard_time" "$@" || machine_guard_status=$?
        machine_guard_footprint=$(awk '/peak memory footprint/ { print $1 }' "$machine_guard_time")
        machine_guard_resident=$(awk '/maximum resident set size/ { print $1 }' "$machine_guard_time")
    elif /usr/bin/time -v true > /dev/null 2>&1; then
        # GNU time reports kilobytes and no footprint.
        machine_guard_status=0
        /usr/bin/time -v -o "$machine_guard_time" "$@" || machine_guard_status=$?
        machine_guard_footprint=''
        machine_guard_resident=$(awk -F: '/Maximum resident set size/ { gsub(/ /, "", $2); if ($2 != "") print $2 * 1024 }' "$machine_guard_time")
    else
        machine_guard_status=0
        "$@" || machine_guard_status=$?
        machine_guard_footprint=''
        machine_guard_resident=''
    fi
    machine_guard_end=$(date +%s)
    rm -f "$machine_guard_time"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$machine_guard_label" \
        "${machine_guard_footprint:-unknown}" "${machine_guard_resident:-unknown}" \
        "$(( machine_guard_end - machine_guard_start ))" "$machine_guard_status" >> "$machine_guard_log"
    return "$machine_guard_status"
}

# Run as a command rather than sourced. A sourcing script's own $0 never names this file.
case ${0##*/} in
    machine-guard.sh)
        set -eu
        case ${1:-} in
            level) machine_guard_level ;;
            wait) shift; machine_guard_wait "${1:-heavy work}" ;;
            run)
                shift
                [ "$#" -ge 2 ] || { echo 'usage: scripts/machine-guard.sh run LABEL CMD...' >&2; exit 64; }
                machine_guard_run "$@" ;;
            *) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 64 ;;
        esac ;;
esac
