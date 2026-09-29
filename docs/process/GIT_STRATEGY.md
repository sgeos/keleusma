# Git Strategy

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

Version control conventions for Keleusma.

**Operator clarification, 2026-09-28.** Commit once per prompt. Only feature branches may commit on red. Version branches and `main` must remain green. Full gates must pass before publication or merging into `main`. Keep version branches green through remote continuous integration. Local checks may remain targeted when they provide high confidence that remote continuous integration will pass. Record the selected checks and their coverage limits. A version-branch merge does not itself require a full local gate. Detached packages need relevant local checks because root continuous integration does not cover them. Their complete gates remain mandatory before publication or merging their changes into `main`. A feature may be revised across prompts or abandoned. This policy supersedes conflicting historical guidance below.


## Branch Model

Keleusma uses a **release-branch model** with a four-level hierarchy: `main` holds releases, a
`vX.Y.Z` version branch integrates the next version, feature branches develop one increment, and
sub-feature branches decompose a feature. Work flows *up* the hierarchy through merges, and each
level has a defined green bar. This keeps the release line always shippable, keeps integration
continuous, and preserves the per-increment history the self-hosted compiler's byte-identical
differential oracle and the [design journal](./DESIGN_JOURNAL.md) rely on.

(This supersedes the earlier "trunk-based, merge-into-`main`, enforce-rebase" framing. The model is
a release-branch model, not trunk-based: the version branch — not `main` — is the day-to-day
integration trunk, and feature integration uses merge commits, not rebase-to-linear.)

### `main`

- Holds releases and is the single source of truth for what has shipped.
- **Must always be green** — it compiles, passes the full gate, and its CI is green. A red `main` is
  remedied **immediately**, as the top priority, ahead of other work.
- **Remedying a red `main`.** A trivial, urgent fix may be pushed **directly** to `main`, but the
  more proper path is a short `fix/` branch cut from `main`, brought green, and merged back with a
  no-fast-forward merge — it keeps the fix gated and reviewable. Either way, **forward-port the fix
  to the active version branch** (merge `main` forward, or re-apply it there) so the two lines do not
  diverge.
- Releases are cut **only from an all-green `main`**.
- Receives changes only by merging an all-green version branch (below). It is normal and expected for
  `main` to sit *behind* the active version branch between releases; that is the model working, not a
  divergence to reconcile.

### Version branch (`vX.Y.Z`)

- The integration branch for the next version, named for the version under development (for example
  `v0.2.3`).
- **Kept green at every merge point.** Because every feature merges in green (below), the branch is
  green after each merge; any transient red state is resolved **promptly and before the next merge**.
- **Must be all-green before merging into `main`**, via a no-fast-forward merge commit.
- Feature branches merge in via **no-fast-forward merge commits** (see [Merge Mechanics](#merge-mechanics)).
- **Direct commits are permitted only for small, green, documentation or process-file changes** (for
  example checkpointing `REVERSE_PROMPT.md`, `DESIGN_JOURNAL.md`, or `TASKLOG.md`). Every **code**
  change flows through a feature branch. A direct docs commit must itself be green when made.

### Feature branches

- **Cut from the active version branch.** Naming convention `<scope>/<short-description>` (for
  example `feat/selfhost-nested-eq`, `fix/parser-error-recovery`).
- **Intermediate commits may be red.** A feature branch is a workspace. Successive prompts may produce commits passing through red states, to converge on a working approach; only the branch **tip at
  merge** must be green. Nothing on the branch is load-bearing until it merges, so commit once per prompt with the verification state recorded.
- **Abandoning the branch is an acceptable outcome, not a failure.** If an approach does not
  converge, discard the branch (delete it unmerged) and start a new one rather than force an unsound
  approach to green. The version branch is only ever touched by green merges, so a dead-end feature
  branch costs nothing but the branch itself. This is the worst case, and it is a normal one.
- **Must be all-green before merging back** into the version branch. Use the remote continuous integration requirements below and relevant targeted checks for any detached package affected by the change. A duplicate local full workspace gate is not required when continuous integration covers it.
- **A PUSH CANCELS THE RUNNING CONTINUOUS-INTEGRATION JOB FOR THAT PULL REQUEST.** The workflow sets
  `cancel-in-progress: true` on a concurrency group keyed by the pull request's ref, which is
  correct for superseding a stale run and fatal for a session that pushes faster than the run
  finishes. **Measured 2026-09-11: five consecutive runs on one branch were cancelled and none ever
  completed**, because an automated loop woke every twenty minutes against a run that takes about
  forty. Six increments accumulated with the merge gate never once satisfied.
- **So: once a branch is ready, stop pushing to it and let the run finish.** A further increment is
  not progress if it destroys the verification the merge depends on. **A self-paced loop whose
  period is shorter than the verification it depends on will cancel that verification forever.**
  Work that cannot wait belongs on a separate branch, whose run is a different concurrency group.
- Merge via a **no-fast-forward merge commit**, so the version branch's first-parent history stays
  green and readable while the granular per-increment commits are preserved on the merged bubble.

### Sub-feature branches

- **Same process and standards as a feature branch**, except they are cut from a feature branch and
  merged back into **that feature branch** (not the version branch). Use them to decompose a large
  feature; the parent feature still merges into the version branch under the feature-branch rules.
- Nesting is **nominally unbounded**, but **one level of sub-feature is the practical limit**. A
  sub-sub-feature or deeper rarely makes sense and usually signals the parent feature should be split
  instead.

Supported `<scope>` values (branch names and commit subjects alike): `feat` (new feature), `fix`
(bug fix), `docs` (documentation), `refactor` (code restructuring), `test` (tests), `chore`
(maintenance).

## Merge Mechanics

- **Feature → version branch** and **version branch → `main`** use **no-fast-forward merge commits**
  (`git merge --no-ff`). The first-parent history of the target stays green; red work-in-progress
  lives only on the merged side branch, never on the target's spine.
- **Keep nested branches fast-forwardable relative to their base.** While a feature or sub-feature
  branch develops, keep it a **linear descendant of its base** (the version branch, or the parent
  feature) — when the base advances, **rebase the branch onto it** rather than merging the base in.
  The branch stays fast-forwardable (its base tip remains an ancestor of the branch tip), so the
  eventual no-fast-forward merge wraps a clean linear series with no conflict. This is orthogonal to
  the no-ff merge rule: the branch stays linear *toward* its base and is merged *into* the base with a
  bubble. Rebasing rewrites the branch's own (possibly red) work-in-progress commits, which is fine
  because a nested branch is private until it merges.
- A merge **proceeds once CI is green on a draft pull request** from the feature branch to the
  version branch (changed 2026-08-11; the local gate no longer gates a merge) — see
  [Definition of Green](#definition-of-green). The merging agent does not wait for CI to start the
  merge, but CI is binding afterward.
- **Direct commits** to the version branch or `main` are limited to small green documentation or
  process changes. Everything else flows through a feature branch.

### `git add -A` after a branch switch is the dangerous case

Untracked files survive a branch switch, but **ignore rules do not** — they are
tracked content and change with the branch. So a working tree built under one
branch, staged with `git add -A` on another whose ignore rules differ, sweeps in
whatever the second branch does not know to ignore. Nothing warns; the files are
simply untracked-and-unignored, which is exactly what `-A` is for.

This happened on 2026-08-09: a package's `.gitignore` lived on a feature branch,
the working tree was built there, and a `git add -A` on the version branch — which
did not carry the package — staged 571 build artifacts.

Two defences, in order of reliability:

- **Put the rule at the repository root**, where it exists on every branch that
  has the root file. `**/target/` covers every package, present or future, on
  every branch. A rule in a package subdirectory protects only branches that
  carry that subdirectory.
- **Read `git status --short` before committing after any branch switch**, and
  treat an unexpected file count as the signal it is.

### Exception: a line that is itself rebased

The no-fast-forward rule assumes the target is never rebased. A version branch kept as a linear
extension of another — `v0.3.0` onto `v0.2.3` as of 2026-08-08 — breaks that assumption, and the
rule fails in a way that is easy to miss.

**`git rebase` drops merge commits.** So a `--no-ff` bubble on a rebased line is destroyed by the
next sync, and the red work-in-progress it was protecting the spine from is replayed *onto the
spine*. The bubble buys nothing and the invariant is silently lost.

On such a line, land feature branches as **one green commit**, by squash or by keeping the branch
to a single commit, then fast-forward:

```
git rebase origin/<line> && <gate> && git checkout <line> && git merge --squash <branch>
```

This keeps what the no-ff rule actually protects — no red commit ever on the spine — while staying
linear and rebase-stable. The cost is the per-increment commits, which is the price of a rebased
line and is why only lines that need linearity should be rebased.

**A bare `--ff-only` is correct only when every commit on the branch is green.** It is not
equivalent to the above; it puts each of the branch's commits on the spine individually, so a red
intermediate lands on the target.

### Note on `scripts/merge-to-trunk.sh`

The script implements the **linear** form: rebase onto `origin/$TRUNK`, gate, re-check the tip, then
`git merge --ff-only`. That is correct for a rebased line whose commits are all green, and it is
**not** the `--no-ff` behaviour this section prescribes for an ordinary version branch.

The script and this document have differed since both existed. No harm has resulted, because merges
into `v0.2.3` have all been done by hand with `--no-ff`, and the script's users have been on the
linear line. Know which form you want before running it: on a `--no-ff` line it will flatten a
branch onto the spine without saying so.

## Definition of Green

Remote continuous integration establishes the version-branch green requirement. Local checks provide confidence before pushing and may target the affected scope. Full local gates are mandatory before publication or merging into `main`, including complete gates for detached packages. A stale complete-gate record means that complete verification is not current. It does not alone establish a test failure or block version-branch integration supported by green remote checks and relevant local evidence.

**Remote continuous integration gates version-branch merges. Full local gates are required before publication and merges into `main`, and are available for offline work.** Changed
2026-08-11, because **gate time is the project's bottleneck** and two sessions were serialising on
one machine.

Open a **draft pull request** from the feature branch to the version branch. `pull_request:
branches: [main, 'v*']` triggers the full matrix, so the branch is verified on hosted runners.
**Merge on CI green**, at the commit CI ran, without rebasing.

| | local `release-gate.sh` | CI |
|---|---|---|
| wall clock | ~2h30m | **~61 min** (22 jobs, one of them the critical path) |
| contends for the shared machine | **yes, exclusively** | no |
| two sessions at once | impossible | **yes** |
| coverage | 12 steps | **all 12, plus 10 more** |

**CI IS A VERIFIED STRICT SUPERSET, checked step by step rather than asserted.** Every local step has
a CI job — including `keleusma-wire` in *both* configurations, which one job covers in two `run`
lines. CI additionally runs Miri, two MSRV checks, `no_std`, the RTOS `thumbv8m` cross-build,
`keleusma-bench`, the SDL3 examples, the LSP, the VS Code extension and the WASM playground, none of
which the local gate touches. The local gate was always the weaker instrument; it was merely the
nearer one.

**The wall-clock figure was measured on 2026-09-08 and had been understated.** This row read
"~48 min (23 parallel jobs)". Five completed runs on `v0.2.3` that day took **57, 60, 61, 66 and 71
minutes** end to end, including queue time, which is the interval that matters because it is how long
until you know. Median 61.

**The run is not bounded by its parallelism; it is bounded by ONE job.** In a representative run
`Test (self-host feature)` took **59 minutes** and the next longest 44, across 22 jobs. Adding runners
cannot help. **The only way to shorten CI materially is to split that job**, which is a project-level
call and is recorded here rather than adopted.

**The conclusion is unchanged and the correction is small.** Sixty-one minutes against two and a half
hours is still roughly two and a half times faster, and CI still costs no time on the contended
machine, so the 2026-08-11 decision stands on the same reasoning with an honest number. Five samples
from one day, under whatever load the runners had.

**The obvious objection inverts.** `perf_canary` on a shared runner is noisier than on a quiet
desktop — but a CI false trip costs a 48-minute re-run that consumes **no local time at all**, while
a local false trip burns 2h30m of the contended resource. The expensive failure is the local one.

**What the local gate is still for**: a pre-publication run (with `--miri`), and working without a
network. It remains the recommended mirror when you want one pass locally rather than several red CI
jobs — but it is no longer what a merge waits on.

**A red CI result on the version branch or `main` is still remedied immediately**, as the top
priority for that branch, before further increments land. CI was always the final word; it is now
also the first.

## Lifespan

Feature branches should be short-lived — ideally under 24 hours. Long-lived branches accumulate merge
conflicts and diverge from the version branch in ways that are difficult to reconcile. The
self-hosted `compiler/` pipeline is internally lockstep, so its increments are one serial stream;
parallelize only across disjoint construct areas or the independent crates.

## Parallel-Agent Development

More than one agent or human may work concurrently. Each gets an isolated git worktree on its own
feature branch cut from the active version branch, so working trees never collide, and the version
branch and the full gate are entered one branch at a time. The worktree helper is
[`scripts/worktree.sh`](../../scripts/worktree.sh); the isolation, per-branch communication,
merge-serialization, and gate-discipline rules are in
[`PARALLEL_DEVELOPMENT.md`](./PARALLEL_DEVELOPMENT.md).

## Commit Conventions

### Format

```
<scope>: <imperative summary>

Optional body providing additional context.

Co-Authored-By: Claude <noreply@anthropic.com>
```

### Summary Line

Write the summary in imperative mood ("add type checker", not "added" or "adds"). Keep it under 72
characters. Use the same scopes as branch naming: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`.

### When to Commit

Commit after completing a prompted request. Each commit should represent one logical change. Avoid
combining unrelated changes in a single commit. The AI agent commits once after all tasks in a prompt
are complete, including the `REVERSE_PROMPT.md` update.

The operator resolved the previously recorded commit-frequency conflict on 2026-09-28. The once-per-prompt rule applies, and red commits are permitted only on feature branches. Attribute assisted commits to the assistant that contributed, as specified in `CLAUDE.md`.

## Pre-Push Checklist

Before pushing, verify:

- Relevant targeted tests, lint and formatting checks provide high confidence in a green remote result
- The receipt identifies checks run and coverage omitted
- Commit messages follow the conventions above
- No secrets, credentials, or sensitive data are included in the commit

The push itself runs the cargo-husky pre-push hook (the default-feature workspace tests, fmt, clippy,
doc, markdown links). Per the test tiers (process audit item 1), that hook runs the **routine `quick`
tier**, which excludes the ~198 self-hosted byte-identity tests (the `selfhost_*` binaries); it also
does **not** exercise the `--no-default-features`/`signatures` feature matrix, and it does **not** run
the detached `compiler/` subproject. The hook does not establish complete integration coverage. The full self-host suite, feature matrix and compiler subproject are covered by continuous integration. Detached native-code-generation checks remain separate.

## Integration and release checkpoints

Before merging into a version branch, require green remote continuous integration on the pull request. Select local checks by affected scope and the confidence needed for that remote result. Native-only changes need relevant native checks because root continuous integration does not exercise the detached package. Both complete native configurations are not mandatory for every version-branch integration.

Before publication or merging into `main`, full gates must pass against the candidate inputs. Run `scripts/release-gate.sh`, with `--miri` for publication, and the complete gates for affected detached packages. For the native backend, that includes both float configurations with current clean, frozen, passing records. Remote green remains required. Publication requires explicit in-session confirmation.
