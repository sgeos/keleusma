# Parallel-Agent Development

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

How to run more than one agent (or human) on this repository at the same time
without the working trees, the process channels, or the release-line gate
colliding. This document is the protocol; [`scripts/worktree.sh`](../../scripts/worktree.sh)
is the mechanical enabler.

The guiding principle is **isolate the mutable, serialize the shared**. Each agent
gets a private working tree and branch (isolated). The release line and the full
gate are shared resources and are entered one agent at a time (serialized).

## 0. Two version branches (the current arrangement, 2026-08-08)

**Sections 1 to 5 assume several feature branches off ONE trunk. That is not the
current topology, and the difference changes what they prescribe.** Two version
branches are live:

| Branch | Owns | Working tree |
|---|---|---|
| `v0.2.3` | The wire-format programme | the primary repo directory |
| `v0.3.0` | Native code generation | `../keleusma-worktrees/<leaf>` |

`v0.3.0` was cut from `v0.2.3` and they have since diverged. Feature branches are
cut from whichever version branch owns the work, with `KEL_TRUNK=v0.3.0` where
needed — both `worktree.sh` and `merge-to-trunk.sh` honour it.

### The three channels are single-writer and belong to `v0.2.3`

`REVERSE_PROMPT.md`, `DESIGN_JOURNAL.md`, and `TASKLOG.md` would conflict on
**every** merge between version branches. `DESIGN_JOURNAL.md` is newest-first, so
its conflict lands at the top of the file every single time.

**`v0.3.0` therefore touches none of the three** and writes only
`docs/process/handoffs/v0.3.0.md`, which exists **on `v0.3.0` only** and is
deliberately not linked here, since a cross-branch link cannot resolve on this
branch. `v0.2.3` keeps the three channels moving so there
is a coherent history to inherit when `v0.3.0` becomes a line in its own right.
This asymmetry is deliberate and the cost falls on `v0.3.0`.

### The downstream branch REBASES, and owns doing it

**Operator decision, 2026-08-08: `v0.3.0` is rebased onto `v0.2.3` as work
proceeds, not merged.** Keeping it a linear extension means the eventual
integration is a fast-forward rather than an accumulated tangle of merge commits.

```
git fetch origin && git checkout v0.3.0 && git rebase origin/v0.2.3
```

`v0.3.0` pulls; **`v0.2.3` never pushes into `v0.3.0`**. One-directional ownership
means nobody edits a branch they do not own, and — critically under a rebase
policy — nobody rewrites history under a session that has local work built on it.

**Sync before starting each increment, not on a timer.** `v0.3.0` was cut two
commits stale on 2026-08-08 and inherited a specification describing a format that
no longer existed plus a plan document saying its own work was blocked. Drift here
does not announce itself; it presents as confidently reading the wrong thing.

#### What rebasing costs, stated plainly

Rebasing rewrites history, so once `v0.3.0` is published each sync needs a
**force-push**. That is normally a guarded action; here it is standing policy for
`v0.3.0` specifically, and it carries obligations:

- Use `--force-with-lease`, never a bare `--force`. It refuses when the remote
  moved unexpectedly, which is the difference between overwriting your own stale
  view and overwriting someone else's work.
- **Only the branch owner force-pushes it**, and only when no other session has
  unpushed work on top. Announce it rather than assume.
- **Feature branches off `v0.3.0` must rebase after each such sync.**
  `merge-to-trunk.sh` already rebases onto `origin/$TRUNK` before gating, so the
  normal merge path handles this; a long-lived feature branch between syncs is the
  case that needs manual attention.
- A gate result is only valid for the exact tip it ran against. A rebase changes
  the tip, so a gate that predates it must be re-run.

**Rebase early and often.** The cost scales with how much has accumulated on both
sides, and the force-push hazard scales with how many sessions have built on the
old history. A sync per increment is nearly free; a sync per week is not.

### Cross-branch code dependencies are read-only, declared, and one-directional

`v0.3.0` reads `src/wire_schema.rs` (`AuxView`, `AuxOffsets`) and
`src/bytecode.rs`, and commits to modifying neither. `v0.2.3` commits to
announcing changes to that surface before making them. If a widening is genuinely
needed, it is requested rather than taken.

Note `AuxResolved` in `src/vm.rs` is **private** and deliberately not part of that
surface; it is a VM-internal cache. `AuxView` is the shared read surface.

### Genuinely shared files

[`scripts/release-gate.sh`](../../scripts/release-gate.sh) is the only one so far.
Keep edits in separate regions — the orphan-reap preflight sits above the first
`step()` call, per-package steps go at the end — and expect to rebase rather than
merge when both lines touch it.

### Where `v0.3.0` merges

The rebase policy settles the mechanism: `v0.3.0` stays a linear extension of
`v0.2.3`, so its integration is a fast-forward whenever it happens, and
`merge-to-trunk.sh`'s `v0.2.3` default is the correct one rather than an accident.

What remains open is only the *timing* — whether `v0.3.0` lands while `v0.2.3` is
still the active line, or follows it to `main` afterwards. That is a release
question rather than a branching one, and it does not need answering to proceed.

### What does not change

Full-gate serialization (section 4) still binds, and matters more here than under
sibling feature branches. A full gate runs about 2h30m, only one may run at a
time, and the result reaches the other session only through the operator. Plan for
roughly one merge per stream per half-day and batch accordingly.

**Reap orphans with a PATH-SCOPED pattern.** `pkill -f "$PWD/target/debug/deps"`
from the worktree whose gate died. An unscoped `pkill -f 'target/debug/deps'`
matches every worktree on the machine and will kill a sibling session's live gate.

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

### Worktrees or separate clones?

Both give an agent an independent working directory on its own branch. The choice:

- **Worktrees (the default, what `worktree.sh` uses).** One clone, many working
  directories sharing a single `.git` object store, remotes, and hooks.
  Disk-efficient, one fetch serves every tree, and a branch may be checked out in
  at most one tree at a time — a useful guardrail against two agents editing the
  same branch. Each tree gets its own `target/`, so builds do not collide. Best
  for several agents on one machine.
- **Separate clones (`git clone` per agent).** Full independence: each clone has
  its own object store, config, hooks, and `target/`. Heavier on disk (a whole
  copy apiece) and each fetches on its own, but there is zero shared state and two
  clones may sit on the same branch. Best when agents run on different machines,
  when you want total isolation from a shared hook or object store, or when you
  deliberately want two checkouts of one branch.

Recommendation: prefer worktrees on a single development machine — they are what
the tooling here assumes and they avoid duplicating the object store — and reach
for separate clones for cross-machine work or a fully decoupled checkout. Either
way the per-branch handoffs, the merge protocol, and the gate discipline below
apply unchanged.

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

The self-hosted pipeline still has coupling that bounds true concurrency, though the
biggest single blocker is now gone:

- **P11 (encoding capacity) has LANDED** (2026-07-24, see
  [`docs/decisions/ENCODING_CAPACITY_BRIEF.md`](../decisions/ENCODING_CAPACITY_BRIEF.md)):
  the inter-stage encodings have ample tag headroom and the split-tag workarounds are
  retired. New construct work no longer competes for scarce encoding slots, so several
  self-host constructs that touch *disjoint* `.kel` code can now proceed in parallel — the
  former hard serialization is lifted.
- Remaining coupling: a change to the **shared inter-stage protocol** (a stage's record/
  token/op format, or the host driver in `compiler/src/` — one shared `drive_parse_records`
  after the consolidation) still needs cross-stage coordination and should be a single
  stream. Likewise a runtime **wire-format** change edits `src/` and requires re-syncing
  `compiler/`, so it is not concurrent with self-host work.
- Two agents editing the *same* `.kel` stage still conflict; partition by stage or by
  disjoint construct areas.

So near-term concurrency spans the independent streams (guide/book, arena, bench, the RTOS
example, runtime features that do not touch the wire format) **and now** self-host construct
work on disjoint areas. Reserve a single stream only for shared-protocol / wire-format
changes.

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

Run [`scripts/merge-to-trunk.sh`](../../scripts/merge-to-trunk.sh) from the feature
branch. It performs the whole serialization sequence and refuses to merge if the
trunk moved under it:

1. Rebase the branch onto the current trunk tip (`git fetch` + `git rebase origin/v0.2.3`).
2. Run `scripts/release-gate.sh` to green (the mandatory pre-merge gate).
3. Re-check the trunk tip. If it moved while the gate ran, another agent merged
   first — abort and re-run (which rebases onto the new tip and re-gates).
4. Fast-forward merge and push.

The script is a dry-run by default (rebase and gate, then stop and print the
commands); pass `--execute` to merge and push. Doing it by hand is fine too — the
sequence above IS the protocol. The re-check in step 3 is the guard against a gate
that raced another merge; a gate result is only valid for the exact trunk tip it
ran against, so there is no lock daemon, just "gate, then confirm nothing moved."

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
2. Confirm no stream depends on a wire-format or shared inter-stage-protocol change
   that another stream is making concurrently.
3. `scripts/worktree.sh new <branch>` per stream.
4. Each agent iterates with `scripts/fast-check.sh`; per-branch handoff in
   `docs/process/handoffs/`.
5. Merge back one at a time with `scripts/merge-to-trunk.sh` (section 4).
6. `scripts/worktree.sh rm <branch>` when merged.

## Status

**Two version branches have been live since 2026-08-08** — see section 0, which
supersedes the single-trunk assumption in sections 1 to 5 where they differ.
`v0.3.0` is kept a linear extension of `v0.2.3` by rebasing rather than merging
(operator decision, 2026-08-08), which makes its eventual integration a
fast-forward.

The **P11** encoding-capacity change has landed (2026-07-24), which was the hard blocker
on self-host parallelism — the inter-stage encodings now have headroom, so construct work
on disjoint `.kel` areas can run in parallel (section 2). The remaining constraint is only
shared-protocol / wire-format changes, which stay single-stream. See
[`docs/decisions/ENCODING_CAPACITY_BRIEF.md`](../decisions/ENCODING_CAPACITY_BRIEF.md).
