# Type System

> **Navigation**: [Design](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma uses a static nominal type system with Rust syntax. There is no implicit coercion between types. Type inference is limited to local `let` bindings where the type can be determined from the right-hand side expression. All function signatures, struct fields, and enum variants require explicit type annotations.

## Primitive Types

| Type | Description | Size (bytes) | Alignment (bytes) |
|------|-------------|---|---|
| `i64` | 64-bit signed integer | 8 | 8 |
| `f64` | 64-bit floating-point number | 8 | 8 |
| `bool` | Boolean value | 1 | 1 |
| `()` | Unit type | 0 | 1 |

All numeric operations use `i64` or `f64`. When host structs contain smaller integer types such as `i32` or `u16`, those values are widened to `i64` at the boundary between the host and the script.

Sizes and alignments above assume the modern 64-bit target. Future work extends the type system with `word`, `byte`, `bit`, and `address` primitives whose sizes and alignments are target-defined. See R33 and B10 for the modern-target assumption and the portability future work.

## String Types

Keleusma distinguishes two string types with distinct lifetimes and allowed flow paths.

### Static strings

Static strings reside in the read-only data section of the loaded code image. Source-level string literals compile to static strings. The runtime representation is an index or pointer into the constant pool. Static strings are immutable and have a fixed-size handle, namely the index.

| Property | Value |
|---|---|
| Lifetime | Bound to the code image. Replaced at hot update with the rest of rodata. |
| Allowed flow paths | Anywhere admissible. Function arguments, return values, dialogue type B, native function arguments and returns, local bindings. |
| Data segment | Permitted at the bytecode level. Surface grammar does not expose static strings as `data` field types. The host may write static-string handles into data slots via `set_data` and is responsible for validity across hot updates. |
| Mutability | Immutable. |
| Cost | Fixed-size handle, no allocation at use site. |

### Dynamic strings

Dynamic strings reside in the arena heap region. They are produced by native function calls that allocate from the arena. Dynamic strings are immutable from the script's perspective, namely the script cannot mutate the string contents in place.

| Property | Value |
|---|---|
| Lifetime | Bound to the arena. Cleared at RESET. |
| Allowed flow paths | Stack, local bindings, native function parameters by borrow, native function returns. |
| Cross-yield prohibition | A dynamic string cannot appear in the dialogue type B. The yield expression cannot be a value whose static type contains a dynamic string. |
| Data segment | Forbidden. The fixed-size discipline excludes variable-length types from the data segment. |
| Mutability | Immutable from the script. The arena owns the storage and reclaims it at RESET. |
| Cost | Variable-length allocation in the arena. Counted against `heap_wcmu`. |

The cross-yield prohibition is the load-bearing safety property of the dynamic string design. A dynamic string is an arena pointer. Allowing one across the yield boundary would either require the host to consume it before the next RESET or accept dangling references after the arena is cleared. Prohibiting it structurally is simpler and preserves the safe-swapping guarantee.

### String surface features

The surface language supports string literals only. There is no concatenation operator, no formatting syntax, no slicing or indexing built into the grammar. All variable-cost string operations are host-supplied native functions. This freeze is intentional. Keleusma is not a value-add for string processing. Anything fancier than literal handling and native function delegation is deferred per B5.

## Composite Types

### Structs

Structs are named product types with named fields. All fields must be provided at construction time. Field access uses dot notation.

```
struct Point {
    x: f64,
    y: f64,
}

let p = Point { x: 1.0, y: 2.0 };
let dx = p.x;
```

### Enums

Enums are named sum types with variants. Each variant may carry data fields or may be a unit variant with no associated data.

```
enum Shape {
    Circle { radius: f64 },
    Rectangle { width: f64, height: f64 },
    Empty,
}
```

### Tuples

Tuples are anonymous product types. Field access uses numeric index notation.

```
let pair = (10, 20);
let first = pair.0;
let second = pair.1;
```

### Fixed-Size Arrays

Fixed-size arrays are homogeneous sequences with a known length. The syntax is `[T; N]` where `T` is the element type and `N` is the length. Element access uses index notation with an `i64` index.

```
let values: [f64; 4] = [1.0, 2.0, 3.0, 4.0];
let first = values[0];
```

### Option

Option represents nullable values. It uses two variants: `Option::Some(value)` for present values and `Option::None` for absent values.

```
let found: Option<i64> = Option::Some(42);
let missing: Option<i64> = Option::None;
```

## Opaque Types

Opaque types are Rust types registered by the host that scripts can receive from and pass to native functions but cannot destructure, inspect, or construct. The compiler recognizes opaque type names from the native function registry and permits them in type positions without requiring a struct or enum definition.

Opaque types are useful for passing handles, references to host resources, or complex Rust types through scripts without exposing their internal structure to the scripting layer.

## Type Coercion

Keleusma does not perform implicit type coercion. To convert between numeric types, use the `as` keyword.

- `i64` to `f64`: Widens the integer to a floating-point value.
- `f64` to `i64`: Truncates toward zero, discarding the fractional part.

```
let x: i64 = 42;
let y: f64 = x as f64;

let a: f64 = 3.9;
let b: i64 = a as i64;  // b is 3
```

No other type conversions are available through the `as` keyword. Conversions between non-numeric types require explicit function calls.

## Runtime Value Representation

All values in the virtual machine are represented as variants of the `Value` enum.

| Variant | Contents | Description |
|---------|----------|-------------|
| `Value::Unit` | None | The unit value `()` |
| `Value::Bool(bool)` | A boolean | True or false |
| `Value::Int(i64)` | A 64-bit integer | Signed integer value |
| `Value::Float(f64)` | A 64-bit float | Floating-point value |
| `Value::StaticStr(String)` | A UTF-8 static string | Static string referenced from the code image |
| `Value::DynStr(String)` | A UTF-8 dynamic string | Arena-allocated string produced at runtime |
| `Value::Tuple(Vec<Value>)` | A vector of values | Anonymous product type |
| `Value::Array(Vec<Value>)` | A vector of values | Homogeneous fixed-size array |
| `Value::Struct { type_name, fields }` | Name and field map | Named product type instance |
| `Value::Enum { type_name, variant, fields }` | Name, variant, and field map | Named sum type instance |
| `Value::None` | None | Represents `Option::None` |

The `Struct` variant stores the type name as a string and the fields as an ordered collection of name-value pairs. The `Enum` variant additionally stores the variant name. This representation allows pattern matching and field access at runtime without requiring type metadata beyond what is embedded in the value itself.

## Data Segment Field Types

Fields declared in a `data` block are subject to a stricter constraint than ordinary value types. Each field must have a statically known fixed size. This constraint follows directly from the `.data` section analogy described in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md). The host context struct must have a fixed layout and size to be installable as the preinitialized region for a code image.

The following type expressions are admissible as data segment field types.

| Type form | Admissible | Rationale |
|---|---|---|
| `i64`, `f64`, `bool` | Yes | Fixed-size primitives. |
| `()` | Yes | Zero-size unit. |
| Fixed-arity tuple of admissible types | Yes | Compositional. |
| Fixed-length array `[T; N]` of admissible elements | Yes | Size is element size times length. |
| `Option<T>` where `T` is admissible | Yes | Tag plus payload, fixed size. |
| Nominal struct of admissible fields | Yes | Compositional. |
| Nominal enum where all variants have admissible payloads | Yes | Discriminator plus the maximum-size payload. |
| `StaticStr` | Conditional | Permitted at the bytecode level. Surface grammar does not currently expose static strings as data field types. The host may store static-string handles in data slots through `set_data` and bears responsibility for validity across hot updates. |
| `DynStr` | No | Variable-length and arena-bound. Lifetime conflicts with cross-RESET persistence. |
| Variable-length array | No | Variable length. |
| Opaque types | Conditional | Admissible only if the host declares a fixed size for the type. Subject to future specification. |

The constraint is enforced at the data block declaration boundary. Programs that declare data fields with non-admissible types are rejected at compile time with a clear diagnostic referencing the offending field.

The ordinary value types described above remain available without restriction in function parameters, return types, local bindings, and constant pool entries. The fixed-size constraint applies specifically to data segment field declarations.

## Cross-References

- [GRAMMAR.md](./GRAMMAR.md) Section 3 provides the formal syntax for type expressions.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the four memory regions and the `.data` analogy.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) specifies the data segment ownership and lifetime.
