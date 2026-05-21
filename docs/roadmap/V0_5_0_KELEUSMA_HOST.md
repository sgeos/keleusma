# V0.5.0: Keleusma-Hosted Keleusma

> **Navigation**: [Roadmap](./README.md) | [Documentation Root](../README.md)

**Status**: Preliminary strategy, awaiting V0.3.0 self-hosted compiler and V0.4.0 native code generation. Architecture and prerequisites identified; scope partitioned across V0.5.0 core and V0.5.x follow-ons. Implementation deferred until the prerequisites land. Expect further refinement after the V0.4.0 strategy document drafts and as V0.5.0 implementation approaches.

## Goal

Migrate the Keleusma compiler's host application from a Rust program to a Keleusma program. The endpoint is a configuration in which the orchestration logic (command-line dispatch, pipeline coordination, file-system sequencing) lives in Keleusma source. A minimal Rust shim remains to provide the operating-system interface (process launch, memory allocator binding, signal handling) and to host the runtime VM where bytecode execution is selected.

The migration target is the `keleusma` command-line driver. Two driver shapes are first-class in V0.5.0:

| Driver shape | Termination | Productivity | Use case |
|---|---|---|---|
| `impure fn main` | Terminates per invocation | Each impure call returns | CLI utilities. The compiler driver is the canonical example. |
| `impure loop main` | Productive divergent, productivity delegated to sub-coroutines | Sub-coroutine yield is the productivity witness | Long-running drivers. Servers, RTOS tasks, game loops, autonomous-probe controllers. |

The compiler driver is the first shape: it runs once per invocation against an input source file, compiles, writes the output, and exits. Long-running daemons and embedded controllers select the second shape. The two coexist; a deployment may host either or both.

The runtime VM and the arena allocator remain in Rust. V0.5.0 closes the "host" loop without attempting to close the "VM" loop. Replacing the VM itself is a V0.6+ aspiration whose feasibility V0.5.0 informs but does not decide.

## Why Keleusma-hosted matters

Three reasons.

First, the demonstration is stronger than self-hosting alone. V0.3.0 demonstrates that Keleusma can express its own compiler. V0.5.0 demonstrates that Keleusma can host applications of substantial complexity, of which the compiler driver is one example. The signal to a prospective adopter shifts from "this language can compile itself" to "this language can drive arbitrary applications, including its own toolchain, under its own bounded-resource discipline."

Second, V0.5.0 shortens the dependency graph for certification-adjacent use cases. With V0.5.0 in place, the only Rust code that ships in the operator-facing path is the runtime VM and a small shim. The compiler driver, the CLI surface, the file orchestration, and the pipeline coordination all live in Keleusma source, are subject to Keleusma's verifier, and inherit its bounded-WCMU and bounded-WCET properties.

Third, V0.5.0 forces the runtime to support several primitives that are independently valuable: structured live code update with verification, multiple-module compilation with interface contracts, sub-DAG arena partitioning with master-WCMU-based allocation, and an impurity modifier for I/O-performing functions. The host migration is the forcing function that brings these primitives together.

## Prerequisites

V0.5.0 implementation is gated on the following landings.

1. **V0.3.0 self-hosted compiler.** The compiler is implemented in Keleusma source, structured as the lexer/parser/compiler pipeline described in [V0_3_0_SELF_HOSTING.md](./V0_3_0_SELF_HOSTING.md). Without this, the host has no Keleusma compiler to dispatch.

2. **V0.4.0 native code generation.** The Keleusma compiler can be compiled to native code via LLVM and linked as a static library. V0.5.0's primary deployment shape is native code; bytecode-via-VM is the fallback. Without V0.4.0, the host runs only in the fallback configuration, which is acceptable for proof-of-concept but not the shipping toolchain.

3. **Sub-coroutines (callable ephemeral `loop` constructs).** A `loop` function callable as a sub-coroutine from inside another `loop` function or from an impure driver. Each sub-coroutine has its own program counter, its own call-frame stack, its own operand stack, and its own arena slot. The full specification lives in [SUB_COROUTINES.md](../architecture/SUB_COROUTINES.md). V0.5.0 consumes the spec; the spec itself is a separate piece of design work, with implementation gated on the same V0.5.0 timeline.

4. **Three-mode purity discipline.** A pure-by-default function attribute system. Pure functions cannot perform I/O or accept impure callbacks. Impure functions may perform I/O and accept any callbacks. Transitive functions have pure bodies but may accept impure callbacks and pass them through, with effective purity inherited from the callsite. See "Function purity discipline" below.

5. **File-based modules with explicit interface declarations.** A module system in the Modula-2 and Ada tradition: each module ships a separately compiled implementation paired with an explicit, auditable interface declaration. Cross-module type checking, generics resolution, and monomorphization specialization all observe module boundaries. See "Modules" below.

6. **Declared sub-DAG arena partitions.** Source-level declarations of partition boundaries within and across modules. The compiler assigns each declared partition its own arena slot. Auto-detection is deferred to V0.5.x or later; declaration is the V0.5.0 mechanism.

7. **Operating-system interface natives, file and stdio scope only.** The Rust shim exposes a small set of native functions: file open, file read, file write, file close, command-line argument iteration, process exit code, stdout and stderr write. Bare-metal hardware-control natives (volatile memory access, interrupt registration, device-register I/O) are deferred to V0.5.x.

## Architecture

V0.5.0 native shape (primary):

```
┌─────────────────────────────────────────────────────────┐
│ Rust shim (target: less than 500 lines)                 │
│   - Process entry point                                  │
│   - Memory allocator binding (GlobalAlloc)               │
│   - Native function registration (file/stdio interface)  │
│   - Hand-off to Keleusma host's entry point              │
└──────────────────────┬───────────────────────────────────┘
                       │ static link
                       ▼
┌─────────────────────────────────────────────────────────┐
│ Keleusma host program (native code, via V0.4.0 LLVM)     │
│   `impure fn main` for CLI utilities                     │
│   or `impure loop main` for long-running drivers         │
│   - parses command-line arguments                        │
│   - dispatches subcommand                                │
│   - orchestrates compiler pipeline as sub-coroutines     │
│   - handles file-system sequencing                       │
│   - writes output artefacts                              │
└─────────────────────────────────────────────────────────┘
```

V0.5.0 bytecode shape (fallback):

```
┌─────────────────────────────────────────────────────────┐
│ Rust shim                                                │
│   - As above plus VM instantiation                       │
│   - Loads `host.kel.bin` from disk or embedded resource  │
│   - Dispatches through the VM                            │
└──────────────────────┬───────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│ Keleusma host program (bytecode, executed by VM)         │
│   Same source; compiled by V0.3.0 self-hosted compiler   │
└─────────────────────────────────────────────────────────┘
```

The native shape is the primary deployment. The bytecode shape is supported and tested but is intended for development, debugging, and platforms where the native toolchain is unavailable.

## Function purity discipline

V0.5.0 introduces a three-mode purity discipline. Purity is a function attribute orthogonal to the existing category (`fn`, `yield`, `loop`). Functions are *pure by default*; impurity must be declared explicitly.

| Purity mode | Body may perform I/O | Callback parameters may be impure | Caller's context |
|---|---|---|---|
| `pure` (default, no modifier) | No | No | Pure or impure |
| `impure` | Yes | Yes | Impure only |
| `transitive` | No | Yes | Inherits from callbacks: pure if all callbacks are pure, impure if any callback is impure |

The `transitive` mode is the *purity-polymorphic* declaration. A function declared `transitive` has a pure body but is explicitly authorised to accept impure callbacks. When invoked with pure callbacks, the call is pure. When invoked with at least one impure callback, the call is in an impure context.

Verifier call rules:

- A `pure` function cannot call an `impure` function directly, nor accept an impure callback. A `pure` body that attempts to invoke an impure callback at any callsite is rejected.
- A `pure` function may call a `transitive` function, but only when supplying pure callbacks to it. The verifier checks the callback signatures at the callsite.
- An `impure` function may call any function (pure, impure, or transitive, with any callback purity).
- A `transitive` function may call pure functions freely, and may call other transitive functions passing its received callbacks through. A `transitive` body cannot call `impure` functions directly because the body itself is pure.

The strict transitivity rule from the broader purity discipline continues to hold: a function cannot hide impurity by chaining through callbacks. Either the function performs I/O (declare `impure`) or it passes a caller-supplied callback through (declare `transitive`). There is no third path; in particular, a `pure` function cannot become impure by quietly invoking an impure callback received from somewhere.

Note on terminology: the word "transitive" appears in two senses in the Keleusma vocabulary. The phrase "transitive impurity is not allowed" describes the call-graph invariant (a pure function cannot reach an impure function through any chain of calls). The keyword `transitive` describes the purity-polymorphic function type (pure body, callback-dependent effective purity). The two are consistent: a `transitive` function does not violate the call-graph rule because its body never directly calls impure code, and its effective purity is observable at the callsite.

WCET for impure functions is best-effort. The host promises bounds on I/O latency; the verifier reports the impure best-effort bound clearly marked as host-promise-dependent. Operators are responsible for verifying that their host meets the promise. `transitive` functions inherit the WCET treatment from their callback parameters at the callsite: pure invocation gets the strict bound, impure invocation gets the best-effort bound.

Direct memory access (volatile reads and writes, memory-mapped I/O, hardware-register access) is a stricter subcategory of impurity. The V0.5.0 surface covers file and stdio impurity only; volatile-access additions are deferred to V0.5.x.

The two driver shapes are both declared `impure`: `impure fn main` for CLI utilities, `impure loop main` for long-running drivers. The compiler pipeline stages remain pure (or transitive, where they accept caller-supplied functions). Impure code lives at the host boundary.

## Modules

V0.5.0 introduces file-based modules with explicit interface declarations following the Modula-2 and Ada tradition.

Per module, two source files (exact extensions to be finalised during implementation):

- An implementation file containing all definitions.
- An interface file declaring exported names, types, generics, traits, declared WCMU bounds, declared WCET bounds, declared signed-modifier status, and exported error types.

The interface declaration is the load-bearing artefact for cross-module verification. Consumers compile against the interface; the implementation is verified to honor its declared bounds at compile time. A change to the interface is a breaking change.

Compilation model: separate compilation. Each module compiles to its own native object file and its own bytecode artefact, with both products consistent with the same interface declaration. The build system orchestrates module compilation and linking, similar to a conventional Rust or Go build.

Cross-module generics resolution: specialization happens at the consumer site, with per-consumer-module specialization tables. The specialization table for a generic exported by module `foo` consumed by module `bar` lives in `bar`'s compilation output, not in `foo`'s. This bounds each module's table by its own complexity rather than by transitive consumption.

Error propagation across modules: standard `Result` semantics. A function in module `foo` returning `Result<T, FooError>` is consumed at the boundary; the consumer either propagates with `?`-style syntax or handles the error locally. The interface declaration includes the error type. A change in error type is a breaking interface change.

Module-level signing: each module's bytecode artefact and native object file may carry an Ed25519 signature, scoped to the artefact. Consumers may declare verifying keys per imported module. The loader rejects modules whose signature fails verification against the declared key. Module-level signing composes with the V0.2.0 signing infrastructure directly.

## Arena partitioning

Sub-DAG partitions are the unit of arena assignment. A partition is a declared collection of modules (or sub-units of modules) that share an arena slot at runtime.

Declaration: a partition manifest (likely a top-level configuration artefact, exact format to be finalised) lists each partition's modules and bound. The compiler computes per-partition WCMU as the sum of contained-module bounds and verifies that the sum is finite.

Allocation strategy: *master-WCMU-based*. The total arena is sized at compile time as the sum of per-partition bounds, with mutual-exclusivity refinements where the compiler can prove that two partitions never coexist at runtime. Dynamic allocation and runtime-managed allocation are not options; arena layouts are fixed at compile time.

Mutual-exclusivity analysis is an optional V0.5.0 refinement. Partitions whose lifetimes are statically disjoint may share an arena slot. The refinement reduces the total arena size from the sum to the maximum of mutually exclusive sets. The analysis is similar to rate-monotonic analysis in real-time scheduling. V0.5.0 may ship without the refinement (using the simple sum) and add it in V0.5.x once a real case justifies the analysis cost.

Auto-detection of partition boundaries (analysing the program graph and partitioning automatically) is deferred to V0.5.x or later. Declaration is preferable for certification: partition boundaries are auditable, stable across edits, and reviewed as part of the source.

Ephemeral partition pools (a pool of N similar partition slots in a shared arena, each slot holding an ephemeral sub-coroutine) are deferred to V0.5.x. The pattern fits particle systems, network connection handlers, and RTOS task pools, none of which are V0.5.0 deliverables.

## Live code update

V0.5.0 introduces *structured live code update with verification*, following the Erlang/OTP terminology rather than the classical self-modifying-code tradition. The properties are:

- Replacement happens at quiescent points, not mid-instruction.
- Replacement units are named (modules and sub-DAG partitions), not arbitrary byte ranges.
- Each replacement is verified for structural soundness, WCMU bound, WCET bound, signature validity, and interface compatibility before installation.
- The interface contract is enforced; a swap that breaks the contract is rejected.

The wire-format header gains an **interface-fingerprint** field: a hash over the module's exported names, types, generic signatures, declared bounds, exported error types, and signed-modifier status. The signature covers both the code body and the interface-fingerprint. Tampering with either invalidates the signature.

Hot-swap acceptance rule: the new module's interface-fingerprint must match the in-place module's, or match a documented compatible extension (added exports, tighter bounds, never relaxed bounds or removed exports). Cross-module references resolve against the in-place fingerprints; a swap that would invalidate any consumer's resolution is rejected.

| Live-update capability | Target |
|---|---|
| Module-level hot swap with signature and bound verification | V0.5.0 |
| Cross-module hot swap with interface-fingerprint enforcement | V0.5.0 |
| State migration across compatible swaps, preserving variable values | V0.5.x |
| Sub-function patching and bandwidth-constrained delta updates | V0.6 or later |
| Quiescence-point detection for guaranteed-safe replacement | V0.5.x |

V0.5.0 ships the foundational mechanism. State migration, sub-function patching, and quiescence-point detection are independently complex problems that V0.5.x and later releases address.

### Hot-replacement granularity is a build-mode choice

V0.4.0 research surfaces a cost that V0.5.0 deployments must choose around. Native-level hot replacement requires suppressing cross-module inlining at hot-replacement boundaries; otherwise the inlined callee is baked into the caller's object file and the callee's module is no longer replaceable. The performance cost of suppressing cross-module inlining is real and may be substantial for inlining-sensitive code paths.

V0.5.0 therefore admits two build modes that select per-deployment:

| Build mode | Hot replacement at native level | Native-shape performance |
|---|---|---|
| Hot-replacement-friendly | Per-module, with interface-fingerprint check | Reduced. Cross-module inlining suppressed at hot-replacement boundaries. |
| Performance-friendly | Not supported at native level. Replacement requires binary restart, or use of the bytecode shape. | Full LLVM optimisation, including cross-module inlining. |

The bytecode shape always supports hot replacement regardless of build mode. Deployments that need hot replacement and cannot pay the native performance cost run their host in the bytecode shape. Deployments that need maximum native performance and can tolerate restart-only upgrade use the performance-friendly build.

Long-lived autonomous systems (where binary restart is undesirable and bytecode performance is acceptable) lean toward the hot-replacement-friendly native build or the bytecode shape with native acceleration of hot paths. Short-lived utilities and high-throughput servers lean toward the performance-friendly native build. The choice is operational and should be documented per deployment.

### Native WCET is best-effort, not hard

The bytecode-level WCET is the verification artefact. LLVM optimisation reorders, inlines, vectorises, and combines instructions during native code generation in ways the bytecode-instruction-cost model cannot predict. The native code is typically faster than the bytecode in expectation; the bytecode WCET claim is a soft upper bound on native execution, not a tight bound on native execution time.

Operators who need hard real-time guarantees use the bytecode shape on a verified-cost VM where the bytecode WCET claim is the certified bound. Operators who use the native shape accept the best-effort timing convention, similar to the impure-WCET convention. The V0.4.0 strategy document covers this in detail; the V0.5.0 implication is that the Keleusma host's WCMU bounds compose cleanly across the native lowering, but its WCET bounds carry the best-effort label.

See [V0_4_0_NATIVE_CODEGEN.md](./V0_4_0_NATIVE_CODEGEN.md) for the per-target WCET analysis options and the V0.4.x and V0.5+ refinements available.

The combination of verified hot replacement plus bounded-resource verification plus signed artefacts is the differentiating capability. Existing live-update systems either skip verification (Smalltalk images, Lisp `redefine`) or accept restricted update surfaces (Erlang's specific module-boundary semantics, real-time-OS static configuration). Keleusma offers both wide coverage and strong verification simultaneously.

## Bootstrap procedure

Four phases. The first three are analogous to V0.3.0's bootstrap, shifted to apply to the host program rather than the compiler. The fourth phase is the migration from bytecode shape to native shape, which is the V0.5.0 shipping configuration.

**Phase α. Cross-host, bytecode shape.** The Keleusma host program is written in Keleusma source under `host/main.kel` and supporting files. The V0.3.0 self-hosted compiler, running on the Rust-hosted VM, produces `host.kel.bin`. The Rust shim is built. The shim loads `host.kel.bin` and dispatches the entry point (`impure fn main` or `impure loop main`). The host orchestrates a compile of a trivial program end-to-end. Success: a Keleusma-hosted Keleusma compile produces the same output as a Rust-hosted Keleusma compile.

**Phase β. Self-host the compiler.** The Keleusma-hosted toolchain compiles `host/main.kel` itself. The output is `host.1.kel.bin`. If Phase α is correct, `host.1.kel.bin` is byte-identical to `host.kel.bin` modulo non-essential ordering.

**Phase γ. Fixed point on the host.** The Keleusma-hosted toolchain re-compiles `host/main.kel` to produce `host.2.kel.bin`, byte-identical to `host.1.kel.bin`. Fixed-point reached.

**Phase δ. Migrate to native shape.** The Keleusma host is compiled to native code via V0.4.0 and linked into the Rust shim as a static library. The resulting binary is the shipping configuration. The bytecode shape from Phases α through γ remains available as the fallback.

Validation runs alongside Phases β, γ, and δ: the regression corpus is compiled through the Keleusma-hosted toolchain at each phase and compared against the all-Rust baseline. Divergence is a bug in the host or in the underlying compiler.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Sub-coroutine dispatch overhead is unacceptable compared to native Rust orchestration | Profile in the bytecode shape. The native shape (Phase δ) flattens sub-coroutine dispatch where the LLVM coroutine intrinsics support it. |
| Operating-system native interface accretes surface area | Publish a fixed file-and-stdio interface up front and resist extension. Bare-metal natives are a separate proposal deferred to V0.5.x. |
| Sub-coroutines introduce verifier complexity (per-coroutine WCMU, productivity, spawn-site accounting) | Specify and implement the verifier extension as prerequisite work, not inline with the host migration. The host migration consumes a verified primitive; it does not introduce verifier changes inline. |
| The host's WCMU bound is harder to articulate than the per-stage compiler bound | Publish the host's bound explicitly as overhead per file compiled, exclusive of compiler stages. Compose by sum still holds. |
| Diagnostic quality for host-level errors regresses compared to the Rust shim | Document accepted regression. Invest in diagnostic quality alongside the migration. The host has access to Keleusma's existing error machinery; the question is whether the operating-system interface natives surface enough detail to produce comparable messages. |
| The Rust shim grows beyond budget (target: less than 500 lines) | Track size as a project metric. Factor functionality into the Keleusma host or into additional natives if the shim grows. |
| Live code update introduces consistency hazards in cross-module reference resolution | Reject swaps whose interface-fingerprint cannot be reconciled with consumer expectations. Build cross-module dependency graph at load time; refuse swaps that would invalidate any reachable consumer. |
| Module interface declarations accumulate maintenance overhead | Accept the cost. The certification posture and the live-update model both require explicit interface contracts. Tooling assists by checking that implementation and declaration agree. |
| Master-WCMU sum is loose enough to make some programs infeasible | Apply mutual-exclusivity refinement where partitions are statically disjoint. If the sum still exceeds available memory, the program is genuinely too large for the deployment target and the operator must reduce scope or relax bounds. |
| Native object files compiled at different toolchain versions accidentally link | Reject at link time by stamping each object file with the toolchain version. Mixing across versions is unsupported; matching versions link successfully. |

## Out of scope

- **Replacing the VM.** The runtime VM remains in Rust. Compiling the VM itself in Keleusma is a V0.6+ aspiration.
- **Replacing the arena allocator.** Same as above.
- **Removing all Rust code.** The Rust shim is the irreducible OS-interface surface. V0.5.0 minimises it; V0.5.0 does not eliminate it.
- **Bare-metal hardware-control surface-language additions.** Volatile memory access, interrupt registration, and device-register I/O are deferred to V0.5.x. V0.5.0 covers file and stdio impurity only.
- **Auto-detection of arena partitions.** V0.5.0 ships declaration-only.
- **Ephemeral partition pools.** Deferred to V0.5.x.
- **State migration across hot swaps.** Deferred to V0.5.x.
- **Sub-function patching and delta updates.** Deferred to V0.6 or later.
- **Quiescence-point detection.** Deferred to V0.5.x.
- **Single-CPU async event loops.** A candidate V0.5.x deliverable, built on top of sub-coroutines.
- **Multi-CPU execution.** V0.7+ realistically. Requires shared-data synchronization primitives, memory ordering, and a runtime scheduler that V0.5.0 does not contemplate.
- **Cross-platform shim variants.** The initial Rust shim targets a single host platform. Other platforms negotiate their own shims as separate work.
- **Embedding API for tertiary applications.** A Keleusma host program exposing its own embedding API for downstream applications is a V0.5.x or later concern.

## Resolved questions

The following questions have been settled during V0.5.0 strategy refinement. They are recorded here for traceability; the resolution is reflected in the corresponding sections of this document and in [SUB_COROUTINES.md](../architecture/SUB_COROUTINES.md).

- **Default purity.** Pure by default. Impurity must be declared explicitly.
- **Purity polymorphism.** Admitted, via the `transitive` mode. A function may have a pure body while accepting impure callbacks, with effective purity inherited from the callsite.
- **Strict transitive impurity rule.** Affirmed. A pure function cannot reach an impure function through any chain of calls. Either the function performs I/O (`impure`) or it explicitly passes a caller-supplied callback through (`transitive`).
- **Driver shapes.** Both `impure fn main` (terminating, for CLI utilities) and `impure loop main` (productive divergent, for long-running drivers) are first-class entry points.
- **Sub-coroutine model.** Asymmetric, call-down and yield-up. Full specification in [SUB_COROUTINES.md](../architecture/SUB_COROUTINES.md).
- **Arena slot reservation.** Arena slots are reserved for a sub-coroutine's entire life and cannot be reassigned mid-execution. Ephemeral and persistent sub-coroutines differ in what happens at completion, not during execution.

## Open questions

1. **Mutual-exclusivity analysis in V0.5.0 versus V0.5.x.** Does V0.5.0 ship with the simple-sum allocation, or with the mutual-exclusivity refinement? The simple sum is sufficient for the compiler driver. Real-time and embedded applications likely need the refinement.

2. **Sub-coroutine surface syntax.** Multiple candidates documented in [SUB_COROUTINES.md](../architecture/SUB_COROUTINES.md). Choice deferred to that specification's settlement.

3. **Arena partitioning unit-of-declaration.** Is a partition a module, a collection of modules, or a sub-unit of a module? The default is "one module equals one partition," but a partition manifest may compose or subdivide. The manifest syntax needs specification.

4. **Host upgrade path on the operator's machine.** If the Keleusma host program is itself a `.kel.bin` artefact, how is it signed and verified at load time? The existing `signed` modifier and verifying-key registration cover this case directly. If the host is statically linked native code, the host distribution channel is whatever the operator uses for signed executables. Documented case-by-case.

5. **Interface-fingerprint hash function and stability.** What hash, what input encoding, what stability guarantees across toolchain versions? The choice affects whether hot swaps remain compatible across compiler updates. Likely SHA-256 over a canonical serialisation of the interface declaration; specification needed.

6. **Module file extension and naming.** The implementation file and the interface file need stable extensions. Candidates include `.kel` plus `.def.kel`, `.kel` plus `.kdef`, or other shapes. The choice is cosmetic but should be settled before the V0.5.0 implementation begins to avoid churn.

7. **Transitive purity edge cases.** When a `transitive` function returns a value that closes over its callback parameter, what is the purity of the returned value? Likely "the returned value carries the callback's purity," but the specification needs to cover the storage and re-invocation cases. The closure prohibition in the safe verifier limits the practical surface of this question; first-class function values are still admitted and need treatment.

## Prior art

The "language hosts itself" configuration is established. Representative examples:

- **LLVM and Clang.** The Clang driver is a C++ program that orchestrates the compiler. The driver is itself compiled by Clang. The C++ language hosts its own compiler driver.
- **Rust rustc.** The Rust compiler driver is a Rust program. Rust hosts its own toolchain. Initial bootstrap was from OCaml; the OCaml dependency was retired in 2011.
- **Go gc.** The Go compiler driver is a Go program. Go hosts its own toolchain. Initial bootstrap was from Plan 9 C; the C dependency was retired in Go 1.5, 2015.
- **Smalltalk-80.** The most aggressive instance. The Smalltalk system hosts its compiler, its image manipulation tools, its development environment, and its garbage collector. Pharo and Squeak Smalltalk continue this tradition.
- **Common Lisp implementations.** Several Common Lisp implementations (SBCL is the most prominent) host themselves in Lisp.
- **Erlang/OTP.** The hot code reload mechanism in OTP is the closest precedent for Keleusma's live code update model. OTP supports per-module hot swap with version-tagged interfaces, supervisor-mediated restart semantics, and explicit state migration callbacks. The Erlang model is widely deployed in telephony and distributed systems with multi-decade production track records.
- **Modula-2 and Ada.** The explicit-interface module model originates here. The pattern of "definition module" plus "implementation module" in Modula-2 and "package specification" plus "package body" in Ada is the direct precedent for Keleusma's separate-implementation-and-interface shape.

The novelty in V0.5.0's case is the combination: language hosts itself, plus structured live code update, plus bounded-resource verification, plus signed artefacts. Each component has precedent; the combination is to our knowledge new.

## References

- Wirth and Gutknecht, *Project Oberon: The Design of an Operating System and Compiler*, Addison-Wesley, 1992; revised edition 2013.
- Brinch Hansen, *The Architecture of Concurrent Programs*, Prentice-Hall, 1977, ISBN 0-13-044628-9.
- Brinch Hansen, *Brinch Hansen on Pascal Compilers*, Prentice-Hall, 1985, ISBN 0-13-083098-4.
- Joe Armstrong, *Programming Erlang: Software for a Concurrent World*, Pragmatic Bookshelf, 2007; second edition 2013, ISBN 978-1-937785-53-6. The canonical Erlang reference, including hot code reload semantics and the OTP design principles.
- Wirth, *Programming in Modula-2*, Springer-Verlag, 1982; third edition 1985. The separate-compilation model with definition and implementation modules.
- Ada 2012 Reference Manual, ISO/IEC 8652:2012. The package specification and body model.
- Cross-reference: [V0_3_0_SELF_HOSTING.md](./V0_3_0_SELF_HOSTING.md) for the self-hosted compiler that V0.5.0 dispatches.
- Cross-reference: [V0_4_0_NATIVE_CODEGEN.md](./V0_4_0_NATIVE_CODEGEN.md) for the native-code-generation path that V0.5.0's Phase δ depends on.
