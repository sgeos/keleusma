# Instruction Set

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

The Keleusma VM executes a stack-based bytecode using block-structured control flow. All instructions operate on a value stack. This document lists every instruction with its operands, behavior, and WCET cost.

Each instruction carries a relative integer cost via `Op::cost()` for worst-case execution time analysis. Costs are unitless relative weights, not cycle counts. Higher values indicate more expensive operations.

For details on how bytecode is generated from source, see [COMPILATION_PIPELINE.md](../architecture/COMPILATION_PIPELINE.md). For the structural ISA specification including block hierarchy and verification rules, see [TARGET_ISA.md](./TARGET_ISA.md).

## Constants

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Const | u16 index | 1 | Push constant from pool onto stack |
| PushUnit | none | 1 | Push unit value |
| PushTrue | none | 1 | Push boolean true |
| PushFalse | none | 1 | Push boolean false |

## Local Variables

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| GetLocal | u16 slot | 1 | Push local variable onto stack |
| SetLocal | u16 slot | 1 | Pop stack into local variable slot |

## Arithmetic

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Add | none | 2 | Pop two values, push sum |
| Sub | none | 2 | Pop two values, push difference |
| Mul | none | 2 | Pop two values, push product |
| Div | none | 3 | Pop two values, push quotient. Error on division by zero |
| Mod | none | 3 | Pop two values, push remainder. Error on division by zero |
| Neg | none | 2 | Pop value, push negation |

## Comparisons

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| CmpEq | none | 2 | Pop two values, push true if equal |
| CmpNe | none | 2 | Pop two values, push true if not equal |
| CmpLt | none | 2 | Pop two values, push true if less than |
| CmpGt | none | 2 | Pop two values, push true if greater than |
| CmpLe | none | 2 | Pop two values, push true if less than or equal |
| CmpGe | none | 2 | Pop two values, push true if greater than or equal |

## Logic

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Not | none | 1 | Pop boolean, push logical negation |

## Control Flow

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| If | u32 offset | 1 | Pop boolean. If false, skip forward by offset to matching Else or EndIf |
| Else | u32 offset | 1 | Unconditional skip forward by offset to matching EndIf |
| EndIf | none | 1 | End of if or if-else block. No operation |
| Loop | u32 offset | 1 | Start of loop block. Offset is distance to matching EndLoop |
| EndLoop | u32 offset | 1 | Unconditional jump backward by offset to matching Loop |
| Break | u32 depth | 1 | Exit enclosing loop at nesting depth |
| BreakIf | u32 depth | 1 | Pop boolean. If true, exit enclosing loop at nesting depth |

## Function Calls

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Call | u16 index, u8 argc | 10 | Call function chunk with arguments |
| CallNative | u16 index, u8 argc | 10 | Call native function with arguments |

## Return, Yield, and Streaming

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Return | none | 2 | Return from current function |
| Yield | none | 1 | Suspend coroutine, exchange output B for input A with host |
| Stream | none | 1 | Entry of the streaming region. Only Reset may target it |
| Reset | none | 1 | Clear arena, activate hot swap if scheduled, jump to Stream |

## Stack

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Pop | none | 1 | Discard top of stack |
| Dup | none | 1 | Duplicate top of stack |

## Type Construction

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| NewStruct | u16 template | 5 | Pop field values, create struct from template |
| NewEnum | u16 type, u16 variant, u8 fields | 5 | Pop field values, create enum variant |
| NewArray | u16 length | 5 | Pop N values, create array |
| NewTuple | u8 length | 5 | Pop N values, create tuple |
| WrapSome | none | 1 | Pop value, wrap in Option::Some |
| PushNone | none | 1 | Push Option::None |

## Field Access

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| GetField | u16 name index | 3 | Pop struct, push named field value |
| GetIndex | none | 2 | Pop index and array, push element |
| GetTupleField | u8 index | 2 | Pop tuple, push element at index |
| GetEnumField | u8 index | 2 | Pop enum variant, push field at index |
| Len | none | 2 | Pop composite value (Array, String, Tuple), push length as Int |

## Type Testing

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| IsEnum | u16 type, u16 variant | 3 | Pop value, push true if it matches the enum type and variant |
| IsStruct | u16 name | 3 | Pop value, push true if it matches the struct type |

## Casting

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| IntToFloat | none | 2 | Pop i64, push as f64 |
| FloatToInt | none | 2 | Pop f64, push as i64. Truncates toward zero |

## Error

| Instruction | Operands | Cost | Description |
|-------------|----------|------|-------------|
| Trap | u16 message index | 1 | Halt execution with error message from constant pool |

## Cost Summary

Costs are relative weights used by `wcet_stream_iteration()` for worst-case execution time analysis. They are preliminary and subject to refinement.

| Cost | Instructions |
|------|-------------|
| 1 | Const, PushUnit, PushTrue, PushFalse, GetLocal, SetLocal, Pop, Dup, PushNone, WrapSome, Not, If, Else, EndIf, Loop, EndLoop, Break, BreakIf, Stream, Reset, Yield, Trap |
| 2 | Add, Sub, Mul, Neg, CmpEq, CmpNe, CmpLt, CmpGt, CmpLe, CmpGe, GetIndex, GetTupleField, GetEnumField, Len, IntToFloat, FloatToInt, Return |
| 3 | Div, Mod, GetField, IsEnum, IsStruct |
| 5 | NewStruct, NewEnum, NewArray, NewTuple |
| 10 | Call, CallNative |
