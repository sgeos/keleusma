# Absorption 106

> **Navigation** [Decisions](./README.md) | [Absorption 105](./ABSORPTION_105_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `571f0fb3`, which is absorption 105 on
`feat/v3-absorption-105` as pull request 567, and upstream `715cb866` on
2026-10-26, with 3 unabsorbed commits measured after fetching. The merge base
is `fecc737b`. The separate conflict command
`git merge-tree --write-tree --name-only 571f0fb3 origin/v0.2.3` exited zero
and produced tree `bf3ce8901487fdec64340a4f05ef0eb5d8b52566` with no
conflicting file. The commits are V0.2.X pull request 564. The self-hosted
parser now commits `assert c;` as a node that codegen lowers to its
continuation alone, as the reference compiles an assertion out of a release
build, and the message form is refused at its comma. The merge was prepared on
`feat/v3-absorption-106`, cut from absorption 105, with
`--no-ff --no-commit`. This absorption is not on `v0.3.0` until pull request
567 merges first.

## Merge

No file conflicted. The first 183 lines of `docs/process/REVERSE_PROMPT.md`
were compared byte for byte with upstream's file, and this line's block
follows them. `CLAUDE.md` keeps this line's routing paragraph. The merged
`compiler/`, `src/` and `tests/` equal upstream's except for
`tests/gate_ci_correspondence.rs`, whose difference predates this absorption.
The one-line change to `tests/selfhost_codegen.rs` adds kind 83 to a node-kind
match and does not touch the pinned construct-support boundary counts.

## Effect on this line

Three stage sources moved, `codegen.kel`, `parse.kel` and `reconstruct.kel`,
and their digests in `corpus_fingerprint.rs` were re-pinned. All three are
native corpus subjects, with no native cost pinned for them by name. The
native compiler consumes the reference compiler's bytecode, and the reference
is unchanged, so the assertion change itself does not reach native output.
Upstream also changed root `tests/selfhost_typecheck.rs` and
`keleusma-cli/tests/self_hosted_refusal.rs`, so those two were run in
addition to the usual set.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1` after this
receipt and the re-pin were written, on a tree that did not change during the
run. Logs are under ignored `tmp/abs-106/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 841 passed each, 0 failed |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 35 passed |
| Root `selfhost_typecheck` with `self-host` | 103 passed |
| `keleusma-cli` `self_hosted_refusal` | 5 passed |

Every pinned native figure held in both feature sets. The full-language count
rose from 34 to 35 because upstream added one test. This table was added
after the run. The tests that read this record were then rerun,
`prediction_stamp`, `outstanding_reports`, `handoff_figures` and the native
and root `comment_citations`, with the root `documentation_links`. The root
workspace suite and the heavy self-host tier were not run locally. The hosted
run on the pull request is the integration check.
