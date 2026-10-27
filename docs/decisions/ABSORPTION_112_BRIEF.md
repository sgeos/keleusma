# Absorption 112

> **Navigation** [Decisions](./README.md) | [Absorption 111](./ABSORPTION_111_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `dd36f6d1`, which is pull request 580 merged into
`v0.3.0`, and upstream `22b61456` on 2026-10-27, with 3 unabsorbed commits
measured after fetching. The merge base is `0b434c2d`. The separate conflict
command `git merge-tree --write-tree --name-only dd36f6d1 origin/v0.2.3`
exited one and named `docs/process/REVERSE_PROMPT.md` as its only conflicting
file. The commits are V0.2.X pull request 581. The self-hosted toolchain now
erases information-flow labels, `classify`, `declassify` and newtypes from
the code it emits, as the reference does, and V0.2.X's Order 6 audit is
revised. The merge was prepared on `feat/v3-absorption-112` with
`--no-ff --no-commit`.

## Merge

The reverse prompt was rebuilt by the ignored helper `tmp/rebuild_rp.sh`. The
merged file is upstream's new 246 lines followed by this line's unchanged
717-line block, and both parts were compared byte for byte with their
sources. `CLAUDE.md` merged without a conflict. Upstream's change there
records V0.2.X's Order 6 as complete pending acceptance, and this line's
routing paragraph is kept. The merged `compiler/`, `src/` and `tests/` equal
upstream's except for `tests/gate_ci_correspondence.rs`, whose difference
predates this absorption.

## V0.2.X's Order 6 and this line's

**V0.2.X's revised audit records its Order 6 as complete for its first-pass
scope, on V0.2.X's reading of the gate and pending the operator's
acceptance.** The five forms the first version left waiting on V0.2.X's
question 10 now compile byte-identically, and so does
`examples/scripts/07_refinement.kel`. V0.2.X states that question 10 stays
open as a residual of its type stage. These are V0.2.X's records and
measurements, and this line has not re-measured them.

This line's [Order 6 record](./ORDER_6_COMPLETION.md) and question 18 of
[the operator question list](../process/handoffs/v0.3.0-QUESTIONS.md) were
revised. The question now asks whether the dependency is met once the
operator accepts V0.2.X's audit, or was met already through the reference
bytecode. Until it is answered, the record treats the dependency as unmet.

The first version of question 18 left one item unchecked, whether native
lowering reaches `07_refinement.kel`. An exploratory run of the native
`differential` and `corpus_differential` suites with their output shown, kept
in ignored `tmp/abs-112-reach.log`, lists 10 exempt modules, and
`07_refinement.kel` is not among them. A module that is not exempt is lowered
natively and compared with the virtual machine, so native lowering reaches
it. That is an inference from the exempt list, because the run prints no
verdict for the file by name.

## Effect on this line

Four stage sources moved, `codegen.kel`, `lexer.kel`, `parse.kel` and
`reconstruct.kel`, and their digests in `corpus_fingerprint.rs` were
re-pinned. **`lexer.kel` is the native library subject** with the most
pinned native figures, and its change adds `punct_label`, called from
`punct_op` for the `@` and lone `!` bytes. An exploratory run of the native
default suite after the re-pin, kept in ignored `tmp/abs-112-probe.log` and
not counted as this record's verification, passed 841 of 841, so every
pinned figure held. As before, no cost is pinned for a stage module by name,
and the lexer exports no composed cost claim, which is question 5. A rise in
its native cost is therefore neither confirmed nor excluded. Upstream also
changed root `tests/self_hosted_backend.rs`, so it was run in addition to the
usual set. The brief and checkpoint hold 120 messages.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-112/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 42 passed |
| Root `self_hosted_backend` with `self-host` | 8 passed |

Every pinned native figure held in both feature sets, `lexer.kel`'s included.
The full-language count rose from 41 to 42 because upstream added one test.
This table was added after the run. The tests that read this record were then
rerun, `prediction_stamp`, `outstanding_reports`, `handoff_figures` and the
native and root `comment_citations`, with the root `documentation_links`. The
root workspace suite and the heavy self-host tier were not run locally. The
hosted run on the pull request is the integration check.
