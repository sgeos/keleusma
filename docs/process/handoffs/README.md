# Per-Branch Handoffs

> **Navigation**: [Process](../README.md) | [Documentation Root](../../README.md)

Per-branch mailboxes for parallel-agent development. When more than one agent works at
once, each writes its status here instead of overwriting the shared
[`REVERSE_PROMPT.md`](../REVERSE_PROMPT.md), which is single-writer, and instead of
relaying prose through the operator. See
[`PARALLEL_DEVELOPMENT.md`](../PARALLEL_DEVELOPMENT.md) section 0a.

**A mailbox is named for, and lives on, the VERSION branch** —
`docs/process/handoffs/<version-branch>.md`, updated by direct commit there. Not on a
feature branch: a session doing real work is on one, so its mailbox would sit a branch
away from where readers look, and `git show <version-branch>:<path>` would hand back the
version branch's older copy with **no error at all**. That happened on 2026-08-09, the
convention's first day.

**Open every mailbox with the branch it describes and the tip it was written against**, so a
reader who reaches the wrong file can tell.

A handoff file follows the same structure as `REVERSE_PROMPT.md` (last updated,
verification, summary, questions, concerns, next step). The primary agent
reconciles these back into `REVERSE_PROMPT.md` when the parallel burst finishes,
after which the per-branch file may be removed.

Solo sessions do not use this directory; they overwrite `REVERSE_PROMPT.md` as
before.
