# Absorption 146

> **Navigation** [Decisions](./README.md) | [Absorption 145](./ABSORPTION_145_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `bde314f6`, which is pull request 757 merged into
`v0.3.0`, and upstream `8657a85a` on 2026-11-08, with 0 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-146`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **759** and **760** are the proofs line's mechanisation of Part I of the
  composite region reuse proof in the Rocq Prover, under
  `docs/proofs/rocq/region_reuse/`, with a check script that refuses any
  axiom or admitted lemma, and an audit record.
- **758** is Kaizen's note on generational proof language.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, nothing
outside `docs/` changed, and the interline checkpoint is not touched.

**What it means for the native backend.** The mechanisation checks the theory
over an abstract machine, and its README states first that it does not check
that Keleusma is that machine, since the reuse theorems take a flow model as a
certificate supplied by the caller. The native backend's region plan, which
gives every call site a disjoint block and is checked by `region_nonreuse.rs`,
is therefore neither confirmed nor refuted by it. Nothing here asks the
backend to change.

## Verification

Made with `RUSTC_WRAPPER=` on the complete records. The change is
documentation only, and the Rocq check was not run here.

| Check | Result |
|---|---|
| Root `documentation_links`, `comment_citations` and `state_file_budget` | 10 passed |
| Native `handoff_figures` | 6 passed |

The hosted run on the pull request covers the rest.
