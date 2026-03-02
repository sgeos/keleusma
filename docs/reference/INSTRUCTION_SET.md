# Instruction Set

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

The Keleusma VM executes a 48-instruction bytecode. All instructions operate on a value stack. This document lists every instruction with its operands and behavior.

For details on how bytecode is generated from source, see [COMPILATION_PIPELINE.md](../architecture/COMPILATION_PIPELINE.md).

This document describes the current implementation bytecode. The long-term target ISA, designed for safety-critical certification with structural verification, is documented in [TARGET_ISA.md](./TARGET_ISA.md).

## Constants

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Const | u16 index | Push constant from pool onto stack |
| PushUnit | none | Push unit value |
| PushTrue | none | Push boolean true |
| PushFalse | none | Push boolean false |

## Local Variables

| Instruction | Operands | Description |
|-------------|----------|-------------|
| GetLocal | u16 slot | Push local variable onto stack |
| SetLocal | u16 slot | Pop stack into local variable slot |

## Arithmetic

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Add | none | Pop two values, push sum |
| Sub | none | Pop two values, push difference |
| Mul | none | Pop two values, push product |
| Div | none | Pop two values, push quotient. Error on division by zero |
| Mod | none | Pop two values, push remainder. Error on division by zero |
| Neg | none | Pop value, push negation |

## Comparisons

| Instruction | Operands | Description |
|-------------|----------|-------------|
| CmpEq | none | Pop two values, push true if equal |
| CmpNe | none | Pop two values, push true if not equal |
| CmpLt | none | Pop two values, push true if less than |
| CmpGt | none | Pop two values, push true if greater than |
| CmpLe | none | Pop two values, push true if less than or equal |
| CmpGe | none | Pop two values, push true if greater than or equal |

## Logic

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Not | none | Pop boolean, push logical negation |

## Control Flow

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Jump | u32 target | Unconditional jump to instruction index |
| JumpIfFalse | u32 target | Pop boolean, jump if false |

## Function Calls

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Call | u16 index, u8 argc | Call function chunk with arguments |
| CallNative | u16 index, u8 argc | Call native function with arguments |

## Return and Yield

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Return | none | Return from current function |
| Yield | none | Suspend coroutine, return yielded value to host |

## Stack

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Pop | none | Discard top of stack |
| Dup | none | Duplicate top of stack |

## Type Construction

| Instruction | Operands | Description |
|-------------|----------|-------------|
| NewStruct | u16 template | Pop field values, create struct from template |
| NewEnum | u16 type, u16 variant, u8 fields | Pop field values, create enum variant |
| NewArray | u16 length | Pop N values, create array |
| NewTuple | u8 length | Pop N values, create tuple |
| WrapSome | none | Pop value, wrap in Option::Some |
| PushNone | none | Push Option::None |

## Field Access

| Instruction | Operands | Description |
|-------------|----------|-------------|
| GetField | u16 name index | Pop struct, push named field value |
| GetIndex | none | Pop index and array, push element |
| GetTupleField | u8 index | Pop tuple, push element at index |
| GetEnumField | u8 index | Pop enum variant, push field at index |

## Type Testing

| Instruction | Operands | Description |
|-------------|----------|-------------|
| TestEnum | u16 type, u16 variant, u32 jump | Test if value matches enum variant, jump if not |
| TestStruct | u16 name, u32 jump | Test if value matches struct type, jump if not |

## Casting

| Instruction | Operands | Description |
|-------------|----------|-------------|
| IntToFloat | none | Pop i64, push as f64 |
| FloatToInt | none | Pop f64, push as i64. Truncates toward zero |

## Error

| Instruction | Operands | Description |
|-------------|----------|-------------|
| Trap | u16 message index | Halt execution with error message from constant pool |
