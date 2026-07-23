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

The push itself runs the cargo-husky pre-push hook (the full default-feature workspace, fmt, clippy, doc, markdown links). That hook does **not** exercise the `--no-default-features`/`signatures` feature matrix, and it does **not** run the detached `compiler/` subproject. Those live only in the pre-merge gate below.

## Pre-Merge Gate (mandatory)

Before merging a feature branch into the active release line, run the full gate:

```
scripts/release-gate.sh
```

This is **mandatory, not optional**. It is a superset of the pre-push hook: it runs the `--no-default-features` and `signatures`/`signatures,shell` feature matrix **and** the detached `compiler/` subproject (`cd compiler && cargo test`), neither of which the pre-push hook nor CI covers. Skipping it is how a break reaches the release line undetected — for example, a stale decoder in `compiler/src/selfhost.rs` shipped `unknown op tag 62` into `v0.2.3` because the subproject was gated nowhere (process audit, 2026-07-22). A merge whose `release-gate.sh` is not green does not proceed.

> **Known branching-model inconsistency (operator decision pending).** This document's "Trunk-Based Development" section states work merges into `main`, but current practice merges feature branches into the active `v0.2.x` release line, and `main` has diverged well behind it. CI triggers on `main` only, so the `v0.2.x` line is not CI-gated — which is *why* the pre-merge gate above is load-bearing. Reconciling the branching model (catch `main` up, or make CI track the release line) is an operator decision, flagged here so the gate's necessity is understood.
