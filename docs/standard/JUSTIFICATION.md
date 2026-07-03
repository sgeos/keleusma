# The Keleusma Justification Notes

**Status.** Draft. This document is informative. It carries the rationale for the
normative [`STANDARD.md`](STANDARD.md) and the comparative review of the standards
that informed it. It never defines conformance and is never cited to resolve a
conformance question. It cites the Standard by section number, external standards by
name, and internal documents by path.

---

## J1. Purpose and relationship to the Standard

The Standard states the contract and only the contract. This document states why
each part of the contract is the way it is. The two-document structure, being a
normative specification beside an informative rationale, follows the Ada Reference
Manual with its Rationale, the Definition of Standard ML with its Commentary, and
the World Wide Web Consortium process of a normative recommendation beside a primer.
The document conventions, being the requirement keywords, the uniform rule template,
the stability levels, and the direction of authority, are drawn from the same three
sources. The blueprint that fixes the structure of both documents is
[`META_SPECIFICATION.md`](META_SPECIFICATION.md).

The value proposition of Keleusma is a definitive worst-case execution time and
worst-case memory usage. A program whose bounds cannot be proven is rejected. This
single commitment shapes every other decision, from the prohibition of recursion to
the fixed arena to the single canonical representation.

---

## J2. Comparative standards review

This section is organized as one subsection per reviewed standard. Each subsection is
the authoritative treatment of that standard, stating briefly what it is, the
specific property Keleusma adopts or deliberately rejects, and the Standard sections
it informs. Depth is proportional to influence. A closing index in J2.20 tabulates
each standard against the sections it informs.

### J2.1 WebAssembly

WebAssembly is a portable stack-machine bytecode with a formal small-step semantics,
a validation pass over the bytecode, and a binary format beside a text format. It is
the closest structural template for Keleusma. Keleusma adopts the small-step
operational style for the instruction semantics of Standard 6 and Standard 7, the
validation-before-execution posture of Standard 8, and the separation of a binary
wire format from the abstract semantics. Keleusma diverges by fixing the memory
budget statically rather than growing linear memory, and by carrying the worst-case
bounds in the module header of Standard 9.1. It informs Standard 6, Standard 7, and
Standard 8.

### J2.2 The Java Virtual Machine

The Java Virtual Machine defines a class-file format and a bytecode verifier that
reconstructs operand-stack and local-variable types by abstract interpretation, so
that verified bytecode is memory-safe without runtime type checks. This is the
precedent for the typed operand-stack pass of Standard 8.2, which reconstructs
operand types to validate the baked composite offsets of Standard 5.2.6. Keleusma
narrows the ambition, since it does not support dynamic loading of mutually
recursive types and forbids recursion outright. It informs Standard 8.2 and Standard
5.2.6.

### J2.3 SPARK with Ada and the Ravenscar profile

SPARK is a subset of Ada with a proof obligation for the absence of run-time errors,
and the Ravenscar profile is a restricted tasking subset for high-integrity systems.
The precedent Keleusma draws is the idea of a restricted, statically analyzable
subset whose restriction is the enabling property rather than a limitation, which is
the Sealed profile of Standard 2.6. Ada's uniform per-construct rule categories,
being legality rules, static semantics, dynamic semantics, and the bounded-error and
erroneous categories, are the model for the uniform rule template of the
meta-specification, though Keleusma removes the erroneous-execution category entirely
under Standard 6.3. Ada representation clauses, which let a programmer pin the byte
layout of a type, are the precedent for the width-parameterized canonical layout of
Standard 5.2. It informs Standard 2.6, Standard 5.2, and Standard 6.3.

### J2.4 The Definition of Standard ML

The Definition of Standard ML is a fully formal definition of a language by
inference rules over judgments, with a clean separation of static semantics from
dynamic semantics and a companion Commentary. It is the model for defining the
semantic objects once, being the abstract-machine configuration of Standard 6.1, and
for the static-versus-dynamic split that runs through the Standard. Its separation of
a small core from a separately specified Basis Library is the precedent for keeping
the built-in set small in Standard 10.3 and leaving the library to the host. It
informs Standard 6.1 and Standard 10.3.

### J2.5 MISRA C

MISRA C is a set of guidelines that restrict C to a safer analyzable subset for
critical systems. It is the precedent for the conservative-verification stance, being
that a construct whose safety cannot be established is excluded rather than trusted.
Keleusma differs by enforcing the restriction in the verifier rather than by external
guideline checking. It informs Standard 2.3 and Standard 8.

### J2.6 The Ferrocene Rust specification

The Ferrocene specification is a precise description of the Rust language produced
for an existing implementation. Its relevance is twofold, since the Keleusma runtime
is written in Rust and depends on a describable Rust, and since it demonstrates
writing a precise language specification for an existing implementation rather than a
greenfield design. It informs the consolidation posture of the meta-specification
and, indirectly, Standard 1.3.

### J2.7 The synchronous languages Lustre with SCADE and Esterel

The synchronous languages describe systems as stream functions over discrete logical
time, compiled to bounded-memory bounded-time step functions, and are used in
avionics. They are the closest match to the Keleusma stream-processor model with its
per-iteration bounds. Keleusma adopts the stream-iteration framing of Standard 5.5
and the per-iteration worst-case bounds of Standard 8.3. It differs by exposing an
imperative stack machine rather than a dataflow surface. It informs Standard 5.5 and
Standard 8.3.

### J2.8 Forth

Forth is a stack-based language close to the machine, with a small, directly
dispatchable instruction repertoire. It is the precedent for the stack-machine
instruction set of Standard 7 and for a small dense opcode space. It informs Standard
7.

### J2.9 The LLVM language reference

The LLVM language reference specifies a typed intermediate representation and its
semantics. It is the reference point for the native code generation contract of
Standard 12, in which verified bytecode is the intermediate form lowered to a target.
It informs Standard 12.

### J2.10 The extended Berkeley Packet Filter

The extended Berkeley Packet Filter is an in-kernel bytecode with a static verifier
that proves termination and memory safety before a program is allowed to run, and its
instruction set is being standardized. It is a second precedent, beside the Java
Virtual Machine, for accepting only what a static verifier proves. It informs
Standard 8.

### J2.11 The BEAM virtual machine, Erlang, Elixir, and High Performance Erlang

The BEAM virtual machine, which is the shared runtime for Erlang and Elixir, schedules
reduction-counted processes and supports hot code loading, and High Performance Erlang
compiles the same bytecode to native code. That two surface languages target one
verified-loading virtual machine is itself a precedent for a single instruction set
beneath more than one surface, which bears on the self-hosting and native paths of
Standard 12. The reduction-counted scheduling is the precedent for the reset-bounded
stream iteration as a scheduling unit, and the hot code loading is the precedent for
the hot-swap contract of Standard 5.5 and Standard 12.4. It informs Standard 5.5 and
Standard 12.4.

### J2.12 C23

C23 is the current International Organization for Standardization C standard. It is
the reference for the strictly-portable-versus-conforming program distinction of J9,
and its treatment of implementation-defined behavior is the model for the
characteristics register of Annex B. It informs J9 and Annex B.

### J2.13 The hardware description languages

The hardware description languages, being Verilog with the Institute of Electrical
and Electronics Engineers standard 1364, VHDL with the standard 1076, and
SystemVerilog with the standard 1800, describe digital logic and are compiled either
to a simulation or, through a restricted synthesizable subset, directly to gates. Two
properties carry across. The synthesizable subset is a restricted subset of a larger
language whose restriction is exactly what makes it realizable in silicon, which is
the direct analogue of the Sealed profile of Standard 2.6 and of the fixed-width
instruction encoding of Standard 7.1, whose regularity is what lets the instruction
decode reduce to a table lookup or a decode read-only memory. The second property is a
cycle-deterministic semantics, in which behavior is defined per clock step, which
parallels the per-stream-iteration bounds of Standard 8.3. Keleusma is a stack machine
rather than a description of logic, so it does not adopt the dataflow surface, but it
adopts the discipline that the realizable subset is the governed one. It informs
Standard 2.6, Standard 7.1, and Standard 8.3.

### J2.14 RISC-V

The RISC-V instruction set specification defines a small base integer instruction set
with a set of optional standard extensions, each of which an implementation may or may
not provide, and a profile names the base and the extensions met. This
base-plus-extensions structure is the direct precedent for the Core-plus-profiles
conformance model of Standard 2.5, in which a minimal implementation satisfies Core and
named profiles add capability. RISC-V is also a modern example of an instruction set
specified cleanly enough to be realized directly from the document, which is the
posture of Standard 7. It informs Standard 2.5 and Standard 7.

### J2.15 The Common Language Infrastructure

The Common Language Infrastructure, being the ECMA-335 and International Organization
for Standardization standard behind the Common Intermediate Language and its runtime,
defines a stack-based bytecode, a bytecode verifier that establishes type and memory
safety before execution, and a metadata format that carries the type and member
tables. It is a peer to the Java Virtual Machine and a second precedent for a
standardized verified stack bytecode with a separately specified metadata section,
which parallels the opcode-record section and the auxiliary body of Standard 9.
Keleusma narrows the model by forbidding recursion and dynamic loading and by carrying
the worst-case bounds in the header. It informs Standard 7, Standard 8, and Standard 9.

### J2.16 Zig

Zig is a systems language whose design makes allocation explicit, so that a function
that allocates receives an allocator, and there is no hidden allocation and no hidden
control flow. This is the precedent for the arena model of Standard 5.5 and the
no-allocation-after-initialization discipline that makes the worst-case memory usage of
Standard 8.3 a static figure. Keleusma goes further by fixing the arena at construction
and reclaiming it at each reset. It informs Standard 5.5 and Standard 8.3.

### J2.17 The total functional lineage

The total functional lineage, being Dhall as a configuration language that is total by
design and the totality checkers of Idris and Agda, establishes that a language can
guarantee termination by construction rather than by convention. This is the precedent
for the totality discipline of Standard 4.6, in which every non-divergent function
terminates and the one divergent function is productive. The Definition of Standard ML
supplies the type discipline, and this lineage supplies the totality guarantee that is
the defining property of Keleusma. Keleusma differs by admitting one productively
divergent function, so that a stream processor runs without end while still producing
an output on every step. It informs Standard 4.6.

### J2.18 CompCert

CompCert is a C compiler whose translation from source to machine code is proven to
preserve semantics, so that a property established of the source holds of the generated
code. It is the precedent for the semantics-preservation requirement of the native code
generation contract of Standard 12.1, which does require the generated code to preserve
the observable semantics of the bytecode. Its translation-validation posture, in which a
verified checker over the output reduces the trust placed in the producer, is separately
the precedent for having a verifier check the emitted bytecode rather than proving the
compiler once and for all, which bears on the bootstrap-trust argument of J10. The
distinction to keep is that CompCert establishes semantic preservation, being that the
output computes what the source means, whereas the Keleusma bytecode verifier reduces
producer trust only for safety, structural validity, totality, and the bounds, and not
for semantic preservation or functional correctness, per Standard 8.5. It informs
Standard 12.1 and J10.

### J2.19 Fortran, cited narrowly

Fortran is cited only for its storage-association model and its interoperability with
C, as a reference point for the section model of Standard 5.3. It is a minor
influence.

### J2.20 Index of informed sections

| Standard | Sections informed |
|---|---|
| WebAssembly | 6, 7, 8 |
| Java Virtual Machine | 8.2, 5.2.6 |
| SPARK, Ada, Ravenscar | 2.6, 5.2, 6.3 |
| Definition of Standard ML | 6.1, 10.3 |
| MISRA C | 2.3, 8 |
| Ferrocene Rust | 1.3, consolidation |
| Lustre, SCADE, Esterel | 5.5, 8.3 |
| Forth | 7 |
| LLVM | 12 |
| extended Berkeley Packet Filter | 8 |
| BEAM, Erlang, Elixir, High Performance Erlang | 5.5, 12.4 |
| C23 | J9, Annex B |
| Hardware description languages | 2.6, 7.1, 8.3 |
| RISC-V | 2.5, 7 |
| Common Language Infrastructure | 7, 8, 9 |
| Zig | 5.5, 8.3 |
| Total functional lineage | 4.6 |
| CompCert | 12.1, J10 |
| Fortran | 5.3 |

---

## J3. Per-section rationale

This section gives the reason for each Standard section and cites the relevant J2
subsection by reference rather than restating the precedent, so a design decision
lives here and a language's treatment lives in J2.

- Standard 2, the three-verdict model, exists because acceptance must rest on proof
  and the analysis is not decidable in general. The decidable structural set is
  separated from the semidecidable bound analysis so that a stronger analysis can
  admit more programs and remain conforming. See J2.5 and J2.10.
- Standard 4.6, totality, exists because a definitive worst-case bound requires that
  every non-divergent function terminate and that the one divergent function be
  productive. Recursion is prohibited because an unbounded call depth defeats the
  frame-stack bound. See J2.7 and J2.17.
- Standard 5.2, the canonical flat layout, is the crux and is treated in J4.
- Standard 6, the abstract machine, is defined once so that every instruction rule is
  a transition over one state. See J2.1 and J2.4.
- Standard 7, the instruction set, is a stack machine with a fixed-width encoding for
  the reason in J7. See J2.1, J2.8, J2.13, J2.14, and J2.15.
- Standard 8, the verifier, is where the value proposition is delivered, and its
  scope boundary in Standard 8.5 is drawn honestly so that a reader does not mistake
  safety for correctness. See J2.2, J2.5, and J2.10.
- Standard 9, the wire format, carries the bounds in the header so that a loader
  admits a module against its own budget, and its cryptographic framing is profiled
  so that a minimal build carries no cryptography.
- Standard 11, the host contract and trust boundary, exists because the guarantees are
  conditional on the host, and stating the boundary is more honest than leaving it
  implicit.

---

## J4. The single-representation lesson

The single canonical representation of Standard 5.2.4 is the direct remedy for a
class of defects found during development, in which a value had more than one
physical layout depending on the runtime value, a size threshold, or the producing
agent. When a producer and a consumer can disagree on the bytes of a value, a
self-hosted compiler cannot emit bytecode that another instance will read correctly
without a runtime negotiation. The remedy is that layout is a pure function of the
type and the target widths, that a producer-supplied layout is trusted only when
verified equal to the computed layout, and that the empty-option case has one
representation resolved by the type. The flat-byte composite work that removed the
last owned-body representation and pinned the value slot to 32 bytes is the
implementation of this lesson. The defect history it removes includes an out-of-
bounds nested-composite offset, a mismatched flat text field offset, and an
attacker-choosable enumeration padding hint, each of which is closed by construction
once layout is type-determined and verified.

---

## J5. The conservative-verification stance and the three-verdict model

The verifier admits only what it can prove. The surface language accepts a broader
set of programs than the analysis can bound, and rejection of the unproven remainder
is the safety property, not a limitation. A program that Keleusma accepts is one whose
bound is proved, not one whose bound merely exists. This is why the three verdicts of
Standard 2.3 separate a decidable structural set, which every implementation decides
completely, from a semidecidable bound analysis, which may vary in strength and is the
sole occupant of the MAY-REJECT band.

The conformance suite of Standard 13 is the objective arbiter in place of an external
body, because issuance is dictatorial and there is no such body. The Standard is the
authority and the suite is verification evidence. The argument for test-based
arbitration in place of a fully mechanized semantics rests on two properties that make
exhaustive negative testing meaningful, being the totality of the language, so there is
an outcome for every input, and the closed fault set of Standard 6.3, so the space of
outcomes is enumerable. Because testing demonstrates behavior on the tested cases but
does not by itself establish verifier soundness, a mechanized soundness argument
connecting the verifier to the semantics of Standard 6 is a planned activity, not
merely future work.

---

## J6. Litmus tests

Two examples stress the guarantees, one line each.

- The sixty-five-oh-two native target stresses the width-parameterized layout of
  Standard 5.2, because an 8-bit word forces every byte size to be computed at the
  target widths rather than assumed at 64 bits.
- The aerospace control loop stresses the totality of Standard 4.6 and the worst-case
  bounds of Standard 8.3, because a control loop must produce an output on every
  iteration within a fixed time and a fixed memory budget.

The litmus examples and the native-target examples are kept generic and free of any
control, guidance, or targeting specifics, so both documents remain publishable
without export-control entanglement.

---

## J7. The dispatch note

The instruction encoding of Standard 7.1 is fixed-width, being a 4-byte opcode record
with a 7-bit identifier over a small dense opcode space. The first-order consequence,
stated once and neutrally, is that dispatch reduces to a single table lookup, realized
as a dispatch table in software or a decode read-only memory in hardware. This is why
the opcode count is a governed constraint of Annex C.

---

## J8. Governance

Issuance is dictatorial at present, per Standard 1.2. A more mature process may add
peer review, but the likely shape is peer-reviewed evolution with dictatorial
issuance, in which proposals are reviewed openly and the issuing authority retains the
act of issuance. The stability levels of the meta-specification, being Normative,
Provisional, At-risk, and Reserved, give the issuing authority a graded instrument
short of full revision. The information-flow-control labels of Standard 4.5 were
promoted from Provisional to Normative, and the big-number family of Standard 5.1.2 is
recorded as a future feature.

---

## J9. Portability and target-defined behavior

Keleusma defines an outcome for every admissible program and has no undefined
behavior and no unspecified category. Behavior is either portable, meaning identical
on every conforming target, or target-defined, meaning defined and documented for each
target and permitted to differ across targets. Target-defined behavior admits
architecture-specific optimization while preserving totality and the worst-case
bounds, and every target-defined point is recorded in Annex B. A program that relies
on no target-defined behavior is strictly portable and produces the same observable
result on every conforming target. A program that relies on target-defined behavior,
for example a specific word width, remains conforming but is not portable. This
distinction follows C, which separates a strictly conforming program from a conforming
one. It is worth stating that the excluded undefined behavior and the admitted
target-defined behavior are different things, since the first is a source of
unpredictability the language forbids and the second is a deliberate and documented
latitude.

---

## J10. The conforming self-hosted producer

The self-hosted compiler is a conforming Keleusma program, not a privileged host
application. It is a bounded per-input-unit stream processor after the manner of a
single-pass compiler, so its worst-case memory usage is a per-unit bound rather than a
whole-run bound, with the productive divergent loop of Standard 4.6 supplying unbounded
total work and the reset of Standard 5.5 reclaiming per-unit memory. The accumulating
symbol and type environment resides in the persistent or shared region of Standard 5.4
and never in the per-iteration arena, which is a general rule for every program.
Forward references are resolved by fixups, with no resident whole-program syntax tree.
The single-pass stream-compiler precedent is the reason this is expressible under
totality.

Because the compiler, its verifier, and its conformance suite share an origin,
self-hosting weakens the independence between production and verification and raises
the bootstrap-trust problem, in which an erroneous bootstrap compiler could produce a
self-hosted compiler that still passes its own verifier. An independently implemented
verifier, which is the trust anchor for the whole model, or a diverse
double-compilation, addresses this. The verifier that independently checks every module
the compiler emits is itself the independent checker that self-hosting otherwise lacks.

---

## J11. Cross-reference index

The internal rationale and decisions that ground this document live in the following
paths. The architecture narratives are in `docs/architecture/`, the resolved and
backlog decisions are in `docs/decisions/`, the consolidated specifications this
document elevates are in `docs/spec/`, and the blueprint that fixes the structure of
the Standard and this document is `docs/standard/META_SPECIFICATION.md`. The
conservative-verification stance is stated at
`docs/architecture/LANGUAGE_DESIGN.md`. The execution model, the arena, and the reset
semantics are narrated in `docs/architecture/EXECUTION_MODEL.md`, which lags the
current code at several source-location references noted in Annex A of the Standard.
