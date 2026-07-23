# Parallel-Agent Development

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

How to run more than one agent (or human) on this repository at the same time
without the working trees, the process channels, or the release-line gate
colliding. This document is the protocol; [`scripts/worktree.sh`](../../scripts/worktree.sh)
is the mechanical enabler.

The guiding principle is **isolate the mutable, serialize the shared**. Each agent
gets a private working tree and branch (isolated). The release line and the full
gate are shared resources and are entered one agent at a time (serialized).

## 1. Isolation: one worktree and one branch per agent

Every concurrent agent works in its own git worktree on its own short-lived
feature branch cut from the active trunk (currently `v0.2.3`). Worktrees share
one `.git` object store but have independent working directories and indexes, so
two agents never fight over one dirty tree.

```
scripts/worktree.sh new  feat/some-thing    # tree + branch off the trunk
scripts/worktree.sh list                     # show all trees
scripts/worktree.sh rm   feat/some-thing     # remove tree + delete branch
```

Trees live under `../keleusma-worktrees/<leaf>`, siblings of the repo. The trunk
defaults to `v0.2.3` and is overridable with `KEL_TRUNK`. Branch names follow the
[`GIT_STRATEGY.md`](./GIT_STRATEGY.md) scope convention (`feat/`, `fix/`, `docs/`,
`refactor/`, `test/`, `chore/`).

When spawning agents from within a Claude Code session instead of separate
terminals, the `Agent` tool's `isolation: "worktree"` option provides the same
isolation automatically for agents that mutate files.

## 2. Pick non-conflicting workstreams

Parallelism only helps when the streams do not edit the same files. The table
below maps each stream to the paths it owns. Two agents are safely concurrent
when their owned paths do not overlap.

| Workstream | Owns (primary paths) | Notes |
|-----------|----------------------|-------|
| Runtime / ISA | `src/`, `tests/` | A wire-format or `BYTECODE_VERSION` change here forces a `compiler/` re-sync — see the coupling note below |
| Self-host pipeline | `compiler/kel/`, `compiler/src/` | Internally **lockstep**; treat as a single stream (see below) |
| Guide / book | `book/`, `docs/` | Independent of code |
| Arena | `keleusma-arena/` | Standalone crate, independent |
| Cost-model bench | `keleusma-bench/` | Independent |
| CLI | `keleusma-cli/` | Mostly independent |
| RTOS example | `examples/rtos/` | Detached crate, fully independent |

### The coupling that limits self-host parallelism (be honest about this)

The self-hosted pipeline is **not** internally parallelizable right now, and this
bounds how much true concurrency the roadmap currently offers:

- The encoding-capacity change (priority **P11**, see
  [`docs/decisions/ENCODING_CAPACITY_BRIEF.md`](../decisions/ENCODING_CAPACITY_BRIEF.md))
  touches every `.kel` stage and both Rust drivers in **lockstep**. It cannot be
  split across agents and it must land before dependent self-host work.
- The nested-equality frontier (tuple-of-struct, enum-in-struct, deeper nesting)
  consumes the very encoding P11 changes, so it conflicts with P11 and with
  itself. One self-host agent at a time.
- A runtime wire-format change (for example the 24-bit shared-data widening plan)
  edits `src/` **and** requires re-synchronising `compiler/`, so it is not
  concurrent with any self-host work.

Genuine near-term concurrency therefore lives across the *independent* streams:
guide/book, arena, bench, the RTOS example, and any runtime feature that does not
touch the wire format. Self-host work is effectively single-threaded until P11
lands and de-couples the stages. Do not pretend otherwise when planning a fan-out.

## 3. Communication channels under parallelism

The single-writer process files in [`COMMUNICATION.md`](./COMMUNICATION.md) assume
one active session. Under parallelism they need per-agent lanes so agents do not
overwrite each other:

| Channel | Solo behaviour | Parallel behaviour |
|---------|----------------|--------------------|
| `REVERSE_PROMPT.md` | Overwritten each task | **Do not** overwrite from a parallel branch. Write a per-branch handoff at `docs/process/handoffs/<branch-leaf>.md` instead |
| `DESIGN_JOURNAL.md` | Append-only | Still append-only; append a dated, branch-tagged entry. Append/append merge conflicts are trivial (keep both) |
| `TASKLOG.md` | Shared, incremental | Claim one task row per agent, tagged with the branch; edit only your own row |
| `PROMPT.md` | Human to AI, read-only for AI | Unchanged |

The primary agent (the one that will overwrite `REVERSE_PROMPT.md` and integrate
the handoffs at the end) reconciles the per-branch handoffs back into
`REVERSE_PROMPT.md` when the parallel burst finishes.

## 4. Serialize the release line and the full gate

The release line (`v0.2.3`) and `scripts/release-gate.sh` are shared and must be
entered one agent at a time.

**Merge protocol (per branch, in turn):**

1. Rebase the branch onto the current trunk tip: `git fetch origin v0.2.3 && git rebase origin/v0.2.3`.
2. Run `scripts/release-gate.sh` to green (mandatory pre-merge gate).
3. Re-check the trunk tip with `git ls-remote origin v0.2.3`. If it moved while
   you were gating, another agent merged first — go back to step 1.
4. Fast-forward merge and push.

This "rebase, gate, re-check tip, merge" loop is the serialization mechanism.
There is no lock daemon; the `ls-remote` re-check in step 3 is the guard against a
gate that raced another merge. A gate result is only valid for the exact trunk tip
it ran against.

**Gate discipline (this is also process-audit item 1):**

- Inner loop: [`scripts/fast-check.sh`](../../scripts/fast-check.sh) is cheap and
  safe to run concurrently across agents.
- The full `scripts/release-gate.sh` is CPU-heavy, and the self-host tests already
  contend for cores. Do **not** run several full gates concurrently — it saturates
  the machine and inflates every agent's wall-clock. Serialize full gates at the
  merge point per the protocol above. This is why the merge is a queue, not a
  free-for-all.

## 5. Checklist for launching a parallel burst

1. Confirm the chosen streams own disjoint paths (section 2).
2. Confirm no stream depends on a wire-format or P11 change that another stream is
   making concurrently.
3. `scripts/worktree.sh new <branch>` per stream.
4. Each agent iterates with `scripts/fast-check.sh`; per-branch handoff in
   `docs/process/handoffs/`.
5. Merge back one at a time via the section 4 protocol.
6. `scripts/worktree.sh rm <branch>` when merged.

## Open dependency

Full self-host parallelism is gated on the **P11** encoding-capacity decision
landing first (it de-couples the pipeline stages). Until then, keep self-host work
single-threaded and parallelise only the independent streams in section 2. See
[`docs/decisions/ENCODING_CAPACITY_BRIEF.md`](../decisions/ENCODING_CAPACITY_BRIEF.md).
