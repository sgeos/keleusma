# Release Process

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

How to cut and publish a Keleusma workspace release. Keleusma's value proposition is
*definitive* bounds for mission-critical and rad-hard targets; the release discipline is
held to the same standard. **A publish is an irreversible mission event.** This procedure
is therefore run as a mission-assurance checklist, not a task list: a sequence of
**Go/No-Go gates** converging on a single, clearly marked **point of no return**, with the
irreversible action gated on independent human authorization and backed by a defined abort
and rollback path.

The governing invariants: **nothing ships red; nothing is published that the registry
cannot resolve; nothing but the exact CI-proven commit is published; and nothing is
published without an explicit, current go-ahead.**

This document is the authoritative procedure. The everyday gate
(`cargo test && cargo clippy`) is *not* sufficient for a release; run the full gate
(`scripts/release-gate.sh`) and follow the steps below in order. **No gate may be skipped,
waived, reordered, or deferred; a single NO-GO halts the sequence until it is cleared.**

## Release doctrine

Five rules govern every release. They bind the human maintainer and any AI agent equally.

1. **Go/No-Go, not best-effort.** Each numbered step is a gate with a binary verdict —
   GO or NO-GO. Any NO-GO halts the sequence; you do not proceed "mostly green," "green
   except," or "green enough." Clear the condition, re-verify, then continue.

2. **The point of no return is the publish (step 9).** Steps 0–8 are entirely reversible:
   a commit can be amended, `main` reset, a tag deleted and re-cut, a branch renamed, a
   red CI run discarded — all at zero external cost. The publish cannot be undone: a
   crates.io version is immutable, yankable but never deleted or replaced. Cross the point
   of no return deliberately, never incidentally.

3. **Independent authorization; no self-certification.** The two mandatory gates — CI
   green (step 6) and authorization to publish (step 8) — are rendered by the human
   maintainer, not self-attested by the party that prepared the release. The preparer
   reports evidence (the CI run URL and conclusion, the version and commit SHA); the
   maintainer reads it back and issues the GO. Preparation and go-ahead are separated
   hands — the software analog of the two-person rule for an irreversible action.

4. **Configuration control.** Exactly one artifact is releasable: the annotated-tag commit
   SHA that CI proved green. Immediately before each `cargo publish`, confirm the working
   tree *is* that SHA — clean `git status`, and `git rev-parse HEAD` equal to
   `git rev-parse vX.Y.Z^{commit}`. Never publish a dirty tree, an untagged commit, or a
   commit CI has not passed.

5. **Every release leaves a record.** Complete the Release Record (§11) — version, commit
   SHA, CI run URL, audit reference, authorizing person, and per-crate publish
   confirmation with timestamps — and commit it. The record is the auditable provenance of
   an irreversible act.

## The crates

Five crates publish to crates.io, in this dependency order:

1. `keleusma-macros` — proc-macro, no internal deps.
2. `keleusma-arena` — standalone allocator, no internal deps. **Versioned independently** (0.3.x line).
3. `keleusma` — the runtime; depends on `keleusma-macros` and `keleusma-arena`.
4. `keleusma-bench` — depends on `keleusma` and `keleusma-arena`.
5. `keleusma-cli` — the `keleusma` binary; depends on `keleusma` and `keleusma-arena`.

`keleusma`, `keleusma-cli`, `keleusma-bench`, and `keleusma-macros` track the
major-minor of `keleusma`. `keleusma-arena` has its own version and bumps only when
its public API changes.

## Preflight checklist

Run top to bottom. Each line is a gate; mark **GO** only when its condition is *fully*
met. A single **NO-GO** halts the sequence. Do not cross the point-of-no-return rule
until every preflight line above it is GO. The two `***` gates are human-confirmed and
may not be self-certified by the preparer (doctrine rule 3).

```
── PREFLIGHT · reversible · abort freely at any line ──────────────────────
[ ] 0. Clean tree on the release branch; healthy stable toolchain.
[ ] 1. Bump versions and stamp changelogs.
[ ] 2. Local pre-check gate is GREEN (scripts/release-gate.sh, incl. cargo doc).
[ ] 3. Registry-publishability check (dry-run in dependency order).
[ ] 4. Operational-security scrub + tarball-contents check.
[ ] 5. Commit; advance main; push the release commit AND the annotated tag.
[ ] 6. *** CI IS GREEN ON THE RELEASE COMMIT *** — human-confirmed (authoritative gate).
[ ] 7. External release audit returns a clear GO (scope to the change for a patch).
[ ] 8. *** EXPLICIT, CURRENT AUTHORIZATION TO PUBLISH THIS RELEASE *** — human-confirmed
       (a prior "expedite" or general go-ahead does not count).
[ ] 8a. CONFIG CONTROL: working tree == tagged, CI-green SHA (clean status; HEAD == tag).
╔═══════════════ POINT OF NO RETURN — the publish is irreversible ═══════════════╗
[ ] 9. Publish in dependency order.
╚════════════════════════════════════════════════════════════════════════════════╝
── POST · finalize ────────────────────────────────────────────────────────
[ ] 10. Cut the GitHub Release; prune branches; verify crates.io + docs.rs.
[ ] 11. Release Record completed and committed.
```

**Two gates prevent a repeat, and neither is self-certified.** *First, never publish, and
never merge to `main`, on a red CI run (step 6).* The local gate (step 2) is a fast
pre-check — it *cannot* replicate the cross-compilation jobs, the feature-combination
builds, or the latest-stable toolchain that CI runs, which is exactly where every V0.2.1
CI failure hid (a broken doc link, a 32-bit `Value`-layout assertion, a
`verify`-without-`floats` build, and stable-1.97 clippy lints). CI on the pushed release
commit (step 6) is the gate that catches them; publishing (step 9) is downstream of it,
not before it. *Second, never publish without an explicit, current, release-specific
go-ahead (step 8).* A crates.io version is irreversible, so the publish is not a step the
preparer takes on its own initiative: a prior directive to "expedite," a general blessing
given before the release was even cut, or a request about a different artifact is **not**
authorization to push it. Everything up to and including the tag and CI is reversible; the
publish is not, and it waits for the maintainer's word.

## 0. Prerequisites

- The working tree is clean and you are on the release branch.
- The stable toolchain is healthy. If `rustc --version` errors with *"the rustc
  binary … is not applicable"*, repair it before gating:
  `rustup component add rustc --toolchain stable`.

## 1. Versions and changelogs

- Bump `keleusma`, `keleusma-cli`, `keleusma-bench`, and `keleusma-macros` to the new
  `X.Y.Z`.
- Bump `keleusma-arena` **only if its public API changed** since its last *published*
  version. If it did, also bump the arena version requirement in every dependent
  (`keleusma`, `keleusma-cli`, `keleusma-bench`) to `">= new"` so a downstream
  resolution cannot select the older, incompatible arena. (This is the V0.2.1 arena
  0.3.0 → 0.3.1 lesson; see step 3.)
- Stamp each crate's `CHANGELOG.md`: rename the `[Unreleased]` section to
  `[X.Y.Z] - <YYYY-MM-DD>` and open a fresh empty `[Unreleased]` above it. The
  published tarball should carry a dated, stamped changelog, not `[Unreleased]`.
- Update the `**Status**` line in `CLAUDE.md`.

## 2. The local pre-check gate

Run the whole gate; do not hand-pick a subset:

```sh
scripts/release-gate.sh          # fmt, clippy, tests, DOC, doc-links
scripts/release-gate.sh --miri   # add Miri (nightly, Tree Borrows) for a release
```

It runs, at minimum: `cargo fmt --check`; `cargo clippy --workspace --all-targets
-D warnings`; the test matrix (default, `--no-default-features`, `signatures`,
`signatures,shell`); the relative-Markdown-link check; and — the step whose absence
let a red CI ship with V0.2.1 — **`cargo doc --no-deps` under `RUSTDOCFLAGS='-D
warnings'` for every crate**, which turns a broken or private intra-doc link into an
error. For a release, also run `--miri` (Tree Borrows; the project's Miri runs
require `-Zmiri-tree-borrows` because rkyv archive validation trips Stacked Borrows).

**This is a pre-check, not a substitute for CI.** It runs on your local host and
toolchain, so it *cannot* catch what only appears in CI, and every V0.2.1 CI failure
lived in that blind spot:

- **Cross-compilation jobs** — `no_std` (`thumbv7em-none-eabihf`) and the RTOS
  demonstrator (`thumbv8m.main-none-eabihf`). A 32-bit pointer width changed the
  `Value` layout and panicked a hardcoded `size_of == 32` assertion. The local gate
  builds only for the host.
- **Feature-combination builds** — e.g. `verify` without `floats`, which referenced
  `floats`-gated variants and failed to compile. The local gate does not enumerate
  every feature combination CI does.
- **The latest stable toolchain** — CI uses `dtolnay/rust-toolchain@stable` (the
  newest release), which can add lints (`unstable_name_collisions`, `question_mark`,
  redundant-`format!`-borrow) your local, possibly older, toolchain does not have.

Because of this, a green local gate is necessary but **not sufficient**; the
authoritative gate is CI on the pushed release commit (step 6).

**Why `cargo doc` is mandatory here.** `fmt`/`clippy`/`test` do not exercise
rustdoc. A broken intra-doc link is invisible to them and only fails the CI `Doc`
job (and the pre-push hook). Skipping `cargo doc` — or bypassing the pre-push hook
with `--no-verify` without running it manually — is exactly how V0.2.1 shipped with a
red CI Doc job. The gate script closes that hole.

## 3. Registry-publishability check — the path-dependency trap

The workspace builds against **local path dependencies**, which hides the case where
a crate uses API newer than a dependency's *published* version. The audit gate and
the local test gate both pass, yet `cargo publish` fails at the registry-resolved
verify build. Check this before publishing, in dependency order:

```sh
cargo publish -p keleusma-macros --dry-run
cargo publish -p keleusma-arena  --dry-run
# keleusma's dry-run resolves its deps from the registry, so it only succeeds after
# macros and arena are actually published. The definitive check for keleusma and the
# downstream crates is therefore the real publish in step 9, one at a time.
```

If a dependent needs API a published dependency lacks, return to step 1: bump that
dependency and the dependents' version requirements. (V0.2.1: `keleusma` used arena
methods absent from the published `keleusma-arena` 0.3.0, forcing an arena 0.3.1
release mid-publish.)

## 4. Operational-security scrub and tarball contents

- Confirm the shippable tree carries zero Tier 1 / Tier 2 vocabulary (per the
  security process): grep the tracked tree, paying special attention to
  `docs/reference/RELATED_WORK.md` and `docs/reference/GLOSSARY.md`. `secret/` and
  `tmp/` must be untracked/gitignored and excluded.
- Confirm the tarball contents with `cargo package -p <crate> --list --allow-dirty`:
  the `exclude` list keeps out internal docs (`docs/decisions/`, `docs/process/`,
  `docs/roadmap/`), the book (`book/`), the self-hosting subproject (`compiler/`),
  and agent files (`CLAUDE.md`, `AGENTS.md`); user documentation still ships.

## 5. Commit, advance `main`, and push — *before* publishing

Publishing is downstream of CI, so the code and tag must be on `origin` and CI must
have run before step 9. Do the git work here:

- Commit the version and changelog changes.
- Ensure `main` contains the release commit (advance it; it is what CI and the
  crate-version badge track).
- Create an **annotated** tag `vX.Y.Z` on the release commit, matching the existing
  convention (`v0.1.0`, `v0.2.0`, `v0.2.1` are all annotated).
- **Push the release commit, `main`, and the tag now.** With a release branch named
  `vX.Y.Z`, push the tag with an explicit refspec — `git push origin refs/tags/vX.Y.Z`
  — so git does not refuse the ambiguous ref. The pre-push hook runs `cargo doc`; if
  bypassed with `--no-verify` (e.g. a local toolchain mid-repair), you must have run
  the gate (step 2) manually first.

## 6. CI green on the release commit — the mandatory gate

Wait for the CI run on the pushed release commit and confirm **every job is green**:

```sh
gh run list --branch main --limit 1
gh run view <run-id> --json conclusion,jobs \
  -q '.conclusion, [.jobs[] | select(.conclusion=="failure").name]'
```

If any job is red, **stop** — fix it, push, and re-confirm. This is the gate the local
pre-check cannot be: it covers the cross-compilation jobs, the feature-combination
builds, the latest-stable toolchain, and the doc build. Publishing through a red CI
run is the exact mistake that shipped with V0.2.1. Never do it.

**This gate is human-confirmed (doctrine rule 3).** The preparer reports the CI run URL
and its `conclusion`; the maintainer reads it back and renders the GO. A party that both
ran the pipeline and declared it green has self-certified — precisely the coupling this
rule forbids. Capture the run URL for the Release Record (§11).

## 7. External release audit

A release audit must return a clear **GO** — no open critical, high, or medium
finding on the safe load path, and the verifier scope stated honestly (a pass of
documented scope, not a complete verifiable kernel). Address findings and re-audit
until satisfied. For a small patch on a low-traction line, the review may be **scoped
to the delta** since the last audited release rather than a full re-audit, at the
maintainer's discretion; a functional bugfix with green CI (step 6) and a clean gate
(step 2) may proceed on that basis. Hold publication until the review is satisfied.

## 8. Explicit authorization to publish — the irreversible-action gate

Everything through step 7 is reversible. A commit can be amended, `main` can be reset,
the tag can be deleted and re-cut, a branch can be renamed back, and a red CI run costs
nothing. **Publishing (step 9) is the one action that cannot be undone** — a crates.io
version is immutable; it can be yanked but never deleted or replaced. Because of that
asymmetry, the publish is gated on an explicit, current, release-specific go-ahead from
the maintainer, and the agent does not cross this line on its own initiative.

**What does *not* authorize a publish:**

- An earlier directive to "expedite," "get it ready," "proceed with the release," or
  similar, given *before* the release was cut and CI was green. Intent to move quickly
  is not consent to the irreversible step.
- A narrowly-scoped request about a *different* artifact — e.g. "cut the GitHub Release
  for the already-published `vX.Y.Z`." Do exactly the thing asked; do not widen it into
  a new publish.
- Authorization granted for a *prior* release. Each publish is its own gate.

**What does authorize it:** a clear, current instruction to publish *this* version —
"publish 0.2.3," "go ahead and publish," "ship it." The maintainer issues the GO after an
explicit **readback of the version and commit SHA** to be published (doctrine rule 3);
the preparer does not infer authorization from tone, momentum, or a prior message. If
there is any doubt, **stop at the reversible boundary** (tag pushed, CI green, dry-runs
clean) and ask. The reversible preparation in steps 0–7 may run freely and without
prompting; the irreversible step waits for the word.

> V0.2.2 lesson: an agent treated an earlier "V0.2.2 should be expedited" as standing
> authorization and published all four crates, when the actual in-session request was
> only to cut the *V0.2.1* GitHub Release. The release content was sound and CI-green,
> but the consent was never given. Soundness is not a substitute for authorization.

## 9. Publish in dependency order

Only after CI is green (step 6), the review is satisfied (step 7), **and the publish is
explicitly authorized (step 8)**. This is the point of no return.

**Configuration-control precheck (doctrine rule 4).** Immediately before the first
publish, prove you are shipping the CI-green artifact and nothing else:

```sh
git status --porcelain                                        # must print nothing
[ "$(git rev-parse HEAD)" = "$(git rev-parse vX.Y.Z^{commit})" ] && echo "HEAD == tag: OK"
```

Publish only if both hold. A dirty tree or a HEAD that has drifted from the tag means the
CI-green proof no longer covers what you are about to upload — that is NO-GO.

Publish one crate at a time, waiting for each to be available before its dependents;
modern `cargo publish` (1.66+) waits for registry availability before returning. **Skip
any crate whose source is unchanged since its last published version** (a version-only
bump does not require a re-publish, but a dependent may then keep the older dependency
version — see step 1).

```sh
cargo publish -p keleusma-macros    # skip if unchanged at this version
cargo publish -p keleusma-arena     # only if bumped; skip if unchanged and already published
cargo publish -p keleusma
cargo publish -p keleusma-bench
cargo publish -p keleusma-cli
```

A crate version, once published, is **immutable** — it can be yanked, never replaced.

## 10. Finalize and post-publish verification

- **Cut the GitHub Release from the tag.** A git tag is *not* a GitHub Release; the tag
  alone shows up under Tags but not under Releases. Create the Release with the
  version's changelog section as the notes, matching the `Keleusma VX.Y.Z` title
  convention:

  ```sh
  # extract this version's changelog section as the notes
  awk '/^## \[X.Y.Z\]/{f=1;next} f&&/^## \[/{exit} f' CHANGELOG.md > /tmp/notes.md
  gh release create vX.Y.Z --title "Keleusma VX.Y.Z" --notes-file /tmp/notes.md --verify-tag
  gh release list   # confirm it exists and is marked Latest
  ```

  (V0.2.1 was published to crates.io and tagged but had no GitHub Release until it was
  cut retroactively; this step closes that gap.)
- Prune the merged release branch (the prior release exists only as a tag; the branch
  is retired after tagging). Delete the remote branch with an explicit `refs/heads/`
  refspec so the same-named tag is preserved: `git push origin :refs/heads/vX.Y.Z`.
- Confirm crates.io shows every published version.
- Confirm docs.rs built the docs (docs.rs does **not** use `-D warnings`, so it
  tolerates the intra-doc-link warnings the CI Doc job rejects; the gate in step 2 and
  CI in step 6 are what keep those from existing in the first place).

## 11. Release record

Doctrine rule 5: every release leaves auditable provenance. Record the following and
commit it — in the annotated tag message (durable and SHA-bound) and the GitHub Release
body, and append a one-line entry to a release log if one is maintained. Fill every
field; `n/a` is a valid value, blank is not.

```
Release:        Keleusma vX.Y.Z
Commit SHA:     <full 40-char SHA of the tagged release commit>
Crates:         keleusma X.Y.Z, keleusma-cli X.Y.Z, keleusma-bench X.Y.Z,
                keleusma-macros X.Y.Z, keleusma-arena A.B.C
CI run (green): <URL of the green CI run on the release commit>
Local gate:     scripts/release-gate.sh [--miri] — PASS on <toolchain> <YYYY-MM-DD>
Audit:          <report path / commit / "delta-scoped, maintainer discretion">
Authorized by:  <maintainer>  at  <UTC timestamp>
Published:      macros <ts>, arena <ts|skipped>, keleusma <ts>, bench <ts>, cli <ts>
GitHub Release: <URL>
Notes:          <anything the next releaser must know>
```

## Abort criteria — any one is NO-GO

Conditions that mandate a halt, to be checked continuously through the preflight. Any one
present before the point of no return means **do not proceed**:

- Any CI job red on the release commit (step 6).
- Publish authorization absent, stale, or scoped to a different artifact (step 8).
- Working tree dirty, or `HEAD` ≠ tag, at the configuration-control check (step 8a / 9).
- A dependency's published version lacks API the workspace uses — a dry-run fails (step 3).
- An open critical / high / medium audit finding on the safe load path (step 7).
- Tier 1 / Tier 2 vocabulary present in the shippable tree (step 4).
- The stable toolchain is unhealthy and was not repaired (step 0).
- **Any uncertainty about whether a gate is truly GO. Uncertainty is NO-GO.**

Aborting before step 9 costs nothing: reset, fix, and re-run the affected gates from the
earliest disturbed step. There is no penalty for holding; the only unrecoverable error is
crossing the point of no return without cause.

## Rollback and contingency — after the point of no return

A publish cannot be reversed, so this is the *containment* arm of the process, reached only
when the preflight has already failed to stop a defect. **It is not part of the normal
cadence.** Do not let its existence relax the gates: an operating procedure that treats
"ship, then yank" as routine quietly teaches that a green gate is optional — the moral
hazard this whole document exists to prevent. Roll forward by default; yank only when a
version is genuinely unfit.

A crates.io version is immutable. The two tools are **supersede** (publish a fixed higher
version) and **yank** (hide a version from *new* resolution; existing lockfiles still
resolve it). They are not interchangeable — supersede is the fix, yank is only containment
— and the yank is governed by a fitness test, not by the fact that a mistake occurred.

### When to yank, and when not to

Yank on *unfitness*, never on mere supersession or a cosmetically-red CI run. A version
that still builds and behaves correctly on its supported configurations stays published
even if a better one now exists; semver carries range dependents forward on its own.
Over-yanking also erodes the signal — if a yank comes to mean "merely old," it stops
meaning "broken or dangerous," and a wall of yanked versions reads as a troubled project.

| Condition of the published version | Action |
|---|---|
| Fails to build on a supported target; unsound; violates its WCET/WCMU contract; a security defect; or published entirely in error | **Supersede, then yank** |
| Merely superseded, or red on *cosmetic* CI (doc link, new lint) but builds and behaves correctly on supported configs | **Supersede only — do not yank** |
| Git surface wrong (tag or Release), crates fine | Delete and re-cut the tag/Release; **do not yank sound crates** |

V0.2.1 met the first row: it did not compile for the Cortex-M `no_std` targets, so it was
yanked once V0.2.2 was live. V0.1.0 likewise, for a wrong MSRV that 0.1.1 corrected. A
release that was merely *premature*, or red for a doc-link or lint reason but otherwise
correct, would be the second row — supersede and leave in place.

### Procedure

1. **Publish the compatible fix first (supersede).** Cut `X.Y.Z+1` through this entire
   procedure from step 0 — you cannot replace the bad version, only supersede it. The fix
   must be **semver-compatible** with the bad version so range dependents adopt it
   automatically; if it needs a breaking change, yanking the predecessor can strand
   `^`-dependents who cannot cross the boundary, and that case needs deliberate handling,
   not a reflexive yank. State in the fix's changelog what it supersedes and why (as V0.2.2
   did for V0.2.1's cross-target breakage).
2. **Then yank the defective version.** `cargo yank --version X.Y.Z <crate>` (the crate is
   a positional argument; there is no `-p` for yank). Publishing the fix first guarantees
   no range dependent is ever left with no installable version. The one exception is an
   *actively dangerous* defect — security, unsoundness, data corruption — with an older
   compatible fallback already published; there you may yank immediately to stop the
   bleeding, accept the fallback, then supersede. Yank is reversible with `cargo yank
   --undo` if issued in error.
3. **Scope the yank to the defective crate and version only.** Never yank an artifact a
   live good release still depends on. `keleusma-arena` 0.3.1 is the standing example: it
   shipped in the V0.2.1 cohort but is *also* required by V0.2.2, so it must not be yanked.
   Yank the crate that holds the defect, not the whole release cohort.
4. **Record it.** Note the yank, its reason, and its replacement in the changelog, the
   affected GitHub Release body, and the Release Record (§11), so a human landing on the
   yanked version is directed forward.

## Hard-won lessons (the "do not repeat these")

- **Never publish, and never merge to `main`, on a red CI run (step 6).** V0.2.1
  shipped with four red CI jobs because the local gate was treated as the gate; it is
  only a pre-check.
- **Never publish without explicit, current authorization (step 8).** The publish is
  irreversible, so it is the maintainer's call, not the agent's initiative. A prior
  "expedite," a general go-ahead given before the cut, authorization for a *previous*
  release, or a request about a *different* artifact are none of them consent to push
  *this* version. V0.2.2 was published on a stale "expedite" while the actual request
  was only to cut the V0.2.1 GitHub Release. When in doubt, stop at the reversible
  boundary (tag pushed, CI green, dry-runs clean) and ask.
- **The local gate cannot replace CI.** It runs on one host and one toolchain, so it
  misses the cross-compilation jobs (a 32-bit `Value`-layout assertion), the
  feature-combination builds (`verify` without `floats`), and the latest-stable
  toolchain (new clippy lints). Confirm green CI on the pushed release commit.
- **Run `cargo doc -D warnings` in the release gate.** It was absent from the manual
  gate and the pre-push hook was bypassed with `--no-verify`, so a broken-intra-doc-
  link CI failure shipped with V0.2.1. `scripts/release-gate.sh` includes it.
- **Do a registry-resolved dry-run before publishing.** Local path-dep builds hide a
  dependency whose published version lacks API the workspace now uses (the arena
  0.3.0 → 0.3.1 mid-publish scramble).
- **Bump a dependency's version *and* the dependents' requirements together** when the
  dependency's public API grows; otherwise the manifest claims a compatibility that
  does not exist.
- **Stamp changelogs at the cut**, not "later"; the published tarball is immutable.
- **Publish only the CI-green SHA (configuration control).** Re-check `HEAD == tag` and a
  clean tree at step 8a / step 9; a drifted or dirty tree voids the CI-green proof for
  what you are actually uploading.
- **No self-certification of the two hard gates.** The party that prepared the release
  does not also render its GO; the maintainer confirms CI-green (step 6) and authorization
  (step 8) from reported evidence and a version/SHA readback.
- **Publishing is irreversible — but not without recourse.** Gate it on green CI, a
  satisfied review, explicit authorization, and passing dry-runs — in that order — and if
  a defective version does ship, contain it with `cargo yank` and supersede it with a
  fixed patch (see *Rollback and contingency*).
