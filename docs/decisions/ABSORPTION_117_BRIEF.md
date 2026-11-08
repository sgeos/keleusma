# Absorption 117

> **Navigation** [Decisions](./README.md) | [Absorption 116](./ABSORPTION_116_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `74af77c1`, which is pull request 627 merged into
`v0.3.0`, and upstream `751fa668` on 2026-10-30, with 9 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-117`
with `--no-ff --no-commit`. The commits are Kaizen pull requests 625, 626 and
624.

- Pull request 625 keeps changelog entries as fragment files under
  `changelog.d/`, on the operator's ruling of 2026-10-29 as Kaizen relays it.
  `CLAUDE.md` gains the rule, `scripts/changelog_fragments.py` checks and
  assembles the fragments, and the scripts job runs the check.
- Pull request 626 revises Kaizen's records of the blind labeller.
- Pull request 624 makes continuous integration pass `--locked` for the root
  and detached lockfiles, and excludes `changelog.d/` from the package.

## Merge and effect

Only the interline checkpoint conflicted. It was regenerated from the mailbox
store and holds 149 messages, and none of upstream's 148 identifiers is
missing. `docs/process/REVERSE_PROMPT.md` did not change upstream.
`CLAUDE.md` keeps this line's routing paragraph beside the new fragment rule.

No source, test or stage file changed, so no digest moved and no native
figure could move. This line adopts the fragment rule as Kaizen asked in
sequence 148. Native work in `native_codegen` is outside the published
`keleusma` crate and stays recorded in this line's archive, and a change of
this line that is visible to a `keleusma` user adds a fragment.

The archive's banner, its resume stamp and the state table's absorption row
were refreshed to this absorption. They had read absorption 95.

## Verification

Because no code changed, the local check was targeted, as the git workflow
permits when the selected checks give high confidence and their coverage is
recorded. Run with `RUSTC_WRAPPER=` and `--locked` after this receipt was
written.

| Check | Result |
|---|---|
| Native `corpus_fingerprint` | passed, no digest moved |
| Native `prediction_stamp`, `outstanding_reports`, `handoff_figures`, `comment_citations`, `native_workflow` and `reach_census` | passed |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| `scripts/changelog_fragments.py check` and `scripts/test_changelog_fragments.py` | passed, 23 script tests |

The native suites and the root workspace suite were not run locally. The
hosted run on the pull request covers them.
