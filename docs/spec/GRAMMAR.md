# Keleusma Language Grammar Specification

> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)

## 1. Overview

Keleusma is a lightweight, embeddable Total Functional Stream Processor that compiles to bytecode and runs on a stack-based virtual machine. It targets `no_std+alloc` environments.

Without any host-plugged functions, Keleusma can only define pure functions that are guaranteed to yield or exit. The language has no standard library. All domain functionality is provided by native Rust functions registered by the host application. This design produces specialized domain-specific languages for audio engine control and game scripting.

### Design Goals

1. **Rust with Elixir quality-of-life features**: Multiheaded functions, pattern matching, guard clauses, pipelines, using Rust-style curly brace blocks.
2. **Rust type system**: Nominal types using Rust syntax for primitives, structs, enums, and arrays.
3. **Bidirectional typed yield**: Scripts are coroutines that receive typed input and yield typed output.
4. **Pipeline composition**: The `|>` operator chains function calls for readable data transformation.
5. **Native function binding**: Rust functions are registered and callable from scripts with type checking at compile time.
6. **Deterministic execution**: No floating-point ambiguity, no undefined behavior, no garbage collection pauses.
7. **Guaranteed termination or productivity**: Three function categories ensure that scripts either terminate or yield, verifiable by static analysis.

### Scope Inclusions and Exclusions

The following features were originally listed as out of scope and have since shipped (V0.1 baseline, carried forward in V0.2.0).

- Hindley-Milner type inference with Robinson unification, the occurs check, and `Type::Var` for inferred positions. The transitional `Type::Unknown` sentinel was removed in V0.2.0 (B15 closed).
- Generic type parameters with trait bounds, traits, impl blocks, and compile-time monomorphization with inference reach across literals, identifiers, function-call returns, method-call returns, casts, enum variants, struct constructions, tuple and array literals, if and match arms, field access, tuple-index, and array-index.
- Hot code swap at the reset boundary of a `loop` script. Native registrations persist across the swap; the data segment is supplied fresh by the host.

The following are explicitly out of scope.

- Closures with environment capture, first-class function references, and indirect dispatch. The construct existed transitionally through V0.1 but is now rejected at the type-checker stage with a diagnostic that names the construct. Programs that require definitive Worst-Case Execution Time and Worst-Case Memory Usage bounds restrict themselves to direct calls and trait dispatch. The `Op::CallIndirect`, `Op::PushFunc`, `Op::MakeClosure`, and `Op::MakeRecursiveClosure` opcodes were retired in V0.2.0 Phase 4 alongside the `Value::Func` runtime variant.
- F-string interpolation. The surface form `f"text {expr}"` was removed in V0.2.0 Phase 3.5. Programs compose dynamic text through host-registered natives such as a `format` function that returns `Value::KStr`.

- Ownership, borrowing, or lifetime annotations at the surface language level. Rust's borrow checker is unnecessary because script values are conceptually immutable and the data segment is the sole mutable region.
- Bundled standard-library text natives. The runtime ships `register_utility_natives` registering only `println`. Dynamic-text composition is the host's responsibility through verified natives.

Structural verification at the bytecode level is implemented. See [STRUCTURAL_ISA.md](./STRUCTURAL_ISA.md) for the verification specification. The conservative-verification stance is described in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification).

## 2. Lexical Structure

### Keywords

````
fn  yield  loop  break  let  for  in  if  else  match
use  external  struct  enum  newtype  trait  impl  data  true  false  as  when
not  and  or  xor  andalso  orelse  pure  shared  private  const  ephemeral  signed  where
overflow  underflow  saturate_max  saturate_min
lsl  asl  lsr  asr
band  bor  bxor  bnot
````

All keywords are reserved and cannot be used as identifiers.

The `classify` and `declassify` identifiers are intentionally **not** reserved keywords. They are recognised as information-flow operators by the parser only in expression position when not followed by `(`; the same identifier is admissible as a user-defined function name in any context. See Section 4 "Expressions" for the operator forms and Section 7 "Information-Flow Labels" for the type-system rule.

### Identifiers

````
lower_ident  = [a-z_][a-z0-9_]*
upper_ident  = [A-Z][A-Za-z0-9]*
````

Variable names, function names, and field names use `lower_ident`. Type names, struct names, and enum names use `upper_ident`.

### Literals

````
integer_lit   = [0-9]+ [ int_suffix ] | 0x[0-9a-fA-F]+ | 0b[01]+
float_lit     = [0-9]+ '.' [0-9]+ [ real_suffix ]
int_suffix    = 'Word' | 'Byte' | 'Float' | fixed_suffix
real_suffix   = 'Float' | fixed_suffix
fixed_suffix  = 'Fixed' '<' [0-9]+ '>'
string_lit    = '"' ( [^"\\] | '\\' escape_char )* '"'
bool_lit      = 'true' | 'false'
escape_char   = 'n' | 't' | 'r' | '\\' | '"' | '0'
````

Integer literals support decimal, hexadecimal, and binary notation. Float literals require digits on both sides of the decimal point. String literals use double quotes with backslash escape sequences.

A numeric literal may carry a type suffix that sets and checks the literal's type. Integer-form literals admit `Word`, `Byte`, `Float`, and `Fixed<N>`, for example `42Word`, `42Byte`, `42Float`, and `42Fixed<16>`. Fractional literals admit only the real-valued suffixes `Float` and `Fixed<N>`, for example `3.14Float` and `3.14Fixed<16>`; an integer type suffix on a fractional literal is rejected. A `Byte` suffix is range-checked to `0..=255` at lex time, and a `Fixed<N>` suffix requires the fraction-bit count `N` in the range `[0, 62]`. The `Fixed<N>` suffix mirrors the `Fixed<N>` type syntax, so `42Fixed<16>` encodes the Q-format value `42 << 16` and `3.14Fixed<16>` encodes `round(3.14 * 2^16)`. The earlier `i64` and `f64` suffixes are removed; a bare `i64` or `f64` immediately following a numeral now lexes as a separate identifier.

### Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Shift | `lsl`, `asl`, `lsr`, `asr` |
| Bitwise | `band`, `bor`, `bxor`, `bnot` |
| Comparison | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| Logical (eager) | `and`, `or`, `xor`, `not` |
| Logical (short-circuit) | `andalso`, `orelse` |
| Pipeline | `\|>` |
| Assignment | `=` |
| Field access | `.` |
| Path separator | `::` |
| Range | `..` |
| Statement terminator | `;` |
| Return type | `->` |
| Match arm | `=>` |
| Information-flow label | `@` |
| Arm alternation | `\|` |

### Comments

````
// This is a line comment. Everything after // is ignored.

/* This is a block comment.
   It can span multiple lines. */
````

Both line comments (`//`) and block comments (`/* */`) are supported. Block comments do not nest.

### Block Delimiters

Keleusma uses curly braces for block delimitation. This is consistent with the Rust host language.

````
fn greet(name: Text) -> Text {
  "Hello, " + name
}
````

### Whitespace and Semicolons

Whitespace (spaces, tabs, newlines) is not significant except as a token separator. Semicolons terminate statements, as in Rust. Multiple statements may appear on one line. The last expression in a block is the return value and does not require a trailing semicolon.

## 3. Type System

### Primitive Types

| Type | Description | Rust Equivalent on default `Vm<i64, u64, f64>` |
|------|-------------|-----------------|
| `Word` | Signed integer of the runtime's word width | `i64` |
| `Float` | Floating-point number of the runtime's float width | `f64` |
| `bool` | Boolean value | `bool` |
| `Text` | UTF-8 string | `String` |
| `()` | Unit type | `()` |

The `Word` and `Float` Rust equivalents shown reflect the bundled default runtime. Hosts that instantiate the parametric `GenericVm<W, A, F>` shape pick narrower Rust types for `Word` and `Float`. See [TYPE_SYSTEM.md, Primitive Types](TYPE_SYSTEM.md#primitive-types) for the full statement.

All numeric operations use `Word` or `Float`. Smaller integer types (`u8`, `u32`) from host structs are widened to `Word` when accessed in Keleusma. Native function bindings handle the narrowing conversion at the boundary.

### Composite Types

**Structs**: Named product types with named fields.

````
struct Note {
  channel: Word,
  pitch: Word,
  velocity: Float,
}
````

**Enums**: Named sum types with variants. Variants may carry data and may carry an explicit numeric discriminant.

````
enum Command {
  NoteOn(Word, Word, Float),
  NoteOff(Word),
  SetTempo(Float),
  Silence,
}
````

Variants without an explicit `= N` clause receive an implicit discriminant. The first variant defaults to zero; each subsequent implicit variant takes one more than the preceding variant's discriminant. Variants with an explicit clause take the literal value.

````
enum StatusErrorCode {
  // Discriminant 0 is reserved as the unused / default-initialised
  // sentinel; the `Status::Ok` variant carries the no-error case.
  OutOfRange = 1,
  NotConfigured = 2,
  Busy = 3,
  Timeout = 4,
  HardwareFault = 5,
  Unsupported = 6,
}
````

Explicit and implicit forms may be mixed; implicit values continue from the most recent explicit one.

````
enum Mixed {
  A,       // discriminant 0
  B = 10,  // discriminant 10
  C,       // discriminant 11
  D = 20,  // discriminant 20
  E,       // discriminant 21
}
````

The discriminant clause accepts an integer literal, optionally preceded by a unary minus. Expression-position arithmetic, named constants, and casts are not admissible in the discriminant clause itself. Duplicate discriminant values within a single enum are rejected by the parser with an error pointing at the second occurrence.

```
enum Signed {
  Below = -2,
  Just  = -1,
  Zero  = 0,
  Above = 1,
}
```

````
enum Bad {
  A = 1,
  B = 1,  // ERROR: variant `B` discriminant 1 duplicates variant `A`
}
````

**Tuples**: Anonymous product types.

````
let pair: (Word, Float) = (42, 3.14);
````

**Fixed-size arrays**: Homogeneous sequences of known length.

````
let channels: [Float; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
````

**Optionals**: Nullable values using Option.

````
let maybe_name: Option<Text> = Option::Some("Alice");
let nothing: Option<Text> = Option::None;
````

### Opaque Types

V0.2.0 introduced first-class opaque type support. The host implements the `HostOpaque` marker trait on a Rust newtype around the value it wants to expose; the script declares the type by name in function signatures and the type checker resolves the name as `Type::Opaque`. Native functions produce opaque values through `host_arc(...)` (returning `Value::Opaque(Arc<dyn HostOpaque>)`) and consume them by extracting a typed reference through `dyn HostOpaque::downcast_ref::<T>()`. Opaque values are host-managed through `Arc`, have a lifetime independent of the arena, may cross the yield boundary in the dialogue type, and contribute zero to the script-side WCMU bound. Equality is by `Arc` pointer identity. See [`examples/opaque_rust_string.rs`](../../examples/opaque_rust_string.rs) for the worked end-to-end pattern.

````
// ChannelHandle is the host's opaque newtype, registered with the
// VM through register_native. The script declares it by name; the
// type checker resolves it as Type::Opaque and the native receives
// Value::Opaque(Arc<dyn HostOpaque>).
let ch: ChannelHandle = audio::get_channel(0);
audio::set_frequency(ch, 440.0);
````

A future release will add a `Value::Opaque` variant and a marshalling path so opaque host types can flow as themselves rather than as `Word` handles. Tracked in the backlog.

### Type Coercion

No implicit type coercion exists. Numeric conversion requires the `as` keyword.

````
let x: Word = 42;
let y: Float = x as Float;
let z: Word = y as Word;    // Truncates toward zero.
````

Enum values cast to `Word` produce the variant's discriminant.

````
enum Code { Ok = 0, Busy = 3, Timeout = 4 }

fn classify(c: Code) -> Word {
    c as Word
}
````

The cast respects both implicit and explicit discriminants. The reverse direction, a `Word` cast back to an enum, is admissible only through the discriminant-to-enum construct described in the "Discriminant-to-Enum Construct" section below. A bare `Word as Enum` cast without the outcome-arm block remains inadmissible, because a discriminant alone cannot reconstruct a payload-bearing variant.

Text conversion is the host's responsibility. The runtime does not bundle a `to_string` native; hosts register one (typically named `format` or `to_string`) through `register_verified_native` or the `register_fn` marshalling layer.

## 4. Expressions

### Arithmetic Expressions

Standard arithmetic with operator precedence. Multiplication, division, and modulo bind tighter than addition and subtraction. Parentheses override precedence.

````
let result = (a + b) * c / d - e % f;
````

Integer arithmetic (`Word`) and floating-point arithmetic (`Float`) do not mix without explicit `as` casts.

### Shift and Bitwise Expressions

Shift operators are assembly-mnemonic keywords: `lsl` logical left, `asl` arithmetic left, `lsr` logical (zero-fill) right, and `asr` arithmetic (sign-preserving) right. The `lsl` and `asl` forms produce the same value; `asl` additionally admits overflow capture in the checked-arithmetic construct because it denotes `x * 2^k` (the overflow-capturing form requires a constant amount). Shifts operate on `Word`, `Byte`, and `Multiword<N>` values and bind tighter than the additive operators. The amount is a `Word` and may be a constant literal or a runtime-variable value; a constant literal is range-checked at compile time, and a runtime amount at or beyond the value width shifts every bit out rather than trapping.

Bitwise operators are keyword mnemonics: `band`, `bor`, and `bxor` are the binary and, or, and exclusive-or; `bnot` is the prefix complement. They operate on `Word`, `Byte`, and `Multiword<N>` values (a `Byte` at the byte width). On a `Multiword<N>` the operation is applied limb by limb with no cross-limb interaction. Among the binary bitwise operators, `band` binds tightest, then `bxor`, then `bor`, and the whole bitwise group binds tighter than comparison. Disambiguation is by operator name and never by operand type.

````
let mask  = flags band 0x0F;
let flip  = value bxor all_ones;
let clear = bnot dirty_bits;
````

### Comparison and Logical Expressions

Comparison operators produce `bool`. The eager logical operators `and`, `or`, and `xor` always evaluate both operands; `not` is the prefix negation. The short-circuit operators `andalso` and `orelse` skip the right operand when the left already determines the result. There are no short-circuit `xor` or `not` forms because those operations cannot short-circuit. In a pure total context the eager and short-circuit forms yield the same value; the eager forms are preferred where a native side effect on the right must run unconditionally or where a data-independent evaluation is wanted for worst-case-execution-time analysis. Precedence among the boolean operators binds loosest to tightest as `orelse`, `andalso`, `or`, `xor`, `and`, then comparison.

````
if x > 0 and not done {
  process(x);
}

// The short-circuit form guards a call that must not run on the null case.
if ptr_valid orelse recover(state) {
  step(ptr);
}
````

### Function Call Expressions

````
let result = function_name(arg1, arg2, arg3);
````

Arguments are evaluated left to right.

### Pipeline Expressions

The pipeline operator `|>` passes the result of the left-hand expression as an argument to the right-hand function call.

**Default behavior**: The piped value becomes the first argument.

````
value |> transform() |> filter(threshold) |> output();

// Equivalent to:
output(filter(transform(value), threshold));
````

**Placeholder behavior**: When `_` appears in the argument list, the piped value is inserted at that position instead.

````
value |> insert(collection, _);

// Equivalent to:
insert(collection, value);
````

Only function calls are valid pipeline targets. The pipeline operator is left-associative with lower precedence than all other operators.

### Yield Expressions

The `yield` expression suspends the coroutine, sends a value to the host, and evaluates to the next input received from the host when the coroutine is resumed.

````
let next_input = yield output_value;
````

Yield can only appear in `loop` functions, `yield` functions, or functions that propagate yield (see Section 6). The type of `output_value` must match the yield output type declared in the function signature. The type of `next_input` matches the yield input type.

### Match Expressions

Pattern matching on values. Arms are evaluated top to bottom. The first matching arm executes.

````
match command {
  Command::NoteOn(ch, note, vel) => play_note(ch, note, vel),
  Command::NoteOff(ch) => stop_note(ch),
  Command::SetTempo(bpm) => set_tempo(bpm),
  Command::Silence => silence_all(),
}
````

Match expressions must be exhaustive. The compiler verifies that all enum variants are covered, or that a wildcard `_` arm is present.

### If/Else Expressions

````
let label = if level > 0.8 {
  "Loud"
} else {
  "Normal"
}
````

The `else` branch is required when the `if` is used as an expression (producing a value). Both branches must have the same type. When used as a statement, the `else` branch is optional.

### Struct Construction

````
let note = Note { channel: 0, pitch: 60, velocity: 0.8 };
````

All fields must be provided. There is no struct update syntax.

### Field Access

````
let ch = note.channel;
let vel = note.velocity;
````

Field access works on structs and tuples (using numeric index for tuples: `pair.0`, `pair.1`).

### Array Indexing

````
let value = channels[i];
let from_data = state.idx[ch];
state.idx[ch] = 0;
let cell = state.grid[row][col];
````

Array indexing uses `Word` indices. Out-of-bounds access causes the script to yield a runtime error to the host.

Indexed access on a data-segment field declared as an array type behaves identically at the surface but is lowered to direct indexed slot reads and writes against the data segment rather than to stack-resident array operations. The same syntax handles single-dimensional access (`state.idx[ch]`) and multi-dimensional access (`state.grid[row][col]`). Indexed assignment `state.field[i][j]... = expr;` is a distinct statement form (see the EBNF entry `data_field_index_assign`) because the language reserves data-segment assignment to a small set of LHS shapes.

## 5. Statements

### Variable Binding

````
let x: Word = 42;
let name = "Alice";        // Type inferred from right-hand side.
````

Variables are immutable once bound. Rebinding with a new `let` shadows the previous binding.

````
let x = 10;
let x = x + 1;             // Shadows the previous x.
````

### Expression Statements

Any expression can appear as a statement. The result is discarded.

````
play_note(0, 60, 0.8);     // Call for side effect; result discarded.
````

### For Loop

The `for` loop iterates over arrays and ranges. Iteration is guaranteed to terminate because arrays have fixed size and ranges have fixed bounds.

````
for note in notes {
  play_note(note.channel, note.pitch, note.velocity);
}

for i in 0..8 {
  audio::set_volume(i, 0.0);
}
````

The loop variable is immutable within each iteration. Ranges use `..` for exclusive upper bound. The compiler verifies that the iterable is a fixed-size array or a range expression with statically known or bounded endpoints.

All host-provided iterable types are assumed finite by contract. The compiler checks that only iterable types are used with `for..in`. The host is responsible for not providing infinite iterators.

### Break Statement

The `break` keyword exits a `for` loop early. It is valid only inside `for` loops.

````
for i in 0..8 {
  if channels[i] > 0.0 {
    break;
  }
  audio::set_volume(i, 1.0);
}
````

`break` is not valid in `loop` functions. The `loop` construct represents the coroutine tick loop and must always reach a `yield` on every iteration.

### Assert Statement

`assert cond` and `assert cond, "message"` express a debug assertion. The condition is a `bool`. The construct is a compile-out debug aid: under a debug build (`keleusma compile --debug`, or `compiler::compile_with_options` with `emit_debug`) the compiler emits a runtime check that traps when the condition is false; under an ordinary build the statement compiles out entirely and contributes no opcodes. The optional message and the source span ride in a strippable `AssertionContext` debug record (backlog item B29), so `keleusma strip` reduces a failure to a generic assertion trap while leaving the check in place.

````
assert n > 0;
assert index < len, "index past end of buffer";
````

Like `classify` and `declassify`, `assert` is **not** a reserved keyword. It is recognised as the assertion statement only at statement position when not followed by `(`; `assert(x)` remains a call to a user-defined function named `assert`. Because a debug and a release build differ in whether the check is present, the two are distinct compilations rather than a single artefact bridged by `strip` (which removes only the debug record, never opcodes).

### Loop Statement

````
loop {
  let input = yield process(input);
}
````

The `loop` construct repeats indefinitely. It is valid only within `loop` functions (see Section 6). The body must contain at least one `yield` on every execution path.

## 6. Functions

Keleusma scripts fall into three function categories based on their termination and yield properties. Each category uses a distinct declaration keyword.

### 6.1 Atomic Total Functions (`fn`)

Atomic functions do not yield. They must terminate, assuming all called native functions return. The compiler rejects `yield` expressions within atomic functions.

````
fn add(a: Word, b: Word) -> Word {
  a + b
}
````

The last expression in a function body is the return value. There is no `return` keyword.

Atomic functions may call other atomic functions. They may not call `loop` or `yield` functions.

### 6.2 Non-Atomic Total Functions (`yield`)

Non-atomic total functions may yield to the host and must eventually exit, assuming all called native functions return. They are declared with the `yield` keyword instead of `fn`.

````
yield configure_channel(ch: Word, cmd: AudioCommand) -> AudioAction {
  ch |> configure_vco(1, "sawtooth", 0.8);
  let cmd = yield AudioAction::SetChannelParam(ch, "vco_ready", 1.0);
  ch |> configure_adsr(0.1, 0.2, 0.8, 0.3);
  yield AudioAction::SetChannelParam(ch, "adsr_ready", 1.0)
}
````

Non-atomic total functions may call:
- Any atomic function (`fn`).
- Other non-atomic functions (`yield`) that share the same yield contract (same input and output types for the bidirectional yield).

The compiler verifies that all `yield` functions eventually reach the end of their body on every execution path.

### 6.3 Productive Divergent Functions (`loop`)

Productive divergent functions are yielding coroutine control loops. They restart at the top when they reach the bottom and must yield on every iteration. They never exit. Only the top-level entry point function of a script may be of this type.

````
loop main(cmd: AudioCommand) -> AudioAction {
  let cmd = yield process(cmd);
}
````

The implicit loop behavior means the function body restarts from the beginning after its last statement executes. The compiler guarantees that at least one `yield` occurs on every path through the body.

Productive divergent functions may call:
- Any atomic function (`fn`).
- Non-atomic functions (`yield`) that share the same yield contract.

Only one `loop` function may exist per script, and it serves as the coroutine entry point.

### 6.4 Yield Contract

The yield contract of a `loop` or `yield` function is defined by its parameter type (input from host) and return type (output to host). When a `yield` function is called from a `loop` or another `yield` function, the contracts must match. This allows yield expressions in the callee to transparently propagate to the host.

````
// Both share the yield contract: AudioCommand -> AudioAction
loop main(cmd: AudioCommand) -> AudioAction {
  let cmd = yield handle(cmd);
}

yield handle(AudioCommand::NoteOn(ch, note, vel)) -> AudioAction {
  AudioAction::PlayNote(ch, note, vel)
}

yield handle(AudioCommand::ConfigureChannel(ch)) -> AudioAction {
  ch |> configure_vco(1, "sawtooth", 0.8);
  ch |> configure_adsr(0.1, 0.2, 0.8, 0.3);
  ch |> configure_filter("lowpass", 2000.0, 0.7);
  let cmd = yield AudioAction::SetChannelParam(ch, "filter_ready", 1.0);

  // Apply initial modulation routing.
  for i in 0..8 {
    audio::set_mod_depth(ch, i, 0.5Float);
  }
  AudioAction::SetChannelParam(ch, "ready", 1.0)
}
````

### 6.5 Multiheaded Functions

Multiple function definitions with the same name, arity, and category form a single logical function. The runtime dispatches to the first head whose pattern matches the arguments, evaluated top to bottom.

The example below assumes the host has registered a `to_string` verified native (`Vm::register_verified_native("to_string", …)`) that converts a numeric argument to `Value::KStr`. Text composition through `+` allocates the result in the arena's top region.

````
use to_string

fn describe(Command::NoteOn(ch, note, vel)) -> Text {
  "Play note " + (note as Float |> to_string()) + " on channel " + (ch as Float |> to_string())
}

fn describe(Command::NoteOff(ch)) -> Text {
  "Stop channel " + (ch as Float |> to_string())
}

fn describe(Command::SetTempo(bpm)) -> Text {
  "Tempo: " + (bpm |> to_string())
}

fn describe(Command::Silence) -> Text {
  "Silence all channels"
}
````

Note that the last expression in each function body is the return value and does not require a semicolon.

**Rules**:
- All heads must have the same arity (number of parameters).
- All heads must have the same return type.
- All heads must use the same function category keyword.
- Pattern matching applies to all parameters, not just the first.
- The compiler checks exhaustiveness across all heads for enum types.

Multiheaded functions work with all three function categories (`fn`, `yield`, `loop`).

### 6.6 Guard Clauses

The `when` keyword adds boolean conditions to function heads. Guards are evaluated after pattern matching succeeds.

````
fn severity(level: Float) -> Text when level >= 0.9 {
  "critical"
}

fn severity(level: Float) -> Text when level >= 0.5 {
  "warning"
}

fn severity(level: Float) -> Text {
  "normal"
}
````

Guard expressions are limited to comparison operators, logical operators, arithmetic, and field access. Guard expressions must not call functions (to preserve deterministic dispatch ordering).

## 7. Pure and Impure Functions

Purity is a property of native functions registered by the host. The Keleusma compiler does not verify purity; analysis and optimization trust the host's declaration. The current registration API does not require an explicit purity annotation. Future analysis passes that exploit purity will read it from a host-supplied attestation.

### Pure Functions

Pure functions are deterministic and side-effect-free. A Keleusma function that calls only pure native functions and other pure Keleusma functions is itself pure.

````rust
// Rust host code, not Keleusma syntax.
vm.register_fn("math::lerp", |a: f64, b: f64, t: f64| -> f64 {
    a + (b - a) * t
});
````

### Impure Functions

Any routine may call an impure function. Any routine that calls an impure function is itself impure. Impurity is transitive.

````rust
// Rust host code, not Keleusma syntax.
vm.register_fn("audio::set_frequency", |channel_handle: i64, freq: f64| {
    // mutate host audio engine state here
});
````

### Purity as a Host Declaration

Purity is a declaration from the host, not verified by the Keleusma compiler. Analysis trusts the declaration. Certain guarantees, such as deterministic replay and optimization, will be invalid if the declaration is false.

## 7.5. V0.2 Surface Extensions

The following constructs were added to the surface during the V0.2 design pass. They supplement the core surface defined in Sections 1 through 7.

### Newtype Declarations

````
newtype_decl   = 'newtype' upper_ident '=' type_expr
                 [ 'where' lower_ident ]
                 [ 'with' saturate_clause { ',' saturate_clause } ]
                 ';'
saturate_clause = ( 'saturate_max' | 'saturate_min' ) '=' signed_int_lit
````

Introduces a distinct nominal type wrapping an underlying type. The bytecode representation is identical to the underlying. Two newtypes with different names are never interchangeable even when their underlying types match. Construction at expression position uses `Name(value)`; extraction uses `value as Underlying`. The optional `where` clause names a predicate function of signature `fn(Underlying) -> bool` that the compiler emits at every construction site (a trap fires on a false result). The optional `with` clause declares context-determined saturation values for the `saturate_max` and `saturate_min` keywords inside a numeric overflow construct. Either or both saturation values may be supplied. When the surrounding expected type (function return, annotated let binding) is the refined newtype, the keyword resolves to a constructor call against the declared literal; the refinement predicate is verified at runtime on that literal exactly as for any other constructor invocation.

Example:

````
newtype LocalProperMs = Word;
newtype OriginFrameMs = Word;

fn in_servo_range(x: Word) -> bool { x >= 0 and x <= 180 }
newtype ServoAngle = Word where in_servo_range;

fn nonneg(x: Word) -> bool { x >= 0 }
newtype Limited = Word where nonneg with saturate_max = 100, saturate_min = 0;

let t: LocalProperMs = LocalProperMs(42);
let raw: Word = t as Word;
let theta: ServoAngle = ServoAngle(90);    // predicate passes
````

### Numeric Overflow Construct

````
overflow_expr  = arith_expr '{' overflow_arm { ',' overflow_arm } [ ',' ] '}'
overflow_arm   = overflow_kind [ 'when' expr ] '=>' expr
overflow_kind  = 'ok' '(' arm_pattern ')'
               | 'overflow' '(' arm_pattern ',' arm_pattern ')'
               | 'underflow' '(' arm_pattern ',' arm_pattern ')'
arm_pattern    = '_' | lower_ident | signed_int_lit
````

Guards a single arithmetic operation against overflow, underflow, and, for division and modulo, a zero divisor. The operation is `+`, `-`, `*`, `/`, `%`, or unary `-` on `Word` operands, `+`, `-`, `*`, `/`, `%` on `Byte` operands, `+`, `-`, `*`, `/` on `Float` operands, or `+`, `-`, `*`, `/`, `%`, or unary `-` on `Fixed<N>` operands. On `Word` the runtime computes the true result in `i128` and pushes `(high, low, flag)`; the `overflow` and `underflow` arms destructure the high and low halves `(h, l)` so big-number arithmetic can chain through successive checked operations. On `Float` the Institute of Electrical and Electronics Engineers 754 operations are total, so there is no trap and no zero divisor; instead a `nan` arm matches a not-a-number result, `overflow` matches positive infinity, `underflow` matches negative infinity, and a division by zero produces one of these rather than trapping. The `ok` class must have an unguarded catch-all arm (bare identifier or wildcard). The `overflow` and `underflow` classes are optional; an omitted or unmatched class defaults to two's-complement wrapping. For `/` and `%`, a `zero_divisor(numerator)` arm handles a zero divisor and binds the numerator, and an unhandled zero divisor traps as a division by zero. An arm whose outcome cannot arise for the operator and operand type is a compile error. On the signed `Word` type, `+`, `-`, and `*` admit `overflow` and `underflow`, unary `-` admits `overflow` only, `/` admits `overflow` and `zero_divisor`, and `%` admits `zero_divisor` only. On the unsigned `Byte` type, `+` and `*` admit `overflow`, `-` admits `underflow`, `/` and `%` admit `zero_divisor`, and unary `-` is not available; the `Byte` `overflow` and `underflow` arms bind a single pattern matching the wrapped `Byte` result, written `overflow(w)`, rather than two halves. On the `Float` type, `+`, `-`, `*`, and `/` admit `overflow`, `underflow`, and `nan`, while `%` and unary `-` are not available; the `Float` `overflow`, `underflow`, and `nan` arms each bind a single pattern matching the result. The signed `Fixed<N>` type admits the same outcomes as `Word` (`+`, `-`, and `*` admit `overflow` and `underflow`, unary `-` admits `overflow`, `/` admits `overflow` and `zero_divisor`, and `%` admits `zero_divisor`), and `nan` is not available; the `Fixed` `overflow` and `underflow` arms bind a single pattern matching the wrapped `Fixed` result, written `overflow(v)`, rather than two halves. The checked `Fixed` multiply and divide wrap an out-of-range result to the bound value, unlike the plain `Fixed` multiply and divide which saturate. Patterns are admitted from a restricted subset (wildcard, variable, signed integer literal); an optional `when expr` guard between the pattern and the `=>` is checked as `Bool` and falls through to the next arm when false.

The `saturate_max` and `saturate_min` keywords inside arm bodies denote context-determined saturation values, resolved at the construct's operand type. On `Word` they resolve to `Word::MAX` and `Word::MIN`, on `Byte` to `255` and `0`, on `Float` to the largest and most-negative finite `Float`, and on `Fixed<N>` to the extremal `Q`-format bit patterns. When the surrounding expected type is a refined newtype declared with a `with saturate_max = N` or `with saturate_min = M` clause, the keyword resolves to a constructor call against that literal.

Example:

````
let y = state.x + n {
    ok(v) => v,
    overflow(_, _) => saturate_max,
    underflow(_, _) => saturate_min,
};

// Big-number addition. The high half is the carry word; the low
// half is the wrapped i64 result. Subsequent words consume the
// carry through similar checked additions.
let (hi, lo) = a + b {
    ok(v) => (0, v),
    overflow(h, l) => (h, l),
    underflow(h, l) => (h, l),
};

// Pattern-matched arms with a guard. The specialized arm fires
// only when the high half equals zero and the guard returns true.
let result = x + y {
    ok(v) => v,
    overflow(0, l) when l > 0 - 1000 => l,
    overflow(_, _) => saturate_max,
    underflow(_, _) => saturate_min,
};
````

### Array Indexing Construct

````
index_construct = array_index '{' index_arm { ',' index_arm } [ ',' ] '}'
index_arm      = index_kind [ 'when' expr ] '=>' expr
index_kind     = 'ok' '(' arm_pattern ')'
               | 'invalid_index' '(' arm_pattern ')'
arm_pattern    = '_' | lower_ident | signed_int_lit
````

Guards an array index against an out-of-bounds access. The construct shares the arm-block syntax of the numeric overflow construct, attached to an array-index expression rather than an arithmetic operation. The `ok` arm binds the indexed element, and the `invalid_index` arm binds the offending index `Word`. An index is out of bounds when it is negative or not less than the array length. The `ok` class must have an unguarded catch-all arm. The `invalid_index` class is optional; an unhandled out-of-bounds index traps as `IndexOutOfBounds`, carrying the index and the length. The arithmetic outcome arms (`overflow`, `underflow`, `zero_divisor`, `nan`) are inadmissible on an index, and `invalid_index` is inadmissible on an arithmetic operation. Patterns are drawn from the same restricted subset, and an optional `when expr` guard is checked as `Bool`.

Example:

````
let element = buffer[i] {
    ok(v) => v,
    invalid_index(_) => 0,
};

// Binding and reusing the offending index.
let safe = table[i] {
    ok(v) => v,
    invalid_index(idx) when idx < 0 => 0,
    invalid_index(idx) => table[idx % length(table)],
};
````

### Newtype Construction Construct

````
newtype_construct = newtype_call '{' newtype_arm { ',' newtype_arm } [ ',' ] '}'
newtype_arm    = newtype_kind [ 'when' expr ] '=>' expr
newtype_kind   = 'ok' '(' arm_pattern ')'
               | 'invalid_newtype' '(' arm_pattern ')'
arm_pattern    = '_' | lower_ident | signed_int_lit
````

Guards the construction of a refined newtype against a refinement-predicate failure. The construct shares the arm-block syntax of the other construct-family members, attached to a newtype constructor call. The `ok` arm binds the constructed newtype value, and the `invalid_newtype` arm binds the underlying value that the predicate rejected. The `ok` class must have an unguarded catch-all arm. The `invalid_newtype` class is optional, and an unhandled failure traps as `RefinementFailed`, the same fault a bare construction produces. The `invalid_newtype` arm is admissible only when the newtype carries a refinement predicate, because a non-refined newtype's construction is total and cannot fail. The arithmetic and indexing outcome arms are inadmissible on a construction. Patterns are drawn from the same restricted subset, and an optional `when expr` guard is checked as `Bool`.

Example:

````
fn in_range(x: Word) -> bool { x >= 0 and x <= 100 }
newtype Percent = Word where in_range;

let p = Percent(raw_value) {
    ok(v) => v,
    invalid_newtype(x) when x < 0 => Percent(0),
    invalid_newtype(_) => Percent(100),
};
````

### Discriminant-to-Enum Construct

````
discriminant_construct = word_as_enum_cast '{' disc_arm { ',' disc_arm } [ ',' ] '}'
disc_arm       = disc_kind [ 'when' expr ] '=>' expr
disc_kind      = 'ok' '(' disc_target ')'
               | 'payload_discriminant' '(' disc_target ')'
               | 'invalid_discriminant' '(' arm_pattern ')'
disc_target    = upper_ident | '_'
arm_pattern    = '_' | lower_ident | signed_int_lit
````

Converts a `Word` discriminant back into an enum value, the reverse of the enum-to-`Word` cast. Because only the discriminant is available, the three arm kinds split the variants by what the discriminant can determine. Arms match by variant name, not by raw discriminant, so the construct is robust to discriminant renumbering.

An `ok(Variant)` arm names a unit, that is discriminant-only, variant and overrides its conversion. A unit variant with no `ok` arm converts to itself. A generic `ok(v)` or `ok(_)` arm binds the converted unit-variant value as a blanket post-processor, applied to any unit variant without a specific `ok` arm. A `payload_discriminant(Variant)` arm names a payload-bearing variant whose payload the arm body supplies, since the discriminant cannot carry it; `payload_discriminant(_)` is a catch-all over the remaining payload variants. Coverage of every payload-bearing variant is mandatory, specifically or through the catch-all. An `invalid_discriminant(raw)` arm binds the raw `Word` of a discriminant that matches no variant; it is optional, and an unhandled invalid discriminant traps as `EnumVariantUnmapped`. The source must be a `Word`. The type checker rejects a payload variant in `ok`, a unit variant in `payload_discriminant`, and the arms of the other construct families.

Example:

````
enum Status { Ok = 0, Pending = 1, Failed(Word) = 2 }

let s = code as Status {
    ok(Pending) => Status::Ok,
    payload_discriminant(Failed) => Status::Failed(code),
    invalid_discriminant(raw) => Status::Failed(raw),
};
````

### Native-Error Construct

````
native_construct = native_call '{' native_arm { ',' native_arm } [ ',' ] '}'
native_arm     = native_kind [ 'when' expr ] '=>' expr
native_kind    = 'ok' '(' arm_pattern ')'
               | 'error' '(' arm_pattern ')'
arm_pattern    = '_' | lower_ident | signed_int_lit
````

Guards a native function call against a host failure. The construct shares the arm-block syntax of the other construct-family members, attached to a native call. The `ok` arm binds the success value, and the `error` arm binds the `Word` error code the native reported on failure. The `ok` class must have an unguarded catch-all arm. The `error` class is optional, and an unhandled native error propagates the host failure exactly as it would without the construct. The `error` arm is admissible on any native call, because fallibility is not tracked at compile time; on a native that never fails, the `error` arm is simply never taken. The arithmetic, indexing, newtype, and discriminant arms are inadmissible.

The host reports the `Word` code by returning an error that converts to it. The `keleusma-macros` `KeleusmaError` derive generates `From<E> for VmError` for a fieldless enum, mapping each variant to its discriminant as the code, so the native may `return Err(MyError::Variant.into())`. Pairing the host enum's discriminants with a script-side `enum` lets the error code be recovered structurally with the discriminant-to-enum construct.

A native that fails as part of ordinary control flow should instead return an option or result enum as a normal value and be handled by a standard match, which keeps expected failures in the type system. The `error` arm is reserved for exceptional host failures.

Example:

````
let row = host::fetch(id) {
    ok(v) => v,
    error(code) => code as FetchError {
        invalid_discriminant(raw) => FetchError::Unknown(raw),
    } |> recover(),
};
````

### Information-Flow Labels

````
labelled_type = type_expr_inner '@' label_spec
label_spec    = label_atom
              | '{' label_atom { ',' label_atom } '}'
label_atom    = upper_ident | '!' upper_ident
classify_expr = 'classify' postfix_expr '@' positive_label_spec
declassify_expr = 'declassify' postfix_expr '@' positive_label_spec
positive_label_spec = upper_ident | '{' upper_ident { ',' upper_ident } '}'
````

Types carry a set of user-defined information-flow labels written as `T@Label` for a single positive label or `T@{L1, L2}` for multiple. The empty label set is the pure state. The `classify` operator adds labels to a value; `declassify` removes them. Labels propagate through arithmetic operations, comparisons, conditional branches, and composite-type positions (tuple elements, array elements, option payloads). The label-flow rule at every position is `source.labels ⊆ target.labels`; violations are rejected at compile time. The mechanism is zero-cost at the bytecode layer.

A label atom inside `@{ ... }` may be prefixed with `!` to denote a *negative* label: `T@!Label` and `T@{!N1, !N2}` admit the wrapper at top-level boundary positions. Three boundary categories are recognised: function parameter and return types, `shared` data field types (the host-script boundary), and `private` data field types (the yield-resume boundary). The negative-disjoint rule is a boundary clause: a value flowing across the boundary may not carry any of the listed labels in its positive label set. The positive-label upper-bound rule is relaxed at boundaries with negative labels so the position admits any unlisted label. The check fires at every call site, return, yield, resume, and data-field assignment. Mixed positive-and-negative sets (e.g., `@{Open, !Secret}`) are rejected at parse time. The wrapper is admissible only at the top level of these positions; nested positions inside tuples, arrays, or options reject it. Negative labels do not appear in `classify` or `declassify` expressions because those operate on positive labels only. See `R43` in [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) for the design rationale and `B21` in [`docs/decisions/BACKLOG.md`](../decisions/BACKLOG.md) for the deferred value-side extension.

Example:

````
use host::transmit(payload: Word@Open) -> bool

fn read_position() -> Word@MissionSecret { 42 }

fn main() -> bool {
    let pos: Word@MissionSecret = read_position();
    // host::transmit(pos);                       // type error: label leak
    host::transmit(declassify pos@MissionSecret)  // explicit audit point
}
````

Example with a negative-label parameter at a native sink:

````
use host::log_open(payload: Word@!MissionSecret) -> ()

fn produce_telemetry() -> Word@Telemetry { 42 }
fn produce_classified() -> Word@MissionSecret { 99 }

fn main() -> () {
    host::log_open(produce_telemetry());        // ok: Telemetry != MissionSecret
    // host::log_open(produce_classified());    // type error: parameter forbids MissionSecret
}
````

### Signed Entry Function

````
function_def    = { 'ephemeral' | 'signed' } [ 'pure' ] (fn_def | yield_def | loop_def)
````

The `signed` modifier on the entry function declaration sets the wire-format flag `FLAG_REQUIRES_SIGNATURE = 0x02` in the module header. A host that loads the bytecode must verify the attached Ed25519 signature against a trust matrix populated through `Vm::register_verifying_key` before the module runs. The signing operation itself is a toolchain step independent of the compiler; the surface keyword expresses the requirement, the compiler emits the flag, and the runtime enforces it. Hosts loading signed modules use `Vm::load_signed_bytes(bytes, arena, &keys)` for the initial load or `Vm::replace_module_from_bytes` for hot-swap; `Vm::load_bytes` rejects signed modules with a diagnostic pointing at the correct entry point.

The modifier is admissible on any of the three function categories (`fn`, `yield`, `loop`) and may combine with `ephemeral` in either order. The compiler rejects the modifier on any function other than the module's entry point. The signing scheme is Ed25519 in V0.2.0; the wire format carries a `scheme_id` byte for future scheme migrations without an ABI break. See `R42` in [`docs/decisions/RESOLVED.md`](../decisions/RESOLVED.md) for the design rationale and [`docs/spec/WIRE_FORMAT.md`](./WIRE_FORMAT.md) for the header layout.

Example:

````
signed loop main(input: Word) -> Word {
    let next = yield input * 2;
    next
}
````

A device loading this bytecode rejects the module unless the attached signature verifies against a public key the operator has registered.

## 8. Pattern Matching

Patterns appear in function heads, `match` arms, and `let` bindings.

### Pattern Forms

| Pattern | Example | Matches |
|---------|---------|---------|
| Literal | `42`, `"hello"`, `true` | Exact value |
| Enum variant | `Command::NoteOn(ch, note, vel)` | Variant with bindings |
| Enum unit variant | `Command::Silence` | Variant without data |
| Struct destructuring | `Note { channel, pitch }` | Struct with field bindings |
| Tuple destructuring | `(a, b)` | Tuple with element bindings (two or more elements) |
| Grouped | `(p)` | Identical to `p`; the parentheses group only |
| Wildcard | `_` | Any value (ignored) |
| Variable | `x` | Any value (bound to name) |
| Option Some | `Option::Some(value)` | Non-None optional |
| Option None | `Option::None` | None optional |

A parenthesized pattern is disambiguated by its element count. A single parenthesized pattern `(p)` is a transparent grouping equivalent to `p`, and two or more comma-separated patterns `(a, b)` form a tuple pattern. A trailing-comma form `(p,)` is not a pattern and is rejected, mirroring the one-element tuple-literal rule in the expression grammar. A unit value is matched with a wildcard or a variable binding, not with `()`.

### Exhaustiveness

The compiler checks that `match` expressions and multiheaded function groups cover all possible values. For enum types, all variants must have a corresponding arm or a wildcard must be present. For numeric or string types, a wildcard or variable binding must be present as the final arm.

### Nested Patterns

Patterns can be nested.

````
match result {
  Option::Some(Command::NoteOn(ch, note, vel)) => play(ch, note, vel),
  Option::Some(Command::Silence) => silence(),
  Option::None => idle(),
  _ => skip(),
}
````

## 9. Native Function Binding

### Import Syntax

Scripts import native function groups using `use` declarations at the top of the file.

````
use audio::set_frequency
use audio::set_volume
use audio::play_note
use game::get_relationship
use game::display_message
````

Wildcard imports are also supported.

````
use audio::*
use game::*
````

### Host Registration

The host application registers native functions before compiling scripts. The current ergonomic API accepts ordinary Rust functions and closures of arity zero through four whose argument and return types implement the `KeleusmaType` trait. The marshalling layer derives the parameter and return types automatically; no separate type-list registration is required.

````rust
// Rust host code, not Keleusma syntax.
vm.register_fn("math::lerp", |a: f64, b: f64, t: f64| -> f64 {
    a + (b - a) * t
});
vm.register_fn("audio::set_frequency", |channel: i64, freq: f64| {
    // mutate host state
});
vm.register_fn("audio::play_note", |channel: i64, note: i64, vel: f64| {
    // ...
});
vm.register_fn_fallible("game::get_relationship", |a: i64, b: i64| -> Result<f64, VmError> {
    // ...
});
````

For functions that need to inspect arbitrary `Value` variants, the lower-level `register_native(name, fn(&[Value]) -> Result<Value, VmError>)` and `register_native_closure(name, Box<dyn Fn(...)>)` paths are also available. Native functions that allocate dynamic strings into the host-owned arena register through `register_native_with_ctx` and receive a `NativeCtx<'_>` carrying a borrow of the arena. See [EMBEDDING.md](../guide/EMBEDDING.md) for the complete registration surface.

### Type Validation

The compiler validates all native function calls against the registered set at compile time. Calling an unregistered function produces a compile error. Argument-type validation happens at the marshalling boundary at runtime; type mismatches surface as `VmError::TypeError`.

### Opaque Types

V0.2.0 introduced first-class opaque type support through the `HostOpaque` marker trait and the `Value::Opaque(Arc<dyn HostOpaque>)` runtime variant. The host registers a native that produces opaque values through `host_arc(...)`; native consumers extract a typed reference through `dyn HostOpaque::downcast_ref::<T>()`. The script declares the opaque type by name in function signatures and the type checker resolves the name as `Type::Opaque`. Opaque values are host-managed through `Arc`, have a lifetime independent of the arena, may cross the yield boundary in the dialogue type, and contribute zero to the script-side WCMU bound. See the "Opaque Host Types" section of [`docs/guide/EMBEDDING.md`](../guide/EMBEDDING.md) for the full host-side surface and [`examples/opaque_rust_string.rs`](../../examples/opaque_rust_string.rs) for a worked end-to-end example.

````
let ch: ChannelHandle = audio::get_channel(0);
audio::set_frequency(ch, 440.0);
````

A future release will add a `Value::Opaque` variant and a marshalling path so that opaque host types can flow as themselves rather than as `Word` handles. Tracked in the backlog.

## 10. Module System

Each script file constitutes one module. Modules cannot import other Keleusma modules. All external functionality comes from native function registrations.

````
// audio_track.kel
use audio::*

loop main(cmd: AudioCommand) -> AudioAction {
  let cmd = yield process(cmd);
}
````

File extension is `.kel` for source files and `.kel.bin` for compiled bytecode.

## 11. Formal Grammar (EBNF)

````
program         = [ shebang_line ]
                  { use_decl }
                  { type_def | data_decl | trait_def | impl_block | function_def }

(* Shebang. The lexer skips a leading '#!...' line so source scripts may
   carry a Unix shebang. Line numbers in error messages start from line 2
   in that case. *)
shebang_line    = '#!' { ? any character except newline ? } newline

(* Imports *)
(* The optional `external` modifier marks the import as an external
   native whose per-iteration cost is bounded by invocation count
   rather than by an attested per-call budget. A single-segment path
   (`use foo`) is admissible; the leading `module_path ::` prefix is
   optional. The optional native-import signature clause declares the
   parameter type list and return type of the imported native so the
   type checker can validate call-site argument types and assign the
   declared return type. *)
use_decl        = 'use' [ 'external' ]
                  ( [ module_path '::' ] lower_ident [ native_signature ]
                  | module_path '::' '*' )
native_signature = '(' [ type_list ] ')' '->' type_expr
module_path     = lower_ident { '::' lower_ident }

(* Type Definitions *)
type_def        = struct_def | enum_def
struct_def      = 'struct' upper_ident [ type_params ] '{' { field_decl } '}'
field_decl      = lower_ident ':' type_expr
enum_def        = 'enum' upper_ident [ type_params ] '{' { variant_decl } '}'
variant_decl    = upper_ident [ '(' type_list ')' | '{' field_decl_list '}' ]
type_list       = type_expr { ',' type_expr }
field_decl_list = field_decl { ',' field_decl }

(* Generic Type Parameters and Bounds *)
(* A parameter list mixes type parameters and const parameters. A const
   parameter is a lowercase name introduced by the `const` keyword; its
   type is `Word`, the only admissible const-parameter type, and may be
   written explicitly. Type parameters and const parameters may appear in
   any order in the declaration. *)
type_params     = '<' generic_param { ',' generic_param } '>'
generic_param   = type_param | const_param
type_param      = upper_ident [ ':' trait_bound_list ]
const_param     = 'const' lower_ident [ ':' 'Word' ]
trait_bound_list = upper_ident { '+' upper_ident }

(* A const expression appears in a const-argument position and in an
   array size or Multiword parameter. It is total arithmetic over the
   operators `+`, `-`, and `*` with the usual precedence and left
   associativity, over integer literals and const parameters, with
   parenthesised grouping. Division and modulo are excluded so evaluation
   cannot fail. Comparison and shift operators are excluded so a closing
   `>` in a `<...>` list is never ambiguous with an operator. *)
const_expr      = const_add
const_add       = const_mul { ('+' | '-') const_mul }
const_mul       = const_atom { '*' const_atom }
const_atom      = integer_lit | lower_ident | '(' const_expr ')'

(* Trait Declarations *)
trait_def       = 'trait' upper_ident [ type_params ] '{'
                  { trait_method_sig } '}'
trait_method_sig = 'fn' lower_ident '(' [ type_signature_list ] ')' '->' type_expr ';'
type_signature_list = type_expr { ',' type_expr }

(* Impl Blocks *)
impl_block      = 'impl' [ type_params ] upper_ident 'for' type_expr '{'
                  { impl_method } '}'
impl_method     = 'fn' lower_ident '(' [ param_list ] ')' '->' type_expr
                  '{' block '}'

(* Data Declarations *)
data_decl          = [ visibility_mod ] 'data' lower_ident
                     '{' { data_field_decl } '}'
visibility_mod     = 'shared' | 'private' | 'const'
data_field_decl    = lower_ident ':' type_expr [ '=' const_initializer ]
const_initializer  = scalar_literal
                   | '(' const_initializer { ',' const_initializer } ')'
                   | '[' [ const_initializer { ',' const_initializer } ] ']'
scalar_literal     = [ '-' ] integer_lit
                   | [ '-' ] float_lit
                   | 'true' | 'false'
                   | string_lit
                   | '(' ')'

(* Types *)
type_expr       = prim_type | named_type | tuple_type | array_type | option_type
                | multiword_type
prim_type       = 'Word' | 'Float' | 'bool' | 'Text' | '(' ')'
(* A named type's argument list carries type arguments followed by const
   arguments. A const argument is a const expression; a type argument may
   not follow a const argument. A pure const argument such as `Buf<8>` or
   `Buf<n + 1>` is admissible. *)
named_type      = upper_ident [ '<' generic_arg { ',' generic_arg } '>' ]
generic_arg     = type_expr | const_expr
tuple_type      = '(' type_expr ',' type_expr { ',' type_expr } ')'
array_type      = '[' type_expr ';' const_expr ']'
option_type     = 'Option' '<' type_expr '>'
(* Multi-word fixed-point type. The first argument is the word count
   N in [1, 65535]; the optional second argument is the fraction-bit
   count F in [0, 65535], defaulting to 0 (the integer case). Each
   argument is a const expression. A literal argument is range-checked at
   parse time; a symbolic argument is range-checked after substitution at
   monomorphization. *)
multiword_type  = 'Multiword' '<' const_expr [ ',' const_expr ] '>'

(* Functions *)
(* The `ephemeral` and `signed` modifiers are permitted only on
   the entry point. Either or both may appear in any order. The
   compiler rejects the modifier on a non-entry function with a
   diagnostic naming the offending declaration. The optional `pure`
   modifier follows any `ephemeral`/`signed` modifiers and is a
   purity annotation on the definition. *)
function_def    = { 'ephemeral' | 'signed' } [ 'pure' ] (fn_def | yield_def | loop_def)
fn_def          = 'fn' lower_ident [ type_params ] '(' [ param_list ] ')' '->' type_expr
                  [ 'when' guard_expr ] '{' block '}'
yield_def       = 'yield' lower_ident [ type_params ] '(' [ param_list ] ')' '->' type_expr
                  [ 'when' guard_expr ] '{' block '}'
loop_def        = 'loop' lower_ident [ type_params ] '(' [ param_list ] ')' '->' type_expr
                  [ 'when' guard_expr ] '{' block '}'
param_list      = param { ',' param }
param           = pattern ':' type_expr
                | pattern

(* Guard expressions, restricted to comparisons and logic. *)
guard_expr      = guard_term { ('and' | 'or') guard_term }
guard_term      = [ 'not' ] guard_atom
guard_atom      = expression comparison_op expression
                | '(' guard_expr ')'
comparison_op   = '==' | '!=' | '<' | '>' | '<=' | '>='

(* Blocks and Statements *)
block           = { statement } [ expression ]
statement       = let_stmt | for_stmt | break_stmt | assert_stmt
                | data_field_assign | data_field_index_assign
                | expr_stmt
let_stmt        = 'let' pattern [ ':' type_expr ] '=' expression ';'
assert_stmt     = 'assert' expression [ ',' string_lit ] ';'
for_stmt        = 'for' lower_ident 'in' iterable '{' block '}'
iterable        = expression
                | expression '..' expression
break_stmt      = 'break' ';'
data_field_assign       = lower_ident '.' lower_ident '=' expression ';'
data_field_index_assign = lower_ident '.' lower_ident '[' expression ']'
                          { '[' expression ']' } '=' expression ';'
expr_stmt       = expression ';'

(* Expressions *)
expression      = pipeline_expr
pipeline_expr   = logical_expr { '|>' qualified_name '(' [ arg_list ] ')' }
(* The boolean operators, loosest to tightest binding: `orelse`,
   `andalso`, `or`, `xor`, `and`, then comparison. `andalso` and `orelse`
   are the short-circuit control forms; `and`, `or`, and `xor` are eager
   and always evaluate both operands. All are left-associative. *)
logical_expr    = orelse_expr
orelse_expr     = andalso_expr { 'orelse' andalso_expr }
andalso_expr    = or_expr { 'andalso' or_expr }
or_expr         = xor_expr { 'or' xor_expr }
xor_expr        = and_expr { 'xor' and_expr }
and_expr        = comparison_expr { 'and' comparison_expr }
comparison_expr = bitwise_expr [ comparison_op bitwise_expr ]
(* Bitwise operators bind below the comparisons and above the shifts.
   They are keyword mnemonics, so the operation is chosen by name and
   never by operand type. `band` binds tightest, then `bxor`, then `bor`;
   all are left-associative and operate on `Word` and `Multiword<N>`. *)
bitwise_expr    = bxor_expr { 'bor' bxor_expr }
bxor_expr       = band_expr { 'bxor' band_expr }
band_expr       = shift_expr { 'band' shift_expr }
(* Shifts bind below the bitwise operators and above the additive
   operators. They are keyword operators named after the assembly
   mnemonics, so the arithmetic-versus-logical choice is explicit. `lsl`
   is the logical left shift, `asl` the arithmetic left shift, `lsr` the
   logical (zero-fill) right shift, and `asr` the arithmetic
   (sign-preserving) right shift. The right operand is a
   compile-time-constant amount in the current increment. Because the
   operators are keywords, they never collide with a stacked generic
   close such as `Option<Option<T>>`. *)
shift_expr      = additive_expr { ('lsl' | 'asl' | 'lsr' | 'asr') additive_expr }
additive_expr   = multiplicative_expr { ('+' | '-') multiplicative_expr }
multiplicative_expr = unary_expr { ('*' | '/' | '%') unary_expr }
unary_expr      = [ 'not' | '-' | 'bnot' ] postfix_expr
postfix_expr    = primary_expr { method_call | field_access | tuple_index | array_index }
method_call     = '.' lower_ident '(' [ arg_list ] ')'
field_access    = '.' lower_ident
tuple_index     = '.' integer_lit
array_index     = '[' expression ']'
primary_expr    = literal
                | lower_ident
                | upper_ident [ const_args ] '::' upper_ident [ '(' [ arg_list ] ')' ]
                | upper_ident [ const_args ] [ '{' field_init_list '}' ]
                | function_call
                | yield_expr
                | if_expr
                | match_expr
                | loop_block
                | '(' ')'
                | '(' expression ')'
                | '(' expression ',' expression { ',' expression } [ ',' ] ')'
                | '[' [ arg_list ] ']'
                | expression 'as' type_expr
                | multiword_ctor

(* Multi-word fixed-point constructor. A turbofish carrying the word
   count and optional fraction bits, applied to N word arguments. It
   desugars to a tuple of the arguments cast to the same Multiword
   type, and is the only construction form for the single-word case
   since a one-element tuple is not surface syntax. *)
multiword_ctor  = 'Multiword' '::' '<' integer_lit [ ',' integer_lit ] '>'
                  '(' [ arg_list ] ')'

(* Closures, f-strings, and first-class function references are not
   admitted in V0.2.0. Closure-shaped expressions and bare function
   identifiers in non-call position are rejected at the type-checker
   stage with a diagnostic that names the construct. F-string
   interpolation was removed in V0.2.0 Phase 3.5; hosts compose
   dynamic text through a registered `format` native. *)

literal         = integer_lit | float_lit | string_lit | bool_lit
qualified_name  = lower_ident { '::' lower_ident }
(* A call, a struct construction, and an enum-variant construction each
   supply const arguments through an explicit turbofish, because a const
   argument cannot be inferred from the value arguments. The turbofish
   carries const expressions only. *)
const_args      = '::' '<' const_expr { ',' const_expr } '>'
function_call   = qualified_name [ const_args ] '(' [ arg_list ] ')'
arg_list        = expression { ',' expression }
                | expression { ',' expression } ',' '_'
                | '_' { ',' expression }
field_init_list = field_init { ',' field_init }
field_init      = lower_ident ':' expression

(* Yield *)
yield_expr      = 'yield' expression

(* If / Else *)
if_expr         = 'if' expression '{' block '}' [ 'else' '{' block '}' ]

(* Loop block. A bare `loop { body }` is a primary expression yielding
   a loop expression. It is admissible only in a `loop`-category entry
   function, where it denotes the coroutine tick loop, and its body
   must reach a `yield` on every path (see Section 5, "Loop
   Statement", and Section 6.3). *)
loop_block      = 'loop' '{' block '}'

(* Match *)
(* An arm carries an optional `when` guard, checked as `bool` after
   the pattern matches; a false guard falls through to the next arm.
   The arm-terminating comma is optional, so the final arm may omit
   it. *)
match_expr      = 'match' expression '{' { match_arm } '}'
match_arm       = pattern [ 'when' expression ] '=>' expression [ ',' ]

(* Patterns *)
pattern         = literal_pattern
                | enum_pattern
                | struct_pattern
                | tuple_pattern
                | wildcard_pattern
                | variable_pattern
literal_pattern = integer_lit | float_lit | string_lit | bool_lit | '(' ')'
enum_pattern    = upper_ident '::' upper_ident [ '(' pattern_list ')'
                                                | '{' field_pattern_list '}' ]
struct_pattern  = upper_ident '{' field_pattern_list '}'
tuple_pattern   = '(' pattern ',' pattern { ',' pattern } ')'
wildcard_pattern = '_'
variable_pattern = lower_ident
pattern_list    = pattern { ',' pattern }
field_pattern_list = field_pattern { ',' field_pattern }
field_pattern   = lower_ident [ ':' pattern ]

(* Lexical *)
lower_ident     = [a-z_] [a-z0-9_]*
upper_ident     = [A-Z] [A-Za-z0-9]*
integer_lit     = ( [0-9]+ [ 'Word' ] ) | '0x' [0-9a-fA-F]+ | '0b' [01]+
float_lit       = [0-9]+ '.' [0-9]+ [ 'Float' ]
string_lit      = '"' { string_char } '"'
string_char     = [^"\\] | '\\' escape_char
escape_char     = 'n' | 't' | 'r' | '\\' | '"' | '0'
bool_lit        = 'true' | 'false'
line_comment    = '//' { any_char } newline
block_comment   = '/*' { any_char } '*/'
````

### Notes on the EBNF

This grammar is the authoritative, normative specification of the surface syntax. The parser at [`src/parser.rs`](../../src/parser.rs) is expected to conform to it. A disagreement between the grammar and the parser is a defect to be resolved by correcting whichever side is wrong; the parser does not automatically win, and the grammar is not automatically deemed stale.

The grammar describes the surface only. The verifier rejects programs whose Worst-Case Execution Time or Worst-Case Memory Usage cannot be statically bounded under the conservative-verification stance, and the type checker enforces additional constraints around generics, trait bounds, exhaustive match, and the data-segment fixed-size discipline. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) for those layers.

The parser no longer accepts closure literals or first-class function references; both are rejected at the type-checker stage with a diagnostic that names the construct. The `Op::CallIndirect`, `Op::PushFunc`, `Op::MakeClosure`, and `Op::MakeRecursiveClosure` opcodes were retired in V0.2.0 Phase 4 alongside the `Value::Func` runtime variant. Programs that require definitive Worst-Case Execution Time and Worst-Case Memory Usage bounds restrict themselves to direct calls and trait dispatch.

The pipeline operator `|>` requires parentheses on the right-hand call even when the call is nullary; `expr |> f` is a parse error, `expr |> f()` is correct.

If-else and match expressions used at statement position require an explicit trailing semicolon when followed by another statement, even though they evaluate to unit. The parser does not auto-insert.

## 12. Example Programs

### Audio DSL: MS-20 Channel Controller

This script receives audio commands per tick and yields synthesis configuration actions.

````
use audio::*;

enum AudioCommand {
  NoteOn(Word, Word, Float),
  NoteOff(Word),
  SetTempo(Float),
  ConfigureChannel(Word),
  Tick,
}

enum AudioAction {
  PlayNote(Word, Word, Float),
  StopNote(Word),
  SetChannelParam(Word, Text, Float),
  NoOp,
}

loop main(cmd: AudioCommand) -> AudioAction {
  let cmd = yield process(cmd);
}

yield process(AudioCommand::NoteOn(ch, note, vel)) -> AudioAction {
  AudioAction::PlayNote(ch, note, vel)
}

yield process(AudioCommand::NoteOff(ch)) -> AudioAction {
  AudioAction::StopNote(ch)
}

yield process(AudioCommand::ConfigureChannel(ch)) -> AudioAction {
  // Set up an MS-20 voice with sawtooth VCO, medium attack, full sustain.
  ch |> configure_vco(1, "sawtooth", 0.8);
  ch |> configure_adsr(0.1, 0.2, 0.8, 0.3);
  ch |> configure_filter("lowpass", 2000.0, 0.7);
  let cmd = yield AudioAction::SetChannelParam(ch, "filter_ready", 1.0);

  // Apply initial modulation routing.
  for i in 0..8 {
    audio::set_mod_depth(ch, i, 0.5Float);
  }
  AudioAction::SetChannelParam(ch, "ready", 1.0)
}

yield process(AudioCommand::SetTempo(bpm)) -> AudioAction {
  set_global_tempo(bpm);
  AudioAction::NoOp
}

yield process(AudioCommand::Tick) -> AudioAction {
  AudioAction::NoOp
}

fn configure_vco(ch: Word, vco_id: Word, waveform: Text, amplitude: Float) -> () {
  audio::set_vco_waveform(ch, vco_id, waveform);
  audio::set_vco_amplitude(ch, vco_id, amplitude);
}

fn configure_adsr(ch: Word, a: Float, d: Float, s: Float, r: Float) -> () {
  audio::set_envelope(ch, "eg2", a, d, s, r);
}

fn configure_filter(ch: Word, filter_type: Text, cutoff: Float, resonance: Float) -> () {
  audio::set_lpf_cutoff(ch, cutoff);
  audio::set_lpf_resonance(ch, resonance);
}
````

Note that `process` is declared with `yield` because calling it from the `loop main` function requires yield propagation. The `configure_vco`, `configure_adsr`, and `configure_filter` helpers are declared with `fn` because they do not yield.

### Game DSL: Scenario Event Handler

This script receives game events per turn and yields scripted actions.

````
use game::*;

enum GameEvent {
  TurnStart(Word, Text),
  DeploymentWarning(Word),
  CharacterDeath(Word, Text),
  BombDetonation(Word, Word),
  DateCompleted(Word, Word, Float),
  Idle,
}

enum ScriptAction {
  DisplayMessage(Text, Text),
  ModifyRelationship(Word, Word, Float),
  TriggerEvent(Text),
  NoAction,
}

loop main(event: GameEvent) -> ScriptAction {
  let event = yield handle(event);
}

yield handle(GameEvent::TurnStart(turn, turn_type)) -> ScriptAction {
  if turn == 1Word {
    ScriptAction::DisplayMessage("Narrator", "Welcome aboard the generation ship.")
  } else {
    ScriptAction::NoAction
  }
}

yield handle(GameEvent::DeploymentWarning(turns_until)) -> ScriptAction when turns_until <= 3Word {
  ScriptAction::DisplayMessage("Commander", "Deployment imminent. Prepare for combat.")
}

yield handle(GameEvent::DeploymentWarning(turns_until)) -> ScriptAction {
  ScriptAction::NoAction
}

yield handle(GameEvent::CharacterDeath(char_id, name)) -> ScriptAction {
  let relationship = game::get_friendliness(0Word, char_id);
  if relationship > 50.0 {
    ScriptAction::DisplayMessage("Narrator", name + " is gone. You feel the loss deeply.")
  } else {
    ScriptAction::DisplayMessage("Narrator", name + " did not survive the deployment.")
  }
}

yield handle(GameEvent::BombDetonation(source_id, target_id)) -> ScriptAction {
  let source_name = game::get_character_name(source_id);
  ScriptAction::DisplayMessage(source_name, "I cannot forgive what happened between us.")
}

yield handle(GameEvent::DateCompleted(actor, target, delta)) -> ScriptAction when delta > 3.0 {
  ScriptAction::TriggerEvent("great_date")
}

yield handle(GameEvent::DateCompleted(actor, target, delta)) -> ScriptAction when delta < -3.0 {
  ScriptAction::TriggerEvent("terrible_date")
}

yield handle(GameEvent::DateCompleted(_, _, _)) -> ScriptAction {
  ScriptAction::NoAction
}

yield handle(GameEvent::Idle) -> ScriptAction {
  ScriptAction::NoAction
}
````

Note that `handle` is declared with `yield` because it is called from the `loop main` function. Although some heads do not contain explicit `yield` expressions, the function category must be consistent across all heads. A `yield` function that does not yield on a given code path simply returns its value without suspending.

## 13. Comparison with Related Languages

### Keleusma and Elixir

| Feature | Keleusma | Elixir |
|---------|--------|--------|
| Block delimiters | `{ }` | `do`/`end` |
| Multiheaded functions | Yes, same concept | Yes |
| Guard clauses | `when` keyword | `when` keyword |
| Pattern matching | Function heads and `match` | Function heads, `case`, `cond` |
| Pipeline operator | `\|>` (first arg or `_` placeholder) | `\|>` (first arg only) |
| Type annotations | Required (Rust syntax) | Optional (typespec) |
| Variables | Immutable (shadow rebinding) | Immutable (rebinding) |
| Module system | One file per module | Multi-module files |
| Macros | None | Extensive |
| Concurrency | Single coroutine | Actor model (OTP) |
| String interpolation | None | `#{}` syntax |

**Rationale**: Keleusma adopts Elixir's quality-of-life patterns (multihead, guards, pipeline) while using Rust type syntax and curly brace blocks for consistency with the host language.

### Keleusma and Rust

| Feature | Keleusma | Rust |
|---------|--------|------|
| Type syntax | Same | Same |
| Block delimiters | `{ }` | `{ }` |
| Ownership | None (VM-managed values) | Affine types |
| Lifetimes | None | Yes |
| Traits | None | Yes |
| Generics | None | Yes |
| Closures | None | Yes |
| Error handling | Host-managed | Result/Option |
| Memory model | Stack-based VM | Stack/heap with RAII |
| Compilation target | Bytecode | Machine code |
| Enums | Yes (same syntax) | Yes |
| Structs | Yes (same syntax) | Yes |
| Match | Yes (exhaustive) | Yes (exhaustive) |
| For loops | Over arrays and ranges | Over iterators |
| Function categories | `fn`/`yield`/`loop` | `fn`/`async fn` |

**Rationale**: Keleusma uses Rust type syntax and curly brace blocks for familiarity and interoperability, but strips away ownership, generics, and traits to keep the VM simple and compilation fast. The three function categories (`fn`/`yield`/`loop`) provide coroutine support analogous to Rust's `async`/`await` but with bidirectional typed yield.

### Keleusma and Rhai

| Feature | Keleusma | Rhai |
|---------|--------|------|
| Typing | Static nominal | Dynamic |
| `no_std` | Yes | No |
| Pattern matching | Multihead + match | Switch statement |
| Pipeline | `\|>` operator | No |
| Coroutines | First-class yield | No |
| Function registration | Typed registry with purity | Typed via Rust generics |
| Closures | None | Yes |
| OOP | None | Property access |
| Purity tracking | Yes (host-declared) | No |

**Rationale**: Keleusma provides stronger static guarantees, purity tracking, and a coroutine model that Rhai lacks, at the cost of dynamic features like closures. The `no_std` requirement eliminates Rhai as a direct alternative.

### Keleusma and Synchronous Languages (Lustre/Esterel/SCADE)

| Feature | Keleusma | Lustre/Esterel/SCADE |
|---------|--------|------|
| Synchronous hypothesis | Yes (yield-to-yield bounded) | Yes (tick-based) |
| Temporal model | Single yield clock + phase clock | Multi-clock (Lustre), concurrent (Esterel) |
| Compilation target | Bytecode VM | C code / automata (Lustre, SCADE) |
| Concurrency | Single coroutine | Concurrent composition (Esterel) |
| Memory model | Arena (bump allocation, cleared at RESET) | Static allocation |
| Host interaction | Bidirectional typed yield | Sensor/actuator interface |

**Rationale**: Keleusma applies synchronous language principles to an embeddable bytecode VM for game and audio scripting. It shares the synchronous hypothesis (all computation within a logical tick completes before the next tick) but targets a different execution model (interpreted bytecode with coroutine yield) and application domain (embedded scripting rather than high-assurance control). See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 1 for detailed analysis and citations.

### Keleusma and WebAssembly

| Feature | Keleusma | WebAssembly |
|---------|--------|------|
| Control flow | Block-structured, no flat jumps | Block-structured, no flat jumps |
| Validation | Single-pass structural | Single-pass structural |
| Type system | Nominal static types | Structural stack typing |
| Execution model | Coroutine yield/resume | Call/return |
| Target use case | Embedded scripting (audio, games) | Portable execution (web, cloud) |
| Streaming primitives | Stream, Yield, Reset | None |

**Rationale**: Keleusma adopts WebAssembly's insight that block-structured control flow enables efficient single-pass validation without constructing a full control flow graph. Both formats prohibit flat jumps for the same reason: verifiability. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 3 for detailed analysis and citations.

## 14. Termination and Productivity Guarantees

Keleusma provides static guarantees about script behavior through the three function categories.

### Recursion Prohibition

All forms of recursion are prohibited. The compiler rejects any cycle in the call graph among Keleusma functions. This includes direct recursion (function `a` calling itself) and mutual recursion (function `a` calling `b` which calls `a`). Recursive algorithms must be supplied by the host as registered native functions. At that point, termination guarantees rest on the host, not the Keleusma compiler.

The only form of "recursion" in Keleusma is the implicit re-entry of `loop` functions, which is not a function call but a coroutine iteration with a mandatory yield on every path.

### Atomic Functions (`fn`)

Must terminate. The compiler verifies:
- No `yield` expressions.
- No `loop` expressions (bare loops would diverge without yield).
- `for` loops iterate over fixed-size arrays or bounded ranges.
- No recursive calls (direct or mutual).

Assuming all native functions called within atomic functions are total (they return), atomic functions are guaranteed to terminate.

### Non-Atomic Total Functions (`yield`)

Must eventually exit. The compiler verifies:
- No bare `loop` expressions (would diverge).
- `for` loops iterate over bounded iterables.
- No recursive calls (direct or mutual).
- May contain `yield` expressions (suspending and resuming does not violate termination because each resume moves toward the function exit).

Assuming all called native functions return and the host resumes the coroutine after each yield, non-atomic total functions are guaranteed to eventually return.

### Productive Divergent Functions (`loop`)

Must yield on every iteration but never exit. The compiler verifies:
- The function body implicitly loops (restarts from the top).
- At least one `yield` expression exists on every execution path through the body.
- Only one `loop` function exists per script (the entry point).

The host is guaranteed to receive output on every coroutine step.

### Theoretical Foundations

The three function categories correspond to Turner's data/codata distinction [T1]: `fn` functions operate on finite data and must terminate (inductive), while `loop` functions produce infinite streams and must be productive (coinductive). The `yield` category bridges the two. The productivity invariant (every path from STREAM to RESET must pass through at least one YIELD) is a concrete instance of productivity for corecursive definitions as studied by Endrullis et al. [C4].

The productivity verification pass (`analyze_yield_coverage` in `src/verify.rs`) is an instance of abstract interpretation [AI1] over a two-element boolean lattice. The bounded-step execution property enables WCET analysis by counting weighted opcodes on the longest path between yield points [WC1]. See [RELATED_WORK.md](../reference/RELATED_WORK.md) for full citations and analysis.

### Formalization Status

The termination and productivity guarantees are enforced by the compiler (recursion prohibition, bounded loops, block type constraints) and the structural verifier (productivity rule verification). The productivity verification pass has been implemented and tested against both handwritten bytecode and compiled programs.

Formal verification of soundness (a machine-checked proof that the verifier correctly rejects all programs violating the stated properties) is pending. Watt's mechanized verification of WebAssembly [W2] provides a model for what such a proof would require. The current guarantees rely on the assumptions that (1) host-registered native functions are total, and (2) the host resumes the coroutine after each yield.

## 15. Error Propagation

Error propagation through `yield` is supported through the resume-value pattern (B7). The `yield` and `resume` cycle accepts any `Value` as the resumed input, so the script chooses an appropriate dialogue type. The script declares a `Result`-shaped enum (or any other variant union appropriate to the dialogue surface) and pattern-matches on the resumed value. The host signals errors by constructing the `Err` variant when calling `Vm::resume`. The convenience alias `Vm::resume_err` documents host intent without changing the underlying mechanism.

````
enum Reply { Ok(Word), Err }

loop main(input: Reply) -> Word {
    let reply = yield 0;
    match reply {
        Reply::Ok(v) => v,
        Reply::Err => -1,
    }
}
````

The host calls `vm.resume(Value::Enum { ... Ok ... })` for success and `vm.resume_err(Value::Enum { ... Err ... })` for failure. Both go through the same operand-stack mechanism. This pattern requires no special compiler or runtime support beyond the existing yield and resume cycle. See [`examples/yield_error.rs`](../../examples/yield_error.rs) for a runnable demonstration.

## 16. Resolved Design Decisions

The following questions from the initial specification have been resolved.

| Question | Resolution |
|----------|-----------|
| String interpolation | Removed in V0.2.0 Phase 3.5. The f-string surface form (`f"text {expr}"`) and its lexer-level desugaring to `concat` / `to_string` calls were retired. Hosts compose dynamic text through a registered `format` native that returns `Value::KStr`. |
| Array iteration | `for` loops iterate over arrays and bounded ranges. The compiler infers static array length from declared types and emits a `Const(N)` end bound for the strict-mode WCMU verifier. |
| Error propagation | Implemented as the resume-value pattern (B7). The script declares a `Result`-shaped enum and pattern-matches on the resumed value. No `Result<T, E>` syntactic sugar at the `yield` site; the dialogue type is the script's own enum. |
| Numeric literal type suffixes | Supported. `42Word`, `42Byte`, `42Float`, `42Fixed<16>`, `3.14Float`, and `3.14Fixed<16>` are valid; the `Byte` suffix is range-checked and `Fixed<N>` requires the fraction-bit count. The earlier `i64`/`f64` suffixes are removed. |
| Nested yield | Yield-propagating functions must share the caller's yield contract (same input and output types). Enforced via the `yield` function category keyword. |
| Block delimiters | Curly braces. Consistent with Rust host language. |
| Function categories | Three categories. `loop` for productive divergent, `yield` for non-atomic total, `fn` for atomic total. |
| Pure / impure | Host declares purity. Analysis trusts the declaration. Impurity is transitive. |
| Comment syntax | `//` line comments and `/* */` block comments. The lexer additionally skips a leading `#!` line so source scripts may carry a Unix shebang. |
| Statement termination | Semicolons required, as in Rust. Last expression in a block is the return value (no semicolon). |
| Recursion | Explicitly prohibited in `fn` and `yield` categories. Only `loop` functions admit cyclic execution, and only through the productive RESET cycle. Compiler rejects all call-graph cycles. |
| `for..in` finiteness | All host-provided iterable types are assumed finite by contract. The compiler checks that only iterable types are used. |
| `break` keyword | Allowed in `for` loops. Not allowed in `loop` functions (which must always yield). |
| Language classification | Total Functional Stream Processor. Without host-plugged functions, only pure functions that yield or exit can be defined. |
| Hindley-Milner inference | Implemented (B1) with Robinson unification and the occurs check. `Type::Var` represents inferred positions; substitution applies at end of `check_function`. |
| Generics with traits and bounds | Implemented (B2 / B2.3 / B2.4). Generic functions, structs, and enums with type parameters and trait bounds; impl-method registration with structural validation against the trait declaration. |
| Compile-time monomorphization | Implemented (B2.4). Specialization across literals, identifiers, function-call returns, method-call returns, casts, enum variants, struct constructions, tuple and array literals, if and match arms, field access, tuple-index, and array-index. Generic functions with no specializations are retained for runtime tag dispatch on `Value` tags. |
| Closures | Removed in V0.2.0 Phase 4. Closure-shaped expressions and first-class function references are rejected at the type-checker stage with a diagnostic that names the construct. The `Op::CallIndirect`, `Op::PushFunc`, `Op::MakeClosure`, and `Op::MakeRecursiveClosure` opcodes and the `Value::Func` runtime variant were retired alongside the surface form. |
| Hot code swap | Implemented at the reset boundary of a `loop` script through `Vm::replace_module`. Native registrations persist; the data segment is supplied fresh by the host. Dialogue type must remain stable across swaps. |
| Error recovery model | `Vm::reset_after_error` clears volatile state (operand stack, frames, arena top) and preserves the data segment. Hosts call this after `Err` from `call` or `resume` to return the VM to a clean callable state. |
| WCET unit | Pipelined cycles per Stream-to-Reset slice. The bundled `NOMINAL_COST_MODEL` is unmeasured and provides relative ordering only; hosts construct a calibrated `CostModel` whose `op_cycles` returns measured pipelined cycles for the target hardware (the `keleusma-bench` workspace member generates such tables). |
| WCMU unit | Bytes per Stream-to-Reset slice, separately for stack and heap regions. Both are summed and checked against the arena capacity at module load. |
| Conservative-verification stance | The compile pipeline admits a broader surface than the WCET and WCMU analyses can prove bounded. The safe constructor `Vm::new` is the source of truth for what loads. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification). |
| VM allocation model | Per-VM arena with a single-ownership contract. A shared arena across multiple `Vm` instances was considered and rejected as not-applicable; see B8 in BACKLOG. The arena itself is host-owned and borrowed by the VM under a lifetime parameter. |

## 17. Unresolved Questions

The remaining open questions concern target-application policy and ecosystem rather than language semantics.

1. **Tick granularity for audio embedders.** The CPU sample rate (typically 44.1 or 48 kHz) is too fast for per-sample scripting; per-buffer or per-beat subdivision is the realistic granularity. The SDL3 piano-roll example uses 16th-note ticks at 120 BPM (125 ms per tick). The host application chooses the granularity that fits its real-time deadline. No language change required.

2. **AArch64 cost-model calibration.** *Resolved.* The earlier bench output collapsed all categories to one cycle on AArch64 because `CNTVCT_EL0` ticks at the architectural counter frequency (24 MHz on Apple Silicon), not the CPU clock. The fix reads `CNTFRQ_EL0` at runtime and multiplies the counter delta by `assumed_cpu_hz / counter_hz` to convert ticks to CPU cycles. `keleusma-bench` exposes `--cpu-hz <Hz>` and the `KELEUSMA_BENCH_CPU_HZ` environment variable for the assumption; the default is documented in `keleusma-bench/src/counter.rs`. Pre-generated fragments are committed under [`keleusma-bench/measured_cost_models/`](../../keleusma-bench/measured_cost_models/) for `aarch64-apple-darwin` (M1 Max P-core nominal at 3.228 GHz) and `thumbv8m-main-none-eabihf` (STM32N6570-DK Cortex-M55 at 800 MHz, using DWT_CYCCNT which counts CPU cycles directly).

3. **Opaque-type runtime path.** The type checker tracks opaque types but the runtime cannot marshal them across the native boundary as themselves. Adding a `Value::Opaque` variant and a marshalling path is tracked for a future release; in the meantime, hosts pass opaque values as `Word` handles.

4. **Application-domain DSL conventions.** Audio engines, game scripting, embedded control loops, and industrial control each have their own register-fn vocabularies. Conventions for these domains will emerge from real adopters rather than from up-front specification.
