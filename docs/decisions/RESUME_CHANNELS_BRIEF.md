# Brief — the append-only channel has no record of this session's second half

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-02.**

---

## The goal set, and the capability queue is empty

| goal | state |
|---|---|
| **G14** complete the resume channels for this session | **unblocked, and the subject of this brief** |
| `f16` | **no oracle** — the reference refuses widths 3 and 4 at load, so a lowering cannot be validated |
| `Text<N>`, `Opaque` | the `v0.2.3` line's |
| publication | held |
| absorption 46 | nothing unabsorbed |

**There is no unblocked capability work and I am not going to invent some.** Saying so is the
accurate report; manufacturing a goal to avoid saying it would be worse than the gap.

## The gap

`CLAUDE.md` names three resume channels. Two have been maintained this session — the handoff and the
bounded latest-state channel. **The third has not.**

`DESIGN_JOURNAL.md` is the **append-only** record of increment reasoning. Its newest entry is the
C-host example, which predates **twelve commits**: the fused-multiply-add guard, both linkage
censuses, the unwind removal, the bound-transfer narrowing, the no-floats sentinel work, the
reachability fix, the generated host contract, the untested-combination run, the ladder correction,
absorptions 44 and 45, and the roadmap correction.

**The handoff and the bounded channel are overwritten**, so they carry state, not history. **The
journal is where a future session looks for why something was done rather than what is true now.**
Twelve increments of reasoning currently survive only in commit messages, which nobody reads in
order.

## What the entry must contain, and what it must not

**The reasoning, not the changelog.** A list of what landed is already in the handoff. What is absent
is why — in particular the decisions taken *against* doing something, which are invisible in a diff.

**The reversals, as reversals.** Two decisions were recorded and then changed: "change nothing in the
whitelist", and the claim that narrow float rungs buy native instructions. Both were corrected with
their superseded text kept. **A journal that records only the final position teaches nothing**, which
is the same argument used for keeping the superseded text in the first place.

**The errors, including the ones nobody else would have found.** A test verified under one
configuration while its claim ranged over two. A false statement nearly shipped into a generated
header. A count quoted without its population, by the author of the record naming that failure.

**Do NOT re-derive figures.** Every count in the journal is carried from a measurement already in the
tree, and is marked as carried with its population attached.

**Do NOT append a second entry for the same session.** The journal is append-only and newest-first;
one entry per session is its shape.

## A named residual, stated rather than closed

**The workspace was verified on this branch under DEFAULT FEATURES ONLY.** The release gate runs five
configurations; the other four have not been run against this branch's corpus.

**Whether that matters is bounded and should be said accurately**: the corpus-reading tests are not
feature-gated, so default features already exercises them, and the `self-host` configuration reads
`src/selfhost/kel`, where this branch is byte-identical to `origin/v0.2.3`. **So the marginal risk is
low and it is not zero, and low-and-not-zero is the honest form** rather than either "verified" or "a
gap".

Recording it beats half-closing it in the last hour of a long session.

## The wrong turns

**1. Do not write a triumphal entry.** The most useful entries this line has are the ones recording
what was got wrong.

**2. Do not restate the handoff.** Different channel, different job. Duplication rots at different
rates and the reader cannot tell which copy is current.

**3. Do not let the journal claim the session is finished.** It records increments; whether the line
is done is not its call.
