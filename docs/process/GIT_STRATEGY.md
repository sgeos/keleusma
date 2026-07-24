# Git Strategy

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

Version control conventions for Keleusma.

## Trunk-Based Development

All work flows through short-lived feature branches that merge into main. This approach reduces merge conflicts, maintains a linear history readable for AI agents, encourages frequent integration, and promotes small atomic changes.

## Branch Strategy

### Main Branch

The main branch is the single source of truth. It must always compile and pass tests. Direct commits to main are acceptable for small documentation changes and process file updates.

### Feature Branches

Feature branches use the naming convention `<scope>/<short-description>`.

Supported scopes:

- `feat` for new features
- `fix` for bug fixes
- `docs` for documentation changes
- `refactor` for code restructuring
- `test` for test additions or modifications
- `chore` for maintenance tasks

Example branch names: `feat/type-checker`, `fix/parser-error-recovery`, `docs/api-guide`.

### Lifespan

Feature branches should not live longer than 24 hours. Long-lived branches accumulate merge conflicts and diverge from main in ways that are difficult to reconcile.

## Parallel-Agent Development

More than one agent or human may work concurrently. Each gets an isolated git
worktree on its own feature branch cut from the active trunk, so working trees
never collide, and the release line and the full gate are entered one branch at a
time. The worktree helper is [`scripts/worktree.sh`](../../scripts/worktree.sh); the
isolation, per-branch communication, merge-serialization, and gate-discipline
rules are in [`PARALLEL_DEVELOPMENT.md`](./PARALLEL_DEVELOPMENT.md). Note that the
self-hosted `compiler/` pipeline is internally lockstep and is not parallelizable
until the P11 encoding-capacity change lands.

## Linear History

Enforce rebase, not merge. Linear history keeps the commit log readable and makes bisecting straightforward. When merging a feature branch, rebase it onto main before completing the merge.

## Commit Conventions

### Format

```
<scope>: <imperative summary>

Optional body providing additional context.

[Task: <task-identifier>]
Co-Authored-By: Claude <noreply@anthropic.com>
```

### Summary Line

Write the summary in imperative mood ("add type checker", not "added type checker" or "adds type checker"). Keep it under 72 characters. Use the same scopes as branch naming: feat, fix, docs, refactor, test, chore.

### When to Commit

Commit after completing a prompted request. Each commit should represent one logical change. Avoid combining unrelated changes in a single commit. The AI agent commits once after all tasks in a prompt are complete, including the REVERSE_PROMPT.md update.

## Pre-Push Checklist

Before pushing to the remote repository, verify the following:

- `cargo test` passes with no failures
- `cargo clippy -- -D warnings` produces zero warnings
- `cargo fmt --check` reports no formatting issues
- Commit messages follow the conventions described above
- The branch is rebased onto the latest main
- No secrets, credentials, or sensitive data are included in the commit

The push itself runs the cargo-husky pre-push hook (the default-feature workspace tests, fmt, clippy, doc, markdown links). Per the test tiers (process audit item 1), that hook runs the **routine `quick` tier**, which excludes the ~198 self-hosted byte-identity tests (the `selfhost_*` binaries); it also does **not** exercise the `--no-default-features`/`signatures` feature matrix, and it does **not** run the detached `compiler/` subproject. All three — the full self-host suite, the feature matrix, and the subproject — live only in the pre-merge gate below. This makes the pre-merge gate the sole enforcement point for self-host regressions on the release line, so running it before a merge is not optional.

## Pre-Merge Gate (mandatory)

Before merging a feature branch into the active release line, run the full gate:

```
scripts/release-gate.sh
```

This is the **recommended local pre-push mirror of CI**: it runs the `--no-default-features` and `signatures`/`signatures,shell` feature matrix **and** the detached `compiler/` subproject (`cd compiler && cargo test`) — the same coverage CI now provides (as of 2026-07-24 CI triggers on the `v*` release line and includes a `selfhost-compiler` subproject job). Run it before pushing to the release line so a break is caught locally in one pass rather than across several red CI jobs; CI is the authoritative gate. Historically the subproject was gated **nowhere**, which is how a stale decoder shipped `unknown op tag 62` into `v0.2.3` (process audit item 4); that gap is now closed in both places.

> **Branching model (reconciled 2026-07-24).** Work merges into the active `v0.2.x`
> release line (not `main`, which has diverged behind it). CI now triggers on `main` **and**
> any `v*` version branch and includes the full feature matrix plus the detached `compiler/`
> subproject, so **the release line is CI-gated**. The document's older "merges into `main`"
> framing reflects the eventual trunk model; until `main` is caught up, the `v*` line is the
> CI-gated integration branch. (`main`'s own `ci.yml` should pick up the same `v*` trigger
> when it is next updated, so version branches cut from `main` are gated from the start.)
