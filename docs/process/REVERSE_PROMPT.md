# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-04
**Status**: B28 P2 nested-composite inlining is complete end to end on the sub-feature branch `feat-flat-memory-nested`, cut from `feat-flat-memory-model`, followed by a layout-arithmetic consolidation that folds the compiler's flat-layout helpers onto the P1 `LayoutContext`/`LayoutDescriptor`. The full verification gate is green under default features, all features, clippy on both, strict rustdoc, and rustfmt. Both pieces are committed locally and not pushed.

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

Remaining B28 P2 is arena residence of `FlatComposite`, which is still a heap byte vector. After that, P3 is reference fields as handles, P4 is the worst-case-memory-usage recompute against flat sizes, and P5 is hot-swap migration, documentation, and decision closure. Awaiting your direction on whether to merge `feat-flat-memory-nested` into `feat-flat-memory-model` and push, and on which item to take next.
