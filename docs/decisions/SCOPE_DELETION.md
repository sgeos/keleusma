# The failure class both lines produced six times in one day

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Recorded 2026-09-01**, jointly by the V0.3.X and `v0.2.3` lines during a day in which each caught
the other producing it. **The unification is the `v0.2.3` line's**; the instances are from both.

---

## The class

**A true measurement, quoted with its scope deleted.**

| what was said | what was true |
|---|---|
| "2 of 11 test names overclaim" | of names **opening with a quantifier** |
| "the branch compiles" | under **default features** |
| "the documentation does not exist" | on **remote branches** |
| "the gate is green" | by **the wrapper's exit code** |
| "eleven tests cover the sites" | **four** of them |
| "the toolchain requires strictly more" | by **count**, not by containment |

**Not one of these is a wrong number, and not one came from a careless check.** Every measurement was
performed correctly. What went missing was the clause naming what it ranged over.

## Why it is durable

**The scoped and the unscoped statement are the same sentence minus a clause.** Nothing looks missing.
There is no gap on the page where the population used to be, so proofreading does not catch it and
neither does a reviewer, because both are reading a sentence that parses.

**The strongest evidence that it is structural rather than a lapse**: this line's own design journal
had already recorded that its 2-of-11 figure was a lower bound over a filtered population, **under a
heading reading "The rate is a lower bound, and saying so matters"** — and the figure was then quoted
unscoped anyway. **The record was correct and one layer away, and one layer away was enough.**

## The discipline, which is not "measure more carefully"

Both lines measured correctly every time. Care was not the missing ingredient.

> **Write the population into the sentence, so that deleting it is visible as a deletion.**

"2 of 11 quantifier-opening names." "Compiles under default features." "Green by the wrapper's exit
code." "Strictly more by count, not by containment."

**Clumsier, and the clumsiness is the point.** A clause you have to actively remove is one you notice
removing. A clause that was never written is one nobody can miss.

## ⚠ THE SIXTH INSTANCE IS DIFFERENT IN KIND, AND IT IS THE WORST OF THE SET

The `v0.2.3` line's observation, and it is the most uncomfortable statement in this record.

**The first five were claims nobody had built an instrument for. The sixth had three**, written for
the express purpose of catching that claim being wrong, and **all three passed.**

They passed because they tested containment in **one direction**, and the claim was about
containment. **The falsifiers inherited the claim's own unstated framing.** They could establish that
the claim held within that framing and could not reach the framing itself.

> **A falsifier that shares its claim's framing cannot falsify the framing.**

It is not a weak instrument. It is a correctly built instrument **pointed along the axis the error is
not on**. And it is worse than no instrument, because it produces a green that reads as vindication
rather than as absence of evidence — the same argument as a test whose name asserts a subject its body
never reaches.

**So "one layer away was enough" has two forms, and the second is worse.** In the journal case the
correct caveat sat one document away. Here **the correct check sat inside the test written to catch
exactly this**, and was still on the wrong axis.

## The sibling shape, which appeared three times the same day

**A covered cell and an unexamined cell are indistinguishable without the reason recorded beside
them.** It arrived from three directions:

- **A witness table.** An empty cell meaning "cannot exist" and one meaning "not found yet" look
  identical. Established when a modulo witness search over 400,000 float pairs correctly found
  nothing, because the result is exactly representable.
- **A mutation table.** A *survived* cell meaning "exact, so narrowing is the identity" and one
  meaning "nobody wrote the test" look identical.
- **A name asserting reach.** Mutation checks it **only where a mutant exists**; a name asserting
  reach into an unmutated site is unchecked and looks exactly like one asserting reach into a covered
  site.

This is the same class one level up: **the scope of a coverage claim, deleted.** The cell says
covered; what is true is covered-by-the-instruments-that-exist.

## What is mechanised, and what is not

Recorded because "we have no instrument" was said on this line and was wrong.

| the name asserts | checkable by |
|---|---|
| which **site** the body reaches | **mutation** — remove the behaviour at the named site; a test that still passes never reached it |
| a **property** the body establishes | nothing mechanical; killing a mutant says nothing about whether an assertion means what its words say |
| a name appearing in a **comment** | the citation guard, which verifies it resolves to something real |

**The expensive half is the checkable half**, and that is the reason to use it: a name asserting
coverage is precisely the name a reader trusts when deciding **not** to write another test.

## No rate is given here, and the reason is stronger than caution

**Six instances in one day is a count over an unbounded denominator.** Neither line knows how many
scoped claims it made correctly today, and **a correct one leaves no trace** — nobody records a
sentence that kept its population.

So the denominator is not merely unmeasured. **It is unmeasurable from the artifacts either line
produced.** A rate here would not only be this class committing itself inside its own record; it would
be a rate whose denominator does not exist.

## Where the other line's copy lives, and why not at this path

The `v0.2.3` line records the same finding in its own design journal, where five of the six instances
occurred, **referenced in prose rather than by link**. Two reasons, both theirs: an add-add collision
on one path conflicts on every sync, and a link to a file existing only on `v0.3.0` fails that line's
markdown gate.

**This file stays canonical.** That is the third time in one day the two lines' records have had to be
joined by prose instead of a link — a small standing cost, deliberately preferred to a permanent
conflict.
