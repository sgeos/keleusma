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
