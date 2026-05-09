# Task Log

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

Current sprint source of truth.

---

## Current Phase

**V0.0**: Bootstrap, near completion. Data segment design formalized. Implementation in progress.

## Active Milestone

None. V0.0-M6 complete. Arena extracted to standalone keleusma-arena crate. Auto-arena sizing and call-graph WCMU integration remain as P8 follow-on for V0.0-M7.

## Task Breakdown

| ID | Description | Status | Verification |
|----|-------------|--------|--------------|
| V0.0-M0-T1 | Extract crate from workspace | Complete | cargo test passes |
| V0.0-M0-T2 | Create knowledge graph | Complete | All docs files present |
| V0.0-M1-T1 | Block-structured ISA transition | Complete | R22 |
| V0.0-M1-T2 | Productivity verification and WCET analysis | Complete | R23 |
| V0.0-M2-T1 | For-in over arrays, tuple literals, utility natives | Complete | 216 tests pass, zero clippy warnings |
| V0.0-M2-T2 | Formal related work and citations across knowledge graph | Complete | RELATED_WORK.md present, citations applied to LANGUAGE_DESIGN, EXECUTION_MODEL, TARGET_ISA, GRAMMAR, GLOSSARY |
| V0.0-M3-T1 | Data segment design specification | Complete | R24 through R28 |
| V0.0-M3-T2 | Data segment formalization pass | Complete | RELATED_WORK Section 8, citations across architecture and design docs |
| V0.0-M3-T3 | Data segment partial implementation | Complete | Parser, AST, bytecode, compiler, VM scaffolding for data blocks |
| V0.0-M3-T4 | Singular data block enforcement | Complete | Compile-time error referencing R28, test coverage |
| V0.0-M3-T5 | Fixed-size field type enforcement | Complete | Recursive validation at data block declaration boundary, test coverage for primitives, tuples, arrays, options, structs, enums, and rejection cases |
| V0.0-M3-T6 | Verifier bounds checking for GetData and SetData | Complete | Slot index validated against data layout slot count, test coverage |
| V0.0-M3-T7 | Host interoperability layer specification | Complete | R29. Slot-based Vec<Value> interface. Documented in EXECUTION_MODEL and COMPILATION_PIPELINE |
| V0.0-M3-T8 | End-to-end data segment integration tests | Complete | Six hot swap tests added covering same-schema, new-schema, size mismatch, no-data module, swap at reset, and rollback. 238 tests pass, zero clippy warnings |
| V0.0-M3-T9 | Hot swap API replace_module on Vm | Complete | replace_module, data_len added with documentation |
| V0.0-M3-T10 | Concurrency contract specification | Complete | Single-ownership enforced by Rust borrow checker. Documented in EXECUTION_MODEL |
| V0.0-M4-T1 | Cargo workspace conversion with keleusma-macros | Complete | Workspace member added, runtime crate retains src/ at root |
| V0.0-M4-T2 | KeleusmaType trait and primitive impls | Complete | Trait in src/marshall.rs covers i64, f64, bool, (), Option, fixed-arity tuples through 4, and fixed-length arrays |
| V0.0-M4-T3 | KeleusmaType derive for structs and enums | Complete | Named-field structs and enums with unit, tuple, and struct-style variants |
| V0.0-M4-T4 | IntoNativeFn family and register_fn API | Complete | Arities 0 through 4 for both infallible and Result-returning host functions |
| V0.0-M4-T5 | Migrate audio_natives and utility_natives to register_fn | Complete | Twelve functions migrated; the three Value-polymorphic functions remain on register_native |
| V0.0-M4-T6 | Integration tests for derive and register_fn | Complete | tests/marshall.rs adds 17 integration tests |
| V0.0-M4-T7 | Documentation pass for marshalling layer | Complete | LANGUAGE_DESIGN, COMPILATION_PIPELINE, GLOSSARY, RELATED_WORK Section 9, R30, B5 |
| V0.0-M5-T1 | Record R31 R32 R33 and B6 B9 B10 | Complete | Decisions and backlog entries documented |
| V0.0-M5-T2 | Two-string-type discipline at runtime | Complete | Value::StaticStr and Value::DynStr distinct variants. Source literals compile to StaticStr. to_string returns DynStr. String concat result is DynStr. |
| V0.0-M5-T3 | Cross-yield prohibition on dynamic strings | Complete | Runtime structural check at Op::Yield. Value::contains_dynstr helper. Tests for static yield, dynamic yield rejection, tuple-with-dynstr rejection. |
| V0.0-M5-T4 | Documentation pass for fifth guarantee | Complete | TYPE_SYSTEM, LANGUAGE_DESIGN, EXECUTION_MODEL, GLOSSARY updated. Five guarantees explicit. Dual-end arena documented. |
| V0.0-M5-T5 | Arena allocator with allocator-api2 | Foundation complete in V0.0-M6 | See V0.0-M6 below. |
| V0.0-M5-T6 | WCMU instrumentation | Deferred | Tracked as P8. Pairs with the deeper arena integration. |
| V0.0-M6-T1 | Add allocator-api2 dependency | Complete | Cargo.toml updated, no_std + alloc preserved |
| V0.0-M6-T2 | Implement Arena type | Complete | src/arena.rs with dual-end bump pointers, alignment-aware allocation, reset method |
| V0.0-M6-T3 | Implement Allocator trait for handles | Complete | StackHandle and HeapHandle types implementing allocator_api2::Allocator |
| V0.0-M6-T4 | Wire arena into Vm | Complete | Arena field, configurable capacity, reset on Op::Reset and replace_module |
| V0.0-M6-T5 | Documentation pass | Complete | R34 added, EXECUTION_MODEL updated, GLOSSARY updated, P7 marked foundation-complete |
| V0.0-M6-T6 | Operand stack and DynStr arena migration | Open | P7 follow-on work. Substantial refactor due to arena lifetime parameter cascade |
| V0.0-M6-T7 | WCMU instrumentation | Complete | Op::stack_growth, Op::stack_shrink, Op::heap_alloc methods. wcmu_stream_iteration in verify.rs. verify_resource_bounds called from Vm::new and Vm::replace_module |
| V0.0-M6-T8 | Native attestation API for WCET and WCMU | Complete | Vm::set_native_bounds. R35 records the implementation |
| V0.0-M6-T9 | Tests for WCMU analysis and verification | Complete | Eight new tests covering simple stream, branching, NewStruct heap, NewArray heap, non-stream rejection, resource bounds pass, oversized rejection, and non-stream skip |
| V0.0-M6-T10 | Auto-arena sizing | Open | P8 follow-on. Compute WCMU at module load and size the arena automatically. |
| V0.0-M6-T11 | Call-graph WCMU integration | Open | P8 follow-on. Walk the call graph to include transitive heap and stack contributions of called functions |
| V0.0-M6-T12 | keleusma-arena pre-publication polish | Complete | crates.io name available. Drop impl audit comment in source. MSRV verified at 1.85. miri stacked-borrows clean (21 of 22 tests; one ignored due to deliberate Vec leak). 16-byte aligned heap allocation via alloc_zeroed and matching dealloc on drop. Non-global-allocator note in Arena type doc. CHANGELOG.md following Keep a Changelog conventions. mixed_allocator example demonstrating arena alongside global allocator. cargo publish --dry-run succeeds. |
| V0.0-M6-T13 | keleusma-arena pre-publication final pass | Complete | Tree borrows verified (21 of 22 miri tests pass, same single deliberate-leak ignore). docs.rs metadata block added (all-features, rustdoc-args=docsrs). Doctest added on Arena::with_capacity demonstrating construction, allocator-api2 integration, and observability. CI extended with miri (stacked and tree borrows), MSRV pin (1.85, default and no-default-features), and clippy --workspace --all-targets. |
| V0.0-M6-T14 | keleusma-arena docs polish | Complete | doc_cfg annotation added to Arena::with_capacity so docs.rs renders the alloc feature badge. README wired into crate-level documentation via include_str so all README code blocks run as doctests. Static-Buffer Use example rewritten to use addr_of_mut for edition 2024 compatibility. Existing structured reference content preserved as ## API Reference subsection following the README. |

## History

| Date | Summary |
|------|---------|
| 2026-03-02 | Crate extracted from Vows of Love and War workspace. Knowledge graph created. |
| 2026-03-02 | Block-structured ISA transition complete. R22. |
| 2026-05-08 | Productivity verification and WCET analysis complete. R23. |
| 2026-05-08 | For-in over arrays, tuple literals, utility natives, README. V0.0-M2 complete. 216 tests passing. |
| 2026-05-08 | Formal related work pass with citations across knowledge graph. RELATED_WORK.md created. |
| 2026-05-08 | Data segment design feasibility analysis. R24 through R28 added. RELATED_WORK Section 8 added covering hot code update with persistent state. |
| 2026-05-08 | Data segment source conformance brought into alignment with specification. Singular block check, fixed-size field type validation, verifier bounds check. 232 tests pass, zero clippy warnings. |
| 2026-05-08 | V0.0-M3 complete. Hot swap API added with replace_module and data_len. Six hot swap integration tests added. R29 records the slot-based interoperability decision. 238 tests pass. |
| 2026-05-08 | V0.0-M4 complete. Workspace conversion. keleusma-macros proc-macro crate added. KeleusmaType trait, derive, IntoNativeFn family, and register_fn API. Audio and utility natives migrated. 268 tests pass. R30 records the static marshalling decision. |
| 2026-05-08 | V0.0-M5 partial complete. Two-string-type discipline at runtime with Value::StaticStr and Value::DynStr. Cross-yield prohibition enforced. R31 R32 R33 added. WCMU and dual-end arena documented as the fifth guarantee. 272 tests pass. Arena allocator and WCMU instrumentation deferred to V0.0-M6 as P7 and P8. |
| 2026-05-08 | V0.0-M6 partial complete. Arena allocator foundation in place. allocator-api2 dependency added. Arena type with dual-end bump pointers, StackHandle and HeapHandle implementing allocator_api2::Allocator. Vm holds Arena, reset on Op::Reset and replace_module. R34 records the implementation. 286 tests pass. Operand stack and DynStr arena migration tracked as P7 follow-on. WCMU instrumentation tracked as P8. |
| 2026-05-08 | V0.0-M6 substantially complete. WCMU instrumentation added with per-op stack and heap cost methods. wcmu_stream_iteration parallels wcet_stream_iteration. verify_resource_bounds enforces stack_wcmu plus heap_wcmu fits within arena_capacity at Vm::new and Vm::replace_module. Native function attestation widened to include WCET and WCMU bounds via Vm::set_native_bounds. R35 records the implementation. 294 tests pass. Auto-arena sizing and call-graph integration deferred to V0.0-M7 as P8 follow-on. |
| 2026-05-08 | V0.0-M6 complete. Arena extracted to standalone keleusma-arena crate. Three constructors (with_capacity, from_static_buffer, from_buffer_unchecked). Renamed handles to BottomHandle and TopHandle. Added Budget contract for generic budget verification. Added mark and rewind API with unsafe per-end reset. Added peak watermark tracking. core-only operation without alloc supported. R36 records the extraction. 300 tests across the workspace. Tagline "Simple and boring memory allocator for exciting applications." |
| 2026-05-08 | P8 complete. Call-graph integration via module_wcmu walks DAG topologically. Native attestation propagation. Vm::new_auto auto-sizes arena. Vm::verify_resources re-checks with current attestations. Three keleusma examples added (wcmu_basic, wcmu_attestation, wcmu_rejection). R37 records the work. 310 tests pass. |
| 2026-05-08 | P9 complete. Bounded-iteration loop analysis. extract_loop_iteration_bound pattern-matches the canonical for-range bytecode shape. WCMU heap and WCET cost multiplied by iteration count for literal bounds. Conservative one-iteration fallback for non-literal bounds. R38 records the implementation. 315 tests pass. |
| 2026-05-08 | keleusma-arena pre-publication polish complete. Storage migrated from `Box<[u8]>` to `alloc_zeroed` with explicit 16-byte alignment and matching `dealloc` on drop, eliminating provenance ambiguity under miri's stacked-borrows model. Drop impl audit comment added. Non-global-allocator note added to the Arena type-level doc. CHANGELOG.md added in Keep a Changelog format. New `mixed_allocator` example demonstrates the arena coexisting with the global allocator. miri runs clean for 21 of 22 tests. cargo publish --dry-run succeeds. |
| 2026-05-08 | keleusma-arena pre-publication final pass. Tree borrows verified clean. docs.rs metadata block added so feature-gated APIs render and doc_cfg activates. Doctest added on Arena::with_capacity to catch documentation drift. CI extended with miri job covering both stacked and tree borrows, MSRV pin job at Rust 1.85 covering default and no-default-features builds, and clippy upgraded to --workspace --all-targets to lint examples. |
| 2026-05-08 | keleusma-arena docs polish. Activated the docs.rs alloc feature badge by adding cfg_attr doc_cfg to Arena::with_capacity. Wired README into crate-level documentation via include_str so all README code blocks run as doctests under cargo test --doc. Rewrote Static-Buffer Use example to use core::ptr::addr_of_mut for edition 2024 compatibility and inline form. Existing structured reference content kept as the API Reference subsection following the README intro. Six doctests pass total (five from the README, one on Arena::with_capacity). |
