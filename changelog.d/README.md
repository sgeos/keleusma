# Changelog Fragments

Each user-visible change adds one file here instead of editing `CHANGELOG.md`. No two
changes write the same file, so merging one pull request no longer makes the others
conflict in the changelog. At release, `scripts/changelog_fragments.py assemble` moves
every fragment into the `[Unreleased]` section of `CHANGELOG.md` and deletes it.

## Writing a fragment

Name the file `YYYY-MM-DD-<slug>.<section>.md`.

- The date is the day the entry is written. It orders the entries, newest first.
- The slug is lower case words joined by hyphens, such as `verify-backward-targets`.
- The section is one of `added`, `changed`, `deprecated`, `removed`, `fixed` or
  `security`, the headings of [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

The body is one or more Markdown list items, written as they should appear in the
changelog, with no headings. For example, `2026-10-29-verify-backward-targets.fixed.md`
holds the following.

```markdown
- **The module verifier entries terminate on an unverified module.** They now apply
  the forward-target check and refuse such a module. Pinned by
  `tests/verify_hostile_termination.rs`.
```

A change that touches verifier soundness names the soundness invariant it preserves or
relaxes in its fragment, as `CONTRIBUTING.md` requires of every changelog entry.

## Checking

```sh
python3 scripts/changelog_fragments.py check     # every file here is a valid fragment
python3 scripts/changelog_fragments.py preview   # the [Unreleased] section as assembled
```

Continuous integration runs `check` in the scripts job. It fails on any file in this
directory that is not a fragment or this README, so a misnamed entry cannot be left out
of a release unnoticed.
