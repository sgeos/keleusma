# V0.3.X Roadmap: Native Code Generation

> **Navigation**: [Roadmap](./README.md) | [Documentation Root](../README.md)

**Status**: Preliminary. Gated on the V0.3.0 full self-hosting solution landing. This document
mirrors [`V0_2_X_ROADMAP.md`](./V0_2_X_ROADMAP.md): it sequences the V0.3.x release line toward
its milestone. The milestone is **V0.4.0, full native code generation**, whose architecture
stays authoritative in [`V0_4_0_NATIVE_CODEGEN.md`](./V0_4_0_NATIVE_CODEGEN.md). Because the
V0.3.x work has not started, the workstream detail here is coarser than V0.2.x and will sharpen
as V0.3.0 stabilises. It is a plan, not a promise.

## Purpose and version semantics

The goal of the V0.3.x line is to add a native code generator to the self-hosted toolchain, so
that a Keleusma module, already verified as bytecode, lowers to native object code that links
as a static library against a host and runs at native speed with the same bounded-resource
guarantees. **V0.3.x is native-code-generation work in progress; V0.4.0 is full native code
generation**, meaning the whole language lowers to native across the target set, and native is
a first-class deployment shape alongside bytecode.

This is the second rung of the version ladder introduced in `V0_2_X_ROADMAP.md`. It inherits
that ladder's discipline: a subset first, one reviewable increment per release, the prior
artefact retained as a differential oracle, and language feature additions and revisions
expected along the way rather than front-loaded.

The bytecode shape does not go away. Per `V0_4_0_NATIVE_CODEGEN.md`, native is a second
deployment shape; the bytecode remains the verification artefact and the portable form, and the
VM remains for embedding and fallback.

## Entry baseline (what V0.3.0 hands to V0.3.x)

- A self-hosted compiler that lowers the full language to bytecode, its output byte-identical to
  the retired Rust reference over a full-language corpus.
- A self-hosted validator that reproduces the whole `verify()` verdict, including the
  `Trap`-scanning trap-freedom check (V0.2.x Workstream C), so that "total opcodes plus explicit
  `Trap`" is the ISA the native lowering consumes.
- A hosted meta-circular runtime as an executable specification of the VM semantics that native
  code must preserve.

## Workstreams

Each workstream targets a self-hosting-style subset first (lower the self-hosted compiler itself
to native), then widens to the full language and target set. Workstream sources are the sections
of `V0_4_0_NATIVE_CODEGEN.md`.

### A. Bytecode-to-LLVM-IR lowering

The core code generator. Translate verified bytecode chunks to LLVM IR, then let LLVM lower to
native object files. First pass lowers the subset the self-hosted compiler emits; full pass
lowers every opcode of the full-language ISA.

### B. Sub-coroutine lowering via LLVM coroutine intrinsics

The load-bearing primitive. A Keleusma `loop`/`yield`/`resume` coroutine lowers to an LLVM
coroutine so a host can call coroutine-driven native functions whose state machines LLVM
manages. This is the piece the V0.4.0 strategy identifies as where the risk concentrates.

### C. Arena-resident coroutine frames and the native arena model

Coroutine frames live in the arena, not the C stack, preserving the bounded-WCMU model in
native code. The native runtime reproduces the arena bump-and-reset discipline.

### D. Static-library linkage, the host ABI, and foreign-linkable object files

The native artefact links as a static library against a Rust host and exposes sub-coroutine
entry points. The ABI between native Keleusma code and the host, including the native form of
the marshalling boundary, is specified here (it is the native counterpart of the V0.2.x native
ABI definition).

Beyond a Rust host, the artefact links into a project written in **any** language. This is the
incremental-adoption path: rather than replace a whole system, an operator writes one
safety-critical piece in Keleusma and links its object file into existing C, Rust, or Ada code.
The canonical example is an **interrupt handler object file** (see the V0.4.x low-level tier,
`V0_4_X_ROADMAP.md` Workstream I), whose hard problems, stack size, execution time, and
trap-freedom, are exactly Keleusma's strengths. The requirements:

- A **stable C ABI**: the object exports a known symbol under the platform C ABI, freestanding,
  so it links without dragging in a Keleusma runtime or VM. Symbol mangling is fixed per the
  V0.4.0 strategy's resolved question.
- **The bounds exported as a linkable, self-describing contract.** The object emits its
  WCMU-sized arena requirement (a symbol or linker constant) so the foreign build statically
  provisions a correctly sized buffer and passes it in; the WCET bound and the trap-free verdict
  export the same way, as machine-readable metadata beside the object. The foreign project
  inherits "this artefact needs N bytes of arena, runs in at most C cycles, and cannot trap" as a
  checkable claim.
- **The shared-data region as a C-representable layout** at a known symbol, so the foreign code
  and the Keleusma object agree on the cross-boundary state.

The trust boundary is explicit: the guarantees are **sound on the Keleusma side and contingent on
the foreign caller honoring the contract** (provisioning the declared arena, respecting the
shared layout, calling under the declared discipline). Keleusma cannot verify the foreign side;
this is the standard FFI island boundary, the verified artefact being the object file, its
enforcement being the linker's and the foreign project's responsibility.

### E. WCET and WCMU preservation across native compilation

The bounded-resource guarantees must survive lowering. The bytecode is the verification
artefact; native compilation must not invalidate the WCET and WCMU bounds proven on it. This
workstream defines how the bounds map onto native code and how they are re-checked or attested.

### F. Partial-operation native lowering

The native lowering of the total-opcode-plus-`Trap` ISA (B35 P8 in the backlog). Each total
partial operation lowers to native code that produces its flag; an unhandled operation lowers to
a native trap, a handled one to a native branch. The V0.2.x trap design and this lowering are
two ends of the same mechanism.

### G. The flat-machine ISA redesign

Native code generation enables the deferred flat-machine ISA (B28): an untyped byte operand
stack, composite values as pure bytes, offsets baked into access instructions, and the kind
carried by the opcode. The V0.2.x Rust runtime deliberately keeps a tagged stack; native
supersedes it by resolving everything statically. This workstream lands that redesign where
native makes it free.

### H. Hot replacement at the native level

Structured live code update for native artefacts, the native counterpart of bytecode hot swap,
carried forward into the V0.5.0 host.

### I. Language feature additions and revisions

Surface-language work continues through V0.3.x as native lowering surfaces constraints, in the
same co-evolution posture as V0.2.x.

## Dependency ordering and indicative release mapping

Indicative and revised as increments land.

| Order | Milestone | Workstreams | Gate |
|-------|-----------|-------------|------|
| 1 | Subset bytecode lowers to native | A (first pass) | The self-hosted compiler's own bytecode runs correctly as native code, differential-tested against the VM. |
| 2 | Sub-coroutines native | B, C | Coroutine-driven native functions callable from a host, frames arena-resident. |
| 3 | Host linkage and ABI | D | A native static library links against a Rust host and runs a real program. |
| 4 | Bounds preserved | E, F | WCET and WCMU bounds carried onto native; the total-opcode-plus-`Trap` ISA lowered. |
| 5 | Flat-machine ISA and hot replacement | G, H | The flat-machine ISA in native; native hot swap. |
| 6 | **Full native code generation → V0.4.0** | A (full), plus widening | The full language lowers to native across the target set; native is a first-class deployment shape. |

## The oracle and trust story

Native code raises the same trust question as a self-hosted validator: native output that
diverges from the verified semantics fails the bounded-resource contract silently. The
discipline mirrors V0.2.x:

- **The VM stays as a differential oracle.** Every native lowering is differential-tested for
  identical observable behaviour and identical resource bounds against VM execution of the same
  bytecode, over a growing corpus, until independently reviewed.
- **The bytecode remains the verification artefact.** Verification happens on bytecode; native
  is the deployment shape. Native lowering must be shown to preserve, not re-establish, the
  proven bounds.

## Cross-cutting concerns

- **Target set and cross-compilation.** The target order (per the V0.4.0 strategy's resolved
  questions) governs which architectures land first, including embedded and vintage targets.
- **Toolchain integration.** Linker, package manager, container, and signed-installer pipelines
  are the reason native matters for standalone deployment.
- **LLVM version pin and bindings.** Fixed per the V0.4.0 strategy's resolved questions.
- **Debug metadata in native.** Source mapping must survive to native for stack traces.

## Open decisions

Carried from `V0_4_0_NATIVE_CODEGEN.md`'s open questions, resolved as V0.3.x approaches:

1. **WCET on native is hard or best-effort.** Whether native execution preserves a hard WCET
   bound or a best-effort one; the V0.5.0 host strategy already treats native WCET as
   best-effort, which informs this.
2. **JIT versus AOT scope.** Whether V0.3.x pursues ahead-of-time only or admits a JIT path.
3. **Flat-machine ISA timing.** Whether the B28 flat-machine redesign lands within V0.3.x or is
   staged into V0.4.x.

## Relationship to other roadmaps

- [`V0_2_X_ROADMAP.md`](./V0_2_X_ROADMAP.md): the prior rung; V0.3.0 is its milestone and this
  line's entry baseline.
- [`V0_4_0_NATIVE_CODEGEN.md`](./V0_4_0_NATIVE_CODEGEN.md): authoritative architecture for the
  V0.4.0 milestone this line targets.
- [`V0_4_X_ROADMAP.md`](./V0_4_X_ROADMAP.md): the next rung; V0.4.0 is its entry baseline, and
  its milestone V0.5.0 (Rust host retirement) depends on native sub-coroutines from this line.

## Success criteria

The V0.3.x line is complete, and V0.4.0 is ready, when:

1. The full language lowers to native object code across the committed target set.
2. Native artefacts link as static libraries against a host and expose sub-coroutine entry
   points via LLVM coroutine intrinsics.
3. The proven WCET and WCMU bounds are carried onto native and differential-tested against the
   VM, with the total-opcode-plus-`Trap` ISA lowered natively.
4. Native and bytecode coexist as first-class deployment shapes, the bytecode remaining the
   verification artefact.
