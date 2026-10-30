# Order 3 completion audit

> **Navigation** [Completion plan](./ORDER3_COMPLETION_PLAN.md) | [Handler surface plan](./ORDER3_HANDLER_SURFACE_PLAN.md) | [V0.2.X roadmap](../roadmap/V0_2_X_ROADMAP.md)


**Accepted by the operator first-hand on 2026-10-28.** The operator accepted the Order 3
first-pass gate as recorded in this audit.
## Scope and disposition

This audit records the state of Order 3, first-pass trap analysis, on
2026-10-24. Its scope is the completion plan's requirement register, the seven
acceptance rows the handler surface plan added after the operator rulings of
2026-10-20, and the three strengthening items that remained open for Order 3.
**Disposition.** Order 3 is complete for its first-pass scope on 2026-10-24,
on the local and remote evidence recorded below. The
residual limits below are part of the disposition.

The first pass covers Word and Byte arithmetic, indexing and indexed data
access, handled indexed writes, native-call host failures and composite element
reads. Other flat scalar element kinds, boxed field reads and hand-built
unreified native calls are outside it, and both analyses report them as
unsupported rather than as trap-free or possibly trapping. Full-language support
remains Order 6. The flags register that replaces in-band outcome flags is
V0.2.4 work by the operator ruling of 2026-10-20.

`BYTECODE_VERSION` remains 2, the format fingerprint remains `0x4327_63E1` and the
instruction set remains 66 opcodes. No capacity was raised and nothing was
published.

## Operator rulings applied

The rulings of 2026-10-13, 2026-10-20 and 2026-10-21 are recorded in the
completion plan and the handler surface plan. The rulings of 2026-10-24 were
given first-hand in the V0.2.X session after a survey of the code, and are
recorded in the handler surface plan. They selected reifying every native call
with a new HostError trap kind, total composite reads with a zero-filled
placeholder, and reference-compiler support for handled writes in `let`
initializers with a self-hosted refusal if mirroring was not possible.

## Requirement evidence

| Requirement | Evidence | Disposition |
|---|---|---|
| Complete first-pass fault inventory | tests/runtime_fault_census.rs, twenty-one source cases including Fixed division, with valid controls. tests/composite_index_reads.rs and tests/native_host_error.rs add composite reads and host failures. | Met for the first-pass profile. |
| Total first-pass primitive contracts | ORDER3_ARITHMETIC_CONTRACTS.md and ORDER3_BOUNDS_CONTRACTS.md, with opcode-level controls in tests/legacy_division_outcomes.rs, tests/bounds_outcomes.rs, tests/indexed_read_outcomes.rs, tests/indexed_write_outcomes.rs, tests/data_read_outcomes.rs and tests/composite_index_reads.rs. The placeholder control reads a field of a nested placeholder from hand-built code and gets zero. | Met. Opcode count 66. |
| Compiler lowering | Byte identity of the fourteen stage artifacts, focused handled and bare cases in tests/indexed_write_handlers.rs, tests/selfhost_checked_outcomes.rs and tests/composite_index_reads.rs. Covering handlers emit no fallback trap and guarded or literal handlers keep it. | Met, with the self-hosted refusals listed below. |
| Reference and Keleusma verdicts | tests/trap_analysis.rs and tests/trap_free_corpus.rs. The corpus has 570 composed trap-free modules and 399 possibly trapping controls, and both analyses agree on every one. `the_stage_owns_the_trap_decision` in src/selfhost/traps.rs is the stage-decision intervention. | Met. Strengthening item 12 addressed. |
| Host load policy | ORDER3_HOST_POLICY.md and tests/trap_policy.rs. | Met, as recorded on 2026-10-15. |
| No-check execution | ORDER3_PROVEN_EXECUTION.md and tests/proven_execution.rs. By the operator ruling of 2026-10-20 the proved checks are retained, because the project rejects undefined behaviour. The proof establishes that they cannot fail, and src/vm_operand_effect_tests.rs compares every executed instruction's stack change with the effect table the proof uses. | Met as modified by that ruling. No speedup is claimed. |
| Safety and trust boundary | Trap absence does not remove malformed-bytecode, shape, lifetime or host-contract guards. A malformed native result and arena exhaustion remain runtime faults outside the trap classification. | Met. The boundary is stated, not removed. |
| Compatibility and resource analysis | Stack effects, both memory analyses, both text analyses, both typed passes, the wire codecs and the documentation agree with every changed contract. tests/stale_contract_artifacts.rs pins what pre-change artifacts now do. | Met, with the stale-artifact hazard recorded below. |
| Write handler grammar and lowering | tests/indexed_write_handlers.rs, including the `let`-initializer form. | Met. |
| Short circuit | `bare_writes_evaluate_the_index_first_and_short_circuit_a_bad_index` and `a_let_bound_write_with_a_bad_index_skips_its_value`. | Met. |
| Native-call handlers | tests/native_host_error.rs. Unregistered natives are rejected before execution. A runtime host failure reaches a `host_error` arm or the default HostError trap, which surfaces the host's own error. | Met in the reference compiler. The self-hosted compiler has no native calls. |
| Composite reads | tests/composite_index_reads.rs. A bare read reaches the explicit bounds trap and a handler sees no placeholder. | Met. The self-hosted compiler refuses the handled form. |
| FixedDiv totalization | tests/runtime_fault_census.rs and tests/trap_lowering.rs, as recorded on 2026-10-22. | Met. |
| Trap analysis of every new form | tests/trap_analysis.rs, tests/native_host_error.rs, tests/composite_index_reads.rs and tests/trap_free_corpus.rs. | Met. |

## Defects found and repaired in this increment

1. **Understated memory bound for handled native calls.** `verify::wcmu_region`
   read the raw count byte of a native call, reify flag included, as the argument
   count, and counted one pushed slot. A one-argument handled call moved the
   offset by 1 - 129, so nothing after it could raise the stack peak. The
   Keleusma transport and a test's copy of it carried the same reading, so the two
   analyses agreed on the wrong value. Pinned by
   `the_memory_bound_counts_both_slots_of_a_reified_call`.
2. **Raw count in the text analyses.** The reference text walk and both Keleusma
   mirrors popped the raw count and pushed one slot for a native call.
3. **Trap payload pops in the linear text walks.** The text walks are linear and
   popped a trap's payload from the fall-through stack. Once a native call gained
   its HostError tail, the text it returned was removed before a comparison, and
   two text-cost tests caught it. The same fault existed latently for the bounds
   trap. A trap now has no stack effect in the text walks.

4. **One-element equality constant order.** Making nested composite reads total
   gave every compiler-generated element-equality loop a bounds tail. The
   self-hosted emitters interned that tail's `1` after the loop's result
   constants, while the reference interns it after the element indices. Arrays of
   two or more elements already hold `1` as an index, so only a one-element
   nested array diverged, which the heavy tier caught. The emitters now intern it
   in the reference's order, pinned by
   `one_element_array_equality_matches_the_self_hosted_compiler`.

## Residual limits

- Flat scalar element reads other than Word and Byte, boxed field reads and
  unreified native calls remain unsupported by both analyses.
- The self-hosted compiler refuses handled composite reads, handled multi-index
  writes, Fixed division and every native call. Each is a refusal, not a
  divergence.
- The self-hosted parser alone accepts a bare indexed write as a `let`
  initializer. The shipping path refuses it only through the reference
  cross-check. This leniency predates this increment.
- An artifact built before an in-place contract change may verify and run with
  changed meaning. tests/stale_contract_artifacts.rs shows this for legacy `Div`
  and flat Word reads. The operator accepted the hazard for the unpublished
  cycle.
- The nested placeholder's allocation is charged on every composite read, which
  overstates the bound for reads that never fail.
- The trap-analysis comparison runs only under the self-host feature.

## Verification receipts

A full local matrix ran between 21:27 and 22:29 UTC on 2026-10-24, on the tree
of merge commit 5eca731f, which takes v0.2.3 with the kaizen branch from pull
request 502, plus this increment's working changes. No tracked file changed
during the run, and the only later tracked edits are this receipt and the
process records. Every step used nextest in the Cargo release profile with
`--no-fail-fast`, except formatting, Clippy and the documentation build.

| Command | Result |
|---|---|
| `cargo fmt --all --check` | Exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | Exit 0, with Checking output |
| `cargo clippy --tests --features signatures,shell,self-host -- -D warnings` | Exit 0, with Checking output |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | Exit 0 |
| `cargo nextest run --cargo-profile release --workspace` | Exit 0, 3502 passed, 2 skipped |
| `cargo test --release --workspace --doc` | Exit 0, 12 passed, 3 ignored |
| `cargo nextest run --cargo-profile release -p keleusma --features self-host` | Exit 0, 3242 passed, 2 skipped |
| `cargo nextest run --cargo-profile release -p keleusma --no-default-features` | Exit 0, 333 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures` | Exit 0, 2490 passed |
| `cargo nextest run --cargo-profile release -p keleusma --features signatures,shell` | Exit 0, 2507 passed |
| In compiler/, `cargo fmt --all -- --check`, deny-warning `cargo clippy --all-targets` and `cargo test --release` | Exit 0, 86 passed |

The workspace population includes the heavy self-host binaries, so stage byte
identity, the fixed points and the command stage corpus ran on this tree. This
satisfies strengthening item 11 for this tree in release. The release profile
does not establish a debug pass, which remote continuous integration supplies.

Not run here: Miri. This increment adds no unsafe code. The nested placeholder is
built through the existing `FlatComposite::build_in_arena`. The continuous
integration Miri job covers the arena and the undefined-behaviour pin.
`cargo clippy -p keleusma --lib --tests --no-default-features` still fails on dead
code in tests/op_tag_tables.rs, a configuration continuous integration does not
run and a defect that predates this increment.

Remote continuous integration then ran PR 506 at bcb7c6fa as workflow run
[40143090487](https://github.com/sgeos/keleusma/actions/runs/40143090487), with
Book run 40143090457, between 23:31 and 00:13 UTC on 2026-10-24 and 2026-10-25.
Every job passed and only the Pages deployment was skipped. The test jobs use the
debug test profile at opt-level 1, which supplies the debug pass. The pull request
merged into v0.2.3 as a20011dd. Two earlier runs on the same branch failed only on
the generated book: the instruction-set page, its translation template and its
Japanese catalogue, each regenerated in turn.

| Job | Nextest summary |
|---|---|
| Test, the workspace without the self-host binaries | 2643 passed |
| Test, self-host feature, shards 1 to 4 | 794, 796, 771 and 881 passed, 3242 in total |
| Test, no default features | 333 passed |
| Test, signatures feature | 2490 passed |
| Test, broad features | 2507 passed |
| Test, keleusma-wire and keleusma-bench | 19 and 6 passed |

The first heavy run on an earlier tree, 857 of 859, found two failures, both
caused by this increment and both repaired before the matrix: the one-element
equality constant order and the moved fused-compile record counts.
