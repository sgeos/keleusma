# Release Process

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

How to cut and publish a Keleusma workspace release. Keleusma's value proposition
is *definitive* bounds; the release discipline is held to the same standard —
**nothing ships red, and nothing is published that the registry cannot resolve.**

This document is the authoritative procedure. The everyday gate
(`cargo test && cargo clippy`) is *not* sufficient for a release; run the full gate
(`scripts/release-gate.sh`) and follow the steps below in order.

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

## Checklist

```
[ ] 0. Clean tree on the release branch; healthy stable toolchain.
[ ] 1. Bump versions and stamp changelogs.
[ ] 2. Full verification gate is GREEN (scripts/release-gate.sh, incl. cargo doc).
[ ] 3. Registry-publishability check (dry-run in dependency order).
[ ] 4. Operational-security scrub + tarball-contents check.
[ ] 5. External release audit returns a clear GO. Hold until green.
[ ] 6. Commit, advance branches, annotate the tag.
[ ] 7. Publish in dependency order.
[ ] 8. Push branches and the tag; prune merged branches.
[ ] 9. Post-publish verification (crates.io, docs.rs).
```

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

## 2. The verification gate — must be fully green

Run the whole gate; do not hand-pick a subset:

```sh
scripts/release-gate.sh          # fmt, clippy, tests, DOC, doc-links
scripts/release-gate.sh --miri   # add Miri (nightly, Tree Borrows) for a release
```

It mirrors `.github/workflows/ci.yml`, so a green run here means CI will be green.
It runs, at minimum: `cargo fmt --check`; `cargo clippy --workspace --all-targets
-D warnings`; the test matrix (default, `--no-default-features`, `signatures`,
`signatures,shell`); the relative-Markdown-link check; and — the step whose absence
let a red CI ship with V0.2.1 — **`cargo doc --no-deps` under `RUSTDOCFLAGS='-D
warnings'` for every crate**, which turns a broken or private intra-doc link into an
error. For a release, also run `--miri` (Tree Borrows; the project's Miri runs
require `-Zmiri-tree-borrows` because rkyv archive validation trips Stacked Borrows).

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
# downstream crates is therefore the real publish in step 7, one at a time.
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

## 5. External release audit

A release audit must return a clear **GO** — no open critical, high, or medium
finding on the safe load path, and the verifier scope stated honestly (a pass of
documented scope, not a complete verifiable kernel). Address findings and re-audit
until green. **Hold publication until the audit is green.** The empirical audit
should run the executed gate (build, test, clippy, and Miri) plus, per this
document, the `cargo doc` gate.

## 6. Commit, advance branches, tag

- Commit the version and changelog changes.
- Ensure `main` contains the release commit (advance it; it is what CI and the
  crate-version badge track).
- Create an **annotated** tag `vX.Y.Z` on the release commit, matching the existing
  convention (`v0.1.0`, `v0.2.0`, `v0.2.1` are all annotated).
- Note the branch/tag name collision: with a release branch named `vX.Y.Z`, push the
  tag with an explicit refspec — `git push origin refs/tags/vX.Y.Z` — so git does not
  refuse the ambiguous ref.

## 7. Publish in dependency order

Publish one crate at a time, waiting for each to be available before its dependents.
Modern `cargo publish` (1.66+) waits for registry availability before returning.

```sh
cargo publish -p keleusma-macros
cargo publish -p keleusma-arena     # only if bumped; skip if unchanged and already published
cargo publish -p keleusma
cargo publish -p keleusma-bench
cargo publish -p keleusma-cli
```

A crate version, once published, is **immutable** — it can be yanked, never
replaced. Do not publish until the audit is green and the dry-runs pass.

## 8. Push and finalize

- Push the release branch and `main`. The pre-push hook runs `cargo doc`; if it is
  bypassed with `--no-verify` (for example because the local toolchain is
  mid-repair), you **must** have run the gate manually first (step 2).
- Push the tag with the explicit refspec (step 6).
- Prune the merged release branch (the prior release exists only as a tag; the branch
  is retired after tagging). Delete the remote branch with an explicit `refs/heads/`
  refspec so the same-named tag is preserved:
  `git push origin :refs/heads/vX.Y.Z`.

## 9. Post-publish verification

- Confirm crates.io shows every published version.
- Confirm docs.rs built the docs (docs.rs does **not** use `-D warnings`, so it
  tolerates the intra-doc-link warnings the CI Doc job rejects; the gate in step 2 is
  what keeps those from existing in the first place).

## Hard-won lessons (the "do not repeat these")

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
- **Hold publication for the green audit.** Publishing is irreversible.
