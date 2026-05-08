# Task Log

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

Current sprint source of truth.

---

## Current Phase

**V0.0**: Bootstrap, near completion. Data segment design formalized. Implementation in progress.

## Active Milestone

None. V0.0-M5 partial complete. Type discipline implemented. Arena allocator and WCMU instrumentation tracked as P7 and P8 for V0.0-M6.

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
| V0.0-M5-T5 | Arena allocator with allocator-api2 | Deferred | Tracked as P7 for V0.0-M6. Substantial infrastructure work. The two-string-type discipline is operational without it. |
| V0.0-M5-T6 | WCMU instrumentation | Deferred | Tracked as P8 for V0.0-M6. Pairs naturally with the arena allocator. |

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
