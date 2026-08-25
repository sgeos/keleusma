# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-24 (session 53) — the confinement analysis exists, and a census it was
commissioned from turned out to be measuring the wrong thing

## ONE THING IS WAITING ON YOU, AND IT IS NOT NEW

`origin/v0.2.3` is at `44b3d071`, **142 merges**. Publication remains held.

**The floating-point entry ABI is the last of your eight rulings that is not implemented**, and the
`v0.3.0` line has attached a second question to it that you have not seen. Both are described below.
Nothing else needs you.

## What landed

**The confinement analysis is done.** `src/confine.rs` answers *is this construction site's region
unreachable once its enclosing iteration ends?* — per site, over a chunk the caller holds, as
**confined / cannot establish / escapes**, exactly the interface that was settled before it was
written. It is a library predicate for the other line's native code generation and is deliberately
**not wired into `verify()`**: a predicate that rejects nothing has no business in the load path.

**Three of the four per-iteration corpus sites come back confined.** The crude test the other line
ran admitted none of three.

## The finding I would put in front of you if you read one paragraph

**A measurement I was given as a requirement was an artefact of the instrument that produced it.**

The `v0.3.0` line measured that every composite site in the corpus was disqualified by *two*
independent things, and concluded that a confinement analysis needed two features on day one or it
would admit nothing. I took that as the specification and wrote to it. **Only one of the two was
real.** `12_sensor_window.kel` calls `scale(raw[i])`, and `raw[i]` is a `Word` — the call never
touches the composite at all. Their test saw the *opcode*; a dataflow analysis follows the *value*.

I want to be precise about what this does and does not say. **Their conclusion that admissibility
needed measuring was right, and it is why the corpus was extended and why the isolate script
exists.** What was wrong was what the measurement said, and only a better instrument settled it.
Both lines reached this independently within the same day, and their census now reports the two
causes separately instead of conflated.

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

## What I would spend the next increment on

**The callee summary**, which is the one thing the confinement analysis is missing and whose effect
is already visible as a number that should move: the 4 `cannot-establish` verdicts in the corpus
count. The call graph is acyclic, so a bottom-up summary terminates with no fixpoint.

**Then Order 1's bare-`for`**, unchanged and still the largest single win.
