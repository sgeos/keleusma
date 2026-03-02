# Backlog Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Deferred decisions for future consideration. These are explicitly out of scope for the current development phase.

## B1. Hindley-Milner type inference

Full type inference would reduce annotation burden by allowing the compiler to deduce types from usage rather than requiring explicit declarations. Deferred due to implementation complexity and the current lack of generic types in the language.

## B2. Traits or generic type parameters

Traits or generic type parameters would enable polymorphic functions and reusable abstractions across different data types. Deferred to keep the VM and compiler simple during early development.

## B3. Closures or anonymous functions

Closures or anonymous functions would enable higher-order programming patterns such as callbacks and inline transformations. Deferred to keep the VM simple. Multiheaded function dispatch serves as a partial alternative for pattern-based dispatch.

## B4. Hot-swap mechanism

Live reloading of scripts without restarting the VM would enable rapid iteration during development. Deferred due to the complexity of state migration, including preserving coroutine state and local variables across script reloads.

## B5. Formal verification at bytecode level

Proving termination and productivity guarantees at the bytecode level rather than relying on source-level analysis would strengthen correctness assurances. Deferred pending a formal specification of the bytecode semantics.

## B6. String interpolation

String interpolation is not needed for a control language. Keleusma scripts primarily produce structured enum values and numeric outputs, not formatted strings. If formatting is needed, the host can provide native functions for string construction.

## B7. Error propagation through yield

Allowing yield to return `Result<T, E>` so the host can signal errors to the script would enable bidirectional error handling. Deferred due to type system complexity and the need to define error recovery semantics at the language level.

## B8. VM allocation model

Should the VM allocate per-script or share an arena across all active scripts? Currently each VM instance is independent with its own heap allocations. A shared arena could reduce allocation overhead for hosts running many concurrent scripts, but would add complexity to lifetime management.
