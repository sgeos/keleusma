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

- **Safety-critical control systems.** The totality guarantees of the language, bounded-step execution, and static Worst-Case Execution Time (WCET) analysis make it suitable for industrially certifiable control loops.
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

## Five Guarantees

Keleusma provides five static guarantees about program behavior.

1. **Totality.** No partial functions or undefined behavior. Every execution path terminates or yields. This follows Turner's argument for total functional programming [T1] and is enforced through recursion prohibition and bounded loops.
2. **Productivity.** Each iteration of a productive divergent function produces observable output via at least one yield. This is the coinductive dual of termination, as studied by Endrullis et al. for stream definitions [C4] and unified with termination checking by Abel and Pientka [C3].
3. **Bounded-step.** There exists a statically provable upper bound on instructions executed between any two yield points (WCET analyzable). This corresponds to the synchronous hypothesis [L1, SY1] and enables WCET analysis [WC1].
4. **Bounded-memory.** There exists a statically provable upper bound on arena memory consumed during one Stream-to-Reset cycle, separately for the stack region and the heap region. The Worst-Case Memory Usage (WCMU) analysis is the memory analog of WCET. The arena is sized to accommodate the worst case the program can produce. Programs whose WCMU cannot be statically computed are rejected at verification time. This guarantee parallels the timing bound and is required for full safety-critical certification under DO-178C and ISO 26262.
5. **Safe swapping.** Hot code swaps preserve type safety and stream continuity. Only the dialogue type must remain invariant across swaps.

## Conservative Verification

Keleusma's surface language admits the description of programs that the verifier may reject. The separation between description and admission is intentional and is part of the language's contract. This property may seem alien to readers coming from programming paradigms where successfully compiling means the program is admitted at runtime.

The compile pipeline admits a broader surface than the WCET and WCMU analyses can prove bounded. The pipeline includes the parser, the type checker, the monomorphizer, and the bytecode emitter. The verifier runs at the safe constructors `Vm::new` and `Vm::load_bytes`. It rejects any program whose execution time or memory use cannot be statically bounded. Some constructs are rejected earlier than the verifier: V0.2.0 Phase 4 moved closure-shaped expressions and first-class function references into the type-checker stage, where they surface as a diagnostic that names the construct.

Two categories of programs fall in the gap between the surface and the verifier's admittance set.

**First category, provably unbounded constructs.** A program that demonstrably admits unbounded execution at runtime falls in this category. The legacy example was a closure that dispatched to itself through indirect call. After V0.2.0 the language no longer describes closures, but the conservative-verification stance still applies to constructs the verifier cannot bound: recursive call graphs, unbounded loop iteration counts, and similar shapes are rejected.

**Second category, bounded but not yet proven constructs.** A program whose execution is bounded in fact but whose proof has not yet been implemented also falls in this category. Examples include programs that read a host-attested invocation-count bound through `register_external_native` whose chunk-level integration is forward-looking, and programs that depend on refinement-type elision passes that have not yet been generalised. Future analysis improvements can move such programs out of the rejection set without changing the surface language.

This stance differs from the conventional pattern in most programming languages. There, programs that compile typically admit runtime execution, and analysis tools layer on top to flag potential issues. In Keleusma, the verifier is the source of truth. Programs that fail verification are rejected at the safe constructor regardless of whether they would have terminated in practice. The two categories above are coherent because the language treats rejection as the safety property: a program admitted by `Vm::new` is one whose bound is proved, not one whose bound exists.

### Implications

Hosts develop scripts knowing that the verifier defines the admitted set. Programs that ship through real-time embedding must be designed within the verifier's current capability. Programs that require richer constructs and accept the unbounded risk can use `Vm::new_unchecked`, which is intentional misuse outside the WCET contract.

Tooling can highlight verifier-rejected constructs so developers see the gap before runtime.

The language can grow its admitted set without surface changes. As analysis techniques mature, more programs become admissible. Candidate techniques include flow analysis for indirect dispatch, attestation APIs for declared bounds, and inter-procedural reach extension. The surface remains stable. Only the verifier's reach changes.

The rejection-by-default stance is the dual of the conventional acceptance-by-default stance. Both are coherent design choices. Keleusma's choice follows from its safety-critical positioning. A sound bound on time and memory is the load-bearing guarantee. The safest place to draw the boundary is the analysis's current capability.

### Worked examples

Closure-shaped expressions and first-class function references are rejected by the **type checker** in V0.2.0 with a diagnostic that names the construct. The legacy verifier-stage rejection through `Op::CallIndirect` and `Op::MakeRecursiveClosure` is no longer the relevant path: those opcodes and the `Value::Func` runtime variant were retired in V0.2.0 Phase 4 along with the closure-hoisting compiler pass.

Recursive call-graph cycles are rejected by the **compiler** with a diagnostic that names the cycle. Recursion is a first-category construct: a self-referential or mutually-referential call chain admits unbounded execution within a single Stream-to-Reset slice by construction.

A `loop` function without a guaranteed `yield` on every cycle is rejected by the **verifier** as a productivity violation. The productivity rule keeps `loop` functions from spinning without progress.

A program whose declared WCET or WCMU exceeds the runtime's bound, or whose loop bound cannot be statically inferred, is rejected by the **verifier**'s resource-bounds pass. Hosts that have measured a bound out-of-band may convey it through the external-native attestation API or accept the program through `Vm::new_unchecked` (intentional misuse outside the WCET contract).

## Memory Model

Surface-language semantics. Script-defined values are conceptually immutable. Local bindings, the operand stack, and the arena are not observable as mutable state at the surface. The data segment is the sole region of mutable state observable to the script that persists beyond a single function activation; scripts read and write it through a fixed schema declared in a `data` block. Strings divide into two surface kinds. Static strings reside in the rodata region and may flow anywhere admissible. Dynamic strings reside in the arena heap, are produced by native function calls, and may not cross the yield boundary. See [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md) for the full string discipline.

Runtime layout. Memory is organized into four regions analogous to the System V ABI sections `.text`, `.rodata`, `.data`, and `.bss`, with the `.bss` region implemented as a dual-end bump-allocated arena. See [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) for the canonical region table, the source-level implementation mapping, and memory bookkeeping. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 8 for the academic and engineering precedents and citations [H1, H2, H3, SC1] for the persistent-state and mode-automaton lineage.

### Data Segment Partition

V0.2 partitions the data segment into three visibility classes that each have different storage and lifecycle. The surface syntax is a modifier on the `data` declaration.

| Class | Surface form | Storage | Host API | Lifecycle |
|-------|--------------|---------|----------|-----------|
| Shared | `shared data ctx { ... }` or `data ctx { ... }` | Vm-owned vector | `Vm::set_data`/`Vm::get_data` | Persists across RESET |
| Private | `private data state { ... }` | Arena's persistent region | Not exposed | Persists across RESET |
| Const | `const data cfg { field: T = literal, ... }` | Per-chunk constant pool | Not applicable (immutable) | Identical to bytecode lifetime |

The compiler partitions slot indices so shared slots occupy the low range and private slots the high range. The arena's persistent region holds private slots as raw `Value` structs; hosts call `vm::required_persistent_capacity_for(&module)` and `arena.resize_persistent(needed)` before constructing the VM. Const data fields compile to `Op::Const` reads from the per-chunk constant pool and do not consume runtime data slots. Programs that declare a private data block whose slots are never mutated are rejected at compile time with a diagnostic recommending the `const data` rewrite.

The framing header carries `shared_data_bytes` and `private_data_bytes` (both `u32`) in `VALUE_SLOT_SIZE_BYTES`-sized logical units. Const data does not contribute to either field.

### Ephemeral Modules

A module is **ephemeral** when no value loaded from arena memory allocated prior to a resume or entry is read by any subsequent code, and no arena-owned value flows through any yield or return that crosses the host-VM boundary. Ephemerality licenses the host to fully reset the arena between successive invocations without changing observed behaviour. The `FLAG_EPHEMERAL` bit in the framing header signals the property.

The compiler emits the flag automatically when the verifier proves the property. Programmers may opt in to a build-time assertion by declaring the entry point with the `ephemeral` modifier:

```text
ephemeral fn main(...) -> ...
ephemeral yield main(...) -> ...
ephemeral loop main(...) -> ...
```

The compile pipeline rejects the module if the assertion fails, with a diagnostic naming the reason (private data declared, or signature carries `Text`).

Sufficient verifier rule. The module is ephemeral when `private_data_bytes == 0` AND no `Text`-typed parameter of the entry function is referenced in the body AND the entry chunk does not leave a text-typed value on top of the abstract operand stack at any boundary-crossing op. The second clause is the parameter-usage refinement from Phase 7. The third clause is the per-yield arena dataflow refinement from Phase 8 and is realised by extending the existing text-size abstract interpretation pass to also peek the abstract stack at `Op::Yield` and `Op::Return`. The pass walks the call graph in topological order so per-callee text-ness is resolved by the time each caller is analysed. The text-size lattice widens to `Unbounded` inside loops and conditional branches, which preserves soundness without losing per-callee precision through the topological scan. The dataflow is sound but conservative on control flow that the lattice cannot narrow; programs that pass the previous signature-only rule continue to pass the refinement.

## Hot Code Swapping

Keleusma supports hot code swapping at the RESET boundary of a productive divergent function iteration. Only the dialogue type, namely the yield contract from A to B, must remain invariant across swaps. Text, rodata, and the data segment schema may all change across a swap, and each routine's WCET and reset-to-reset bound is certified independently. Cross-swap data handling follows Replace semantics, with the host atomically supplying the data instance appropriate for the new code version. The model parallels the Erlang and Open Telecom Platform multi-version code coexistence pattern [H1, H2], with the simplification that the migration callback resides in the host rather than in the script. See [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) for the full specification including atomicity, rollback, and stale-slot behavior.

## WCET and WCMU Analysis

Worst-Case Execution Time is measured from yield to yield. Each yield-to-yield slice must have a statically provable upper bound on instructions executed. In the absence of dynamic dispatch, every execution path is a static directed acyclic graph between yield points. WCET counts weighted opcodes on the longest path. Wilhelm et al. provide a comprehensive survey of WCET analysis methods and tools [WC1].

Worst-Case Memory Usage is measured per Stream-to-Reset iteration. The analysis computes a separate stack and heap bound. Both are summed against the arena capacity at module load.

### Units

WCET is reported in **pipelined cycles**. WCMU is reported in **bytes**.

A pipelined cycle is a CPU cycle in which the host's pipeline operates at steady-state throughput. The cycle assumes warm instruction and data caches, correctly predicted branches, and no contention from other cores or peripherals on the memory bus. The pipelined-cycle metric is what CPU optimization tables, including Agner Fog's instruction tables and the Intel Optimization Reference Manual, call "throughput" or "reciprocal throughput" per instruction. The metric is observable, reproducible, and measurable through standard benchmarking with warmed caches and a stable branch predictor.

The byte unit is target-independent in principle. The actual byte count returned by the analysis depends on the runtime's value-slot size, which the cost model carries as `value_slot_bytes`. The bundled default Keleusma runtime (`Vm<i64, u64, f64>`) declares 32 bytes per slot, a conservative bound that includes alignment padding for the runtime-tagged `GenericValue<i64, f64>` enum. Hosts targeting sub-64-bit native runtimes through the parametric `GenericVm<W, A, F>` shape consume fewer bytes per slot, although the verifier currently retains the 32-byte conservative bound regardless of the chosen `W` and `F`; see B16 in [BACKLOG.md](../decisions/BACKLOG.md) for the parametric-Vm cascade.

### What the language guarantees

The verifier proves a definitive pipelined-cycle bound for each Stream-to-Reset iteration. The bound is sound for the abstract pipelined-cycle metric. A program admitted by `Vm::new` executes a number of pipelined cycles per iteration that is at most the analyzed bound.

The language does not guarantee a wall-clock-time bound. Wall-clock time depends on the host CPU's stall behavior, clock period, and operating frequency. The language does not guarantee an actual-cycle bound. Actual cycles depend on the host CPU's stall behavior. Both gaps are the host's responsibility to characterize during deployment.

### Caveats for actual cycles

Actual cycles executed on a real host CPU exceed the pipelined-cycle bound by the host's stall budget. Stalls arise from instruction-cache misses, data-cache misses, TLB misses, branch mispredictions, speculative-execution recovery, and contention on the memory bus from other cores or peripheral DMA. The pipelined-cycle bound does not account for these costs.

The relationship between pipelined cycles and actual cycles depends on the host CPU and the deployment environment. For a worst-case-driven application running alone on a quiescent core with cache-warm inputs, actual cycles are typically within a small constant factor of the pipelined-cycle bound. For an application running in a contended environment, the factor is larger and more variable.

### Caveats for wall-clock time

Wall-clock time equals actual cycles times the clock period. The clock period is well-defined when frequency scaling is disabled. When frequency scaling is active, the cycle count is consistent but the wall-clock duration varies with operating frequency. Time-predictable platforms, including the Java Optimized Processor [WC5], reduce the gap between pipelined and actual cycles toward unity by hardware design and run at fixed frequencies, so the wall-clock time is a tight scalar of the analyzed bound.

### Bounded order-of-magnitude WCET

Keleusma proves a definitive bound in pipelined cycles. For practical applications, the pipelined-cycle bound is order-of-magnitude correct relative to the actual wall-clock WCET on a specific deployment platform. The conversion from analyzed bound to deployed wall-clock WCET is a platform-specific scalar, conventionally called the **calibration factor** or **dilation factor** in the WCET literature. The factor accounts for both the stall budget (pipelined cycles to actual cycles) and the clock period (actual cycles to wall-clock seconds).

For many practical applications, an order-of-magnitude bound is sufficient. Audio engines need to know that one stream iteration completes within the audio-buffer period. Game scripts need to know that one tick completes within the frame budget. Embedded controllers need to know that one control-loop iteration completes within the sample interval. The pipelined-cycle bound multiplied by a measured calibration factor gives an effective approximation of the worst-case wall-clock execution time. The calibration factor is established once per deployment configuration during host validation and remains stable across program updates that pass the verifier on the same platform.

The host accepts responsibility for the calibration factor. The language guarantees the pipelined-cycle bound. The host attests to the calibration factor appropriate for its deployment. This is the right place to draw the abstraction boundary because the factor depends on the host platform, the host operating environment, and the host certification process, none of which the language can determine unilaterally.

### Cost model

The `crate::bytecode::CostModel` struct carries the per-opcode pipelined-cycle table and the value-slot byte size. The bundled `NOMINAL_COST_MODEL` constant supplies unmeasured pipelined-cycle estimates suitable for relative ordering of programs on a single platform. The scale assigns one pipelined cycle to data movement and trivial control flow, two to arithmetic and comparison, three to division and field lookup, five to composite construction, ten to function calls. These values are not validated against any specific host CPU; they are intended to be replaced by measured tables during deployment validation.

Hosts construct a calibrated cost model by setting `value_slot_bytes` to the runtime's value-slot size and `op_cycles` to a function pointer that returns measured pipelined-cycle counts for the target hardware. The measured tables are obtained through standard benchmarking with warm caches and a stable predictor. The verify entry point `verify_resource_bounds_with_cost_model` accepts a custom model.

Internal threading of the host-supplied cost model through the per-chunk WCMU computation is a tracked refinement. The current implementation accepts the model parameter in the public API surface. The per-chunk computation uses the bundled nominal model. Hosts that build against the cost-model contract will see measured cycle and byte tables flow through to the bound when the threading work lands. The contract is stable.

### Limitations

Pipelined cycles do not directly correspond to actual cycles or to wall-clock time. The conversion to actual cycles requires the platform's stall budget. The conversion to wall-clock time additionally requires the clock period. Industrial WCET analysis tools such as aiT [WC2] account for pipeline effects, cache behavior, and branch prediction on the target hardware to produce a tight actual-cycle bound. For safety-critical certification, a sound wall-clock bound requires either a time-predictable execution platform [WC5] or a validated mapping from pipelined cycles to physical time. Keleusma's pipelined-cycle bound is sufficient for relative comparison of programs and, multiplied by a deployment-specific calibration factor, sufficient for soft real-time and many embedded scheduling applications. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 4 for the full discussion.

## Turing Completeness and Temporal Domains

Individual yield-to-yield slices are not Turing complete by design. The language is deliberately bounded in isolation; Turing completeness arises only from the VM-Host pair operating over the unbounded RESET cycle, with the host providing input through YIELD exchanges and persistent state across RESET serving as unbounded external memory. This separation is what makes static WCET analysis and industrial certification possible. The two temporal domains (yield-to-yield for fine-grained scheduling, reset-to-reset for coarse-grained phase control) are specified in [EXECUTION_MODEL.md](./EXECUTION_MODEL.md).

## Coroutine Model

Scripts execute as coroutines managed by the host. The host initiates execution by calling `Vm::call(args)`, which begins at the designated loop function entry point. When a script yields, execution suspends and the host receives the yielded output through `VmState::Yielded`. The host resumes execution by calling `Vm::resume(input)`. For loop functions, the input replaces the parameter slot so the next iteration operates on fresh host data. For yield functions, the resume value is returned at the yield site. The model allows scripts to operate as persistent stream processors with the host driving the schedule. Bidirectional error handling between host and script through the yield boundary follows the resume-value pattern documented in [BACKLOG.md](../decisions/BACKLOG.md) under B7.

## Native Function Interface

The host registers native functions with the virtual machine before compilation. Four registration methods are available, in order from most to least ergonomic.

**Ergonomic typed registration.** The host calls `vm.register_fn(name, func)` where `func` is an ordinary Rust function or closure of arity zero through four whose argument and return types implement `KeleusmaType`. Argument extraction, arity checking, and return value wrapping happen automatically. Use `vm.register_fn_fallible(name, func)` when the host function returns `Result<R, VmError>`. This is the recommended path for new code.

**Function pointers.** The host provides a function name and a Rust function pointer with the signature `fn(&[Value]) -> Result<Value, VmError>`. Suitable when the host function must inspect arbitrary `Value` variants.

**Closures.** The host provides a function name and a boxed closure that captures external state. Suitable for functions that need to read from or write to host resources or that must inspect arbitrary `Value` variants.

Native functions participate in the script call graph like any other function. The compiler resolves native function names during compilation, and the virtual machine dispatches to the registered implementation at runtime.

Type coercion at the native function boundary is flexible. Integer arguments are accepted where floating-point parameters are expected, with automatic widening from `Word` to `Float`. Purity of native functions is declared by the host and is not verified by the compiler. The host is responsible for ensuring that functions declared as pure do not produce side effects.

The host is responsible for verifying and certifying host functions. Native functions can define domain-specific vocabularies tailored to the target application.

### KeleusmaType and Static Marshalling

The `KeleusmaType` trait defines the static marshalling contract between Rust types and the runtime `Value` enum. Host structs and enums become implementations through `#[derive(KeleusmaType)]` from the `keleusma-macros` crate. The derive accepts named-field structs and enums whose variants may be unit, tuple-style, or struct-style. Field types compose admissible interop types per the rules in [TYPE_SYSTEM.md](../design/TYPE_SYSTEM.md).

The static marshalling approach contrasts with the dynamic approach of Rhai, which relies on `Box<dyn Any>` to carry arbitrary Rust types. Keleusma's discipline of fixed-size, fixed-layout interop types makes static dispatch sufficient and avoids the unsafe pointer manipulation and runtime type-erasure overhead of the dynamic approach. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 9 for the full comparison.

## Scope Inclusions and Exclusions

Features now implemented under V0.1.

- Hindley-Milner type inference foundation. `Type::Var`, the substitution machinery, and Robinson unification with the occurs check. `Type::Unknown` remains as a transitional sentinel for runtime-only dispatch positions; removing it requires declaring native function signatures.
- Generics. Generic functions, structs, and enums with type parameters. Trait declarations, trait bounds with `+` separator, and impl method registration with structural validation against the trait.
- Compile-time monomorphization. Function, struct, and enum specialization. Inference reach across literals, identifiers, function-call returns, method-call returns, unary and binary operators, casts, enum variants, struct constructions, tuple and array literals, if and match arms, field access, tuple-index, and array-index expressions.
Features explicitly excluded from the current design.

- Closures, anonymous functions, and first-class function references. V0.2.0 Phase 4 retired the closure surface, the closure-hoisting compiler pass, the `Value::Func` runtime variant, and the `Op::CallIndirect`, `Op::PushFunc`, `Op::MakeClosure`, and `Op::MakeRecursiveClosure` opcodes. The type checker now rejects the construct directly with a diagnostic that names it. Programs that previously dispatched through closures restructure as top-level `fn` definitions or trait methods.
- F-string interpolation and bundled text-composition natives. V0.2.0 Phase 3.5 removed the `f"text {expr}"` surface and the bundled `to_string` / `concat` / `slice` / `length` utility natives. Dynamic text composition is the host's responsibility through verified natives that allocate `Value::KStr` into the arena.
- Ownership, borrowing, and lifetimes at the surface language level. Rust's borrow checker is unnecessary because script values are conceptually immutable and the data segment is the sole mutable region.
- Recursion in `fn` and `yield` categories. Only `loop` functions admit cyclic execution, and only through the productive RESET cycle.
- Variable-iteration loops without static bounds. The verifier rejects programs whose loop iteration count cannot be bounded statically.

Hot code swapping at the bytecode level is part of the design and is described in [EXECUTION_MODEL.md](./EXECUTION_MODEL.md). Structural verification is implemented and described in [TARGET_ISA.md](../reference/TARGET_ISA.md).

Keleusma's design choices are informed by synchronous reactive language principles and are favorable for eventual safety-critical certification, but current claims of suitability for safety-critical control systems are design aspirations, not certification status. See [RELATED_WORK.md](../reference/RELATED_WORK.md) Section 7 for a gap analysis between the current implementation and industrial certification readiness.

## Surface Extensions Added in V0.2

The following language-level mechanisms were added during the V0.2 design pass and supplement the core design described above.

### Newtype Declarations

`newtype Name = Underlying;` introduces a distinct nominal type that wraps an underlying type. The bytecode representation is identical to the underlying. The type checker rejects mixing the newtype with its underlying without explicit construction or extraction. Construction at expression position uses `Name(value)`; extraction uses `value as Underlying`. Newtypes compose with refinement predicates through the `where` clause.

### Refinement Types

`newtype Name = Underlying where predicate;` augments a newtype with a user-defined predicate that the compiler emits at every construction site. The predicate must be a declared atomic-total function of signature `fn(Underlying) -> bool`. The construction site is compiled to a sequence that calls the predicate and traps on a false result. The runtime cost is paid at every construction; compile-time elision when the argument is provably in range is a follow-on optimisation.

A refined newtype may additionally declare saturation contracts through the `with saturate_max = N` and `with saturate_min = M` clauses. The contract is consulted by the numeric overflow construct described below; the underlying type and the refinement predicate are unaffected.

### Numeric Overflow Construct

`expr { ok(p) => ..., overflow(ph, pl) => ..., underflow(ph, pl) => ... }` guards a single arithmetic operation against overflow and underflow. The supported operations are `+`, `-`, `*`, `/`, `%`, and unary `-` on Word operands. The runtime computes the true result in the next-larger signed integer (`W::Wide`: `i16` for `Word = i8`, `i32` for `i16`, `i64` for `i32`, `i128` for `i64`) and pushes `(high, low, flag)` so arm patterns can destructure the high and low halves; this is the load-bearing mechanism for big-number arithmetic. On the default `i64` runtime, the widened type is `i128`.

Arms follow pattern-match semantics. Each pattern position admits the wildcard `_`, a bare identifier (binds a `Word`), or a signed integer literal (matches by equality). An optional `when expr` guard between the pattern and the `=>` is checked as `Bool` and falls through to the next arm when false. Each outcome class (`ok`, `overflow`, `underflow`) must end in an unguarded catch-all arm. Multiple specialized arms per class are admitted as long as the catch-all is the last covering arm.

The `saturate_max` and `saturate_min` keywords inside arm bodies denote context-determined saturation values. When the surrounding expected type is `Word` (the default), they resolve to `Word::MAX` and `Word::MIN` respectively. When the surrounding expected type is a refined newtype declared with a `with saturate_max` or `with saturate_min` clause, the keyword resolves to a constructor call against the declared literal; the refinement predicate is verified at runtime on that literal exactly as for any other constructor invocation. The bidirectional propagation draws on annotated `let` bindings and on function return types; the expected type is the top of a static expected-type stack maintained during type checking.

### Information-Flow Labels

Types may carry a set of user-defined information-flow labels written as `T@Label` for a single label or `T@{L1, L2}` for multiple. The empty label set is the pure state; more labels indicate more restrictions. Two operators relabel values: `classify expr@Label` adds labels (always admitted because adding labels only tightens flow restrictions), and `declassify expr@Label` removes labels (always admitted but constitutes an explicit information-disclosure audit point that an operator's review process should track).

Labels propagate through every position of a composite type independently. Arithmetic on labeled operands unions the operand labels into the result. Branching on a labeled condition propagates the condition's labels to the result, because an observer of the result can infer information about the condition. The label-flow rule at every position is `source.labels ⊆ target.labels`; violations are rejected at compile time. The mechanism is zero-cost at the bytecode layer; the bytecode is identical regardless of label annotations.

Native function signatures admit labels in both parameter and return positions: `use host::transmit(payload: Word@Open) -> Status` rejects calls that pass a labeled value without explicit declassification.

## Cross-References

- [GRAMMAR.md](../design/GRAMMAR.md) provides the formal EBNF grammar specification.
- [EXECUTION_MODEL.md](./EXECUTION_MODEL.md) describes the target execution model with temporal domains.
- [TARGET_ISA.md](../reference/TARGET_ISA.md) describes the structural ISA specification.
- [BACKLOG.md](../decisions/BACKLOG.md) records features that fall outside the verifier's current admittance set. B3 closures, B14 CallIndirect flow analysis, and related entries record V0.1-era investigations that V0.2.0 retired.
- [RELATED_WORK.md](../reference/RELATED_WORK.md) positions Keleusma within the academic and industrial landscape.

## Citation Key

Citations in this document use bracket notation (e.g., [SY1], [C1]) referring to entries in the bibliography in [RELATED_WORK.md](../reference/RELATED_WORK.md).
