# Instruction Set

> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)

The Keleusma VM executes a stack-based bytecode using block-structured control flow. All instructions operate on a value stack. This document lists every instruction with its operands, behavior, and cost contribution to the WCET (worst-case execution time) and WCMU (worst-case memory usage) analyses.

Each instruction carries a relative integer cost. Costs are unitless relative weights, not cycle counts. Higher values indicate more expensive operations. The cost table is consulted by `wcet_stream_iteration()`; the per-instruction stack and heap effects are consulted by `wcmu_stream_iteration()`.

For details on how bytecode is generated from source, see [COMPILATION_PIPELINE.md](../architecture/COMPILATION_PIPELINE.md). For the bytecode wire format including the framing header, opcode-stream encoding, and operand pool, see [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md). For the structural ISA specification including block hierarchy and verification rules, see [STRUCTURAL_ISA.md](./STRUCTURAL_ISA.md).

## Constants

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Const | u16 index | 1 | Push constant from the chunk's constant pool. |
| PushImmediate | u8 value | 1 | Push an inline immediate value. The operand encodes one of a small set of sentinel values or small integers; see "PushImmediate encoding" below. |

### PushImmediate encoding

| Operand value | Meaning |
|--------------|---------|
| `0` | `Value::Unit` |
| `1` | `Value::Bool(true)` |
| `2` | `Value::Bool(false)` |
| `3` | `Value::None` (the `Option::None` sentinel) |
| `4` | `Value::Int(0)` |
| `5` | `Value::Int(1)` |
| ... | ... |
| `19` | `Value::Int(15)` |
| `20..255` | Reserved. Decoder treats as a corruption signal. |

Sixteen small-integer literals (`Int(0)` through `Int(15)`) are encoded inline. Larger or non-immediate literals continue to use `Const` referencing the constant pool. The extraction rule is predictable: operand values 0..3 select sentinels; values 4..19 select `Int(value - 4)`; values 20..255 signal corruption.

## Local Variables

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| GetLocal | u16 slot | 1 | Push local variable onto stack. |
| SetLocal | u16 slot | 1 | Pop stack into local variable slot. |

## Data Segment

The unified slot index space partitions into shared slots `[0, shared_count)` and private slots `[shared_count, shared_count + private_count)`. Shared slots live in the host-owned buffer borrowed at each call and are reached by the host through `Vm::get_shared`/`Vm::set_shared` (B28 item 2); private slots are script-only and live in the arena's persistent region. The opcodes below admit both partitions; the runtime dispatches by comparing the slot index against the cached `shared_slot_count`, sending a shared slot to the borrowed buffer by byte offset and a private slot to the arena. Const data fields do not consume a slot; field reads compile to `Const` and writes are compile errors.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| GetData | u24 slot | 1 | Push data segment slot value onto stack. |
| SetData | u24 slot | 1 | Pop value and store into a data segment slot. A scalar stores inline; a flat composite copies its body into the persistent composite pool at the offset the module's private-composite layout table records for the slot (no dedicated composite-write opcode). |
| GetDataIndexed | u24 base, u24 len | 8 | Pop a Word, Byte or Unit fallback and then a Word index. Read in range, otherwise use the supplied fallback. Push value, index, four Word byte limbs of the bound and an outcome flag. Invalid spans and operand types remain contract errors. |
| SetDataIndexed | u24 base, u24 len | 8 | Pop value and Word index. A valid index writes one slot. An invalid index performs no write. Push the index, four Word byte limbs of the bound and flag zero for success or one for invalid. Invalid spans and storage-contract violations remain errors. |
| BoundsCheck | u16 bound | 7 | Preserve the Word index and append four Word byte limbs of the unsigned bound, little-endian, then a flag. An invalid index reifies as flag `1`, while an index in `[0, bound)` yields flag `0`. The limbs are sign-extended from eight bits. No indexed data access or language bounds fault occurs. |

## Arithmetic

Integer arithmetic uses the checked-arithmetic family. Each `CheckedAdd`, `CheckedSub`, `CheckedMul`, and `CheckedNeg` opcode pops `Value::Int` operands (two for the binary forms, one for `CheckedNeg`), computes the true result in `i128`, and pushes three slots: the low half, the high half, and an outcome flag (`Int(0)` ok, `Int(1)` overflow, `Int(2)` underflow). The push order places `low` at the bottom and `flag` on top so that surface-level wrapping expressions, such as `a + b` on `Int` operands, compile to the checked opcode followed by `PopN(2)` and leave the wrapping result on the stack. Source-level pattern-arm matching destructures the three outputs.

The wrapping arithmetic opcodes `Add`, `Sub`, `Mul`, and `Neg` remain in the instruction set but no longer accept `Value::Int` operands. Their permitted operand types are `Byte`, `Fixed`, and `Float`. The V0.2.0 Consolidation B pass narrowed these opcodes by routing all `Int` arithmetic through the checked family; the compiler emits `CheckedXxx; PopN(2)` for every `Int` operand position. Operands whose type the compiler cannot statically infer fall through to the `Int` path as well, because `Word` is the default numeric type.

`Op::Div` and `Op::Mod` return low, high and flag outcomes. Div shares `CheckedDiv(0)` semantics. Mod shares `CheckedMod` semantics for Word, Byte and Fixed and also supports Float remainder with floating-point classification. Bare `Word` and `Byte` division and modulo lower to `CheckedDiv(0)` or
`CheckedMod`, followed by a flag-three conditional `Trap` and removal of the high
slot. Overflow retains the wrapping low result. A checked construct with an
unguarded variable or wildcard `zero_divisor` arm omits that fallback trap.
Guarded and literal-pattern handlers retain it. Legacy instructions also reify integer zero divisors as flag three, including in hand-built bytecode. Bare Float legacy operations discard high and flag. Bare Fixed remainder dispatches flag three through an explicit trap.

`CheckedMul` and `CheckedDiv` carry a `u8` fraction-bit count that selects integer or `Q`-format arithmetic, where `0` is integer and a positive count is `Fixed`. This is the only place the integer and fixed-point datapaths differ, namely a shift by the fraction-bit count around the multiply or divide, so a single parameterized opcode serves both rather than separate opcodes. `CheckedAdd`, `CheckedSub`, `CheckedMod`, and `CheckedNeg` need no such parameter because their `Fixed` forms involve no shift, and they dispatch on the operand type alone.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| CheckedAdd | none | 2 | Pop two `Int` operands; push `(low, high, flag)`. The `flag` and `high` halves report at the bytecode-declared word width through the shared `vm::checked_arith_outputs` helper. |
| CheckedSub | none | 2 | Pop two `Int` operands; push `(low, high, flag)`. |
| CheckedMul | u8 frac_bits | 2 | Pop two operands; push `(low, high, flag)`. The fraction-bit count selects the format. With `0` it is integer multiply and the high half is the load-bearing value for big-number multiplication. With a count greater than `0` the operands are `Fixed`, the `i128` product is shifted right by that many bits before the range check, the wrapped result is a single word, and the high slot is unused. So `0` fraction bits is exactly integer multiply. |
| CheckedNeg | none | 2 | Pop one `Int` operand; push `(low, high, flag)`. The only overflow case under the default 64-bit declared width is `-i64::MIN`. |
| CheckedDiv | u8 frac_bits | 2 | Pop two operands; push `(low, high, flag)`. The fraction-bit count selects the format. With `0` it is integer divide whose only overflow case at the default 64-bit width is `i64::MIN / -1`. With a count greater than `0` the operands are `Fixed` and the dividend is left-shifted by that many bits in the `i128` domain before dividing. A zero divisor reifies as flag `3` carrying the numerator rather than trapping. Word outcomes use the module-declared width, including when the runtime Word is wider. So `0` fraction bits is exactly integer divide. |
| CheckedMod | none | 2 | Pop two Word, Byte or Fixed operands; push `(low, high, flag)`. A zero divisor reifies as flag `3` carrying the numerator rather than trapping, mirroring `CheckedDiv`; this row said "traps on divide-by-zero" until 2026-09-18, which the implementation has never done. `i64::MIN % -1` yields zero without overflow. |
| Add | none | 2 | Pop two operands of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 sum. The `Int` operand position is excluded; the compiler routes `Int + Int` through `CheckedAdd; PopN(2)`. |
| Sub | none | 2 | Pop two operands of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 difference. The `Int` operand position is excluded. |
| Mul | none | 2 | Pop two operands of type `Byte` or `Float`; push the wrapping or IEEE 754 product. `Fixed` multiplication uses `FixedMul(n)`; the `Int` operand position is excluded. |
| Neg | none | 2 | Pop one operand of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 negation. The `Int` operand position is excluded. |
| Div | none | 5 | Pop two operands; push `(low, high, flag)` with the same semantics as `CheckedDiv(0)`. A zero integer divisor reifies as flag `3` carrying the numerator. |
| Mod | none | 5 | Pop two operands; push `(low, high, flag)`. Word, Byte and Fixed use `CheckedMod` semantics. A zero integer divisor reifies as flag `3` carrying the numerator. Float remainder returns a zero high result and flag zero for finite, one for positive infinity, two for negative infinity or four for NaN. |

## Comparisons

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| CmpEq | none | 2 | Pop two values, push true if equal. |
| CmpNe | none | 2 | Pop two values, push true if not equal. |
| CmpLt | none | 2 | Pop two values, push true if less than. |
| CmpGt | none | 2 | Pop two values, push true if greater than. |
| CmpLe | none | 2 | Pop two values, push true if less than or equal. |
| CmpGe | none | 2 | Pop two values, push true if greater than or equal. |

## Logic

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Not | none | 1 | Pop boolean, push logical negation. |

Short-circuit AND and OR are encoded as `If`-branching at the bytecode level; there are no `LogicalAnd` or `LogicalOr` opcodes.

## Bitwise

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| BitAnd | none | 2 | Pop two `Int` operands, push bitwise AND. |
| BitOr | none | 2 | Pop two `Int` operands, push bitwise OR. |
| BitXor | none | 2 | Pop two `Int` operands, push bitwise XOR. |
| Shl | none | 2 | Pop shift count then value; push value shifted left by `count & (word_width - 1)`. |
| Shr | none | 2 | Pop shift count then value; push arithmetic-right-shifted value (sign-preserving). |

## Control Flow

Block-structured control flow opcodes carry `u16` jump targets. A chunk's opcode count is therefore bounded at 65,535 (`CHUNK_SIZE_HARD_LIMIT`). The compiler emits a [`CompileWarning`](../../src/compiler.rs) when a single chunk crosses 80% of the limit (52,428 ops, `CHUNK_SIZE_SOFT_WARN_THRESHOLD`), prompting decomposition into helper functions; the bytecode at the limit remains valid. Chunks exceeding `CHUNK_SIZE_HARD_LIMIT` are rejected at compile time as a `CompileError`. The host invokes `keleusma::compiler::compile_with_warnings(program, target)` to receive the warning vector alongside the module; `compile_with_target` and `compile` discard the warnings for callers that do not need them.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| If | u16 offset | 1 | Pop boolean. If false, skip forward to matching Else or EndIf. |
| Else | u16 offset | 1 | Unconditional skip forward to matching EndIf. |
| EndIf | none | 1 | End of if or if-else block. No-op. |
| Loop | u16 offset | 1 | Start of loop block. Offset is distance to matching EndLoop. |
| EndLoop | u16 offset | 1 | Unconditional jump backward to matching Loop. |
| Break | u16 depth | 1 | Exit enclosing loop at the given nesting depth. |
| BreakIf | u16 depth | 1 | Pop boolean. If true, exit enclosing loop at the given nesting depth. |

## Function Calls

Native function calls partition into two classes distinguished by the source-level `use` declaration and a matching host-side registration ABI:

- **Verified natives.** Imported with `use module::name`. Host registers through `Vm::register_verified_native(name, fn, wcet_bound, wcmu_bound)`. The host-attested cost folds into the iteration's WCET and WCMU budget. Compiler emits `CallVerifiedNative`.
- **External natives.** Imported with `use external module::name`. Host registers through `Vm::register_external_native(name, fn, max_invocations_per_iteration)`. The host attests an upper bound on per-iteration invocation count rather than per-call cost. Compiler emits `CallExternalNative`.

The runtime cross-checks each declared native against its host registration at the entry of `Vm::call_function` and at every explicit invocation of `Vm::verify_native_classifications`. The check walks every native call site in the module and rejects a mismatch (e.g., a bytecode importing `use math::sqrt` but a host registering `sqrt` through `register_external_native`) as `VmError::VerifyError`. The result is cached after the first successful walk; any `register_*` call or `replace_module` invocation invalidates the cache. Hosts that prefer to surface mismatches at a deployment-validation step rather than at first call may invoke `verify_native_classifications` explicitly after registration.

V0.2.0 Phase 5 introduced the verified-versus-external split. The legacy `Op::CallNative` opcode was retired; every native call site compiles to either `Op::CallVerifiedNative` or `Op::CallExternalNative`. Hosts that previously called `Vm::register_native` continue to register verified natives because that method ascribes the verified classification.

The `max_invocations_per_iteration` attestation on external natives is recorded at registration but is not yet folded into the WCMU bound. The verifier treats external natives as contributing zero to the script's per-iteration WCMU budget; the host accepts that external natives are outside the script's resource contract and has separately verified their resource use. The chunk-level integration (which would bound external-native cost as `max_invocations_per_iteration * per_call_wcmu` per chunk rather than per static call site) is forward-looking work tracked under B20.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Call | u16 chunk_idx, u8 argc | 10 | Direct call to a compiled chunk by index with `argc` arguments. |
| CallVerifiedNative | u16 native_idx, u8 argc | 10 | Call a verified native function. Cost folds into the iteration budget per host attestation. The low seven bits of `argc` are the argument count. The high bit is the error-reify flag. When it is set, the call pushes the value and flag `0` on success and the `Word` error code and flag `1` on a soft host failure instead of propagating it. The compiler sets it on every native call. |
| CallExternalNative | u16 native_idx, u8 argc | 10 | Call an external native function. Iteration cost budget pauses during the call; the verifier tracks invocation count per iteration. `argc` carries the error-reify flag as for `CallVerifiedNative`. |
| Return | none | 2 | Return from the current chunk. |

The closure-construction and indirect-dispatch opcodes (`PushFunc`, `MakeClosure`, `MakeRecursiveClosure`, `CallIndirect`) are not present in the ISA. Closure-shaped surface expressions are rejected at the type-checker stage with a diagnostic that names the construct; first-class function values are likewise rejected. The `Value::Func` runtime variant was retired alongside the opcodes in V0.2.0 Phase 4. Surface programs that previously used closures must be rewritten as top-level `fn` definitions or trait methods.

## Coroutine and Streaming

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Yield | none | 1 | Pop output value and suspend. On resume, the host's input value is pushed onto the stack. |
| Stream | none | 1 | Entry of the streaming region. Only `Reset` may target it. |
| Reset | none | 1 | Clear the arena's top region, activate hot swap if scheduled, jump to the matching `Stream`. |

## Stack

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| PopN | u8 count | 1 | Discard `count` values from the top of the stack. The compiler emits `PopN(1)` for single-slot pops and `PopN(n)` for multi-slot discards, including the three-slot discard after checked-arithmetic opcodes when the wrapping semantics is wanted. |
| Dup | none | 1 | Duplicate top of stack. |

## Type Construction

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| NewComposite | kind, count, byte_size or meta | 5 | Pop `count` values and construct one composite of the given kind (struct, tuple, array, or enum). The flat form packs the popped values into `byte_size` bytes; the boxed form builds a heap composite from the template index `meta`. An enum's leading discriminant counts as one of the `count` values. The single opcode replaces the four V0.2.0 construct opcodes (wire ids 34-37, retired). |

A tuple is an anonymous struct, an array a homogeneous struct, and a flat enum a struct whose first packed value is the discriminant, so flat construction is one operation across all four kinds. The operand carries the composite kind, the operand-stack pop count, and either the exact flat allocation size in bytes (flat form) or a struct-template index (boxed form). The flat byte size is the precise worst-case-memory-usage contribution the verifier sums; see the Heap allocation table below.

The `Option::None` sentinel and `Option::Some` wrap are handled through `PushImmediate(3)` and the natural representation of the wrapped value, respectively; there are no dedicated `PushNone` or `WrapSome` opcodes.

## Field Access

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| GetField | `StructField` (flat/nested/boxed) | 3 | Pop struct, push field value. The baked `StructField` operand selects a flat read at a byte offset, a nested-composite extraction, or a boxed positional/named lookup. |
| GetIndex | `ArrayElem` (flat/nested/boxed) | 2 or 8 | Flat Word and Byte, nested composite and boxed forms pop array and index and push the value, index, four Word byte limbs of the bound, and an outcome flag. Invalid indices yield a placeholder and flag one without reading an element. The placeholder is typed zero for Word and Byte, a zero-filled body of the element's size for a nested composite, and `Unit` for a boxed element. Valid indices yield the element and flag zero. These forms cost eight. The nested form charges its element size as heap allocation. Other flat scalar forms retain a single result and cost two, and remain outside the first-pass trap-free proof. |
| GetTupleField | `TupleField` (flat/nested/boxed) | 2 | Pop tuple, push element. The baked `TupleField` operand selects a flat read at a byte offset, a nested-composite extraction, or a boxed positional index. |
| GetEnumField | `EnumField` (flat/nested/boxed) | 2 | Pop enum variant, push payload field. The baked `EnumField` operand selects a flat read past the discriminant word, a nested-composite extraction, or a boxed positional index. |
| Len | none | 2 | Pop composite value (Array, Text, Tuple), push length as Int. |

## Type Testing

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| IsEnum | u16 enum-name, u16 variant-name, u16 discriminant-value (all constant indices) | 3 | Peek the top of the stack; push true if it matches the enum type and variant. |
| IsStruct | u16 name | 3 | Peek the top of the stack; push true if it matches the struct type. |

## Casting and Fixed-Point Arithmetic

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| IntToFloat | none | 2 | Pop Word, push as Float. Gated on the `floats` feature. |
| FloatToInt | none | 2 | Pop Float, push as Word (truncates toward zero). Gated on the `floats` feature. |
| WordToByte | none | 2 | Pop Word, push the low 8 bits as a Byte. |
| ByteToWord | none | 2 | Pop Byte, zero-extend to Word. |
| WordToFixed | u8 frac_bits | 2 | Pop Word, push the corresponding Q-format Fixed value with the given fraction-bit count. |
| FixedToWord | u8 frac_bits | 2 | Pop Fixed, push the integer portion as a Word; saturating. |
| FixedMul | u8 frac_bits | 2 | Pop two Q-format Fixed values, push their product. Shifts the `i128` product right by the fraction-bit count and saturates. |
| FixedDiv | u8 frac_bits | 2 | Pop two Q-format Fixed values and push `(low, high, flag)`, the outcome triple of a Fixed `Mod`. The low slot is the quotient, computed by left-shifting the dividend by the fraction-bit count before dividing, and saturated. The high slot is `Fixed(0)` and the flag is `0`, saturated or not. A zero divisor pushes the numerator, `Fixed(0)` and flag `3` and does not fault. The compiler's default dispatch traps on flag `3`. |

## Faults

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Trap | u16 kind code | 1 or 6 | Halt with the error selected by TrapKind. Code `8` consumes five Words, the index followed by four little-endian byte limbs of an unsigned 32 bit bound, and returns IndexOutOfBounds. The runtime uses the low eight bits of each limb. Its nominal cost is `6`. Code `9`, HostError, consumes the `Word` code of a reified native failure and returns the host's own error when the code matches the most recently reified failure, and otherwise `NativeErrorCode` with that code. Other trap kinds cost `1`. Unknown kind codes and malformed payloads are contract errors. |

## Opcode count and operand-shape inventory

The instruction set contains 66 opcodes. The B28 consolidation retired the four V0.2.0 construct opcodes (`NewStruct`, `NewEnum`, `NewArray`, `NewTuple`, wire ids 34-37) in favour of the single `NewComposite` opcode (wire id 69). The retired ids are reserved and not reused; the maximum live wire id is 69. Operand shapes:

The shapes below are the `Op`-variant operand types as defined in `src/bytecode.rs`, which differ from the flattened inline wire encodings documented in [WIRE_FORMAT.md](./WIRE_FORMAT.md). In particular the four baked field-and-element access opcodes carry bespoke operand structs rather than a plain integer.

| Shape | Used by |
|-------|---------|
| None (zero-operand) | 33 opcodes (arithmetic, comparison, bit ops, type coercions, stack manipulation, streaming, coroutine, etc.) |
| `u8` | 8 opcodes (`PushImmediate`, `PopN`, `WordToFixed`, `FixedToWord`, `FixedMul`, `FixedDiv`, `CheckedMul`, `CheckedDiv`) |
| `u16` | 12 opcodes (`Const`, `GetLocal`, `SetLocal`, `IsStruct`, `If`, `Else`, `Loop`, `EndLoop`, `Break`, `BreakIf`, `BoundsCheck`, `Trap`) |
| `(u16, u8)` | 3 opcodes (`Call`, `CallVerifiedNative`, `CallExternalNative`) |
| `u32` | 2 opcodes (`GetData`, `SetData`), encoded as u24 on the wire |
| `(u32, u32)` | 2 opcodes (`GetDataIndexed`, `SetDataIndexed`), encoded as two u24 values on the wire |
| `(u16, u16, u16)` | 1 opcode (`IsEnum`, carrying the enum-name, variant-name, and discriminant-value constant indices) |
| Baked access operand (bespoke) | 4 opcodes (`GetField(StructField)`, `GetIndex(ArrayElem)`, `GetTupleField(TupleField)`, `GetEnumField(EnumField)`). Each carries a bespoke B28 operand struct with a compiler-selected `Flat`, `FlatNested`, or `Boxed` form. The flat forms bake a byte offset and a scalar `kind` (or a nested composite `size` and `variant`); the boxed forms carry a positional index or a field-name constant index for the pre-B28 `Vec` body. See the `StructField`, `ArrayElem`, `TupleField`, and `EnumField` enums in `src/bytecode.rs`. |
| NewComposite (bespoke) | 1 opcode (`NewComposite`). The flat form packs the composite kind, the operand-stack pop count (0 through 62), and the exact flat byte size into the three operand bytes of the record. The boxed form, or a flat field count above 62, spills a 24-bit operand-pool index to a `(u16, u16, u8)` entry carrying `(count, byte_size-or-meta, boxed_flag)`. |

The wire encoding flattens these `Op`-level shapes: 62 of the 66 opcodes always carry their operand inline in the 4-byte opcode record. Three opcodes (`GetDataIndexed`, `SetDataIndexed`, and `IsEnum`) always reference an entry in the operand pool by index. `NewComposite` carries its operand inline in the common small flat form and references a `(u16, u16, u8)` operand-pool entry only for the boxed form or a flat field count above 62. The `(u16, u8)` opcodes (`Call`, `CallVerifiedNative`, `CallExternalNative`) fit inline because the `u8` lands in byte 3 of the record. The four baked access opcodes fit their compiler-selected operand form inline. See [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) and [WIRE_FORMAT.md](./WIRE_FORMAT.md) for the wire format that encodes these shapes.

## Cost Summary

The cost groupings reproduce `bytecode::nominal_op_cycles`. Hosts that need wall-clock WCET supply a custom `CostModel` calibrated to the target.

| Cost | Instructions |
|------|-------------|
| 1 | Const, PushImmediate, GetLocal, SetLocal, GetData, SetData, Dup, Not, If, Else, EndIf, Loop, EndLoop, Break, BreakIf, Stream, Reset, Yield, Trap except kind 8, PopN |
| 2 | Add, Sub, Mul, Neg, CheckedAdd, CheckedSub, CheckedMul, CheckedNeg, CheckedDiv, CheckedMod, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, GetIndex other flat scalar forms, GetTupleField, GetEnumField, Len, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, FixedMul, FixedDiv, Return, BitAnd, BitOr, BitXor, Shl, Shr |
| 3 | GetField, IsEnum, IsStruct |
| 5 | Div, Mod, NewComposite |
| 6 | Trap kind 8 |
| 7 | BoundsCheck |
| 8 | GetDataIndexed, SetDataIndexed, GetIndex flat Word or Byte, nested or boxed |
| 10 | Call, CallVerifiedNative, CallExternalNative |

## WCMU contributions

WCMU costs are reported separately as stack slot growth, stack slot shrink, and heap allocation in bytes. The constant `VALUE_SLOT_SIZE_BYTES` converts slot counts to bytes; the parametric `Vm<W, A, F>` shape uses `size_of::<GenericValue<W, F>>()` directly. Computed by `wcmu_stream_iteration()` in `src/verify.rs`.

### Stack growth (peak net delta during execution)

The values reproduce `Op::stack_growth` in `src/bytecode.rs`. For multi-output opcodes the value is the net peak delta against the starting depth, not the raw push count: e.g. `CheckedAdd` pops two and pushes three, so the peak depth relative to the start is `+1`. `IsEnum` and `IsStruct` peek the scrutinee (no pop) and push a Bool, so their net delta is `+1`.

| Growth | Instructions |
|--------|--------------|
| 0 | Not, Neg, Add, Sub, Mul, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, SetLocal, SetData, If, BreakIf, Else, EndIf, Loop, EndLoop, Break, Stream, Reset, Yield, Return, GetField, GetIndex other flat scalar forms, GetTupleField, GetEnumField, Len, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, FixedMul, Trap, PopN, BitAnd, BitOr, BitXor, Shl, Shr |
| 1 | Const, PushImmediate, GetLocal, GetData, Dup, Div, Mod, FixedDiv, CheckedAdd, CheckedSub, CheckedMul, CheckedDiv, CheckedMod, Call, CallVerifiedNative and CallExternalNative without the reify flag, NewComposite, IsEnum, IsStruct |
| 2 | CheckedNeg, CallVerifiedNative and CallExternalNative with the reify flag |
| 4 | SetDataIndexed |
| 5 | GetDataIndexed, BoundsCheck, GetIndex flat Word or Byte, nested or boxed |

### Stack shrink (slots popped during execution)

The values reproduce `Op::stack_shrink`. For opcodes whose net delta is non-negative (e.g. `CheckedAdd`, `CheckedNeg`) the shrink reads zero because the verifier accounts for the peak through `stack_growth` and there is no net pop.

| Shrink | Instructions |
|--------|--------------|
| 0 | Const, PushImmediate, GetLocal, GetData, Dup, Not, Neg, Div, Mod, CheckedAdd, CheckedSub, CheckedMul, CheckedNeg, CheckedDiv, CheckedMod, GetDataIndexed, SetDataIndexed, BoundsCheck, GetIndex flat Word or Byte, nested or boxed, Else, EndIf, Loop, EndLoop, Break, Stream, Reset, Return, Len, IsEnum, IsStruct, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, FixedDiv, Trap except kinds 8 and 9 |
| 1 | Add, Sub, Mul, FixedMul, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, SetLocal, SetData, If, BreakIf, Yield, GetField, GetIndex other flat scalar forms, GetTupleField, GetEnumField, BitAnd, BitOr, BitXor, Shl, Shr |
| 1 | Trap kind 9 |
| 5 | Trap kind 8 |
| n | PopN(n), Call(_, n), CallVerifiedNative(_, n) and CallExternalNative(_, n) with the reify flag masked out of `n`, NewComposite(count) |

### Heap allocation (bytes)

| Heap | Instructions |
|------|--------------|
| 0 | All instructions not listed below |
| `byte_size` from operand | NewComposite, flat form. The exact flat allocation size is baked into the operand at compile time, so the worst-case-memory-usage contribution is the precise byte count rather than a `count * VALUE_SLOT_SIZE_BYTES` estimate. The boxed form reports zero flat bytes; its body is the heap `Vec`, accounted separately. |
| element `size` from operand | GetIndex, nested composite form. An invalid index allocates a zero-filled placeholder of the element's size, and the bound charges it on every execution because validity is not known statically. |
| host-attested | CallVerifiedNative through host registration; CallExternalNative through per-iteration invocation-count bound |
