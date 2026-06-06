# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-06-05
**Status**: B28 P4 (precise WCMU through the `NewComposite` consolidation) is complete on the sub-feature branch `feat-flat-memory-wcmu`. Milestones 1 through 4 have all landed green. The branch is ready to merge into `feat-flat-memory-model`. Committed locally, not pushed.

## What landed this session

The four V0.2.0 construct opcodes are consolidated into one `NewComposite`, and the worst-case-memory-usage verifier now reads each composite's exact flat allocation size from the opcode operand rather than estimating `count * VALUE_SLOT_SIZE_BYTES`.

- Milestone 1 (`ca419bb`): the four constructors delegate to one `GenericValue::try_pack_flat` packer that appends fields and zeros only the trailing slack. Behaviour preserving.
- Milestone 2 (`93cf9d3`): `Op::NewComposite` (wire id 69) is fully defined and handled, coexisting with the four old ops. `NewCompositeOperand` is `Flat { kind, count, byte_size }` packed inline, or `Boxed { kind, count, meta }` carried by a `(u16, u16, u8)` pool entry. The WCMU heap-cost arm uses the exact `alloc_bytes`.
- Milestone 3 (`4e641d0`): the compiler emits `NewComposite` at every construct site. Struct and enum are operand driven because the named type is reliable. Tuple and array are value driven because element-type inference can fail, so the VM decides flat or boxed from the runtime values through `tuple_with_widths` and `array_with_widths`, and the operand `byte_size` is a verifier annotation that is exact when inference succeeds and conservative otherwise. A flat enum pushes its discriminant as the first packed value, which retired the `min_payload`-via-stack hack.
- Milestone 4 (`c9ad834`): the four old opcodes and every arm naming them are removed across `bytecode.rs`, `vm.rs`, `verify.rs`, and `wire_format.rs`. The orphaned `decode_pool_u16_u16_u8` helper is deleted. The bench generator and the two measured cost models collapse the composite group to one `NewComposite` entry. The legacy-op tests migrate to `NewComposite`, and the WCMU heap tests now assert the precise operand-carried byte size. The array-length alias tracer in the verifier keys on `NewComposite(Array)`.

The live ISA is now 66 opcodes. Wire ids 34 through 37 are retired and reserved, the maximum live id is 69, and `BYTECODE_VERSION` stays at 1.

## Documentation reconciliation

The authoritative specs and the current-system narrative are updated to the consolidated ISA.

- `docs/spec/INSTRUCTION_SET.md`: the Type Construction table is one `NewComposite` row, the opcode count is 66 with ids 34 to 37 retired, the operand-shape inventory adds the bespoke `NewComposite` shape and corrects the `u8` and `u16` counts, and the cost, stack-growth, stack-shrink, and heap-allocation tables name `NewComposite`. The heap table reports the exact operand `byte_size`.
- `docs/spec/WIRE_FORMAT.md`: the opcode-identifier paragraph records the retirement, the operand-shape bullets add the `NewComposite` flat inline shape and its 16-bit pool-index boxed form, and the rationale and pool-tag notes name `NewComposite`.
- `docs/architecture/EXECUTION_MODEL.md`: the operand-shape table and the inline-versus-pool paragraph reflect the consolidation, and the `.rodata` row describes the new enum-construction path. The opcode-id validity note lists the live id ranges.

The decision records under `docs/decisions/` are left as historical artifacts. They legitimately reference the four old opcodes as the state at the time of those decisions.

## Verification

- Default workspace: lib 1137 plus rogue 53, arena 37, marshall 27, narrow 17, zero-copy 17, bench 6, cli 32, and the rest. All pass.
- All features: all pass.
- Default plus signatures: all pass.
- Clippy on default and on all features with warnings denied: clean.
- Strict rustdoc with warnings denied: clean.
- rustfmt: clean.
- The inline golden-bytecode round-trip test passes. The fixture program returns a constant and constructs no composite, so the consolidation does not change its bytes.

## Open concern

Tuple and array construction is value driven, so the operand `byte_size` for those kinds is a conservative annotation when element-type inference fails, not always the exact flat size. The runtime still constructs the correct body from the values, and the verifier over-approximates rather than under-approximates in that case, so the WCMU bound stays sound. The exact-size path is taken whenever inference succeeds, which is the common case.

A deferred follow-up remains from P2. Padding slack in a flat enum body is read by content equality and so must be deterministic zero. Compiling equality field-wise would remove that requirement and let the packer skip the slack zero entirely.

## Intended next step

Merge `feat-flat-memory-wcmu` into `feat-flat-memory-model` with a fast-forward merge, then continue B28 with P3 (reference fields, namely Text and Opaque, carried as handles) followed by P5 (hot-swap migration, documentation, and decision closure). Awaiting direction on whether to merge and push now and on whether to take P3 or P5 next.
