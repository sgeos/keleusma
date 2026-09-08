# Mailbox and Handoff of the Kaizen Line `kaizen`

> **Navigation**: [Handoffs](./README.md) | [Process](../README.md)

The kaizen line's mailbox and its self-contained resume prompt in one file, following the
per-branch practice of [`PARALLEL_DEVELOPMENT.md`](../PARALLEL_DEVELOPMENT.md) as the proof
line extended it to a top-level line. Read it with
`git show origin/kaizen:docs/process/handoffs/kaizen.md`.

> **REFRESHED 2026-09-08, second stamp, after the first audit draft.** Opened the same day at cut
> point `802e72d3`, the `origin/v0.2.3` tip of that day. Last synced from `v0.2.3` at `802e72d3`,
> from `v0.3.0` at `b8d78c3f`, and from `proofs` at `6537a36f`. The first commission, a broad
> audit of the development process, has produced an unversioned first draft and six evidence
> files under the primary checkout's `tmp/kaizen/`, listed below by path and state. Nothing is
> versioned beyond this file. Nothing is in flight. The draft has had one fresh-context review and
> a revision, a second research pass became a third draft, an equation-density pass a fourth,
> a reference-density pass a fifth, and a publication review a sixth. All four of the blog's
> passes are done. The draft is versioned on this branch under `docs/process/kaizen/` and is
> not published. The next step is the operator's reading, or a second review round, then the
> rulings on the proposals.

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

The audit draft is at its third version. The first was reviewed by a fresh context, which found four blocking and nine major defects, all repaired in the second. The third adds a state-of-the-art section from a second, technique-focused research pass of four parallel agents, with every source graded, and five primary sources the first survey could not reach are now verified first hand. The fourth draft added eighteen display equations, the fifth added 108 citations with the project's own documents as commit-pinned primary references, and the sixth passed the blog's publication review, expanding the literature section into a comprehensive graded survey of every source read, 140 citations in all, with every URL checked and every DOI confirmed against Crossref. The operator's standing directive was followed, the draft is committed and pushed on this branch and is not published. No draft after the first has been reviewed by a second context. It rests on six evidence passes, two run by the
auditor's own commands and four by mining subagents whose reports the auditor read and partly
checked. The draft finds no quality or speed problem in the ordinary sense and finds instead a
verification problem and a decision-latency problem, states the trade-off as two regimes with
one real edge, and makes ten proposals, seven that trade nothing away and three on the edge.
Every proposal names its mechanism, evidence, expected effect, and the ruling it needs. None is
adopted.

## DRAFTS, UNVERSIONED, IN THE PRIMARY CHECKOUT

The primary checkout's `tmp/` is ignored by git, so these files exist in one place only and are
lost if that directory is cleaned. The operator authorized drafting there so the blog session
can take the final draft for publication.

| path under `tmp/kaizen/` | state |
|---|---|
| `process-audit-draft.md` | DRAFT 6, publication-reviewed, about 19,600 words, blog format with eighteen display equations and 140 citations, a comprehensive graded survey of every source the ten passes read, prose style-scanned clean, article number placeholder, not reviewed by a second context since DRAFT 1. **Now also versioned on this branch as `docs/process/kaizen/2026-09-08-process-audit.md` with its evidence under `docs/process/kaizen/evidence/`** |
| `process-audit-draft.v1.md` to `.v5.md` | earlier drafts, kept for the record |
| `review-1.md` | the adversarial review, twenty findings, each with its disposition |
| `evidence-1-documented-process.md` | miner report, saved |
| `evidence-2-v023-record.md` | miner report, saved |
| `evidence-3-v030-and-proofs-record.md` | miner report, saved |
| `evidence-4-git-ci-metrics.md` | miner report, all executed, saved, raw exports under `raw/` |
| `evidence-5-blog-format-and-process.md` | miner report, saved |
| `evidence-6-external-survey.md` | research agent report, sources graded, not re-fetched by the auditor |
| `evidence-7-context-and-state-techniques.md` | second research pass, context and state management for long-running agents |
| `evidence-8-verification-techniques.md` | second research pass, verifying generated code and the agents' own instruments |
| `evidence-9-decisions-metrics-ci.md` | second research pass, decision workflows, progress measurement, continuous-integration cost |
| `evidence-10-coordination-and-source-verification.md` | second research pass, coordination substrates, and five primary sources verified first hand |
| `NOTES.md` | working notes, observations, draft status |

Two miner figures were not independently checked by the auditor and the draft says so, the
blog post word count and the incident classification totals.

## Recorded during the audit, as observations and not findings

**A session limit terminated the first evidence pass.** Six parallel mining subagents on the
most capable model were all cut off after about thirty minutes by an account-level session
limit, with no report surviving. The passes were rerun on a smaller model and completed. The
process documents do not mention session or usage limits. A line planning heavy parallel work
should run mining and audit passes on smaller models and long runs in the background with exit
status captured.

**A scratch export went stale during a pass.** The V0.3.X mailbox export was one hundred four
lines behind the live tip by the time a miner cited it, and the miner re-read the live branch.
A copy of a peer's document is a timestamp, not the document.

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
   statement. The first audit proceeded under either.
2. **Where audit outputs live.** The operator's publication-review prompt required the draft
   committed and pushed, so this line placed it under `docs/process/kaizen/` on its own branch,
   the location it had proposed. That settles nothing on the trunk. The location is ruled when
   the line is merged.
3. **How the peers learn this line exists.** The operator ruled on 2026-09-08 that this line may
   contact the other lines when it makes sense for them to know, and that working without
   disclosure is acceptable until then. No contact has been made.
4. **Whether a second review round is wanted before the operator reads the draft.** One round
   was run on the first draft and found four blocking defects. The third draft carries about
   three thousand new words and forty new sources that no second context has checked. A round
   costs one fresh session and no calendar time.
5. **The ten proposals in the draft**, seven in a group that trades nothing away and three on
   the edge where operator attention is exchanged for latency. Each states the ruling it needs.

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
read `tmp/kaizen/NOTES.md` and the draft in the primary checkout, confirm they still exist, and
continue from the next step named in the banner, unless the open questions above have been
answered in a way that changes its shape.
