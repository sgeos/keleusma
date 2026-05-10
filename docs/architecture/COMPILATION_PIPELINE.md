# Compilation Pipeline

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma processes source text through a four-stage pipeline that transforms human-readable scripts into executable bytecode. Each stage produces a well-defined intermediate representation that the next stage consumes.

## Pipeline Diagram

```
Source Text
  -> tokenize()        -> Vec<Token>
  -> parse()           -> Program (AST)
  -> typecheck()       -> Program (validated)
  -> monomorphize()    -> Program (specialized)
  -> typecheck()       -> Program (re-validated under concrete types)
  -> hoist_closures()  -> Program (closures hoisted to top-level chunks)
  -> emit chunks       -> Module (bytecode)
  -> verify()          -> Module (structural + WCMU + WCET admissible)
  -> Vm::new()         -> Vm
  -> Vm::call()        -> VmState
```

The `compile()` entry point bundles the typecheck, monomorphize, hoist, and chunk emission steps. `Vm::new()` runs `verify()` and the resource-bounds check; `Vm::new_unchecked()` skips the bounds check while preserving structural verification.

## Stage 1: Lexer

**Source**: `src/lexer.rs`

**Public API**:
```rust
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError>
```

The lexer transforms source text into a flat sequence of tokens. It handles keywords, identifiers, numeric literals in decimal, hexadecimal, and binary formats, floating-point literals, and string literals with escape sequences.

Line comments beginning with `//` and block comments delimited by `/* */` are recognized and skipped. The lexer does not produce comment tokens.

Every token carries a `Span` value that records the source location, including line number, column number, and byte offset. Downstream stages use span information to produce error messages that reference the original source position.

## Stage 2: Parser

**Source**: `src/parser.rs`

**Public API**:
```rust
pub fn parse(tokens: &[Token]) -> Result<Program, ParseError>
```

The parser is a recursive descent parser that consumes the token sequence and produces an abstract syntax tree. The root AST node is `Program`, which contains use declarations, type definitions, and function definitions.

Operator precedence is handled through the standard recursive descent technique of layered parsing functions, where each precedence level delegates to the next higher level. The parser supports pattern matching in three contexts: function parameter heads, match expression arms, and let bindings.

## Stage 3: Compiler

**Source**: `src/compiler.rs`

**Public API**:
```rust
pub fn compile(program: &Program) -> Result<Module, CompileError>
pub fn compile_with_target(program: &Program, target: &Target) -> Result<Module, CompileError>
```

The compiler runs typecheck, then monomorphizes generic functions and structs/enums per concrete instantiation, then re-typechecks the specialized program, then hoists closure literals into top-level synthetic chunks, then emits each chunk's bytecode in a final pass. The constant pool is per-chunk and deduplicates identical values. Control flow constructs use forward jumps that are patched after the target offset is known.

`compile()` is a thin wrapper over `compile_with_target(program, &Target::host())`. The target descriptor is documented in [BACKLOG.md](../decisions/BACKLOG.md) under B10 and the source-level surface is `src/target.rs`. Programs that use features unsupported by the target (such as floating-point operations on a no-float target) are rejected at compile time.

Recursion detection in the direct-call graph is performed by the verifier (`verify::topological_call_order`) at module load, not at compile. The verifier rejects modules whose direct-call graph contains a cycle, enforcing the totality guarantees of the `fn` and `yield` function categories. Recursion through `Op::CallIndirect` (recursive closures) is rejected separately by `verify::module_wcmu` because the WCMU analysis cannot bound it. See Structural Verification in [EXECUTION_MODEL.md](./EXECUTION_MODEL.md). Guard clause validation runs at typecheck time and ensures that guard expressions are valid boolean expressions and that multiheaded functions have consistent parameter counts.

## Stage 4: Structural Verification

**Source**: `src/verify.rs`

**Public API**:
```rust
pub fn verify(module: &Module) -> Result<(), VerifyError>
pub fn wcet_stream_iteration(chunk: &Chunk) -> Result<u32, VerifyError>
```

The structural verifier validates compiled modules before they are loaded into the VM. It performs five passes per chunk.

1. **Block nesting.** Every If is matched by EndIf (with optional Else). Every Loop is matched by EndLoop. No orphaned delimiters.
2. **Offset validation.** All jump targets are within bounds and point to the correct matching delimiter.
3. **Block type constraints.** Func chunks contain no Yield, Stream, or Reset. Reentrant chunks contain at least one Yield and no Stream or Reset. Stream chunks contain exactly one Stream, exactly one Reset, and at least one Yield.
4. **Break containment.** Every Break and BreakIf is inside a Loop/EndLoop.
5. **Productivity rule.** Abstract interpretation over a two-element lattice verifies that all control flow paths from Stream to Reset pass through at least one Yield. The analysis handles If/Else/EndIf, If/EndIf (without Else), Loop/EndLoop, Break, and BreakIf using the same recursive block-structured traversal as the ISA itself.

The `wcet_stream_iteration()` function computes the worst-case execution cost of one Stream-to-Reset iteration. Each instruction carries a relative cost via `Op::cost()`. The analysis uses the same recursive traversal, taking the maximum cost branch at each control flow join, and returns the worst-case total as a unitless integer.

## Stage 5: Virtual Machine

**Source**: `src/vm.rs`

**Public API**:
```rust
impl<'a, 'arena> Vm<'a, 'arena> {
    pub fn new(module: Module, arena: &'arena Arena) -> Result<Self, VmError>;
    pub fn register_native(&mut self, name: &str, func: fn(&[Value]) -> Result<Value, VmError>);
    pub fn call(&mut self, args: &[Value]) -> Result<VmState, VmError>;
    pub fn resume(&mut self, input: Value) -> Result<VmState, VmError>;
    pub fn resume_err(&mut self, error_value: Value) -> Result<VmState, VmError>;
}
```

`Vm` carries two lifetimes: `'a` for borrowed bytecode (zero-copy execution from `&[u8]`) and `'arena` for the host-owned arena that backs the operand stack, call frames, and `KString` allocations.

The virtual machine is a stack-based interpreter that executes bytecode operations from a compiled module. It maintains a value stack, a call frame stack, and the registered native function table.

`Vm::new()` returns a `Result` because construction can fail if the module has no entry point or if native function bindings are inconsistent.

Execution begins when the host calls `call()`, which pushes an initial call frame for the module entry point. The VM executes instructions until it encounters a yield operation, a reset operation, or reaches the end of the entry function. The return value is wrapped in a `VmState` enum.

- `VmState::Yielded(Value)` indicates that the script has suspended and produced an output value. The host may call `resume(input)` to continue execution.
- `VmState::Finished(Value)` indicates that the script has completed and returned a final value.
- `VmState::Reset` indicates that a stream function has reached its Reset boundary. The host may call `resume(input)` to begin the next iteration.

Coroutine state, including the value stack, call frames, and instruction pointer, is preserved across yield boundaries. When the host calls `resume()`, execution continues from the exact point where the yield occurred.

## Error Types

Each pipeline stage defines its own error type.

| Error Type | Stage | Source Location |
|------------|-------|-----------------|
| `LexError` | Lexer | Includes `Span` with line, column, and byte offset |
| `ParseError` | Parser | Includes `Span` from the token that caused the error |
| `CompileError` | Compiler | Includes `Span` from the AST node that caused the error |
| `VerifyError` | Verifier | Includes chunk name and failure description |
| `VmError` | Virtual Machine | Runtime error without source location |

The first three error types carry span information that allows the host to produce human-readable error messages referencing the original source text. `VmError` is a runtime error and does not carry source location information because bytecode instructions do not retain span data.

## Module Structure

A compiled `Module` is the output of the compiler and the input to the virtual machine. It contains the following components.

- **Chunks.** A vector of `Chunk` values, each representing a compiled function. Every chunk contains its bytecode operations, a constant pool, struct templates for struct construction, the local variable count, the parameter count, and a `block_type` classification (Func, Reentrant, or Stream) that determines structural verification rules.
- **Entry point index.** The index into the chunk vector that identifies the main or loop function serving as the script entry point.
- **Native names.** The names of native functions declared via `use` statements, used to bind native function registrations at VM construction time.

## Typical Host Usage

The recommended pipeline for loading and executing a Keleusma script from the host is as follows.

```rust
let tokens = tokenize(source)?;
let program = parse(&tokens)?;
let module = compile(&program)?;
let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
let mut vm = Vm::new(module, &arena)?;
// Register native functions.
// Initialize data segment slots if the module declares a data block.
for (slot, value) in initial_values.iter().enumerate() {
    vm.set_data(slot, value.clone())?;
}
// Drive coroutine execution.
match vm.call(&[])? {
    VmState::Yielded(output) => { /* host processes output */ }
    VmState::Reset => { /* host may hot swap or resume */ }
    VmState::Finished(value) => { /* terminal result */ }
}
```

`Vm::new()` runs structural verification and the WCMU/WCET resource-bounds check; either failing returns a `VmError`. The data segment is allocated to match the declared layout slot count and zero-initialized to `Value::Unit`. The host calls `set_data` to populate slots before execution begins.

Native functions are registered via `Vm::register_fn` for the ergonomic typed registration that handles arity, type coercion, and return wrapping automatically. Argument and return types must implement `KeleusmaType`, which the `#[derive(KeleusmaType)]` macro provides for host structs and enums. Use `Vm::register_fn_fallible` when the host function returns `Result<R, VmError>`. The lower-level `register_native` and `register_native_closure` remain available for functions that must inspect arbitrary `Value` variants.

Hot code update is performed by calling `vm.replace_module(new_module, initial_data)` between a `VmState::Reset` and the next `call`. The new module is verified before replacement. The supplied data vector length must match the new module's declared slot count. After a successful swap, the host calls `call` to start the new module's entry point. The same mechanism supports rollback by replacing with an older module and an appropriate data instance.

See [TARGET_ISA.md](../reference/TARGET_ISA.md) for the complete structural ISA specification and [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) for the data segment specification.

## Cross-References

- [INSTRUCTION_SET.md](../reference/INSTRUCTION_SET.md) provides the complete bytecode instruction reference.
