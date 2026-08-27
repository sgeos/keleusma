# Adversarial Audit, Round Two, 2026-08-24

> **Navigation**: [Documentation Root](../README.md) | [Round one](./AUDIT_2026-08-24.md)

**Subject.** The post-audit revision of `COMPOSITE_REGION_REUSE_PROOF.md` at commit `15532455`.
**Method.** Five fresh contexts on the round-one pattern, with one change. The four mathematics
auditors were instructed not to open the round-one record, so their attacks were fresh rather
than anchored, while the fit auditor read it precisely to check the repair claims against the
text. One auditor was interrupted by a session limit and resumed.

## Outcome in one paragraph

The round-one repairs held where they were attacked, the new axioms M8 and M9, the operand
notion, the provenance-based reply closure, and the plan-invariance insight of the composition
lemma all withstood refutation, and the fifth auditor verified every reachable pin, commit, and
figure, several to the letter, including confirming the round-one record's claims against the
pre-audit text. What did not hold was the newly written material and two definitional roots
beneath everything. Round two therefore found the layer below round one, which is what a second
round is for, and it also found that one round-one repair was itself defective, Theorem A2's
statement box still carrying the hypothesis its own proof refutes.

## The two roots

**Root one, frame-local scoping.** Definition 3 scoped an execution to the innermost loop of its
frame, so an instruction in a loop-free callee was scoped to the whole cycle, and two auditors
independently built countermodels through it, a callee writing a confined reference into a
pre-loop caller stack entry legitimately, and consecutive executions of a loop-free callee's
site sharing one scope with no separating boundary. The repairs are a frame-locality clause, an
execution touches only its own frame's operand stack, and the separation hypothesis stated on
every reuse theorem rather than only on Theorem B2.

**Root two, statement-box discipline.** Theorem A2 carried three mutually inconsistent
hypothesis formulations, the boxed one refuted by its own proof, the mid-proof repair never
lifted into the statement, and the trailing restatement re-admitting the original countermodel.
Corollary A3 quantified over shapes its proof never treated, two same-arm sites sharing a slot
refute it outright, and Corollary B2b omitted the separation hypothesis Theorem B2 itself
carries. The lesson generalizes, a repair applied inside a proof body while the statement is
left unchanged is not a repair.

## Further verified findings

Corollary A1s has no case for calls in a setting that has calls, and the bridge between unit
structure and cycles is unstated. The comparison remark after Theorem C is refuted by a
counterexample, extracting a reused site's slot outside a conditional's arm maximum can exceed
it, so a plan's footprint can exceed the all-bump bound on branch-dominated shapes, and the
honest guidance is to compare the two valid bounds per plan. Lemma 4's per-link equivalence was
asserted as verbatim where it needs a stated first-divergence method, machine determinism, and
address-erased receipt events in the environment's observation history. Axiom text drifted from
proof text in three places, M5 lacking the write-before-read exemption Theorem B1r reads into
it, M6(b) saying early exits where the lemmas need every exit, and M6(d) counting frame
destruction as touching. Epoch freshness, that epoch values never repeat, was assumed by every
cross-cycle argument and stated nowhere. Definition 9 left the copy of a stale source undefined
and its region wording ambiguous about slot residency. Lemma 5's statement quantified over
cycle scopes its inherited proof cannot reach and understated its own conclusion. The precision
stratum also caught the completion assumption making a crash-inducing plan vacuously sound
absent prefix comparison, Lemma 1's largely definitional character, undefined view derivation
and transport primitives, and reachability used without definition.

## Instantiation findings

The scoping paragraph claimed Theorem B2 holds in the instantiation while Appendix C correctly
says the discipline machine does not exist, the M8 row's Discharged outran its checkable
artifacts with the ordering-comparison family unaddressed, the M3 row said both CopiesOut rows
executed where the table has three and `SetDataIndexed` is unexecuted, the M6(b) measurement is
partially circular because the iteration discriminator excludes by construction the violating
shape it would need to see, the backend arm-overlap license in Appendix D outran the
virtual-machine-only discharge, enforcement claims lacked the not-in-this-tree qualifier the M1
row carries, and one retraction citation pointed at the wrong artifact. Three resulting
questions went to the V0.2.X line, ordering comparisons under M8, a `SetDataIndexed`
discriminator, and whether iterating lowerings can emit a value-carrying self-break.

## Record correction to round one

The round-one record's Family 3 names per-site execution hypotheses as Theorem A2's repair and
its disposition says every verified finding is repaired. Round two found the A2 repair
defective in statement form, so that certification was inaccurate for A2, and the round-one
record now carries a pointer to this file rather than a silent edit.

## Disposition

Every verified finding is repaired in the second revision recorded in the proof document's
provenance appendix, with the three instantiation questions pending on the V0.2.X line. The
diminishing-returns question, whether a third round is warranted and at what scope, is put to
the operator in the session record rather than decided here.
