# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-25 (session 53 CLOSE) — Order 1's largest item is done, and my own estimate of it
was wrong in both directions

## ONE THING IS WAITING ON YOU, AND IT IS NOT NEW

`origin/v0.2.3` is at `153a2d65`, **149 merges**, and **one pull request is open**: `#278`, which
carries the bare-`for` support this document describes. Its continuous integration was restarted by
a force-push and had not settled at session close; the local gate was green on all three signals.
**Merge on 22 of 22.** Publication remains held.

**The floating-point entry ABI is the last of your eight rulings that is not implemented**, and the
`v0.3.0` line has attached a second question to it that you have not seen. Both are described below.
Nothing else needs you.

## What landed

**The confinement analysis is finished, callee summary included.** `src/confine.rs` answers *is this
construction site's region unreachable once its enclosing iteration ends?* per site, as **confined /
cannot establish / escapes**. `module_confinement` summarises what each chunk does with each
parameter first; `chunk_confinement` keeps the summary-free answer. It is a library predicate for the
other line's code generation and is deliberately **not wired into `verify()`**.

| path | sites | confined | escapes | cannot establish |
|---|---|---|---|---|
| no summaries | 33 | 17 | 12 | **4** |
| summarised | 33 | **23** | 10 | **0** |

**Also repaired**, on the `v0.3.0` line's report: a `src/compiler.rs` comment asserting two
`Op::IsStruct` routes verify and then trap `InvalidBytecode` — the class `verify()` exists to exclude
— while the tests beside it disproved it. Under it, a citation to a test **that was never written**,
twice. `tests/comment_citations.rs` now makes a new one fail.

## The finding I would put in front of you if you read one paragraph

**Closing the gap revealed that two verdicts had been wrong, not merely unestablished.**

I aimed the callee summary at the four `cannot-establish` verdicts and it closed all four. It also
moved **two `escapes` to `confined`** — and those had been *false*. Without a summary, a call's
return value is assumed to alias every argument, so a composite passed to a helper and then reached
by the enclosing `return` was reported as escaping **through a route that does not exist**.

**Nothing in the corpus said so.** The count looked healthy and the analysis was confidently
reporting a route that was not there. A conservative default hides false positives exactly as well as
it hides gaps, and the only reason this surfaced is that the fix for one happened to remove the
other. I would not have found it by reading.

**The related correction, which the other line has accepted:** their census concluded two analysis
features were mandatory on day one because 3 of 3 sites were disqualified by `Call`. Only one was
needed — `12_sensor_window.kel` passes a `Word`, so the call never touches the composite. Their test
saw the *opcode*; a dataflow analysis follows the *value*. Their conclusion that admissibility needed
measuring was right; what the measurement said was not.

## The remaining ruling, and the question now attached to it

**The floating-point entry ABI.** Your ruling stands: floating-point registers gate on a feature,
`Fixed` is always available. The asymmetry is unchanged — the FP half may assume `floats`, and the
`Fixed` half is unconditional and is the harder one.

**The `v0.3.0` line has since found a second, related question and it is genuinely yours.** A
`Fixed` value's *representation* is settled — a signed Q-format integer of the word width — but its
**scale is not host-visible**. `Fixed<16>` and `Fixed<8>` differ by 256x and compile to byte-identical
shared-slot layouts, so a host cannot tell them apart. That is sound inside a module, where the type
checker already enforced compatibility, and a shared slot is not inside the module. They measured it
rather than reasoned it, and they price three options, preferring: **refuse `Fixed` in a
host-visible position at the source and make hosts marshal through `Word`.** That is a breaking
source change and needs your authorization. I have not acted on it and it is theirs to bring you.

## The second thing that landed, and why it was worth the detour

The `v0.3.0` line reported a comment in `src/compiler.rs` asserting that two `Op::IsStruct` routes
"verify, receive a memory bound, load, and then trap `InvalidBytecode`" — **the exact class
`verify()` exists to exclude** — while the tests beside it disproved it. Re-measured with controls
rather than taken on trust: it was wrong on **three** counts, not the two reported.

**Under it was the sharper defect. The comment cited a test that was never written**, twice, and the
same file held a second dangling citation of the same kind. **A citation to a test that does not
exist cannot fail** — the shape of the three could-not-fail checks this line paid for in session 52,
one level up.

So it is scoped by class rather than by where I looked. `tests/comment_citations.rs` requires every
four-or-more-word backticked citation in a `src/` or `tests/` comment to resolve somewhere in the
repository. **24 did not.** Three verified and fixed, 21 recorded as a debt register with a guard
against the excuse list outliving its own justification in either direction.

**The threshold is measured rather than asserted**, on the other line's fair point that a cut
defended by rationale is a blind spot with a story attached: two words gives 897 citations and 104
unresolved, three gives 453 and 48, four gives 175 and 21. The 83 extra at two words are dominated
by standard-library names, `.kel` file stems, and prose — **three** would repay triage, not eighty.
The file says plainly that it is silent about shorter citations.

## The smaller thing, because it is the one I would want told

**My citation guard manufactured two of the three findings I forwarded to the other line.**
`must_contain` and `head_name` are ordinary function parameters written inline in a single-line
signature, which the scan did not reach. I passed the list on without checking it, and the substitute
for ground truth this time was **my own instrument's output** — which feels like ground truth in a way
another line's does not. The scanner now reaches inline parameters and the rule is tested directly.

**The one that was real is the best find in it.** A comment cited a *vacuity control* by a name that
does not exist. The control is real under another name, so the guard was never missing — but a reader
checking whether that test could go vacuous would have found nothing and concluded there was none.

## A class I measured and did NOT fix, recorded so it is not mistaken for done

**A measurement written into prose, in a file where every other claim is enforced, reads as a record
and therefore as already-checked.** My own threshold table went stale within hours — in the very
commit that staled it — and every figure moved except the one a test pins. The `v0.3.0` line found
the same in three blocks of one file, including one where the drifted number was the *justification*
for a decision.

**The class is large here: about 180 comment lines across `src/` and `tests/` carry a "measured"
claim.** Most are probably fine and I have not audited them. I fixed my instance and I am not
claiming more than that. The rule both lines arrived at independently is worth applying to new ones:
**a measurement written into a file of tests needs a date and an enforced-or-not marker at the moment
it is written.**

## THE BARE `for` FORM SELF-COMPILES

**Order 1's largest single item is done.** `for v in a..b { .. }` goes through the whole
self-hosted pipeline and matches the reference byte for byte. The construct-support boundary reads
**91 SOk / 1 Refuses / 3 Diverges / 1 RefRejects**.

**Three edits, and the third was not in the estimate.** `parse.kel` accepts the header and emits a
short parts ladder; `reconstruct.kel` assembles the seven-word entry and **synthesises** `i >= limit`
and `i + 1`, since neither corresponds to any token. And **neither driver ever read `for_parts` back
from `reconstruct.kel`** — the plumbing existed and ran in one direction only, so the lowering
received seven zeros and produced a correct loop with every operand at slot 0.

**The six-bit tag space is full**, which the statement fold did not know. Kind 70 truncated to 6 and
the loop vanished into a stray `Not`. I had written that hazard into the plan one increment earlier
and walked into it anyway, because naming a hazard is not finding every site that has it.

**Five gap pins fired and all five are converted**, each saying what became of what it watched.

## `wire.kel` IS CLOSER AND NOT THERE

It **parses correctly now**, to 486 chunks that mean something — the mis-parse that made the old
count a wrong answer is gone. It does **not** self-compile: `self_host_compile` reaches a capacity
limit further down. **A bound is a different failure from a mis-parse**, and it is the next thing
between `wire.kel` and the byte-identity corpus.

## What I would spend the next increment on

**`wire.kel`'s capacity limit**, which is now the only thing between it and the byte-identity
corpus. `self_host_compile(wire.kel)` fails with `IndexOutOfBounds(-1, 1024)` — the shape of a node
array bound, on the largest stage in the corpus at 486 chunks. Diagnose it before costing it: the
last two estimates on this file were both wrong, one high and one low.
Or the remaining `.kel` stages' own bare-`for` uses, which is what keeps `wire.kel` out of the
byte-identity corpus.
