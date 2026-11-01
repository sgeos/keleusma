#!/usr/bin/env bash
# ci-status.sh — in-flight continuous-integration state, for the status line.
#
# WHY THIS EXISTS. The gate display beside it was built when a local
# `release-gate.sh` run was the project's bottleneck. The workflow changed on
# 2026-08-11: feature branches are gated by continuous integration and the local
# gate is reserved for pre-publication and offline work. The display did not
# follow, so on 2026-08-13 it read
#
#     ⏸ native@1c1ffb1e ▰▰▱▱▱▱ 34% quiet 66h21m
#
# which is an abandoned local gate from sixty-six hours earlier, shown while two
# pull requests sat in live continuous integration. That is worse than a missing
# feature: the instrument pointed confidently at the thing that had stopped
# mattering.
#
# THIS DOES NOT MODIFY `gate-status.sh`. That script is a separate instrument
# with its own reasoning and its own failure modes, and both sessions read it.
# The two are composed by `statusline-segment.sh` instead, so a change here
# cannot alter how a gate is reported.
#
# THE COST CONSTRAINT IS THE WHOLE DESIGN. The harness applies a hard timeout of
# roughly 1.09 s to the whole segment. Measured on this machine: the existing
# gate line costs 0.413 s and ONE `gh pr checks` call costs 0.884 s. A live query
# per render would blow the budget on the first pull request and rate-limit the
# API besides. So this reads a cache and never calls `gh` on the render path.
#
# THE REFRESH IS DETACHED AND SELF-THROTTLING. When the cache is older than
# REFRESH_SECS the reader forks a background refresh and returns the STALE value
# immediately. No scheduler, no daemon, and both sessions get it for free.
#
# SILENCE IS NOT SUCCESS, which is the failure this project has already paid for.
# A waiter that pinned a dead log path once watched it until timeout while a gate
# it was supposed to follow had restarted. So the age of the cache is DISPLAYED:
# a refresher that has died reads as stale rather than as green, and a reader can
# tell "no pull requests open" from "I have not looked in an hour".
set -uo pipefail

CACHE="${TMPDIR:-/tmp}/keleusma-ci-status.$(id -u).cache"
REFRESH_SECS=90
STALE_SECS=300

# --- the refresh half, run detached -----------------------------------------
if [ "${1:-}" = "--refresh" ]; then
    command -v gh >/dev/null 2>&1 || { printf 'gh-absent\n' >"$CACHE"; exit 0; }
    out=""
    # `gh pr list` may fail transiently (offline, rate limit, auth). A failed
    # refresh must leave the previous cache in place rather than blank it, or a
    # dropped network reads as "no pull requests".
    prs=$(gh pr list --state open --json number,baseRefName 2>/dev/null) || exit 0
    [ -z "$prs" ] && prs="[]"
    n=$(printf '%s' "$prs" | jq 'length' 2>/dev/null || echo 0)
    if [ "$n" = "0" ]; then
        printf 'none\n' >"$CACHE"
        exit 0
    fi
    for row in $(printf '%s' "$prs" | jq -r '.[] | "\(.number):\(.baseRefName)"' 2>/dev/null); do
        num=${row%%:*}
        base=${row#*:}
        j=$(gh pr checks "$num" --json state 2>/dev/null) || continue
        [ -z "$j" ] && continue
        tot=$(printf '%s' "$j" | jq 'length' 2>/dev/null || echo 0)
        [ "$tot" = "0" ] && continue
        pend=$(printf '%s' "$j" | jq '[.[]|select(.state=="QUEUED" or .state=="IN_PROGRESS" or .state=="PENDING")]|length' 2>/dev/null || echo 0)
        bad=$(printf '%s' "$j" | jq '[.[]|select(.state!="SUCCESS" and .state!="NEUTRAL" and .state!="SKIPPED" and .state!="QUEUED" and .state!="IN_PROGRESS" and .state!="PENDING")]|length' 2>/dev/null || echo 0)
        # The base branch is shown because both development lines share one
        # account, so an author cannot tell them apart. Base branch can.
        if [ "$bad" != "0" ]; then
            out="$out #$num/$base✗$bad"
        elif [ "$pend" != "0" ]; then
            done_n=$((tot - pend))
            out="$out #$num/$base⋯$done_n/$tot"
        else
            out="$out #$num/$base✓$tot"
        fi
    done
    [ -z "$out" ] && out=" none"
    printf 'CI%s\n' "$out" >"$CACHE"
    exit 0
fi

# --- the read half, which must be fast --------------------------------------
now=$(date +%s)
age=99999
if [ -f "$CACHE" ]; then
    mtime=$(stat -f %m "$CACHE" 2>/dev/null || stat -c %Y "$CACHE" 2>/dev/null || echo 0)
    age=$((now - mtime))
fi

# Fork a detached refresh when stale. Redirected and disowned so nothing of it
# reaches the status line or blocks the render.
if [ "$age" -ge "$REFRESH_SECS" ]; then
    ( "$0" --refresh >/dev/null 2>&1 & ) >/dev/null 2>&1
fi

[ -f "$CACHE" ] || { printf 'CI ?\n'; exit 0; }
line=$(head -1 "$CACHE" 2>/dev/null)
case "$line" in
    none) line="CI none" ;;
    gh-absent) printf ''; exit 0 ;;
    "") line="CI ?" ;;
esac

# Age is shown once the cache is old enough that it might be lying. Below the
# threshold the number is noise; above it, it is the whole point.
if [ "$age" -ge "$STALE_SECS" ]; then
    if [ "$age" -ge 3600 ]; then
        printf '%s stale %dh\n' "$line" "$((age / 3600))"
    else
        printf '%s stale %dm\n' "$line" "$((age / 60))"
    fi
else
    printf '%s\n' "$line"
fi
