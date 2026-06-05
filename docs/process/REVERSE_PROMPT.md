# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-04
**Status**: B28 P2 is complete and merged-and-pushed on `feat-flat-memory-model` (nested inlining, the layout fold, and arena residence Phase 1 and Phase 2; the P2 row in `BACKLOG.md` is marked complete). P4 (precise WCMU) is in progress on the sub-feature branch `feat-flat-memory-wcmu`: milestone 1 (`ca419bb`, the unified packer) and milestone 2 (`93cf9d3`, the `Op::NewComposite` opcode coexisting and fully tested) landed green; milestones 3 (compiler emits `NewComposite`) and 4 (remove the four old ops) are staged below, not yet built. Lib 1100 green through `93cf9d3`. Committed locally, not pushed.

## P4 design (agreed with the operator) and progress

The operator's framing: the WCMU verifier should operate on post-compilation bytecode, with each composite construction carrying its allocation byte count explicitly (conceptually `ALLOCATEBYTES n`), so the verifier sums those counts with no type tables. The artifact dissolved type definitions, so this explicit size is the only way the verifier can recompute precise WCMU. Today the verifier estimates `count * VALUE_SLOT_SIZE_BYTES` (32 bytes/field): sound for flat scalar composites, but it can undercount a composite with a large nested-composite field (a nested `[Word; 100]` counts as one 32-byte slot). The runtime fails safe (`OutOfArena`).

Decisions taken with the operator:
- Consolidate the four construct opcodes (`NewStruct`/`NewTuple`/`NewArray`/`NewEnum`) into one `NewComposite` (a net minus-three opcodes). A tuple is an anonymous struct, an array a homogeneous struct, and a flat enum a struct whose first field is the discriminant, so flat construction is one operation: allocate `byte_size` bytes, pack `count` values, wrap as `kind`. This also absorbs the enum `min_payload`/disc-via-stack hack into the operand.
- The allocation size is a static verifier annotation (the runtime already derives the size from the popped values). It rides inline, not in the operand pool: flat `NewComposite` = `byte_size:u16` plus `(kind:2 | count:6):u8` in the three inline operand bytes (`count` up to 62, `byte_size` up to 64 KiB); the boxed form (a reference-bearing field, or `Option`; vanishes after P3) and oversized composites use the pool. The operator's "allocation instructions are not large instructions" is honoured for the common flat case.
- Memory is not blanket-zeroed: the pack writes the used bytes, and only the trailing slack (an enum padded to its largest variant) is zeroed. This is done (commit `ca419bb`).
- `STACKALLOCATE`/`HEAPALLOCATE` as separate generic allocation instructions are the right shape for the V0.4 untyped-stack ISA, not B28: B28 keeps the tagged operand stack, so allocation must stay coupled to the composite kind (hence `NewComposite`, not a bare allocate). Recorded as the V0.4 destination.

Progress (green-milestone path: groundwork, opcode coexisting, compiler emit, remove old):
- Milestone 1 done (`ca419bb`): the four constructors delegate to one `GenericValue::try_pack_flat` packer that appends fields and zeros only the slack; `min_bytes` is the explicit allocation. Behaviour-preserving.
- Milestone 2 done (`93cf9d3`): `Op::NewComposite` (id 69) is fully defined and handled in every exhaustive `Op` match, coexisting with the four old ops (not yet emitted). `NewCompositeOperand` is `Flat { kind, count, byte_size }` (encoded inline: byte size in operand bytes one and two, `kind` in byte three's high two bits and `count` 0..=62 in the low six) or `Boxed { kind, count, meta }` (a `from_u16_u16_u8` pool entry `(count, meta, boxed=1)`, also used for a flat count beyond 62). `new_composite_flat` packs the values into `byte_size` and wraps by kind; `new_composite_boxed` builds the named body. The WCMU heap-cost arm uses the exact `alloc_bytes` rather than the `count * VALUE_SLOT` estimate. The VM handler pops `count` materialised values and constructs flat or boxed, migrating to the arena. A wire round-trip test covers inline flat, pool flat, and boxed. Lib 1100 green.
- Milestone 3 (next): the compiler emits `NewComposite` at all six construct sites (struct literal, tuple, array, `EnumVariant`, and the two checked-arm unit-variant sites). Each computes the flat `byte_size` from the type via `type_flat_size` (the reconciled `LayoutContext`): `Flat { kind, count, byte_size }` when the type is flat, else `Boxed { kind, count, meta }` (`meta` = the struct template, reused for enums to carry the type and variant names). For a flat enum the discriminant becomes the first packed value (push `Const(disc)` first, `count = 1 + payload`, `byte_size = word + payload_max`), which retires the `min_payload`-via-stack push. Golden-bytecode tests change here.
- Milestone 4: remove `NewStruct`/`NewTuple`/`NewArray`/`NewEnum` and every arm naming them, regenerate golden bytecode, update the three bench cost models, and fix the tests that match the old ops. Full gate green.

The remaining `STACK`/`HEAP ALLOCATE` generic-allocation idea is the V0.4 untyped-stack destination, recorded above, not B28.

One soundness gap is open by design and deferred to P4: the worst-case-memory-usage verifier does not yet count composite top-head bytes, so a WCMU bound can undercount a composite-heavy program. The runtime fails safe (an arena exhaustion surfaces a clean `OutOfArena`, not undefined behaviour); P4 closes the bound.

## Layout-arithmetic consolidation (follow-up this session)

The open concern below, that the compiler-side eligibility helpers reimplemented layout arithmetic the P1 pass already had, is resolved. `LayoutDescriptor` gained `flat_byte_size`, `flat_scalar_kind`, and `flat_composite_kind`, which fold flat-eligibility (excluding float, text, and opaque), the `Option`-boxed rule, enum uniformity with word-sized-discriminant padding, and the recursive size into one place. The compiler's `type_flat_size`, `enum_uniform_flat_payload_max`, and `classify_flat_field` are now thin queries over a `LayoutContext` built from the module's struct and enum definitions (added to `TypeInfo` as `struct_defs` and `enum_defs`); the ad-hoc `type_flat_scalar_kind`, `type_flat_composite_kind`, and `unwrap_labels` helpers were removed. The change is behaviour-preserving and the full gate stayed green. The runtime construction choke points remain a separate value-driven computation, which is inherent since the runtime has no type tables at construction; they agree with the type-side predicate by construction and are exercised by the same corpus.

## What landed this session

A composite-typed field of a flat composite now inlines into the parent's flat byte body rather than forcing the parent boxed. The four composite kinds nest recursively. Access reads a nested field by extracting its byte range and re-wrapping it as a flat composite value. Nested enums are included, which required reconciling the enum body to one fixed size.

### Representation and access

- Recursive eligibility. A field is flat-eligible when it is a non-reference non-float scalar or when it is itself a transitively-flat composite. `flat_field_size` and `flat_body_bytes` on the runtime value, and `type_flat_size`, `type_flat_composite_kind`, and `classify_flat_field` on the compiler type side, mirror each other so a baked access always agrees with the constructed body.
- New operand form. Each access operand gained a `FlatNested { offset, size, variant }` variant. `ArrayElem` carries only `size`, since the element offset is `index * size`. The `variant` is a new `CompositeKind` tag identifying which value variant to re-wrap as.
- Wire encoding. A nested access spills the offset and size into a `from_u16_u16` operand-pool entry, references it by a sixteen-bit pool index in operand bytes one and two, and marks byte three with the sentinel `0xF0` combined with the variant tag. The scalar `Flat` and `Boxed` forms stay inline and unchanged. A nested access whose pool index would exceed sixteen bits is rejected at encode time, which the small modules of the target never reach.
- VM read. `GetField`, `GetTupleField`, `GetIndex`, and `GetEnumField` gained a `FlatNested` arm that slices the child body out of the parent and re-wraps it through `GenericValue::from_flat_nested_bytes`.

### Enum reconciliation

The shipped flat enum body was variant-sized, which a nested enum field cannot use because a field needs a fixed slot. Two changes resolve this.

- Uniform fixed size. A uniformly-flat enum, meaning one whose every variant is flat, is padded to `word_bytes` plus the largest variant payload, computed by the compiler and delivered to `NewEnum` as a minimum-payload constant pushed beneath the discriminant on the stack. The stack delivery avoids any wire-format, golden-byte, or cost-model churn. `enum_with_widths` gained a `min_payload` parameter, and the convenience `enum_value` passes zero, so the public constructor and its many call sites are unchanged.
- Padding-tolerant equality. Flat enum bodies compare by their overlapping prefix with each trailing remainder required to be zero. A compiler-padded body and an unpadded variant-sized body of the same value therefore compare equal without a type table at the equality site. This avoids a mixed-variant regression. A non-uniform enum, meaning one with a non-flat variant, keeps per-variant flat-or-boxed bodies standalone, and a nested field of such an enum forces the parent boxed.

### Layout-pass reconciliation

The P1 `LayoutDescriptor::Enum` and `LayoutContext` modelled a one-byte discriminant, which disagreed with the shipped word-sized runtime discriminant. The descriptor now uses a word-sized discriminant plus the largest variant payload, and the P1 tests were updated. The pass is still not consulted at run time. The access baking uses the focused recursive helpers over the compiler type tables.

### Host marshalling

Flattening nested-composite composites changed how `struct_with_widths` and its siblings build a host-marshalled struct that nests a flat composite, so the derive flat-read path had to learn nested fields or it would have regressed the previously-working boxed round-trip. `KeleusmaType` gained two defaulted methods, `flat_byte_size` and `from_flat_bytes`, so existing implementations stay valid. The array and tuple implementations and the `#[derive(KeleusmaType)]` struct and enum expansions override them to read and write nested flat composites at packed offsets. The derived enum computes its largest-variant payload at run time so a host-built nested enum pads to the same slot the compiler bakes for a script.

## Verification

- Default workspace: lib 1124, plus rogue 53, arena 37, marshall 27, narrow 17, zero-copy 17, bench 6, cli 32, and the rest. All pass.
- All features: all pass.
- Clippy on default and all features with warnings denied: clean.
- Strict rustdoc with warnings denied: clean. One pre-existing unresolvable intra-doc link in `keleusma-macros` was demoted to a code span. It predated this work and the local pre-push hook had tolerated it.
- rustfmt: clean.
- New tests: four script-level cases drive the full pipeline, namely a nested struct in a struct, a nested tuple in a struct, the extracted nested struct bound as a value, and a uniformly-flat enum nested in a struct and matched. Three derive cases cover the nested flat struct and tuple round-trip, the uniformly-flat enum padding, and a struct nesting a flat enum.

## Decisions taken and concerns

- You chose the larger scope that includes nested enums rather than deferring them. The enum reconciliation above is the consequence.
- A nested access uses one operand-pool entry. The common scalar field access stays inline, so only the nesting case pays a pool entry.
- The mixed-variant enum case keeps its current standalone behaviour. There is no flatness regression.
- Const-folded composites that would nest an enum are not flat-folded, so a variant-sized const enum is never inlined into a fixed parent slot. A const struct nesting a fixed-size composite such as a tuple, array, or struct is fine because those carry no variant-dependent size.
- Resolved concern. The compiler-side eligibility helpers no longer reimplement the layout arithmetic; they query the reconciled `LayoutContext`/`LayoutDescriptor` (see the consolidation section above). The runtime value-driven path is the one remaining separate computation, which is inherent and not duplicative of the type-side pass.

## Intended next step

## B28 P2 arena residence (Phase 1 and Phase 2 done)

Composite bodies now live on the arena's top ephemeral head rather than the global heap, under the epoch-guarded handle model you directed (validity and equality orthogonal, `if_exists` then `if_equals`).

**Phase 1 (commit `6bf782a`).** `FlatComposite` became `Inline(Vec<u8>) | Arena(ArenaHandle<[u8]>)`, mirroring `KString`. `in_arena(arena)` migrates an inline body to the top head (unsafe alloc encapsulated like `KString::alloc`; empty bodies stay inline). `resolve(arena)` reads (epoch-checked for arena bodies, direct for inline). `to_inline(arena)` and `inline_bytes()` bridge bodies to no-arena contexts. `eq_in_arena` and the proof tests established the equality model.

**Phase 2 (this session).** The VM now allocates and reads arena bodies:
- Construction. `NewTuple`/`NewStruct`/`NewArray`/`NewEnum` materialise any arena-resident child to inline (so the shared `*_with_widths` packer can read its bytes), then migrate the finished parent with `into_arena_body(arena)`, mapping arena exhaustion to a clean `OutOfArena`.
- Reads. `GetField`/`GetTupleField`/`GetIndex`/`GetEnumField` and the `IsEnum` discriminant read go through `resolve(arena)`, mapping a stale body to `stale_arena_body()` (or, for `IsEnum`, to not-matching).
- Equality. `CmpEq`/`CmpNe` materialise both operands to inline, then compare with `PartialEq` (content). This realises `if_exists` then `if_equals` without a type table and reuses the existing content comparison; the flat-enum `PartialEq` arm was made arena-safe via `inline_bytes()`.
- Escape and persistence. Returned and yielded values are materialised to inline so they survive a later `RESET` or the arena being dropped. `SetData`/`SetDataIndexed` materialise to inline before writing a persistent data slot, since that region outlives `RESET`.
- Native boundary. Composite arguments are materialised to inline before a native call, since `from_value` has no arena; native return values are already inline.

A new test asserts a VM-built struct lands on the arena top head (`top_peak >= 24`).

**Known gap, deferred to P4.** The worst-case-memory-usage verifier does not yet count composite top-head bytes, so a WCMU bound can undercount a composite-heavy program. The runtime fails safe (`OutOfArena`, not undefined behaviour). P4 recomputes WCMU against the flat sizes and closes this.

After P2, P3 is reference fields (Text, Opaque) as handles, P4 the WCMU recompute, and P5 hot-swap migration, documentation, and decision closure.

## Awaiting direction

- Whether to merge `feat-flat-memory-arena` into `feat-flat-memory-model` and push.
- Which item to take next: P4 (close the WCMU gap, which pairs naturally with the arena residence just landed) or P3 (reference fields as handles).
