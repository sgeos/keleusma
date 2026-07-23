# Per-Branch Handoffs

> **Navigation**: [Process](../README.md) | [Documentation Root](../../README.md)

Per-branch AI-to-human handoffs for parallel-agent development. When more than one
agent works at once, each writes its status here at `docs/process/handoffs/<branch-leaf>.md`
instead of overwriting the shared [`REVERSE_PROMPT.md`](../REVERSE_PROMPT.md), which
is single-writer. See [`PARALLEL_DEVELOPMENT.md`](../PARALLEL_DEVELOPMENT.md) section 3.

A handoff file follows the same structure as `REVERSE_PROMPT.md` (last updated,
verification, summary, questions, concerns, next step). The primary agent
reconciles these back into `REVERSE_PROMPT.md` when the parallel burst finishes,
after which the per-branch file may be removed.

Solo sessions do not use this directory; they overwrite `REVERSE_PROMPT.md` as
before.
