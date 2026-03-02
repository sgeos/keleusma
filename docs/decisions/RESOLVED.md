# Resolved Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Completed design and architectural decisions with rationale.

## R1. no_std + alloc target

The crate targets `no_std` with `alloc` to maximize portability. It can run in embedded, WASM, and standard environments without modification. The only external dependency is `libm` for math functions. This constraint ensures the language runtime imposes no operating system requirements on the host application.

## R2. Stack-based virtual machine

A stack-based VM was chosen over a register-based VM for simplicity of implementation and natural support for expression evaluation. Stack-based architectures map directly to the recursive structure of arithmetic and function call expressions. The stack model also simplifies coroutine state preservation across yields, since the entire evaluation state lives on a single stack that can be captured and restored.

## R3. Three function categories

Functions are categorized as `fn` for atomic total functions, `yield` for non-atomic total functions, and `loop` for productive divergent functions. This categorization enables static verification of termination and productivity guarantees without a full type checker. The compiler can enforce that `fn` functions never yield, that `yield` functions always terminate, and that `loop` functions yield on every iteration.

## R4. Recursion prohibition

All forms of recursion are rejected at compile time. The compiler detects cycles in the call graph and reports them as errors. Recursive algorithms must be supplied by the host as native functions. This simplifies termination analysis by ensuring that the call graph is a directed acyclic graph, which makes it possible to verify termination through topological ordering alone.

## R5. No closures or anonymous functions

Closures and anonymous functions are excluded to keep the VM simple. All functions are named and defined at the top level of a module. Higher-order patterns are achieved through multiheaded function dispatch, which allows a single function name to match different argument patterns. This avoids the need for captured environments and upvalue management in the VM.

## R6. libm as sole dependency

The `libm` crate provides math functions such as `sin`, `cos`, `pow`, and `log10` in `no_std` environments. No other external dependencies are used. This minimizes the supply chain surface and ensures the crate can compile in any environment that supports `alloc`.

## R7. Curly brace block delimiters

Keleusma uses curly braces for block delimitation rather than `do`/`end` or significant indentation. This is consistent with the Rust host language and reduces parser ambiguity. Curly braces provide unambiguous block boundaries without requiring whitespace sensitivity in the lexer.

## R8. Semicolons for statement termination

Semicolons are required to terminate statements, following Rust conventions. The last expression in a block is the return value and does not require a trailing semicolon. This convention provides clear visual separation between statements while preserving expression-oriented block semantics.

## R9. Host-declared purity

Purity of native functions is declared by the host at registration time, not verified by the compiler. Analysis trusts the declaration. Impurity is transitive through the call graph, meaning any function that calls an impure function is itself considered impure. Since native functions execute arbitrary host code, the compiler cannot verify their purity and must rely on the host to declare it honestly.

## R10. Single module per file

Each script file constitutes one module. Modules cannot import other Keleusma modules. All external functionality comes from native function registrations provided by the host. This eliminates the need for a module resolution system, dependency tracking, or linking phase. Composition happens at the host level by registering different sets of native functions for different scripts.

## R11. .kma file extension

Script files use the `.kma` file extension. This provides a distinctive identifier for tooling, editor support, and file association without conflicting with other language extensions.

## R12. Stream coalgebra model

Top-level productive divergent functions model stream transformations of the form f : Stream<A> -> Stream<B>. This coalgebraic formulation enables mathematical reasoning about infinite stream transformations and provides a formal foundation for productivity proofs. Helper functions may yield but must share the top-level function's dialogue type.

## R13. Arena memory model

The VM uses an arena memory model consisting of a stack and a scratchpad heap. The arena persists across yields within a single stream phase but is cleared at the top of every productive divergent function iteration (the RESET boundary). This prevents memory leaks, ensures predictable resource usage, and eliminates memory debt across mission phases. Memory bounds are statically analyzable per stream phase.

## R14. Two temporal domains

Execution is governed by two temporal domains. The yield domain (control clock) provides fine-grained scheduling with WCET measured yield-to-yield. The reset domain (phase clock) provides coarse-grained phase control with swap latency measured reset-to-reset. This separation allows independent analysis and certification of timing properties at each granularity.

## R15. Structural ISA verification

Programs are verified at load time via block-graph coloring. The structural ISA uses block types (STREAM, REENTRANT, FUNC, LOOP_N) that make invalid or unproductive programs impossible to define. A linear scan verifies that all paths from STREAM to RESET contain at least one YIELD and that all FUNC blocks are free of yields. Invalid programs are rejected before execution begins.
