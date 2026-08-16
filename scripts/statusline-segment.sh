#!/usr/bin/env bash
# statusline-segment.sh — this project's contribution to the Claude Code status line.
#
# The harness contract, implemented by `~/.claude/statusline.sh`: if a project
# has an executable `scripts/statusline-segment.sh`, one line of its stdout is
# appended to the status line. The caller applies a hard timeout, discards
# stderr, ignores a non-zero exit, and truncates the result — so this script may
# fail or be slow without breaking anything, but it should do neither.
#
# It lives in `scripts/` rather than `.claude/` because `.claude/` is gitignored
# here, and an integration point that is not version-controlled silently differs
# between machines.
#
# WHAT THIS PROJECT PUTS THERE: gate progress. A full gate runs two to three and
# a half hours in a detached worktree, often with a second session's gate
# alongside, and "is it still going, and where" was previously answered by
# ad-hoc `pgrep` at the prompt — which produced three separate defects in one
# day, including a self-matching `pgrep` that deadlocked a sibling session.
# See `scripts/gate-status.sh` for the reasoning and the failure modes it avoids.
#
# TWO INSTRUMENTS, COMPOSED HERE RATHER THAN MERGED. `gate-status.sh` reports a
# local `release-gate.sh` run and `ci-status.sh` reports continuous integration.
# They are separate scripts with separate failure modes, and this file is the
# only place that knows about both, so a change to either cannot alter how the
# other is reported.
#
# THE ORDER IS DELIBERATE. Continuous integration gates feature branches under
# the workflow adopted on 2026-08-11 and the local gate is reserved for
# pre-publication and offline work, so the thing that is usually in flight goes
# first. Before this, the segment showed only the local gate, and on 2026-08-13
# that meant it displayed an abandoned run from sixty-six hours earlier while two
# pull requests sat in live continuous integration.
#
# NEITHER PART MAY BLOCK. `ci-status.sh` reads a cache and forks its refresh;
# `gate-status.sh` reads log files. Measured: 0.026 s warm and 0.33 s on the
# render that forks a refresh, against the caller's timeout of about 1.09 s.
set -uo pipefail
here="$(dirname "$0")"

ci=$("$here/ci-status.sh" 2>/dev/null | head -1)
gate=$("$here/gate-status.sh" --oneline 2>/dev/null | head -1)

# A part that has nothing to say contributes nothing, rather than an empty
# separator that reads as a missing value.
out=""
[ -n "$ci" ] && out="$ci"
[ -n "$gate" ] && { [ -n "$out" ] && out="$out  $gate" || out="$gate"; }
printf '%s\n' "$out"
