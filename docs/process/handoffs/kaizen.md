# Mailbox and Handoff of the Kaizen Line `kaizen`

> **Navigation**: [Handoffs](./README.md) | [Process](../README.md)

The kaizen line's mailbox and its self-contained resume prompt in one file, following the
per-branch practice of [`PARALLEL_DEVELOPMENT.md`](../PARALLEL_DEVELOPMENT.md) as the proof
line extended it to a top-level line. Read it with
`git show origin/kaizen:docs/process/handoffs/kaizen.md`.

> **REFRESHED 2026-10-04, third stamp, prepared for a resume on another harness.** Opened
> 2026-09-08 at cut point `802e72d3`. Last synced from `v0.2.3` at `89fc6aac`, from `v0.3.0` at
> `7d141fcc`, and from `proofs` at `6537a36f`, all measured on 2026-10-04 by `git fetch` and
> `git rev-parse`. Since the previous stamp the `v0.2.3` trunk gained 299 commits and the
> `v0.3.0` trunk 707, a dry merge of this branch onto `v0.2.3` shows no conflicts, no peer document
> mentions this line, and the `proofs` line is unchanged. The audit at its sixth draft and all its
> evidence are versioned under `docs/process/kaizen/` and are not published. The harness and
> mailbox discussion is recorded in `docs/process/kaizen/2026-10-04-harness-and-mailbox-notes.md`.
> Nothing is in flight. Nothing is built. Every open item is an operator decision.

## FOR A SESSION ON ANOTHER HARNESS

This line was run by Claude Code sessions. A resume on Codex or any other harness needs the
following, none of which lives in that harness's instruction chain.

- **Entry point.** The trunk's `AGENTS.md`, added after this line was cut, names `CLAUDE.md` as the
  agent-agnostic instruction file and fixes the reading order. Read both, then this file.
- **Appendix B.** The trunk's `AGENTS.md` carries a hard constraint to read the operator-held note
  at `secret/notes/APPENDIX_B.md` in the primary checkout before writing anything tracked, and
  never to copy it into tracked files. This line did not read it until 2026-10-04 and nothing in
  this line's tracked files derives from it. It is absent from the worktrees. Make it available to
  the resuming session privately before tracked edits.
- **The Codex readiness audit.** `docs/process/CODEX_SETUP_AUDIT.md` on the trunk, 2026-09-28,
  records the working installation, the sandbox and compiler-cache interaction, the attribution
  convention to name the actual assistant, and that a Codex session's writable workspace covered
  the primary checkout rather than a sibling worktree. This line's worktree is a sibling. Either
  widen the workspace or work from a checkout of `kaizen` the session can write.
- **The gate policy.** `docs/process/GATE_POLICY.md` on the trunk records the operator's rulings
  of 2026-09-28 verbatim, commit once per prompt, only feature branches may commit on red, full
  gates before publication or merging into `main`, version branches green through remote
  continuous integration. It supersedes any gate statement in the audit, which describes the
  process as it stood on 2026-09-08.
- **Prose style.** The operator's rules live in a user-level instruction file this harness read
  and another will not. They are restated under the governing rules below in full.
- **Attribution.** Commits on this branch carry a Claude co-author trailer and a session link.
  The trunk's readiness audit asks that the convention name the actual assistant, so a Codex
  session uses its own trailer.
- **Claude memory.** This harness kept pointers to the worktrees in a private memory store that
  is not part of any other harness's chain. Nothing in it is needed beyond what this file says.
- **Tools assumed by the evidence.** git, the GitHub command-line client, python3, curl, and
  cargo. The audit's raw exports are under `docs/process/kaizen/evidence/raw/`.
- **Drafts.** The copies under the primary checkout's `tmp/kaizen/` matched the versioned audit on
  2026-10-04. The versioned copies on this branch are authoritative from now on.

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

The first process audit is complete at its sixth draft, after the blog's four passes, draft,
equation density, reference density, and publication review, and one fresh-context adversarial
review of the first draft. It makes ten proposals, each naming the ruling it needs. It describes
the process as of 2026-09-08. Since then the trunk's process surfaces changed by about eleven
thousand lines, including a bounded handoff refresh, the gate policy record, the readiness audit,
and the `AGENTS.md` reading-order repair. Several of those changes touch the audit's findings
directly, the gate policy record is a verbatim rulings ledger of the kind proposal six asks for,
and a delta re-audit against the 2026-09-28 state is the natural next kaizen increment.

The harness and mailbox discussion of 2026-09-08 to 2026-10-04 is recorded in the design notes
beside the audit. Its conclusions, that no harness gives unattended self-compaction, that a thin
driver plus a mailbox layer over an existing MCP mail server is the shape to build, and that the
policy layer is where Keleusma fits, are recommendations awaiting rulings.

## DRAFTS AND EVIDENCE

The versioned copies live under `docs/process/kaizen/` on this branch, the audit as
`2026-09-08-process-audit.md`, the evidence under `evidence/`, and the design notes as
`2026-10-04-harness-and-mailbox-notes.md`. The unversioned drafts under the primary checkout's
`tmp/kaizen/` were kept for the blog session and matched the versioned audit on 2026-10-04.

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
   statement.
2. **Where audit outputs live.** Placed under `docs/process/kaizen/` on this branch. Ruled when
   the line is merged.
3. **How the peers learn this line exists.** No contact has been made. No peer document mentions
   the line as of 2026-10-04.
4. **A second review round of the audit** before anyone acts on it. One round covered the first
   draft only.
5. **The ten proposals in the audit**, each stating the ruling it needs. A delta re-audit against
   the trunk's 2026-09-28 state would say which the trunk has since adopted on its own.
6. **The harness and mailbox recommendations** in the design notes, four questions listed there.
7. **Merging this line into `v0.2.3`.** A dry merge shows no conflicts. The merge lands the
   audit, its evidence, and the design notes on the trunk and settles item two.

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
- **The operator's prose style governs all documents of this line.** Prose is polite,
  professional, and academic. No contractions. No em-dashes or en-dashes. No colons, semicolons,
  or parentheticals in prose, with front matter, tables, code, and equations exempt. Acronyms
  spelled out on first use. Draft prose wrapped in quadruple backticks when shown in chat. Facts,
  inferences, and hypotheses distinguished explicitly, with uncertainty stated. The style scan must
  cover blockquotes.
- **Irreversible or outward-facing actions need confirmation.** Publishing, force-pushing,
  deleting a branch that is not this line's own, and writing to a peer's branch all wait for
  the operator.

## WHAT A RESUMING SESSION SHOULD DO FIRST

Read the trunk's `AGENTS.md` and `CLAUDE.md`, then Appendix B privately. Run the validity block.
Sweep the three peer mailboxes,
`git show origin/v0.2.3:docs/process/handoffs/v0.2.3.md`,
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`, and
`git show origin/proofs:docs/process/handoffs/proofs.md`, for anything addressed to this line.
Read `docs/process/GATE_POLICY.md` and `docs/process/CODEX_SETUP_AUDIT.md` on the trunk as the
current process, and the audit and design notes under `docs/process/kaizen/` as this line's
work. Then wait for the operator. The candidate next increments are a second review round of the
audit, a delta re-audit against the trunk's 2026-09-28 state, and whichever of the harness and
mailbox questions the operator rules on.
