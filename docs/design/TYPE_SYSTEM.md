# Type System

> **Navigation**: [Design](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma uses a static nominal type system with Rust syntax. There is no implicit coercion between types. Type inference is limited to local `let` bindings where the type can be determined from the right-hand side expression. All function signatures, struct fields, and enum variants require explicit type annotations.

## Primitive Types

| Type | Description | Rust Equivalent |
|------|-------------|-----------------|
| `i64` | 64-bit signed integer | `i64` |
| `f64` | 64-bit floating-point number | `f64` |
| `bool` | Boolean value | `bool` |
| `String` | UTF-8 string | `String` |
| `()` | Unit type | `()` |

All numeric operations use `i64` or `f64`. When host structs contain smaller integer types such as `i32` or `u16`, those values are widened to `i64` at the boundary between the host and the script.

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
| `Value::Str(String)` | A UTF-8 string | Heap-allocated string |
| `Value::Tuple(Vec<Value>)` | A vector of values | Anonymous product type |
| `Value::Array(Vec<Value>)` | A vector of values | Homogeneous fixed-size array |
| `Value::Struct { type_name, fields }` | Name and field map | Named product type instance |
| `Value::Enum { type_name, variant, fields }` | Name, variant, and field map | Named sum type instance |
| `Value::None` | None | Represents `Option::None` |

The `Struct` variant stores the type name as a string and the fields as an ordered collection of name-value pairs. The `Enum` variant additionally stores the variant name. This representation allows pattern matching and field access at runtime without requiring type metadata beyond what is embedded in the value itself.

## Cross-References

- [GRAMMAR.md](./GRAMMAR.md) Section 3 provides the formal syntax for type expressions.
