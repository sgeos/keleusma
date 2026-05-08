# Glossary

> **Navigation**: [Reference](./README.md) | [Documentation Root](../README.md)

Key terminology used in the Keleusma documentation and source code. Citations use bracket notation (e.g., [AI1], [SY1]) referring to entries in the bibliography in [RELATED_WORK.md](./RELATED_WORK.md).

**Abstract interpretation** -- A general framework for sound static analysis of programs, introduced by Cousot and Cousot [AI1]. The framework defines analysis as computation over abstract domains (lattices) that soundly approximate concrete program behavior. In Keleusma, the productivity analysis operates over a two-element boolean lattice and the WCET analysis operates over the natural numbers, both following the abstract interpretation methodology.

**Arena** -- The ephemeral memory region consisting of a single contiguous buffer with two pointers growing toward each other from opposite ends. The operand stack grows from one end. The heap, used for dynamic strings and other arena allocations, grows from the other. The arena persists across yields within a single stream phase but is cleared at the RESET boundary by resetting both pointers. No arena memory survives across RESET. Memory bounds are statically analyzable in aligned bytes.

**Dual-end arena** -- The arena's stack-and-heap arrangement, in which two bump pointers grow from opposite ends of a single contiguous buffer. There is no fixed boundary. Allocation fails only when the two pointers would meet. The verifier proves at compile time that worst-case stack consumption plus worst-case heap consumption stays under the arena size. Implemented as the `Arena` type in `src/arena.rs` with `StackHandle` and `HeapHandle` exposing the `allocator_api2::Allocator` trait.

**StackHandle / HeapHandle** -- Allocation handles for the two ends of an `Arena`. Both implement `allocator_api2::Allocator`. Passed to `allocator_api2::vec::Vec::new_in(handle)` and similar constructors to obtain arena-backed collections. The two-handle design distinguishes the two arena ends at the type level rather than through a runtime discriminator.

**DynStr** -- A dynamic string runtime variant. Represents a UTF-8 string allocated in the arena heap. Produced by native function calls. Lifetime bound to the arena, namely cleared at RESET. Cannot cross the yield boundary. Cannot reside in the data segment. May appear on the operand stack, in local bindings, and as a parameter or return value of a native function. The cross-yield prohibition is the load-bearing safety property.

**StaticStr** -- A static string runtime variant. Represents a UTF-8 string referenced from the rodata region of the loaded code image. Source-level string literals compile to `StaticStr` values. Fixed-size handle, namely an index or pointer into the constant pool. May flow anywhere admissible. Permitted at the bytecode level in the data segment, with the host responsible for validity across hot updates.

**WCMU** -- Worst-Case Memory Usage. The memory analog of WCET. Computed by `wcmu_stream_iteration()` in `src/verify.rs`. The unit of measurement is aligned bytes. Reported separately for the stack region and the heap region of the dual-end arena. The fifth Keleusma guarantee. Sound for programs without calls and without variable-iteration loops at present. The constant `VALUE_SLOT_SIZE_BYTES` (32 on the modern target) converts slot counts to bytes.

**Op::stack_growth, Op::stack_shrink, Op::heap_alloc** -- Per-instruction cost methods on `Op`. `stack_growth` and `stack_shrink` return slot counts. `heap_alloc` returns bytes. The WCMU analysis composes these to compute the per-region peak stack depth and total heap allocation.

**Native attestation** -- The host declaration of WCET cost and WCMU memory bounds for each registered native function. Set via `Vm::set_native_bounds(name, wcet, wcmu_bytes)`. Defaults are `DEFAULT_NATIVE_WCET` (10) and `DEFAULT_NATIVE_WCMU_BYTES` (0). Native functions that allocate from the arena must override the WCMU default for the analysis to remain sound.

**verify_resource_bounds** -- The function in `src/verify.rs` that checks whether the WCMU computed from a module fits within the configured arena capacity. Called from `Vm::new` and `Vm::replace_module` at load time. Programs whose WCMU exceeds the arena are rejected with a `VerifyError` describing the violation.

**Atomic function** -- A function declared with the `fn` keyword that must terminate without yielding. May call other atomic functions and native functions.

**Bounded-step invariant** -- The property that there exists a statically provable upper bound on instructions executed between any two yield points. This enables WCET analysis for safety-critical certification.

**Bump allocator** -- The arena allocation strategy. Allocations advance a pointer linearly through a contiguous buffer. Deallocation occurs only at RESET when the entire arena is cleared.

**Bytecode** -- The compiled representation of a Keleusma program. A sequence of instructions executed by the virtual machine.

**Chunk** -- A compiled function in the bytecode representation. Contains an instruction sequence, constant pool, struct templates, and metadata.

**Coroutine** -- An execution context that can be suspended via yield and resumed by the host. Keleusma scripts are coroutines.

**Cost table** -- A mapping from each bytecode instruction to its execution cost in abstract time units, implemented as `Op::cost()` in `src/bytecode.rs`. Costs are relative integer weights across five tiers: 1 for data movement, 2 for arithmetic, 3 for division and field lookup, 5 for composite construction, and 10 for function calls. Used by `wcet_stream_iteration()` for worst-case execution time analysis. These values are preliminary and subject to refinement.

**Data segment** -- The fourth memory region in the Keleusma runtime, corresponding to the conventional `.data` section of an executable. A fixed-size, fixed-layout region of mutable storage owned by the host and presented to the script as a preinitialized context. Declared in source through a singular `data` block. Read and written via `GetData` and `SetData` instructions. Persists across yield and reset boundaries. Schema may change arbitrarily across hot updates. Conceptually analogous to the persistent state of an Open Telecom Platform `gen_server` [H1, H2] and to the state vector of a SCADE mode automaton [H3, SC1]. See [EXECUTION_MODEL.md](../architecture/EXECUTION_MODEL.md) and [RELATED_WORK.md](./RELATED_WORK.md) Section 8.

**Dialogue type** -- The pair of types (A, B) that defines the yield contract between a stream program and its host. Input A is provided by the host on resume. Output B is produced by the program on yield. The dialogue type is the only invariant required across hot code swaps. Text, rodata, and the data segment schema may all change.

**Double buffering** -- The hot swap mechanism. The host loads new text and rodata into a secondary buffer while the current code continues executing. RESET activates the new buffer. The old buffer is retained for rollback.

**Guard clause** -- A boolean condition attached to a function head using the `when` keyword. Evaluated after pattern matching succeeds. Limited to comparisons and logical operators.

**Host** -- The Rust application that embeds the Keleusma VM, registers native functions, and drives coroutine execution by calling `call()` and `resume()`. The host owns the data segment storage and is responsible for installing and selecting code versions across hot updates.

**Hot code update** -- The replacement of executable code while the program continues running. In Keleusma, hot code updates occur only at RESET. Text, rodata, and the data segment schema may all change across an update. Only the dialogue type is invariant. Cross-swap data segment value handling follows Replace semantics. Atomicity is logical only. Rollback is mechanically identical to a forward update with an older code version selected. Drawn from the multi-version code coexistence model of Erlang and the Open Telecom Platform [H1, H2] with adaptation for the synchronous reset boundary of Keleusma. See [RELATED_WORK.md](./RELATED_WORK.md) Section 8.

**Keleusma** -- A Total Functional Stream Processor that compiles to bytecode. The name derives from the Greek word for a command or signal, specifically the rhythmic calls used by ancient Greek rowing masters to coordinate oar strokes.

**keleusma_type** -- A Rust attribute macro (`#[keleusma_type]`) that enforces interoperable memory layout on host types used in the dialogue signature. Ensures the host and VM agree on binary representation of A and B types. Aspirational. The current implementation uses `#[derive(KeleusmaType)]` for both ergonomic native function marshalling and dialogue type implementation.

**KeleusmaType** -- A trait defined in the runtime crate that describes the static marshalling contract between a Rust type and the runtime `Value` enum. Implemented for primitives, the unit type, fixed-arity tuples, fixed-length arrays, and `Option<T>`. Host structs and enums implement the trait through the `#[derive(KeleusmaType)]` macro from the `keleusma-macros` crate. The `from_value` method extracts the Rust value from a `Value`. The `into_value` method produces a `Value` from the Rust value. Together these support automatic argument and return-value marshalling at the native function boundary.

**IntoNativeFn** -- A trait family with one implementation per arity zero through four that allows ordinary Rust functions and closures to be registered as native functions through `Vm::register_fn`. The trait wraps the host function in the uniform `fn(&[Value]) -> Result<Value, VmError>` shape required by the VM. The companion `IntoFallibleNativeFn` covers host functions whose return type is `Result<R, VmError>`.

**register_fn** -- The Vm method that registers a host function with automatic argument and return-value marshalling. The recommended registration path for new code. Use `register_fn_fallible` for host functions that may return errors.

**Logical atomicity** -- The property that the script never observes a partial hot swap. Either the entire old image is in effect or the entire new image is in effect. Realized by requiring the new code text and rodata to be resident and the host-supplied data segment instance to conform to the new schema before the candidate is eligible for installation. Distinct from crash atomicity, which concerns recovery from a fault during the swap and is the responsibility of the host platform [H4, H5].

**Loop function** -- A function declared with the `loop` keyword that never exits. Must yield on every iteration. Exactly one per script. Serves as the coroutine entry point.

**Mode change** -- A construct in the synchronous reactive language tradition that transitions a program between distinct operating modes with associated state vectors [H3, SC1]. The Keleusma RESET boundary is the closest analogue, and the data segment is the closest analogue to the SCADE mode automaton state vector. Keleusma extends the model by permitting the schema of the state vector to change when the transition coincides with a hot code update.

**Module** -- The compiled output of the compiler. Contains chunks for compiled functions, an entry point index, enum definitions, and an optional data segment layout.

**Multiheaded function** -- Multiple function definitions with the same name and arity that form a single logical function. Dispatch selects the first head whose pattern matches the arguments.

**Native function** -- A Rust function registered by the host and callable from Keleusma scripts. Registered via `vm.register_native()` or `vm.register_native_closure()`.

**Opaque type** -- A host-defined type that scripts can pass through function calls but cannot destructure or inspect. Recognized from the native function registry.

**Phase clock** -- The coarse-grained temporal domain governed by RESET. Swap latency and arena lifetime are measured from RESET to RESET. See also: yield domain.

**Pipeline** -- The `|>` operator that passes the result of the left expression as an argument to the right function call. Supports placeholder `_` for argument position control.

**Productive divergent function** -- A function declared with the `loop` keyword. It diverges by never exiting, but is productive because it yields a value on every iteration.

**Productivity invariant** -- The property that every control path from STREAM to RESET must encounter at least one YIELD. This ensures that every iteration of a productive divergent function produces observable output. Enforced by Pass 5 of the structural verifier via abstract interpretation [AI1] over a two-element lattice. The coinductive dual of termination, as studied by Endrullis et al. [C4].

**Productivity verification** -- The static analysis pass that enforces the productivity invariant. Implemented as `analyze_yield_coverage()` in `src/verify.rs`. The analysis walks the block-structured control flow, tracking whether all paths have yielded. At If/Else branches, both branches must yield. At loops, all break-exit paths must have yielded. Programs that violate the invariant are rejected at load time.

**REENTRANT block** -- A structural ISA block type that must contain at least one YIELD. Used for logic that interacts with the host. Corresponds to non-atomic total functions and productive divergent functions in the surface language.

**Replace semantics** -- The cross-swap value-handling discipline for the data segment. The host owns the data segment storage and supplies a memory instance appropriate for the new code version at each RESET. The script observes whatever the host presents, with no obligation on the host to preserve any field across the swap. The host may keep, modify, migrate, or substitute the underlying storage transparently. The simplification relative to the Open Telecom Platform `code_change` callback model [H1, H2] is that the migration responsibility resides in the host rather than in the script.

**replace_module** -- The Vm method that performs a hot code update. The host calls it between a `VmState::Reset` and the next `call`. The new module is verified before replacement. The host supplies an initial data segment instance whose length must match the new module's declared slot count. Frames and stack are cleared so the next `call` starts the new module's entry point. The same mechanism supports forward update and rollback. Dialogue type compatibility across the swap is the host's responsibility because dialogue types are erased at the bytecode level.

**RESET** -- A structural ISA primitive that clears the arena, performs a hot swap if scheduled, and jumps to the STREAM entry point. RESET is the only instruction allowed to target STREAM and is the only global back-edge in the program. RESET is the only update point at which a hot code update may take effect.

**Soundness** -- The property that a verification pass rejects all invalid programs. A sound verifier never accepts a program that violates the property it checks. The structural verifier's soundness has not been formally proven. See [RELATED_WORK.md](./RELATED_WORK.md) Section 7 for the certification gap analysis.

**Schema** -- The number, names, and types of fields declared in a `data` block. Within a single code image the schema is fixed at compile time. Across hot updates the schema may change arbitrarily because the update applies at RESET and the host supplies a conforming data segment instance.

**Span** -- A source location record containing byte offsets, line number, and column number. Attached to tokens and AST nodes for error reporting.

**Stack quiescence** -- The property that the operand stack contains no values whose interpretation depends on the previous code version at the moment of a hot code update. In Keleusma, stack quiescence holds trivially because the operand stack is empty at RESET by construction. This contrasts with the dynamic software update literature for general-purpose C programs, where stack quiescence must be reasoned about explicitly [H4, H5].

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
