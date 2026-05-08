# Backlog Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Deferred decisions for future consideration. These are explicitly out of scope for the current development phase.

## B1. Hindley-Milner type inference

Full type inference would reduce annotation burden by allowing the compiler to deduce types from usage rather than requiring explicit declarations. Deferred due to implementation complexity and the current lack of generic types in the language.

## B2. Traits or generic type parameters

Traits or generic type parameters would enable polymorphic functions and reusable abstractions across different data types. Deferred to keep the VM and compiler simple during early development.

## B3. Closures or anonymous functions

Closures or anonymous functions would enable higher-order programming patterns such as callbacks and inline transformations. Deferred to keep the VM simple. Multiheaded function dispatch serves as a partial alternative for pattern-based dispatch.

## ~~B4. Hot code swap implementation~~ (Resolved as R29)

Hot code swap is implemented through `Vm::replace_module`. The host calls it between a `VmState::Reset` and the next `call`. The new module is verified before replacement. The host supplies an initial data segment instance whose length must match the new module's declared slot count. Frames and stack are cleared so the next `call` starts the new module's entry point. The same mechanism supports forward update and rollback. See R29 in [RESOLVED.md](./RESOLVED.md).

## ~~B5. Structural verification implementation~~ (Resolved as R22, R23)

Structural verification is implemented. See R22 and R23 in [RESOLVED.md](./RESOLVED.md).

## B5. Static string discipline

String values currently use `Value::Str(String)`, which is heap-allocated and variable-length. This is in tension with the fixed-size discipline adopted for interop types in R29 and R30. A future redesign would intern string literals into the `.rodata` section and replace `Value::Str(String)` with an index-based representation. Dynamic string operations would move to host-supplied native functions. The user has indicated that this redesign is acceptable and that anything fancy with strings should be the host's responsibility. Deferred to allow the V0.0-M4 marshalling work to land without disturbing existing tests.

## B6. String interpolation

String interpolation is not needed for a control language. Keleusma scripts primarily produce structured enum values and numeric outputs, not formatted strings. If formatting is needed, the host can provide native functions for string construction.

## B7. Error propagation through yield

Allowing yield to return `Result<T, E>` so the host can signal errors to the script would enable bidirectional error handling. Deferred due to type system complexity and the need to define error recovery semantics at the language level.

## B8. VM allocation model

Should the VM allocate per-script or share an arena across all active scripts? Currently each VM instance is independent with its own heap allocations. A shared arena could reduce allocation overhead for hosts running many concurrent scripts, but would add complexity to lifetime management.
