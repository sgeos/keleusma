# Absorption 104

> **Navigation** [Decisions](./README.md) | [Absorption 103](./ABSORPTION_103_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `ab0add66`, which is absorption 103 and became pull
request 561, merged into `v0.3.0` as `ff07b106` while this record was written,
and upstream `e58a3c02` on 2026-10-26, with 4 unabsorbed commits measured
after fetching. The merge base is `c309e3d3`. The separate conflict command
`git merge-tree --write-tree --name-only ab0add66 origin/v0.2.3` exited zero
and produced tree `fe8b68b88fc25b10ddb7fea2270520fc36dbc958` with no
conflicting file. The commits are V0.2.X pull requests 559 and 560. They raise
the self-hosted stages' capacities. The per-chunk opcode capacity of the five
analysis stages rises from 2,048 to 8,192, the parser's field table from 640
to 1,024, a function's forest from 1,024 to 4,096 nodes, and a chunk's
constant pool from 64 to 1,024. The merge was prepared on
`feat/v3-absorption-104`, cut from absorption 103, with `--no-ff --no-commit`.

## Merge

No file conflicted. Upstream changed `docs/process/REVERSE_PROMPT.md`, and
the first 183 lines of the merged file were compared byte for byte with
upstream's file. This line's block follows them. `CLAUDE.md` keeps this
line's routing paragraph. The construct-support boundary counts pinned in
`tests/selfhost_codegen.rs` are not among the lines upstream changed.

Both upstream commits cite an operator ruling of 2026-10-26 that limits may
be raised. It reached this line through V0.2.X's commits, not from the
operator directly. The raise is V0.2.X's change, and this line only absorbs
it.

## Effect on this line

Eight stage sources moved, `analyze.kel`, `codegen.kel`, `parse.kel`,
`reconstruct.kel`, `verify_depth.kel`, `verify_structural.kel`,
`verify_typed.kel` and `verify_yield.kel`, and their digests in
`corpus_fingerprint.rs` were re-pinned.

**All eight are also native corpus subjects, and every pinned native figure
held.** Their arrays and walk budgets grew by up to four times, so moved
figures were expected. An exploratory run of the native default suite after
the re-pin, kept in ignored `tmp/abs-104-probe.log` and not counted as this
record's verification, passed 841 of 841. Its test list and its five
skip-marker lines match absorption 103's run, and the differential asserts
its own reach counts, so the subjects were reached as before. No native
source names the raised capacities. **This does not show that the native cost
of these stages is unchanged.** As in absorption 103, no test pins a cost or
instruction figure for a stage module by name, so a rise is neither
confirmed nor excluded.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-104/`. Upstream changed root
`tests/reconstruct_failure_modes.rs`, so it was run in addition to the usual
set.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 32 passed |
| Root `reconstruct_failure_modes` with `self-host` | 16 passed |

Every pinned native figure held in both feature sets, and no count changed.
This table was added after the run. The tests that read this record were then
rerun, `prediction_stamp`, `outstanding_reports`, `handoff_figures` and the
native and root `comment_citations`, with the root `documentation_links`. The
root workspace suite and the heavy self-host tier, which upstream reports
rose from about 60 to 167 seconds per validator and trap corpus test, were
not run locally. The hosted run on the pull request is the integration check.
