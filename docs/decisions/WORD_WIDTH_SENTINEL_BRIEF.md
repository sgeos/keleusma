# Brief — the float side got a completeness sentinel and the word side never did

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Drafted 2026-09-04.**

---

## The present goals

| goal | state |
|---|---|
| **the word-width sentinel** | **this brief** |
| `f16` lowering | blocked on **reference `f16` arithmetic**, and not this line's |
| narrow word widths in the backend | **deliberately not started** — see below |
| the mutation census | stale and expensive to un-stale; no cheap route |
| publication | held |
| absorption | nothing unabsorbed |

## Why not the obvious increment

Narrow word width lowering is the one substantial backend item **not** blocked on anyone: the machine
supports `narrow-word-8` and `narrow-word-16`, so the differential oracle exists.

**It was scoped and deliberately not started.** The plumbing is tractable — the word type is bound in
**five** places and threaded through 88 integer-typed references — but the semantics are not.
Narrowing changes overflow behaviour, comparison, shift masking, constant truncation and the entry
signature. **Starting a large change to the core lowering and leaving it half-finished would be worse
than not starting**, and the increment below is its precondition rather than a substitute.

## The gap, which is exact

`check_word_width` is a hand-written `word_bits_log2 == 6`. What tests it:

| test | what it actually pins |
|---|---|
| `no_float_sentinel::the_embedded_targets_are_refused_for_word_width_not_float_width` | two targets, `embedded_16` and `embedded_8` |
| `backend_support_census::a_module_level_refusal_is_visible_to_module_refusals` | uses width 5 to test a **different** question: that a module-level refusal is visible to `module_refusals` |
| `differential.rs` | a harness **precondition**, not a test of the refusal |

**No test enumerates the widths, and nothing asserts that 6 is the ONLY accepted one.**

### The concrete failure this admits

Someone widens `check_word_width` to admit 32-bit without updating every width-dependent site. **Both
embedded targets are still refused — they are 8 and 16 — so that test stays green.** A 32-bit module
then lowers with 64-bit semantics: wrong code, no failing test.

**This line already built exactly this sentinel for FLOATS**, enumerating every `float_bits_log2` from
0 to 7 and asserting the accepted and refused sets account for all eight. **The word side never got
it.** The asymmetry is the finding: a guard was hardened on one axis and the neighbouring axis, with
the same shape and the same hand-written equality, was left unenumerated.

## Prior failures this is exposed to

**A guard that agrees with itself.** Repeatedly found this session. The check must be shown to fail
under a mutation, or a pass says nothing.

**A vacuous construction.** The compiler stamps the build's own width, so a width must be imposed
after compilation. If the imposed value equalled the build's, the test would measure nothing —
`backend_support_census` already guards this and its guard should be copied rather than reinvented.

**A refusal credited to the wrong cause.** `no_float_sentinel` keeps the width refusal separate from
the float refusal precisely because folding them would credit float handling with a refusal it did not
make. The same care applies here.

## The wrong turns

**1. Do not widen `check_word_width` while adding the test.** The test records the boundary as it is.
Moving the boundary is the separate, larger increment deliberately not started.

**2. Do not assert only that non-64 widths are refused.** Assert the partition is **complete**, so a
width that fell through both arms fails rather than passing unnoticed.

**3. Do not accept any error as the refusal.** An unrelated failure would satisfy a loose check and
hide a missing guard.

**4. Do not call this coverage of narrow-word support.** It pins a **refusal**. It says nothing about
whether narrow widths would lower correctly, which is what the oracle would be for.
