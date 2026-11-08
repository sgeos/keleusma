# Absorption 122

> **Navigation** [Decisions](./README.md) | [Absorption 121](./ABSORPTION_121_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `e8c38469`, which is pull request 661 merged into
`v0.3.0`, and upstream `5ed0bdfd` on 2026-10-30, with 25 unabsorbed commits
measured after fetching. The merge was prepared on
`feat/v3-absorption-122` with `--no-ff --no-commit`. The absorbed upstream
pull request is V0.2.X's batch 658, announced in message 166. It carries
these changes.

- **The narrowed definite-initialization rule.** `src/verify_init.rs` refuses
  a `GetLocal` of a local that no path writes, with `Stream` resetting the
  written set to the stream parameter. Composite private data and a local
  written on one path only are not checked. `verify_depth.kel` decides the
  same rule for the self-hosted validator.
- **`Text<N>` values and the literal cast.** A `Text<N>` is a flat composite
  of a length word and `N` bytes. Comparison, destructuring, other casts, a
  `shared` field and a native signature naming it stay refused, and the
  self-hosted compiler reports it as unsupported.
- **A host composite size check.** The virtual machine compares a composite
  entry argument or resume value with the chunk signature and refuses a
  mismatch with `TypeError` before any instruction runs.
- **Composite host inputs in `auto_arena_capacity_for`**, with the new
  `auto_arena_capacity_with_input_text` and `host_input_arena_bytes_for`. The
  native region supplement is not part of this, so question 22 stays open.
- Fuzz targets and their seed corpora, and a `LoadError` formatting fix the
  fuzz targets found.

## Merge and effect

`docs/decisions/README.md` conflicted on one row. This line's rows were kept
and upstream's newer `TEXT_CAPACITY_TYPE.md` row was taken. `CLAUDE.md` keeps
this line's routing paragraph. `docs/process/REVERSE_PROMPT.md` did not change
upstream, so it was not rebuilt, and this line's report 2 in it is marked
answered. The interline checkpoint was regenerated at 167 messages, and none
of upstream's 164 identifiers is missing.

`corpus_fingerprint` reported `verify_depth.kel` modified, which was
re-pinned. Eight native tests then failed in both float configurations, each
because the reference now refuses something earlier.

| Test | Cause | Now asserts |
|---|---|---|
| `outstanding_reports::report_two_still_reproduces_on_the_reference` | A stream reading a later parameter after its rewind no longer compiles | Renamed `report_two_is_answered_and_the_answer_holds`, the refusal by the definite-initialization rule |
| `probe_multi_param_stream::the_runtime_faults_on_a_second_parameter_after_the_rewind` | The same | The refusal and its reason |
| `narrow_stream_differential::a_stream_with_other_than_one_parameter_is_refused` | The same for the reading case | The refusal for a read second parameter, and the native refusal for an unread one and for zero parameters |
| `retcon_bytecode::reset_clears_runtime_kinds_of_later_entry_parameters` | The same | The refusal and its reason |
| `retcon_bytecode::stream_reset_does_not_replenish_later_parameters` | The same | The refusal and its reason |
| `differential::control_unwritten_local_reads_as_unit_not_undef` | The verifier refuses a hand-built read of an unwritten local | The refusal, and the native store of `Unit` that `lower_module` keeps for an unverified module |
| `retcon_bytecode::stream_composite_replies_keep_their_actual_extents` | The virtual machine refuses a reply of another size | The refusal, and the native result under the declared dialogues |
| `retcon_bytecode::shortened_stream_reply_traps_before_an_out_of_bounds_read` | The same | The refusal, and the native bounds trap |

The native refusal of a stream with more than one parameter, and the native
clearing of a later parameter's kind at `Reset`, are now unreachable from
source, because `coroutine::lower` verifies first. They stay as defense in
depth.

The per-yield reply contracts of `coroutine::lower_with_dialogues` read each
reply at its declared extent, so they admit a reply size the virtual machine
now refuses. That interface is used only by tests, and the C host interface
does not expose it.

`static_kind_lowering` measures 1 of 192,475 instructions needing a run-time
kind, at the private slot guard in `14_frame_log.kel`, with 30 static, 1
dynamic and 2 refused coroutines. That is the residual question 26 describes.

## Verification

The full native run was made with `RUSTC_WRAPPER=` and
`KEL_REQUIRE_LLVM_TOOLS=1` twice. The first run, before the test changes,
found the eight failures in both suites, and its other steps passed, the root
steps among them. The second run, on the changed tree, passed every step.

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suite, default features | 863 passed, 0 failed, 1 ignored |
| Native suite, `narrow-float-32` | 863 passed, 0 failed, 1 ignored |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis` and `rogue_scripts` | passed |
| Root `selfhost_full_language` and `selfhost_parse_refusals` with `self-host` | passed |

This paragraph and its table were written after that run, and the
documentation readers were rerun on them. The root workspace suite was not
run locally. The hosted run on the pull request covers it.
