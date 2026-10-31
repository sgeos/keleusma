# Absorption 123

> **Navigation** [Decisions](./README.md) | [Absorption 122](./ABSORPTION_122_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `6545a7d6`, which is pull request 665 merged into
`v0.3.0`, and upstream `5ea94b3a` on 2026-10-30, with 14 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-123`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **662** splits the three longest self-host tests per stage and the target
  descriptor grid into one test binary per word width.
- **663** records the operator's consolidated rulings of 2026-10-30, given in
  V0.2.X's session, their outcomes and the decisions still open there, and
  adds `ARRAY_WIRE_SLOT_DESIGN.md` and `RING_BUFFER_DESIGN.md`.

No file under `src/` or `native_codegen/` changed upstream.

## Merge and effect

`docs/process/REVERSE_PROMPT.md` conflicted and was rebuilt by
`tmp/rebuild_rp.sh` against `5ed0bdfd`, as upstream's 228 lines followed by
this line's 726. The interline checkpoint was regenerated at 168 messages,
and none of upstream's 164 identifiers is missing. `CLAUDE.md` merged
cleanly, takes upstream's re-measured test inventory, and keeps this line's
routing paragraph. `corpus_fingerprint` passed with no digest moved.

Two of the relayed rulings bear on this line. D3, branch protection, is
already applied to `v0.3.0`. C5 requires an independent review of the
self-hosted validator before V0.3.0. This line holds it only as relayed, so
it is question 27 for the operator to confirm.

## Verification

Made with `RUSTC_WRAPPER=` on this absorption.

| Check | Result |
|---|---|
| Native `corpus_fingerprint` | passed, no digest moved |
| Native documentation readers | passed |
| Root `documentation_links` and `comment_citations` | passed |

The full native suites were not rerun, because nothing they compile changed.
The hosted run covers the root workspace, including the split tests.
