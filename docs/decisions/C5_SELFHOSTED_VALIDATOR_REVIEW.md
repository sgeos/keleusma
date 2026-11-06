# C5 review of the self-hosted validator

> **Navigation** [Decisions](./README.md) | [Order 2 audit](./ORDER2_COMPLETION_AUDIT.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

Reviewed on 2026-11-01 and 2026-11-02 on `docs/v02-c5-validator-review`, cut from `v0.2.3` at
`7c76d771`. The two open soundness residuals were repaired on 2026-11-04 on
`fix/v02-c5-residuals`, cut from `v0.2.3` at `ff48e35c`, and the verdict below was revised
then.

## Verdict

**C5 is met for the corpus scope.** The criterion is that the self-hosted validator never
accepts a module the reference `verify` or admission rejects. Over the enlarged mutation corpus
of 13,307 corrupted modules it accepts none that the reference rejects, and it rejects none
that the reference accepts. Both counts are measured. **The scope is the corpus and nothing
wider.** Agreement over these mutants is evidence about these mutants and is not a proof for
arbitrary bytecode, and the residuals listed under "Other residuals" stay open, among them the
common-mode transport tables that no differential can see. The shipping reference gate is
unchanged and remains necessary.

The first pass, on 2026-11-02, recorded "C5 is not met". It found and repaired three soundness
defects, which took the corpus of that day from 41 unsound mutants to none, and it left two
soundness residuals open. The second pass, on 2026-11-04, repaired both residuals and a third
gap of the same layout family, and is recorded under "The residual repair" below.

## What C5 is

The operator ruled C5 first-hand in V0.2.X's session on 2026-10-30, as "Keep the Rust
reference for now. Note that we need the review before V0.3.0." The V0.3.X line raised it as
its question 27, and on 2026-10-31 the operator confirmed first-hand in V0.2.X's session that
it is a V0.3.0 condition for both lines with V0.2.X owning the review, recorded as "Yes,
V0.2.X owns it" in `docs/process/REVERSE_PROMPT.md`. No record states what the review must
establish. This review therefore applied the criterion its assignment gave. The self-hosted
validator must be sound against the reference verifier, meaning it never accepts a module the
reference rejects, and it should be complete against the specification, meaning it does not
reject a module the reference accepts. The first is the condition. The second is reported.

The reference is `verify` in `src/verify.rs` with `src/verify_init.rs` and
`src/verify_typed.rs`, and admission is `verify` followed by `verify_resource_bounds`. The
self-hosted validator is `verify_module_via_kel` and `admit_module_via_kel`, which drive the
stages `verify_structural.kel`, `verify_depth.kel`, `verify_yield.kel`, `verify_typed.kel`,
`verify_datalayout.kel` and `analyze.kel`. `verify_types.kel` is the source type checker of
Order 1 and is outside this review, as is the trap analysis of Order 3.

## Method

1. Every check the reference makes was listed from its source and mapped to the stage that
   decides it, from a reading of both. The table below is that reading.
2. A mutation corpus was written, `tests/selfhost_validator/c5_corpus.rs`. It compiles
   thirteen small programs chosen for opcode and table variety and corrupts each by every
   applicable mutation in eleven families. Every mutant goes to both validators at `verify`
   and at admission. A mutant the self-hosted validator accepts and the reference rejects is
   unsound, and the test fails on any one it does not pin. The generator follows
   `tests/hostile_module_mutation.rs`, which never reaches the self-hosted validator.
3. Each unsound mutant was traced to its cause, the cause was repaired in the stage, and the
   mutants were kept as failing-first controls of the repair.
4. Disagreements that the corpus does not reach but the reading found were written as witness
   tests.

The earlier Gate 3 corpus of `tests/selfhost_full_language.rs`, 323 modules in seven
families, found on 2026-10-26 that the self-hosted validator accepted every module whose
final `Return` was removed. That defect was repaired then in `verify_depth.kel`, and the
corpus still passes. This review's corpus is larger and reaches the typed and table checks
the earlier one did not.

## Check inventory

Measured facts are the mutation counts later in this record. The mapping in this table is
inferred from reading the code and is not verified per row by a dedicated test, except where
a row names one.

| Reference check | Reference site | Self-hosted site | Status |
|---|---|---|---|
| Float opcode on a build without `floats` | `check_no_float_opcodes` | none | Missing, unreachable today, see residual 4 |
| `If`, `Else` and `Loop` targets forward and in range | `check_forward_control_flow_targets` | `verify_structural.kel` `run` | Mapped |
| `If` target in bounds, Else body start or EndIf | `verify_chunk` pass 1 | `verify_structural.kel` `step`, `handle_else`, `handle_endif` | Mapped |
| `Else` matches an open `If` and targets an `EndIf` | pass 1 | `handle_else` | Mapped |
| `EndIf` closes an `If`, `EndLoop` closes a `Loop` | pass 1 | `handle_endif`, `handle_endloop` | Mapped |
| `EndLoop` back edge and `Loop` exit targets exact | pass 1 | `handle_endloop` | Mapped |
| `Break` and `BreakIf` inside a loop, in range, at its exit | pass 1 | `handle_break` | Mapped |
| No unclosed block at chunk end | pass 1 | `run` | Mapped |
| Data slot declared and in range | pass 1 | `check_opbounds` kind 1 | Mapped |
| Data range within the layout | pass 1 | kind 2 | Mapped |
| Constant indices of `Const`, `IsStruct`, boxed `GetField`, `IsEnum` | pass 1 | kinds 3 and 4 | Mapped |
| Call target in range and arity within callee locals | pass 1 | kind 5 | Mapped |
| Native index within the native table | pass 1 | kind 9 | Mapped |
| Fixed-point fraction bits below the word width | pass 1 | kind 6 | Mapped |
| Local slot in range | pass 1 | kind 7 | Mapped |
| Boxed struct or enum template in range | pass 1 | kind 8 | Mapped |
| Block-type marker profile of Func, Reentrant and Stream | pass 2 | `check_block_type` | Mapped |
| Always-yielding fixpoint | `compute_always_yielding` | `verify_yield.kel`, rounds scheduled by `yield_fixpoint.rs` | Mapped |
| Productivity of every Stream to Reset path | pass 3 | `verify_yield.kel` | Mapped |
| Operand-stack underflow | `verify_stack_depth` | `verify_depth.kel` | Mapped, with a 32-level nesting cap |
| Definite initialization of locals, rule 1 | `verify_init.rs` | `verify_depth.kel` | Mapped |
| Definite initialization of private data, rule 2 | `verify_init.rs` | `verify_depth.kel`, callee order by `validation.rs` | Mapped |
| Shared slots a contiguous prefix with one layout entry each | `validate_data_layout` | `verify_datalayout.kel` | Mapped |
| Shared slot within the shared buffer | `validate_data_layout` | `verify_datalayout.kel` | Mapped |
| Private composite layout ascending and in range | `validate_data_layout` | `verify_datalayout.kel` | Mapped |
| Typed `StackUnderflow` and `LoopFloorBreach` | `apply_op`, `check_floor` | `verify_typed.kel` `apply_typed` | Mapped |
| `OffsetOutOfBounds`, `ExpectedComposite` | `check_flat_scalar`, `check_flat_nested` | `check_flat_extent` | Mapped, precision repaired, see defects 1 and 2 |
| `ArrayStrideMismatch` | `check_flat_array_stride` | `check_stride` | Mapped |
| `NewCompositeSizeMismatch` | `apply_op` | `apply_typed` kind 6 | Mapped, precision repaired, see defect 3 |
| `EnumBodySizeMismatch` | `apply_op` | `eb_check` | Mapped |
| `TextFieldMismatch` | `check_text_field`, `check_text_elements` | same names | Mapped. Repaired on 2026-11-04 with variable-length layouts, see residual 2 |
| `BranchHeightMismatch` | `join_stacks` | `resolve_pending` | Mapped |
| `LoopNotNeutral` | `interp_region` | `resolve_loop` | Mapped by height. Inferred equivalent, since the floor keeps every entry below the loop's entry height |
| `LocalTypeMismatch`, `CallArgMismatch` | `apply_op` | `shapes_compat` | Mapped |
| Shape joins at `EndIf` and after a loop | `join_ends`, `join_all`, the loop-head fixpoint | `resolve_pending`, `resolve_loop` | Repaired at `EndIf` on 2026-11-01 and after a loop on 2026-11-04, see residual 1 |
| Call graph acyclic | `topological_call_order` | `call_graph.rs` with `analyze.kel` | Mapped |
| Region nesting within the walk depth | `wcmu_region_at` | `analyze.kel` | Mapped |
| Loop iteration bound extractable | `extract_loop_iteration_bound` | `analyze.kel` | Mapped |
| Text heap bounded | `module_chunk_text_analyses` | `analyze.kel` | Mapped |
| Stream WCMU within the arena | `enforce_arena_capacity` | `resources.rs` with `analyze.kel` | Mapped |

## Mutation results

These are the first pass's counts, over its 13 compiled bases. The counts of the residual
repair, over all 18 bases, are under "The residual repair" below. All counts are measured, in release builds with an empty `RUSTC_WRAPPER`, by
`cargo test --release -p keleusma --features self-host --test selfhost_validator -- --include-ignored`.

| Measure | Before the repair, at `7c76d771` | After the repair |
|---|---|---|
| Mutants | 10,703 | 10,703 |
| Rejected by the reference `verify` | 8,589 | 8,589 |
| Rejected by reference admission | 8,788 | 8,788 |
| Accepted by the self-hosted validator and rejected by the reference | 41 | 0 |
| Rejected by the self-hosted validator and accepted by the reference | 0 | 0 |
| Unavailable self-hosted analysis | 16 | 16 |
| Panics in either validator | 0 | 0 |
| Wall time | 1,974 s | 2,577 s, on a machine shared with other builds |

The 41 unsound mutants were all refused by the reference's typed pass. By family they were
16 operand rewrites, 17 substitutions, 5 transpositions, 2 signature mutations and 1
access-kind rewrite. The 16
unavailable analyses are the chunks given 300 locals, past the typed stage's capacity of 256,
which is unavailable analysis and not a verdict.

The per-family counts after the repair are these.

| Family | Mutants | Reference `verify` rejects | Reference admission rejects |
|---|---|---|---|
| operand | 1,242 | 925 | 943 |
| access kind | 39 | 11 | 11 |
| substitution | 8,122 | 6,884 | 7,054 |
| deletion | 282 | 244 | 244 |
| duplication | 282 | 224 | 224 |
| transposition | 266 | 168 | 179 |
| chunk metadata | 129 | 86 | 86 |
| signature | 264 | 18 | 18 |
| enum layout | 17 | 4 | 4 |
| native shape | 7 | 1 | 1 |
| data layout | 53 | 24 | 24 |

The data-layout row was measured against the slot, shared-layout and private-placement tables.
Encoding F replaced those tables with one record per declared field, and the data-layout
mutations were ported to it. Each field's visibility is flipped and each field is removed, each
shared field's offset and kind are rewritten, and each pooled private field's first slot and
offset are rewritten, at the boundary values the tables used. Measured on the merged tree in a
release build with the family alone, the ported family generates 51 mutants, the reference
rejects 25 at `verify` and 25 at admission, and none is unsound, over-rejected or unavailable.
The other rows were not re-measured after the port.

The whole corpus costs more than a test can spend in a continuous-integration shard, so it was
`#[ignore]`d as `c5_the_whole_mutation_corpus` and was the review's instrument. The test that
ran by default, `c5_the_self_hosted_validator_accepts_no_sampled_mutant_the_reference_rejects`,
took one mutant in sixteen of each family and every mutant the repair pinned. It judged 712
mutants after the repair, with none unsound. `c5_the_repaired_mutants_are_refused` holds the 41
as failing-first controls. All three were measured to pass after the repair, and the 41 were
measured to be accepted before it. Since 2026-11-06 continuous integration runs the whole
corpus as sixteen partitions and the sample is retired, see "The corpus in continuous
integration".

## Defects found and repaired

All three were in `src/selfhost/kel/verify_typed.kel`, and one needed a transport change in
`typed_desc` in `src/selfhost/mod.rs`.

1. **A loop forgot every local.** The stage set every local to unknown at a `Loop`, where the
   reference checks the body's first iteration from the entry locals. A field read of a local
   the loop never writes was therefore deferred where the reference refused it. The body is
   now checked from the entry state, and after the loop only the locals the body writes are
   forgotten. A forward pass fills, for each `If` and `Loop`, the set of locals its region
   writes. The claim that the reference's later iterations refuse nothing the first does not
   is an inference from the monotonicity of its checks, which defer on an unknown shape, and
   is not proven.
2. **A join forgot every local and every operand entry.** At an `EndIf` the stage set every
   operand entry and every local to unknown, where the reference keeps a shape both paths
   agree on. An `If` now saves its entry stack and the entry shapes of the locals its region
   writes in two last-in-first-out pools, the else-arm starts from that entry state, and the
   `EndIf` joins entry by entry by the reference's rule. A pool past its 8,192 entries is the
   frame capacity, unavailable analysis rather than a verdict.
3. **A native's error flag was unknown.** A reified native call pushes its value and a `Word`
   flag. The stage pushed the flag as unknown, so a flat construction packing it skipped its
   size check. The host now transports the word width for a native call and the stage pushes
   the flag as a `Word` scalar.

A cross-join witness that `keleusma-cli/tests/validator.rs` pinned as a disagreement since the
Order 2 audit is now refused by both validators, and the test now pins residual 1's witness in
its place, so the command's disagreement path keeps its control.

## The residual repair

Made on 2026-11-04 in `src/selfhost/kel/verify_typed.kel`, with one transport change in
`seed_typed_shared` in `src/selfhost/mod.rs`, which now passes the chunk's local count.

1. **The loop fixpoint, residual 1.** A loop body now runs from a head state, first the entry
   state. When a pass falls through, the locals the region writes are joined into the head, and
   while the head changes the body runs again from it. The pass counter stops at the reference's
   cap of the local count plus two, past which the head's written locals are forgotten and one
   final pass runs, as `invalidate_written_locals` does in the reference. Every break of a pass
   is joined into an accumulator of its loop, holding the operand entries above the loop's entry
   height and the written locals, in a break pool that is last in first out as loops nest. The
   state after the loop is that join of the last pass, or with no break the body's end state, or
   with neither the head, which is the reference's `join_all(loop_breaks)`, `body_end` and
   `head`. Termination is verifiable by inspection, since each loop runs at most `local_count + 4`
   passes per entry and the whole walk is bounded by the stage's step budget of 32,768, past
   which the result is unavailable analysis. The step budget was not raised, and no chunk of the
   self-host tier exhausted it, measured.
2. **Variable-length text layouts, residual 2.** An entry's `Text` offsets are now a record in a
   layout heap of 65,536 words, a count and the ascending offsets, and entries hold the record's
   index. A loop's restart frees every record its previous pass made, since a join keeps the
   older record whenever it keeps a layout, so no state that survives the restart refers to a
   freed record. A record past the heap is the frame capacity, cause 2, which is unavailable
   analysis and never a verdict, pinned by
   `c5_a_layout_heap_past_its_capacity_is_unavailable_analysis`.
3. **The element layout, a gap of the same family that the first pass did not list.** An element
   read from an array of flat bodies pushed an unknown layout, where the reference's
   `element_text_layout` gives the layout every element shares. A `Text` read of such an
   element's `Word` field was refused by the reference and accepted by the stage, measured on
   `ff48e35c`. The stage now computes the shared layout. It relies on a known layout's offsets
   being ascending, which holds by construction since a body packs its elements in order and
   each element's offsets lie within it. That invariant is argued from the code and is not
   checked at run time.

The corpus gained five bases for these gaps. `text_fields` holds six `Text` fields and
`text_array` an array of bodies with text. The language binds each name once, so no compiled
program reaches a local a loop writes that is read after it, and three hand-built bases,
`loop_local`, `loop_widening` and `loop_operand`, are built by `loop_module` in the corpus file.
They take the corpus from 10,703 mutants to 13,307.

The two witness tests now assert agreement. They are
`c5_a_local_written_in_a_loop_keeps_the_join_of_its_break_states` and
`c5_text_layouts_of_any_width_agree_with_the_reference`, the latter at 1, 4, 5, 16 and 40
fields. `c5_a_loop_exit_agrees_with_the_reference_fixpoint` adds the widening case, in which a
join of the first pass's breaks alone would refuse a read the reference defers. It is a check
that the fixpoint and not one pass was implemented. `c5_an_array_element_keeps_the_layout_its_elements_share`
pins the element layout. Every one of these was measured to fail on `ff48e35c` before the
repair. The 20 corpus mutants the stage accepted before the repair are kept as failing-first
controls in `REPAIRED_RESIDUALS`. 18 are of the loop bases and 2 are of the text bases.

`reference_disagreement_prevents_successful_admission` in `keleusma-cli/tests/validator.rs`
pinned residual 1's witness as its disagreement control. Both validators now refuse that
module, and the test now drives residual 6 in its place, a `Bool` and a `Byte` joined at an
`EndIf`, which the reference accepts and the stage refuses. The command's disagreement path
therefore keeps a control.

| Measure, whole corpus | Before the repair, on `ff48e35c` | After the repair |
|---|---|---|
| Mutants | 13,307 | 13,307 |
| Rejected by the reference `verify` | 10,794 | 10,794 |
| Rejected by reference admission | 11,160 | 11,160 |
| Accepted by the self-hosted validator and rejected by the reference | 20 | 0 |
| Rejected by the self-hosted validator and accepted by the reference | 0 | 0 |
| Unavailable self-hosted analysis | 21 | 21 |
| Panics in either validator | 0 | 0 |
| Wall time | 4,544 s | 2,739 s, with five other tests, on a machine shared with other builds |

The 20 unsound mutants before the repair were 10 operand rewrites, 4 access-kind rewrites, 5
substitutions and 1 transposition. The 21 unavailable analyses are the chunks given 300 locals,
one per chunk of the 18 bases, past the typed stage's capacity of 256. The 13 bases of the
first pass account for the 10,703 of the earlier table and still give 0 unsound mutants. The
sampled gate judged 875 mutants before the repair, with 2 unsound, and 893 after it, with none
unsound, none over-rejected and none unavailable. It reaches more after the repair because it
also takes every mutant `REPAIRED_RESIDUALS` names.

## The corpus in continuous integration

On 2026-11-04 the operator ruled on running the whole corpus, verbatim, "A. Consider sharding
if reasonable and possible." Option A kept the one-in-sixteen sample in continuous integration
and ran the whole corpus before each release. The sharding was measured and adopted on
2026-11-06 on `ci/v02-c5-corpus-shards`, cut from `v0.2.3` at `cecd93db`.

**The profile.** The self-host shards run an archive that `cargo nextest archive` builds in
the `test` profile, which inherits `dev` at opt-level 1 with debug assertions on. They are not
release builds, so the release figures above do not describe their cost.

**The shards before the change.** Measured from `gh run view --json jobs` and the job log of
run 42232522967, the ready run of 2026-11-06 on `3aefa398`. The four shards took 37.0, 30.2,
35.9 and 31.9 minutes of wall time. Each spent between 1,336 and 1,635 seconds in its pinned
step and between 386 and 480 seconds in its hashed step. Their tests summed to 7,432, 6,518,
7,256 and 6,791 test-seconds, which is between 91 and 94 percent of the two steps' wall time
times four threads. The four threads are inferred from the runner size and not read from the
log. Run 42017081168 gave 37.3, 16.4, 32.1 and 34.2 minutes, and run 42083974780 had one shard
at 137.8 minutes, 104 of them in the artifact download, so the shard times vary by run.

**The corpus in that profile.** In run 42232522967 the sample took 183.2 seconds and
`c5_the_repaired_mutants_are_refused` took 25.3 seconds. The sample judged about 893 mutants,
the count measured locally after the residual repair and not read from the log. That gives
about 0.2 seconds a mutant in a loaded shard, and about 2,600 test-seconds for the corpus in a
shard, which is an estimate from a timed fraction and not a measurement. Locally the whole
corpus was measured in the `test` profile, with
`cargo nextest run -p keleusma --features self-host --test selfhost_validator -E 'test(/c5_/)' --run-ignored all`,
on a shared machine whose one-minute load average was 5.8 at the start and 3.1 at the end.
`c5_the_whole_mutation_corpus` took 1,169 seconds, and the sixteen partitions, running beside
it on four threads, took between 66 and 78 seconds each and 1,171 test-seconds together. They
judged nine partitions of 832 mutants and seven of 831, which is 13,305, and the whole test
judged the same 13,305. The corpus is two smaller than the 13,307 of the table above, which
is consistent with the data-layout port's 51 mutants in place of 53. Over those 13,305 the
self-hosted validator accepted none the reference rejects and rejected none it accepts, 21
analyses were unavailable, and the reference rejected 10,795 at `verify` and 11,161 at
admission. A local partition is therefore about 2.3 times faster than the sample was in a
shard, so a partition in a shard is estimated at about 170 seconds.

**The decision.** One test holding the whole corpus would run for about forty minutes in a
shard and lengthen that shard by about as much. Sixteen partitions cost about what the sample
cost each, which is shorter than the longest pinned test of every shard, so no partition
lengthens a shard's critical path. Pinned four to a shard, they add about 680 test-seconds to
each shard less the sample's 183 seconds on one of them. At four threads near saturation that
is estimated at about 170 seconds of wall time per shard, under ten percent of a 30 to 37
minute shard and well inside the 25 percent bound the change was held to. Remote continuous
integration on this branch has not run, so the growth is an estimate.

**What runs.** `c5_corpus_partition_00` to `c5_corpus_partition_15` judge the mutants whose
index in the generator's order is the partition number modulo 16, and shard k pins the
partitions whose number modulo 4 is k minus 1. Each fails on a panic, on an unsound mutant
`KNOWN_UNSOUND` does not name, on a named mutant of its partition that is no longer unsound,
and on a count other than its stride assigns it. `c5_the_partitions_cover_the_whole_corpus_once`
judges no mutant. It checks that two generations agree, that every index has exactly one
owning partition, that each partition has its test function, and the whole-corpus properties
no partition can see, which are the corpus and family sizes, a reference rejection in every
family and that every `KNOWN_UNSOUND` mutant is still generated.
`the_c5_corpus_partitions_are_pinned_evenly_across_the_shards` in `tests/ci_shard_coverage.rs`
checks that each partition is pinned to exactly one shard in equal shares, and it was measured
to fail on the workflow of `cecd93db`.

**The sample is retired.** The partitions judge every mutant the sample judged, including
every mutant `REPAIRED` and `REPAIRED_RESIDUALS` name, and they reach every family. Keeping it
would judge about 893 mutants twice in every run, which the operator's ruling of 2026-10-30
against redundant testing excludes. `c5_the_repaired_mutants_are_refused` stays, because it
asserts more than soundness, that the self-hosted validator rejects each repaired mutant
rather than finding its analysis unavailable. `c5_the_whole_mutation_corpus` stays ignored as
the one-run local check.

## Residual gaps

### Closed soundness residuals

1. **After a loop the typed stage forgot the locals the body writes and every operand entry
   above the loop's entry height.** Closed on 2026-11-04, see "The residual repair".
2. **The typed stage recorded at most four `Text` offsets of a body's layout.** Closed on
   2026-11-04, with the element layout of the same family, see "The residual repair".

### Other residuals

3. **Shared transport tables are common mode.** `op_depth_effect`, `ScalarKind::size_in_bytes`
   and the decoding of the module are used by both validators, and `analyze_class`,
   `structural_opbounds` and `typed_desc` are host transport the stages trust. A defect in
   one of them is invisible to a differential and to this corpus. Not measured.
4. **The float-opcode check has no self-hosted counterpart.** The reference refuses
   `IntToFloat` and `FloatToInt` on a build without `floats`. The `self-host` feature does not
   build without `floats`, measured as a compile error at the `ConstValue::Float` arm in
   `src/selfhost/mod.rs`, so the gap is unreachable today. It becomes a soundness gap if that
   configuration is ever made to build.
5. **Capacities are narrower than the reference.** A chunk past 8,192 operations, a typed
   chunk past 256 locals, a nesting past 32 levels in the depth stage or 64 in the typed
   stage, a pool past 8,192 entries, a layout heap past 65,536 words and a walk past 32,768
   steps are unavailable analysis. They narrow the admitted set and are not unsound. The
   corpus measured 21 such cases, all of them the 256-local capacity.
6. **The typed stage does not distinguish scalar kinds of one size.** It records a scalar's
   size and whether it is `Text`, so a join of a `Bool` and a `Byte`, or of an `Int` and a
   `Fixed`, keeps a scalar where the reference forgets it. The stage could then refuse a field
   read the reference defers. This is an over-rejection, not a soundness gap. It is not
   observed in the corpus, and it is observed in the hand-built module that
   `keleusma-cli/tests/validator.rs` now uses as its disagreement control. Its loop analogue
   could also move the fixpoint's pass count away from the reference's, which would change
   precision and not soundness. That is inferred and not measured.
7. **The corpus is small programs.** Agreement over these mutants is evidence about these
   mutants. The stage corpus and the repository sources are covered by the existing
   agreement tests of `tests/selfhost_full_language.rs`, `tests/selfhost_admission.rs` and
   `tests/selfhost_resource_admission.rs`, which both passes reran and did not extend.
8. **The fixpoint's cap path is not exercised by a test.** The stage copies the reference's cap,
   the local count plus two widening passes, and its forgetting final pass. No module in the
   corpus or the tests reaches the cap. It is inferred, and not proven, that no module reaches
   it, since a slot's shape changes at most twice and a change travels one local per pass, so a
   head stabilizes within about as many passes as the loop writes locals. If the stage and the
   reference ever disagreed there, a cap reached by only one of them would change precision and
   not soundness, since a later pass starts from a head that the earlier one refines.

## Verification

Measured for the partitions on 2026-11-06, with an empty `RUSTC_WRAPPER`, on the tree this
record is committed with.

| Command | Result |
|---|---|
| `cargo nextest run -p keleusma --features self-host --test selfhost_validator -E 'test(/c5_/)' --run-ignored all --no-fail-fast` | 24 passed. The sixteen partitions judged 13,305 mutants in 1,171 test-seconds, and the whole corpus judged 13,305 in 1,169 s |
| `c5_the_partitions_cover_the_whole_corpus_once` with `in_partition` changed to `index % PARTITIONS <= p` | Failed, reporting mutant 0 in all sixteen partitions |
| `cargo nextest run -p keleusma --test ci_shard_coverage` with the workflow of `cecd93db` | `the_c5_corpus_partitions_are_pinned_evenly_across_the_shards` failed, naming each partition as pinned nowhere |
| `cargo nextest run -p keleusma --features self-host --test selfhost_validator --test ci_shard_coverage` | 41 passed, 1 skipped, the whole corpus |
| `cargo nextest run --workspace --profile quick --no-fail-fast` | 2,944 run, 2,944 passed, 1 skipped |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` and `cargo clippy --no-default-features -p keleusma --tests -- -D warnings` | Passed, each checking `keleusma` |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` and `cargo fmt --check` | Passed |

Remote continuous integration has not run on this change, so the shard growth is estimated
and not measured. The release-profile heavy tier was not rerun, since no stage source or
`src/selfhost` file changed.

Measured in the residual repair, on 2026-11-04, with an empty `RUSTC_WRAPPER`, on the tree this
record is committed with unless a row says otherwise.

| Command | Result |
|---|---|
| The `selfhost_validator` release binary with `--include-ignored c5_`, built from the stage at `ff48e35c` with this pass's tests and corpus | Before. 5 of the new residual tests failed at their self-hosted assertions, the whole corpus failed with 20 unsound mutants and the sampled gate failed with 2 |
| The same binary built from this tree, which is `cargo test --release -p keleusma --features self-host --test selfhost_validator c5_ -- --include-ignored` | After. 8 passed, the whole corpus in 2,739 s |
| `cargo test --release -p keleusma --features self-host --test selfhost_validator -- --skip c5_the` | 12 passed, 3 filtered |
| `cargo nextest run --workspace --release -E 'binary(~selfhost)'` | 1,024 run, 1,023 passed, 3 skipped. The one failure was the residency pin of `verify_typed.kel` in `the_fused_compile_holds_one_group_not_the_program`, which moved from 3,066 records and a peak of 570 to 3,844 and 604, was re-pinned, and then passed alone. The byte-identical self-compile of `verify_typed.kel` passed in that run |
| `cargo nextest run --workspace --profile quick` | 2,916 run, 2,916 passed, 1 skipped |
| `cargo test --release -p keleusma-cli --test validator reference_disagreement` | 1 passed |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings`, with `-p keleusma-cli --tests` and `--no-default-features -p keleusma --tests` | Passed, each checking `keleusma` |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `cargo fmt --check` and `python3 scripts/changelog_fragments.py check` | Passed |

The cost of the sampled gate in the debug profile of a continuous-integration shard was not
measured then. It was measured on 2026-11-06 from run 42232522967, see "The corpus in
continuous integration". Remote continuous integration and every configuration other than the default
features with `self-host` are not verified here.

The first pass measured the following on 2026-11-02, on the tree it was committed with.

| Command | Result |
|---|---|
| `cargo test --release -p keleusma --features self-host --test selfhost_validator c5_the -- --include-ignored` | 3 passed, the whole corpus in 2,577 s |
| `cargo nextest run --workspace --release -E 'binary(~selfhost)'` | 1,020 run, 1,019 passed, 3 skipped. The one failure was the residency pin of `verify_typed.kel` in `the_fused_compile_holds_one_group_not_the_program`, which moved from 2,142 records and a peak of 557 to 3,066 and 570, was re-pinned, and then passed alone |
| `cargo nextest run --workspace --profile quick` | 2,902 run, 2,902 passed, 1 skipped |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings`, with `-p keleusma-cli --tests` and `--no-default-features -p keleusma --tests` | Passed, each checking `keleusma` |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` and `cargo fmt --check` | Passed |
