# V0.4.X Roadmap: Rust Host Retirement

> **Navigation**: [Roadmap](./README.md) | [Documentation Root](../README.md)

**Status**: Preliminary. Gated on the V0.4.0 full native code generation milestone landing.
This document mirrors [`V0_2_X_ROADMAP.md`](./V0_2_X_ROADMAP.md) and
[`V0_3_X_ROADMAP.md`](./V0_3_X_ROADMAP.md): it sequences the V0.4.x release line toward its
milestone. The milestone is **V0.5.0, Rust host retirement**, meaning a host written in
Keleusma, whose architecture stays authoritative in
[`V0_5_0_KELEUSMA_HOST.md`](./V0_5_0_KELEUSMA_HOST.md). Because the V0.4.x work has not started,
the detail here is coarser than V0.2.x and sharpens as V0.4.0 stabilises. It is a plan, not a
promise.

## Purpose and version semantics

The goal of the V0.4.x line is to move the toolchain's host application from Rust into Keleusma,
so that the compiler driver, the command-line surface, the file orchestration, and the pipeline
coordination all live in Keleusma source and inherit the verifier's bounded-resource guarantees.
**V0.4.x is host-migration work in progress; V0.5.0 is Rust host retirement**, meaning the host
application is a Keleusma program and only a minimal Rust shim remains.

This is the third rung of the version ladder introduced in `V0_2_X_ROADMAP.md`, with the same
discipline: a subset first, one reviewable increment per release, the prior artefact retained as
a differential oracle, and language feature additions and revisions expected along the way.

**What "retirement" does and does not mean.** A retired host means the program is hosted by the
operating system, or by bare metal directly, exactly as any ahead-of-time-compiled program is.
The interface it crosses to reach the world, the startup sequence, the syscall or device
boundary, the memory the arena draws from, and signal or interrupt handling, is a **universal
native-program concern, not a Keleusma-specific one**. It is a tractable, well-trodden problem
that every systems language solves in its own runtime, and Keleusma's quirks are semantic
(totality, bounded WCET and WCMU, coroutine structure) rather than anything that changes how a
compiled program reaches the operating system. The `examples/rtos/` cooperative microkernel
already runs bare metal, so the target is demonstrably reachable.

This tightens the framing in `V0_5_0_KELEUSMA_HOST.md`, which describes a minimal Rust shim
remaining. Under this roadmap the shim need not be Rust: it is the ordinary OS-or-bare-metal
boundary, and once V0.4.0 native code generation exists it can itself be Keleusma-native, the
platform's syscalls emitted directly or bare metal from the reset vector up. **Expressing even
the entry stub in Keleusma is a deliberate language goal, not a concession to a non-Keleusma
crt0.** The reset-vector entry, the initial stack-pointer load before any stack-using code can
run, and the raw syscall or trap instruction are the irreducible lowest rung, and reaching them
requires the language to grow a demarcated low-level tier (Workstream I) whose primitive is an
instruction-emitting escape hatch, in the manner of Rust inline assembly or Ada machine-code
insertions. The bounded WCET and WCMU and totality guarantees are scoped to the pure core above
that tier; the tier is an explicit impure, unsafe island, which is the same discipline every
systems language with high-level guarantees uses. Two axes stay distinct. The **host application** (driver,
orchestration) is what V0.5.0 retires. The **runtime VM** is a separate concern that exists only
for the bytecode-interpretation deployment shape; the native deployment shape runs no VM, so a
native, Keleusma-hosted program on an OS or bare metal can carry no Rust in its operator path at
all. The Rust VM survives only as long as bytecode interpretation is offered as a deployment
shape, and replacing it there is the V0.6-and-later question. The meta-circular runtime that
V0.2.x self-hosted is an executable specification of VM semantics, not the shipping VM.

## Entry baseline (what V0.4.0 hands to V0.4.x)

- A native code generator that lowers the full language to native and links as a static library
  against a host, with sub-coroutine entry points via LLVM coroutine intrinsics. The Keleusma
  host runs as native code so that its orchestration is at native speed rather than
  interpretation overhead.
- The self-hosted compiler and validator from V0.3.0, now compilable to native.

## Workstreams

Each workstream targets a working host over a subset of driver behaviour first, then widens.
Workstream sources are the prerequisites and architecture sections of `V0_5_0_KELEUSMA_HOST.md`.

### A. Sub-coroutines (callable ephemeral `loop`)

The enabling runtime primitive: a `loop` function callable as a sub-coroutine from inside another
`loop` function or from an impure driver, each with its own program counter, call-frame stack,
operand stack, and arena slot. The specification is a separate design piece
(`docs/architecture/SUB_COROUTINES.md`); this line consumes it. The host orchestrates the
compiler pipeline stages as sub-coroutines.

### B. Three-mode purity discipline

A pure-by-default attribute system with three modes: pure (no I/O, no impure callbacks), impure
(may perform I/O and accept any callback), and transitive (pure body, may accept and pass through
impure callbacks, effective purity inherited from the callsite). This is what lets an I/O-driven
host coexist with the pure, bounded core.

### C. File-based modules with interface declarations

A module system in the Modula-2 and Ada tradition: a separately compiled implementation paired
with an explicit, auditable interface declaration, with cross-module type checking, generics
resolution, and monomorphization observing module boundaries. The host and the compiler become
multi-module programs.

### D. Declared sub-DAG arena partitions

Source-level partition-boundary declarations, each assigned its own arena slot, with
master-WCMU-based allocation. Auto-detection is deferred; declaration is the V0.5.0 mechanism.

### E. Operating-system or bare-metal interface

The universal boundary a native program crosses to reach the world: file open, read, write,
close; command-line argument iteration; process exit code; and stdout and stderr write on a
hosted OS, or the device and interrupt boundary on bare metal. This is the designated impure
seam (Workstream B): the syscalls and device reads are inherently impure and unbounded, and
Keleusma's bounded, total guarantees are about the pure core inside the seam. That is the
standard bounded-core-with-impure-edge shape of any real-time system, not a Keleusma
peculiarity. The layer is small and target-specific; it is not a permanent Rust dependency and
is expressed in Keleusma over the low-level tier (Workstream I), including the entry stub. The
V0.5.0 scope is the file-and-stdio host;
bare-metal hardware-control natives (volatile access, interrupt registration) are the V0.5.x
widening, for which `examples/rtos/` is the reachable-target precedent.

### F. The host driver in Keleusma

The retirement itself. The `keleusma` command-line driver becomes a Keleusma program in one of
two first-class shapes: an `impure fn main` that terminates per invocation (the compiler driver),
and an `impure loop main` that is productively divergent with productivity delegated to
sub-coroutines (long-running servers, RTOS tasks, controllers). The compiler driver is the first
shape and the canonical example.

### G. Structured live code update

Verified live code update integrated into the host, the host-level counterpart of native hot
replacement from V0.3.x.

### H. Language feature additions and revisions

Surface-language work continues through V0.4.x as the host and module system surface constraints,
in the same co-evolution posture as the earlier rungs.

### I. Low-level and freestanding language tier

The language capability that lets the OS-or-bare-metal interface, and the entry stub itself, be
Keleusma rather than a foreign stub. It is a deliberate language goal that Keleusma express
ASM-level logic directly, accepting the feature additions that requires. The tier comprises:

- A **freestanding mode** with no assumed runtime or startup, and a definable entry point (the
  process entry on a hosted OS, the reset vector on bare metal).
- **Raw and volatile memory access** for memory-mapped device registers and fixed-address I/O,
  distinct from the arena and shared-data models.
- A **direct syscall or trap intrinsic** (`svc`, `ecall`, `syscall`, `int`) as the raw OS or
  supervisor boundary.
- **Interrupt and exception handlers** with the platform's required calling convention (naked
  entry, no prologue), and control over **memory layout and linker sections** (the vector table,
  code and data placement).
- At the bottom, an **instruction-emitting escape hatch** (inline-assembly-equivalent or a set
  of instruction intrinsics) for the handful of operations a stack-based functional core cannot
  synthesize, such as loading the initial stack pointer.

**State-and-handlers decomposition (minimal new surface).** The tier reuses two constructs
Keleusma already has, so it is a small addition rather than a new sublanguage. The low-level
**state** is a `shared data` block: the vector table, the syscall scratch and argument area,
saved context, and MMIO base pointers all live there. The only extensions are placement (a block
pinned to a fixed physical address for a memory-mapped device, or to a named linker section for
the vector table) and volatile access semantics. The **handlers** are ordinary functions over
that block; only the entry and exit wrapper needs the tier, a naked-entry attribute plus the
escape hatch for the irreducible instructions (the trap or `eret` return, the context save and
restore), while the body is normal Keleusma. This matches the Oberon `SYSTEM`-module shape, a
demarcated primitive set plus ordinary procedures, which is the closest precedent and is already
in the roadmap's Wirth lineage.

**Interrupt handlers and WCMU-sized arenas.** A hardware-invoked handler fires asynchronously and
must not disturb the interrupted context, so it runs against **its own arena paired with the
shared-data block**, not the interrupted code's arena. The arena is the handler's execution
memory (its operand stack, call frames, and transient composites are arena-resident); the shared
block carries the cross-boundary state. This is where the definitive-WCMU property pays off
directly: the arena a handler needs is its computed WCMU, and a program whose handler WCMU cannot
be bounded is rejected rather than shipped to overflow. Sizing an interrupt stack is a
notoriously error-prone embedded problem that C and Rust answer with over-provisioning or a
separate, often-unsound worst-case-stack tool; here it is a sound, compiler-computed number. The
one honest addend is the register-save frame the naked entry writes, which lives in the low-level
tier outside WCMU but is a fixed, architecture-known constant, so the arena is
`WCMU(body) + fixed_save_frame`. Nested, preempting interrupts sum their WCMUs along the deepest
preemption chain, which WCMU also makes computable; a non-nesting design takes the maximum. Each
handler arena is a declared arena partition (Workstream D, master-WCMU-based allocation); the
sub-coroutine slot model (Workstream A) is the general form of an isolated context, of which a
handler arena is the simplest case.

**Foreign-linkable handler objects.** A Keleusma interrupt handler is a strong incremental
-adoption artefact: compiled to a native object file (a V0.4.0 native-codegen capability,
`V0_3_X_ROADMAP.md` Workstream D), it links into an existing C, Rust, or Ada firmware so an
operator writes one safety-critical ISR in Keleusma without rewriting the system. The object
exports a C-ABI symbol and, as a self-describing contract, its WCMU-sized arena requirement, its
WCET bound, and its trap-free verdict, so the foreign build provisions the arena and inherits the
claims. Two entry shapes: the object owns the naked interrupt entry (self-contained, but its
entry wrapper is low-level-tier code), or the foreign project keeps its own trampoline and calls
a plain C-ABI Keleusma function (which keeps the Keleusma side entirely in the verified core plus
a C export, minimizing the unsafe surface). The guarantees are sound on the Keleusma side and
contingent on the foreign caller honoring the contract, the standard FFI island boundary.

This tier is an explicit **impure, unsafe island**: totality and bounded WCET and WCMU do not
hold inside it, and the verifier's soundness is scoped to the pure core above it, exactly as
Rust `unsafe`, Ada `System.Machine_Code`, and the Oberon `SYSTEM` pseudo-module are demarcated. Its timing is host-attested or best-effort, per
the native WCET stance. It widens Keleusma's positioning from a bounded embeddable stream
processor toward a bounded systems language with a low-level escape hatch, a deliberate
strengthening that also widens the trusted surface. The tier is a language-feature theme that
begins here because bare metal forces it, but its primitives are designed as general language
additions, not host-migration one-offs.

## Dependency ordering and indicative release mapping

Indicative and revised as increments land.

| Order | Milestone | Workstreams | Gate |
|-------|-----------|-------------|------|
| 1 | Sub-coroutines and purity | A, B | The pipeline stages run as sub-coroutines from an impure driver, purity discipline enforced. |
| 2 | Modules and partitions | C, D | The host and compiler are multi-module with declared arena partitions. |
| 3 | Low-level and freestanding tier | I | A freestanding Keleusma program reaches the OS by its own syscall path or runs bare metal from the reset vector, with the instruction-emitting escape hatch in place. |
| 4 | OS or bare-metal interface | E | The file and stdio (hosted) or device (bare-metal) native surface is expressed in Keleusma over the low-level tier. |
| 5 | Host driver in Keleusma | F | The `impure fn main` compiler driver runs the full pipeline against a source file and exits, native-hosted. |
| 6 | Live update | G | Structured live code update through the host. |
| 7 | **Rust host retirement → V0.5.0** | F (both shapes), plus widening | The host application and the OS-or-bare-metal interface, including the entry stub, are Keleusma; in the native deployment shape no Rust remains in the operator path. |

## The oracle and trust story

The host orchestrates the compiler and validator, so a wrong host produces wrong artefacts. The
discipline mirrors the earlier rungs:

- **The Rust host stays as a differential oracle** until the Keleusma host is independently
  reviewed: same inputs, same outputs, same exit behaviour.
- **The minimal shim is the audited trust base.** Retirement shrinks the operator-facing Rust to
  the OS shim and the VM; those become the reviewed trust boundary, and everything above them is
  Keleusma subject to the verifier.

## Cross-cutting concerns

- **Impurity and I/O bounds.** I/O-performing functions sit outside the pure bounded core; the
  discipline must keep the bounded guarantees meaningful at the impure boundary.
- **Interface fingerprinting.** The module interface hash (per the V0.5.0 strategy's resolved
  questions) governs cross-module compatibility and live update.
- **Native WCET is best-effort.** The V0.5.0 strategy already treats native WCET as best-effort,
  not hard; the host inherits that.
- **Bare-metal scope.** Hardware-control natives are deferred to V0.5.x; V0.4.x targets the file
  and stdio host only.

## Open decisions

Carried from `V0_5_0_KELEUSMA_HOST.md`'s open questions, resolved as V0.4.x approaches:

1. **Hot-replacement granularity** as a build-mode choice.
2. **Sub-coroutine surface syntax** (a resolved question in the strategy, revisited as
   implementation approaches).
3. **VM-loop closure horizon.** Whether and when replacing the shipping VM itself (V0.6+) becomes
   a committed goal; V0.5.0 informs but does not decide it.
4. **Low-level tier design (Workstream I).** Whether the instruction-emitting escape hatch is an
   inline-assembly form, a fixed set of instruction intrinsics, or both; how the low-level tier
   is demarcated syntactically and how far the verifier's soundness scoping extends around it;
   and which targets' syscall and reset-vector conventions land first. This is a language-design
   decision with a security-surface cost, since the tier is where the bounded guarantees are
   suspended.

## Relationship to other roadmaps

- [`V0_3_X_ROADMAP.md`](./V0_3_X_ROADMAP.md): the prior rung; V0.4.0 is its milestone and this
  line's entry baseline, and native sub-coroutines from it are a hard prerequisite here.
- [`V0_5_0_KELEUSMA_HOST.md`](./V0_5_0_KELEUSMA_HOST.md): authoritative architecture for the
  V0.5.0 milestone this line targets.
- [`V0_2_X_ROADMAP.md`](./V0_2_X_ROADMAP.md): the first rung; establishes the ladder discipline
  this line inherits.

## Success criteria

The V0.4.x line is complete, and V0.5.0 is ready, when:

1. The `keleusma` compiler driver is a Keleusma program in the `impure fn main` shape, running
   the full self-hosted pipeline against a source file and exiting, native-hosted.
2. The `impure loop main` long-running shape is demonstrated for at least one non-compiler
   driver.
3. Sub-coroutines, the three-mode purity discipline, file-based modules with interface
   declarations, and declared arena partitions are all in place and verifier-enforced.
4. In the native deployment shape, the operator-facing path carries no Rust: the OS-or-bare-metal
   interface is Keleusma-native atop the platform's standard entry stub, and no VM runs. The Rust
   runtime VM remains only for the separate bytecode-interpretation deployment shape (its
   replacement is the V0.6-and-later question), and the Rust host driver is retired to a
   differential oracle pending its own retirement decision.
