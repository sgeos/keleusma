# R2 is met in design, unrealised in code, and my prediction about why was wrong

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X. **Checked 2026-09-02**, after absorption 47 landed the `Text<N>` type surface.
Brief: [`TEXT_REQUIREMENT_CHECK_BRIEF.md`](./TEXT_REQUIREMENT_CHECK_BRIEF.md).

---

## The claim being checked

`TEXT_N_REQUIREMENTS.md`, addressed to the `v0.2.3` line: **R2, a flat layout with no reference
field**, on the ground that this backend already packs and reads flat composites and would need no new
machinery. **That was a statement about how much work this line has to do, told to another line so
they could plan around it**, and it had never been checked against anything.

## The status, stated as one of three rather than as an absence of contradiction

**Met in design. Unrealised in code. And the current `Text` is deliberately the opposite shape.**

**Met in design**, explicitly and in the same terms. `TEXT_CAPACITY_TYPE.md`:

> **"Runtime representation: a FLAT composite, no handle"** — *"A `Text<N>` is a flat composite
> carrying no reference field — for example a length word…"*

That document also records **"THIS SECTION FIRST SPECIFIED A HANDLE, AND THAT WAS WRONG"**, so the
shape R2 depends on was reached by correction rather than by default.

**Unrealised in code, deliberately.** `ScalarKind::Text` is still **two words**, and the comment says
why:

> *"A flat `Text` field is an arena `(ptr, len)` handle … an arena string's length lives in the second
> word and nowhere else … Sizing this by one address would drop the length and produce a silent wrong
> read … the one-address form becomes correct only once `Text<N>` removes the dynamic case from this
> kind, and doing it in one step spends a single `BYTECODE_VERSION` authorization rather than two."*

**So R2's condition does not hold for today's `Text`, and their plan is precisely to make it hold.**

## ⚠ MY PREDICTION WAS WRONG, AND IN A WAY WORTH RECORDING

**Predicted: no layout exists yet**, reasoning from the landed commit touching the parser, type
checker, monomorphiser, layout pass and zero values — but not `value_layout.rs`.

**A layout does exist.** It is for the *existing* `Text`, not for `Text<N>`, so I inferred "no layout
for the new type" from "no change to the layout file" and stated the stronger thing. **The falsifier
fired and the reasoning behind it was a scope error**: absence of a change is not absence of a
definition.

## What this does NOT establish

**The backend refuses `Text` at every reachable route**, so R2 is **unexercised**, not met in
practice. A refusal is not compliance, and nothing here has packed a text value.

**No work is required on this line yet**, and the "my share is small" claim I gave them remains
conditional on a shape that exists in their design and not in their code.
