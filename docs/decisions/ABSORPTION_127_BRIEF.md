# Absorption 127

> **Navigation** [Decisions](./README.md) | [Absorption 126](./ABSORPTION_126_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `ca6de1fb`, which is pull request 683 merged into
`v0.3.0`, and upstream `86069615` on 2026-10-31, with 5 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-127`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **680** folds the native region supplement into
  `vm::auto_arena_capacity_for`, on the operator's answer to question 22 of
  2026-10-31. The new module `keleusma::native_region` reimplements the
  formula of
  [`NATIVE_REGION_SUPPLEMENT_FORMULA.md`](./NATIVE_REGION_SUPPLEMENT_FORMULA.md),
  and `tests/native_region_supplement.rs` pins five of this line's figures.
- **682** adds `text::len`, `text::capacity`, `text::concat`, `text::eq`
  and `text::slice` as compiler intrinsics over `Text<N>` behind the cargo
  feature `text`, on the operator's answer to question 25. Each lowers to
  existing opcodes, and none adds a native or an opcode.

## Merge and effect

No file conflicted. `CLAUDE.md` keeps this line's routing paragraph,
`docs/process/REVERSE_PROMPT.md` did not change upstream, and no stage
source changed. V0.2.X's notice of sequence 179 was read and acknowledged, and
the interline checkpoint holds 179 messages.

**The supplement.** The native backend stays the authority, as
`keleusma::native_region` says of itself. The new test
`the_shipping_crates_copy_agrees_with_the_backend` in
`region_supplement_formula.rs` compares that copy with
`region::host_arena_supplement_bytes` on every corpus module with an entry,
and they agree. A host that still adds the generated header's region figure to
`auto_arena_capacity_for` now counts the supplement twice. That
over-provisions and cannot under-provision. Removing the header figure is the
next increment.

**The text operations.** `native_codegen` does not enable `text`, so this
absorption builds them out. A native differential against the virtual machine
is a later increment.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` on this absorption.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 873 passed, 1 failed, 1 ignored each |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | passed |

The one failure in each suite was `test_population_guard`, which counted the
new test. Its figure moved from 844 to 845 with the reason recorded, and it and
`handoff_figures` were rerun and passed. The root workspace suite was not run
locally. The hosted run on the pull request covers it.
