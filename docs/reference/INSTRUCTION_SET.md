# Instruction Set

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

The Keleusma VM executes a stack-based bytecode using block-structured control flow. All instructions operate on a value stack. This document lists every instruction with its operands, behavior, and cost contribution to the WCET (worst-case execution time) and WCMU (worst-case memory usage) analyses.

Each instruction carries a relative integer cost. Costs are unitless relative weights, not cycle counts. Higher values indicate more expensive operations. The cost table is consulted by `wcet_stream_iteration()`; the per-instruction stack and heap effects are consulted by `wcmu_stream_iteration()`.

For details on how bytecode is generated from source, see [COMPILATION_PIPELINE.md](../architecture/COMPILATION_PIPELINE.md). For the bytecode wire format including the framing header, opcode-stream encoding, and operand pool, see [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md). For the structural ISA specification including block hierarchy and verification rules, see [TARGET_ISA.md](./TARGET_ISA.md).

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

The unified slot index space partitions into shared slots `[0, shared_count)` and private slots `[shared_count, shared_count + private_count)`. Shared slots are host-accessible through `Vm::set_data`/`Vm::get_data` and live in the Vm's owned vector. Private slots are script-only and live in the arena's persistent region. The opcodes below admit both partitions; the runtime dispatches by comparing the slot index against the cached `shared_slot_count`. Const data fields do not consume a slot; field reads compile to `Const` and writes are compile errors.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| GetData | u16 slot | 1 | Push data segment slot value onto stack. |
| SetData | u16 slot | 1 | Pop value and store into data segment slot. |
| GetDataIndexed | u16 base, u16 len | 2 | Pop array index, bounds-check against `len`, push the value at `base + index`. |
| SetDataIndexed | u16 base, u16 len | 2 | Pop array index then pop value, bounds-check against `len`, store into the slot at `base + index`. |
| BoundsCheck | u16 bound | 1 | Peek the top of the stack as an `Int`, trap if outside `[0, bound)`. Emitted by the compiler between levels of a multi-dimensional indexed access. |

## Arithmetic

Integer arithmetic uses the checked-arithmetic family. Each `CheckedAdd`, `CheckedSub`, `CheckedMul`, and `CheckedNeg` opcode pops `Value::Int` operands (two for the binary forms, one for `CheckedNeg`), computes the true result in `i128`, and pushes three slots: the low half, the high half, and an outcome flag (`Int(0)` ok, `Int(1)` overflow, `Int(2)` underflow). The push order places `low` at the bottom and `flag` on top so that surface-level wrapping expressions, such as `a + b` on `Int` operands, compile to the checked opcode followed by `PopN(2)` and leave the wrapping result on the stack. Source-level pattern-arm matching destructures the three outputs.

The wrapping arithmetic opcodes `Add`, `Sub`, `Mul`, and `Neg` remain in the instruction set but no longer accept `Value::Int` operands. Their permitted operand types are `Byte`, `Fixed`, and `Float`. The V0.2.0 Consolidation B pass narrowed these opcodes by routing all `Int` arithmetic through the checked family; the compiler emits `CheckedXxx; PopN(2)` for every `Int` operand position. Operands whose type the compiler cannot statically infer fall through to the `Int` path as well, because `Word` is the default numeric type.

`Op::Div` and `Op::Mod` remain polymorphic over `Int`, `Byte`, and `Float`. Their checked counterparts `CheckedDiv` and `CheckedMod` expose the corner cases of signed division.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| CheckedAdd | none | 3 | Pop two `Int` operands; push `(low, high, flag)`. |
| CheckedSub | none | 3 | Pop two `Int` operands; push `(low, high, flag)`. |
| CheckedMul | none | 4 | Pop two `Int` operands; push the full 128-bit product split into `(low, high, flag)`. The high half is the load-bearing value for big-number multiplication. |
| CheckedNeg | none | 2 | Pop one `Int` operand; push `(low, high, flag)`. The only overflow case is `-i64::MIN`. |
| CheckedDiv | none | 4 | Pop two `Int` operands; push `(low, high, flag)`. Traps on divide-by-zero. The only overflow case is `i64::MIN / -1`. |
| CheckedMod | none | 4 | Pop two `Int` operands; push `(low, high, flag)`. Traps on divide-by-zero. The only overflow case is `i64::MIN % -1`. |
| Add | none | 2 | Pop two operands of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 sum. The `Int` operand position is excluded; the compiler routes `Int + Int` through `CheckedAdd; PopN(2)`. |
| Sub | none | 2 | Pop two operands of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 difference. The `Int` operand position is excluded. |
| Mul | none | 2 | Pop two operands of type `Byte` or `Float`; push the wrapping or IEEE 754 product. `Fixed` multiplication uses `FixedMul(n)`; the `Int` operand position is excluded. |
| Neg | none | 1 | Pop one operand of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 negation. The `Int` operand position is excluded. |
| Div | none | 3 | Pop two values; push quotient. Traps on divide-by-zero. No overflow flag. |
| Mod | none | 3 | Pop two values; push remainder. Traps on divide-by-zero. No overflow flag. |

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

Block-structured control flow opcodes carry `u16` jump targets. A chunk's opcode count is therefore bounded at 65,535. The compiler emits a soft warning when a single chunk crosses 80% of the limit, prompting decomposition into helper functions; the bytecode at the limit remains valid.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| If | u16 offset | 2 | Pop boolean. If false, skip forward to matching Else or EndIf. |
| Else | u16 offset | 1 | Unconditional skip forward to matching EndIf. |
| EndIf | none | 1 | End of if or if-else block. No-op. |
| Loop | u16 offset | 1 | Start of loop block. Offset is distance to matching EndLoop. |
| EndLoop | u16 offset | 1 | Unconditional jump backward to matching Loop. |
| Break | u16 depth | 1 | Exit enclosing loop at the given nesting depth. |
| BreakIf | u16 depth | 2 | Pop boolean. If true, exit enclosing loop at the given nesting depth. |

## Function Calls

Native function calls partition into two classes distinguished by the source-level `use` declaration and a matching host-side registration ABI:

- **Verified natives.** Imported with `use module::name`. Host registers through `Vm::register_verified_native(name, fn, wcet_bound, wcmu_bound)`. The host-attested cost folds into the iteration's WCET and WCMU budget. Compiler emits `CallVerifiedNative`.
- **External natives.** Imported with `use external module::name`. Host registers through `Vm::register_external_native(name, fn, max_invocations_per_iteration)`. The host attests an upper bound on per-iteration invocation count rather than per-call cost. Compiler emits `CallExternalNative`.

The runtime cross-checks each declared native against its host registration at `Vm::new`. A mismatch (e.g., a bytecode importing `use math::sqrt` but a host registering `sqrt` through `register_external_native`) is rejected at load time.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Call | u16 chunk_idx, u8 argc | 5 | Direct call to a compiled chunk by index with `argc` arguments. |
| CallVerifiedNative | u16 native_idx, u8 argc | 5 | Call a verified native function. Cost folds into the iteration budget per host attestation. |
| CallExternalNative | u16 native_idx, u8 argc | 5 | Call an external native function. Iteration cost budget pauses during the call; the verifier tracks invocation count per iteration. |
| Return | none | 2 | Return from the current chunk. |

The closure-construction and indirect-dispatch opcodes (`PushFunc`, `MakeClosure`, `MakeRecursiveClosure`, `CallIndirect`) are not present in the ISA. Closure-shaped surface expressions are rejected at the type-checker stage with a diagnostic that names the construct; first-class function values are likewise rejected. The `Value::Func` runtime variant was retired alongside the opcodes in V0.2.0 Phase 4. Surface programs that previously used closures must be rewritten as top-level `fn` definitions or trait methods.

## Coroutine and Streaming

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Yield | none | 5 | Pop output value and suspend. On resume, the host's input value is pushed onto the stack. |
| Stream | none | 1 | Entry of the streaming region. Only `Reset` may target it. |
| Reset | none | 4 | Clear the arena's top region, activate hot swap if scheduled, jump to the matching `Stream`. |

## Stack

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| PopN | u8 count | 1 | Discard `count` values from the top of the stack. The compiler emits `PopN(1)` for single-slot pops and `PopN(n)` for multi-slot discards, including the three-slot discard after checked-arithmetic opcodes when the wrapping semantics is wanted. |
| Dup | none | 1 | Duplicate top of stack. |

## Type Construction

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| NewStruct | u16 template | 3 | Pop field values, create struct from template. |
| NewEnum | u16 type, u16 variant, u8 fields | 3 | Pop field values, create enum variant. |
| NewArray | u16 length | 3 | Pop N values, create array. |
| NewTuple | u8 length | 3 | Pop N values, create tuple. |

The `Option::None` sentinel and `Option::Some` wrap are handled through `PushImmediate(3)` and the natural representation of the wrapped value, respectively; there are no dedicated `PushNone` or `WrapSome` opcodes.

## Field Access

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| GetField | u16 name index | 2 | Pop struct, push named field value. |
| GetIndex | none | 3 | Pop index and array, push element. |
| GetTupleField | u8 index | 2 | Pop tuple, push element at index. |
| GetEnumField | u8 index | 2 | Pop enum variant, push field at index. |
| Len | none | 2 | Pop composite value (Array, Text, Tuple), push length as Int. |

## Type Testing

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| IsEnum | u16 type, u16 variant | 2 | Peek the top of the stack; push true if it matches the enum type and variant. |
| IsStruct | u16 name | 2 | Peek the top of the stack; push true if it matches the struct type. |

## Casting and Fixed-Point Arithmetic

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| IntToFloat | none | 2 | Pop Word, push as Float. Gated on the `floats` feature. |
| FloatToInt | none | 2 | Pop Float, push as Word (truncates toward zero). Gated on the `floats` feature. |
| WordToByte | none | 1 | Pop Word, push the low 8 bits as a Byte. |
| ByteToWord | none | 1 | Pop Byte, zero-extend to Word. |
| WordToFixed | u8 frac_bits | 2 | Pop Word, push the corresponding Q-format Fixed value with the given fraction-bit count. |
| FixedToWord | u8 frac_bits | 2 | Pop Fixed, push the integer portion as a Word; saturating. |
| FixedMul | u8 frac_bits | 4 | Pop two Q-format Fixed values, push their product. Shifts the `i128` product right by the fraction-bit count and saturates. |
| FixedDiv | u8 frac_bits | 4 | Pop two Q-format Fixed values, push their quotient. Left-shifts the dividend by the fraction-bit count before dividing and saturates. |

## Faults

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Trap | u16 message index | 1 | Halt execution with an error message from the constant pool. |

## Opcode count and operand-shape inventory

The instruction set contains 65 opcodes. Operand shapes:

| Shape | Used by |
|-------|---------|
| None (zero-operand) | 38 opcodes (arithmetic, comparison, bit ops, type coercions, stack manipulation, streaming, coroutine, etc.) |
| `u8` | 9 opcodes (`PushImmediate`, `PopN`, `GetTupleField`, `GetEnumField`, `NewTuple`, `WordToFixed`, `FixedToWord`, `FixedMul`, `FixedDiv`) |
| `u16` | 17 opcodes (`Const`, `GetLocal`, `SetLocal`, `GetData`, `SetData`, `GetField`, `IsStruct`, `NewStruct`, `NewArray`, `If`, `Else`, `Loop`, `EndLoop`, `Break`, `BreakIf`, `BoundsCheck`, `Trap`) |
| `(u16, u8)` | 3 opcodes (`Call`, `CallVerifiedNative`, `CallExternalNative`) |
| `(u16, u16)` | 3 opcodes (`GetDataIndexed`, `SetDataIndexed`, `IsEnum`) |
| `(u16, u16, u8)` | 1 opcode (`NewEnum`) |

58 of 65 opcodes carry their operand inline in the 4-byte opcode record; the 7 opcodes with compound operands reference the bytecode's operand pool. See [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) for the wire format that encodes these shapes.

## Cost Summary

| Cost | Instructions |
|------|-------------|
| 1 | Const, PushImmediate, GetLocal, SetLocal, GetData, SetData, BoundsCheck, PopN, Dup, Not, If, Else, EndIf, Loop, EndLoop, Break, BreakIf, Stream, Trap, WordToByte, ByteToWord, BreakIf |
| 2 | CheckedNeg, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, BitAnd, BitOr, BitXor, Shl, Shr, GetField, GetTupleField, GetEnumField, Len, IsEnum, IsStruct, IntToFloat, FloatToInt, WordToFixed, FixedToWord, GetDataIndexed, SetDataIndexed, Return, If, BreakIf |
| 3 | CheckedAdd, CheckedSub, Div, Mod, GetIndex, NewStruct, NewEnum, NewArray, NewTuple |
| 4 | CheckedMul, CheckedDiv, CheckedMod, FixedMul, FixedDiv, Reset |
| 5 | Call, CallVerifiedNative, CallExternalNative, Yield |

## WCMU contributions

WCMU costs are reported separately as stack slot growth, stack slot shrink, and heap allocation in bytes. The constant `VALUE_SLOT_SIZE_BYTES` converts slot counts to bytes; the parametric `Vm<W, A, F>` shape uses `size_of::<GenericValue<W, F>>()` directly. Computed by `wcmu_stream_iteration()` in `src/verify.rs`.

### Stack growth (slots pushed during execution)

| Growth | Instructions |
|--------|--------------|
| 0 | SetLocal, SetData, PopN, If, BreakIf, Else, EndIf, Loop, EndLoop, Break, Stream, Reset, Yield, Return, Not, CmpEq through CmpGe, BitAnd, BitOr, BitXor, Shl, Shr, GetField, GetIndex, GetTupleField, GetEnumField, Len, IsEnum, IsStruct, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, Trap |
| 1 | Const, PushImmediate, GetLocal, GetData, Dup, Call, CallVerifiedNative, CallExternalNative, NewStruct, NewEnum, NewArray, NewTuple |
| 3 | CheckedAdd, CheckedSub, CheckedMul, CheckedNeg, CheckedDiv, CheckedMod (push `(high, low, flag)`) |

### Stack shrink (slots popped during execution)

| Shrink | Instructions |
|--------|--------------|
| 0 | Const, PushImmediate, GetLocal, GetData, Dup, Else, EndIf, Loop, EndLoop, Break, Stream, Reset, Return, Not, NewStruct (template-driven), Len, IsEnum, IsStruct, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, Trap |
| 1 | SetLocal, SetData, If, BreakIf, Yield, CheckedNeg, CmpEq through CmpGe, BitAnd, BitOr, BitXor, Shl, Shr, GetField, GetIndex, GetTupleField, GetEnumField |
| 2 | CheckedAdd, CheckedSub, CheckedMul, CheckedDiv, CheckedMod, Div, Mod, FixedMul, FixedDiv |
| n | PopN(n), Call(_, n), CallVerifiedNative(_, n), CallExternalNative(_, n), NewEnum(_, _, n), NewArray(n), NewTuple(n) |

### Heap allocation (bytes)

| Heap | Instructions |
|------|--------------|
| 0 | All instructions not listed below |
| n * `VALUE_SLOT_SIZE_BYTES` | NewStruct (n = field count from template), NewEnum(_, _, n), NewArray(n), NewTuple(n) |
| host-attested | CallVerifiedNative through host registration; CallExternalNative through per-iteration invocation-count bound |
