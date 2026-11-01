# Absorption 114

> **Navigation** [Decisions](./README.md) | [Absorption 113](./ABSORPTION_113_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `97f34b1e`, which is pull request 597 merged into
`v0.3.0`, and upstream `c6366172` on 2026-10-28, with 7 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-114`
with `--no-ff --no-commit`. The commits are Kaizen pull requests 589 and 595,
with pull request 590 carried forward through 595. They synchronize the
repository with its project template, adding a code of conduct, Dependabot
configuration, release notes configuration, code owners and issue template
settings, and they resolve tracked disclosures and propose a team setup in
Kaizen's records.

## Merge and effect

Two files conflicted. In `.gitignore` this line keeps its own comment and its
`*.py[cod]` entry and takes upstream's new entries for environment files, the
Codex directory and editor files. The interline checkpoint was regenerated
from the mailbox store. It holds 126 messages, and none of upstream's 124
identifiers is missing.

The only source change is a reworded documentation comment on
`kind::PROVENANCE` in `src/wire_schema.rs`. No code, test or stage file
changed, so no digest moved and no native figure could move.
`docs/process/REVERSE_PROMPT.md` is unchanged, and `CLAUDE.md` keeps this
line's routing paragraph.

## Verification

Because no code changed, the local check was targeted rather than the full
run, as the git workflow permits when the selected checks give high
confidence and their coverage is recorded. Run with `RUSTC_WRAPPER=` after
this receipt was written, and the documentation readers were run again after
its figures were filled in.

| Check | Result |
|---|---|
| Native `corpus_fingerprint` | 3 passed, no digest moved |
| Native `prediction_stamp`, `outstanding_reports`, `handoff_figures` and `comment_citations` | 1, 6, 6 and 8 passed |
| Root `documentation_links` and `comment_citations` | 2 and 6 passed |

The native suites, the root guards beyond these two and the root workspace
suite were not run locally. The hosted run on the pull request covers them.
