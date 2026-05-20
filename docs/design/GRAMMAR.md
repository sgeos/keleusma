# Keleusma Language Grammar Specification

> **Navigation**: [Design](./README.md) | [Documentation Root](../README.md)

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

The following features were originally listed as out of scope and have since shipped under V0.1.

- Hindley-Milner type inference with Robinson unification, the occurs check, and a transitional `Type::Var` for inferred positions.
- Generic type parameters with trait bounds, traits, impl blocks, and compile-time monomorphization with inference reach across literals, identifiers, function-call returns, method-call returns, casts, enum variants, struct constructions, tuple and array literals, if and match arms, field access, tuple-index, and array-index.
- Closures with environment capture and transitive nested capture. Closures are rejected by the safe verifier under the conservative-verification stance because indirect dispatch through `Op::CallIndirect` cannot be statically bounded; programs that require definitive Worst-Case Execution Time and Worst-Case Memory Usage bounds restrict themselves to direct calls.
- Hot code swap at the reset boundary of a `loop` script. Native registrations persist across the swap; the data segment is supplied fresh by the host.
- String interpolation through f-strings (`f"text {expr}"`), which the lexer desugars to `concat` and `to_string` calls.

The following remains explicitly out of scope.

- Ownership, borrowing, or lifetime annotations at the surface language level. Rust's borrow checker is unnecessary because script values are conceptually immutable and the data segment is the sole mutable region.

Structural verification at the bytecode level is implemented. See [TARGET_ISA.md](../reference/TARGET_ISA.md) for the verification specification. The conservative-verification stance is described in [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification).

## 2. Lexical Structure

### Keywords

````
fn  yield  loop  break  let  for  in  if  else  match
use  struct  enum  newtype  trait  impl  data  true  false  as  when
not  and  or  pure  shared  private  const  ephemeral  where
overflow  underflow  saturate_max  saturate_min
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
float_lit     = [0-9]+ '.' [0-9]+ [ float_suffix ]
int_suffix    = 'i64'
float_suffix  = 'f64'
string_lit    = '"' ( [^"\\] | '\\' escape_char )* '"'
bool_lit      = 'true' | 'false'
escape_char   = 'n' | 't' | 'r' | '\\' | '"' | '0'
````

Integer literals support decimal, hexadecimal, and binary notation. Float literals require digits on both sides of the decimal point. String literals use double quotes with backslash escape sequences. Numeric literal suffixes (`42i64`, `3.14f64`) are supported for explicit typing.

### Operators

| Category | Operators |
|----------|-----------|
| Arithmetic | `+`, `-`, `*`, `/`, `%` |
| Comparison | `==`, `!=`, `<`, `>`, `<=`, `>=` |
| Logical | `and`, `or`, `not` |
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

Type-system support is partial in V0.1.x. The type checker tracks an opaque type for every named type that is not declared as a struct or enum, so source code that mentions an opaque type compiles and type-checks. The runtime path is incomplete: there is no `Value::Opaque` variant and the `KeleusmaType` derive does not produce marshalling code for opaque types, so a host cannot pass a domain-specific Rust value through the native boundary as itself today. The intended pattern in V0.1.x is to pass opaque values through a primitive handle (typically `Word`) that the host translates to and from its real type at the native function boundary.

````
// ChannelHandle is documented as opaque at the type-system level.
// In V0.1.x the host marshals the handle as Word across the native
// boundary and translates internally.
let ch: Word = audio::get_channel(0);
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

The cast respects both implicit and explicit discriminants. The reverse direction (a `Word` cast back to an enum) is not currently admissible; construct enum values with the variant syntax.

String conversion uses the `to_string` native function.

## 4. Expressions

### Arithmetic Expressions

Standard arithmetic with operator precedence. Multiplication, division, and modulo bind tighter than addition and subtraction. Parentheses override precedence.

````
let result = (a + b) * c / d - e % f;
````

Integer arithmetic (`Word`) and floating-point arithmetic (`Float`) do not mix without explicit `as` casts.

### Comparison and Logical Expressions

Comparison operators produce `bool`. Logical operators `and` and `or` short-circuit. `not` is a prefix operator.

````
if x > 0 and not done {
  process(x);
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
    audio::set_mod_depth(ch, i, 0.5f64);
  }
  AudioAction::SetChannelParam(ch, "ready", 1.0)
}
````

### 6.5 Multiheaded Functions

Multiple function definitions with the same name, arity, and category form a single logical function. The runtime dispatches to the first head whose pattern matches the arguments, evaluated top to bottom.

````
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

Guards a single arithmetic operation against overflow and underflow. The operation may be `+`, `-`, `*`, `/`, `%`, or unary `-` on Word operands. The runtime computes the true result in `i128` and pushes `(high, low, flag)`; arm patterns destructure the high and low halves so big-number arithmetic can chain through successive checked operations. Each outcome class (`ok`, `overflow`, `underflow`) must have at least one arm, and the last covering arm per class must be an unguarded catch-all (bare identifier or wildcard in every position). Patterns are admitted from a restricted subset (wildcard, variable, signed integer literal); an optional `when expr` guard between the pattern and the `=>` is checked as `Bool` and falls through to the next arm when false.

The `saturate_max` and `saturate_min` keywords inside arm bodies denote context-determined saturation values. When the surrounding expected type is `Word`, they resolve to `Word::MAX` and `Word::MIN` respectively. When the surrounding expected type is a refined newtype declared with a `with saturate_max = N` or `with saturate_min = M` clause, the keyword resolves to a constructor call against that literal.

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

### Information-Flow Labels

````
labelled_type = type_expr_inner '@' label_spec
label_spec    = upper_ident | '{' upper_ident { ',' upper_ident } '}'
classify_expr = 'classify' postfix_expr '@' label_spec
declassify_expr = 'declassify' postfix_expr '@' label_spec
````

Types carry a set of user-defined information-flow labels written as `T@Label` for a single label or `T@{L1, L2}` for multiple. The empty label set is the pure state. The `classify` operator adds labels to a value; `declassify` removes them. Labels propagate through arithmetic operations, comparisons, conditional branches, and composite-type positions (tuple elements, array elements, option payloads). The label-flow rule at every position is `source.labels ⊆ target.labels`; violations are rejected at compile time. The mechanism is zero-cost at the bytecode layer.

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

## 8. Pattern Matching

Patterns appear in function heads, `match` arms, and `let` bindings.

### Pattern Forms

| Pattern | Example | Matches |
|---------|---------|---------|
| Literal | `42`, `"hello"`, `true` | Exact value |
| Enum variant | `Command::NoteOn(ch, note, vel)` | Variant with bindings |
| Enum unit variant | `Command::Silence` | Variant without data |
| Struct destructuring | `Note { channel, pitch }` | Struct with field bindings |
| Tuple destructuring | `(a, b)` | Tuple with element bindings |
| Wildcard | `_` | Any value (ignored) |
| Variable | `x` | Any value (bound to name) |
| Option Some | `Option::Some(value)` | Non-None optional |
| Option None | `Option::None` | None optional |

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

Opaque type support is partial in V0.1.x. The type checker tracks opaque types correctly and the compiler accepts them in parameter and return positions, but the marshalling layer does not yet have a path for opaque host values to flow across the native boundary as themselves. The recommended pattern for V0.1.x is to pass opaque host values through a primitive handle (typically `Word`) that the host translates to and from its real Rust type at the native function boundary.

````
let ch: Word = audio::get_channel(0);
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
use_decl        = 'use' module_path '::' ( lower_ident | '*' )
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
type_params     = '<' type_param { ',' type_param } '>'
type_param      = upper_ident [ ':' trait_bound_list ]
trait_bound_list = upper_ident { '+' upper_ident }

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
prim_type       = 'Word' | 'Float' | 'bool' | 'Text' | '(' ')'
named_type      = upper_ident [ '<' type_expr { ',' type_expr } '>' ]
tuple_type      = '(' type_expr ',' type_expr { ',' type_expr } ')'
array_type      = '[' type_expr ';' integer_lit ']'
option_type     = 'Option' '<' type_expr '>'

(* Functions *)
(* The `ephemeral` modifier is permitted only on the entry point. *)
function_def    = [ 'ephemeral' ] (fn_def | yield_def | loop_def)
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
statement       = let_stmt | for_stmt | break_stmt
                | data_field_assign | data_field_index_assign
                | expr_stmt
let_stmt        = 'let' pattern [ ':' type_expr ] '=' expression ';'
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
logical_expr    = comparison_expr { ('and' | 'or') comparison_expr }
comparison_expr = additive_expr [ comparison_op additive_expr ]
additive_expr   = multiplicative_expr { ('+' | '-') multiplicative_expr }
multiplicative_expr = unary_expr { ('*' | '/' | '%') unary_expr }
unary_expr      = [ 'not' | '-' ] postfix_expr
postfix_expr    = primary_expr { method_call | field_access | tuple_index | array_index }
method_call     = '.' lower_ident '(' [ arg_list ] ')'
field_access    = '.' lower_ident
tuple_index     = '.' integer_lit
array_index     = '[' expression ']'
primary_expr    = literal
                | fstring_lit
                | lower_ident
                | upper_ident '::' upper_ident [ '(' [ arg_list ] ')' ]
                | upper_ident [ '{' field_init_list '}' ]
                | function_call
                | yield_expr
                | if_expr
                | match_expr
                | closure_expr
                | '(' ')'
                | '(' expression ')'
                | '(' expression ',' expression { ',' expression } [ ',' ] ')'
                | '[' [ arg_list ] ']'
                | expression 'as' type_expr

(* Closures. Body may be a single expression or a block. *)
closure_expr    = '|' [ closure_params ] '|' [ '->' type_expr ] closure_body
closure_params  = closure_param { ',' closure_param }
closure_param   = lower_ident [ ':' type_expr ]
closure_body    = expression
                | '{' block '}'

literal         = integer_lit | float_lit | string_lit | bool_lit
qualified_name  = lower_ident { '::' lower_ident }
function_call   = qualified_name '(' [ arg_list ] ')'
arg_list        = expression { ',' expression }
                | expression { ',' expression } ',' '_'
                | '_' { ',' expression }
field_init_list = field_init { ',' field_init }
field_init      = lower_ident ':' expression

(* F-strings. Lexer-level desugaring to a chain of `concat` and
   `to_string` native calls. The literal segments are static strings;
   the interpolated segments are arbitrary expressions. *)
fstring_lit     = 'f"' { fstring_segment } '"'
fstring_segment = fstring_text | '{' expression '}'
fstring_text    = ? any character except '"' or '{' ?

(* Yield *)
yield_expr      = 'yield' expression

(* If / Else *)
if_expr         = 'if' expression '{' block '}' [ 'else' '{' block '}' ]

(* Match *)
match_expr      = 'match' expression '{' { match_arm } '}'
match_arm       = pattern '=>' expression ','

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

The grammar is descriptive, not normative. The reference implementation is the parser at [`src/parser.rs`](../../src/parser.rs); when the two disagree, the parser wins and the EBNF should be updated.

The grammar describes the surface only. The verifier rejects programs whose Worst-Case Execution Time or Worst-Case Memory Usage cannot be statically bounded under the conservative-verification stance, and the type checker enforces additional constraints around generics, trait bounds, exhaustive match, and the data-segment fixed-size discipline. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md) for those layers.

Closures parse but the safe verifier rejects programs that invoke them through `Op::CallIndirect`. This is a deliberate property of the conservative-verification stance, not a parser limitation. Programs that require definitive Worst-Case Execution Time and Worst-Case Memory Usage bounds restrict themselves to direct calls.

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
    audio::set_mod_depth(ch, i, 0.5f64);
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
  if turn == 1i64 {
    ScriptAction::DisplayMessage("Narrator", "Welcome aboard the generation ship.")
  } else {
    ScriptAction::NoAction
  }
}

yield handle(GameEvent::DeploymentWarning(turns_until)) -> ScriptAction when turns_until <= 3i64 {
  ScriptAction::DisplayMessage("Commander", "Deployment imminent. Prepare for combat.")
}

yield handle(GameEvent::DeploymentWarning(turns_until)) -> ScriptAction {
  ScriptAction::NoAction
}

yield handle(GameEvent::CharacterDeath(char_id, name)) -> ScriptAction {
  let relationship = game::get_friendliness(0i64, char_id);
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
| Certification status | Design aspiration | DO-178C TQL-1 qualified (SCADE KCG) |
| Concurrency | Single coroutine | Concurrent composition (Esterel) |
| Memory model | Arena (bump allocation, cleared at RESET) | Static allocation |
| Host interaction | Bidirectional typed yield | Sensor/actuator interface |

**Rationale**: Keleusma applies synchronous language principles to an embeddable bytecode VM for game and audio scripting. It shares the synchronous hypothesis (all computation within a logical tick completes before the next tick) but targets a different execution model (interpreted bytecode with coroutine yield) and application domain (embedded scripting rather than safety-critical control). See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 1 for detailed analysis and citations.

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
| String interpolation | Implemented as f-string desugaring. `f"text {expr}"` compiles to `concat` and `to_string` calls at lex time. |
| Array iteration | `for` loops iterate over arrays and bounded ranges. The compiler infers static array length from declared types and emits a `Const(N)` end bound for the strict-mode WCMU verifier. |
| Error propagation | Implemented as the resume-value pattern (B7). The script declares a `Result`-shaped enum and pattern-matches on the resumed value. No `Result<T, E>` syntactic sugar at the `yield` site; the dialogue type is the script's own enum. |
| Numeric literal suffixes | Supported. `42i64` and `3.14f64` are valid. |
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
| Closures | Implemented (B3) with environment capture and transitive nested capture. Rejected by the safe verifier under the conservative-verification stance because indirect dispatch through `Op::CallIndirect` cannot be statically bounded. The construct exists in the language so the rejection can be precise rather than approximate. |
| Hot code swap | Implemented at the reset boundary of a `loop` script through `Vm::replace_module`. Native registrations persist; the data segment is supplied fresh by the host. Dialogue type must remain stable across swaps. |
| Error recovery model | `Vm::reset_after_error` clears volatile state (operand stack, frames, arena top) and preserves the data segment. Hosts call this after `Err` from `call` or `resume` to return the VM to a clean callable state. |
| WCET unit | Pipelined cycles per Stream-to-Reset slice. The bundled `NOMINAL_COST_MODEL` is unmeasured and provides relative ordering only; hosts construct a calibrated `CostModel` whose `op_cycles` returns measured pipelined cycles for the target hardware (the `keleusma-bench` workspace member generates such tables). |
| WCMU unit | Bytes per Stream-to-Reset slice, separately for stack and heap regions. Both are summed and checked against the arena capacity at module load. |
| Conservative-verification stance | The compile pipeline admits a broader surface than the WCET and WCMU analyses can prove bounded. The safe constructor `Vm::new` is the source of truth for what loads. See [LANGUAGE_DESIGN.md](../architecture/LANGUAGE_DESIGN.md#conservative-verification). |
| VM allocation model | Per-VM arena with a single-ownership contract. A shared arena across multiple `Vm` instances was considered and rejected as not-applicable; see B8 in BACKLOG. The arena itself is host-owned and borrowed by the VM under a lifetime parameter. |

## 17. Unresolved Questions

The remaining open questions concern target-application policy and ecosystem rather than language semantics.

1. **Tick granularity for audio embedders.** The CPU sample rate (typically 44.1 or 48 kHz) is too fast for per-sample scripting; per-buffer or per-beat subdivision is the realistic granularity. The SDL3 piano-roll example uses 16th-note ticks at 120 BPM (125 ms per tick). The host application chooses the granularity that fits its real-time deadline. No language change required.

2. **AArch64 cost-model calibration.** The `keleusma-bench` calibration tool produces useful measurements on x86 (RDTSC) and degenerate "everything is one cycle" output on AArch64 because the architectural counter `CNTVCT_EL0` runs at the system counter frequency, not the CPU clock. The fix is either reading `CNTFRQ_EL0` and converting through CPU frequency, or using `PMCCNTR_EL0` (which requires kernel-level enable). Tracked in the backlog.

3. **Opaque-type runtime path.** The type checker tracks opaque types but the runtime cannot marshal them across the native boundary as themselves. Adding a `Value::Opaque` variant and a marshalling path is tracked for a future release; in the meantime, hosts pass opaque values as `Word` handles.

4. **Application-domain DSL conventions.** Audio engines, game scripting, embedded control loops, and UAV mission control each have their own register-fn vocabularies. Conventions for these domains will emerge from real adopters rather than from up-front specification.
