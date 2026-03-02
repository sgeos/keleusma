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

### Scope Exclusions

The following features are explicitly out of scope for the initial specification.

- Hindley-Milner type inference
- Ownership, borrowing, or lifetime annotations
- Traits or generic type parameters
- Closures or anonymous functions
- Hot-swap mechanism
- Formal verification at bytecode level
- String interpolation (not needed for a control language)

## 2. Lexical Structure

### Keywords

````
fn  yield  loop  break  let  for  in  if  else  match
use  struct  enum  true  false  as  when  not  and  or  pure
````

All keywords are reserved and cannot be used as identifiers.

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
fn greet(name: String) -> String {
  "Hello, " + name
}
````

### Whitespace and Semicolons

Whitespace (spaces, tabs, newlines) is not significant except as a token separator. Semicolons terminate statements, as in Rust. Multiple statements may appear on one line. The last expression in a block is the return value and does not require a trailing semicolon.

## 3. Type System

### Primitive Types

| Type | Description | Rust Equivalent |
|------|-------------|-----------------|
| `i64` | 64-bit signed integer | `i64` |
| `f64` | 64-bit floating point | `f64` |
| `bool` | Boolean value | `bool` |
| `String` | UTF-8 string | `String` |
| `()` | Unit type | `()` |

All numeric operations use `i64` or `f64`. Smaller integer types (`u8`, `u32`) from host structs are widened to `i64` when accessed in Keleusma. Native function bindings handle the narrowing conversion at the boundary.

### Composite Types

**Structs**: Named product types with named fields.

````
struct Note {
  channel: i64,
  pitch: i64,
  velocity: f64,
}
````

**Enums**: Named sum types with variants. Variants may carry data.

````
enum Command {
  NoteOn(i64, i64, f64),
  NoteOff(i64),
  SetTempo(f64),
  Silence,
}
````

**Tuples**: Anonymous product types.

````
let pair: (i64, f64) = (42, 3.14);
````

**Fixed-size arrays**: Homogeneous sequences of known length.

````
let channels: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
````

**Optionals**: Nullable values using Option.

````
let maybe_name: Option<String> = Option::Some("Alice");
let nothing: Option<String> = Option::None;
````

### Opaque Types

Host-registered Rust types that scripts can pass through function calls but cannot destructure or inspect. Opaque types appear as `upper_ident` in type annotations. The compiler recognizes them from the native function registry.

````
// ChannelHandle is opaque; scripts can only pass it to native functions.
let ch: ChannelHandle = audio::get_channel(0);
audio::set_frequency(ch, 440.0);
````

### Type Coercion

No implicit type coercion exists. Numeric conversion requires the `as` keyword.

````
let x: i64 = 42;
let y: f64 = x as f64;
let z: i64 = y as i64;    // Truncates toward zero.
````

String conversion uses the `to_string` native function.

## 4. Expressions

### Arithmetic Expressions

Standard arithmetic with operator precedence. Multiplication, division, and modulo bind tighter than addition and subtraction. Parentheses override precedence.

````
let result = (a + b) * c / d - e % f;
````

Integer arithmetic (`i64`) and floating-point arithmetic (`f64`) do not mix without explicit `as` casts.

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
````

Array indexing uses `i64` indices. Out-of-bounds access causes the script to yield a runtime error to the host.

## 5. Statements

### Variable Binding

````
let x: i64 = 42;
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
fn add(a: i64, b: i64) -> i64 {
  a + b
}
````

The last expression in a function body is the return value. There is no `return` keyword.

Atomic functions may call other atomic functions. They may not call `loop` or `yield` functions.

### 6.2 Non-Atomic Total Functions (`yield`)

Non-atomic total functions may yield to the host and must eventually exit, assuming all called native functions return. They are declared with the `yield` keyword instead of `fn`.

````
yield configure_channel(ch: i64, cmd: AudioCommand) -> AudioAction {
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
fn describe(Command::NoteOn(ch, note, vel)) -> String {
  "Play note " + (note as f64 |> to_string()) + " on channel " + (ch as f64 |> to_string())
}

fn describe(Command::NoteOff(ch)) -> String {
  "Stop channel " + (ch as f64 |> to_string())
}

fn describe(Command::SetTempo(bpm)) -> String {
  "Tempo: " + (bpm |> to_string())
}

fn describe(Command::Silence) -> String {
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
fn severity(level: f64) -> String when level >= 0.9 {
  "critical"
}

fn severity(level: f64) -> String when level >= 0.5 {
  "warning"
}

fn severity(level: f64) -> String {
  "normal"
}
````

Guard expressions are limited to comparison operators, logical operators, arithmetic, and field access. Guard expressions must not call functions (to preserve deterministic dispatch ordering).

## 7. Pure and Impure Functions

Native functions registered by the host must be declared as either pure or impure.

### Pure Functions

Pure functions are declared by the host to be deterministic functions without side effects. The compiler trusts this declaration for the purposes of analysis and optimization.

````rust
// Rust host code (not Keleusma syntax).
vm.register_pure_fn("math::lerp", &[Type::F64, Type::F64, Type::F64], Type::F64);
````

A Keleusma function that calls only pure native functions and other pure Keleusma functions is itself pure.

### Impure Functions

Any routine may call an impure function. Any routine that calls an impure function is itself impure. Impurity is transitive.

````rust
// Rust host code (not Keleusma syntax).
vm.register_fn("audio::set_frequency", &[Type::Opaque("ChannelHandle"), Type::F64], Type::Unit);
````

### Purity as a Host Declaration

Purity is a declaration from the host, not verified by the Keleusma compiler. Analysis trusts the declaration. Certain guarantees (such as deterministic replay and optimization) will be invalid if the declaration is false.

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

The host application registers native functions before compiling scripts. Each registration provides the function name, parameter types, return type, and purity.

````rust
// Rust host code (not Keleusma syntax).
vm.register_pure_fn("math::lerp", &[Type::F64, Type::F64, Type::F64], Type::F64);
vm.register_fn("audio::set_frequency", &[Type::Opaque("ChannelHandle"), Type::F64], Type::Unit);
vm.register_fn("audio::play_note", &[Type::I64, Type::I64, Type::F64], Type::Unit);
vm.register_pure_fn("game::get_relationship", &[Type::I64, Type::I64], Type::F64);
````

### Type Validation

The compiler validates all native function calls against the registry at compile time. Calling an unregistered function or passing arguments of the wrong type produces a compile error.

### Opaque Types

Opaque types represent host-side Rust values that scripts can receive and pass to other native functions but cannot inspect, destructure, or create.

````
let ch: ChannelHandle = audio::get_channel(0);
audio::set_frequency(ch, 440.0);
// ch.internal_field    // ERROR: Cannot access fields of opaque type.
````

## 10. Module System

Each script file constitutes one module. Modules cannot import other Keleusma modules. All external functionality comes from native function registrations.

````
// audio_track.kma
use audio::*;

loop main(cmd: AudioCommand) -> AudioAction {
  let cmd = yield process(cmd);
}
````

File extension: `.kma`

## 11. Formal Grammar (EBNF)

````
program         = { use_decl } { type_def } { function_def }

(* Imports *)
use_decl        = 'use' module_path '::' ( lower_ident | '*' )
module_path     = lower_ident { '::' lower_ident }

(* Type Definitions *)
type_def        = struct_def | enum_def
struct_def      = 'struct' upper_ident '{' { field_decl } '}'
field_decl      = lower_ident ':' type_expr
enum_def        = 'enum' upper_ident '{' { variant_decl } '}'
variant_decl    = upper_ident [ '(' type_list ')' ]
type_list       = type_expr { ',' type_expr }

(* Types *)
type_expr       = prim_type | upper_ident | tuple_type | array_type | option_type
prim_type       = 'i64' | 'f64' | 'bool' | 'String' | '(' ')'
tuple_type      = '(' type_expr ',' type_expr { ',' type_expr } ')'
array_type      = '[' type_expr ';' integer_lit ']'
option_type     = 'Option' '<' type_expr '>'

(* Functions *)
function_def    = fn_def | yield_def | loop_def
fn_def          = 'fn' lower_ident '(' [ param_list ] ')' '->' type_expr
                  [ 'when' guard_expr ] '{' block '}'
yield_def       = 'yield' lower_ident '(' [ param_list ] ')' '->' type_expr
                  [ 'when' guard_expr ] '{' block '}'
loop_def        = 'loop' lower_ident '(' [ param_list ] ')' '->' type_expr
                  [ 'when' guard_expr ] '{' block '}'
param_list      = param { ',' param }
param           = pattern ':' type_expr
                | pattern

(* Guard expressions: restricted to comparisons and logic *)
guard_expr      = guard_term { ('and' | 'or') guard_term }
guard_term      = [ 'not' ] guard_atom
guard_atom      = expr comparison_op expr
                | '(' guard_expr ')'
comparison_op   = '==' | '!=' | '<' | '>' | '<=' | '>='

(* Blocks and Statements *)
block           = { statement } [ expression ]
statement       = let_stmt | for_stmt | break_stmt | expr_stmt
let_stmt        = 'let' pattern [ ':' type_expr ] '=' expression ';'
for_stmt        = 'for' lower_ident 'in' iterable '{' block '}'
iterable        = expression
                | expression '..' expression
break_stmt      = 'break' ';'
expr_stmt       = expression ';'

(* Expressions *)
expression      = pipeline_expr
pipeline_expr   = logical_expr { '|>' function_call }
logical_expr    = comparison_expr { ('and' | 'or') comparison_expr }
comparison_expr = additive_expr [ comparison_op additive_expr ]
additive_expr   = multiplicative_expr { ('+' | '-') multiplicative_expr }
multiplicative_expr = unary_expr { ('*' | '/' | '%') unary_expr }
unary_expr      = [ 'not' | '-' ] postfix_expr
postfix_expr    = primary_expr { '.' lower_ident | '.' integer_lit | '[' expression ']' }
primary_expr    = literal
                | lower_ident
                | upper_ident '::' upper_ident [ '(' [ arg_list ] ')' ]
                | upper_ident '{' field_init_list '}'
                | function_call
                | yield_expr
                | if_expr
                | match_expr
                | loop_expr
                | '(' expression ')'
                | '[' [ arg_list ] ']'
                | expression 'as' type_expr

literal         = integer_lit | float_lit | string_lit | bool_lit
function_call   = lower_ident '(' [ arg_list ] ')'
arg_list        = expression { ',' expression }
                | expression { ',' expression } ',' '_'
                | '_' { ',' expression }
field_init_list = field_init { ',' field_init }
field_init      = lower_ident ':' expression

(* Yield *)
yield_expr      = 'yield' expression

(* If/Else *)
if_expr         = 'if' expression '{' block '}' [ 'else' '{' block '}' ]

(* Match *)
match_expr      = 'match' expression '{' { match_arm } '}'
match_arm       = pattern '=>' expression

(* Loop expression - only valid inside loop functions *)
loop_expr       = 'loop' '{' block '}'

(* For Loop *)
for_expr        = 'for' lower_ident 'in' iterable '{' block '}'

(* Patterns *)
pattern         = literal_pattern
                | enum_pattern
                | struct_pattern
                | tuple_pattern
                | wildcard_pattern
                | variable_pattern
literal_pattern = integer_lit | float_lit | string_lit | bool_lit
enum_pattern    = upper_ident '::' upper_ident [ '(' pattern_list ')' ]
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
integer_lit     = ( [0-9]+ [ 'i64' ] ) | '0x' [0-9a-fA-F]+ | '0b' [01]+
float_lit       = [0-9]+ '.' [0-9]+ [ 'f64' ]
string_lit      = '"' { string_char } '"'
string_char     = [^"\\] | '\\' escape_char
escape_char     = 'n' | 't' | 'r' | '\\' | '"' | '0'
bool_lit        = 'true' | 'false'
line_comment    = '//' { any_char } newline
block_comment   = '/*' { any_char } '*/'
````

## 12. Example Programs

### Audio DSL: MS-20 Channel Controller

This script receives audio commands per tick and yields synthesis configuration actions.

````
use audio::*;

enum AudioCommand {
  NoteOn(i64, i64, f64),
  NoteOff(i64),
  SetTempo(f64),
  ConfigureChannel(i64),
  Tick,
}

enum AudioAction {
  PlayNote(i64, i64, f64),
  StopNote(i64),
  SetChannelParam(i64, String, f64),
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

fn configure_vco(ch: i64, vco_id: i64, waveform: String, amplitude: f64) -> () {
  audio::set_vco_waveform(ch, vco_id, waveform);
  audio::set_vco_amplitude(ch, vco_id, amplitude);
}

fn configure_adsr(ch: i64, a: f64, d: f64, s: f64, r: f64) -> () {
  audio::set_envelope(ch, "eg2", a, d, s, r);
}

fn configure_filter(ch: i64, filter_type: String, cutoff: f64, resonance: f64) -> () {
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
  TurnStart(i64, String),
  DeploymentWarning(i64),
  CharacterDeath(i64, String),
  BombDetonation(i64, i64),
  DateCompleted(i64, i64, f64),
  Idle,
}

enum ScriptAction {
  DisplayMessage(String, String),
  ModifyRelationship(i64, i64, f64),
  TriggerEvent(String),
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

### Formalization Status

The termination guarantees described above represent the intended design. Formal verification of these properties is pending further specification by the project owner. The current guarantees rely on the assumptions that (1) host-registered native functions are total, and (2) the host resumes the coroutine after each yield.

## 15. Error Propagation

Error propagation through yield is an optional feature. If implemented, `yield` returns `Result<T, E>` where `T` is the normal input type and `E` is an error type. This allows the host to signal errors to the script.

````
yield configure(ch: i64, cmd: AudioCommand) -> Result<AudioAction, String> {
  let result = yield AudioAction::SetChannelParam(ch, "vco", 1.0);
  match result {
    Result::Ok(cmd) => process(cmd),
    Result::Err(msg) => AudioAction::NoOp,
  }
}
````

This feature adds complexity to the type system and VM. If implementation proves difficult, it will be deferred to the feature backlog. Scripts can use sentinel values or Option types as an alternative error signaling mechanism.

## 16. Resolved Design Decisions

The following questions from the initial specification have been resolved by the project owner.

| Question | Resolution |
|----------|-----------|
| String interpolation | Not needed. Keleusma is a control language. |
| Array iteration | `for` loops iterate over arrays and ranges. Any iterable that can be guaranteed to terminate is acceptable. |
| Error propagation | Acceptable but optional. Implies `yield` returns `Result<T, E>`. Can be deferred if complicated. |
| Numeric literal suffixes | Supported. `42i64` and `3.14f64` are valid. |
| Nested yield | Yield-propagating functions must share the caller's yield contract (same input and output types). Enforced via the `yield` function category keyword. |
| Block delimiters | Curly braces `{ }` instead of `do`/`end`. Consistent with Rust host language. |
| Function categories | Three categories: `loop` (productive divergent), `yield` (non-atomic total), `fn` (atomic total). |
| Pure/impure | Host declares purity. Analysis trusts the declaration. Impurity is transitive. |
| Comment syntax | `//` line comments and `/* */` block comments. Consistent with Rust host language. |
| Statement termination | Semicolons required, as in Rust. Last expression in a block is the return value (no semicolon). |
| Recursion | Explicitly prohibited. The compiler rejects all call graph cycles. Recursive algorithms must be supplied by the host. |
| `for..in` finiteness | All host-provided iterable types are assumed finite by contract. The compiler checks that only iterable types are used. The host is responsible for not providing infinite iterators. |
| `break` keyword | Allowed in `for` loops. Not allowed in `loop` functions (which must always yield). |
| Language classification | Total Functional Stream Processor. Without host-plugged functions, only pure functions that yield or exit can be defined. |

## 17. Unresolved Questions

1. **Audio tick granularity**: What is the update rate for the audio DSL? The 48kHz sample rate is too fast for per-sample scripting. A reasonable granularity would be per-buffer (every 256-1024 samples, giving 47-187 Hz update rate) or per-beat subdivision.

2. **VM allocation model**: Should the VM allocate per-script or share an arena across all active scripts?

3. **Error recovery model**: What should happen when a script encounters a runtime error during audio rendering? A panic could cause audio glitches. Options include yielding a default value, suspending the script, or notifying the host.

4. **Game DSL state access**: Should game scripts have read access to the full GameState or a restricted view?
