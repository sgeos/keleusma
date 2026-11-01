# Absorption 111

> **Navigation** [Decisions](./README.md) | [Absorption 110](./ABSORPTION_110_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `c501b78a`, which is absorption 110 on
`feat/v3-absorption-110` as pull request 579, and upstream `0b434c2d` on
2026-10-27, with 5 unabsorbed commits measured after fetching. The merge base
is `a5e3a290`. The separate conflict command
`git merge-tree --write-tree --name-only c501b78a origin/v0.2.3` exited one
and named `docs/process/REVERSE_PROMPT.md` as its only conflicting file. The
commits are V0.2.X pull requests 577 and 578. They add
`keleusma-cli/tests/self_hosted_full_language.rs`, which runs the shipping
command over every Identical census row, V0.2.X's
[Order 6 completion audit](./ORDER6_COMPLETION_AUDIT.md), and V0.2.X's
handoff with a mailbox checkpoint. The merge was prepared on
`feat/v3-absorption-111`, cut from absorption 110, with
`--no-ff --no-commit`. This absorption is not on `v0.3.0` until pull request
579 merges first.

## Merge

The reverse prompt was rebuilt by the ignored helper `tmp/rebuild_rp.sh`. The
merged file is upstream's new 236 lines followed by this line's unchanged
717-line block, and both parts were compared byte for byte with their
sources. `docs/process/interline/checkpoint.json` merged without a conflict,
and it was regenerated from the mailbox store anyway. Both sides held the same
119 identifiers, and the regenerated file holds all of them. `CLAUDE.md` keeps
this line's routing paragraph. The merged `compiler/`, `src/` and `tests/`
equal upstream's except for `tests/gate_ci_correspondence.rs`, whose
difference predates this absorption.

## Effect on this line

No stage source moved, and no digest was re-pinned.

**V0.2.X's Order 6 audit bears on this line's Order 6.** It records V0.2.X's
Order 6 as not complete. Every gate condition is met except five forms, the
information-flow labels and the newtype, which wait on V0.2.X's operator
question 10. The audit states that its gate is V0.2.X's own reading and has
not been ruled on by the operator. This line's
[Order 6 record](./ORDER_6_COMPLETION.md) lists V0.2.X's Order 6 as an unmet
dependency, and now cites the audit. Question 18 was added to
[the operator question list](../process/handoffs/v0.3.0-QUESTIONS.md). It asks
whether that dependency still stands, given that native code consumes the
reference compiler's bytecode. This line assumes meanwhile that it stands.
The audit's figures are V0.2.X's measurement and were not re-measured here
beyond the tests in the table below.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt was written, on a tree that did not change during the run. Logs are
under ignored `tmp/abs-111/`. Upstream added
`keleusma-cli/tests/self_hosted_full_language.rs`, so it was run in addition
to the usual set.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 41 passed |
| `keleusma-cli` `self_hosted_full_language`, debug profile | 1 passed |

Every pinned native figure held in both feature sets, and no count changed
except the new command-line test. That test is one function over the census
rows. How many rows it reached was not counted here, and the audit's own
figure is V0.2.X's. This table was added after the run. The tests that read
this record were then rerun, `prediction_stamp`, `outstanding_reports`,
`handoff_figures` and the native and root `comment_citations`, with the root
`documentation_links`. The root workspace suite and the heavy self-host tier
were not run locally. The hosted run on the pull request is the integration
check.
