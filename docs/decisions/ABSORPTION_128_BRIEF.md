# Absorption 128

> **Navigation** [Decisions](./README.md) | [Absorption 127](./ABSORPTION_127_BRIEF.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

Computed against native `02e0c852`, which is pull request 686 merged into
`v0.3.0`, and upstream `35955511` on 2026-10-31, with 9 unabsorbed commits
measured after fetching. The merge was prepared on `feat/v3-absorption-128`
with `--no-ff --no-commit`. The absorbed upstream pull requests are these.

- **684** fixes four compiler defects. A handled indexed write copies a
  composite old value first, let patterns are checked against the value type,
  a yield's resume value takes the type of the first parameter of `loop main`,
  and a pipe into a generic function is monomorphized as its call is.
- **685** gives the lone wire joins the reference's constant roots, private
  initializers included, and gives the self-hosted validator its twin of the
  private-data definite-initialization rule in `verify_depth.kel`.

## Merge and effect

No file conflicted. `CLAUDE.md` keeps this line's routing paragraph,
`docs/process/REVERSE_PROMPT.md` did not change upstream, and
`corpus_fingerprint` reported `verify_depth.kel` modified, which was
re-pinned. V0.2.X's sequences 179, 180 and 182 were read and acknowledged,
this line sent sequences 181 and 184, and the interline checkpoint holds 184
messages.

**Sequence 170 is closed.** `stage_differential.rs` again compares every join
of `14_frame_log.kel` with the reference exactly, and the 26-byte exception is
removed.

**Sequence 171 is closed for an annotated operand.**
`a_pipe_into_a_generic_template_agrees_with_the_vm` in
`general_stream_sequence.rs` lowers `t |> id()` as `id__Word`. The same program
with an unannotated resume binding fails to compile in the reference whenever
the template already has an instance, reported in sequence 181 and accepted by
V0.2.X as a defect in sequence 182.

**The resume type.** Fourteen native tests in both float configurations built
programs whose resume value mixed its type or extent with another, on purpose,
to reach the native backend's run-time reply guards, and the reference now
refuses them. Each was handled as follows.

| Test | Now |
|---|---|
| `float_param_stream::a_float_parameter_stream_reaches_a_decision_rather_than_a_panic` | The float-in, word-out subject converts its resume value, `yield (r as Word)`, and keeps its subject |
| `coroutine_scalar_admission_requires_complete_parameter_signatures` | The corrupted-signature subject is a `Byte` stream, `-> Byte`, and keeps its subject |
| `delegated_composite_yields_remain_refused` | The callee ends with a tail of its declared tuple type, and the native refusal stands |
| `scalar_reply_widths_and_kinds_match_the_vm` | The `Float` callee ends with a `Float` tail, and the native refusal stands |
| `enum_refinement_is_invalidated_when_the_receiver_local_changes` | Both arms are enums, and the bytecode mutation that writes a scalar into the receiver keeps its subject |
| `mixed_array_extents_trap_before_out_of_bounds_access` | The mixed source is a pinned refusal, and the bounds trap is reached by indexing the resume value itself |
| `scalar_replies_do_not_become_enum_pointers` | 22 sources are pinned refusals, and the 2 the reference still admits, a unit variant matched against a `Word`, keep the native comparison |
| `enum_field_reads_use_the_minimum_extent_without_overcopying`, `mixed_array_extents_use_the_selected_runtime_bound`, `mixed_enum_receiver_kinds_execute_with_guarded_body_inspection`, `mixed_owned_bodies_keep_selected_extents_across_suspension`, `mixed_reply_arithmetic_is_refused_where_the_vm_faults`, `owned_variable_bodies_cross_calls_and_delegated_suspension`, `reply_tags_are_checked_through_calls_and_branch_joins` | Every mixed source is a pinned refusal, with its count pinned. The last keeps its matching `Byte` control |

`assert_refused_for_its_resume_type` accepts only the type errors a mixed reply
raises, so a fixture refused for another reason fails. **Coverage change.** The
native guards for a joined body of mixed extent or kind are no longer reached
from checked source. They stay as defense in depth, as the guard subjects
retired at absorption 124 did.

**Question 28.** The operator answered option a first hand on 2026-10-31. The
generated header keeps `<prefix>_region_bytes`, and the library documentation
in `linkage.rs` says that a host sizing from `auto_arena_capacity_for` must
not add it.

## Verification

Made with `RUSTC_WRAPPER=` and `KEL_REQUIRE_LLVM_TOOLS=1`.

The first full run, before the test changes, found the fourteen failures
above in each suite and the receipt link missing. After them:

| Check | Result |
|---|---|
| Native fmt, clippy in both float configurations, doc and shellcheck | passed |
| Native suites, default features and `narrow-float-32` | 875 passed, 0 failed, 1 ignored each |
| Root `documentation_links`, `comment_citations`, `ci_shard_coverage` and `gate_ci_correspondence` | passed |
| Root `confinement_analysis`, `rogue_scripts`, and with `self-host` `selfhost_full_language` and `selfhost_parse_refusals` | passed |

Sequence 184 reports two observations to V0.2.X for its disposition. A unit
variant pattern still matches a `Word` resume value, and a delegated callee may
still yield a scalar type that differs from the entry's declared output, which
the native backend refuses. The root workspace suite was not run locally. The
hosted run on the pull request covers it.
