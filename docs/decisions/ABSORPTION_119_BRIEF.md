# Absorption 119

> **Navigation** [Decisions](./README.md) | [Absorption 118](./ABSORPTION_118_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `638c82e3`, which is pull request 639 merged into
`v0.3.0`, and upstream `fb877f77` on 2026-10-30, with 22 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-119`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **636** adds `src/selfhost/kel/template_scan.kel` as the fifteenth stage
  source. By V0.2.X's notice in sequence 152, the generic template header scan
  moved out of the host on the operator's ruling. It also makes
  `Module::schema_hash` cover each data slot's declared type, which is V0.2.X's
  half of question 8.
- **637** adds the flat `Text` layout proof to the reference typed pass and to
  `verify_typed.kel`, refusing a `Text` read from a field built as another kind.
- **638 and 640** are V0.2.X's handoff and a withdrawn question of its own.

## Merge and effect

Two files conflicted. `docs/process/REVERSE_PROMPT.md` was rebuilt by
`tmp/rebuild_rp.sh` against `4e0c2304`, as upstream's 147 lines followed by
this line's 720. The interline checkpoint was regenerated from the mailbox
store, and none of upstream's identifiers is missing. `CLAUDE.md` keeps this
line's routing paragraph.

`corpus_fingerprint` reported `template_scan.kel` added and `verify_typed.kel`
modified. The first was pinned and the second re-pinned. Eight native tests
then failed, each on a corpus figure the added stage moves, and each was
re-pinned with its measured cause.

| Test | Was | Now | Cause |
|---|---|---|---|
| `corpus_differential` vacuous set | empty | `template_scan.kel` | Its run agrees and changes nothing, because this harness feeds it no token batch |
| `differential_coverage` executed modules | 67 | 68 | The added stage executes |
| State table corpus row | 78 | 79 | The added stage |
| `native_stage_executor` stage count | 14 | 15 | The added stage |
| `region_composition` streaming and majority | 32 and 28 | 33 and 29 | The added stage reserves 90% of a 568-byte figure |
| `spike_bounds_transfer` population | 29, 406, 12 | 30, 435, 24 | Without the added chunk the population is the previous 29 with 12 inversions, so all 12 new ones involve it |
| `streaming_arena_touch` streaming and driven | 32 and 16 | 33 and 17 | The added stage drives and touches zero bytes |

The inversion count is taken over the population sorted by bound, so a count
in one direction is complete. That was checked while establishing the cause.

The native suites rebuilt from a clean tree, because the build tree was
cleaned with the operator's approval on 2026-10-30 after it filled the disk.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` on this absorption with the `frame.kel` bridge,
the next pull request, stacked on it. Every step passed.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suite, default features | 861 passed, 0 failed, with the bridge's test |
| Native suite, `narrow-float-32` | 861 passed, 0 failed, with the bridge's test |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis` and `rogue_scripts` | passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | passed |

The eight re-pinned files and the documentation readers were also run on this
absorption alone. The root workspace suite was not run locally. The hosted run
on the pull request covers it.
