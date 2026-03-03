# Priority Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Open decisions that may block near-term development.

## P1. Type checker implementation

The compiler currently produces bytecode without type checking or name resolution validation. Adding a semantic analysis pass would catch type errors at compile time rather than runtime. This affects the reliability of the compilation pipeline. The scope of such a pass needs to be determined, including whether it should be a simple type-check against declared signatures or a full inference system, and how it interacts with native function type declarations.

## P2. For-in over expressions

The compiler currently only supports range-based for loops of the form `for i in 0..n`. Support for iterating over array expressions, such as `for item in array`, is specified in the grammar but not yet implemented. The implementation requires deciding on iterator semantics, including whether iteration consumes the array or borrows it, and how to handle mutation of the array during iteration.

## P3. Error recovery model

What should happen when a script encounters a runtime error? Options include yielding a default value, suspending the script, or notifying the host via `VmError`. The current implementation halts execution on error. A recovery model would need to define whether the host can resume a script after an error, and if so, what value the host supplies at the recovery point.

## P4. Structural ISA implementation

The structural ISA with Stream, Reset, Reentrant, Func, and block-structured control flow is currently being implemented (R21). The transition from the previous 48-instruction flat-jump bytecode to the structural ISA requires compiler and VM changes, including replacing flat jumps with block-structured control flow, adding block primitives to the bytecode format, implementing the arena memory model with Reset-triggered clearing, and implementing the structural verification pass.

## P5. WCET analysis tooling

Static analysis to compute worst-case execution time between yield points is required for safety-critical certification. The analysis must enumerate all paths between YIELD instructions and count opcodes on the longest path. The absence of dynamic dispatch ensures that all paths are statically enumerable, but the tooling to perform this analysis has not yet been implemented.
