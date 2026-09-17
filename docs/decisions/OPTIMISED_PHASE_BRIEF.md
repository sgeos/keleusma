# BRIEF — make the optimised run part of the gate, for the half that is cheap

## The reason it stayed opt-in has been measured away

`KEL_OPTIMIZE` existed, was documented, and **no script set it** — so until
2026-09-17 the corpus had been optimised and validated but never optimised and
RUN. The stated reason for leaving it manual was cost.

Measured, on this machine, today:

| phase | at `-O0` | under `default<O2>` |
|---|---|---|
| everything but the corpus | ~175s | **155s** |
| the corpus differential | ~380-420s | **420s** |

**The non-corpus half costs nothing** — it came out faster, which is noise either
way. That half contains every hand-written differential and **all several thousand
generated programs**, which are the densest subjects in the package.

The corpus half is a different matter: adding it doubles the gate's worst phase,
which already had to be SPLIT to fit the harness ceiling under `narrow-float-32`.

## What to do, and what not to

**Add one optimised phase covering the non-corpus half. Leave the corpus half
opt-in**, with the cost written down so the next person weighing it has the number
rather than an impression.

**A run nobody performs proves nothing** — that is what this whole thread
established. A phase in the script is performed by everyone who runs the gate; a
variable in a comment is performed by whoever remembers.

## Prior failures to avoid

- **The gate must still report one verdict.** It accumulates `fail=1` across
  phases; a new phase that exits early or is chained with `&&` breaks that, and an
  assembled verdict silently missing a phase is the ninth row in this project's
  catalogue — added earlier today, after a `clippy` failure stood for four commits.
- **Do not let the variable leak.** It must be set for the new phase only, so no
  other phase silently changes meaning.
- **Do not claim the gate now covers optimised execution generally.** It will
  cover the non-corpus half. The corpus half remains a manual sweep, and the
  script should say so where someone reading it will see it.
- **State the cost where it is decided.** The gate's own header explains why it is
  a script rather than a note; this addition needs the same treatment.

## The check that this increment actually did something

**Run the gate and confirm the new phase appears and passes.** A phase added to a
script and never executed is precisely the shape this thread keeps finding.
