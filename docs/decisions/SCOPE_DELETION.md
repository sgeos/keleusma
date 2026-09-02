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

## THE FOURTH INSTANCE OF THE SIBLING SHAPE, AND IT WAS IN THE TOOL THAT JUDGES THE OTHERS

Found 2026-09-01 while checking a condition on the release gate. `scripts/gate-summary.sh` renders one
row per gate step. A step that **ran and had no tests** and a step that **never ran** both render as:

```text
  Detached native_codegen/ subproject — SKIPPED   0 binaries      0 tests
  Relative Markdown links                         0 binaries      0 tests
```

**Identical apart from a word the step chose to put in its own name.** A step whose author did not
think to write `SKIPPED` would be indistinguishable from one that legitimately reports nothing.

**Found by RUNNING the tool over a synthetic log rather than reading it.** Reading showed a row
appearing, which was the condition being checked. Only running showed the row was indistinguishable
from its neighbour.

### The `v0.2.3` line's diagnosis is sharper than the fix that was proposed for it

The suggestion was a generic marker: render a resultless step as `SKIPPED`. **They refused it, and
correctly.** The tool *cannot tell* the two apart — both leave the same absence in the log — so
rendering either as `SKIPPED` would **invent information**, and be confidently wrong rather than
merely ambiguous.

> **The defect is that `0 binaries 0 tests` LOOKS LIKE A MEASUREMENT.** Zero and zero is a count. It
> reads as *we looked and found none*, when what happened is *there was nothing to look at*. **The
> tool asserts a result it does not have.**

So the fix is `-- no test results --`, plus a footer stating that such a step either ran without tests
or did not run, **and that the summary cannot distinguish them.** It says what the tool knows and
stops.

### The subclass this names

**An absence rendered as a zero.** A count is a claim that something was measured; a null is a claim
that nothing was. Formatting the second as the first is scope deletion at the level of a single cell,
and it is harder to see than the sentence-level form because **a number looks like evidence.**

The step's own `SKIPPED` label survives on top of the generic fix and is strictly better than it,
because it carries the reason the tool cannot recover — **which is the pattern's own lesson: the
reason has to be recorded beside the cell, and only the party who knows it can record it.**

## What is mechanised, and what is not

Recorded because "we have no instrument" was said on this line and was wrong.

| the name asserts | checkable by |
|---|---|
| which **site** the body reaches | **mutation** — remove the behaviour at the named site; a test that still passes never reached it |
| a **property** the body establishes | nothing mechanical; killing a mutant says nothing about whether an assertion means what its words say |
| a name appearing in a **comment** | the citation guard, which verifies it resolves to something real |

**The expensive half is the checkable half**, and that is the reason to use it: a name asserting
coverage is precisely the name a reader trusts when deciding **not** to write another test.

## ⚠ A SUBCLASS WHERE "WRITE THE POPULATION INTO THE SENTENCE" DOES NOT WORK

**The `v0.2.3` line's observation, and it corrects a wrong generalisation of the rule above.**

The prediction for absorption 44 was recorded as a **named test** — that
`the_parameter_is_operated_on_rather_than_only_passed_through` goes from failed to passed — rather
than as a count. The reason: **"457 passed" would have read identically before and after** if some
other test had gone red in exchange.

**That is scope deletion by CONSTRUCTION rather than by carelessness.** Every other instance in this
record is a true statement with a clause dropped, and a reader could in principle have asked "true of
what?" and recovered it. **A total has no clause to drop.** The population is destroyed by the
aggregation itself.

> **"457 passed under `narrow-float-32` on 88 binaries at commit X" is fully scoped and still cannot
> express "this specific test passes."**

**So the defence is different.** Everywhere else the fix is to write the population into the sentence.
Here the fix is **not to use the instrument**: predict a named member, because no amount of scoping
makes a sum say something about one of its terms.

**This matters because the earlier lesson generalises wrongly if taken alone.** "Attach the
population" would have produced a carefully scoped 457 that still could not have told anyone what
they needed to know.

## ⚠ AN EXCLUSION INSIDE AN ENUMERATION, WHICH IS THE SHARPEST FORM YET

**The `v0.2.3` line's, 2026-09-01, and it cost them a live defect.**

They wrote a test enumerating every float width their predicate admits, building a module at each —
the correct instrument, written for exactly this class. **It skipped the zero row**, with a comment
saying a module declaring zero has no float operations to narrow.

> **"I wrote the assumption into the test as a skip, and then the test could not see the case."**

**An exclusion inside an enumeration is a population deleted from an instrument built to check
populations.** It is the sharpest form in this record because the deletion happens *inside the
defence*, and it is invisible for the same reason as every other instance: the skip carried a
justifying comment, so nothing looked missing.

What it hid: a target with `has_floats = true` and `float_bits_log2 = 0` **compiled, loaded, and
returned 3.75** — computed in `f64` while declaring a zero-bit float, which is the defect their whole
arithmetic-width increment exists to remove, surviving at the one width the narrowing treats as
nothing to do.

**No component was wrong.** The allowlist admits zero, correctly, because a module with no floats has
nothing to narrow. The narrowing does nothing at zero, correctly. The field's documentation says it is
honoured only when `has_floats` is true, correctly. **Nothing enforced the pairing** — the eleven
increments finding in miniature.

### ⚠ THE RULE FOUND A SECOND SKIP, AND IT WAS BLIND TO EXACTLY THE DEFECT IT HAD JUST CAUGHT

**Applying "does this skip increment anything?" to their own file found a second `continue` they had
not reported.** It dropped widths 1 and 2 from the domain — **exactly the two widths the denylist had
wrongly admitted an hour earlier.**

So the instrument that found the denylist inversion **was itself blind to the two members the
inversion was about.** It caught the bug by accident: it failed at those widths only because it tried
to build modules there *before* that skip existed. **Had the skip been written first, the inversion
would have survived its own test.**

### The rule needs a second half, which reading alone does not supply

**Their addition.** An accumulator that increments but is **never compared against the domain size**
is a bucket nobody empties — it counts correctly and proves nothing.

> **Does the skip increment? And is the sum checked against the domain?**
>
> The first stops a deletion. The second stops a classification that classifies into a void.

Applied to this line's own float-width enumeration, which **did not check its total.** It was
structurally safe, because the match over the nested `Result` is exhaustive and every arm pushes —
**but that is a property of the current control flow, not of the test**, and a fourth arm or a
`continue` would break it silently. The sum is now asserted against the domain of eight.

**The `v0.2.3` line expects that line to rot first**, since it is the only one that must change when
the domain does. Recorded so whoever changes the domain knows the assertion is load-bearing rather
than decorative.

### The same shape twice in one day, and the same answer both times

An instrument's own exclusion hid what the instrument existed to find. First **a test name asserting a
subject its body never reached**; then **a comment asserting a row need not be checked**. Both were
TRUE statements. Neither was checkable by reading the justification.

> **The justification is not the thing to audit. The accumulator is.**

### The distinction that separates a safe skip from this one

Checked on this line's own censuses afterwards, since a grep for `continue` finds many.

**Most skips are precondition failures** — an unreadable file, a module that will not compile — and
those are legitimately outside a census's population, which is separately reconciled.

**One skips on a MEASURED property**, in `loop_composite_census.rs`: a loop body that carries a
`Break` is skipped. **It is safe, and the reason is structural**: those sites are added to a counted
`amb` bucket and reported. The rows are classified, not dropped.

> **A `continue` that routes into a counted bucket is a classification. A `continue` that routes
> nowhere is a deletion.**

The two are indistinguishable at the call site — one keyword, one line, both with a justifying
comment. **Only the presence of an accumulator tells them apart**, which makes "does this skip
increment anything?" the question to ask of every exclusion inside an instrument.

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
