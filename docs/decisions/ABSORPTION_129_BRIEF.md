# Absorption 129

> **Navigation** [Decisions](./README.md) | [Absorption 128](./ABSORPTION_128_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `5457bb5d`, which is pull request 692 merged into
`v0.3.0`, and upstream `364e33fa` on 2026-11-01, with 0 unabsorbed commits
measured after fetching. The merges were prepared on `feat/v3-absorption-129`
with `--no-ff --no-commit`, in three steps through `ccc606cb`, `3ad5e734` and
`7176e6e8`. The absorbed upstream pull requests are these.

- **687** and **695** record V0.2.X's rulings of 2026-10-31.
- **688** merges `main` into Kaizen's line, **689** shards continuous
  integration by duration, **691** adds an editor configuration for the fuzz
  corpus, and **693**, **696**, **698**, **699** and **706** are Kaizen's
  records, including its scratch-volume proposal and its open decisions.
- **690** copies a private composite read that a later write can reach.
- **694** checks let patterns and compiles pipes in the self-hosted pipeline.
- **697** and **703** link new worktrees and the gate's target directory to
  the scratch volume when `SCRATCH_BASE` is set.
- **700** checkpoints V0.2.X's interline queue.
- **701** turns the `text::` operations into methods such as `a.concat(b)`.
- **702** refuses a delegated callee whose yield type, or whose resume type
  for a `loop`, differs from the entry's, and refuses shadowed call targets.
- **705** compiles nested and single-variant enum let patterns in the
  self-hosted pipeline.
- **704** writes a resume into the innermost stream frame, so a delegated
  `loop` reads the reply after its `Reset`.

## Redaction before the push

Two checkpoint bodies, sequences 193 and 194, held private material that the
interline protocol excludes from checkpoints. On the operator's decision both
unpushed merge commits were rebuilt with only those bodies replaced by a
marker, and the mailbox store was redacted the same way. The archive records
the commits, the checks and the backup reference, which is never pushed and is
deleted after the push succeeds.

## Merge and effect

No file conflicted in the last step. The first step conflicted in the
decisions index, `REVERSE_PROMPT.md` and the checkpoint. The index kept both
sides, the reverse prompt was rebuilt from upstream's block and this line's,
and the checkpoint was exported again from the store. `CLAUDE.md` keeps this line's
routing paragraph. `corpus_fingerprint` was re-pinned three times, for 690,
694 and 705, and the figures it names passed unchanged each time.

**The delegated Reset.** Sequence 209 reported that a delegated `loop` read a
cleared parameter after its `Reset`, and that upstream 690 left zeros in that
path. V0.2.X accepted both as its own in sequence 210, and 704 fixed them. The
native backend had matched the old reference. Its delegate yield helper wrote
each reply into the entry's parameter, and a delegated `loop` cleared its own
at `Reset` without restoring it. Now a delegated `loop` with a parameter
builds its own copy of the delegate context at entry, with its own parameter
slot and flags as the reply's destination. Its yields and the delegates it
calls use that copy, and its `Reset` restores the latest reply as the entry's
does. A `yield` callee keeps the inherited context, so its parameter stays the
argument for the whole call, as upstream pins.
`delegated_loop_resets_rebind_the_innermost_parameter_like_the_vm` drives
every sequence of upstream's `tests/delegated_loop_reset.rs` on the reference
and in native code at both optimization levels.

**The tests that pinned the old state.** In `retcon_bytecode.rs`, tests whose
expectations came from the cleared parameter were handled as follows.

| Test | Now |
|---|---|
| Seven trap and sequence tests on a `Word` parameter | Pin the reply, on the reference and in native code. Five were renamed because their names described the trap |
| `composite_packing_uses_actual_sizes_after_parameter_reset`, `equality_of_rebound_parameters_matches_vm_partial_eq`, `guarded_nested_parameter_uses_execute_before_reset`, `guarded_composite_construction_from_the_parameter_executes`, `internal_calls_preserve_actual_kinds_and_body_extents`, `private_scalar_slots_preserve_stored_parameters` | The `Word` cases pin the reply. Every other parameter type is a pinned refusal under 702 |
| `a_delegated_array_parameter_is_refused_for_its_resume_type`, `a_delegated_enum_parameter_is_refused_for_its_resume_type` | Pinned refusals |
| `native_snapshot_results_survive_calls_suspension_and_completion`, `native_snapshot_completion_links_from_c_and_emits_for_tier_one_targets` | `emit` yields the pair the entry completes with, and the hosts read pairs at each yield |
| `per_site_dialogues_distinguish_yields_replies_and_completion` | The helper yields a `Word`. The three sites still differ in their replies |
| `delegated_composite_yields_remain_refused`, `explicit_flat_replies_survive_reuse_and_delegated_entry_completion`, `scalar_reply_widths_and_kinds_match_the_vm` | The mismatched delegations are pinned refusals at compile time, and the controls stay |

`assert_refused_as_a_mismatched_delegation` accepts only the refusal 702
raises, so a fixture refused for another reason fails. The population moved
from 846 to 847, and `test_population_guard.rs` names each of the ten
renamed tests with its successor.

**Coverage change.** A cleared parameter no longer arises from checked source,
so the native kind trap on it lost its source witnesses. The trap stays
reached by five program-level witnesses through `require_program_kind_trap`.
Native lowering of a delegated callee whose scalar yield type differs from the
entry's is now reached only by bytecode that did not pass the type checker,
and pair yields from a delegate are driven by the snapshot tests.

**Other effects.** The out-of-subset witness in `kel_native_cli.rs` became a
typed destructuring `let`, following upstream's own witness, after 694 and
705 brought pipes and nested patterns into the self-hosted subset. Four tests
follow the private read copy of 690. Six new emitter sites were classified in
`memory_read_census.rs`, `pointer_offset_census.rs` and
`value_movement_census.rs`.

## Verification

Made with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1`, at `915e739c` and
again at `841b75b4` after the records. The figures below were written after
the second run, and only this file changed afterwards.

The first full run at `915e739c` passed every step except the two native
suites, at 874 passed, 2 failed and 1 ignored each. Both failures were records
written after it, the population ledger citing the retired names in backticks
and the state table's test count. After them:

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 876 passed, 0 failed, 1 ignored each, over 178 test binaries |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | 21 passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | 62 and 122 passed |

The root workspace suite was not run locally. The hosted run on the pull
request covers it. The native differential for the `text` methods of 701, and
the module route's refusal of 690's copy, remain open.
