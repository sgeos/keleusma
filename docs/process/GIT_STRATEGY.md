# Git Strategy

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

Version control conventions for Keleusma.

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
- **Intermediate commits may be red.** A feature branch is a workspace. A session may take several
  commits, passing through red states, to converge on a working approach; only the branch **tip at
  merge** must be green. Nothing on the branch is load-bearing until it merges, so a session should
  commit freely to checkpoint work rather than hold a single large uncommitted change.
- **Abandoning the branch is an acceptable outcome, not a failure.** If an approach does not
  converge, discard the branch (delete it unmerged) and start a new one rather than force an unsound
  approach to green. The version branch is only ever touched by green merges, so a dead-end feature
  branch costs nothing but the branch itself. This is the worst case, and it is a normal one.
- **Must be all-green before merging back** into the version branch (a green
  `scripts/release-gate.sh`).
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
- A merge **proceeds once the local full gate (`scripts/release-gate.sh`) is green** — see
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

Two authorities, with a defined relationship:

- **Local** — the full pre-merge gate `scripts/release-gate.sh` passes (feature matrix, the whole
  self-host suite, the detached `compiler/` subproject, docs under `-D warnings`, clippy, fmt).
- **Remote** — the continuous integration run passes.

**A merge may proceed on a green local gate.** Continuous integration is the **binding authority**
after the push: a red CI result on the version branch or `main` is remedied **immediately**, as the
top priority for that branch, before further increments land. The local gate is the recommended
mirror of CI so a break is caught locally in one pass rather than across several red CI jobs; CI is
the final word.

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

## Pre-Push Checklist

Before pushing, verify:

- `cargo test` passes with no failures
- `cargo clippy -- -D warnings` produces zero warnings
- `cargo fmt --check` reports no formatting issues
- Commit messages follow the conventions above
- No secrets, credentials, or sensitive data are included in the commit

The push itself runs the cargo-husky pre-push hook (the default-feature workspace tests, fmt, clippy,
doc, markdown links). Per the test tiers (process audit item 1), that hook runs the **routine `quick`
tier**, which excludes the ~198 self-hosted byte-identity tests (the `selfhost_*` binaries); it also
does **not** exercise the `--no-default-features`/`signatures` feature matrix, and it does **not** run
the detached `compiler/` subproject. All three — the full self-host suite, the feature matrix, and
the subproject — live only in the pre-merge gate below. This makes the pre-merge gate the sole local
enforcement point for self-host regressions on the release line, so running it before a merge is not
optional.

## Pre-Merge Gate (mandatory)

Before merging a feature branch into the version branch (or the version branch into `main`), run the
full gate:

```
scripts/release-gate.sh
```

This is the recommended local mirror of CI: it runs the `--no-default-features` and
`signatures`/`signatures,shell` feature matrix **and** the detached `compiler/` subproject (`cd
compiler && cargo test`) — the same coverage CI provides. As of 2026-07-24 CI triggers on `main`
**and** any `v*` version branch and includes a `selfhost-compiler` subproject job, so **the version
branch is CI-gated**. Run the gate before the merge so a break is caught locally in one pass; CI is
the authoritative confirmation afterward, per [Definition of Green](#definition-of-green).
Historically the subproject was gated **nowhere**, which is how a stale decoder shipped `unknown op
tag 62` into `v0.2.3` (process audit item 4); that gap is now closed in both places.
