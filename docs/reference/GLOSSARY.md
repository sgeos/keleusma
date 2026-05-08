# Glossary

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

Key terminology used in the Keleusma documentation and source code. Citations use bracket notation (e.g., [AI1], [SY1]) referring to entries in the bibliography in [RELATED_WORK.md](./RELATED_WORK.md).

**Abstract interpretation** -- A general framework for sound static analysis of programs, introduced by Cousot and Cousot [AI1]. The framework defines analysis as computation over abstract domains (lattices) that soundly approximate concrete program behavior. In Keleusma, the productivity analysis operates over a two-element boolean lattice and the WCET analysis operates over the natural numbers, both following the abstract interpretation methodology.

**Arena** -- The ephemeral memory region consisting of a single contiguous bump-allocated buffer. The stack grows from one end. There is no heap initially. The arena persists across yields within a single stream phase but is cleared at the RESET boundary by resetting the bump pointer. No memory survives across RESET. Memory bounds are statically analyzable.

**Atomic function** -- A function declared with the `fn` keyword that must terminate without yielding. May call other atomic functions and native functions.

**Bounded-step invariant** -- The property that there exists a statically provable upper bound on instructions executed between any two yield points. This enables WCET analysis for safety-critical certification.

**Bump allocator** -- The arena allocation strategy. Allocations advance a pointer linearly through a contiguous buffer. Deallocation occurs only at RESET when the entire arena is cleared.

**Bytecode** -- The compiled representation of a Keleusma program. A sequence of instructions executed by the virtual machine.

**Chunk** -- A compiled function in the bytecode representation. Contains an instruction sequence, constant pool, struct templates, and metadata.

**Coroutine** -- An execution context that can be suspended via yield and resumed by the host. Keleusma scripts are coroutines.

**Cost table** -- A mapping from each bytecode instruction to its execution cost in abstract time units, implemented as `Op::cost()` in `src/bytecode.rs`. Costs are relative integer weights across five tiers: 1 for data movement, 2 for arithmetic, 3 for division and field lookup, 5 for composite construction, and 10 for function calls. Used by `wcet_stream_iteration()` for worst-case execution time analysis. These values are preliminary and subject to refinement.

**Dialogue type** -- The pair of types (A, B) that defines the yield contract between a stream program and its host. Input A is provided by the host on resume. Output B is produced by the program on yield. The dialogue type must remain invariant across hot code swaps.

**Double buffering** -- The hot swap mechanism. The host loads new text and rodata into a secondary buffer while the current code continues executing. RESET activates the new buffer. The old buffer is retained for rollback.

**Guard clause** -- A boolean condition attached to a function head using the `when` keyword. Evaluated after pattern matching succeeds. Limited to comparisons and logical operators.

**Host** -- The Rust application that embeds the Keleusma VM, registers native functions, and drives coroutine execution by calling `call()` and `resume()`.

**Keleusma** -- A Total Functional Stream Processor that compiles to bytecode. The name derives from the Greek word for a command or signal, specifically the rhythmic calls used by ancient Greek rowing masters to coordinate oar strokes.

**keleusma_type** -- A Rust attribute macro (`#[keleusma_type]`) that enforces interoperable memory layout on host types used in the dialogue signature. Ensures the host and VM agree on binary representation of A and B types.

**Loop function** -- A function declared with the `loop` keyword that never exits. Must yield on every iteration. Exactly one per script. Serves as the coroutine entry point.

**Module** -- The compiled output of the compiler. Contains chunks for compiled functions, an entry point index, and enum definitions.

**Multiheaded function** -- Multiple function definitions with the same name and arity that form a single logical function. Dispatch selects the first head whose pattern matches the arguments.

**Native function** -- A Rust function registered by the host and callable from Keleusma scripts. Registered via `vm.register_native()` or `vm.register_native_closure()`.

**Opaque type** -- A host-defined type that scripts can pass through function calls but cannot destructure or inspect. Recognized from the native function registry.

**Phase clock** -- The coarse-grained temporal domain governed by RESET. Swap latency and arena lifetime are measured from RESET to RESET. See also: yield domain.

**Pipeline** -- The `|>` operator that passes the result of the left expression as an argument to the right function call. Supports placeholder `_` for argument position control.

**Productive divergent function** -- A function declared with the `loop` keyword. It diverges by never exiting, but is productive because it yields a value on every iteration.

**Productivity invariant** -- The property that every control path from STREAM to RESET must encounter at least one YIELD. This ensures that every iteration of a productive divergent function produces observable output. Enforced by Pass 5 of the structural verifier via abstract interpretation [AI1] over a two-element lattice. The coinductive dual of termination, as studied by Endrullis et al. [C4].

**Productivity verification** -- The static analysis pass that enforces the productivity invariant. Implemented as `analyze_yield_coverage()` in `src/verify.rs`. The analysis walks the block-structured control flow, tracking whether all paths have yielded. At If/Else branches, both branches must yield. At loops, all break-exit paths must have yielded. Programs that violate the invariant are rejected at load time.

**REENTRANT block** -- A structural ISA block type that must contain at least one YIELD. Used for logic that interacts with the host. Corresponds to non-atomic total functions and productive divergent functions in the surface language.

**RESET** -- A structural ISA primitive that clears the arena, performs a hot swap if scheduled, and jumps to the STREAM entry point. RESET is the only instruction allowed to target STREAM and is the only global back-edge in the program.

**Soundness** -- The property that a verification pass rejects all invalid programs. A sound verifier never accepts a program that violates the property it checks. The structural verifier's soundness has not been formally proven. See [RELATED_WORK.md](./RELATED_WORK.md) Section 7 for the certification gap analysis.

**Span** -- A source location record containing byte offsets, line number, and column number. Attached to tokens and AST nodes for error reporting.

**Synchronous hypothesis** -- The assumption, originating in the synchronous reactive language tradition [L1, SY1], that computation completes within one logical tick. In Keleusma, each yield-to-yield slice corresponds to one logical tick, and the bounded-step invariant ensures that each tick completes in bounded time.

**STREAM** -- A structural ISA block type representing the mission loop entry point. Zero or one STREAM region may exist per program. STREAM terminates with RESET. The RESET -> STREAM cycle is the only unbounded cycle.

**Tool qualification** -- The certification process for development tools under safety standards such as DO-178C [IC1] and DO-330 [IC2]. A qualified tool has been demonstrated to produce correct output or to detect its own errors. Keleusma's compiler and verifier are not currently qualified. See [RELATED_WORK.md](./RELATED_WORK.md) Section 7 for the certification gap analysis.

**Total function** -- A function guaranteed to terminate, assuming called functions return. Both `fn` and `yield` functions are total. Loop functions are not total but are productive. The totality guarantee follows Turner's argument for total functional programming [T1].

**Value** -- The runtime representation of data in the VM. An enum with variants: Unit, Bool, Int, Float, Str, Tuple, Array, Struct, Enum, and None.

**Structural verification** -- Load-time validation of compiled modules. Implemented as `verify()` in `src/verify.rs`. Performs five passes per chunk: block nesting, offset validation, block type constraints, break containment, and productivity rule enforcement. Programs that fail verification are rejected before execution begins.

**WCET** -- Worst-Case Execution Time. In Keleusma, WCET is measured from yield to yield by counting weighted opcodes on the longest path between any two YIELD instructions. The absence of dynamic dispatch ensures all paths are statically enumerable. Computed by `wcet_stream_iteration()` in `src/verify.rs`. This abstract opcode cost does not directly correspond to wall-clock execution time. Industrial WCET tools [WC1] account for pipeline effects, cache behavior, and branch prediction on target hardware. See [RELATED_WORK.md](./RELATED_WORK.md) Section 4 for the distinction between abstract and hardware-aware WCET analysis.

**WCET analysis** -- The static analysis function `wcet_stream_iteration()` that computes the worst-case cost of one Stream-to-Reset iteration. Uses the same recursive block-structured traversal as productivity verification, but sums `Op::cost()` values and takes the maximum at each control flow join rather than computing a boolean lattice.

**Yield** -- The mechanism by which a coroutine suspends execution, sends a value to the host, and receives a value when resumed. Expressed as `let input = yield output;` in source code.

**Yield contract** -- The agreement between a loop or yield function and the host about the types exchanged. Defined by the parameter type representing input from the host and the return type representing output to the host. Propagating yield functions must share the same contract.

**Yield slice** -- The sequence of instructions between two consecutive YIELD points. Each yield slice must have a statically bounded instruction count for WCET analysis. Yield slices are the fundamental unit of real-time scheduling in the yield domain.
