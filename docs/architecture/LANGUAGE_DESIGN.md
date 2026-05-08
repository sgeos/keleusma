# Language Design

> **Navigation**: [Architecture](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma is a Total Functional Stream Processing Language whose design is informed by the synchronous reactive language tradition [SY1] and the coalgebraic theory of stream processing [C1, C2]. Its philosophy is "boring code enabling exciting behavior": the language enforces simplicity, determinism, and analyzability while allowing external systems to perform complex tasks. It is lightweight, embeddable, compiles to bytecode, and runs on a stack-based virtual machine targeting `no_std+alloc` environments. Its target applications include embedded audio engines, game scripting, and domains where bounded-step execution and deterministic scheduling are required. See [RELATED_WORK.md](../reference/RELATED_WORK.md) for the full academic and industrial context.

## Design Philosophy

Keleusma emphasizes minimal, analyzable primitives. The language eliminates dynamic features such as dynamic dispatch, unbounded recursion, and heap fragmentation to ensure absolute predictability. Exciting system behavior emerges externally, not from language complexity. These design choices are shared with the synchronous reactive language family (Lustre, Esterel, Signal, SCADE) [SY1], which has demonstrated that deterministic, bounded-step languages are practical for real-time embedded applications.

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

Every top-level productive divergent function represents a coalgebra: `f : Stream<A> -> Stream<B>`, following Rutten's theory of universal coalgebra [C1] and coinductive stream calculus [C2]. Functions transform one stream into another, potentially infinitely. Helper functions may yield but must match the top-level function's dialogue type (yield contract). The coalgebraic model enables mathematical reasoning about infinite stream transformations and provides the theoretical foundation for productivity proofs [C4].

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

1. **Totality.** No partial functions or undefined behavior. Every execution path terminates or yields. This follows Turner's argument for total functional programming [T1] and is enforced through recursion prohibition and bounded loops.
2. **Productivity.** Each iteration of a productive divergent function produces observable output via at least one yield. This is the coinductive dual of termination, as studied by Endrullis et al. for stream definitions [C4] and unified with termination checking by Abel and Pientka [C3].
3. **Bounded-step.** There exists a statically provable upper bound on instructions executed between any two yield points (WCET analyzable). This corresponds to the synchronous hypothesis [L1, SY1] and enables WCET analysis [WC1].
4. **Safe swapping.** Hot code swaps preserve type safety and stream continuity. Only the dialogue type must remain invariant across swaps.

## Memory Model

The Keleusma runtime memory layout corresponds to the four conventional executable sections found in the Unix linker tradition and the System V Application Binary Interface, namely `.text`, `.rodata`, `.data`, and `.bss`. This analogy is the organizing frame for runtime memory. See [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) for the detailed specification and [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 8 for the academic and engineering precedents.

| Region | Conventional analogue | Mutability | Lifetime |
|---|---|---|---|
| Bytecode chunks | `.text` | Immutable | Until hot swap at RESET |
| Constant pool and templates | `.rodata` | Immutable | Until hot swap at RESET |
| Data segment | `.data` | Mutable | Persists across yield and reset |
| Arena and operand stack | `.bss` | Mutable | Cleared at RESET |

The Keleusma source language is purely functional with respect to script-defined values. Local bindings, the operand stack, and the arena are conceptually immutable at the surface language level. The data segment is the sole region of mutable state observable to the script that persists beyond a single function activation. The host owns the data segment storage and presents it to the script as a preinitialized `.data` section. Scripts read and write the segment through a fixed schema declared in a `data` block. The schema is fixed within a single code image and may change arbitrarily across hot updates. Cross-yield value preservation is not guaranteed. The host may write to the segment between yields. The data segment design draws on the persistent state model of the Erlang and Open Telecom Platform multi-version code coexistence pattern [H1, H2] and on the state vector model of mode automata in the synchronous reactive language tradition [H3, SC1].

The arena is a single contiguous allocation using bump allocation. The stack grows from one end. The arena persists across yields within an iteration but is cleared at the RESET boundary by resetting the bump pointer. Memory bounds are statically analyzable.

## Hot Code Swapping

Keleusma supports hot code swapping at the RESET boundary of a productive divergent function iteration. Only the dialogue type, namely the yield contract from A to B, must remain invariant across swaps. Text, rodata, and the data segment schema may all change across a swap. Different routines may have different WCETs, and each is certified independently. The arena is cleared before the new code begins executing, ensuring zero memory debt across swap boundaries. Cross-swap data segment value handling follows Replace semantics, in which the host atomically supplies the data instance appropriate for the new code version. The host may keep, modify, migrate, or substitute the underlying storage transparently. Atomicity is logical only. The new image must be resident before the candidate is eligible for installation. Rollback is mechanically identical to a forward update with an older code version selected. The model parallels the Erlang and Open Telecom Platform multi-version code coexistence pattern [H1, H2], with the simplification that the migration callback resides in the host rather than in the script. See [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) for the full specification.

## WCET Analysis

Worst-Case Execution Time is measured from yield to yield. Each yield-to-yield slice must have a statically provable upper bound on instructions executed. In the absence of dynamic dispatch, every execution path is a static directed acyclic graph between yield points. WCET is determined by counting weighted opcodes on the longest path between any two yield points. Wilhelm et al. provide a comprehensive survey of WCET analysis methods and tools [WC1].

Each bytecode instruction carries a relative integer cost via `Op::cost()`, assigned across five tiers: 1 for data movement and control flow markers, 2 for arithmetic and comparisons, 3 for division and field lookup, 5 for composite value construction, and 10 for function calls. The `wcet_stream_iteration()` function in `src/verify.rs` computes the worst-case total cost of one Stream-to-Reset iteration by recursively analyzing block-structured control flow, taking the maximum cost branch at each join point. These cost weights are preliminary and subject to refinement as the instruction set stabilizes.

Abstract opcode cost does not directly correspond to wall-clock execution time. Industrial WCET analysis tools such as aiT [WC2] account for pipeline effects, cache behavior, and branch prediction on the target hardware. For safety-critical certification, a sound bound on real-time WCET requires either a time-predictable execution platform (as demonstrated for JOP in [WC5]) or a validated mapping from abstract cost to physical time. Keleusma's current WCET analysis is sufficient for soft real-time applications (audio engines, game scripting) where approximate cost bounds inform scheduling decisions. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 4 for a full discussion.

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

The host registers native functions with the virtual machine before compilation. Four registration methods are available, in order from most to least ergonomic.

**Ergonomic typed registration.** The host calls `vm.register_fn(name, func)` where `func` is an ordinary Rust function or closure of arity zero through four whose argument and return types implement `KeleusmaType`. Argument extraction, arity checking, and return value wrapping happen automatically. Use `vm.register_fn_fallible(name, func)` when the host function returns `Result<R, VmError>`. This is the recommended path for new code.

**Function pointers.** The host provides a function name and a Rust function pointer with the signature `fn(&[Value]) -> Result<Value, VmError>`. Suitable when the host function must inspect arbitrary `Value` variants.

**Closures.** The host provides a function name and a boxed closure that captures external state. Suitable for functions that need to read from or write to host resources or that must inspect arbitrary `Value` variants.

Native functions participate in the script call graph like any other function. The compiler resolves native function names during compilation, and the virtual machine dispatches to the registered implementation at runtime.

Type coercion at the native function boundary is flexible. Integer arguments are accepted where floating-point parameters are expected, with automatic widening from `i64` to `f64`. Purity of native functions is declared by the host and is not verified by the compiler. The host is responsible for ensuring that functions declared as pure do not produce side effects.

The host is responsible for verifying and certifying host functions. Native functions can define domain-specific vocabularies tailored to the target application.

### KeleusmaType and Static Marshalling

The `KeleusmaType` trait defines the static marshalling contract between Rust types and the runtime `Value` enum. Host structs and enums become implementations through `#[derive(KeleusmaType)]` from the `keleusma-macros` crate. The derive accepts named-field structs and enums whose variants may be unit, tuple-style, or struct-style. Field types compose admissible interop types per the rules in [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md).

The static marshalling approach contrasts with the dynamic approach of Rhai, which relies on `Box<dyn Any>` to carry arbitrary Rust types. Keleusma's discipline of fixed-size, fixed-layout interop types makes static dispatch sufficient and avoids the unsafe pointer manipulation and runtime type-erasure overhead of the dynamic approach. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 9 for the full comparison.

## Scope Exclusions

The following features are explicitly excluded from the current language design.

- Hindley-Milner type inference
- Ownership, borrowing, and lifetimes
- Traits and generics
- Closures and anonymous functions
- String interpolation

Note: Hot code swapping at the bytecode level is part of the design and is described in [EXECUTION_MODEL.md](./EXECUTION_MODEL.md). Structural verification is implemented and described in [TARGET_ISA.md](../reference/TARGET_ISA.md).

Keleusma's design choices are informed by synchronous reactive language principles and are favorable for eventual safety-critical certification, but current claims of suitability for "aerospace, robotics, and flight control" are design aspirations, not certification status. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 7 for a gap analysis between the current implementation and industrial certification readiness.

## Cross-References

- [GRAMMAR.md](../design/GRAMMAR.md) provides the formal EBNF grammar specification.
- [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) describes the target execution model with temporal domains.
- [TARGET_ISA.md](../reference/TARGET_ISA.md) describes the structural ISA specification.
- [RELATED_WORK.md](../reference/RELATED_WORK.md) positions Keleusma within the academic and industrial landscape.

## Citation Key

Citations in this document use bracket notation (e.g., [SY1], [C1]) referring to entries in the bibliography in [RELATED_WORK.md](../reference/RELATED_WORK.md).
