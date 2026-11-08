# The depth sweep was paying for the census's axis

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status: resolved, 2026-08-29. The deep sweep is back in the everyday gate.** Both mutation sweeps now
run on every gate, so **breadth and depth of mutation sensitivity are protected again.**

## The observation

The two sweeps were split by role but not by cost. The census is breadth — every module, one site,
**every variant**. The deep sweep is depth — the subjects the census finds nothing in, up to eight
sites, and it was **also sweeping every variant**. Variants are the census's axis; the deep sweep was
paying for it twice.

## The experiment, which could have refuted it

Killability requires a variant on which the reference behaves differently, so restricting to one
variant **could** have made fewer mutants killable and shrunk the sweep's findings. That was the
risk, and it is why the comparison came before the decision.

**The table is identical to the recorded baseline** — every row of sites, tried, cmp and inert, and the
same YES set:

> `piano_roll_3`, `piano_roll_4`, `verify_depth`, `verify_types` — **4 moved out of undetected**, in
> both configurations.

The variant sweep was redundant *for this instrument*. It is not redundant for the census, which keeps
it.

## Cost, with load recorded

| configuration | time | load |
|---|---|---|
| both sweeps, all variants | **712s** | ~5–6 |
| deep sweep alone, one variant | **401s** | ~3–6 |
| **whole binary, both sweeps, deep at one variant** | **400s** | **~8.2–8.5** |

The last figure is the decisive one and was taken on a **loaded** machine, so it is conservative. The
two sweeps run in parallel threads, so the binary's wall-clock is the larger of the two rather than
their sum.

**Against the threshold of 600s fixed in the previous increment, this passes**, and it passes on
preserved findings rather than on speed alone.

## What changed and what did not

- **Site depth was not reduced.** Eight sites per subject, as before; sites are this sweep's purpose.
- **The mutation family was not narrowed.** It keeps the widened control-flow swaps.
- **The census keeps all variants.** Only the depth sweep dropped to one.
- **Nothing is skipped by default any more.** The binary reports 9 passed, 0 ignored.

## A stale header, fixed with a check that could fail

The table printed a duplicated fragment — *"at three sites, re-swept at up to 8"* — left by an edit
that changed the census sample. **A header describing a measurement that no longer happens is the quiet
form of the defect this line keeps finding.** The first attempt to fix it silently matched nothing;
the second asserted both that the stale fragment was gone and that the new text was present, which is
the discipline this line adopted after a substitution did nothing twice.

The un-ignore was likewise done by **matching attribute lines, not by a text grep** — the previous
increment's assertion had counted the words `` `#[ignore]` `` inside a doc comment.
