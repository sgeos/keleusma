# Absorption 95

Computed against native `560ae327` and upstream `84cbea1d` on 2026-10-25,
with 24 unabsorbed commits measured after fetching. The merge base is
`fde7b65e`. The separate conflict command
`git merge-tree --write-tree origin/v0.3.0 origin/v0.2.3` exited one and
produced tree `c10f2722ad5dbdde54b6f6443fd5e6c716e4f42a` with two conflicting
files, `docs/process/REVERSE_PROMPT.md` and
`docs/process/interline/checkpoint.json`. The merge was prepared on
`feat/v3-absorption-95` with `--no-ff --no-commit`, after this line's Order 6
merged as pull request 539 at `560ae327`.

## What was absorbed

- Kaizen pull requests 528, 529 and 530, the Jev selection baselines, a
  corrected `Cargo.toml` comment and workflow actions pinned by commit.
- Kaizen pull request 533, the continuous integration load reduction with its
  light path and certified branch runs, and pull request 535, which repaired
  that change's guard. Kaizen asked for the two to be absorbed together, as
  they are here.
- Kaizen pull requests 536 and 537, the load report, which counts
  `native_codegen/**` as light for `ci.yml` on this line's answer, and the
  required CI result job for `main`.
- V0.2.X pull requests 532 and 538, its Order 6 slices for full-language
  support. The self-hosted parser now refuses syntax it used to skip, parses
  tuple-destructuring `let`, and lowers use-imported natives.

The Kaizen and V0.2.X notices about these were read and acknowledged.

## Conflict resolution

Upstream's bounded reverse prompt is 86 lines. This line's side is the 50-line
merge-base file, checked byte for byte, followed by its 718 appended lines,
which now include the Order 5 and Order 6 reports. The resolution keeps
upstream's file and appends those lines unchanged, compared byte for byte. The
checkpoint was regenerated from the live store. `CLAUDE.md` keeps this line's
routing paragraph.

## Effect on this line

Five stage sources moved, `codegen.kel`, `lexer.kel`, `parse.kel`,
`reconstruct.kel` and `wire.kel`, and their fingerprints in
`corpus_fingerprint.rs` were re-pinned.

**Six tests failed through upstream's self-hosted parser, all for one reason.**
`parse.kel` now refuses syntax it used to skip. Two harnesses built the
multi-headed `reconstruct.kel` seed from `06_multiheaded.kel`, whose literal
heads `parse.kel` had only appeared to parse, so **that seed had described a
different program from its subject**. Both now use a guarded subject with the
same dispatch, `common::MULTIHEAD_SUBJECT`, which `parse.kel` parses. The
single-head survey pinned that no corpus file qualified, and
`15_pixel_blend.kel` now does and drives the stage, so the survey's stand-in
was retired and the pin reversed.

**The lexer's native figures moved.** Its coroutine frame bound fell from 1416
to 88 bytes, and its machine stack bound rose from 656 to 1216 bytes on
`aarch64-apple-darwin`. The Rust host measured 1184 bytes, within the bound.
Every lexer test still agrees with the virtual machine. The records written
before this absorption keep their dated figures.

**The bounds-transfer inversion count fell from 17 to 12.** Running the probe
at `560ae327` and diffing the per-chunk figures showed that exactly one chunk
moved, `lexer.kel::main`, from a bound of 252 and 1558 native instructions to
a bound of 14 and 9. Upstream moved the body of the lexer's `loop main` into a
function, and the stream chunk is now a thin wrapper. The population stays at
29 chunks and 406 pairs. The lexer's work has left this stream-only
measurement, which the pin in `spike_bounds_transfer.rs` records.

## Verification

Recorded below, run locally with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` on the staged merge, which did not change during
the run.

| Check | Result |
|---|---|
| `cargo fmt --check` in `native_codegen/` | status zero |
| `cargo clippy --locked --all-targets -- -D warnings`, default and `narrow-float-32` | status zero in both |
| `cargo doc --locked --no-deps` with `RUSTDOCFLAGS="-D warnings"` | status zero |
| `shellcheck tools/*.sh examples/lexer_host/run.sh` | status zero |
| Native `cargo test --locked --no-fail-fast`, default features | 840 passed, 1 failed, the inversion pin above |
| Native `cargo test --locked --no-fail-fast --features narrow-float-32` | 840 passed, 1 failed, the same pin |
| `spike_bounds_transfer` after the re-pin, both feature sets | 5 passed each, status zero |
| Root `--test documentation_links --test comment_citations --test ci_shard_coverage --test gate_ci_correspondence` | 17 passed, status zero |

The re-pin changed one assertion and its comment in one test file, and only
that file was rerun after it. The root guards were rerun after this table was
written. These checks do not run the root workspace suite or the self-host
tier. The hosted run on the pull request is the integration check.
