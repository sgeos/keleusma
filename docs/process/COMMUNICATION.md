# Bidirectional Communication Protocol

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

## Overview

This document defines the protocol for structured communication between the human pilot and AI agent across sessions. Three working documents provide the communication channels.

| Document | Direction | Persistence |
|----------|-----------|-------------|
| [PROMPT.md](./PROMPT.md) | Human to AI | Committed with each prompt cycle |
| [REVERSE_PROMPT.md](./REVERSE_PROMPT.md) | AI to Human | Overwritten after each task |
| [TASKLOG.md](./TASKLOG.md) | Shared | Updated incrementally |

## Forward Prompt

PROMPT.md is the human-to-AI instruction staging area. This file is **read-only for the AI agent**. The AI agent must never modify PROMPT.md, but should include it in commits if the human pilot has modified it.

### Structure

- **Comments**: Context or clarifications from the human pilot
- **Objectives**: What the AI agent should accomplish
- **Context**: Relevant background information
- **Constraints**: Boundaries on the approach or implementation
- **Success Criteria**: How to determine the task is complete
- **Notes**: Additional information that does not fit the above categories

## Reverse Prompt

The AI agent overwrites REVERSE_PROMPT.md after completing each task. This file serves as the primary channel for the AI agent to communicate status, concerns, and intent back to the human pilot.

### Structure

- **Last Updated**: Date, task identifier, and status
- **Verification**: Commands run and their results
- **Summary**: What was accomplished
- **Questions for Human Pilot**: Decisions requiring human input
- **Technical Concerns**: Unaddressed risks or issues discovered during work
- **Intended Next Step**: What the AI agent plans to do next
- **Session Context**: State needed to resume work in a new session

### Rules

- If blocked, document the blocker and stop. Do not proceed with assumptions.
- Every task must include verification. A task without verification is not complete.

## Task Log

TASKLOG.md is the shared source of truth for the current sprint. Both the human pilot and AI agent read and write this document.

### Structure

- **Task Name and Status**: Identifier, description, and current status
- **Success Criteria**: How to verify the task is complete
- **Task Breakdown**: Subtasks with individual status tracking
- **Notes**: Observations, decisions, or context discovered during work
- **History**: Chronological record of progress

### Rules

- Update status as work progresses. Do not batch status updates.
- Every task marked Complete must include verification evidence.
- Blocked tasks must document the specific blocker.

### History Maintenance

Consolidate same-day entries to avoid unbounded growth. Retain per-prompt granularity only for the active task. Completed tasks should have a single summary entry in the history table.

## Session Startup Protocol

1. Read [TASKLOG.md](./TASKLOG.md) for current task state.
2. Read [REVERSE_PROMPT.md](./REVERSE_PROMPT.md) for last AI communication.
3. Wait for human prompt before proceeding.

## Work Item Coding System

Work items follow the format **Vw-Mx-Tz**, simplified from the parent project (no separate prompt tracking).

| Component | Meaning | Example |
|-----------|---------|---------|
| Vw | Version (Phase) | V0.0 = Phase 0 |
| Mx | Milestone within version | M1 = first milestone |
| Tz | Task within milestone | T2 = second task |

For example, V0.1-M2-T3 refers to Phase 0.1, Milestone 2, Task 3.

## Task Completion Protocol

1. Complete the implementation and verify it works.
2. Update [TASKLOG.md](./TASKLOG.md) with the task status and verification.
3. Update [REVERSE_PROMPT.md](./REVERSE_PROMPT.md) with summary, concerns, and intended next step.
4. Commit all changes with a conventional commit referencing the task.
5. Proceed to the next task, or stop if blocked.

## Blocking Protocol

1. Document the blocker in [REVERSE_PROMPT.md](./REVERSE_PROMPT.md) with sufficient detail for the human pilot to resolve it.
2. Update [TASKLOG.md](./TASKLOG.md) to mark the task as Blocked.
3. Commit the updated process files.
4. Stop. Do not attempt to work around the blocker without human guidance.
