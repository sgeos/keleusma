# C5 review of the self-hosted validator

> **Navigation** [Decisions](./README.md) | [Order 2 audit](./ORDER2_COMPLETION_AUDIT.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)

Reviewed on 2026-11-01 and 2026-11-02 on `docs/v02-c5-validator-review`, cut from `v0.2.3` at
`7c76d771`.

## Verdict

**C5 is not met.** Two soundness residuals remain open, each pinned by a test that asserts
the present disagreement, and each listed below with an estimate. Three soundness defects
were found and repaired in this pass. Over the mutation corpus of 10,703 corrupted modules
the self-hosted validator now accepts none that the reference rejects, where before the
repair it accepted 41. That is a measured result about that corpus and is not a proof for
arbitrary bytecode. The shipping reference gate is unchanged and remains necessary.

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
| `TextFieldMismatch` | `check_text_field`, `check_text_elements` | same names | Mapped, at most four offsets, see residual 2 |
| `BranchHeightMismatch` | `join_stacks` | `resolve_pending` | Mapped |
| `LoopNotNeutral` | `interp_region` | `resolve_pending` | Mapped by height. Inferred equivalent, since the floor keeps every entry below the loop's entry height |
| `LocalTypeMismatch`, `CallArgMismatch` | `apply_op` | `shapes_compat` | Mapped |
| Shape joins at `EndIf` and after a loop | `join_ends`, `join_all`, the loop-head fixpoint | `resolve_pending` | Repaired at `EndIf`, partial after a loop, see residual 1 |
| Call graph acyclic | `topological_call_order` | `call_graph.rs` with `analyze.kel` | Mapped |
| Region nesting within the walk depth | `wcmu_region_at` | `analyze.kel` | Mapped |
| Loop iteration bound extractable | `extract_loop_iteration_bound` | `analyze.kel` | Mapped |
| Text heap bounded | `module_chunk_text_analyses` | `analyze.kel` | Mapped |
| Stream WCMU within the arena | `enforce_arena_capacity` | `resources.rs` with `analyze.kel` | Mapped |

## Mutation results

All counts are measured, in release builds with an empty `RUSTC_WRAPPER`, by
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

The whole corpus costs more than a test can spend in a continuous-integration shard, so it is
`#[ignore]`d as `c5_the_whole_mutation_corpus` and is the review's instrument. The test that
runs by default, `c5_the_self_hosted_validator_accepts_no_sampled_mutant_the_reference_rejects`,
takes one mutant in sixteen of each family and every mutant the repair pinned. It judged 712
mutants after the repair, with none unsound. `c5_the_repaired_mutants_are_refused` holds the 41
as failing-first controls. All three were measured to pass after the repair, and the 41 were
measured to be accepted before it.

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

## Residual gaps

### Open soundness residuals

1. **After a loop the typed stage forgets the locals the body writes and every operand entry
   above the loop's entry height.** The reference sets them to the join of the break states
   of its loop-head fixpoint, which can keep a shape. Witness
   `c5_residual_a_local_written_in_a_loop_is_forgotten_after_it` in
   `tests/selfhost_validator.rs`, measured. Joining the first iteration's break states would
   be more precise than the reference, so the repair is the reference's fixpoint itself,
   rerunning a loop body until its head locals are stable with the break states accumulated
   in the pools. Estimate, inferred, two to four working days including measuring the stage's
   new cost on the fifteen-stage corpus, since a nested loop then costs a product of
   iteration counts.
2. **The typed stage records at most four `Text` offsets of a body's layout.** A body with
   five or more `Text` fields has an unknown layout in the stage and a known one in the
   reference, so a `Text` read at an offset holding no text is refused by the reference and
   deferred by the stage. Witness
   `c5_residual_a_body_with_more_than_four_text_fields_defers_its_text_reads`, measured with
   four fields agreeing and five disagreeing. Repair is variable-length layout storage in the
   stage's entry, local and pool tables. Estimate, inferred, one working day.

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
   stage, and a pool past 8,192 entries are unavailable analysis. They narrow the admitted
   set and are not unsound. The corpus measured 16 such cases.
6. **The typed stage does not distinguish scalar kinds of one size.** It records a scalar's
   size and whether it is `Text`, so a join of a `Bool` and a `Byte`, or of an `Int` and a
   `Fixed`, keeps a scalar where the reference forgets it. The stage could then refuse a field
   read the reference defers. This is an inferred over-rejection, not observed in the corpus,
   and is not a soundness gap.
7. **The corpus is small programs.** Agreement over these mutants is evidence about these
   mutants. The stage corpus and the repository sources are covered by the existing
   agreement tests of `tests/selfhost_full_language.rs`, `tests/selfhost_admission.rs` and
   `tests/selfhost_resource_admission.rs`, which this review reran and did not extend.

## Verification

Measured in this pass, with an empty `RUSTC_WRAPPER`, on the tree this record is committed
with.

| Command | Result |
|---|---|
| `cargo test --release -p keleusma --features self-host --test selfhost_validator c5_the -- --include-ignored` | 3 passed, the whole corpus in 2,577 s |
| `cargo nextest run --workspace --release -E 'binary(~selfhost)'` | 1,020 run, 1,019 passed, 3 skipped. The one failure was the residency pin of `verify_typed.kel` in `the_fused_compile_holds_one_group_not_the_program`, which moved from 2,142 records and a peak of 557 to 3,066 and 570, was re-pinned, and then passed alone |
| `cargo nextest run --workspace --profile quick` | 2,902 run, 2,902 passed, 1 skipped |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings`, with `-p keleusma-cli --tests` and `--no-default-features -p keleusma --tests` | Passed, each checking `keleusma` |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` and `cargo fmt --check` | Passed |

The sampled gate took 97 s in the release self-host tier. Its cost in the debug profile of a
continuous-integration shard is not measured. Remote continuous integration and every
configuration other than the default features with `self-host` are not verified here.
