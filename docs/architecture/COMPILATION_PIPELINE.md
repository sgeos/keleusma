# Compilation Pipeline

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma processes source text through a four-stage pipeline that transforms human-readable scripts into executable bytecode. Each stage produces a well-defined intermediate representation that the next stage consumes.

## Pipeline Diagram

```
Source Text -> tokenize() -> Vec<Token> -> parse() -> Program (AST) -> compile() -> Module (Bytecode) -> Vm::call() -> VmState
```

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
```

The compiler transforms the AST into a bytecode module in two passes. The first pass collects all function definitions and builds a mapping from function names to indices. The second pass compiles each function body into a sequence of bytecode operations.

The compiler maintains a constant pool with deduplication so that identical constant values share a single pool entry. Control flow constructs such as if-else, match, and for loops use forward jumps that are patched after the target offset is known.

Recursion detection is performed during compilation. The compiler builds a call graph and rejects any program that contains call cycles, enforcing the totality guarantees of the `fn` and `yield` function categories. Guard clause validation ensures that guard expressions are valid boolean expressions and that multiheaded functions have consistent parameter counts.

## Stage 4: Virtual Machine

**Source**: `src/vm.rs`

**Public API**:
```rust
impl Vm {
    pub fn new(module: Module) -> Self;
    pub fn register_native(&mut self, name: &str, func: fn(&[Value]) -> Result<Value, VmError>);
    pub fn call(&mut self, args: &[Value]) -> Result<VmState, VmError>;
    pub fn resume(&mut self, input: Value) -> Result<VmState, VmError>;
}
```

The virtual machine is a stack-based interpreter that executes bytecode operations from a compiled module. It maintains a value stack, a call frame stack, and the registered native function table.

Execution begins when the host calls `call()`, which pushes an initial call frame for the module entry point. The VM executes instructions until it encounters a yield operation or reaches the end of the entry function. The return value is wrapped in a `VmState` enum.

- `VmState::Yielded(Value)` indicates that the script has suspended and produced an output value. The host may call `resume(input)` to continue execution.
- `VmState::Finished(Value)` indicates that the script has completed and returned a final value.

Coroutine state, including the value stack, call frames, and instruction pointer, is preserved across yield boundaries. When the host calls `resume()`, execution continues from the exact point where the yield occurred.

## Error Types

Each pipeline stage defines its own error type.

| Error Type | Stage | Source Location |
|------------|-------|-----------------|
| `LexError` | Lexer | Includes `Span` with line, column, and byte offset |
| `ParseError` | Parser | Includes `Span` from the token that caused the error |
| `CompileError` | Compiler | Includes `Span` from the AST node that caused the error |
| `VmError` | Virtual Machine | Runtime error without source location |

The first three error types carry span information that allows the host to produce human-readable error messages referencing the original source text. `VmError` is a runtime error and does not carry source location information because bytecode instructions do not retain span data.

## Module Structure

A compiled `Module` is the output of the compiler and the input to the virtual machine. It contains the following components.

- **Chunks.** A vector of `Chunk` values, each representing a compiled function. Every chunk contains its bytecode operations, a constant pool, struct templates for struct construction, the local variable count, the parameter count, and an `is_loop` flag indicating whether the function is a loop entry point.
- **Entry point index.** The index into the chunk vector that identifies the loop function serving as the script entry point.
- **Enum definitions.** A table of enum type definitions used for variant construction and pattern matching at runtime.

## Structural Verification

The structural ISA introduces verification rules that validate programs at load time. Block-based nesting ensures that invalid or unproductive programs cannot be loaded. The verifier performs a linear scan that colors blocks based on productivity. All paths from STREAM to RESET must pass through at least one YIELD. All FUNC blocks must be free of yields. Implementation of structural verification is in progress alongside the ISA transition (R21). See [TARGET_ISA.md](../reference/TARGET_ISA.md) for the complete structural ISA specification.

## Cross-References

- [INSTRUCTION_SET.md](../reference/INSTRUCTION_SET.md) provides the complete bytecode instruction reference.
