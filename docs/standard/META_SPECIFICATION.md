# Keleusma Standardization Meta-Specification

**Status:** Draft for review. This document is not itself normative and carries
no conformance weight.

**Purpose:** This document specifies how the two Keleusma standardization
documents are written, scoped, versioned, and related. It is a working
blueprint whose only job is to fix the shape of the program before any
normative prose begins.

**Siblings (forthcoming):**

- [`STANDARD.md`](STANDARD.md) — the Keleusma Standard (normative).
- [`JUSTIFICATION.md`](JUSTIFICATION.md) — the Keleusma Justification Notes
  (informative).

Both are expected to live beside this file in `docs/standard/`. Neither exists
yet. This meta-specification is authored first so the two documents can be
drafted against an agreed structure.

---

## 1. The two documents

### 1.1 The Keleusma Standard (normative)

The Standard states the contract and only the contract. It says what conforms
and what does not. It contains no motivation beyond the minimum a reader needs
to apply a rule.

### 1.2 The Keleusma Justification Notes (informative)

The Justification Notes state why each part of the Standard is the way it is,
cross-reference the reviewed external standards and the internal decisions
record, and carry the comparative analysis. They never define conformance and
are never cited to resolve a conformance question.

### 1.3 Direction of authority

The Standard is normative over the reference implementation, not the reverse.
When the implementation and the Standard disagree, the implementation is wrong
by definition until the Standard is revised through issuance. This direction is
what gives a self-hosted compiler a stable target. The Justification Notes are
never normative over anything.

---

## 2. Governing principles

- **P1 Consolidation, not greenfield.** The Standard is an elevation of the
  existing `docs/spec/` material into one governed document. The Justification
  Notes absorb the rationale now spread across `docs/architecture/` and
  `docs/decisions/`. Every clause elevated from the existing material is checked
  against P2, P7, and the recorded audit findings before it becomes normative,
  and the Standard states the intended semantics rather than transcribing
  current implementation behavior.
- **P2 Single canonical representation.** Every value has exactly one physical
  layout at a given set of target widths. No representation is
  context-dependent. This principle is the direct remedy for the
  dual-representation defects of the current development cycle and is binding on
  every producer, being the compiler, the host boundary, and native code
  generation. Representation and layout are a pure total function of the type
  and the target widths, never of a runtime value, a size threshold, or the
  producing agent, so the value-driven and size-threshold packing paths are
  non-conforming.
- **P3 Semantics, not algorithms.** The Standard specifies what a conforming
  implementation must observably do. It never specifies how, so no compiler
  pass or code-generation algorithm appears in it.
- **P4 Test-suite arbitration.** The Standard is the authority, and the
  conformance suite is the objective verification evidence for it, not a separate
  oracle, so a suite case that conflicts with the Standard is a suite defect and
  never blesses a non-conforming implementation. Issuance is dictatorial and
  there is no external conformance body, so the suite is where a conformance
  dispute is settled against the Standard. Negative cases carry the same weight
  as positive cases. Each MUST-ACCEPT and MUST-REJECT rule cites at least one
  conformance-suite case, and each case cites the rule it exercises. A rule with
  no case and a case with no rule are both defects tracked against issuance.
- **P5 Application neutrality.** The hardware target and its application are out
  of scope for both documents except the single aerospace-control-loop litmus
  reference. The neutral first-order note that fixed-width encoding reduces
  dispatch to a single table lookup may appear once in the Justification Notes
  and nowhere in the Standard.
- **P6 Defined behavior only, with target-defined latitude.** The Standard
  defines an outcome for every admissible program on every input. There is no
  undefined behavior and no erroneous-execution category, and every fault is
  drawn from a closed, enumerated, bounded set. Behavior is either portable,
  meaning identical on every conforming target, or target-defined, meaning
  defined and documented for each target and permitted to differ across targets.
  Target-defined behavior admits architecture-specific optimization while
  preserving totality and the worst-case bounds. A construct whose outcome could
  be unbounded or genuinely undefined is placed in MUST-REJECT rather than
  described as erroneous. The Standard uses no unspecified category, so every
  non-portable outcome is target-defined and recorded, never left as an
  undocumented set.
- **P7 Layout is type-determined and verified, not trusted.** Because layout is
  a total function of the type under P2, a layout, offset, or size value carried
  in a module or baked into an instruction is authoritative only when a
  conforming consumer has verified it equal to the layout computed from the
  type. A module whose carried or baked layout disagrees with the computed
  layout is MUST-REJECT. No offset or size is trusted because a producer
  asserted it. This makes closing the producer-supplied-layout trust hazard one
  principled invariant discharged by the typed pass of Standard 8.2, not an
  ad-hoc per-operand check that can be forgotten.
- **P8 The guarantees are conditional on the host contract.** The totality, the
  worst-case bounds, and the memory-safety guarantees hold for verified bytecode
  under the assumption that the host honors its contract. That contract is that
  every host-registered native honors its declared worst-case cost, its
  termination, and its memory behavior within the model, and that the host
  supplies memory as the model requires, being a correctly sized and non-aliased
  shared buffer and arena. A native is host-provided code the verifier does not
  analyze, so a native that exceeds its cost breaks the bound and a native that
  violates the memory model breaks safety. A violation of the host contract
  places the execution outside the guaranteed set. This trust boundary is stated
  normatively in Standard 11.

---

## 3. Document conventions

- **3.1 Requirement keywords.** MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are
  used in the sense of RFC 2119 as clarified by RFC 8174, and only in upper
  case where a requirement is stated.
- **3.2 Normative versus informative.** Every clause of the Standard is
  normative unless enclosed in an explicitly labelled Note or Example block,
  which is informative. The Justification Notes are informative in whole.
- **3.3 Versioning and change control.** The Standard carries a status and
  authority clause and a standard version identifier with a change record from
  the first issuance. Conformance is always conformance to a named version. The
  standard version is distinct from `BYTECODE_VERSION`, which it governs rather
  than equals. The first issuance records any known divergence of the reference
  implementation from the intended semantics as a known non-conformance in the
  change record, so the Standard is honest about the gap it is closing.
- **3.4 Cross-referencing.** The Standard does not cite the Justification
  Notes. The Justification Notes cite the Standard by section number, cite
  external standards by name and clause, and cite internal documents by path. A
  reader must be able to read the Standard alone. Every reference to a Standard
  section is prefixed with the word Standard, for example Standard 4.5, and a
  bare section number refers to this meta-specification, so the two numbering
  spaces never conflict.
- **3.5 Target parameters.** Everything layout-related or width-dependent is
  parameterized by the target word width, float width, and address width. No
  fixed byte figure appears without its width parameterization.
- **3.6 Notation.** The grammar uses one stated metasyntax. Instruction and
  value semantics use a small-step operational style in the manner of the
  WebAssembly specification. The layout algorithm is stated as total functions
  over the type and the target widths.
- **3.7 Uniform rule template.** Every surface construct and every instruction
  is specified under a fixed sequence of headings, in order, with a heading
  omitted only when empty. The headings are Syntax, then Static rules covering
  well-formedness, typing, and the verifier obligations whose violation places a
  program in MUST-REJECT, then Dynamic semantics stated as a step over the
  abstract machine of 3.8, then Faults giving the closed and bounded fault set
  the step may raise, each fault classified as eliminated at verify time and
  therefore MUST-REJECT or admissible at runtime with a justification under P6,
  then Implementation latitude naming any target-defined or
  analysis-strength freedom, always inside the MAY band and never the MUST band,
  then informative Notes and Examples.
- **3.8 Semantic objects and the abstract machine.** The Standard defines the
  abstract-machine configuration once, as a foundational section placed before
  the instruction semantics. The configuration names the operand stack, the
  frame stack, the arena and its epoch, the shared and private data regions, the
  program counter, and the coroutine and yield-resumption state. Every
  dynamic-semantics rule is a transition over this one configuration, and no
  later section introduces a new kind of state.
- **3.9 Stability levels.** Each Standard section or feature carries one
  stability level. Normative means frozen, changed only by issuance with a
  change-record entry. Provisional means normative for the named version but
  expected to change, so a producer may rely on it only within one version.
  At-risk means specified but subject to removal in the next issuance if no
  conforming implementation and no conformance-suite case exercises it. Reserved
  means named and allotted encoding or syntax space, not yet specified.
- **3.10 Target-defined behavior.** Where 3.5 parameterizes layout by target
  widths, this convention governs behavior that a target may define differently
  to admit architecture-specific optimization. Every target-defined point in the
  body also appears in the implementation-defined and target-defined
  characteristics annex, is defined for each conforming target, remains total,
  and remains within the worst-case bounds. Target-defined behavior is never a
  license for undefined behavior, and the Standard never leaves such a point
  undocumented.
- **3.11 Defined terms and index.** The Standard maintains a glossary of defined
  terms and an index recording where each term is defined, so that every
  normative term has one authoritative definition.
- **3.12 Rationale routing.** The sketch and outline sections of this
  meta-specification interleave rule and rationale for readability. When the
  Standard is drafted, every explanatory clause is lifted into the mirrored
  Justification section per J3, and the Standard clause is left rule-only per 1.1
  and 3.2. In particular the point-in-time-band explanation of 4.2, the
  better-analysis note of 4.3, the self-hosted-producer narrative and the
  independence and bootstrap-trust note of 4.6, the
  fix-this-cycle note of 6.4, and the zero-copy and provability notes of 6.2 and
  6.8 are Justification, not Standard prose.

---

## 4. Conformance model

*Sketch. This becomes a normative section of the Standard.*

**4.1** The Standard names four conformance classes. A conforming module is a
wire artifact that satisfies the wire format and the verifier. A conforming
producer, being the compiler, the host boundary, or a native code generator,
emits only conforming modules and materializes only the canonical layouts
required by P2. A conforming program is well-formed under the grammar and type
system and admissible under the verifier. A conforming implementation obeys the
accept and reject obligations of 4.2.

**4.2** A conforming implementation partitions every well-formed, well-typed
program into exactly one of three verdicts and behaves as follows.

- **MUST-ACCEPT.** An implementation-independent class, being the floor fixed by
  the positive conformance suite under P4. Its members have no structural or
  memory-safety violation and a provable worst-case execution time and worst-case
  memory usage bound, and every conforming implementation accepts every one of
  them. Fixing the floor by the positive suite rather than by a mandated analysis
  keeps P3. The existence of a bound is a portable property, and the numeric
  bound value is a target-defined characteristic recorded in Annex B.
- **MUST-REJECT.** A program with a proven structural or memory-safety violation
  or a proven absence of an upper bound. The set of provable structural and
  memory-safety violations is decidable, and every conforming implementation
  decides it completely, so such a program is never in the MAY-REJECT band. Every
  conforming implementation rejects it.
- **MAY-REJECT.** A program that the implementation has neither proven admissible
  nor proven violating, the reject-when-unproven band. Rejecting here is always
  safe, because the only route to acceptance is upgrading to a proof, so no
  unproven and unsafe program is ever admitted. Rejection here is
  implementation-defined, and a stronger analysis that proves the program
  admissible and accepts it is still conforming. This is the point-in-time band,
  and it is the reason two conforming implementations, including a self-hosted
  one, may draw the line differently and both conform.

MUST-ACCEPT and MUST-REJECT are implementation-independent classes, and
MAY-REJECT is the only implementation-relative band. A given implementation's
accept set is a superset of MUST-ACCEPT, extended into MAY-REJECT by its own
analysis strength, but the three classes are fixed independently of any
implementation.

**4.3** The conformance suite has positive cases that every implementation MUST
accept and run to the stated result, and negative cases that every
implementation MUST reject. Negative cases assert only the MUST-REJECT band,
never the MAY-REJECT band, so that the suite never forbids a better analysis.

**4.4** Conformance is claimed against one or more profiles over a common core.
An implementation conforms to the Core profile by satisfying every Standard
section except the native code generation contract, the cryptographic framing,
and the float scalar, so a Core implementation carries no cryptography and no
floating point and is deterministic by default. Four additive profiles extend
Core. The Native profile adds the native code generation contract. The Signing
profile adds signature verification of a signed module, being the runtime
obligation, with signature generation a producer capability under the same
profile. The Encryption profile adds decryption of an encrypted module and its
counterpart production. The Float profile adds the float scalar with the
exception, rounding, and NaN semantics of Standard 5.1. The Signing and
Encryption profiles are independent, so either may be claimed without the other,
though implementations commonly provide both. A conformance claim names the profiles met, and the conformance suite is
partitioned so that a Core-only implementation is exercised only by Core cases. A
minimal interpreter on a trusted target may claim Core alone, and integrity or
confidentiality is then required by loader policy per deployment rather than by
Core.

**4.5** A program is strictly portable when it relies on no target-defined
behavior, so its observable result is identical on every conforming target. A
conforming program may rely on target-defined behavior, so its result is defined
on each target yet may differ across targets. Reliance on target-defined behavior
does not make a program non-conforming. It makes the program non-portable, and
the points of reliance are recorded in the implementation-defined and
target-defined characteristics annex.

**4.6** The self-hosted compiler is itself a conforming Keleusma program, not a
privileged host application. It is a bounded per-input-unit stream processor
after the manner of a single-pass compiler, so its proven worst-case memory
usage is a per-unit bound rather than a whole-run bound. Unbounded total work
comes from the productive divergent loop, and the arena RESET reclaims per-unit
memory. The accumulating symbol and type environment does not live in the
per-iteration arena. It resides in the persistent or shared region, whose growth
is host-mediated and outside the proven per-iteration bound. Forward
references are resolved by fixups, with no resident whole-program syntax tree.
This is encoded in Standard 4.6, Standard 5.4, and Standard 5.5, and the
Justification records the single-pass precedent in J10. The only normative rule
here is general and not compiler-specific, being that accumulating state resides
in the persistent or shared region and never in the per-iteration arena, which
Standard 5.4 states for every program. The compiler framing is Justification.
Because the compiler, its verifier, and its conformance suite share an origin,
self-hosting weakens the independence between production and verification and
raises the bootstrap-trust problem, which an independently implemented checker or
a diverse double-compilation addresses.

**4.7** The Sealed profile is a restriction over Core rather than a capability
over it. A Sealed implementation resolves every MAY-REJECT program to a single
documented deterministic decision, so its accept and reject sets carry no
implementation latitude. It admits no Provisional, At-risk, or Reserved feature
in the accepted surface. No module executes without verification, and no admitted
path reaches execution without it. It requires the verifier. Because Core is
float-free, a Sealed build is float-free by construction, and Sealed does not
compose with the Float profile, whose exception and rounding behavior is
target-sensitive, while the Q-format fixed-point and big-number families remain,
since they are in Core and fully determined at the target widths. The Sealed
profile is how a deterministic frozen build is described, and it composes with
the other capability profiles, so an implementation may be Sealed and
additionally Native, Signing, or Encryption.

**4.8** Structural coverage of an implementation and requirement-to-code
traceability within it are the implementation's own concern, outside the
Standard, which specifies observable behavior and the requirement-to-test
traceability of P4.

---

## 5. The Keleusma Standard — table of contents

Each entry names the existing material it consolidates.

1. Status, Authority, and Version
2. Scope and Conformance — from section 4 above
3. Terms, Notation, Normative References, and Target Parameters — from
   `target.rs`, `docs/spec/`
   1. Defined terms and the index of definitions
   2. Notation and metasyntax
   3. Normative references. The always-core references are RFC 2119 and RFC 8174,
      and Unicode with UTF-8. The Float profile adds IEEE 754. The Signing profile
      adds RFC 8032 for Ed25519. The Encryption profile adds RFC 7748 for X25519
      key agreement,
      RFC 5869 with FIPS 180-4 for HKDF-SHA-256 key derivation, and FIPS 197 with
      NIST SP 800-38D for AES-256-GCM authenticated encryption
   4. Target parameters, being word width, float width, and address width
4. Surface Language
   1. Lexical structure — `docs/spec/GRAMMAR.md`
   2. Grammar
   3. Type system, being inference, generics, and bounds —
      `docs/spec/TYPE_SYSTEM.md`
   4. Monomorphization
   5. Information-flow-control labels
   6. Totality and the productive divergent loop, and the conforming self-hosted
      producer per meta-specification 4.6
5. Value and Memory Model — the crux; outline in meta-specification section 6
   below
   1. Value domain, with the closed and exhaustive scalar-kind set. Core is
      float-free, and the Float profile adds the float scalar, whose subset,
      rounding mode, and NaN, infinity, and exception treatment are fully
      specified for defined behavior under P6. The fixed-point and big-number
      families are in Core
   2. Canonical flat layout
   3. Section model, being text, rodata, data, and bss
   4. Data segment, being shared and private, and the persistent home of the
      self-hosted compiler's symbol and type environment
   5. Arena model and RESET semantics, stating exactly which state RESET reclaims
      and which persists — the runtime contract
   6. Module replacement and hot code swap — a normative runtime transition,
      with hot swap under native code generation a Native-profile design point
6. Semantic Objects and the Abstract Machine
   1. The machine configuration, being the operand stack, the frame stack, the
      arena and its epoch, the shared and private data regions, the program
      counter, and the coroutine and yield-resumption state
   2. Judgment forms and the small-step transition relation
   3. The closed fault enumeration, the single authoritative list of every
      bounded fault, referenced by each rule's Faults heading per 3.7 and 3.11
7. Bytecode Instruction Set Architecture — `docs/spec/INSTRUCTION_SET.md`,
   `docs/spec/STRUCTURAL_ISA.md`
   1. Encoding, being fixed-width records and the operand pool
   2. Instruction set and operational semantics
   3. Structural constraints
   4. Reserved opcodes and undefined encodings, which a conforming verifier
      MUST-REJECT
8. The Verifier
   1. Structural and memory-safety invariants, decided completely, including
      operand-stack discipline with loop-body neutrality and branch-join
      convergence and operand-index bounds
   2. The typed operand-stack pass, a bytecode-level type-preservation abstract
      interpretation after the manner of the Java Virtual Machine and
      WebAssembly verifiers, validating every baked offset against the canonical
      layout of the accessed type per P7
   3. Worst-case execution time and worst-case memory usage analysis, yielding a
      conservative upper bound whose existence is portable and whose value is
      target-defined, covering the operand stack and the frame stack as well as
      the arena and memory
   4. The soundness obligation, being that verifier acceptance implies the
      semantic safety property holds, stated as a normative property with a
      mechanized argument as a planned activity
   5. The scope of the guarantee, being safety, structural validity, totality,
      and the worst-case bounds, and explicitly not functional correctness, being
      whether the program computes its intended result, which the program author
      establishes by their own testing
   6. The accept-reject decision — ties to Standard 2
9. Wire Format — `docs/spec/WIRE_FORMAT.md`
   1. Module framing and sections
   2. Tables, being constants, type layout, and data layout
   3. Cryptographic framing and loader policy — the Signing and Encryption
      profiles, not part of Core
10. Stable Application Binary Interface
    1. Host and native-function marshalling boundary, whose faults belong to the
       closed set of Standard 6.3 under P6, not a panic path
    2. Native calling convention and memory model
    3. Built-in functions and the standard-library boundary —
       `docs/spec/STANDARD_LIBRARY.md`
11. The Host Contract and the Trust Boundary
    1. Native obligations, being declared worst-case cost, termination, and
       memory behavior within the model of Standard 10.2
    2. Host runtime obligations, being a correctly sized and non-aliased shared
       buffer and arena supplied across the call and resume boundary
    3. The conditional guarantee, being that a violation of the host contract
       places execution outside the guaranteed set
12. Native Code Generation Contract
    1. Semantics-preservation requirement
    2. Target memory model, being the bss arena sized by worst-case memory usage
    3. Section realization per target class
    4. Module replacement under native code generation — a Native-profile design
       point per open item O7
13. Conformance Suite requirements
- Annex A. Change record
- Annex B. Implementation-defined and target-defined characteristics
- Annex C. Encoding registry, being the assignment and reservation of opcode
  identifiers and wire section tags, whose reserved and undefined encodings are
  rejected per Standard 7.4

---

## 6. The flat-layout section (Standard 5.2) — outline

The self-hosting risk. Written first.

- **6.1** Layout is a total function of a type and the target widths. It yields
  a byte size and, for a composite, a field offset for each field.
- **6.2 Scalars.** Byte size for each scalar kind of Standard 5.1 at the target
  widths, which owns the enumeration so this clause does not restate it, and
  little-endian byte order for every stored scalar. The big-number family is a
  parameterized fixed-width multi-word integer, distinct from the closed base
  scalar set in the way an array is distinct from its element, so its layout is a
  word count of little-endian signed two's-complement words. The interval is not
  a value kind. It is a verifier-internal device used to discharge bound proofs
  and, by P3, is outside the normative surface.
- **6.3 Composites.** Tuples and structs pack fields at ascending offsets with
  the stated padding rule. Arrays multiply the element layout by the count.
- **6.4 Enumerations.** One discriminant word at offset zero, then the payload,
  padded to the largest variant, giving every value of the type one fixed size.
  The discriminant word width and signedness are stated at the target widths. The
  discriminant of a variant and the padded size are properties of the type,
  derived from the type definition and computed by the consumer, never supplied
  or trusted from a value producer or a wire enum-layout table, per P7. This
  clause is the normative form of the fix made this cycle.
- **6.5 The single-representation rule.** A value of a given type has exactly
  one flat body at a given set of widths. In particular the empty-option case
  has one representation, resolved here, not two. Every producer materializes
  the same bytes, so a value written by the compiler, by the host boundary, or
  by native code is byte-identical.
- **6.6 Reference-bearing and non-flat types.** The rule for when a type is flat
  and when it remains a reference form, stated as a total predicate over the
  type and widths, so producer and consumer always agree. Representation is a
  pure function of the type and widths, so no value-driven or size-threshold
  selection is admitted.
- **6.7 Layout arithmetic.** Saturating and overflow behavior, so a pathological
  type is rejected rather than mis-sized.
- **6.8 Baked offsets and verification.** Access instructions carry offsets baked
  at compile time from the accessed type. A conforming verifier reconstructs the
  operand-stack types and confirms every baked offset equals the canonical layout
  of the accessed type, per P7 and the typed pass of Standard 8.2. Runtime access
  then uses the baked offset directly with no per-access bounds check, because it
  was proven at load, which preserves the zero-copy goal.

---

## 7. The Keleusma Justification Notes — table of contents

- **J1** Purpose and relationship to the Standard, and the provenance of the
  two-document structure and the document conventions in the Ada Reference
  Manual, the Definition of Standard ML together with its Commentary, and the
  World Wide Web Consortium Process.
- **J2** Comparative standards review, organized as one subsection per reviewed
  standard. Each subsection is the authoritative place for that standard, stating
  briefly what it is, the specific property Keleusma adopts or deliberately
  rejects, and a forward index to the Standard sections it informs. Depth is
  proportional to influence, so a major precedent has a full subsection and a
  minor one has a sentence. Coverage is WebAssembly, the Java Virtual Machine,
  SPARK with Ada and the Ravenscar profile, the Definition of Standard ML, MISRA
  C, the Ferrocene Rust specification, the synchronous languages Lustre with
  SCADE and Esterel, Forth, the LLVM language reference, extended Berkeley Packet
  Filter and its instruction-set work, the Erlang virtual machine with High
  Performance Erlang, and C23. Ada representation clauses are cited under the
  flat layout. Fortran is cited only for storage association and interoperability
  with C. A closing index tabulates each standard against the Standard sections
  it informs, so the reader may enter by language or by section.
- **J3** Per-section rationale, mirroring Standard sections 3 through 13. It
  gives the reason for each section and cites the relevant J2 subsection by
  reference rather than restating the precedent, so a design decision lives in J3
  and a language's treatment lives in J2, per the one-authoritative-definition
  rule of 3.11.
- **J4** The single-representation lesson and the defect history it removes.
- **J5** The conservative-verification stance and the three-verdict model, and
  the argument for test-suite arbitration in place of mechanized semantics,
  resting on totality and the closed fault set that make exhaustive negative
  testing meaningful. Because testing demonstrates behavior on the tested cases
  but does not by itself establish verifier soundness, a mechanized soundness
  argument connecting the verifier to the semantics is a planned activity, not
  merely future work.
- **J6** Litmus tests, one line each tying the example to the guarantee it
  stresses, being the sixty-five-oh-two native target for width-parameterized
  layout and the aerospace control loop for totality and the worst-case bounds.
  The litmus and native-target examples are kept generic and free of any control,
  guidance, or targeting specifics, so both documents remain publishable without
  export-control entanglement.
- **J7** The neutral first-order note that fixed-width encoding reduces dispatch
  to a single table lookup, stated once.
- **J8** Governance, and the evolution toward peer-reviewed evolution with
  dictatorial issuance.
- **J9** Portability and target-defined behavior. Why undefined behavior is
  excluded while target-defined behavior is admitted for architecture-specific
  optimization, and the strictly-portable versus conforming program distinction
  after the manner of C.
- **J10** The conforming self-hosted producer and the single-pass stream-compiler
  precedent, and why the compiler is a conforming Keleusma program rather than a
  privileged host application.
- **J11** Cross-reference index into `docs/architecture/` and `docs/decisions/`.

---

## 8. Authoring order

Order chosen so the highest-risk and most-shared material is fixed first.

1. Standard sections 1, 2, and 3, being status, the conformance model, and
   terms with normative references.
2. Standard section 6, the semantic objects and abstract machine, since every
   instruction rule depends on it.
3. Standard section 5.2, the canonical flat layout, per the outline in
   meta-specification section 6, with its Justification counterpart in J4.
4. Standard section 5, remainder, being the section model, the data segment, and
   the arena and RESET semantics.
5. Standard section 7, the instruction set, with the neutral dispatch note
   placed in J7.
6. Standard section 4, the surface language, and sections 8 through 12, being
   the verifier specified as the typed operand-stack pass of P7, the wire format,
   the application binary interface, the host contract and trust boundary, and
   the native code generation contract.
7. Standard section 13 and Annexes A through C, then the remaining Justification
   sections.
8. The conformance suite, positive and negative, traced against Standard 2 per
   P4, seeding the negative corpus from the recorded audit findings so the suite
   exercises the exact hostile-input classes the model closes by construction.

---

## 9. Open items and their current resolution

Items O1 through O8 are resolved as stated, per the standardization review and
the issuing authority's rulings, and each remains revisable by the issuing
authority before the documents proper are written. O6 corrected a factual error
in the review and now fully specifies the big-number family, and O7 carries a
native-code-generation follow-on.

- **O1 Runtime contract placement.** Resolved by placing the arena and RESET
  runtime contract inside the Value and Memory Model at Standard 5.5 rather than
  in a standalone Runtime chapter, because it is inseparable from the flat
  layout, and because the abstract-machine configuration of Standard 6 sits
  beside the same memory model. Alternative, lift it into its own chapter.
- **O2 Conformance suite status.** Resolved as normative at Standard 13. The
  requirement-to-test traceability of P4 is enforceable only when the suite is
  governed, so the suite is a normative section rather than a separate
  deliverable.
- **O3 Information-flow-control labels.** Resolved. The labels are a first-class
  Normative Standard section at Standard 4.5, promoted from the earlier
  Provisional status, so a Sealed build per 4.7 may rely on them.
- **O4 Standard-library placement.** Resolved by specifying the small built-in
  function set and the marshalling boundary inside the Stable Application Binary
  Interface at Standard 10.3, after the manner of the separation of the Standard
  ML core from its Basis Library. Any larger library is host-supplied and out of
  the Standard. Alternative, a standalone standard-library section.
- **O5 Coroutines and yield in the core.** Resolved. Coroutines and yield are in
  the core and are first-class abstract-machine state in Standard 6. Until the
  host is itself self-hosted, there is a host program that the running module must
  be able to yield to, so yield is not optional.
- **O6 Value-domain scalar set.** Resolved, with a correction to the review. The
  implementation's scalar-kind set is closed and is unit, boolean, byte,
  word-width signed integer, Q-format fixed-point, feature-gated float, the text
  handle, and the opaque host handle. The review's premise was inaccurate. There
  is no built-in big-number type in the implementation. What exists is the
  building material, being the V0.2 checked-arithmetic construct that exposes the
  high half and the carry or borrow of a widened result, and the worked pattern
  that represents a multi-word value as a little-endian `[Word; N]` array, shown
  by `examples/scripts/09_big_numbers.kel` and the `Multbyte<N>` example
  `examples/scripts/10_multbyte.kel`. The `tests/big_number_arithmetic.rs` suite
  exercises that word-width checked arithmetic and the array pattern, not a
  big-number value kind, which agrees with the closed scalar set. The interval is
  not a value type but a verifier-internal abstract-interpretation lattice in
  `src/interval.rs`, outside the normative surface by P3. The issuing authority
  has specified a first-class big-number family, a parameterized fixed-width
  multi-word integer whose word count is part of the type, distinct counts being
  distinct types related by explicit cast. Its semantics are fixed by consistency
  with the word-width integer, being signed two's complement, wrapping at the
  word-count width and therefore defined and bounded under P6, division by zero
  the same bounded fault, widening casts sign-extending and narrowing casts
  wrapping to the low words, casts explicit only, and the word count a positive
  compile-time constant whose maximum is bounded by the encoding and recorded in
  Annex C. This family is not yet implemented and requires new surface, being a
  parametric type family that the note in `10_multbyte.kel` records the language
  does not yet expose, so it is greenfield beyond P1 and is a known
  non-conformance recorded per 3.3 until the runtime provides it.
- **O7 Hot code swap.** Resolved. Module replacement is a normative runtime
  transition specified over the abstract-machine configuration, with its
  transactional invariant and faults, since the feature carries a memory-safety
  obligation the first audit found violated. Hot swap under native code
  generation needs separate design and is a Native-profile design point recorded
  against Standard 12.4.
- **O8 Signing and encryption profiles.** Resolved, revised to Reading A. Core is
  cryptography-free. Signing and encryption are two independent additive profiles
  over Core per 4.4, matching the implementation's Ed25519 signing and its
  X25519-AES256GCM encryption in `src/encryption.rs`. The Signing profile carries
  RFC 8032 for Ed25519, and its runtime obligation is signature verification,
  with signature generation a producer capability. The Encryption profile carries
  RFC 7748 for X25519 key agreement, RFC 5869 with FIPS 180-4 for HKDF-SHA-256 key
  derivation, and FIPS 197 with NIST SP 800-38D for AES-256-GCM authenticated
  encryption. Each profile's references sit with the profile in Standard 3.3, and
  a Core implementation on a trusted target needs neither. The two are commonly
  implemented together in practice, but conformance treats them separately.
