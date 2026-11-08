# Absorption 125

> **Navigation** [Decisions](./README.md) | [Absorption 124](./ABSORPTION_124_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `6592d3c1`, which is pull request 678 merged into
`v0.3.0`, and upstream `d9fb3a5f` on 2026-10-31, with 2 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-125`
with `--no-ff --no-commit`. The absorbed upstream pull request is V0.2.X's 677,
on the operator's ruling B10, option (b), of 2026-10-31 as V0.2.X records it.

- A native may stop the virtual machine with `VmError::HostHalt(message)`. The
  native-error construct does not catch it, the default host-error trap does
  not rewrite it, and its category is `Halt`.
- `VmError` is `#[non_exhaustive]`, which breaks a host that matches it
  exhaustively, once.

## Merge and effect

No file conflicted. `CLAUDE.md` keeps this line's routing paragraph,
`docs/process/REVERSE_PROMPT.md` did not change upstream, and the interline
checkpoint was regenerated with none of upstream's identifiers missing.

Native code needs nothing for it. A C native returns a word through the
generated header and cannot return a `VmError`. The stage factory's closure
bridge already fails a turn on any native error, which this line has not
specialized for `HostHalt` because no stage native returns it. No match on
`VmError` in `native_codegen` lacks a wildcard arm, so the package built
unchanged.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` on this absorption. Every step passed.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suite, default features | 873 passed, 0 failed, 1 ignored |
| Native suite, `narrow-float-32` | 873 passed, 0 failed, 1 ignored |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | passed |

The records were written after that run, and the documentation readers were
rerun on them. The root workspace suite was not run locally. The hosted run on
the pull request covers it.
