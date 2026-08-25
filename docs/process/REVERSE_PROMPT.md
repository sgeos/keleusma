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

`origin/v0.2.3` is at `71792ecc`, **140 merges**. Publication remains held.

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

## A defect on my surface, reported by the other line and confirmed

A comment in `src/compiler.rs` asserts that two `Op::IsStruct` routes "verify, receive a memory
bound, load, and then trap `InvalidBytecode`" — **the exact class `verify()` exists to exclude**. My
own repair closed both routes and the tests beside the comment prove it. An auditor reading that
paragraph would conclude the load-time guarantee is breached while the tests disprove it. **This is
a documentation defect, not a code defect**, and it is next.

## Next intended step

Repair that comment, with the two named routes re-measured rather than assumed closed. Then the
callee summary, which is the one increment the confinement analysis is missing and whose effect is
already visible as a number that should move.
