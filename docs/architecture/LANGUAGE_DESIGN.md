# Language Design

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma is a Total Functional Stream Processor. It is a lightweight, embeddable scripting language that compiles to bytecode and runs on a stack-based virtual machine. The language targets `no_std+alloc` environments, making it suitable for embedding in resource-constrained hosts such as audio engines and game simulations without depending on the Rust standard library.

## Design Goals

Keleusma pursues seven design goals drawn from its grammar specification.

1. **Rust with Elixir quality-of-life features.** The language adopts Rust syntax as its foundation and extends it with multiheaded functions, pattern matching on function parameters, guard clauses, pipeline expressions, and curly-brace block syntax. This combination provides expressive dispatch without introducing unfamiliar syntax for Rust developers.

2. **Rust type system.** Keleusma uses nominal types declared with Rust syntax. Structs, enums, and tuples follow Rust conventions. The type system is static, and types are resolved at compile time.

3. **Bidirectional typed yield.** Scripts are coroutines that receive typed input from the host and yield typed output back. This bidirectional data flow allows scripts to act as stream processors, consuming host events and producing responses across multiple resumption cycles.

4. **Pipeline composition.** The `|>` operator threads the result of one expression as the first argument to the next function call. Pipelines provide a readable left-to-right data flow that reduces nesting and clarifies transformation chains.

5. **Native function binding.** Rust functions can be registered with the virtual machine and called from scripts by name. This allows the host to expose domain-specific functionality without modifying the language itself.

6. **Deterministic execution.** Keleusma avoids floating-point ambiguity, undefined behavior, and garbage collection pauses. Execution is predictable and reproducible given the same inputs, which is essential for audio and simulation workloads where timing and correctness must be verifiable.

7. **Guaranteed termination or productivity.** Every function must either terminate or make observable progress on every iteration. The language enforces this through three function categories, described below, that statically constrain recursion and looping behavior.

## Three Function Categories

Keleusma classifies every function into exactly one of three categories. The category determines what control flow constructs the function may use and what termination guarantees it provides.

### `fn` -- Atomic Total

Functions declared with `fn` must terminate. They may not yield to the host, contain bare loops, or call themselves recursively. For loops are permitted only when iterating over bounded ranges or arrays, ensuring that the iteration count is known before the loop begins. Atomic total functions are suitable for pure computations that transform input to output in a single step.

### `yield` -- Non-Atomic Total

Functions declared with `yield` must eventually exit. They may yield values to the host, suspending execution and resuming when the host provides new input. They may not contain bare loops or call themselves recursively. Non-atomic total functions are suitable for multi-step computations that require host interaction but that will eventually complete.

### `loop` -- Productive Divergent

Functions declared with `loop` never exit. They must yield to the host on every iteration, guaranteeing that the script makes observable progress and that the host retains control. Exactly one loop function may exist per script, and it serves as the script entry point. After executing the last statement in the function body, execution restarts from the top of the body. The parameter slot is updated with the value provided by the host on each resume call.

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

## Scope Exclusions

The following features are explicitly excluded from the current language design.

- Hindley-Milner type inference
- Ownership, borrowing, and lifetimes
- Traits and generics
- Closures and anonymous functions
- Hot-swap of running scripts
- Formal verification at the bytecode level
- String interpolation

## Cross-References

- [GRAMMAR.md](../design/GRAMMAR.md) provides the formal EBNF grammar specification.
