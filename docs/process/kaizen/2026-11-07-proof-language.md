# The Proof Language for the Proofs Line, 2026-11-07

> **Navigation**: [Kaizen](./README.md) | [Process](../README.md)

The operator asked on 2026-11-07 for the proofs line to be spun up, and directed that its
proof language follow the project's canonical use case and not any outside party's
preferences, that the absence of a suitable language be flagged, and that the conventional
option of United States certification practice was the likely choice. The operator then
ruled first-hand in this line's session, "Recommendation accepted. Rocq, plus Kani on the
Rust verifier now." This record gives the requirement, the evidence and the decision. The
operator starts the proofs session.

## The requirement

The canonical use case is in the family the project calls the hierarchical control
scenarios. Its certification plan pursues industry-aligned development assurance, with the
**verifier as the trust anchor**. The verifier is to be a qualified verification tool under
industry-aligned tool qualification, and a soundness argument relating the verifier to the
formal semantics is to support industry-aligned formal-methods credit. The proof that
matters most is therefore that **an accepted module respects its stated worst-case time and
memory bounds under the semantics**, together with the link from that model to the Rust
verifier.

**Fact.** The proof line's one result so far, `docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md`,
is a prose proof checked by three adversarial audit rounds. Its own standing caveat is that
the correspondence argument is prose rather than a mechanised bisimulation. A certification
argument needs that gap closed by a machine-checked proof.

## The evidence

From a sourced research brief of 2026-11-07. The fuller report, with its sources, is held
privately.

- **Fact.** Formal-methods case studies published by a government research laboratory for
  this industry practice treated one control-system example by theorem proving in PVS,
  partly repeated in HOL4, by model checking, and by abstract interpretation. They note that
  a theorem prover must be qualified as a verification tool unless each proof is checked
  independently, and that PVS does not normally emit a proof that a separate checker can
  verify.
- **Fact.** The same laboratory verified algorithms in PVS that underpin an industry
  standard. That is evidence used in writing a standard, not credit in a certified product.
- **Fact.** CompCert, a compiler verified in Rocq, formerly Coq, was reported in 2026 as
  qualified for a certified industrial computer, with industry-aligned tool qualification and
  formal-methods credit claimed. It is the only credit claim of this kind for a
  proof-assistant-based tool that the brief found, and its authority was not a United States
  one.
- **Fact.** A small independent proof checker was qualified in a simulated qualification
  exercise in place of the tool that produced the proofs. Qualifying a small checker instead
  of the whole prover is a recognised route, but it has been simulated, not certified.
- **Fact.** Rocq rechecks proof terms with a small kernel, and its standalone checker
  re-verifies compiled proofs independently. LCF-style systems such as HOL4 and Isabelle are
  equally checkable by design.
- **Fact.** The tools that relate a proof to Rust code, Aeneas to Lean, hax to Rocq, Lean and
  F*, and coq-of-rust, Verus, Kani and Creusot, have no certification precedent that the
  brief found.

## The flag

**No proof language has a precedent for this exact claim.** No mechanised soundness proof
of a bytecode verifier against an operational semantics has earned certification credit in
United States practice, within the brief's search. A suitable language exists, so this is
not the case of having none. What is missing is precedent, which is a certification risk
the plan carries openly.

## The decision

**Rocq** for the soundness proof, with its standalone checker as the independently checked
artefact, linked to the Rust verifier by differential testing, and **Kani** on the Rust
verifier's core now. Ruled by the operator on 2026-11-07.

- **Why Rocq rather than PVS.** PVS is the conventional prover of the research that informs
  this practice. The route to credit, however, is tool qualification, which is far easier
  when a small checker can re-verify every proof, and Rocq supports that where PVS does not.
  Rocq is also the only proof assistant behind a credit claim of this kind.
- **Not Lean or Agda as the primary base.** Neither has certification precedent. Lean's route
  to the Rust code through Aeneas could be added later.
- **The link to the code.** An executable model of the semantics and the bound checker in
  Rocq, soundness proved about the model, and the Rust verifier run against the extracted
  model on generated modules. That is the pattern this project already uses between the Rust
  and self-hosted compilers, one level up. A full proof of the Rust code is far costlier and
  has no precedent.
- **Kani now.** A bounded model checker for Rust, applied to the verifier's core for the
  absence of panics and overflow. It has no certification precedent either, but it is cheap
  and immediately useful evidence.

## The first increment for the proofs line

A Rocq model of a small core, with one function of each of the three kinds, the bound
checker for that subset, and a machine-checked theorem that an accepted module of the subset
stays within its time and memory bounds. The proof is checked by the standalone checker in
continuous integration, with the trusted base stated, which is the kernel, the semantics as
written and the model's correspondence to the code. The existing prose proof's
correspondence argument is a natural second increment. Kani harnesses on the Rust
verifier's core can proceed in parallel, on whichever line owns that code.
