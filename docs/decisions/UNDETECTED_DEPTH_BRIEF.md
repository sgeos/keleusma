# BRIEF — is it the site, or the subject?

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The state

Twelve subjects execute, agree, and did not notice any of **three** arithmetic mutations. Eight are
self-hosted stages, seeded with real input — the modules the V0.3.0 goal depends on most.

**Three sites is a thin basis for that claim.** Two explanations fit equally well:

| explanation | what it would mean |
|---|---|
| **the site** — the sampled ops are not on a path the seeds execute | a seeding/subject gap; add subjects |
| **the subject** — the compared observable does not reflect the computation | the differential is weak for these modules however it is seeded |

Only one of these is a problem with the differential. **They are distinguishable by evidence**: sweep
every applicable site in the undetected set. If some site is detected, it was the site. If **none** of
many is detected, that is a far stronger statement than the current one and points at the observable.

## Why this is worth an increment rather than a footnote

The finding as it stands is scoped to three arbitrary positions. Either outcome improves it:

- **Some site detects** → the previous result was an artefact of sampling, and I must say so. This
  line has now corrected a published figure twice by looking harder, and the correction always went in
  the direction of the subjects being better than I reported.
- **No site detects** → the claim strengthens from "missed three mutations" to "missed every
  arithmetic mutation in the module", which is a real finding about the correctness signal for the
  stages.

## Wrong turns to avoid

- **Do not cap silently.** If sites are sampled rather than exhausted, print the cap and the number
  dropped. A silent truncation reads as "covered everything".
- **Do not drop the filters.** An inadmissible mutant and a faulting mutant are still not wrong
  backends; both already cost this line a SIGBUS and a SIGTRAP.
- **Do not re-derive the undetected set by hand.** It must come from the same query that produced it,
  or the two can disagree and nobody will notice.
- **Do not conclude "the observable is weak" from a null result alone.** A null over N sites is
  evidence, not proof, and the alternative — that no sampled site is executed under these seeds —
  remains live unless separately excluded. Say which one the evidence supports and which it does not.
- **Do not widen the mutation family to manufacture a detection.** The family is pre-registered. If it
  is amended, the reason must be applicability, not results, and must precede classification.
- **Do not report a per-module figure without saying how many sites it rests on.** "Undetected" with
  no denominator is the same defect as a coverage figure with no floor.

## What good looks like

Per undetected subject: how many applicable sites exist, how many were tried, how many were usable
after the filters, and whether any produced a difference. A conclusion that names the explanation the
evidence supports and explicitly leaves the other open.
