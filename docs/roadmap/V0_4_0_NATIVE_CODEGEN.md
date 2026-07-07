# V0.4.0: Native Code Generation

> **Navigation**: [Roadmap](./README.md) | [Documentation Root](../README.md)

**Status**: Strategy draft. Research pass complete; LLVM identified as the codegen backend; sub-coroutine lowering via LLVM coroutine intrinsics identified as the load-bearing primitive; static-library linkage to Rust hosts identified as the primary deliverable. Implementation gated on V0.3.0 self-hosted compiler landing. Expect refinement after the V0.5.0 strategy stabilises and as V0.4.0 implementation approaches.

## Goal

Add native code generation to the Keleusma toolchain. Keleusma source compiles via the V0.3.0 self-hosted compiler to bytecode (the verification artefact), then via a new V0.4.0 LLVM-based code generator to native object files. The native artefact is linkable as a static library against a Rust host, replaces the VM interpretation path in performance-sensitive deployments, and exposes sub-coroutine entry points implemented via LLVM coroutine intrinsics so that the Rust host can call coroutine-driven native functions whose state machines are LLVM-managed.

The bytecode shape continues to ship in parallel. V0.4.0 does not retire the VM; it adds a second deployment shape.

The flat-machine ISA redesign that native code generation enables, an untyped byte operand stack, composite values as pure bytes, field offsets baked into access instructions rather than resolved at run time, and the kind carried by the opcode rather than by a tag on the value, is captured as input under *Deferred ISA redesign* in B28 of [`../decisions/BACKLOG.md`](../decisions/BACKLOG.md). The V0.2.x Rust runtime deliberately keeps a tagged operand stack and resolves offsets at dispatch, which native code generation supersedes by resolving everything statically.

## Why native matters

Three reasons.

First, the VM interpretation tax is paid on every bytecode instruction. For programs whose hot path runs at high frequency (real-time controllers, signal processing, game loops, server request handlers), the interpretation overhead can dominate the total cost. Native code removes the tax. The same source, the same verification artefact, the same bounded-resource guarantees, but with the per-instruction cost of native execution rather than VM dispatch.

Second, native code is the deployment shape most operators expect. A single statically linked binary integrates with conventional toolchains: linker, package manager, container, signed-installer pipeline. Bytecode requires either an embedded VM in the host or a separate VM runtime, which is acceptable for embedding scenarios but is friction for standalone deployment.

Third, V0.4.0 is the precondition for V0.5.0's Keleusma-hosted host. The V0.5.0 host is a Keleusma program that must orchestrate the compiler pipeline as sub-coroutines. If the host runs in the VM, the orchestration cost is interpretation overhead on every call. If the host runs as native code with sub-coroutines lowered to LLVM coroutine intrinsics, the orchestration is at native speed. V0.5.0's shipping configuration depends on V0.4.0 being in place.

## Prior art

A research pass surveyed the LLVM-backed language toolchain tradition and the bytecode-plus-native deployment tradition.

### LLVM-based language toolchains

LLVM has become the de facto target for production language back ends since the mid-2000s.

- **Rust rustc** emits LLVM IR, LLVM lowers to native code. The architecture is documented in the *rustc dev guide* and is the closest precedent for what V0.4.0 implements. Rust's bootstrap was historically OCaml-to-Rust; the current toolchain is Rust-to-LLVM-to-native, paralleling Keleusma's intended Keleusma-to-LLVM-to-native shape.
- **Swift** emits SIL (a Swift-specific higher-level IR) which lowers to LLVM IR which lowers to native. Swift's coroutine model (`async`/`await`) is implemented via LLVM coroutine intrinsics in the same manner V0.4.0 will use for sub-coroutines.
- **Crystal**, **Zig**, **Pony**, and several other modern languages target LLVM directly. The pattern is well understood; the tooling is mature.

The body of practice demonstrates that an LLVM-based back end is a reliable engineering choice. Risk concentrates in the language-specific lowering, not in LLVM itself.

### Bytecode-plus-native deployment

The "bytecode as portable IR, native as deployment artefact" pattern has multiple production-grade examples.

- **WebAssembly (Wasm)** specifies a portable bytecode designed for either JIT or AOT lowering to native. V8, SpiderMonkey, Wasmtime, and Wasmer all implement the lowering. The Wasm specification explicitly designates bytecode as the portable form and native as the runtime form; this is the architectural pattern Keleusma adopts.
- **Java HotSpot JVM** ships JVM bytecode plus a tiered JIT that lowers hot paths to native. The bytecode is the distribution format; the native code is generated at runtime. Two decades of production deployment.
- **.NET CLR** uses CIL bytecode plus a tiered JIT, with optional AOT paths (NGen, ReadyToRun, NativeAOT) that produce native code at install or build time.
- **Erlang BEAM** historically supported HiPE (High-Performance Erlang) as a native-code path alongside the BEAM bytecode interpreter. HiPE has been retired upstream; the current Erlang/OTP includes a JIT (JIT4Erlang) in the BEAM runtime.

Keleusma's positioning relative to these:

- Unlike Wasm, Keleusma's bytecode carries verification metadata (WCMU bounds, WCET claims, productivity proofs, signatures) that the native lowering preserves.
- Unlike HotSpot and CLR, Keleusma's native code is AOT-produced from the bytecode, not JIT-produced at runtime. The same verification artefact is used at every stage.
- Unlike Erlang HiPE, Keleusma's native shape is intended as the primary deployment, not an optional accelerator.

### LLVM coroutine intrinsics

LLVM introduced coroutine intrinsics in version 5.0 (2017) to support C++20 coroutines and similar models in other languages. The reference is Gor Nishanov's documentation in the LLVM project, *Coroutines in LLVM*, and the corresponding intrinsic set under `llvm.coro.*`.

The model is stackless coroutines: each coroutine instance has a state-machine frame (typically heap-allocated by default, with custom-allocator hooks available) that records the coroutine's resumption point and local state. Resume and suspend operations transition the state machine. The compiler transforms the source-level coroutine into a set of ordinary functions plus the state-machine frame.

LLVM offers four coroutine ABI families distinguished by the `coro.id*` intrinsic invoked:

- **Switched-resume** (`@llvm.coro.id`). General purpose; used by C++20 coroutines and Rust's async/await (via lowering). The frame is heap-allocated by default; frontend-managed allocation can pass a "block of memory" to `coro.begin` but the intrinsic itself does not accept allocator function pointers.
- **Returned-continuation** (`@llvm.coro.id.retcon`). Each suspend point returns yielded values plus a continuation pointer for resumption. The intrinsic explicitly accepts allocator and deallocator function pointer arguments, and the frame is maintained in a fixed-size buffer of statically-known size and alignment. **This is the family V0.4.0 uses.**
- **Retcon-once** (`@llvm.coro.id.retcon.once`). A simplified one-shot form of returned-continuation.
- **Async** (`@llvm.coro.id.async`). Swift's async-context model. Not appropriate for Keleusma's sub-coroutine design.

The intrinsics are stable in LLVM 14 and later. The retcon family's documented fixed-size-buffer guarantee and explicit allocator hook are precisely the properties Keleusma's WCMU analysis requires. Earlier drafts of this strategy referenced `@llvm.coro.id.async` for the custom-allocator path; that was an error corrected in the 2026-05-21 research pass (see `tmp/research/WEB_RESEARCH_APPENDIX.md`).

### Cross-platform and embedded targets

LLVM ships back ends for x86-64, AArch64, ARM (including Cortex-M for embedded), RISC-V (RV32 and RV64), PowerPC, MIPS, WebAssembly, and several others. V0.4.0 targets x86-64 Linux as the primary platform, with macOS, Windows, AArch64 Linux, and Cortex-M as follow-on platforms in V0.4.x as the toolchain stabilises.

### Vintage processor targets

Vintage processor targets are aspirational. The current landscape:

- **6502 family.** No upstream LLVM back end. The *llvm-mos* project maintains an out-of-tree LLVM fork with a working 6502 code generator. Active community; production-quality output for the most common 6502 platforms.
- **Motorola 68000 family.** Upstream LLVM back end exists, restored to active development circa 2020-2022 after a dormant period. Output quality varies by target subarchitecture.
- **Zilog Z80.** No LLVM back end, upstream or out-of-tree. The established Z80 toolchains are SDCC and z88dk, both of which use their own intermediate representations and code generators. Targeting Z80 from Keleusma would require either a substantial custom back end or a transpilation path through C to SDCC.

The vintage targets motivate aspirational interest in bounded-resource discipline applied to constrained hardware. They are not V0.4.0 deliverables. Their place in the Keleusma roadmap is exploratory.

## Architecture

The V0.4.0 compilation pipeline:

```
Source (.kel files)
   │
   ▼
[V0.3.0 self-hosted compiler]
   │
   ▼
Bytecode (.kel.bin) + verification metadata
   │
   ▼
[V0.4.0 LLVM IR generator]
   │
   ▼
LLVM IR (.ll or in-memory)
   │
   ▼
[LLVM core: optimisation + target codegen]
   │
   ▼
Native object file (.o or .obj)
   │
   ▼
[platform linker]
   │
   ▼
Static library (.a or .lib) or shared library (.so/.dylib/.dll)
```

The bytecode produced by V0.3.0 is the verification artefact. The LLVM IR generator transforms bytecode plus metadata into LLVM IR that preserves the semantic content while admitting LLVM's optimisations. LLVM then performs target-specific lowering, optimisation, and code generation. The platform linker assembles object files into the deliverable.

### Sub-coroutine lowering

Each Keleusma `loop` sub-coroutine becomes an LLVM coroutine in the returned-continuation kind. The mapping:

| Keleusma sub-coroutine operation | LLVM coroutine intrinsic |
|---|---|
| Spawn (allocate slot, initialise state, return handle) | `@llvm.coro.id.retcon` with Keleusma-provided allocator and deallocator function pointers + `@llvm.coro.begin` |
| Resume (transfer control to coroutine, return yielded value or completion marker) | Indirect call through the current continuation pointer stored in the arena slot |
| Yield (transfer control back to parent) | `@llvm.coro.suspend` returning the yielded value plus the next continuation pointer |
| Release (release slot, invalidate handle) | Deallocator function pointer invocation; arena slot returned to free list |

Each resume returns a new continuation pointer alongside the yielded value. The arena slot holds the current continuation pointer so the Keleusma-level handle is stable across resumes; only the slot's interior changes. This indirection wraps the retcon mechanics behind a stable handle abstraction at the Keleusma surface.

The coroutine handle from the [sub-coroutine specification](../architecture/SUB_COROUTINES.md) corresponds to a Keleusma-defined arena-slot pointer at the LLVM IR level. The Rust host receives the handle through the static-library ABI as an opaque pointer.

### Arena-resident coroutine frames

The retcon ABI was designed for the fixed-buffer, custom-allocator use case Keleusma needs. The V0.4.0 codegen emits IR that names two Keleusma-provided functions per coroutine: an allocator and a deallocator. Both are small native functions (likely Rust, possibly Keleusma `impure fn`) that:

- Allocator: reserves a region in the master arena sized per the coroutine's static bound, returns a pointer to the region. The size and alignment are statically determined at compile time and surface through `coro.id.retcon`'s size and align arguments.
- Deallocator: returns the region to the arena's free list when the coroutine completes or is released.

R4.1 sketched a three-milestone validation plan (`tmp/research/r4_1_llvm_coroutine_allocator.md` plus the correction in `tmp/research/WEB_RESEARCH_APPENDIX.md`). M1 writes a minimal LLVM IR fragment using `coro.id.retcon` with a Keleusma-shaped allocator; M2 lowers it to native and links against a Rust harness exercising spawn/resume/release; M3 measures the per-coroutine overhead. The first milestone is the load-bearing technical risk in V0.4.0 and should be executed before the rest of the IR generator is built out.

### Module linkage

Each Keleusma module compiles to its own LLVM IR module, which lowers to a separate object file. Cross-module references go through linker symbols. The interface fingerprint from V0.5.0's live-update model is embedded in the object file as a custom section, readable by the linker and by hot-replacement tooling.

Symbol mangling: a stable scheme is required so that cross-module references resolve correctly. The natural model is a Keleusma-specific mangling that encodes the module name, the function name, the type arguments (for monomorphised generics), and the purity mode. Documented in the V0.4.0 implementation; details deferred.

### The three deployment shapes after V0.4.0

V0.4.0 produces three shapes that may coexist in a single project.

1. **Bytecode shape (pre-existing).** V0.3.0 self-hosted compiler output, executed by the Keleusma VM. Verified, portable, slower. Use case: development, debugging, environments without native toolchain, hot-replacement-heavy workflows.

2. **Native static-library shape (V0.4.0 primary).** Self-hosted compiler output compiled via LLVM and linked into a Rust host as a `staticlib`. Verified at compile time, fast at runtime, single-binary deployment. Use case: shipping toolchains, performance-sensitive embedding.

3. **Native dynamic-library shape (V0.4.0 secondary).** Same source compiled as `cdylib` for runtime loading. Use case: hot replacement at the native level (subject to the constraints below), plugin architectures.

## Hot replacement at the native level

Native-level hot replacement is materially harder than bytecode-level hot replacement. The bytecode VM holds modules as data structures and swaps them by replacing the data. Native code is position-dependent in subtle ways (relocations, inlined cross-module calls, link-time-resolved symbols) and assumes a specific binary layout at load time.

The V0.4.0 approach: each module that is intended to be hot-replaceable compiles to a separate shared object (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows). Hot replacement uses the platform's dynamic-loader API (`dlopen` and `dlsym` on Unix, `LoadLibrary` and `GetProcAddress` on Windows). The new library is loaded, its interface fingerprint is checked against the V0.5.0 acceptance rule, symbols are re-bound, and the old library is unloaded once no live sub-coroutine still references it.

The cost: cross-module inlining must be suppressed for any boundary that is a hot-replacement boundary. LLVM optimisation that inlines a function call across a module boundary will bake the callee's code into the caller's object file; replacing the callee's module no longer affects the caller. The V0.4.0 codegen will need to mark hot-replacement boundaries with an inlining-suppression attribute, accepting the optimisation cost.

This is a real performance cost. V0.5.0 will need to choose:

- **Hot-replacement-friendly build.** Cross-module inlining suppressed at hot-replacement boundaries. Slower; supports per-module hot swap.
- **Performance-friendly build.** Cross-module inlining permitted. Faster; no hot replacement at the native level. Hot replacement only via re-launching the binary, or via the bytecode shape.

V0.4.0 supports both build modes. V0.5.0 selects per-deployment. This is the most significant V0.5.0 refinement that V0.4.0 research surfaces.

## WCET and WCMU preservation across native compilation

A critical concern that the V0.4.0 strategy must address explicitly.

Keleusma's bytecode WCET model assumes per-instruction cost coefficients derived from the cost-model calibration work in `keleusma-bench`. The verifier sums over the bytecode-instruction graph to produce a WCET bound. When LLVM compiles to native code, it reorders, inlines, deletes, vectorises, and combines instructions in ways the bytecode-level model cannot predict.

Three possible postures:

1. **Native WCET as best-effort.** The verifier reports the bytecode-level WCET claim. The LLVM-produced native code is faster than the bytecode in the typical case; the bytecode WCET claim is a soft upper bound on native execution. Operators who need hard real-time guarantees use the bytecode shape. Operators who use the native shape accept best-effort timing, similar to the impure-WCET convention. **Recommended for V0.4.0.**

2. **Measurement-based native WCET.** After LLVM compilation, the native artefact is benchmarked on the target platform under worst-case input. The measured WCET is published as the verified bound. Per-target, per-build effort; produces hard bounds at the cost of a measurement infrastructure. **Recommended for V0.4.x as the rigorous path.**

3. **Per-target WCET analysis on the native output.** Each LLVM target has documented instruction-cycle behaviour (mostly; modern superscalar pipelines complicate this). The verifier could in principle re-analyse the native output against the target's cycle model. This is the most rigorous and the most expensive. **V0.5+ research, not V0.4.0.**

V0.4.0 ships with posture 1. The verifier produces a WCET claim with explicit "bytecode-bound, best-effort on native" labelling. Posture 2 is added in V0.4.x once a real customer needs the rigorous bound.

WCMU is easier than WCET. The master arena layout is fixed at compile time and does not change under LLVM optimisation. Native code accesses the same arena structure the bytecode would. WCMU bounds are preserved across native compilation.

## Partial-operation native lowering (B35 P8)

The B35 Partial Operation Handling design defines a two-backend contract for every mathematically partial operation, namely division and modulo by zero, out-of-bounds indexing, refinement-newtype construction failure, the invalid discriminant-to-enum conversion, and native-call failure. The virtual-machine side of that contract is implemented in V0.2.x. The virtual machine traps recoverably on any unhandled partial operation, and six source-level constructs let a program handle each outcome so both backends agree.

The native side of the contract is V0.4.0 scope. The IR generator lowers each partial operation to the bare hardware instruction where the target's hardware does not fault, and to a guarded sequence where it would fault. The guarded sequence tests the partial condition, branches to the hardware instruction on the safe path, and produces the contract's defined default value on the unsafe path, namely zero for integer division by zero, the numerator for modulo by zero, the element type's canonical zero or lowest-valid value for an out-of-bounds index, the lowest-valid value for a newtype-predicate failure, and the zero-discriminant variant for an invalid discriminant. A native-call failure traps on both backends, because a host failure has no safe default. Where the source handles the outcome through a construct arm, the IR generator lowers the arm body in place of the default.

The guard adds a small fixed number of operations on the longest path, so it preserves the worst-case execution time and worst-case memory usage bounds the verifier proves on the bytecode shape under posture 1 above. The complete normative contract, including the per-target hardware basis and the canonical-zero and lowest-valid resolution the guards consult, is specified in [`../spec/RUNTIME_FAULTS.md`](../spec/RUNTIME_FAULTS.md). That specification is complete and reviewable now; only the lowering is deferred to V0.4.0.

## Bootstrap procedure

Three phases.

**Phase A. Build the LLVM IR generator in Rust.** The V0.4.0 IR generator is a new Rust crate (or a new module in the existing compiler) that consumes the bytecode plus metadata produced by the existing Rust-hosted compiler (or the V0.3.0 self-hosted compiler in bytecode shape) and emits LLVM IR. This is the engineering core of V0.4.0.

**Phase B. Cross-compile the self-hosted compiler.** The V0.3.0 self-hosted compiler source, currently producing bytecode, is fed through the V0.4.0 IR generator. The output is an LLVM-IR representation of the compiler. LLVM lowers it to native code. The result is a native-code Keleusma compiler. The compiler is statically linked into a Rust host (the minimal V0.5.0 shim, anticipated).

**Phase C. Validation.** The native-code compiler compiles a corpus of Keleusma programs. The resulting bytecode is byte-identical (modulo non-essential ordering) to what the bytecode-shape compiler produces from the same source. Divergence is a bug. The regression corpus from V0.3.0 is the test surface.

Phase C does not require a separate fixed-point check for the native compiler; the native code is a faithful lowering of the same compiler source the V0.3.0 work already fixed-pointed. The V0.4.0 fixed point is: native-compiler-of-source equals bytecode-compiler-of-source, on the regression corpus.

A V0.5.0 follow-on phase D will compile the V0.5.0 Keleusma host program through V0.4.0 native code generation as part of V0.5.0's Phase δ; that is documented in [V0_5_0_KELEUSMA_HOST.md](./V0_5_0_KELEUSMA_HOST.md).

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| LLVM coroutine intrinsics impose overhead that defeats the AOT performance benefit | Profile early. If the overhead is unacceptable for a specific use case, the bytecode shape remains available. If unacceptable across use cases, consider a custom state-machine lowering rather than LLVM coroutines, accepting the maintenance burden. |
| LLVM custom-allocator hook for coroutine frames does not admit arena-based allocation cleanly | Research spike during V0.4.0 implementation. Fallback: a Keleusma-side arena allocator wrapped in the LLVM-expected API surface, with a thin trampoline that reformats addresses if required. |
| Cross-module inlining suppression imposes a significant performance cost | Document the cost. Provide two build modes (hot-replacement-friendly, performance-friendly). V0.5.0 selects per-deployment. |
| WCET claims on native code are weaker than on bytecode | Document explicitly. Operators who need hard timing use the bytecode shape with a verified-cost VM. Native shape is best-effort timing, similar to impure WCET. |
| LLVM API surface shifts across versions | Pin to a specific LLVM major version (likely LLVM 17 or 18 at V0.4.0's implementation date). Document the pin. Upgrade in V0.4.x as the ecosystem moves. |
| Symbol mangling collisions across modules with overlapping type-argument instantiations | Design the mangling scheme up front. Encode module path, function name, type arguments, and purity mode. Test for collision generation. |
| Native artefacts compiled at one toolchain version accidentally link with another version's artefacts | Stamp object files with the toolchain version. Reject mixed-version linkage at link time. Same policy as the V0.5.0 doc records for the host binary. |
| Debugging native code lowered from Keleusma source is harder than debugging bytecode | Generate DWARF debug information that maps native addresses back to Keleusma source positions. Standard LLVM mechanism; requires care during the IR generation step. |
| Vintage processor back ends require substantial engineering not budgeted in V0.4.0 | Defer to V0.4.x or V0.5+ as exploratory. V0.4.0's primary targets are x86-64 Linux, with other modern targets added as the toolchain stabilises. |

## Out of scope

- **Vintage processor back ends.** 6502 family, 68000 family, Z80, and similar are exploratory. V0.4.x or V0.5+ research, not V0.4.0 deliverables. V0.4.0 mentions them only to clarify scope.
- **JIT compilation.** V0.4.0 is AOT only. JIT could be added later if a use case demanded it; the architecture does not preclude it but does not deliver it.
- **Multi-CPU code generation.** Threading, synchronisation primitives, and parallel sub-coroutine execution are V0.6+ at earliest.
- **GPU code generation.** Not a Keleusma target.
- **Non-LLVM back ends.** A custom code generator targeting specific architectures directly (bypassing LLVM) is out of scope. LLVM is the only back end for V0.4.0.
- **Per-target WCET analysis.** V0.5+ research per the WCET section above.
- **Profile-guided optimisation.** Out of scope; the bounded-resource discipline does not naturally compose with PGO.
- **Link-time optimisation across the entire program.** LTO is permitted within a single hot-replacement boundary but suppressed across boundaries. The detail is documented per build mode.

## Resolved design questions

The strategy's open questions were addressed in a dedicated research loop (2026-05-21) with a post-hoc web-research verification pass. Each recommendation below is summarised; the full record lives under `tmp/research/r4_*.md` and `tmp/research/WEB_RESEARCH_APPENDIX.md`.

### LLVM coroutine intrinsic family (R4.1, corrected)

**Recommendation**. Returned-continuation (`@llvm.coro.id.retcon`). The intrinsic accepts allocator and deallocator function pointers and is documented for fixed-size buffer use, matching Keleusma's per-coroutine arena-slot design.

**Correction history**. The original R4.1 recommendation specified switched-resume (`@llvm.coro.id`). Post-hoc web verification of LLVM's documentation surfaced that switched-resume does not accept allocator function pointers in the intrinsic itself; returned-continuation does. The intent of R4.1 (per-coroutine custom allocator with fixed-size buffer) maps directly onto retcon. See `tmp/research/WEB_RESEARCH_APPENDIX.md` for the correction.

**Confidence**. High after correction. The intrinsic signatures are documented explicitly in the LLVM Coroutines reference.

### LLVM version pin (R4.3, revised)

**Recommendation**. Pin to LLVM 19 for V0.4.0. Upgrade through 20, 21, 22 in V0.4.x point releases as the ecosystem moves.

**Revision history**. R4.3 originally recommended LLVM 17 based on a 2025 cutoff in the source documents. Post-hoc verification surfaced that LLVM 22.1 is the current stable as of May 2026. Pinning at LLVM 17 is now three releases behind. LLVM 19 balances maturity (two releases old, widely available in distributions, supported by inkwell 0.8+) with currency.

**Confidence**. Medium. The choice between LLVM 18, 19, 20, or even 22 is a judgment call; the point is that 17 is too old.

### Rust LLVM bindings (R4.4)

**Recommendation**. Primary `inkwell` with the feature flag corresponding to the chosen LLVM version (for instance `llvm19-1` if pinning to LLVM 19). Escape hatch `llvm-sys` for intrinsics inkwell does not expose, concentrated in a small `coro_intrinsics.rs` module. `melior` (MLIR) deferred to long-term watch list.

**Update**. inkwell 0.8.0 supports LLVM 11 through 22; inkwell 0.9.0 (April 2026) maintains active development. Whether inkwell exposes `coro.id.retcon` with a safe wrapper requires source-tree audit when implementation begins; if not, expect the `coro_intrinsics.rs` escape-hatch module to grow.

**Confidence**. High for the broad recommendation; medium for the specific feature-flag name without source-tree confirmation.

### Symbol mangling scheme (R4.2)

**Recommendation**. Format `_K<v>_<purity><category>_<module_path>_<function_name>[_<typeargs>]`. Versioned at v=1. Purity P/I/T (pure, impure, transitive). Category F/Y/L (fn, yield, loop). Module path with `__` separator. Type arguments as 16-hex-digit SHA-256 truncation. ASCII-only, linker-compatible across V0.4.0 targets. Demangleable via a `keleusma demangle` tool.

**Confidence**. High. The format meets all five mangling constraints (uniqueness, ASCII-only, stability across compiler versions, demangleability, reasonable compactness).

### Cross-platform target order (R4.5)

**Recommendation**.

- **Tier 1 (V0.4.0 ship)**: x86-64 Linux, AArch64 Linux, macOS (both architectures).
- **Tier 2 (V0.4.x)**: Windows MSVC, Cortex-M55 (`thumbv8m.main-none-eabihf`), Cortex-M4 (`thumbv7em-none-eabihf`).
- **Tier 3 (V0.5+)**: RISC-V, Wasm, vintage CPUs.

**Confidence**. High. Tier classification follows LLVM back-end maturity, target-platform relevance, and the existing `examples/rtos/` infrastructure for the Cortex-M55 testbed.

## Open questions

These remain unresolved after the 2026-05-21 research pass.

1. **Debug information generation.** DWARF on Linux and macOS, PDB on Windows. Whether V0.4.0 ships with full debug info, minimal debug info, or no debug info is a scope decision deferred to implementation.

2. **Build-system integration.** Cargo is the natural choice for the Rust-host side. Whether the Keleusma-side build system is a Cargo extension, a separate tool invoked from Cargo, or a fully separate build system is a UX question.

3. **Retcon-once optimisation.** Whether any subset of sub-coroutines benefits from the `@llvm.coro.id.retcon.once` form (for one-shot coroutines that yield exactly once before completing) is an optimisation question. Profile-driven.

4. **R4.1 milestone M1 execution.** The empirical validation of the retcon allocator hook in a small standalone IR fragment has not been performed. This is the single highest-risk technical item in V0.4.0 and should be executed before broader IR generator implementation begins.

5. **Native-side WCET cost models.** The bytecode WCET model in V0.2.0 does not translate to native execution. New cost models need calibration on each Tier 1 platform, analogous to what `keleusma-bench` did for the VM. Not in any R-doc; surfaced by `tmp/research/IMPLEMENTATION_ORDER.md`.

## How V0.4.0 research informs V0.5.0

Three V0.5.0 refinements that the V0.4.0 research surfaces.

First, **hot-replacement granularity is a build-mode choice, not a fixed property**. The V0.5.0 strategy currently assumes module-level hot replacement is uniformly available in the native shape. V0.4.0 research shows that cross-module inlining suppression imposes a real cost; some V0.5.0 deployments will choose the performance-friendly build and forgo native-level hot replacement, falling back to bytecode-shape hot replacement or to binary-restart upgrade. V0.5.0 should document the choice explicitly.

Second, **native WCET claims are best-effort, not hard**. The V0.5.0 strategy mentioned host bounds without committing to their character. V0.4.0 research clarifies that the bytecode-level WCET model is the verification artefact; the native lowering is faster in expectation but the bytecode bound is a soft upper bound on native execution, not a tight bound. V0.5.0 deployments that need hard timing use the bytecode shape with a verified-cost VM. V0.5.0 deployments that use the native shape accept the soft bound. This should be called out explicitly in V0.5.0's risk and out-of-scope sections.

Third, **the sub-coroutine specification's "new opcodes" become "LLVM coroutine intrinsic calls" at the native shape**. The bytecode-level opcodes (`SpawnCoroutine`, `ResumeCoroutine`, `ReleaseCoroutine`) lower to the LLVM coroutine intrinsics during V0.4.0 compilation. The same sub-coroutine surface syntax compiles to either bytecode opcodes or LLVM intrinsics depending on deployment shape. The [sub-coroutine spec](../architecture/SUB_COROUTINES.md) should note this lowering explicitly once V0.4.0 has confirmed the intrinsic-to-opcode mapping.

## References

- LLVM Project Documentation, *LLVM Language Reference Manual*. The LLVM IR specification.
- Gor Nishanov, *Coroutines in LLVM*. The LLVM coroutine intrinsics design document.
- *The Rust rustc Development Guide*, particularly the chapters on LLVM IR generation. The closest production precedent for what V0.4.0 implements.
- Apple, *The Swift Programming Language Reference*. Swift's async/await as a production-deployed LLVM coroutine use case.
- WebAssembly Working Group, *WebAssembly Specification*. The portable-bytecode-as-IR pattern with native lowering.
- Lindholm, Yellin, Bracha, Buckley, *The Java Virtual Machine Specification*, Java SE 8 Edition, Addison-Wesley, 2015, ISBN 978-0-13-390590-8. JVM bytecode plus HotSpot JIT.
- Microsoft, *.NET Common Language Infrastructure, Standard ECMA-335*, sixth edition, 2012. CIL bytecode plus tiered JIT and AOT paths.
- Jose Valim and Joe Armstrong, *Erlang and Elixir for Imperative Programmers*. Erlang BEAM bytecode plus the historical HiPE and the current JIT.
- llvm-mos project, *llvm-mos LLVM Distribution*. The out-of-tree 6502 LLVM fork.
- LLVM Project, *M68k Backend Documentation*. The upstream Motorola 68000 back end.
- Cross-reference: [V0_3_0_SELF_HOSTING.md](./V0_3_0_SELF_HOSTING.md) for the self-hosted compiler that V0.4.0 consumes as input.
- Cross-reference: [V0_5_0_KELEUSMA_HOST.md](./V0_5_0_KELEUSMA_HOST.md) for the Keleusma-hosted host that V0.4.0 enables.
- Cross-reference: [SUB_COROUTINES.md](../architecture/SUB_COROUTINES.md) for the sub-coroutine specification whose bytecode opcodes V0.4.0 lowers to LLVM coroutine intrinsics.
