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

**What "retirement" does and does not mean.** Per `V0_5_0_KELEUSMA_HOST.md`, V0.5.0 closes the
host loop, not the VM loop. A minimal Rust shim remains for the operating-system interface
(process launch, allocator binding, signal handling) and to host the runtime VM where bytecode
execution is selected. The shipping VM stays in Rust through V0.5.0; replacing the VM itself is a
V0.6-and-later aspiration. The meta-circular runtime that V0.2.x self-hosted is an executable
specification of VM semantics, not the shipping VM, and it does not change this boundary.

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

### E. Operating-system interface natives

The minimal Rust shim: file open, read, write, close; command-line argument iteration; process
exit code; and stdout and stderr write. This is the irreducible host seam. Bare-metal
hardware-control natives are deferred to V0.5.x.

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

## Dependency ordering and indicative release mapping

Indicative and revised as increments land.

| Order | Milestone | Workstreams | Gate |
|-------|-----------|-------------|------|
| 1 | Sub-coroutines and purity | A, B | The pipeline stages run as sub-coroutines from an impure driver, purity discipline enforced. |
| 2 | Modules and partitions | C, D | The host and compiler are multi-module with declared arena partitions. |
| 3 | OS shim | E | The minimal file and stdio native surface is in place. |
| 4 | Host driver in Keleusma | F | The `impure fn main` compiler driver runs the full pipeline against a source file and exits, native-hosted. |
| 5 | Live update | G | Structured live code update through the host. |
| 6 | **Rust host retirement → V0.5.0** | F (both shapes), plus widening | The host application is Keleusma; only the OS shim and the VM remain in Rust. |

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
4. The only operator-facing Rust remaining is the minimal OS shim and the runtime VM, both a
   reviewed trust base, with the Rust host driver retired to a differential oracle pending its
   own retirement decision.
