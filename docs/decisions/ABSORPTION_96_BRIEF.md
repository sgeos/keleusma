# Absorption 96

> **Navigation** [Decisions](./README.md) | [Absorption 95](./ABSORPTION_95_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `b98524b4`, which is absorption 95 with the witness
move on `feat/v3-absorption-95`, and upstream `22cb258d` on 2026-10-26, with
19 unabsorbed commits measured after fetching. The merge base is `84cbea1d`.
The separate conflict command `git merge-tree --write-tree b98524b4 22cb258d`
exited one and produced tree `7e467469f668335fcf7a4b2571ba9db3c92595c7` with
one conflicting file. The commits are the V0.2.X pull requests 540, 541 and
543. They carry Order 6
slices 3b to 3e, 4 and 2b, among them the pipe operator, literal parameter
heads, `use` path separators, `for` over static scalar arrays, the function
modifiers, trait implementations with method calls, and `use` signatures with a
scalar return. The merge was prepared with `--no-ff --no-commit`. Upstream
merged pull request 544 at `66de2778` while this was being verified, and it is
left for the next absorption.

## Conflicts

One file conflicted, `docs/process/REVERSE_PROMPT.md`. It was rebuilt as
upstream's 102 lines followed by this line's block, and each part was compared
byte for byte with its source. `CLAUDE.md` keeps this line's routing paragraph.

## Effect on this line

Five stage sources moved again, `codegen.kel`, `lexer.kel`, `parse.kel`,
`reconstruct.kel` and `wire.kel`, and their digests in `corpus_fingerprint.rs`
were re-pinned. Every other native test passed with its pinned figure, among
them the lexer's frame and stack bounds and the five censuses the fingerprint
file names.

**Slice 2b settles the full-language collision that absorption 95 found.**
This line's `rogue_dungen.kel` declares `use host::rng_range(Word, Word) ->
Word`. With slice 2b the self-hosted parser reads that declaration, and the
whole of `tests/selfhost_full_language.rs` passed on this tree, including the
per-source outcomes, where the file is NoVerdict as upstream pins it. This
measures what the V0.2.X line predicted in
`v023-20261026-full-language-table-reply`. Its adoption of the typed
declaration in the shared file is still to come and is expected to change
nothing here.

## Verification

Run locally with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1`. Logs are under
ignored `tmp/abs-96/`.

| Check | Result |
|---|---|
| fmt, both clippy configurations, doc and shellcheck | status zero |
| Native suites, default and `narrow-float-32` | 840 passed, 1 failed each, the fingerprint pin |
| `corpus_fingerprint` after the re-pin, both feature sets | 3 passed each |
| The four root guards | 17 passed |
| Root `confinement_analysis` and `rogue_scripts` | 62 passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | 18 passed |

**The tree moved once during the run.** The fingerprint re-pin was written
while the root `self-host` step was still running. The edited file is in the
detached `native_codegen` package, which is not a root workspace member, so it
was not an input to that step. The step is reported with that qualification
rather than rerun. The re-pinned test itself was rerun after the edit. The root
workspace suite was not run locally. The hosted run on the pull request is the
integration check.

The first hosted run of this absorption failed in both native gates on one
test, `prediction_stamp.rs`, because this receipt did not yet state the tree
and unabsorbed count its figures were computed against. The local run had
finished before the receipt was written, and the guards rerun afterwards did
not include that test. The opening now states them, and `prediction_stamp`,
`handoff_figures` and the native `comment_citations` were rerun and passed.
