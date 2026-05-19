# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-19
**Status**: V0.2 Phase 8 complete. Three deferred items closed in a single pass. Struct and enum literal initializers for `const data` fields are accepted at parse, compile, and runtime. Per-yield arena dataflow refinement tightens the ephemerality rule by consulting the existing text-size abstract interpretation pass at boundary-crossing ops. `keleusma-arena 0.3.0` publish dry-run is clean against the registry-resolved dependencies. 611 lib tests pass workspace-wide.

## Completed in this session round

| Directive | Resolution |
|-----------|------------|
| Add struct and enum literal initializers for `const data` fields. | `ConstInitializer` AST extended with `Struct { name, fields: Vec<(String, ConstInitializer)> }` and `Enum { enum_name, variant, args: Vec<ConstInitializer> }` variants. Parser recognises `Name { field: init, ... }` and `Enum::Variant` or `Enum::Variant(arg, ...)` shapes inside const initializer position by looking for a leading `UpperIdent`. Compiler validates the type name against the declared field type (`TypeExpr::Named`) and rejects mismatches with a diagnostic naming both names. Nested composite initializers are admitted through a permissive inner recursion (`const_value_any`) that falls back to type-agnostic literal-to-ConstValue conversion when the precise inner type cannot be determined from the surface context. Three new tests in `vm.rs` cover struct, unit-variant enum, and tuple-variant enum cases. |
| Per-yield arena dataflow analysis for stronger ephemeral inference. | The existing text-size abstract interpretation pass (`src/text_size.rs`) already tracks per-stack-slot `TextSize` lattice values through compiled bytecode in topological call order for WCMU heap-allocation bounding. Phase 8 extends the analysis: `ChunkTextAnalysis` gains a `yields_text` field, computed by peeking the abstract operand stack before `Op::Yield` pops it (mirroring the existing `Op::Return` peek). A new public helper `verify::module_chunk_text_analyses(&Module) -> Result<Vec<ChunkTextAnalysis>, VerifyError>` exposes the per-chunk analysis result. The compiler's ephemerality check consults the entry chunk's analysis: declared `Text` return is now only disqualifying when the entry chunk's compiled body actually leaves a text value on top of the abstract stack at a boundary-crossing op. A negative regression test confirms `fn main() -> Text { "hello" }` is still correctly rejected from ephemerality. A direct unit test of `module_chunk_text_analyses` exercises both flags on a hand-crafted two-chunk module since the source-level positive case for the refinement is blocked by an unrelated type-unification limitation around bare `Option::None` literals in function returns. |
| `keleusma-arena 0.3.0` publish-readiness verification. | `cargo publish -p keleusma-arena --dry-run` is clean. 13 files, 24.1 KiB compressed. The crate is ready for operator-driven `cargo publish -p keleusma-arena`. |

## Verification matrix

```bash
cargo test --workspace --features text                          # 611 lib + 17+17+3+53+37+6+7 integration tests pass
cargo clippy --workspace --tests --features text -- -D warnings # clean
cargo fmt --all                                                 # idempotent
cargo publish -p keleusma-arena --dry-run                       # clean
```

The 611 figure is the runtime crate's lib-test count after Phase 8: prior 606 from V0.2 Phase 7, +3 const-initializer tests (struct, unit-variant enum, tuple-variant enum), +1 negative regression test for declared-text-return ephemerality, +1 direct unit test of `module_chunk_text_analyses`.

## Notes

- The source-level positive test for the per-yield dataflow refinement is blocked by an unrelated type-unification limitation around bare `Option::None` literals in function returns. The type checker rejects `fn main() -> Option<Text> { Option::None }` with "function `main` returns Option<Text> but body produces Option<<unknown>>". A future tightening of the type checker's inference rules would unblock the positive test; in the meantime, the direct unit test of `module_chunk_text_analyses` keeps the dataflow path under automated coverage.
- The compile pipeline's ephemerality check now performs a topological-order walk through `module_chunk_text_analyses` on every compile with `verify` enabled. The cost is proportional to the total opcode count across all chunks. For programs that do not declare a `Text` return type the result is computed but the conservative fallback never fires.
- The `const_value_any` helper is intentionally infallible. It accepts any well-formed `ConstInitializer` and returns a corresponding `ConstValue`. The fallible helper `const_value_from_literal_for_field` continues to validate the initializer's shape against the declared field type and emits CompileError on mismatch. The pair models the distinction between "the surface code is well-formed" and "the surface code matches the declared type."

## Intended Next Step

Awaiting operator prompt. Phase 8 closes the deferred-items list from Phase 7. Several follow-on items remain, none blocking.

1. **Operator action: publish `keleusma-arena 0.3.0` to crates.io.** Dry-run is clean. The agent does not perform `cargo publish`; the operator runs `cargo publish -p keleusma-arena` to complete the release. Once 0.3.0 is live, the parent `keleusma` crate's dependency requirement bumps from the current Cargo.toml pin to `"0.3"`.
2. **Unblock the source-level positive test for the dataflow refinement.** The type checker's bare-`None`-in-function-return limitation is the immediate blocker. Resolving it would also unblock idiomatic `Option<T>` returns in user scripts.
3. **Document the per-yield dataflow refinement in `docs/architecture/LANGUAGE_DESIGN.md`.** The Memory Model section's Ephemeral Modules subsection currently describes the conservative signature-only rule. The refinement should be added as a tightening note that names the abstract interpretation pass and the boundary-crossing peek mechanism.
4. **Decide on V0.2 release tag timing.** With Phase 8 closed, the V0.2 surface is feature-complete relative to the original Phase 0 spec. Pending items in the backlog (additional examples, further binary-size reductions) belong in V0.2.x point releases.
