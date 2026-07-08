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
| GetData | u16 slot | 1 | Push data segment slot value onto stack. |
| SetData | u16 slot | 1 | Pop value and store into a data segment slot. A scalar stores inline; a flat composite copies its body into the persistent composite pool at the offset the module's private-composite layout table records for the slot (no dedicated composite-write opcode). |
| GetDataIndexed | u16 base, u16 len | 2 | Pop array index, bounds-check against `len`, push the value at `base + index`. |
| SetDataIndexed | u16 base, u16 len | 2 | Pop array index then pop value, bounds-check against `len`, store into the slot at `base + index`. |
| BoundsCheck | u16 bound | 2 | Peek the top of the stack as an `Int`, trap if outside `[0, bound)`. Emitted by the compiler between levels of a multi-dimensional indexed access. |

## Arithmetic

Integer arithmetic uses the checked-arithmetic family. Each `CheckedAdd`, `CheckedSub`, `CheckedMul`, and `CheckedNeg` opcode pops `Value::Int` operands (two for the binary forms, one for `CheckedNeg`), computes the true result in `i128`, and pushes three slots: the low half, the high half, and an outcome flag (`Int(0)` ok, `Int(1)` overflow, `Int(2)` underflow). The push order places `low` at the bottom and `flag` on top so that surface-level wrapping expressions, such as `a + b` on `Int` operands, compile to the checked opcode followed by `PopN(2)` and leave the wrapping result on the stack. Source-level pattern-arm matching destructures the three outputs.

The wrapping arithmetic opcodes `Add`, `Sub`, `Mul`, and `Neg` remain in the instruction set but no longer accept `Value::Int` operands. Their permitted operand types are `Byte`, `Fixed`, and `Float`. The V0.2.0 Consolidation B pass narrowed these opcodes by routing all `Int` arithmetic through the checked family; the compiler emits `CheckedXxx; PopN(2)` for every `Int` operand position. Operands whose type the compiler cannot statically infer fall through to the `Int` path as well, because `Word` is the default numeric type.

`Op::Div` and `Op::Mod` remain polymorphic over `Int`, `Byte`, and `Float`. Their checked counterparts `CheckedDiv` and `CheckedMod` expose the corner cases of signed division.

`CheckedMul` and `CheckedDiv` carry a `u8` fraction-bit count that selects integer or `Q`-format arithmetic, where `0` is integer and a positive count is `Fixed`. This is the only place the integer and fixed-point datapaths differ, namely a shift by the fraction-bit count around the multiply or divide, so a single parameterized opcode serves both rather than separate opcodes. `CheckedAdd`, `CheckedSub`, `CheckedMod`, and `CheckedNeg` need no such parameter because their `Fixed` forms involve no shift, and they dispatch on the operand type alone.

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| CheckedAdd | none | 2 | Pop two `Int` operands; push `(low, high, flag)`. The `flag` and `high` halves report at the bytecode-declared word width through the shared `vm::checked_arith_outputs` helper. |
| CheckedSub | none | 2 | Pop two `Int` operands; push `(low, high, flag)`. |
| CheckedMul | u8 frac_bits | 2 | Pop two operands; push `(low, high, flag)`. The fraction-bit count selects the format. With `0` it is integer multiply and the high half is the load-bearing value for big-number multiplication. With a count greater than `0` the operands are `Fixed`, the `i128` product is shifted right by that many bits before the range check, the wrapped result is a single word, and the high slot is unused. So `0` fraction bits is exactly integer multiply. |
| CheckedNeg | none | 2 | Pop one `Int` operand; push `(low, high, flag)`. The only overflow case under the default 64-bit declared width is `-i64::MIN`. |
| CheckedDiv | u8 frac_bits | 2 | Pop two operands; push `(low, high, flag)`. The fraction-bit count selects the format. With `0` it is integer divide whose only overflow case at the default 64-bit width is `i64::MIN / -1`. With a count greater than `0` the operands are `Fixed` and the dividend is left-shifted by that many bits in the `i128` domain before dividing. A zero divisor reifies as flag `3` carrying the numerator rather than trapping. So `0` fraction bits is exactly integer divide. |
| CheckedMod | none | 2 | Pop two `Int` operands; push `(low, high, flag)`. Traps on divide-by-zero. The only overflow case under the default 64-bit declared width is `i64::MIN % -1`. |
| Add | none | 2 | Pop two operands of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 sum. The `Int` operand position is excluded; the compiler routes `Int + Int` through `CheckedAdd; PopN(2)`. |
| Sub | none | 2 | Pop two operands of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 difference. The `Int` operand position is excluded. |
| Mul | none | 2 | Pop two operands of type `Byte` or `Float`; push the wrapping or IEEE 754 product. `Fixed` multiplication uses `FixedMul(n)`; the `Int` operand position is excluded. |
| Neg | none | 2 | Pop one operand of type `Byte`, `Fixed`, or `Float`; push the wrapping or IEEE 754 negation. The `Int` operand position is excluded. |
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
| CallVerifiedNative | u16 native_idx, u8 argc | 10 | Call a verified native function. Cost folds into the iteration budget per host attestation. |
| CallExternalNative | u16 native_idx, u8 argc | 10 | Call an external native function. Iteration cost budget pauses during the call; the verifier tracks invocation count per iteration. |
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
| GetField | u16 name index | 3 | Pop struct, push named field value. |
| GetIndex | none | 2 | Pop index and array, push element. |
| GetTupleField | u8 index | 2 | Pop tuple, push element at index. |
| GetEnumField | u8 index | 2 | Pop enum variant, push field at index. |
| Len | none | 2 | Pop composite value (Array, Text, Tuple), push length as Int. |

## Type Testing

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| IsEnum | u16 type, u16 variant | 3 | Peek the top of the stack; push true if it matches the enum type and variant. |
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
| FixedDiv | u8 frac_bits | 2 | Pop two Q-format Fixed values, push their quotient. Left-shifts the dividend by the fraction-bit count before dividing and saturates. |

## Faults

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Trap | u16 message index | 1 | Halt execution with an error message from the constant pool. |

## Opcode count and operand-shape inventory

The instruction set contains 66 opcodes. The B28 consolidation retired the four V0.2.0 construct opcodes (`NewStruct`, `NewEnum`, `NewArray`, `NewTuple`, wire ids 34-37) in favour of the single `NewComposite` opcode (wire id 69). The retired ids are reserved and not reused; the maximum live wire id is 69. Operand shapes:

| Shape | Used by |
|-------|---------|
| None (zero-operand) | 34 opcodes (arithmetic, comparison, bit ops, type coercions, stack manipulation, streaming, coroutine, etc.) |
| `u8` | 10 opcodes (`PushImmediate`, `PopN`, `GetTupleField`, `GetEnumField`, `WordToFixed`, `FixedToWord`, `FixedMul`, `FixedDiv`, `CheckedMul`, `CheckedDiv`) |
| `u16` | 15 opcodes (`Const`, `GetLocal`, `SetLocal`, `GetData`, `SetData`, `GetField`, `IsStruct`, `If`, `Else`, `Loop`, `EndLoop`, `Break`, `BreakIf`, `BoundsCheck`, `Trap`) |
| `(u16, u8)` | 3 opcodes (`Call`, `CallVerifiedNative`, `CallExternalNative`) |
| `(u16, u16)` | 3 opcodes (`GetDataIndexed`, `SetDataIndexed`, `IsEnum`) |
| NewComposite (bespoke) | 1 opcode (`NewComposite`). The flat form packs the composite kind, the operand-stack pop count (0 through 62), and the exact flat byte size into the three operand bytes of the record. The boxed form, or a flat field count above 62, spills a 24-bit operand-pool index to a `(u16, u16, u8)` entry carrying `(count, byte_size-or-meta, boxed_flag)`. |

62 of the 66 opcodes always carry their operand inline in the 4-byte opcode record. Three opcodes (`GetDataIndexed`, `SetDataIndexed`, and `IsEnum`, all `(u16, u16)`) always reference an entry in the operand pool by index. `NewComposite` carries its operand inline in the common small flat form and references a `(u16, u16, u8)` operand-pool entry only for the boxed form or a flat field count above 62. The `(u16, u8)` opcodes (`Call`, `CallVerifiedNative`, `CallExternalNative`) fit inline because the `u8` lands in byte 3 of the record. See [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) and [WIRE_FORMAT.md](./WIRE_FORMAT.md) for the wire format that encodes these shapes.

## Cost Summary

The cost groupings reproduce `bytecode::nominal_op_cycles`. Hosts that need wall-clock WCET supply a custom `CostModel` calibrated to the target.

| Cost | Instructions |
|------|-------------|
| 1 | Const, PushImmediate, GetLocal, SetLocal, GetData, SetData, Dup, Not, If, Else, EndIf, Loop, EndLoop, Break, BreakIf, Stream, Reset, Yield, Trap, PopN |
| 2 | Add, Sub, Mul, Neg, CheckedAdd, CheckedSub, CheckedMul, CheckedNeg, CheckedDiv, CheckedMod, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, GetIndex, GetTupleField, GetEnumField, Len, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, FixedMul, FixedDiv, Return, GetDataIndexed, SetDataIndexed, BoundsCheck, BitAnd, BitOr, BitXor, Shl, Shr |
| 3 | Div, Mod, GetField, IsEnum, IsStruct |
| 5 | NewComposite |
| 10 | Call, CallVerifiedNative, CallExternalNative |

## WCMU contributions

WCMU costs are reported separately as stack slot growth, stack slot shrink, and heap allocation in bytes. The constant `VALUE_SLOT_SIZE_BYTES` converts slot counts to bytes; the parametric `Vm<W, A, F>` shape uses `size_of::<GenericValue<W, F>>()` directly. Computed by `wcmu_stream_iteration()` in `src/verify.rs`.

### Stack growth (peak net delta during execution)

The values reproduce `Op::stack_growth` in `src/bytecode.rs`. For multi-output opcodes the value is the net peak delta against the starting depth, not the raw push count: e.g. `CheckedAdd` pops two and pushes three, so the peak depth relative to the start is `+1`.

| Growth | Instructions |
|--------|--------------|
| 0 | Not, Neg, Add, Sub, Mul, Div, Mod, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, SetLocal, SetData, SetDataIndexed, BoundsCheck, If, BreakIf, Else, EndIf, Loop, EndLoop, Break, Stream, Reset, Yield, Return, GetField, GetIndex, GetTupleField, GetEnumField, Len, IsEnum, IsStruct, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, FixedMul, FixedDiv, Trap, PopN, BitAnd, BitOr, BitXor, Shl, Shr |
| 1 | Const, PushImmediate, GetLocal, GetData, Dup, GetDataIndexed, CheckedAdd, CheckedSub, CheckedMul, CheckedDiv, CheckedMod, Call, CallVerifiedNative, CallExternalNative, NewComposite |
| 2 | CheckedNeg |

### Stack shrink (slots popped during execution)

The values reproduce `Op::stack_shrink`. For opcodes whose net delta is non-negative (e.g. `CheckedAdd`, `CheckedNeg`) the shrink reads zero because the verifier accounts for the peak through `stack_growth` and there is no net pop.

| Shrink | Instructions |
|--------|--------------|
| 0 | Const, PushImmediate, GetLocal, GetData, Dup, Not, Neg, CheckedAdd, CheckedSub, CheckedMul, CheckedNeg, CheckedDiv, CheckedMod, BoundsCheck, Else, EndIf, Loop, EndLoop, Break, Stream, Reset, Return, Len, IsEnum, IsStruct, IntToFloat, FloatToInt, WordToByte, ByteToWord, WordToFixed, FixedToWord, FixedMul, FixedDiv, Trap |
| 1 | Add, Sub, Mul, Div, Mod, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, SetLocal, SetData, GetDataIndexed, If, BreakIf, Yield, GetField, GetIndex, GetTupleField, GetEnumField, BitAnd, BitOr, BitXor, Shl, Shr |
| 2 | SetDataIndexed |
| n | PopN(n), Call(_, n), CallVerifiedNative(_, n), CallExternalNative(_, n), NewComposite(count) |

### Heap allocation (bytes)

| Heap | Instructions |
|------|--------------|
| 0 | All instructions not listed below |
| `byte_size` from operand | NewComposite, flat form. The exact flat allocation size is baked into the operand at compile time, so the worst-case-memory-usage contribution is the precise byte count rather than a `count * VALUE_SLOT_SIZE_BYTES` estimate. The boxed form reports zero flat bytes; its body is the heap `Vec`, accounted separately. |
| host-attested | CallVerifiedNative through host registration; CallExternalNative through per-iteration invocation-count bound |
