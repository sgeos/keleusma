# Changelog fragments

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

Prepared on 2026-10-29 by the kaizen line. Facts name their source. Inferences and beliefs
that were not tested are marked as such.

## Origin and ruling

The V0.2.X line sent interline request 143 on 2026-10-30 by its clock. It is peer data,
not operator authority. It reported that every pull request into `v0.2.3` adds its entry
at the top of the same `[Unreleased]` section of `CHANGELOG.md`, so each merge makes every
other open pull request conflict there, and each resolution is a push that queues another
full continuous integration run. It named pull requests 601, 605 twice and 618, and 18
queued runs against 2 running. The kaizen line verified the shared insertion point in the
file. It did not verify the conflict history or the run counts.

V0.2.X proposed `CHANGELOG.md merge=union` in `.gitattributes`. The kaizen line advised
against it. Its belief, not tested on this repository, is that GitHub's own mergeability
check does not apply merge drivers named in `.gitattributes`, so the conflict banner and
the resolution pushes would remain, and only local merges would change. A union merge can
also duplicate a line when both sides edit the same entry.

The operator ruled in the kaizen session on 2026-10-29, choosing fragments in these words.

> 1. (a) Use fragments, which I would build on its own branch.

## Design

- A user-visible change adds one file, `changelog.d/YYYY-MM-DD-<slug>.<section>.md`, and
  does not edit `CHANGELOG.md`. The section is a Keep a Changelog heading in lower case.
  The body is one or more list items with no headings.
- `scripts/changelog_fragments.py check` validates every file in the directory and fails
  on any file that is not a fragment or the README. The continuous integration scripts
  job runs it. `preview` prints the assembled `[Unreleased]` section.
- `assemble` runs at release, as step 1 of `RELEASE_PROCESS.md`. It inserts each fragment
  at the top of the first heading of its section in `[Unreleased]`, newest date first and
  then by slug, creates a missing heading at the end of the block, writes the file
  atomically and deletes the fragments it used. A malformed fragment stops it before
  anything is written. With no fragments the file is unchanged byte for byte.
- `changelog.d/` is excluded from the crate tarball. Only the root changelog uses
  fragments. The six crate changelogs were last edited between 2026-07-08 and 2026-09-03
  on `v0.2.3` and do not show the conflict.
- `CLAUDE.md`, `CONTRIBUTING.md`, the pull request template and the header of
  `CHANGELOG.md` say where entries now go.

## Verification

- 23 unit tests in `scripts/test_changelog_fragments.py` cover names, bodies, ordering,
  insertion, missing and repeated headings, the fail-closed paths, preview and assembly.
  The whole `scripts/` suite ran 119 tests and passed.
- Assembling two sample fragments into a copy of the real changelog changed only the
  inserted lines.
- The guard tests that read documentation and process files passed, and the Markdown link
  check passed.

## Limits and transition

- Entries written as fragments are not visible in `CHANGELOG.md` until the release. The
  `preview` command shows them.
- Open pull requests that already edit `CHANGELOG.md` remain valid. Their entries stay
  where they are written. Only new entries use fragments.
- The rule is written down, not enforced. Nothing fails a pull request that edits
  `[Unreleased]` directly, because a release commit and a correction to a published entry
  must still edit the file. A conflict there is the symptom that the rule was missed.
- `v0.3.0` receives the directory and the rule when it next absorbs `v0.2.3`.

## The cost of version-branch push runs

V0.2.X also noted that branch runs are never cancelled, so a burst of merges queues one
full run per merge. The operator asked for a recommendation. The kaizen line recommends
keeping the policy. Its reasons are in the comment above `concurrency` in
`.github/workflows/ci.yml`. A version branch must be green at every commit, and the run
on each merge commit is what confirms that. A branch run whose tree a green pull request
run already tested is skipped as certified. Its inference is that fragments remove the
main source of extra runs, which is the resolution pushes on pull requests, and that the
branch-run cost should be measured again after fragments are in use.

The operator ruled in the kaizen session on 2026-10-29 in these words.

> Adopt 2 (a): keep branch runs uncancelled.

The workflow already implements that policy, so nothing in `ci.yml` changes.
