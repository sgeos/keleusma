# Resolved Decisions

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Completed design and architectural decisions with rationale.

## R1. no_std + alloc target

The crate targets `no_std` with `alloc` to maximize portability. It can run in embedded, WASM, and standard environments without modification. The only external dependency is `libm` for math functions. This constraint ensures the language runtime imposes no operating system requirements on the host application.

## R2. Stack-based virtual machine

A stack-based VM was chosen over a register-based VM for simplicity of implementation and natural support for expression evaluation. Stack-based architectures map directly to the recursive structure of arithmetic and function call expressions. The stack model also simplifies coroutine state preservation across yields, since the entire evaluation state lives on a single stack that can be captured and restored.

## R3. Three function categories

Functions are categorized as `fn` for atomic total functions, `yield` for non-atomic total functions, and `loop` for productive divergent functions. This categorization enables static verification of termination and productivity guarantees without a full type checker. The compiler can enforce that `fn` functions never yield, that `yield` functions always terminate, and that `loop` functions yield on every iteration.

## R4. Recursion prohibition

All forms of recursion are rejected at compile time. The compiler detects cycles in the call graph and reports them as errors. Recursive algorithms must be supplied by the host as native functions. This simplifies termination analysis by ensuring that the call graph is a directed acyclic graph, which makes it possible to verify termination through topological ordering alone.

## R5. No closures or anonymous functions

Closures and anonymous functions are excluded to keep the VM simple. All functions are named and defined at the top level of a module. Higher-order patterns are achieved through multiheaded function dispatch, which allows a single function name to match different argument patterns. This avoids the need for captured environments and upvalue management in the VM.

## R6. libm as sole dependency

The `libm` crate provides math functions such as `sin`, `cos`, `pow`, and `log10` in `no_std` environments. No other external dependencies are used. This minimizes the supply chain surface and ensures the crate can compile in any environment that supports `alloc`.

## R7. Curly brace block delimiters

Keleusma uses curly braces for block delimitation rather than `do`/`end` or significant indentation. This is consistent with the Rust host language and reduces parser ambiguity. Curly braces provide unambiguous block boundaries without requiring whitespace sensitivity in the lexer.

## R8. Semicolons for statement termination

Semicolons are required to terminate statements, following Rust conventions. The last expression in a block is the return value and does not require a trailing semicolon. This convention provides clear visual separation between statements while preserving expression-oriented block semantics.

## R9. Host-declared purity

Purity of native functions is declared by the host at registration time, not verified by the compiler. Analysis trusts the declaration. Impurity is transitive through the call graph, meaning any function that calls an impure function is itself considered impure. Since native functions execute arbitrary host code, the compiler cannot verify their purity and must rely on the host to declare it honestly.

## R10. Single module per file

Each script file constitutes one module. Modules cannot import other Keleusma modules. All external functionality comes from native function registrations provided by the host. This eliminates the need for a module resolution system, dependency tracking, or linking phase. Composition happens at the host level by registering different sets of native functions for different scripts.

## R11. .kma file extension

Script files use the `.kma` file extension. This provides a distinctive identifier for tooling, editor support, and file association without conflicting with other language extensions.

## R12. Stream coalgebra model

Top-level productive divergent functions model stream transformations of the form f : Stream<A> -> Stream<B>. This coalgebraic formulation enables mathematical reasoning about infinite stream transformations and provides a formal foundation for productivity proofs. Helper functions may yield but must share the top-level function's dialogue type.

## R13. Arena memory model

The VM uses an arena memory model consisting of a single contiguous bump-allocated buffer. The stack grows from one end. There is no heap initially. The arena persists across yields within a single stream phase but is cleared at the top of every productive divergent function iteration (the RESET boundary) by resetting the bump pointer. This prevents memory leaks, ensures predictable resource usage, and eliminates memory debt across mission phases. Memory bounds are statically analyzable per stream phase. See R20 for implementation details.

## R14. Two temporal domains

Execution is governed by two temporal domains. The yield domain (control clock) provides fine-grained scheduling with WCET measured yield-to-yield. The reset domain (phase clock) provides coarse-grained phase control with swap latency measured reset-to-reset. This separation allows independent analysis and certification of timing properties at each granularity.

## R15. Structural ISA verification

Programs are verified at load time via block-graph coloring. The structural ISA uses block types (STREAM, REENTRANT, FUNC, LOOP_N) that make invalid or unproductive programs impossible to define. A linear scan verifies that all paths from STREAM to RESET contain at least one YIELD and that all FUNC blocks are free of yields. Invalid programs are rejected before execution begins.

## R16. Stack machine execution

The VM is a stack machine. Individual time slices are not Turing complete. Each yield-to-yield slice executes a bounded number of instructions and then suspends. The VM-Host pair is Turing complete via the unbounded RESET cycle with the host providing the tape through YIELD exchanges. Host-controlled state that persists across resets serves as the unbounded external memory.

## R17. No flat jumps

All control flow uses block-structured instructions (If/Else/EndIf, Loop/EndLoop, Break/BreakIf). Flat JMP and BRANCH instructions are prohibited. Every forward or backward transfer of control is mediated by a matching block delimiter. This constraint ensures that the control flow graph can be statically verified through block nesting alone.

## R18. Surface language compiles down

The surface language (pattern dispatch, pipelines, dynamic types) is syntactic sugar. The compiler lowers rich surface constructs to austere certifiable bytecode. The surface language does not narrow. The bytecode ISA is deliberately minimal and verifiable, while the surface language provides developer ergonomics.

## R19. Double-buffered hot swap

Hot code swapping uses double buffering. The host loads new text and rodata into a secondary buffer while the current code continues executing in the primary buffer. RESET activates the new buffer by swapping primary and secondary. The old buffer is retained for rollback if the host determines that the new code should be reverted.

## R20. Arena implementation

The arena is a single contiguous allocation with bump allocation. The stack grows from one end. There is no heap initially. Allocations advance a pointer linearly through the buffer. Deallocation occurs only at RESET when the entire arena is cleared by resetting the bump pointer.

## R21. Immediate ISA transition

The structural ISA (Stream, Reset, Func, Reentrant, block-structured control flow) replaces the previous 48-instruction flat-jump bytecode immediately rather than as a future phase. The transition includes replacing flat jumps with block-structured If/Else/EndIf and Loop/EndLoop/Break/BreakIf, replacing TestEnum and TestStruct (which contained jump offsets) with IsEnum and IsStruct (which push booleans), and adding Stream and Reset instructions.

## R22. Structural ISA implementation complete

The structural ISA transition (P4, R21) is complete. The compiler emits block-structured bytecode. The VM executes block-structured control flow natively. The structural verification pass (`verify()` in `src/verify.rs`) validates all compiled modules at load time through five checks: block nesting validation, offset bounds checking, block type constraint enforcement (Func, Reentrant, Stream), break containment verification, and the productivity rule. Programs that fail verification are rejected before execution begins.

## R23. WCET analysis and productivity verification

Static analysis tooling for worst-case execution time (P5) and productivity verification is implemented. Each bytecode instruction carries a relative integer cost via `Op::cost()`, assigned across five tiers: 1 for data movement and control flow markers, 2 for arithmetic and comparisons, 3 for division and field lookup, 5 for composite value construction, and 10 for function calls. The `wcet_stream_iteration()` function computes the worst-case total cost of one Stream-to-Reset iteration by recursively analyzing block-structured control flow, taking the maximum cost branch at each join point. The productivity rule is enforced as Pass 3 of the structural verifier: abstract interpretation over a two-element lattice tracks whether all control flow paths from Stream to Reset pass through at least one Yield. Programs that violate productivity are rejected at load time.

## R24. Data segment as fourth memory region

The Keleusma runtime memory layout corresponds to the four conventional executable sections of the System V Application Binary Interface, namely `.text`, `.rodata`, `.data`, and `.bss`. Bytecode chunks correspond to `.text`. The constant pool, struct templates, and enum definitions correspond to `.rodata`. The host-supplied preinitialized context corresponds to `.data` and is referred to as the data segment. The arena and operand stack correspond to `.bss`. The data segment is the sole region of mutable state observable to the script that persists beyond a single function activation. All script-defined values, including local bindings, are conceptually immutable. The data segment is read and written through `GetData` and `SetData` instructions that address slots by index. The `.data` analogy and the persistent state model draw on the Erlang and Open Telecom Platform multi-version code coexistence pattern [H1, H2] and on mode automata in the synchronous reactive language tradition [H3, SC1].

## R25. Schema fixity within image and schema mutability across hot updates

The data segment schema, namely the number, names, and types of declared fields, is fixed at compile time within a single code image. The schema may change arbitrarily across hot updates because hot updates occur only at RESET, where no script invariant spans the boundary on the script side. Cross-yield value preservation is not guaranteed because the host may write to the segment between yields. Cross-call value preservation within a single image is guaranteed because the script holds exclusive ownership during execution. The relaxation from a previously specified layout invariance across swaps is supported by the spacecraft control use case in which a new control script may have more or less mutable state than its predecessor.

## R26. Replace semantics for cross-swap value handling

Cross-swap value handling for the data segment follows Replace semantics. The host owns the data segment storage and supplies a memory instance appropriate for the new code version at each RESET. The script observes whatever the host presents. The host may keep, modify, migrate, or substitute the underlying storage transparently. From the script's point of view, the data segment seen after RESET is whatever the host installs. There is no `code_change` callback within the script. Migration responsibility resides entirely with the host. This is the simplification of the Open Telecom Platform model [H1, H2] consistent with the broader Keleusma division of concerns in which the script is austere and the host is rich.

## R27. Logical atomicity of hot swap

Hot swap atomicity is logical only. The new code text and rodata must be resident in memory and the host-supplied data segment instance must conform to the new schema before the candidate is eligible for installation. The host writes the candidate slot. The VM reads the slot at the next RESET and applies the swap as a single transition from the script's point of view. Crash atomicity, namely recovery from a fault that interrupts the swap, is the responsibility of the host platform and is out of scope for the VM specification. The Ksplice and Kitsune literature treats this question in detail [H4, H5]. Rollback is mechanically identical to a forward update with an older code version selected. After a rollback, the host must mark the rejected version as ineligible or operate in a rollback mode so that the VM does not automatically reinstall the rejected candidate at the next opportunity.

## R28. Singular data block per program

A program may declare zero or one `data` block. The grammar admits the syntactic form of multiple data declarations, but the compiler emits an error if more than one block is declared. This decision follows the philosophy of "boring code that does exciting things," in which the script presents a single coherent context type T to the host. Future extension to multiple blocks composed into a single segment is admissible but is not part of the current specification.

## R29. Host interoperability layer is slot-based

The host interoperability layer for the data segment is a slot-based `Vec<Value>` interface rather than a `repr(C)` Rust struct mapping. The host stores its application-level state in any Rust struct it prefers and marshals between that struct and the slot vector at the YIELD and RESET boundaries. The choice avoids unsafe pointer manipulation, keeps the runtime consistent with the rest of the VM where every value is a `Value` enum, and requires no new infrastructure. The Vm exposes `set_data`, `get_data`, `data_len`, and `replace_module` for host use. Schema mismatch detection at swap time is by size check plus host attestation. Hash comparison and structural type checking against a schema descriptor are deferred. Single-ownership concurrency is enforced by the Rust borrow checker because the host cannot hold a mutable reference to the VM while `call` or `resume` is running.

## R30. Static marshalling for native function ergonomics

Native function registration uses static marshalling through the `KeleusmaType` trait and the `#[derive(KeleusmaType)]` macro rather than the dynamic `Box<dyn Any>` approach used by Rhai and similar dynamically typed embedded scripting engines. The discipline of fixed-size, fixed-layout interop types, established for the data segment and extended to native function arguments and return values, makes static dispatch sufficient. The crate is converted to a Cargo workspace with a `keleusma-macros` proc-macro crate that hosts the derive. The `IntoNativeFn` and `IntoFallibleNativeFn` trait families provide automatic argument-extraction, arity-checking, and return-wrapping glue at arities zero through four. The user-facing entry points are `Vm::register_fn` for infallible host functions and `Vm::register_fn_fallible` for host functions whose return type is `Result<R, VmError>`. The pre-existing `register_native` and `register_native_closure` remain available for host functions that must inspect arbitrary `Value` variants, including the `to_string`, `length`, and `println` utilities that consume any value. The static approach avoids the unsafe pointer manipulation and runtime type-erasure overhead of the dynamic approach and is amenable to qualification under safety standards because no cast site requires trust at runtime.

## R31. Worst-case memory usage as the fifth guarantee

Bounded-memory becomes the fifth Keleusma guarantee, peer to totality, productivity, bounded-step, and safe swapping. Programs whose worst-case memory usage cannot be statically computed are rejected at verification time. The unit of measurement is aligned bytes. Each bytecode instruction carries a memory footprint declaration via a method paralleling `Op::cost()`. The analysis recursively traverses the block-structured control flow taking the maximum at each branch and summing along sequential paths, mirroring `wcet_stream_iteration()`. The host-attestation surface widens. Each native function is declared with both a worst-case execution time and a worst-case memory usage. Auto-arena sizing follows from the WCMU computation, namely the arena is sized to accommodate the worst case the program can produce. Industrial certification standards including DO-178C and ISO 26262 routinely require both timing and memory bounds, so adding WCMU brings Keleusma into closer parity with the safety-critical analysis tradition.

## R32. Dual-end arena with separate stack and heap WCMU bounds

The arena is a single contiguous allocation with two pointers growing toward each other from opposite ends. Stack allocations grow from one end. Heap allocations grow from the other. The two are reported and verified separately at compile time as `stack_wcmu` and `heap_wcmu`. The arena size must satisfy `arena_size >= stack_wcmu + heap_wcmu`. The verifier checks the inequality. There is no fixed boundary between stack and heap regions. Either may use any portion of the arena that the other has not consumed. Allocation fails when the two pointers would meet, producing a runtime error that the host handles. The stack continues to grow with operand pushes during expression evaluation. The heap holds dynamic strings and any other arena-allocated values introduced in future milestones. RESET clears both pointers atomically.

## R33. Modern 64-bit target assumption for V0.0

The current development cycle assumes a modern 64-bit target. Type sizes and alignments are fixed. `i64` is 8 bytes with 8-byte alignment. `f64` is 8 bytes with 8-byte alignment. `bool` is 1 byte with 1-byte alignment. `()` is 0 bytes with 1-byte alignment. Aggregates use C-style alignment rules with padding inserted as needed. Native function memory attestation is in aligned bytes on the same target assumption. Future work expands the type system with `word`, `byte`, `bit`, and `address` primitives and parameterizes the compiler over a target descriptor, enabling code generation for architectures from the 6502 to ARM64. That expansion is recorded as B6.

## R34. Arena allocator implementation with allocator-api2

The dual-end arena specified in R32 is implemented as the `Arena` type in `src/arena.rs`. The arena owns a fixed-size `Box<[u8]>` backing buffer. Two `Cell<usize>` pointers track the stack-end and heap-end allocation cursors. Allocation is constant-time and respects layout alignment. Reset clears both pointers atomically. The arena is single-threaded and uses `Cell` rather than atomics, consistent with the single-threaded VM model.

Two handle types `StackHandle` and `HeapHandle` borrow the arena and implement the `allocator_api2::Allocator` trait. The handles are passed to `allocator_api2::vec::Vec::new_in` and similar constructors to obtain arena-backed collections. The two-handle design distinguishes the two arena ends at the type level rather than through a runtime discriminator.

The `Vm` struct holds an `Arena` field initialized at construction with a default capacity of 65536 bytes. The capacity is configurable via `Vm::new_with_arena_capacity`. The `arena()` and `arena_mut()` accessors expose the arena to host-supplied native functions. The arena is reset at every `Op::Reset` boundary and at every `replace_module` call.

The deeper integration of the operand stack and dynamic-string storage with the arena is recorded as P7 follow-on work and is iterative rather than atomic. Stable Rust does not provide a `String` type with a custom allocator, so a custom `DynStr` storage type backed by `allocator_api2::vec::Vec<u8, A>` is required for full integration. The current state has the arena present and reset on schedule, but operand stack and string storage continue to use the global allocator.

The dependency on `allocator-api2` adds about 0.04 megabytes of dependency code and no runtime cost. The crate is a stable polyfill of the unstable `core::alloc::Allocator` trait. When the standard trait stabilizes, the dependency may be removed in favor of the standard library.

## R38. Strict-mode bounded-iteration loop analysis for WCMU and WCET

The WCMU and WCET analyses operate in strict mode for loops. A loop whose body falls through to its EndLoop must have its iteration count statically extractable through the canonical bytecode patterns. Loops whose body always exits via Break or Trap are accepted with iteration count one because the body executes at most once. All other loops are rejected at verification time.

The helper `extract_loop_iteration_bound` in `src/verify.rs` recognizes two patterns. The for-range pattern uses `Loop GetLocal(var) GetLocal(end) CmpGe BreakIf body... EndLoop` with `var` and `end` set by literal constants. The for-in over literal array pattern uses `NewArray(n) SetLocal(arr) GetLocal(arr) Len SetLocal(end) ... Loop GetLocal(idx) GetLocal(end) ...`, and the helper chases through `GetLocal -> SetLocal` aliasing chains so that for-in over a let-bound literal array is recognized. The iteration count is computed as `end - start` for non-negative integer bounds.

`Op::Trap` is treated as a path-exit similar to `Op::Break`. The path does not fall through; it does not propagate to the enclosing loop's break states. This is the correct semantics for the match expression's virtual loop, whose no-arm-matched fallback reaches a Trap.

The return types of `wcmu_region`, `wcmu_subregion`, and `wcet_region` change to `Result<Option<...>, VerifyError>` to propagate strict mode rejection. The `Option` distinguishes fall-through from path-exit; the `Result` carries the rejection error.

Strict mode is mandatory rather than optional. There is no permissive variant. Programs that the analysis cannot bound are not accepted. This trade-off favors soundness over expressiveness, consistent with Keleusma's stated certification posture.

## R37. Call-graph integration and auto-arena sizing for WCMU

The WCMU analysis is extended to walk the call graph in topological order. Per-chunk WCMU is computed bottom-up, with each chunk's bound including the transitive contributions of any chunks it calls and any host-attested native heap usage. The function `verify::module_wcmu(module, native_wcmu)` returns the per-chunk results. The function `verify::verify_resource_bounds_with_natives(module, capacity, native_wcmu)` checks each Stream chunk's budget against the configured arena capacity using the new analysis.

Auto-arena sizing is implemented as `Vm::new_auto(module)`, which computes the largest WCMU sum across Stream chunks under default native attestations and sizes the arena accordingly. `Vm::auto_arena_capacity()` returns the same value for an existing VM under current native attestations, useful for diagnostics.

Re-verification with current native bounds is provided by `Vm::verify_resources()`. The host calls this after registering natives and declaring their WCET and WCMU through `Vm::set_native_bounds`. The default attestation at registration time is zero heap, which is a sound under-bound for natives that allocate. Hosts must override the default for natives that consume arena memory, or the verification produces an underestimate.

The call-graph analysis enforces the no-recursion rule (R4) by detecting cycles during the topological sort. Programs that violate the rule are rejected with a clear error.

Variable-iteration loops are still treated as one iteration. The mismatch between the bytecode loop shape and the source-language for-range bounds is the responsibility of a separate analysis pass tracked as P9.

## R36. Arena extracted to standalone keleusma-arena crate

The dual-end bump-allocated arena specified in R32 and implemented in R34 is extracted to a standalone workspace member named `keleusma-arena`. The crate is positioned as a general-purpose embedded arena allocator with a differentiated value proposition from `bumpalo`, namely fixed-size storage, fail-fast allocation, dual-end discipline, generic budget contract, and `core`-only operation without `alloc`.

API changes from the in-tree arena. `StackHandle` and `HeapHandle` rename to `BottomHandle` and `TopHandle`. The keleusma runtime preserves the old names as backwards-compatible aliases at the crate root. The arena gains three constructors. `Arena::with_capacity` allocates from the global allocator when the `alloc` feature is on. `Arena::from_static_buffer` borrows a `&'static mut [u8]` for embedded targets with link-time-allocated buffers. `unsafe fn Arena::from_buffer_unchecked(ptr, len)` accepts arbitrary buffers under the caller's lifetime guarantee.

New API surface. `Budget` struct with `bottom_bytes` and `top_bytes` fields and `total()` saturating sum. `Arena::fits_budget(budget)` for admissibility check. `BottomMark` and `TopMark` types for LIFO discipline. Safe `Arena::bottom_mark()` and `Arena::top_mark()` snapshots. Unsafe `Arena::rewind_bottom`, `Arena::rewind_top`, `Arena::reset_bottom`, `Arena::reset_top` operations. Peak watermark tracking with `Arena::bottom_peak()`, `Arena::top_peak()`, and `Arena::clear_peaks()`. The unsafe variants are marked unsafe because they invalidate the rewound region while raw pointers obtained through the `Allocator` trait may still be retained by the caller.

Generic budget contract. The `Budget` type lives in the arena crate so that independent users can compute their own budgets through profiling, manual analysis, or any other mechanism, and use the arena's `fits_budget` to verify admissibility. The keleusma runtime computes its budget from a static analysis of bytecode through a new `verify::budget_for_stream` adapter that produces a `keleusma_arena::Budget`. The `verify_resource_bounds` function uses the arena's `fits_budget` for the actual check.

Tagline. "Simple and boring memory allocator for exciting applications." The phrase aligns with Keleusma's overall philosophy of boring code that does exciting things.

## R35. WCMU instrumentation and host attestation widening

The fifth Keleusma guarantee, namely bounded-memory specified in R31, is implemented as `wcmu_stream_iteration` in `src/verify.rs`. The function parallels `wcet_stream_iteration` and walks the same block-structured control flow graph. It returns a tuple of stack and heap WCMU bounds, both in bytes, computed using the per-instruction cost methods on `Op`.

Per-instruction methods. `Op::stack_growth()` and `Op::stack_shrink()` return the instruction's effect on the operand stack in slots. `Op::heap_alloc(chunk)` returns the bytes allocated to the arena heap, parameterized by the chunk for instructions whose operand resolves to a struct template. The constant `VALUE_SLOT_SIZE_BYTES` is set to 32 on the modern 64-bit target and converts slot counts to bytes.

Aggregation rules. Sequential composition sums heap totals and computes the running peak of stack depth. Branches take the maximum peak across the two arms and the maximum heap total. Loops are treated as one iteration, mirroring the existing WCET limitation. Programs that compile from bounded for-range loops produce sound bounds at the static iteration count, but the analysis underestimates by the iteration factor at present. The same limitation exists for transitive function calls, namely the local stack effect of the call instruction is counted but the called function's own contribution is not.

Host attestation. Native function entries gain `wcet` and `wcmu_bytes` fields, defaulted to `DEFAULT_NATIVE_WCET` (10) and `DEFAULT_NATIVE_WCMU_BYTES` (0) respectively. The host calls `Vm::set_native_bounds(name, wcet, wcmu)` after registration to attest the actual bounds. The defaults are conservative for timing and zero for memory, matching the assumption that natives that do not allocate need no further declaration. Native functions that do allocate from the arena must override the WCMU default for the analysis to remain sound. This widens the host trust boundary established in R9.

Module-load enforcement. The new `verify_resource_bounds(module, arena_capacity)` function computes WCMU for every Stream chunk and checks that `stack_wcmu + heap_wcmu <= arena_capacity`. Programs that exceed the bound are rejected at `Vm::new` and `Vm::replace_module`. The check is sound for programs without calls and without variable-iteration loops, with the limitations noted above.

Auto-arena sizing is not yet implemented. The host configures arena capacity at VM construction. A future iteration could compute the WCMU sum at module load and size the arena automatically. This is recorded as P8 follow-on.

## R39. Precompiled bytecode loading and trust-based verification skip

Compiled Keleusma modules can be serialized to a self-describing byte form and loaded back at runtime through `Module::to_bytes` and `Module::from_bytes`. The wire format is a sixteen-byte header followed by the rkyv-encoded module body followed by a four-byte little-endian CRC-32 trailer. The header consists of the four-byte magic `KELE`, a little-endian sixteen-bit version, a little-endian thirty-two-bit total framing length including the header and the trailer, an eight-bit word size encoded as the base-2 exponent, an eight-bit address size encoded as the base-2 exponent, and four reserved bytes set to zero. The reserved bytes pad the header so the rkyv body begins at an eight-byte-aligned offset within the buffer when the buffer base is itself eight-byte-aligned, which is required for zero-copy access through `rkyv::access`. The minimum framing size is twenty bytes. The header allows the runtime to reject foreign or incompatible bytecode at load time before any deserialization is attempted. The trailer detects bit-level corruption anywhere in the framed range. Bytecode version 4 is paired with `BYTECODE_VERSION = 4` in the crate. A change to any serialized type or to the header layout bumps the version.

The body format is rkyv rather than postcard. The choice was made to enable the planned zero-copy execution path (P10, path B). Rkyv produces a self-relative addressable layout that supports in-place access without deserialization. Rkyv supports `no_std` plus `alloc`. The recursive `Value` type uses `#[rkyv(omit_bounds)]` on self-referential fields (`Tuple`, `Array`, `Struct.fields`, `Enum.fields`) and explicit `serialize_bounds`, `deserialize_bounds`, and `bytecheck(bounds(...))` attributes to break the type-level recursion in the macro expansion.

The current `Module::from_bytes` path (path A) copies the body bytes into an `rkyv::util::AlignedVec<8>` before calling `rkyv::from_bytes`. The copy ensures alignment regardless of the host slice's alignment.

A second path is now available for hosts that supply an aligned buffer. `Module::access_bytes` validates the framing and returns a borrowed `&'a ArchivedModule` through `rkyv::access` without copying the body. `Module::view_bytes` validates through `access_bytes` and deserializes to an owned `Module` for compatibility with the existing execution loop. The corresponding `Vm::view_bytes` and `unsafe Vm::view_bytes_unchecked` constructors compose the validation with the safe and unchecked Vm constructors. The view path requires the body to be 8-byte aligned within the slice. Because the header is 16 bytes, the body is 8-byte aligned when the slice base itself is 8-byte aligned. Hosts arrange this through `rkyv::util::AlignedVec`, through a static buffer with `#[repr(align(8))]`, or through linker placement of bytecode in a section that aligns to at least 8 bytes.

The execution loop continues to operate on the deserialized owned `Module`. True zero-copy execution where the runtime reads from `&ArchivedModule` directly without ever materializing an owned form is the next iteration of P10. That iteration adds a lifetime parameter to the `Vm`, rewrites the execution loop to use archived accessors, and either rewrites the verifier to operate on `&ArchivedModule` or restricts zero-copy execution to the unchecked path.

The recorded length is authoritative. Trailing bytes after the recorded length are ignored by the deserializer, which supports embedding bytecode within a larger buffer such as a flash region with padding or a multi-segment archive. The slice passed to `from_bytes` may exceed the recorded length without rejection. Slices shorter than the recorded length are rejected as `Truncated`. The recorded length must be at least the minimum framing size or the slice is rejected as `Truncated`.

The word size and address size record the assumptions the compiler made about the host runtime when emitting the bytecode. The fields are stored as base-2 exponents. Actual width in bits is `1 << field`. The current Keleusma runtime is built for sixty-four-bit words and sixty-four-bit addresses, so `RUNTIME_WORD_BITS_LOG2` and `RUNTIME_ADDRESS_BITS_LOG2` are both six.

The acceptance policy is bytecode-exponent less than or equal to runtime-exponent. Bytecode compiled for a narrower target runs on a wider runtime. A program compiled for thirty-two-bit words runs on a sixty-four-bit runtime under the integer masking pass described below. A program compiled for sixty-four-bit words is rejected on a thirty-two-bit runtime with `WordSizeMismatch` because the runtime cannot represent the wider integers. The same policy applies to addresses through `AddressSizeMismatch`.

The encoding restricts widths to powers of two. The covered set is one, two, four, eight, sixteen, thirty-two, sixty-four, one-hundred-twenty-eight, and two-hundred-fifty-six bits at exponents zero through eight. The restriction excludes non-power-of-two architectures such as twenty-four-bit DSPs and thirty-six-bit historical machines. Keleusma's stated target range from 6502 through ARM64 is entirely powers of two, so the restriction is acceptable.

The VM applies sign-extending integer truncation to arithmetic results when the bytecode declares a word size narrower than the runtime supports. The truncation is `(value << shift) >> shift` where `shift = 64 - (1 << word_bits_log2)` and the right shift is arithmetic on i64. The truncation is applied after `Add`, `Sub`, `Mul`, `Div`, `Mod`, and `Neg` for integer operands. The float and string paths are unaffected. When the declared width matches or exceeds sixty-four bits, the truncation is the identity and adds no overhead. The masking pass implements the narrower-on-wider semantics so that arithmetic overflow points match the bytecode's declared width on the wider runtime.

The fields prepare the runtime for B10 (target portability), under which the compiler will accept a target descriptor and emit bytecode for various architectures.

The CRC-32 uses the standard IEEE 802.3 reflected polynomial `0xEDB88320`, init `0xFFFFFFFF`, refin and refout true, and xor-out `0xFFFFFFFF`. The implementation is a hand-rolled bit-by-bit loop that fits in fifteen lines of source. Algebraic self-inclusion is achieved through the residue property of this CRC parameterization. Computing the CRC over a byte sequence followed by the little-endian encoding of that sequence's CRC yields a fixed residue constant `0x2144DF1C`. The verifier runs the CRC once over the entire byte slice including the trailer and checks for the residue constant in a single linear pass. The trailer is conceptually part of the checksummed range without requiring zero-fill or position-aware special casing during verification.

Endian portability is by construction. All multi-byte integer fields in the header and trailer are stored in explicit little-endian order through `to_le_bytes` and `from_le_bytes`. Postcard's wire format stores `f32` and `f64` in little-endian raw bytes and uses varints for all other multi-byte integer types. Varints are byte-by-byte and naturally endian-agnostic. The CRC-32 algorithm operates one byte at a time and is endian-agnostic. A bytecode produced on a little-endian host and a bytecode produced on a big-endian host with the same compiler input will be identical byte sequences. The `bytecode_golden_bytes_for_main_returning_one` test pins the expected serialized form of a minimal program to guard against unintended wire format drift and endian-dependent code paths.

The deserialization input is a `&[u8]` slice. The slice may originate from any addressable buffer including in-memory `Vec<u8>` data, file-loaded buffers, or `&'static [u8]` data placed in the `.rodata` section of a host binary. Section placement is the host's responsibility. The runtime crate accepts byte slices from any source and is `no_std` plus `alloc`. File loading is left to the host because bringing `std::fs` into the runtime crate would compromise the `no_std` posture.

The Module type holds owned heap data after deserialization, so the parsed form does not borrow from the input slice. True zero-copy execution where the runtime Module borrows directly from the input buffer is recorded as P10 and deferred. The current implementation supports the full request for runtime loading from any addressable source. The `.rodata` use case is covered because the bytecode buffer can live in `.rodata` even though the parsed form is heap-allocated.

Trust-based verification skip is provided through `unsafe fn Vm::new_unchecked`, the convenience `unsafe fn Vm::load_bytes_unchecked`, and the arena-capacity-bearing variant `unsafe fn Vm::new_unchecked_with_arena_capacity`. All three run structural verification on the module because the VM execution loop relies on those invariants for memory safety, but skip the worst-case-execution-time and worst-case-memory-usage bounds checks. The unsafe marker captures the trust contract. The host attests that the bytecode was previously verified or originates from a trusted compiler. The bounded-memory and bounded-step guarantees are weakened to host attestation under this path. Exceeding the bound at runtime produces an arena allocation failure error rather than memory unsafety, so the unsafe marker captures the loss of contract rather than a memory-safety risk. Structural verification is retained because skipping it could let invalid bytecode corrupt frame state during execution.

The `serde` and `postcard` crates were chosen for the wire format. Both support `no_std` plus `alloc`. Postcard is well-tested in embedded contexts and produces compact output. The choice prefers ergonomics through `#[derive(Serialize, Deserialize)]` on existing types over a custom binary layout. A custom format is admissible if path B in P10 motivates a representation amenable to in-place execution.

The new error variant `VmError::LoadError(String)` carries deserialization and header-validation failures. The structured `bytecode::LoadError` enum at the serialization layer carries the cause and converts to `VmError` through a `From` impl at the Vm boundary. Variants are `BadMagic`, `Truncated`, `UnsupportedVersion`, `WordSizeMismatch`, `AddressSizeMismatch`, `BadChecksum`, and `Codec`.

The validation order in `Module::from_bytes` is truncation, magic, length, CRC residue, version, word size, address size, and body decode. The CRC check precedes the version, word size, and address size checks because a corrupted byte in any of those fields would otherwise be reported as a mismatch rather than the more accurate `BadChecksum`. The length check precedes the CRC check because the CRC range depends on the recorded length. Separate tests in the runtime construct bytecode with deliberately wrong version, word size, and address size fields and recomputed CRC trailers to exercise each rejection path independently of the checksum path.

Existing entries B9 and B10 in the backlog reference the precompiled-code question. R39 implements the loading and trust-skip portions. B9 (yielded static string lifetimes) and the broader portability work in B10 remain open.
