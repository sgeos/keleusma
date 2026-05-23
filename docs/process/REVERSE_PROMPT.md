# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-23
**Status**: B28 design revised. The earlier "preserve the opcode set" framing was wrong; the cleaner design consolidates the composite-construction and field-access opcodes around a single `AllocTransient(byte_size)` plus offset-and-kind read/write opcodes, with the compiler computing byte sizes and field offsets at compile time. P0 infrastructure (`LayoutDescriptor`, `FlatComposite`) remains useful. The phased plan was rewritten to reflect the consolidation. P1 in the original plan ("migrate Value::Tuple to flat-byte") is replaced by P1 in the revised plan ("compile-time layout pass"). No code changes since the P0 commit `45df5bf`; this revision is documentation only.

## Summary of work since the last reverse-prompt update

### B28 redesign

The operator pointed out that the V0.2.0 framing of "preserve the opcode set, change only the runtime" was unnecessarily constraining. The cleaner design is:

1. The compiler computes every type's byte size and field offsets at compile time using `LayoutDescriptor` (already landed in P0).
2. The opcode set consolidates. `NewTuple`, `NewArray`, `NewStruct`, `NewEnum` collapse into `AllocTransient(byte_size)`. `GetField`, `GetTupleField`, `GetEnumField`, `SetField` collapse into `ReadScalarAt(offset, kind)` / `WriteScalarAt(offset, kind)` / `ReadCompositeAt(offset, byte_size)` / `WriteCompositeAt(offset, byte_size)`. `GetData(slot)` / `SetData(slot)` gain offset and kind operands and become `ReadDataField(slot, offset, kind)` / `WriteDataField(slot, offset, kind)`.
3. All memory lives in the arena. The persistent region holds `private data`. The ephemeral region has two bump heads (bottom = stack, top = heap) that grow toward each other and reset together at the closing brace of `loop main()`. `shared data` is host-passed and external to the arena. `const data` lives in `.rodata` (the constant pool).
4. Composite bodies are immutable in the ephemeral region; mutations create fresh bodies at the next bump position. Mark-based reclamation at scope boundaries.

`BYTECODE_VERSION` stays at 1 throughout. Opcode numeric encodings shift, but the operator decision that Keleusma has no production traction means backward compatibility is not a constraint.

### Memory model verification

Verified from the existing `keleusma-arena` and `src/vm.rs` / `src/kstring.rs` code:

- Bottom head: stack. Operand-stack vectors use `BottomHandle` (`src/vm.rs:21`).
- Top head: heap. KString bodies allocate via `alloc_top_bytes` (`src/kstring.rs:39`).
- Both ephemeral heads grow toward each other in the shared middle of the ephemeral region.
- The persistent region survives RESET; both ephemeral heads are cleared on RESET.
- RESET is paired with the closing brace of `loop main()`.

### Phased plan revised

| Phase | Scope | Status |
|-------|-------|--------|
| P0 | `LayoutDescriptor` and `FlatComposite` parallel infrastructure | Complete (`45df5bf`) |
| P1 | Compile-time layout pass; compiler walks every Keleusma type and computes byte sizes and field offsets | Next |
| P2 | New consolidated opcode set defined; op handlers implemented; old opcodes deprecated but functional | |
| P3 | Compiler emission migrates to new opcodes; both code paths exist in parallel | |
| P4 | Composite bodies join the arena's top ephemeral head; mark-based reclamation tested | |
| P5 | Runtime composite Value variant collapse; old opcodes retired | |
| P6 | Strippable `DataSlotAnnotation` and chunk-local `debug_pool` field per B29 | |
| P7 | WCMU and WCET correction; cost model re-calibration | |
| P8 | Native marshalling preservation; R29 hot-code-swap migration update | |
| P9 | Documentation pass; B28 moves to RESOLVED; B26 and B27 transition to "resolved through B28" | |

## Verification

- `cargo test --workspace`: not re-run (documentation-only change since the P0 commit).
- BACKLOG.md modifications affect only the B28 entry. B29, B30, B31 unchanged.
- The revised B28 entry preserves the operational consequences table, the forcing case, and the cross-references.

## Open questions

None at the design layer. The revised B28 is internally consistent and grounded in verified facts about the arena and the existing consumer code.

## Recommended next step

Begin revised P1: compile-time layout pass.

P1 scope:

1. Extend the compiler's type-handling code to compute a `LayoutDescriptor` for every concrete Keleusma type encountered during compilation. The layout uses `src/value_layout.rs` from P0.
2. Cache the layout in the appropriate compiler data structure (likely a `BTreeMap<TypeKey, Arc<LayoutDescriptor>>` keyed by the type's canonical representation).
3. Validate the byte sizes against the existing struct templates and tuple type expressions.
4. No opcode emission changes yet. The layout pass is read-only from the perspective of the bytecode; it just establishes the compile-time data structure that P2 onward will consume.

P1 effort: 3-5 days.

Awaiting operator confirmation to proceed with P1.

## Reference

- `docs/decisions/BACKLOG.md` B28 (revised), B29, B30, B31.
- `src/value_layout.rs` from P0 (`45df5bf`).
- `src/flat_value.rs` from P0 (`45df5bf`).
- `src/vm.rs:21` for the operand-stack-on-bottom-head convention.
- `src/kstring.rs:39` for the KString-on-top-head convention.
- `keleusma-arena/src/lib.rs` for the dual-head arena structure.
