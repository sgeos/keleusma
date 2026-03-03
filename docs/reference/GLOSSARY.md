# Glossary

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

Key terminology used in the Keleusma documentation and source code.

**Arena** -- The ephemeral memory region consisting of a single contiguous bump-allocated buffer. The stack grows from one end. There is no heap initially. The arena persists across yields within a single stream phase but is cleared at the RESET boundary by resetting the bump pointer. No memory survives across RESET. Memory bounds are statically analyzable.

**Atomic function** -- A function declared with the `fn` keyword that must terminate without yielding. May call other atomic functions and native functions.

**Bounded-step invariant** -- The property that there exists a statically provable upper bound on instructions executed between any two yield points. This enables WCET analysis for safety-critical certification.

**Bump allocator** -- The arena allocation strategy. Allocations advance a pointer linearly through a contiguous buffer. Deallocation occurs only at RESET when the entire arena is cleared.

**Bytecode** -- The compiled representation of a Keleusma program. A sequence of instructions executed by the virtual machine.

**Chunk** -- A compiled function in the bytecode representation. Contains an instruction sequence, constant pool, struct templates, and metadata.

**Coroutine** -- An execution context that can be suspended via yield and resumed by the host. Keleusma scripts are coroutines.

**Cost table** -- A mapping from each bytecode instruction to its execution cost in abstract time units. Used for WCET analysis. Initially populated with reasonable estimates, refined as the implementation matures.

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

**Productivity invariant** -- The property that every control path from STREAM to RESET must encounter at least one YIELD. This ensures that every iteration of a productive divergent function produces observable output.

**REENTRANT block** -- A structural ISA block type that must contain at least one YIELD. Used for logic that interacts with the host. Corresponds to non-atomic total functions and productive divergent functions in the surface language.

**RESET** -- A structural ISA primitive that clears the arena, performs a hot swap if scheduled, and jumps to the STREAM entry point. RESET is the only instruction allowed to target STREAM and is the only global back-edge in the program.

**Span** -- A source location record containing byte offsets, line number, and column number. Attached to tokens and AST nodes for error reporting.

**STREAM** -- A structural ISA block type representing the mission loop entry point. Zero or one STREAM region may exist per program. STREAM terminates with RESET. The RESET -> STREAM cycle is the only unbounded cycle.

**Total function** -- A function guaranteed to terminate, assuming called functions return. Both `fn` and `yield` functions are total. Loop functions are not total but are productive.

**Value** -- The runtime representation of data in the VM. An enum with variants: Unit, Bool, Int, Float, Str, Tuple, Array, Struct, Enum, and None.

**WCET** -- Worst-Case Execution Time. In Keleusma, WCET is measured from yield to yield by counting opcodes on the longest path between any two YIELD instructions. The absence of dynamic dispatch ensures all paths are statically enumerable.

**Yield** -- The mechanism by which a coroutine suspends execution, sends a value to the host, and receives a value when resumed. Expressed as `let input = yield output;` in source code.

**Yield contract** -- The agreement between a loop or yield function and the host about the types exchanged. Defined by the parameter type representing input from the host and the return type representing output to the host. Propagating yield functions must share the same contract.

**Yield slice** -- The sequence of instructions between two consecutive YIELD points. Each yield slice must have a statically bounded instruction count for WCET analysis. Yield slices are the fundamental unit of real-time scheduling in the yield domain.
