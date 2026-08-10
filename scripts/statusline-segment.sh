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
set -uo pipefail
exec "$(dirname "$0")/gate-status.sh" --oneline
