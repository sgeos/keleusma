# Harness and Mailbox Design Notes

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

Design notes from the kaizen line's discussion with the operator between 2026-09-08 and 2026-10-04,
recorded so that a session on any harness can continue them. Nothing here is built and nothing is
ruled. The notes are harness-agnostic where the design is, and they say so where a fact is specific
to one harness. Every claim about a product rests on its documentation as read on 2026-09-08 and is
labelled read evidence. Figures were not re-measured on 2026-10-04.

## The operator's requirement

An agent that stays in a development loop and, when context grows high enough, initiates the
sequence draft a handoff, compact or clear its own context, and resume from the handoff. Lines must
be able to log questions and issues the operator answers asynchronously, and lines must be able to
communicate with one another. The operator's view is that the existing mailbox convention can carry
all of this if the agents are given a robust mailbox interface, unread messages, messages from the
operator, messages to the operator, and the last several messages.

## What Claude Code exposes, read from its documentation on 2026-09-08

| Capability | State |
|---|---|
| The model or a hook triggers compaction | not available. Four public requests between March and August 2026 are closed, three as not planned and one consolidated into them |
| Auto-compaction threshold | settable, the `autoCompactWindow` setting, the `CLAUDE_CODE_AUTO_COMPACT_WINDOW` variable, the `/autocompact` command, and the `--autocompact` flag, from 100K to 1M tokens |
| Context usage in hook input | not present in any documented hook field |
| Context usage in the status line script | present, `context_window.used_percentage`, `remaining_percentage`, token totals, and rate-limit percentages, refreshed on events or on a timer, and the script can write a file |
| Hooks around compaction | `PreCompact` with `manual` and `auto` matchers receiving the transcript path, `PostCompact`, and `SessionStart` with a `compact` matcher whose standard output becomes context |
| Usage-limit interruption | the `autoContinueAtUsageLimit` setting waits for the reset and continues |
| Cross-session messaging | `ListAgents` and `SendMessage`, plain text, delivered as a new turn when the target is idle, held or dropped when it is not running, never persisted or queryable, with sender identity, and a receiving session is told the message is not from the operator and cannot treat it as consent |
| Agent SDK | a driver can send `/compact` into a streaming session and read a `compact_boundary` message with the pre-compaction token count, can start fresh sessions, and can expose custom tools whose handlers run in the driver process |
| Plugins | may contain skills, agents, hooks, MCP servers, LSP servers, background monitors, executables, and a settings file naming a main-thread agent. None drives the conversation loop or triggers compaction |

## What other harnesses offer, read on 2026-09-08

| Harness | Primitive | Invoked by | Approval | Agent sees context usage |
|---|---|---|---|---|
| Cline | `new_task`, ends the task and starts a new one preloaded with structured context | the model proposes | yes, with a preview | yes, on every request, with a threshold settable in its rules file |
| Roo Code | `new_task`, an isolated subtask with the parent paused and resumed on its result | the model | yes, per task | not documented |
| Amp | `/handoff`, compaction removed in favour of a generated seed prompt for a new thread | the user | the user edits the draft | not documented |
| Gemini CLI, OpenCode, Codex CLI, Goose | automatic compaction at a threshold | the harness | none | not documented, per comparison articles rather than each tool's documentation |

No harness that was checked gives unattended self-compaction or self-reset. Amp's stated reason for
removing compaction, that it is lossy and encourages long threads with summaries stacked on
summaries, agrees with the audit's finding that an authored handoff is the trusted artifact.

## The recommended harness shape

A thin driver on an agent SDK, with the policy and the mailbox outside it.

1. The driver runs the line as a streaming session and exposes one custom tool, `handoff`, with
   modes compact and reset.
2. The driver watches context usage and injects a one-line warning above a threshold.
3. The agent writes and commits the handoff, posts it to the mailbox, and calls the tool.
4. In compact mode the driver sends the compaction command with an instruction to keep only a
   pointer to the handoff, confirms the boundary message, and sends the resume prompt. In reset
   mode it ends the session and starts a fresh one with the resume prompt.
5. The resume prompt is the handoff, validated by its own ancestry and content checks, plus a
   bounded inbox view.

Reset is the preferred mode, because a compaction summary is written by the harness and the handoff
is authored and validated. Compaction earns its place only where in-session state outside files
matters, such as background processes or monitors, which are not restored across a session boundary.

The minimum version is a shell loop that starts a headless session with the handoff as the prompt
and a session that ends its turn after writing the handoff, about fifty lines, reset only. A session
on a different harness replaces the driver layer with that harness's equivalent and keeps the rest.

Estimated effort for the driver, four to six working days to a usable first version of roughly five
hundred lines, plus a week of running the kaizen line on it before trusting it for a trunk line. The
first test should be a negative control, a deliberately stale handoff that the resume refuses.

## The mailbox

The durable mailbox is built beside the live messaging, which serves only as a doorbell to wake an
idle line.

**Store.** One append-only record per message, immutable once written, in a dedicated git branch or
a small separate repository, so every line and machine sees one history and the audit trail is the
commit log.

| Field | Purpose |
|---|---|
| id, timestamp, in-reply-to | threading and ordering |
| from, to | a line name, `operator`, or `all` |
| kind | question, answer, ruling, issue, notice, handoff |
| provenance | `operator-direct`, `line`, or `relayed`, set by the store from the writer's identity and never by the writer |
| status | unread, read, answered, expired |
| default, deadline | on questions only, the recommended default and when a line may act on it |
| body | markdown, bounded in size |

The provenance field is the mechanism for the audit's relayed-ruling class. Only the operator's
client can write `operator-direct`, so a line forwarding the operator's words produces a `relayed`
record by construction, and policy can require an `operator-direct` answer before any irreversible
action.

**Interface.** An MCP server, so the same tools work in an interactive session, a headless worker,
and the driver, on any harness that speaks MCP. Each call returns bounded output.

| Tool | Returns |
|---|---|
| `inbox(unread, limit)` | headers of unread or recent messages to this line |
| `read(id)` | one message, marked read |
| `last(n, from)` | the last n messages, optionally from one sender |
| `from_operator(since)` | rulings and answers, the ledger view |
| `send(to, kind, body, in_reply_to)` | a record id |
| `ask_operator(question, options, default, deadline)` | a question id |
| `post_handoff(body)` | a handoff record id the resume prompt can name |

The operator's side is a command-line client with `inbox`, `answer`, and `rule`, and a digest of
open questions sorted by deadline.

**Wake-up.** On every write the server posts a one-line notice to the target line's inbox socket,
which Claude Code exposes to hooks as an environment variable, so an idle line starts a turn and
reads. A line that is not running reads on its next start, because the store is the source of truth.

**Questions with defaults.** A question carries a recommended default and a deadline. Policy
decides what happens at expiry, act on the default for reversible kinds and wait for irreversible
ones, and the record shows which happened.

## What already exists, read on 2026-09-08

| Project | Provides | Gap against the design |
|---|---|---|
| MCP Agent Mail, MIT, a Rust rewrite exists | identities, inboxes, threads, acknowledgments, glob file reservations, lexical and semantic search, 38 tools, SQLite plus a git-backed archive with an audit trail, an operations interface and an overseer compose form | no store-enforced provenance label, no question objects with defaults and deadlines, no bounded resume view, no wake-up, and it does not centralize human arbitration |
| Ask User Questions MCP, MIT | one queue of questions from parallel agents answered in a terminal interface or by command, persistent sessions | the agent blocks until answered, no timeout by default, no default answers |
| Agent Peers MCP, MIT, experimental | peer messaging for Claude Code and Codex sessions on one machine, at-least-once delivery with acknowledgment, a push into Claude Code through a channel | single machine, three stars, no threads, no operator role |
| Beads | a git-backed issue tracker as agent memory with dependencies | tasks rather than messages, no provenance |

**Recommendation.** Do not write the mailbox. Evaluate MCP Agent Mail first and measure its context
cost, then build the thin layer that is missing, an operator client that is the only writer able to
produce `operator-direct` records, a question kind with default and deadline, a compact resume view,
and a wake-up notice on every write. The standalone project is that layer plus the driver.

## Keleusma's place in it

The harness core is input and output bound and is the wrong shape for the language. The policy
layer is the right shape, a bounded total stream processor over small typed inputs, context
percentage, event kind, tool-call class, handoff validity, and provenance label, producing
decisions. Three shipped features map onto the audit's findings, module signing so the governed
session cannot edit its own policy, information-flow labels so only operator-labelled inputs can
authorize the irreversible set, and hot code swap for thresholds and deny sets.

Feasible today against Claude Code as it is, using the shell natives the CLI registers, a context
monitor as a plugin background monitor, an irreversible-action guard as a pre-tool-use hook running
a signed module in strict mode, a resume validator as a session-start hook, and a provenance gate.
A full driver in Rust with the policy in Keleusma is feasible and heavier, one to two weeks, and
makes the language load-bearing for the loop rather than for the policy. A greenfield harness over
the raw API is not needed. The staged route is the driver first, the monitor and guard beside it in
the first week, and the policy module as a second stage once the driver shows what the policy's
inputs are.

## Open with the operator

1. Whether to build the harness at all, and as a standalone project.
2. Whether to evaluate MCP Agent Mail before building any mailbox.
3. Whether the policy layer is a Keleusma deliverable, and on which line.
4. Which harness runs the lines next, since the driver layer is harness-specific and the mailbox is not.
