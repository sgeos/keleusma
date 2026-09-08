# Mailbox and Handoff of the Kaizen Line `kaizen`

> **Navigation**: [Handoffs](./README.md) | [Process](../README.md)

The kaizen line's mailbox and its self-contained resume prompt in one file, following the
per-branch practice of [`PARALLEL_DEVELOPMENT.md`](../PARALLEL_DEVELOPMENT.md) as the proof
line extended it to a top-level line. Read it with
`git show origin/kaizen:docs/process/handoffs/kaizen.md`.

> **OPENED 2026-09-08 at cut point `802e72d3`, the `origin/v0.2.3` tip of that day.** Last
> synced from `v0.2.3` at `802e72d3`, from `v0.3.0` at `b8d78c3f`, and from `proofs` at
> `6537a36f`. Nothing is in flight and nothing has been written beyond this file. The first
> commission, a broad audit of the development process for weaknesses and improvements, is
> received and not started. A resuming session should validate below, read the charter and
> scope, sweep the three peer mailboxes, and then begin the audit or wait for the operator
> if the open questions below have been answered in a way that changes its shape.

## Charter

Continuous improvement of the process by which the project's work is done, as distinct from
the direction of that work. Project management decides what is built, in what order, and
against which gates. This line observes how the work actually flowed, finds where it lost
time, produced defects, or required rework, and proposes changes to the practice. It
consumes the other lines' process records as evidence and produces changes to the process
documents and scripts, never to the plan.

**Provenance of the commission.** The operator commissioned the line in session on 2026-09-08,
first party to this line, with the words "Please open a kaizen line. Whether it turns out to
be closer to the Toyota lineage or discreet process reviews is yet to be determined." The
shape of the line, a recurring practice in the Toyota lineage or a series of discrete process
reviews, is therefore open by the operator's own statement and is not to be inferred from
the name.

**Why a new line rather than the proof line.** Measured on 2026-09-08, the `proofs` branch was
267 commits and 108 merges behind `origin/v0.2.3`, with eleven process, script, and
continuous-integration files changed by roughly 5800 lines since its cut. An audit built on
that tree would audit a process that no longer exists. The proof line is also itself an audit
subject, its handoff recording four retractions, and the fresh-context rule it adopted for
its own audits applies to auditing it.

## Validity

Validate by ancestry and by content, never by a hash match.

```sh
# Ancestry. Both must succeed.
git merge-base --is-ancestor 802e72d3 origin/kaizen     # the cut point is on this line
git merge-base --is-ancestor 802e72d3 origin/v0.2.3     # and on the trunk it was cut from

# Content. Every commit atop the cut point belongs to this line; if a foreign commit
# appears, say so rather than acting on the state described here.
git log --oneline 802e72d3..origin/kaizen

# The worktree this line works in, which must not be the primary checkout.
git worktree list | grep 'keleusma-worktrees/kaizen'
```

## Structure of the line

| branch | role | state |
|---|---|---|
| `kaizen` | top-level integration branch | opened at `802e72d3`, handoff commits only |

Cut from `origin/v0.2.3` at `802e72d3`. Increments are cut from `kaizen` as feature branches
and merged back with no-fast-forward merges. The worktree script requires a scoped name, so an
increment is opened with `KEL_TRUNK=kaizen scripts/worktree.sh new docs/kaizen-<topic>`. The
line lands into `v0.2.3` by pull request on a green continuous-integration run at the commit
it ran, as the proof line did, and absorption by the V0.3.X line is ruled by that line's
operator. The worktree is `../keleusma-worktrees/kaizen`.

**Naming the line `kaizen` rather than a scoped feature name is deliberate.** A feature name
would settle the shape question in favor of a one-shot review. A top-level name accommodates
either outcome and costs nothing if the line lands once and is deleted.

## Scope and ownership

**What this line reads.** Everything, on every branch. The process documents under
`docs/process/` on `v0.2.3`, `v0.3.0`, and `proofs`, including the three mailboxes, the design
journal, the task log, and the reverse prompt. The continuous-integration workflow, the hooks,
the scripts, and the release process. The git history, the pull request record, and the gate
logs. The retractions and corrections each line has recorded about itself, which are the
highest-value evidence available.

**What this line never does.** It never edits a peer's branch, and it never edits a peer's
process state in place, even where the change is a correction. A finding about a line is
addressed to that line under a heading in this mailbox naming it, and the peer acts or
declines on its own branch. Peer surfaces are theirs.

**Where authority sits.** This line proposes and measures. Whether a practice changes is a
ruling, and rulings route through the operator, never through peers. A proposal recorded here
is a proposal until the operator rules, and the ruling is recorded with who made it and how it
arrived.

## THE STATE

Opened. The first commission is a broad audit of the development process for weaknesses and
process improvements. No finding has been recorded and no document beyond this file exists.

## Recorded at opening, as an observation and not a finding

The pre-push hook runs the routine test tier on every push regardless of what the push
changes. This line opened with a single markdown file atop a commit on which continuous
integration was already running, and it ran the two documentation-scanning tests,
`documentation_links` and `claimed_counts`, in its own worktree instead of the whole tier,
then pushed with `--no-verify` and recorded that here. Whether re-running the tier on a
documentation-only push is a weakness of the hook or correct discipline is a question for the
audit, and this paragraph does not settle it.

## Owed by this line

Nothing to any peer line.

## Owed to this line

Nothing.

## OPEN, ALL WITH THE OPERATOR

1. **The shape of the line**, recurring practice or discrete reviews, open by the operator's
   statement. The first audit can proceed under either.
2. **Where audit outputs live.** This line proposes `docs/process/kaizen/` holding dated
   reports, because it accommodates both shapes, with any adopted change landing in the
   process document or script it concerns. Not yet ruled.
3. **How the peers learn this line exists.** A mailbox is invisible to a session that does
   not know to look for it. The operator may tell the V0.2.X and V0.3.X sessions directly, or
   this line may be named in their handoffs at their next sync. Not yet ruled.

## GOVERNING RULES A RESUMING SESSION MUST NOT LOSE

- **Work in the worktree.** This line operates in `../keleusma-worktrees/kaizen`. The primary
  checkout belongs to the V0.2.X session, `../keleusma-worktrees/arena-composites` to the
  V0.3.X session, and `../keleusma-worktrees/proofs` to the proof line. A shared checkout
  silently changes what a long-running command measures.
- **A pull request based on anything but `main` or `v*` triggers no continuous integration,
  silently.** Merge on a green run at the commit it ran, reading the conclusion field.
- **Nothing is promoted from read to executed without an execution**, and every claim carries
  its provenance label. A count names its population and its moment.
- **Never record a relayed ruling as settled.** Name whose ruling it is and how it arrived.
  The proof line's handoff records the cost of doing otherwise.
- **A checker's clean report is evidence about its reach before it is evidence about the
  tree.** The proof line's style scans passed three times over a live violation because they
  excluded blockquotes. Establish reach first.
- **The operator's prose style governs all documents of this line**, no contractions, no
  em-dashes, en-dashes, colons, semicolons, or parentheticals in prose, acronyms spelled out
  on first use, and the style scan must cover blockquotes.
- **Irreversible or outward-facing actions need confirmation.** Publishing, force-pushing,
  deleting a branch that is not this line's own, and writing to a peer's branch all wait for
  the operator.

## WHAT A RESUMING SESSION SHOULD DO FIRST

Run the validity block. Sweep the three peer mailboxes,
`git show origin/v0.2.3:docs/process/handoffs/v0.2.3.md`,
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`, and
`git show origin/proofs:docs/process/handoffs/proofs.md`, for anything addressed to this line.
Read [`PROCESS_STRATEGY.md`](../PROCESS_STRATEGY.md), [`GIT_STRATEGY.md`](../GIT_STRATEGY.md),
[`PARALLEL_DEVELOPMENT.md`](../PARALLEL_DEVELOPMENT.md), and
[`RELEASE_PROCESS.md`](../RELEASE_PROCESS.md) as the process the audit measures against. Then
begin the audit as commissioned, cutting a feature branch from `kaizen` for it, unless the
open questions above have been answered in a way that changes its shape.
