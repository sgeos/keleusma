# Every narrow-word-16 failure, classified

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: Complete classification of a finished run. Written 2026-09-08.

## Why this exists

[`FEATURE_COMBINATION_SWEEP.md`](./FEATURE_COMBINATION_SWEEP.md) established that the ten narrow
word, address and float selectors compile and that nothing runs them, then ran the suite once at
`narrow-word-16` and reported the failures grouped **by test name**. It left eighteen of them in an
unexamined remainder and said so.

**A grouping by name is a hypothesis about a premise, not a finding**, and an unexamined remainder in
a durable document reads as a lead for someone else. This document replaces both with a verdict per
failure, each supported by the failing assertion's own text.

**The question it answers.** For each failure: is the TEST assuming a sixty-four-bit host, or is the
RUNTIME wrong at a sixteen-bit word? The first is a statement about the suite. The second is a defect
on the axis this project sells, since a sixteen-bit word is what an embedded target actually has.

## The measurement

`cargo test --features narrow-word-16 --no-fail-fast`, on 2026-09-08, in its own target directory,
with the exit status captured in the log.

| | |
|---|---|
| test binaries passing | 90 |
| test binaries failing | 16 |
| individual tests passing | 1739 |
| **distinct failing tests** | **41** at the time of this measurement; **33** after the repairs below |
| completeness | the run finished; nothing was killed |

**These figures supersede the 89 / 15 / 37 recorded in the sweep document**, which were taken from a
run whose performance-canary binary was killed. That binary now bounds its own runtime and fails
cleanly at 120 seconds, so the suite terminates and the count is a total rather than a lower bound.
Every figure here is re-derivable by summing the per-binary result lines, which is how it was
produced; the earlier document records a count taken from a progress line and corrected, and that is
the mistake being avoided.

## Verdicts

Nine groups. **Members are enumerated so the total can be re-derived by addition** rather than taken
on trust: 14 + 6 + 8 + 2 + 1 + 3 + 1 + 1 + 5 = 41.

> **Both repaired groups are now closed (2026-09-08).** Group I was repaired by taking the opaque
> field's width from the layout, and group F by deriving the word width in the three harness reads
> that hard-coded eight bytes. A re-run gives **33 distinct failures across 106 binaries**, and the
> eight that stop failing are exactly those two groups: 14 + 6 + 8 + 2 + 1 + 1 + 1 = 33.
>
> **Established by DIFFING the failing sets, not by comparing totals.** A total falling by eight is
> equally consistent with fixing nine and breaking one. Nothing newly fails.

| # | group | count | verdict |
|---|---|---|---|
| A | target declares a 64-bit word | 14 | test assumes a wide host |
| B | the narrow-runtime suite's own "wider" target | 6 | test assumes a wide host |
| C | the program carries a `require word` directive | 8 | test assumes a wide host |
| D | Q-format fraction not narrower than the word | 2 | test assumes a wide host |
| E | the test's own literals exceed 16 bits | 1 | test assumes a wide host |
| F | the test reads the body in hard-coded 8-byte words | 3 | test assumes a wide host |
| G | the expectation is a 64-bit constant | 1 | test assumes a wide host |
| H | the mutation target does not exist at a narrow word | 1 | test assumes a wide host |
| I | **an opaque field is sized inconsistently** | **5** | **RUNTIME DEFECT** |

**Every verdict below was reached by reading the failing assertion's message or the test's own
source.** None rests on the test's name. Where a group's members share one message, that message is
quoted once and the members are listed.

### A. The target declares a 64-bit word (14)

All in `tests/float_arith_width.rs`, which states its premise in its own header: *"Every test here
declares a 32-bit float on a 64-bit runtime, which is the configuration the defect lived in."* At a
sixteen-bit runtime that target is inadmissible, and the compiler says so:

> `target word_bits_log2 = 6 exceeds runtime maximum 4`

`add_`, `sub_`, `mul_`, `div_`, `checked_add_`, `checked_sub_`, `checked_mul_`, `checked_div_` and
`int_to_float_narrows_to_the_declared_width`; `mod_agrees_across_widths_because_frem_is_exact`;
`neg_agrees_across_widths_because_it_is_exact`;
`every_encodable_float_width_is_classified_and_none_is_skipped`;
`a_target_claiming_floats_at_a_real_width_still_compiles`;
`the_no_floats_sentinel_still_compiles_a_float_free_program`.

**The refusal is the compiler working.** A module declaring a wider word than the runtime provides is
exactly what the width check exists to reject.

### B. The narrow-runtime suite's own "wider" target (6)

All in `tests/narrow_vm.rs`. Three fail on the same compile refusal as group A. The other three
assert on a specific message and get a different, equally correct one:

> `expected width-mismatch error, got: VerifyError("bytecode declares addr_bits_log2 = 6 but this Vm runs at addr_bits_log2 = 4")`

These tests build a deliberately narrow virtual machine and feed it deliberately wider bytecode. When
the whole runtime is already narrow, the word dimension no longer differs and the address check fires
first. **The rejection still happens; only which check catches it changes.**

`f32_narrow_runtime_can_register_math_library_via_lifted_impl`, `narrow_float_runtime_runs_f32_bytecode`,
`wider_float_bytecode_never_reaches_execution`, `narrow_runtime_rejects_wider_word_bytecode`,
`narrow_runtime_rejects_hot_swap_to_wider_bytecode`,
`narrow_runtime_view_bytes_zero_copy_rejects_wider_bytecode`.

**This file already excludes `narrow-word-8` and `narrow-address-8` in its own configuration
attribute.** That it does not exclude `narrow-word-16` is the reason these six run at all.

### C. The program carries a `require word` directive (8)

The fixture source declares a minimum word width and the compiler enforces it:

> `program requires a word width of at least 32 bits, but the compilation target's word is 16 bits; compile for a wider target or relax the `require word` directive`

`compiled_loops_really_do_carry_a_non_empty_entry_stack` (requires 64);
`the_stage_corpus_leaves_sixteen_op_tags_unexercised_and_names_them`;
`measure_shared_layout_run_distribution`;
`every_self_hosted_stage_round_trips_through_the_new_schema`; and the four
`tests/secded_end_to_end.rs` cases `a_single_flipped_bit_in_a_protected_region_is_corrected`,
`two_flipped_bits_in_one_word_are_detected_as_uncorrectable`,
`the_same_corruption_is_invisible_without_a_plane`,
`a_protected_artifact_still_decodes_through_the_ordinary_path`.

**This is the most emphatic form of a test declaring its own premise.** The program says in its
source that it needs a wider word, and the compiler refuses rather than mis-compiling it.

### D. Q-format fraction not narrower than the word (2)

> `f: FixedMul(16) at 2 declares 16 fraction bits but the word is 16 bits; a Q-format fraction count must be less than the word width`

`the_fixed_point_ops_consume_both_operands` and `the_peak_models_running_offset_never_goes_negative`,
both in `tests/operand_stack_model.rs`. A sixteen-bit fraction needs a wider word to hold a sign and
an integer part. **The refusal is a real invariant, not an incidental limit.**

### E. The test's own literals exceed 16 bits (1)

`constant_loads_in_a_loop_stay_fast`. Its program is
`for i in 0..hi limit 200000 { d.s = d.s + 1234567 + i; }`; both constants exceed a sixteen-bit
word's maximum of 32767.

**Worth noting for its own sake**: this test previously ran for 57 minutes and never finished. It now
fails at its 120-second bound with a message naming the reason. The bound added for a different
purpose is what let this run terminate and produce a total rather than a lower bound.

### F. The test reads the body in hard-coded 8-byte words (3)

Not the runtime: the harness. `tests/composite_escape_window.rs` reads the first field as
`i64::from_le_bytes(bytes[..8])`, and `tests/composite_escape_routes.rs` splits the body with
`as_chunks::<8>()`. At a sixteen-bit word a three-field body is six bytes, so the first panics

> `range end index 8 out of range for slice of length 6`

and the second collects **zero** eight-byte chunks, giving `left: []` against `right: [11, 22, 33]`.

`a_yielded_composite_outlives_its_iteration_and_dies_at_reset`,
`two_iterations_composites_are_live_together_and_distinct`,
`nesting_a_composite_into_a_flat_one_copies_its_bytes_inline`.

**These three are the only failures that survive at a coherent narrow width** (word and address both
sixteen bits), which is what distinguishes them from group I below.

### G. The expectation is a 64-bit constant (1)

`vm::tests::uncaptured_addition_yields_the_low_word_not_the_high` expects
`-9223372036854775808`, which is `i64::MIN` and not representable in a sixteen-bit word. The project
instructions already record this test as the reason `--all-features` fails.

### H. The mutation target does not exist at a narrow word (1)

`c2_flat_text_field_offset_overrun_rejected` fails on its own guard, `expected a flat Text field
access to mutate`, before reaching what it verifies. The compiler keeps `Text` boxed when the word is
narrower than a host pointer, so no flat `Text` field access is emitted to mutate:

```rust
if matches!(kind, ScalarKind::Text) && ti.word_bytes < core::mem::size_of::<usize>() {
    return FlatFieldForm::NotFlat;
}
```

**The guard did its job.** It was written so that a mutation test cannot pass vacuously, and here it
prevented exactly that.

## I. The one runtime defect: an opaque field's width had two answers

Five failures are not the suite's assumptions. **A composite bearing an opaque field is built and
read at two different widths**, and the disagreement produces wrong offsets for every field after the
opaque.

`opaque_bearing_flat_composites_compare_by_identity`, `decode_flat_struct_with_opaque_field`,
`tuple_with_opaque_element_flattens_and_resolves`, `tuple_with_opaque_and_trailing_scalars_offsets`,
`array_of_opaque_flattens_and_indexes`.

### The worst of them returns a wrong answer rather than an error

```
P { h: h, n: 1 } == P { h: h, n: 2 }   evaluated to true
```

Two structures differing in a `Word` field compared **equal**. Nothing in that program is
unrepresentable at sixteen bits. **A silent wrong answer is a worse outcome than any refusal in this
document**, and it is the reason this group was worth separating from the other thirty-six.

### The mechanism: one field, two authorities

`ScalarKind::Opaque.size_in_bytes` sizes the field by the **address** width, and its comment records
that as a deliberate repair of an earlier word-sizing. The compiler bakes every field offset from
that layout, and the typed verifier reconstructs shapes from it.

The runtime disagreed, in four places, each of which assumed a **word**:

| site | what it did |
|---|---|
| the construction path | rewrote the registry index to an `Int`, which is one word wide |
| the arena packer | advanced the write cursor by that `Int`'s width |
| the flat scalar read | read the index back as a word |
| the host decode of an opaque field | asked for a word's worth of bytes from a field the layout had sized |

**The default target makes the word and address widths equal, so all four agreed with the layout by
coincidence.** The two widths are selected independently, by the `narrow-word-*` and
`narrow-address-*` feature families, so they are only equal by configuration.

### The repair takes the width from the layout instead of choosing a side

Which width is *right* is a genuine question -- the field holds a registry index, which argues for a
word, while the layout calls it a host handle, which argues for an address. **That question did not
have to be answered.** Three of the four subsystems already agree on the layout as the authority: the
compiler bakes offsets from it, the typed verifier sizes operands from it, and the marshalling layer
reports field sizes from it. The runtime was the outlier.

So each of the four sites now asks `ScalarKind::Opaque.size_in_bytes` for the width rather than
assuming one. **The disagreement is removed by construction**, and if the layout's own answer is ever
revisited, the runtime follows it without further change.

An index too large for the field now aborts the pack, so the composite falls back to a boxed body.
That is slower and correct; a truncated index would have resolved to the **wrong host object**, which
no downstream bounds check can catch.

### The evidence is differential, across four width configurations

Run over the five affected test files:

| word / address | before | after |
|---|---|---|
| 8 / 8, the default | 0 failures | 0 failures |
| 2 / 8 | 8 failures | 3 |
| 8 / 2 | 3 failures | 0 |
| 2 / 2 | 3 failures | 3 |

**The three that remained in every column were group F**, the tests that read the body in eight-byte
words. They are unaffected by this repair and should be.

**The default configuration is unchanged, and that is the claim to scrutinise**, since it is the only
one anyone ships. It is supported by the full default suite passing before and after, and by the
arithmetic: where the two widths are equal, every changed expression returns what it returned before.

### What guards it, and what that guard does NOT cover

Three unit tests in `src/bytecode.rs` pack a composite with **explicitly differing** word and address
widths, so they exercise the disagreement in the default build, where the runtime's own widths are
equal and would conceal it.

**Their reach was measured by mutation, not asserted.** Sizing the packed field by the word instead
of the layout is caught by one of the three; making the size query itself answer with a word is
caught by two. **The third, which pins the refusal of an over-wide index, was NOT caught by either
mutation** and guards only that fallback.

**~~No guard covers the construction path's rewrite of the index.~~ CLOSED, and the reasoning behind
it was wrong.** This said restoring the collapse would be invisible in the default build because no
continuous-integration job selects a configuration where the two widths differ. **No job has to.**
See the section below.

`tests/composite_width_skew.rs` now drives the whole mechanism end to end at two skews, in the
default build. Its coverage was measured by reverting each of the four repaired sites in turn:

| reverted site | tests that caught it |
|---|---|
| the construction path collapsing the index to a one-word `Int` | 3 |
| the arena packer advancing by a word | 5 |
| the flat scalar read taking a word | **1** |
| the host decode asking for a word | **1** |

**Two sites are held by a single test each.** The read side is observable only by resolving the
index back to its host object, because with a handful of opaques the index fits in one byte and
every other test reads the same number at either width. **One test in that file is caught by no
mutation at all** and is documented there as a witness to the reported symptom rather than as a
guard.

## A width skew needs no feature selection, and that qualifies this document's parent

The finding above rested on an assumption worth stating plainly, because it was **false**: that
reaching a configuration where the word and address widths differ requires selecting a `narrow-*`
feature, and therefore a continuous-integration job.

**It does not.** `GenericVm<W, A, F>` is public and generic over the word, address and float types
independently, and every `Word` (`i8`, `i16`, `i32`, `i64`) and every `Address` (`u8`, `u16`, `u32`,
`u64`) is implemented unconditionally. A host-defined alias therefore reaches **any** pair of widths
in the default build. The `narrow-*` features only choose which pair the bundled `Vm` alias uses.

**And a skewed pair already ships as a named target.** `Target::embedded_8` declares an eight-bit
word with a sixteen-bit address, and the `addr_bits_log2` field's own documentation names the 6502 as
the machine it stands for. The defect this document records was reachable through a shipped
constructor, not only through a feature nobody builds.

### What this does and does not change

**For the FEATURE axis, the parent document's finding stands.** A build that omits `floats` while
enabling `verify` cannot be reached by a host alias; it needs the feature selection, and nothing
selects it. That is what left the float hole unexercised.

**For the WIDTH axis it is weaker than stated.** The behaviour is reachable from an ordinary test in
the default build, so a width-dependent defect can be guarded at no standing cost — no job, no
matrix, nothing added to every push. That is a better position than the parent document assumed, and
it was available the whole time.

**It does not make the narrow widths verified.** What is covered is one mechanism, at two skews, by
seven tests. The thirty-six failures classified above as wide-host assumptions are untouched by this,
and running the suite at a narrow width remains the project this document declines to start.

## What this document does not establish

**~~The narrow widths are still not verified.~~ CORRECTED 2026-09-08: too broad, and in the same
way twice more in this section.** What a suite cannot do is run AS A WHOLE at a narrow width, and
that is what the failures below describe. **Narrow runtimes themselves are driven by 41 tests in the
DEFAULT build**, through host-defined aliases in `tests/narrow_vm.rs` and
`tests/composite_width_skew.rs`, on every continuous-integration run. A `narrow-*` feature narrows
the bundled `Vm` alias so the whole suite runs narrow; a host alias narrows one runtime inside one
test. Conflating them overstates the gap. See
[`FEATURE_COMBINATION_SWEEP.md`](./FEATURE_COMBINATION_SWEEP.md) for the count and its derivation.

Thirty-six of the forty-one failures are the suite declining to run at a width it was not written
for. What has changed is that the failures are now understood individually rather than grouped by
name.

**Making the suite run at sixteen bits remains a project rather than an increment**, and nothing here
recommends starting it. Groups A through D would need per-test judgement about whether the premise is
genuinely void.

**Group F was the exception this paragraph named, and it is now done** (2026-09-08): its three
harness reads take the word width from the crate, so the distinct failures at `narrow-word-16` stand
at 33 rather than 41. The prediction that it was "the only group where a mechanical change would be
both small and clearly right" held -- and repairing the first assertion in one of those tests
exposed a second hard-coded figure inside it, which a single fix per test would have left.

**Only `narrow-word-16` was swept in full.** The other selectors were run over five test files, not
the suite.

**The eight-bit sentence that stood here was the same conflation a third time.** It said the
eight-bit selectors are least exercised because `tests/narrow_vm.rs` excludes them by its own
configuration attribute. That file does exclude the eight-bit FEATURES -- and it also defines an
eight-bit-word runtime and drives it in four tests, to which `composite_width_skew.rs` adds two more.
**The eight-bit SELECTOR is unexercised; the eight-bit RUNTIME is exercised by six tests in the
default build.** The exclusion and the coverage are about different things and sat one sentence
apart.

**One failure was measured twice and the first reading was wrong.** An intermediate run of the width
matrix reported seven failures at `narrow-address-16`, including two tests that had passed a moment
earlier. That run had compiled against a source file being mutated for an unrelated experiment at the
same moment. The figures above come from a serial re-run on a quiescent tree. **A measurement taken
while its subject is being edited measures neither state**, and the only reason it was caught is that
the result was implausible enough to re-examine.
