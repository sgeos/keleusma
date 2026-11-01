# Absorption 85

Computed against native `9dba0d2f` and upstream `be81c214` on 2026-10-24,
with 8 unabsorbed commits measured after fetching. The merge base is
`d7461857`. The separate conflict command
`git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited one and
produced tree `1e476a180c477cc02b772c80460d3e8a60a31c33` with one conflicting
file, `docs/process/REVERSE_PROMPT.md`. The merge was prepared on
`feat/v3-absorption-85` with `--no-ff --no-commit` so the absorption and
handoff could share the prompt's single commit.

## What was absorbed

Pull request 498, merged upstream as `6db0724b`, and the V0.2.X handoff and
receipt commits around it. Pull request 498 adds reference-poisoning controls,
a command corpus and the operator's acceptance of the first-pass Order 2 scope.

- `src/lexer.rs` gains a `reference_poison` module and one call in `tokenize`.
  Both are compiled only under `cfg(all(test, feature = "self-host"))`, so no
  build outside the root test harness contains them.
- `src/selfhost/mod.rs` declares two test modules,
  `src/selfhost/type_corpus.rs` and `src/selfhost/reference_poison_tests.rs`,
  both under `cfg(test)`.
- `tests/selfhost_typecheck.rs` and the new
  `keleusma-cli/tests/self_hosted_stage_corpus.rs` extend the self-host tests.
- The rest is documentation, the Order 1 and Order 2 audits, and a
  verification receipt under `docs/process/verification/`.

No stage source under `src/selfhost/kel/`, opcode or bytecode version changed.

## Conflict resolution

Upstream rewrote its bounded reverse prompt to 267 lines. At the merge base the
file had 351 lines, and this line's side is that base followed by 668 appended
lines beginning at the heading `APPENDED BY THE V0.3.X LINE`. The first 351
lines of this line's side were checked byte for byte against the merge base.
The resolution keeps upstream's file and appends this line's 668 lines
unchanged, as absorption 82 did.

`docs/process/interline/checkpoint.json` merged without a conflict and was then
regenerated from the live store, as the protocol requires. It holds eight
messages. Seven are acknowledged. The one still open is this line's request to
V0.2.X, `v030-20261024-kaizen-merge-coordination`.

## Effect on this line

No `native_codegen` test reads the changed root source or tests. Two native
tests, `outstanding_reports` and `shared_channel_discipline`, read the shared
reverse prompt, so both were run on the resolved file. The added root code is
compiled only in root test builds, and the native package depends on the root
crate as a library. The native corpus fingerprint is unaffected because no
stage source changed. This is an inference from the configuration predicates
and the file list. The targeted checks below test it only in the default
configuration.

## Kaizen branch

The operator asked on 2026-10-24 for Kaizen's testing changes to be merged into
`v0.2.3` before this line proceeds, and for this line to absorb them from
there. Kaizen pull requests 493 and 494 were already absorbed in absorption 84.
Pull request 495 was closed by the operator. The pilot branch
`feat/kaizen-jev-shadow-pilot` is not yet merged. V0.2.X accepted the merge in
`v023-20261024-merge-kaizen-branch-reply`, and Kaizen will prepare the pull
request. This line absorbs it from `v0.2.3` once it lands, in a later absorption.

## Verification

Run locally with `RUSTC_WRAPPER=` on the staged merge. Logs are under ignored
`tmp/absorption-85/`. Figures are read from each log's summary lines and its
own captured status.

| Check | Result |
|---|---|
| Root `cargo test --locked --no-fail-fast` with `--test ci_shard_coverage --test gate_ci_correspondence --test documentation_links --test comment_citations` | 16 passed, status zero |
| Native `cargo test --locked --no-fail-fast` in `native_codegen/` with `KEL_REQUIRE_LLVM_TOOLS=1` and `--test comment_citations --test handoff_figures --test test_population_guard --test gate_record --test shared_channel_discipline --test outstanding_reports --test prediction_stamp --test workspace_coverage_guard` | 37 passed, status zero |

These are targeted checks in the default feature configuration. They do not run
the self-host tier, the other four root feature sets or either complete native
gate. The hosted CI run on the absorption pull request is the version-branch
integration check. No new full-gate record was written. Publication is held and
no merge into `main` is authorized.
