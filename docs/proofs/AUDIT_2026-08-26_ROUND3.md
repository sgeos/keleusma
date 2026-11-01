# Adversarial Audit, Round Three, Targeted, 2026-08-26

> **Navigation**: [Documentation Root](../README.md) | [Round one](./AUDIT_2026-08-24.md) | [Round two](./AUDIT_2026-08-24_ROUND2.md)

**Subject.** The third revision of `COMPOSITE_REGION_REUSE_PROOF.md` at commit `0f59ffed`.
**Method.** Three auditors on operator direction. One over the comparison method, the
definitions and axioms it rests on, and the clearance lemmas. One over every theorem and
corollary of Sections 5 through 8, with countermodel attempts against every hypothesis
combination each statement admits. One running a mechanical statement-box consistency lens over
all of Part I, five checks per result with a complete pass-or-finding table. The two
mathematics auditors did not open the prior audit records.

## Outcome in one paragraph

The round found two soundness holes, each repairable by a single stipulation, and a stratum of
consistency and formalization debt, while the material that previous rounds had hardened held
almost everywhere, both clearance lemmas' case analyses, Theorem B2's copy-chain induction,
Definitions 7 and 8, and the M8 and M9 axioms all withstanding direct attack, with roughly
forty failed attacks recorded across the three reports. Seven of fifteen results passed all
five statement-box checks. The two holes are qualitatively smaller than the prior rounds'
breaks, neither requiring a new hypothesis on any theorem's substance, but they are real, and
under the operator's pre-agreed decision rule they trigger the ceiling assessment recorded in
the session rather than a clean-gate declaration.

## The two soundness holes

**View-epoch laundering.** Nothing stated what epoch a derived view carries, and M2's wording,
the epoch current at its creation, permits a conforming machine to stamp a fresh view with the
current epoch even when derived from a stale source. The environment legally retains a region
handle across a boundary, replays it as a resume reply, the machine derives a view, no
staleness check firing since derivation reads the handle rather than the bytes, and the fresh
view dereferences successfully in both regimes with divergent bytes. This falsifies Theorem A2
from the axioms as written and threads into every composition wherever references reach the
environment. Repair, a view carries its source handle's epoch, stated in Definition 2 and M2,
with A2's cross-cycle sentence extended to post-boundary derivations.

**Startup-cycle allocations against Theorem C.** The occupancy bound counted only the unit's
allocations, and nothing confined allocation to the unit, so allocations outside it, the
startup interval before the first epoch advance included, defeat the literal statement. Repair,
one clause in Hypothesis H, every allocation of every cycle, the startup interval included,
occurs within the unit's traversal, shared by Corollary B2a.

## The consistency breaks and structural weaknesses

Theorem B1's statement box asserted an accounting conclusion its proof never establishes, the
accounting living in Theorem C under Hypothesis H which B1 does not carry, inherited by B1r.
Corollary B2a asserted its bound with no proof block, the only proofless result in Part I.
Definition 3's scope wording admitted two parses, frame-local against dynamic enclosure, with
the proofs using the second while the wording favored the first, and arm containment was
undefined, a callee-inclusive reading admitting countermodels against A2 and A3. Lemma 4's
plan-invariance sentence was false for the confinement lemmas as stated, the correct argument
being an ordered induction discharging links from the baseline outward that the proof did not
perform, its hypothesis import read as vacuous for multi-element plans under a hostile literal
reading, and its enumeration of what per-element arguments use omitted the copy-chain analysis
a designated element's link needs. Lemma 1's third clause was false in the generality claimed.
Lemma 2's callee-return case reached the right conclusion through a wrong justification, and
its scope-end analysis omitted unwinding. Lemma 5's cycle clause was proved for internal
survivors only. The environment's action space needed the symbolic reading written down. The
precision stratum included the prefix-comparison weakening, undefined entry height, the
scrutinee-evaluation convention, cap-counting wording, the footprint sum's pair ambiguity, two
Part II labels calling Corollary A3 a theorem, one wrong attribution, and the composition
results claimed for the instantiation without the without-designated-elements qualifier.

## What withstood attack

The comparison method's observation-inclusive design carried the bump-address-shift attack
correctly, byte equality of prior reads riding the no-prior-divergence assumption. The
clearance lemmas survived direct attack on nested loops, never-returning calls, multiple
activations, reply smuggling, shape-only restoration, partial overwrite, and copy concealment.
Theorem B2's copy-chain induction, Definition 7's boundary anchoring, Definition 8's excusal
semantics, and the A2 repair of round two all held. Every failed attack is enumerated in the
three reports, preserved in the session task outputs and summarized here because failed attacks
are evidence.

## Disposition

Every verified finding is repaired in the fourth revision recorded in the proof document's
provenance appendix, the two soundness holes each by its single stipulation, the consistency
breaks by aligning statement boxes with proofs, Lemma 4 by writing the ordered induction, and
the precision stratum item by item. The ceiling assessment under the operator's decision rule,
whether further prose rounds are warranted or mechanization is the terminal path, is put to the
operator in the session record with this round's evidence, two single-stipulation holes against
the prior rounds' four and five structural breaks.
