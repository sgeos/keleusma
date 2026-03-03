# Language Design

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma is a Total Functional Stream Processing Language designed for safety-critical and industrially certifiable applications such as robotics, flight control, and embedded audio. Its philosophy is "boring code enabling exciting behavior": the language enforces simplicity, determinism, and analyzability while allowing external systems to perform complex tasks. It is lightweight, embeddable, compiles to bytecode, and runs on a stack-based virtual machine targeting `no_std+alloc` environments.

## Design Philosophy

Keleusma emphasizes minimal, analyzable primitives. The language eliminates dynamic features such as dynamic dispatch, unbounded recursion, and heap fragmentation to ensure absolute predictability. Exciting system behavior emerges externally, not from language complexity. The language is designed for formal certifiability in hard real-time domains.

## Design Goals

Keleusma pursues seven design goals drawn from its grammar specification.

1. **Rust with Elixir quality-of-life features.** The language adopts Rust syntax as its foundation and extends it with multiheaded functions, pattern matching on function parameters, guard clauses, pipeline expressions, and curly-brace block syntax. This combination provides expressive dispatch without introducing unfamiliar syntax for Rust developers.

2. **Rust type system.** Keleusma uses nominal types declared with Rust syntax. Structs, enums, and tuples follow Rust conventions. The type system is static, and types are resolved at compile time.

3. **Bidirectional typed yield.** Scripts are coroutines that receive typed input from the host and yield typed output back. This bidirectional data flow allows scripts to act as stream processors, consuming host events and producing responses across multiple resumption cycles.

4. **Pipeline composition.** The `|>` operator threads the result of one expression as the first argument to the next function call. Pipelines provide a readable left-to-right data flow that reduces nesting and clarifies transformation chains.

5. **Native function binding.** Rust functions can be registered with the virtual machine and called from scripts by name. This allows the host to expose domain-specific functionality without modifying the language itself.

6. **Deterministic execution.** Keleusma avoids floating-point ambiguity, undefined behavior, and garbage collection pauses. Execution is predictable and reproducible given the same inputs, which is essential for audio and simulation workloads where timing and correctness must be verifiable.

7. **Guaranteed termination or productivity.** Every function must either terminate or make observable progress on every iteration. The language enforces this through three function categories, described below, that statically constrain recursion and looping behavior.

## Target Applications

Keleusma targets three application domains.

- **Safety-critical systems.** Aerospace, robotics, and flight control. The totality guarantees of the language, bounded-step execution, and static Worst-Case Execution Time (WCET) analysis make it suitable for industrially certifiable control loops.
- **Audio engines.** Real-time audio synthesis and effect processing. The deterministic execution model prevents glitches and timing jitter.
- **Game scripting.** Scenario event handling, NPC behavior, and game logic. The coroutine model allows scripts to process events across multiple game ticks.

## Stream Coalgebra

Every top-level productive divergent function represents a coalgebra: `f : Stream<A> -> Stream<B>`. Functions transform one stream into another, potentially infinitely. Helper functions may yield but must match the top-level function's dialogue type (yield contract). The coalgebraic model enables mathematical reasoning about infinite stream transformations.

## Three Function Categories

Keleusma classifies every function into exactly one of three categories. The category determines what control flow constructs the function may use and what termination guarantees it provides.

### `fn` -- Atomic Total

Functions declared with `fn` must terminate. They may not yield to the host, contain bare loops, or call themselves recursively. For loops are permitted only when iterating over bounded ranges or arrays, ensuring that the iteration count is known before the loop begins. Atomic total functions are suitable for pure computations that transform input to output in a single step.

### `yield` -- Non-Atomic Total

Functions declared with `yield` must eventually exit. They may yield values to the host, suspending execution and resuming when the host provides new input. They may not contain bare loops or call themselves recursively. Non-atomic total functions are suitable for multi-step computations that require host interaction but that will eventually complete.

### `loop` -- Productive Divergent

Functions declared with `loop` never exit. They must yield to the host on every iteration, guaranteeing that the script makes observable progress and that the host retains control. Exactly one loop function may exist per script, and it serves as the script entry point. After executing the last statement in the function body, execution restarts from the top of the body. The parameter slot is updated with the value provided by the host on each resume call.

The three categories map to the structural ISA block types. Atomic functions correspond to FUNC blocks. Non-atomic total functions and productive divergent functions correspond to REENTRANT blocks. The loop function corresponds to the STREAM region. See [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) for the target execution model.

## Four Guarantees

Keleusma provides four static guarantees about program behavior.

1. **Totality.** No partial functions or undefined behavior. Every execution path terminates or yields.
2. **Productivity.** Each iteration of a productive divergent function produces observable output via at least one yield.
3. **Bounded-step.** There exists a statically provable upper bound on instructions executed between any two yield points (WCET analyzable).
4. **Safe swapping.** Hot code swaps preserve type safety and stream continuity. Only the dialogue type must remain invariant across swaps.

## Memory Model

Keleusma uses an arena memory model. The arena is a single contiguous allocation using bump allocation. The stack grows from one end. There is no heap initially. The arena persists across yields within an iteration but is cleared at the top of every productive divergent function iteration (the RESET boundary). Deallocation occurs only at RESET, when the entire arena is cleared by resetting the bump pointer. This prevents memory leaks and ensures predictable resource usage. No memory survives across RESET boundaries. Memory bounds are statically analyzable.

Clear separation exists between three memory regions.

- **Arena (bump-allocated).** Ephemeral per iteration. Single contiguous buffer with stack growing from one end. Cleared at RESET.
- **Read-only sections (rodata + text).** Immutable program code and constant data. Double-buffered and swappable at RESET boundaries during hot code swaps.
- **Host-controlled state.** External to the VM, managed by the host application.

## Hot Code Swapping

Keleusma supports swapping the text and rodata segments at the boundary of a productive divergent function iteration (the RESET point). Only the dialogue type (the yield contract `A -> B`) must remain constant across swaps. Different routines may have different WCETs, and each is certified independently. The arena is cleared before the new code begins executing, ensuring zero memory debt across swap boundaries.

## WCET Analysis

Worst-Case Execution Time is measured from yield to yield. Each yield-to-yield slice must have a statically provable upper bound on instructions executed. In the absence of dynamic dispatch, every execution path is a static directed acyclic graph between yield points. WCET is determined by counting opcodes on the longest path between any two yield points. This enables industrial certification for hard real-time systems.

## Turing Completeness

Individual time slices are not Turing complete. Each yield-to-yield slice executes a bounded number of instructions and then suspends. Turing completeness arises from the VM-Host pair operating over the unbounded RESET cycle. The host provides the "tape" through YIELD exchanges, supplying new input on each resumption. Computation can span arbitrarily many RESET cycles, and the host-controlled state that persists across resets serves as the unbounded external memory that completes the computational model. The language is deliberately not Turing complete in isolation. This constraint is what makes static WCET analysis and industrial certification possible.

## Two Temporal Domains

Keleusma separates temporal control into two domains.

- **Yield domain (control clock).** Fine-grained scheduling. WCET is measured yield-to-yield. Multiple yields may occur within a single iteration.
- **Reset domain (phase clock).** Coarse-grained phase control. Swap latency is measured reset-to-reset. Arena memory is cleared and hot swaps take effect at RESET boundaries.

## Coroutine Model

Scripts execute as coroutines managed by the host. The host initiates execution by calling `call()` on the virtual machine, which begins at the designated loop function entry point. When a script yields a value, execution suspends and the host receives the yielded output along with a `VmState::Yielded` indicator.

The host resumes execution by calling `resume(input)`, providing a new input value. For loop functions, this input value replaces the parameter slot so that the next iteration operates on fresh host data. For yield functions, the resume value is returned at the yield site.

This model allows scripts to operate as persistent stream processors. The host drives the execution schedule, and the script never runs unboundedly without yielding control.

## Native Function Interface

The host registers native functions with the virtual machine before compilation. Two registration methods are available.

**Function pointers.** The host provides a function name and a Rust function pointer with the signature `fn(&[Value]) -> Result<Value, VmError>`. This method is appropriate for stateless utility functions.

**Closures.** The host provides a function name and a boxed closure that captures external state. This method is appropriate for functions that need to read from or write to host resources.

Native functions participate in the script call graph like any other function. The compiler resolves native function names during compilation, and the virtual machine dispatches to the registered implementation at runtime.

Type coercion at the native function boundary is flexible. Integer arguments are accepted where floating-point parameters are expected, with automatic widening from `i64` to `f64`. Purity of native functions is declared by the host and is not verified by the compiler. The host is responsible for ensuring that functions declared as pure do not produce side effects.

The host is responsible for verifying and certifying host functions. Native functions can define domain-specific vocabularies tailored to the target application.

## Scope Exclusions

The following features are explicitly excluded from the current language design.

- Hindley-Milner type inference
- Ownership, borrowing, and lifetimes
- Traits and generics
- Closures and anonymous functions
- String interpolation

Note: Hot code swapping and structural verification at the bytecode level are part of the design and are described in [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) and [TARGET_ISA.md](../reference/TARGET_ISA.md). The structural ISA is currently being implemented.

## Cross-References

- [GRAMMAR.md](../design/GRAMMAR.md) provides the formal EBNF grammar specification.
- [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) describes the target execution model with temporal domains.
- [TARGET_ISA.md](../reference/TARGET_ISA.md) describes the structural ISA specification.
