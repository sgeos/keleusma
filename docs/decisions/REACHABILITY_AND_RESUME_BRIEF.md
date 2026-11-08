# Brief — close the one promise made to another line, and re-anchor the resume

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-01**, late in a long session.

---

## The goal set

| goal | state |
|---|---|
| **G8** make `float_type` unreachable with a width it cannot serve | patch staged; the stated fix was insufficient and the correction is the interesting part |
| **G9** refresh the handoff | **27 commits stale**, banner reads "after absorption 42" |
| absorption 44 | gated on the `v0.2.3` gate; carries the arithmetic width and should turn the one red `f32` test green |

**Nothing else is unblocked.** `f16`, `Text<N>` and `Opaque` are elsewhere. Inventing a fourth goal
at this point in the session would be worse than closing two properly.

## G8: what I promised, and why it was not enough

I told the `v0.2.3` line I would move the eager `float_type` call into the branch that uses it, a
one-line change making a recorded-as-refuted prediction true.

**Staging it found that is insufficient.** The entry-ABI refusal — the guard that rejects a float
signature at a width this backend does not lower — **runs AFTER the chunk-declaration closure.** So
moving the call would fix the no-floats case and leave the case that actually matters: a two-byte
width with a float signature would still reach `float_type` before the refusal fired.

The correction is to **hoist the refusal above the declarations**. Its inputs are the float width and
the module's signatures, both available immediately, so the hoist is safe. Together the two changes
make the property true rather than nearly true.

**This mirrors the `v0.2.3` line's own design**, which moved a refusal to load rather than threading a
`Result` through ten hot-path sites: *do not thread an error through a hot path when you can arrange
for the path never to be reached with a bad value.*

### The wrong turns for G8

**1. Do not thread an `Option` through the call sites.** Six sites, two of them inside a closure that
cannot use `?`. That was the option I rejected earlier for good reason, and the reason has not
changed.

**2. Do not report the prediction as having been right.** It was recorded as refuted by measurement.
Making it true afterwards does not retroactively make it true when written, and the record should say
it was false and then made true.

**3. Hoisting changes which error a multi-fault module reports first.** If a test expects a different
refusal, that is a real consequence and gets reported, not worked around.

**4. The staged patch asserts on every anchor and on the expected call-site counts.** Keep that. A
fix pass that counts its own replacements reports success at four of five every time — measured
today, on this line.

## G9: the handoff is stale, and that is a recorded failure of this line

The banner reads *"after absorption 42"* while 27 commits have landed, including absorption 43. **An
earlier version of this file carried a banner three days out of date while its body was current**, and
the file itself records that the banner is its least-refreshed line.

**What must be true of the refresh:**

- **Run the ancestry block rather than trusting it.** It has been wrong before, once by anchoring on a
  branch tip that moved.
- **Add an anchor per increment landed**, so a future validity check fails loudly if history is
  rewritten.
- **Re-derive every figure, and stamp it even when it re-derives unchanged** — this line's own
  standing rule, on the ground that the stamp is the only evidence anyone looked.
- **Carry the open items forward with their owners**, since three of four are not mine.

### The wrong turns for G9

**1. Do not write a hash into the banner.** A refresh takes more than one commit, so any hash there is
stale on arrival. The file records this having failed twice.

**2. Do not quote a test count from memory.** Every figure gets re-derived or is marked as carried and
dated.

**3. Do not describe the session as a list of what was built.** The most useful entries this line has
recorded are what was *refused* and what was *got wrong*, because those are what a reader needs before
changing the same code.

**4. Scope every figure in the sentence that carries it.** Six instances of scope deletion were
produced today across two lines. A handoff is precisely where an unscoped figure does the most damage,
because it is read by someone with no context to supply the missing clause.
