# Type System

> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma uses a static nominal type system with Rust syntax. There is no implicit coercion between types. Type inference is limited to local `let` bindings where the type can be determined from the right-hand side expression. All function signatures, struct fields, and enum variants require explicit type annotations.

## Primitive Types

| Type | Description | Size on default `Vm<i64, u64, f64>` (bytes) | Alignment (bytes) |
|------|-------------|---|---|
| `Word` | Signed integer of the runtime's word width | 8 | 8 |
| `Float` | Floating-point number of the runtime's float width | 8 | 8 |
| `bool` | Boolean value | 1 | 1 |
| `()` | Unit type | 0 | 1 |

All numeric operations use `Word` or `Float`. When host structs contain integer types other than the runtime's word, those values are widened or truncated through `Word::to_i64` and `Word::from_i64_wrap` at the boundary between the host and the script.

The `Word` and `Float` surface types refer to the runtime's chosen word and float widths. The bundled default runtime is `Vm<i64, u64, f64>`, which makes `Word` a 64-bit signed integer and `Float` a 64-bit IEEE-754 floating-point number; the sizes and alignments above reflect that default. Hosts that instantiate the parametric `GenericVm<W, A, F>` shape with narrower trait parameters change the underlying widths accordingly. The bytecode header's `word_bits_log2`, `addr_bits_log2`, and `float_bits_log2` fields record the declared widths so a runtime can reject mismatched bytecode at load time. See B16 in [BACKLOG.md](../decisions/BACKLOG.md) for the parametric-Vm cascade and `docs/guide/COOKBOOK.md` for the narrow-runtime type-alias recipe.

## Multi-Word Fixed-Point Types

`Multiword<N, F>` is a fixed-width multi-word fixed-point type. It is `N` machine words wide, little-endian two's complement, with `F` fractional bits. The word count `N` sets the total width at `N` times the runtime word width, and `F` places the implied binary point `F` bits above the least significant bit. The surface form `Multiword<N>` abbreviates `Multiword<N, 0>`, the integer case with no fractional component. The type therefore spans both wide integers and fixed-point fractions under one representation.

| Form | Meaning |
|------|---------|
| `Multiword<N>` | `N`-word signed integer, equivalent to `Multiword<N, 0>`. |
| `Multiword<N, F>` | `N`-word signed fixed-point value with `F` fractional bits. |

The word count `N` is in the range `[1, 65535]` and the fraction-bit count `F` is in the range `[0, 65535]`. The runtime representation is a flat array of `N` signed words, digit zero least significant, so a value constructed from a tuple indexes to its underlying words with `m[i]`.

`Multiword<N, F>` is nominal in both parameters. Two multi-word types unify only when their word counts and their fraction-bit counts are equal. A `Multiword<2>` and a `Multiword<2, 16>` are distinct types and do not combine in an arithmetic operation without an explicit cast, because they carry different scales. This prevents a silent scale mismatch, which in fixed-point arithmetic is a correctness fault rather than a rounding difference.

Construction is an explicit cast from a tuple of `N` words, or the equivalent turbofish constructor. The constructor form also expresses the single-word case, which a one-element tuple cannot.

```rust
let a = (42, 7, 0, 0) as Multiword<4>;   // integer, four words
let q = (0, 1) as Multiword<2, 32>;      // fixed-point, thirty-two fractional bits
let b = Multiword::<4>(42, 7, 0, 0);     // equivalent to the tuple cast
let c = Multiword::<1>(77);              // single word, no tuple form
```

Indexing a `Multiword<N, F>` with `m[i]` yields the `i`-th underlying word as a `Word`, digit zero least significant. Indexing is independent of `F`; it reads the stored words regardless of the implied binary point.

The supported operators are addition, subtraction, the six comparisons `==`, `!=`, `<`, `>`, `<=`, `>=`, and multiplication at every scale. Addition and subtraction are the two's-complement multi-word carry and borrow cascades; they are scale-independent, so two same-scale operands add or subtract as their underlying words. Comparisons yield `bool`; the ordering is decided by the most significant differing word, the top word read signed and the lower words read unsigned, which is the correct signed two's-complement multi-word order. Integer multiplication where `F` is zero is the low-`N`-word two's-complement product, computed as an unsigned schoolbook product with a signed-to-unsigned high-word correction on each digit product. Fixed-point multiplication where `F` is greater than zero forms the full double-width signed product and shifts it right by `F`, taking the low `N` words, which preserves the scale; a fraction-bit count greater than `N` times the word width is rejected at compile time. The shift is arithmetic, so a negative product rounds toward negative infinity rather than toward zero, and a product that does not fit in `N` words wraps, matching the wrapping default of the other multi-word operations. Division and the modulo operation are signed with truncation toward zero, as for the `Word` integer, the quotient taking the sign of the operand exclusive-or and the remainder the sign of the dividend; a zero divisor traps as a division by zero. The fixed-point division where `F` is greater than zero pre-shifts the dividend left by `F`, since the raw quotient representing the ratio of two same-scale values is the shifted dividend divided by the divisor, while the fixed-point modulo needs no shift because a same-scale remainder keeps the scale. The four shift operators `lsl`, `asl`, `lsr`, and `asr` apply, with a constant or runtime-variable amount, as described under Shift Operators below; on a multi-word value the arithmetic left shift `asl` wraps like the logical `lsl`, because the multi-word type has no overflow-capture construct. The bitwise operators `band`, `bor`, `bxor`, and the prefix `bnot` also apply, combining the two operands limb by limb with no cross-limb interaction, as described under Bitwise Operators below.

The fixed-point rounding of `Multiword<N, F>` matches the scalar `Fixed<N>` type deliberately, so the two behave alike where their ranges overlap. The multiplication rounds toward negative infinity, because both shift the signed product right by `F` arithmetically. The division truncates toward zero, because both divide the shifted dividend by the divisor with the truncating integer division. This multiply-floors, divide-truncates asymmetry is therefore a property of the `Fixed` family that `Multiword` inherits, not a divergence. A result that does not fit in `N` words wraps, matching the wrapping default of the other multi-word operations and the wrapped low slot of the scalar `Fixed` checked arithmetic. See B19 in [BACKLOG.md](../decisions/BACKLOG.md) for the operator roadmap and the carry-semantics rationale.

Every multi-word operation lowers to an unrolled cascade over the existing single-word opcodes, so the type adds no instructions to the instruction set.

## Shift Operators

The shift operators `lsl`, `asl`, `lsr`, and `asr` apply to a `Word`, a `Byte`, or a `Multiword<N, F>` value. The shift amount is a `Word`. It may be a compile-time-constant literal or a runtime-variable value. A constant literal must be within the value's total bit width, and an out-of-range literal is rejected at compile time. A runtime amount is always total and never traps, though the scalar and multi-word cases treat an out-of-range count differently. In the scalar `Word` case the count is reduced modulo the word width. A count equal to a multiple of the word width is therefore the identity, a count beyond the width wraps back into range, and a negative count wraps to a large in-range count, matching the target's native modular shift. In the multi-word case the count is decomposed into a whole-word offset, which is the count divided by the word width, and a bit offset, which is the count modulo the word width, evaluated across the total value width. A multi-word count at or beyond the total width, or a negative multi-word count, shifts every bit out to the fill value, which is zero for a logical shift and the sign word for an arithmetic right shift. This difference in behavior at and beyond the value width is deliberate, and both the scalar and multi-word cases remain total. A `Byte` value shifts at the byte width through a promote-to-`Word`, shift, and truncate-to-`Byte` lowering; because a `Byte` is unsigned, its arithmetic and logical right shifts coincide.

The four operators are keywords named after the assembly mnemonics, so the arithmetic-versus-logical choice is stated by the operator rather than inferred from operand signedness. This suits the high-assurance language cluster the language targets, which names shift operations explicitly, and it lets the arithmetic left shift carry overflow semantics the logical left shift does not. The overflow-capturing form of `asl` inside the checked-arithmetic construct still requires a compile-time-constant amount, since it lowers to a multiply by the constant `2^k`.

A variable amount preserves the language's definitive worst-case bounds: the scalar case is a single opcode, and the multi-word case is unrolled over the compile-time word count `N` with runtime index arithmetic and branch-free bounds guards, so there is no runtime loop and the worst-case execution time and memory usage stay statically provable.

| Operator | Direction | Fill | Meaning |
|----------|-----------|------|---------|
| `lsl` | left | zero at the bottom | logical left, wrapping and total |
| `asl` | left | zero at the bottom | arithmetic left, the value `x * 2^k`, admits overflow capture |
| `lsr` | right | zero at the top | logical right, unsigned divide, total |
| `asr` | right | sign at the top | arithmetic right, floor divide, total |

A left shift vacates the low bits and fills them with zero regardless of arithmetic or logical interpretation, so `lsl` and `asl` produce the identical value. Their only difference is that `asl` is the value `x * 2^k` and therefore admits the `overflow` and `underflow` arms of the checked-arithmetic construct, while `lsl` is a plain wrapping bit operation with no overflow concept. The two right shifts differ only in the fill and coincide for a non-negative value. The right shifts and the logical left shift never overflow. Shifts bind below the bitwise operators and above the additive operators.

## Bitwise Operators

The bitwise operators `band`, `bor`, and `bxor` are the binary and, or, and exclusive-or, and the prefix `bnot` is the ones-complement. They apply to a `Word`, a `Byte`, or a `Multiword<N, F>` value and yield the same type. A `Byte` is combined at the byte width through the same promote-to-`Word`, operate, truncate-to-`Byte` lowering as the byte shifts, so `bnot` of a `Byte` is the eight-bit complement (`bnot 0Byte` is `255Byte`, not the word-width `-1`). On a `Multiword<N, F>` the operation is applied to each limb independently, with no carry, borrow, or other cross-limb interaction, so the result is scale-independent.

The operators are keyword mnemonics after the Erlang convention, so the operation is chosen by name and never inferred from the operand type. This is the deliberate counterpart to the boolean operators below. Keeping the two families lexically distinct means a program that wants the word-level bit operation and a program that wants the truth-value operation are never disambiguated by whether an operand happens to be a `Word` or a `bool`. Among the binary bitwise operators, `band` binds tightest, then `bxor`, then `bor`; the whole group binds below the comparisons and above the shifts.

## Boolean Operators

The boolean operators take `bool` operands and yield `bool`. The eager operators `and`, `or`, and `xor`, together with the prefix `not`, always evaluate both operands. The short-circuit operators `andalso` and `orelse` evaluate the left operand first and evaluate the right operand only when the left does not already determine the result. There is no short-circuit `xor` or `not`, because neither operation can be decided from one operand.

In a pure total context the eager and short-circuit forms compute the same value, so the distinction is observable only through a native side effect on the skipped operand and through worst-case-execution-time analysis, where the eager forms are branch-free and the short-circuit forms introduce a data-dependent branch. The eager default therefore aligns with the language's definitive-bound goal, while the named short-circuit forms remain available where skipping the right operand is intended. Precedence among the boolean operators binds loosest to tightest as `orelse`, `andalso`, `or`, `xor`, `and`, then comparison; `not` is a prefix operator that binds with the other unary operators.

## Text Types

The Keleusma surface type for textual data is named `Text` to avoid confusion with Rust's `String`. The runtime distinguishes two string variants behind the `Text` surface type with distinct lifetimes and allowed flow paths.

### Static strings

Static strings reside in the read-only data section of the loaded code image. Source-level string literals compile to static strings. The runtime representation is an index or pointer into the constant pool. Static strings are immutable and have a fixed-size handle, namely the index.

| Property | Value |
|---|---|
| Lifetime | Bound to the code image. Replaced at hot update with the rest of rodata. |
| Allowed flow paths | Anywhere admissible. Function arguments, return values, dialogue type B, native function arguments and returns, local bindings. |
| Data segment | Surface grammar does not expose static strings as `data` field types, and a shared composite carrying a `Text` (arena-pointer) field is rejected at compile time, because shared data is a flat host-owned byte buffer that cannot hold an arena pointer the host would dangle after RESET (B28 item 2). The host marshals strings separately rather than through shared slots. |
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

### Strings inside composites (B28 P3)

A `Text` field of a flat composite (struct or enum) is stored in the composite body as a **two-word handle**, the arena data pointer and the byte length. The handle is the `Text` value's compact in-body form. The epoch is not stored in the field. It is supplied by the arena when the field is read out, reconstituting the de-facto three-part arena handle (data pointer, length, epoch) that the runtime already uses for a bare dynamic string. The epoch used is the **originating composite's** epoch, so a read after a RESET resolves to a clean stale outcome rather than a dangling dereference, exactly as for a bare dynamic string. A flat composite's `Text` field is therefore an arena-resident dynamic string.

Because of that, **a value that transitively contains a flat `Text` field cannot cross the yield boundary**. Static text, and any container of only static text, is safe to yield. Dynamic text, and any container of it, is not, because the iteration `RESET` reclaims the arena. A flat `Text` field is always dynamic, so the compiler rejects yielding any struct or enum that transitively contains one. The compiler enforces this from the layout, descending through field and variant-payload types (and through boxed tuples, arrays, and `Option`s that may hold a flat-text struct or enum below), so the guarantee holds for a nested string, not only for a string named directly. A direct `Text` element of a tuple, array, or `Option` is boxed rather than flat and keeps its `StaticStr`/`KStr` distinction; together with a bare `Text`, those are governed by the runtime cross-yield check, which admits static text and rejects dynamic text.

Allowing a struct or enum that carries genuinely *static* text to yield would require a flat `Text` field that references rodata (a constant-pool index) for static strings instead of copying them into the arena. That representation does not yet exist, so a struct or enum with a `Text` field cannot be yielded even when the text is a literal.

### Text surface features

The surface language supports string literals only. There is no concatenation operator, no formatting syntax, no slicing or indexing built into the grammar. All variable-cost string operations are host-supplied native functions. This freeze is intentional. Keleusma is not a value-add for string processing. Anything fancier than literal handling and native function delegation is deferred per B5.

## Composite Types

### Structs

Structs are named product types with named fields. All fields must be provided at construction time. Field access uses dot notation.

```
struct Point {
    x: Float,
    y: Float,
}

let p = Point { x: 1.0, y: 2.0 };
let dx = p.x;
```

### Enums

Enums are named sum types with variants. Each variant may carry data fields or may be a unit variant with no associated data.

```
enum Shape {
    Circle { radius: Float },
    Rectangle { width: Float, height: Float },
    Empty,
}
```

#### Variant discriminants

Each variant carries a numeric discriminant. The discriminant defaults to zero for the first variant and increments by one for each subsequent variant unless an explicit `= N` clause appears after the variant. Explicit clauses set the value directly and reset the auto-increment counter; subsequent implicit variants resume from one past the most recent explicit value.

```
enum StatusErrorCode {
    OutOfRange = 1,
    NotConfigured = 2,
    Busy = 3,
}
```

Variants identified by name are the script-side mechanism for pattern matching; discriminants are the host-side mechanism for stable numeric mapping. The runtime currently identifies variants by name on the wire, so two scripts that agree on variant names but disagree on discriminant values still interoperate. Discriminants matter when scripts cast variants to a numeric type, when host code constructs variants by numeric index, or when an external system (logging, telemetry, audit) wants stable numeric error codes.

Restrictions: discriminants must be integer literals, optionally preceded by a unary minus for negative values. Expressions, named constants, and casts are not admissible in the discriminant clause itself. Duplicate discriminant values within a single enum are rejected at parse time.

#### Casting an enum value to `Word`

An enum-typed value can be cast to `Word` to extract its variant's discriminant.

```
enum Status { Ok = 0, Busy = 3, Timeout = 4 }

fn main() -> Word {
    let s = Status::Busy();
    s as Word  // evaluates to 3
}
```

The cast compiles to a chain of variant tests; on a match it pushes the variant's discriminant as a `Word`. Implicit and explicit discriminants both round-trip correctly. Casts on enum values whose variant is not declared in the source (constructed by host code outside the declaration) trap at runtime; this is an enforced invariant of the type system rather than a fall-through return.

### Tuples

Tuples are anonymous product types. Field access uses numeric index notation.

```
let pair = (10, 20);
let first = pair.0;
let second = pair.1;
```

### Fixed-Size Arrays

Fixed-size arrays are homogeneous sequences with a known length. The syntax is `[T; N]` where `T` is the element type and `N` is the length. The length may be an integer literal, a const parameter of an enclosing definition, or a total const expression over such parameters, as described in the Const Generics section. The length must be positive. A length that resolves to zero or a negative value is rejected, whether it is written as a literal or produced by a const expression after monomorphization, since a zero-length array occupies no storage and can never be indexed without trapping. Element access uses index notation with a `Word` index.

```
let values: [Float; 4] = [1.0, 2.0, 3.0, 4.0];
let first = values[0];
```

### Option

Option represents nullable values. It uses two variants: `Option::Some(value)` for present values and `Option::None` for absent values.

```
let found: Option<Word> = Option::Some(42);
let missing: Option<Word> = Option::None;
```

## Const Generics

A definition may be parameterized by a compile-time constant in addition to its type parameters. A const parameter is a lowercase name introduced by the `const` keyword, whose type is `Word`, the only admissible const-parameter type. Const parameters are declared on functions, structs, and enums, mixed freely with type parameters.

```
fn zeros<const n: Word>() -> [Word; n] { ... }
struct Buf<const n: Word> { cap: Word, items: [Word; n] }
enum Tagged<const n: Word> { Full([Word; n]), Tag(Word) }
```

A const parameter serves in two positions. In a type position it fixes an array length or a `Multiword` parameter, as in `[Word; n]` or `Multiword<n>`. In a value position inside a function body it is an ordinary `Word` value, so `for i in 0..n` and `let cap = n` are admissible. A local binding that reuses a const parameter's name shadows it in the ordinary lexical manner.

Const arguments are always explicit because they cannot be inferred from value arguments. A call writes a turbofish, `zeros::<4>()`. A struct or enum construction writes a turbofish before the body or variant, `Buf::<8> { ... }` and `Tagged::<3>::Tag(...)`. A type reference writes the const in the argument list, `Buf<8>`. A const argument may be a total arithmetic expression over `+`, `-`, and `*`, for example `Buf<n + 1>` and `Multiword<2 * n>`; division and modulo are excluded so evaluation is total.

Two array or `Multiword` types unify when their element or scale parameters agree and their dimensions are equal. A dimension is compared as a folded literal when it is fully constant, and otherwise by a canonical rendering of its symbolic form that folds constant subexpressions and orders the operands of the commutative operators, so `n + 1` and `1 + n` are equal. A symbolic dimension arises only inside a generic body and is resolved by monomorphization.

Monomorphization substitutes every const parameter to a concrete integer literal, minting one specialization per distinct const value and keying each on the concrete value. After substitution every array length, `Multiword` parameter, and loop bound is a literal, so the worst-case-execution-time and worst-case-memory-usage analyses see no symbolic constant and the static bounds are preserved. This erasure is the reason const generics do not weaken the resource guarantees. The mandatory re-typecheck after monomorphization is the soundness gate. A dimension that is symbolically compatible in a generic body but concretely mismatched at an instantiation is rejected there rather than reaching code generation.

## Opaque Types

Opaque types are Rust types registered by the host that scripts can receive from and pass to native functions but cannot destructure, inspect, or construct. The compiler recognizes opaque type names from the native function registry and permits them in type positions without requiring a struct or enum definition.

Opaque types are useful for passing handles, references to host resources, or complex Rust types through scripts without exposing their internal structure to the scripting layer.

### Runtime representation

The runtime value carrying an opaque is `Value::Opaque(Arc<dyn HostOpaque>)`. The host implements the `keleusma::HostOpaque` marker trait for any Rust type it wishes to expose. The trait surface is small: a `type_name` method that returns the script-side name and a sealed-supertrait `TypeId` lookup that the runtime uses for the safe downcast.

| Property | Value |
|---|---|
| Lifetime | Host-managed through `Arc`. Independent of the arena. Persists across resets and hot code swaps. |
| Allowed flow paths | Anywhere admissible. Function arguments, return values, dialogue type B (yields are permitted), local bindings, native function arguments and returns. |
| Data segment | Forbidden. The fixed-size discipline excludes variable-pointer types from the data segment. |
| Mutability | Immutable from the script. The host's Rust type can use interior mutability if needed. |
| Equality | Pointer identity through `Arc::ptr_eq`. Two opaque values are equal only if they share the same allocation. |
| WCMU contribution | Zero from the script side. The allocation is host-managed; hosts that need a bounded heap supply a per-native attestation through `Vm::set_native_bounds`. |
| Cross-yield | Permitted. Opaque values may cross the yield boundary because the storage is not arena-resident. |

### Host registration pattern

The host writes an `impl HostOpaque for MyType` block and registers native functions that produce and consume `Value::Opaque` directly. The script declares the type by name in signatures. See [EMBEDDING.md](../guide/EMBEDDING.md#opaque-host-types) for a worked example and [`examples/opaque_rust_string.rs`](../../examples/opaque_rust_string.rs) for a complete walkthrough exposing `std::string::String` to scripts.

## Type Coercion

Keleusma does not perform implicit type coercion. To convert between numeric types, use the `as` keyword.

- `Word` to `Float`: Widens the integer to a floating-point value.
- `Float` to `Word`: Truncates toward zero, discarding the fractional part.

```
let x: Word = 42;
let y: Float = x as Float;

let a: Float = 3.9;
let b: Word = a as Word;  // b is 3
```

No other type conversions are available through the `as` keyword. Conversions between non-numeric types require explicit function calls.

## Runtime Value Representation

All values in the virtual machine are represented as variants of the `Value` enum.

| Variant | Contents | Description |
|---------|----------|-------------|
| `Value::Unit` | None | The unit value `()` |
| `Value::Bool(bool)` | A boolean | True or false |
| `Value::Int(Word)` | A runtime-width signed integer | Signed integer value |
| `Value::Float(Float)` | A runtime-width floating-point number | Floating-point value |
| `Value::StaticStr(String)` | A UTF-8 static string | Static string referenced from the code image |
| `Value::KStr(KString)` | An arena-resident `KString` handle | Dynamic string allocated in the host-owned arena's top region |
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
| `Word`, `Float`, `bool` | Yes | Fixed-size primitives. |
| `()` | Yes | Zero-size unit. |
| Fixed-arity tuple of admissible types | Yes | Compositional. |
| Fixed-length array `[T; N]` of admissible elements | Yes | Size is element size times length. |
| `Option<T>` where `T` is admissible | Yes | Tag plus payload, fixed size. |
| Nominal struct of admissible fields | Yes | Compositional. |
| Nominal enum where all variants have admissible payloads | Yes | Discriminator plus the maximum-size payload. |
| `StaticStr` | Conditional | Surface grammar does not currently expose static strings as data field types. A `Text` field in shared data is rejected at compile time, because the shared segment is a flat host-owned byte buffer that cannot hold an arena pointer (B28 item 2); the host marshals strings outside the shared slots. |
| `DynStr` | No | Variable-length and arena-bound. Lifetime conflicts with cross-RESET persistence. |
| Variable-length array | No | Variable length. |
| Opaque types | Conditional | Admissible only if the host declares a fixed size for the type. Subject to future specification. |

The constraint is enforced at the data block declaration boundary. Programs that declare data fields with non-admissible types are rejected at compile time with a clear diagnostic referencing the offending field.

The ordinary value types described above remain available without restriction in function parameters, return types, local bindings, and constant pool entries. The fixed-size constraint applies specifically to data segment field declarations.

## Cross-References

- [GRAMMAR.md](./GRAMMAR.md) Section 3 provides the formal syntax for type expressions.
- [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) describes the four memory regions and the `.data` analogy.
- [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) specifies the data segment ownership and lifetime.
