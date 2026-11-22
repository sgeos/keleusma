# Absorption 153

> **Navigation** [Decisions](./README.md) | [Absorption 152](./ABSORPTION_152_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against upstream `ec23afa7` on 2026-11-22, with 0 unabsorbed commits
measured after fetching. The merge was made on `docs/v3-resume-20261118`, after
absorption 152, at the operator's instruction to adopt Kaizen's mainline fixes
so that this line's next CI run passes.

- **778** is V0.2.X's proofs findings work. It refuses recursion at compile
  time, composes the worst-case execution time bound through calls, saturates
  the bound arithmetic, and validates the memory slot width, in `src/verify.rs`,
  `src/typecheck.rs`, `src/vm.rs` and `src/compiler.rs`, with new tests.
- **788** is Kaizen's fix after 778 failed the proofs job. Findings F1 and F2
  now verify, so `tools/proof-kani/check.sh` moves them to the expected passes.
  A branch cut before it fails the proofs job on F1 and F2.
- **789** is Kaizen's handoff, with the interline checkpoint it exported.

## Merge and effect

No file conflicted, `CLAUDE.md` keeps this line's routing paragraph, and nothing
under `native_codegen/` changed. The root changes alter what the compiler and
verifier admit, and `native_codegen` builds on the root crate, so the native
suites were run again on the merge. The checkpoint was scanned with this line's
disclosure-term scan. Of its 565 messages none matches a term, and the only home
paths are the four the operator accepted.

## Verification

Made with `RUSTC_WRAPPER=` at `70ec4613` in a separate worktree, with the
machine status ok.

| Check | Result |
|---|---|
| Native fmt, Clippy with default features and `narrow-float-32`, doc and shellcheck | passed |
| Native suite, default features | 882 passed, 0 failed, 1 ignored |
| Native suite, `narrow-float-32` | 882 passed, 0 failed, 1 ignored |
| Root checks of the verification script | passed |

`main` at `b377ff9d` was then merged as well, by Kaizen's sequence 574, which
reports that pull request 790 carries the Rust 1.100 fixes to `main`. The merge
changes no file, since 790 cherry-picks 782, which absorption 151 brought in,
so it only records `main` as an ancestor.

The Kani proofs job is not run locally, and the hosted run on the pull request
covers it and the rest.
