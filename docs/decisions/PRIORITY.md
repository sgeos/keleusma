# Priority Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Open decisions that may block near-term development.

## P1. Type checker implementation

The compiler currently produces bytecode without type checking or name resolution validation. Adding a semantic analysis pass would catch type errors at compile time rather than runtime. This affects the reliability of the compilation pipeline. The scope of such a pass needs to be determined, including whether it should be a simple type-check against declared signatures or a full inference system, and how it interacts with native function type declarations.

## P2. For-in over expressions

The compiler currently only supports range-based for loops of the form `for i in 0..n`. Support for iterating over array expressions, such as `for item in array`, is specified in the grammar but not yet implemented. The implementation requires deciding on iterator semantics, including whether iteration consumes the array or borrows it, and how to handle mutation of the array during iteration.

## P3. Error recovery model

What should happen when a script encounters a runtime error? Options include yielding a default value, suspending the script, or notifying the host via `VmError`. The current implementation halts execution on error. A recovery model would need to define whether the host can resume a script after an error, and if so, what value the host supplies at the recovery point.

## ~~P4. Structural ISA implementation~~ (Resolved as R22)

## ~~P5. WCET analysis tooling~~ (Resolved as R23)
