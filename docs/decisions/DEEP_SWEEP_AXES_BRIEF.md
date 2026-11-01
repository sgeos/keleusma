# BRIEF — the depth sweep is paying for an axis that is not its own

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The situation

The deep sweep is **opt-in at 710s**, so **regression in the depth of mutation sensitivity is
unprotected day to day.** The census, at 206s, runs in the gate and covers breadth.

**The two instruments were split by role but not by cost.** The census is breadth — every module, one
site, **every variant**. The deep sweep is depth — the subjects the census finds nothing in, **up to
eight sites, and also every variant**. Variants are the census's axis. The deep sweep pays for it
twice.

Stages carry several subject seeds times several argument vectors, so the variant factor is the
plausible multiplier behind 710s against 206s.

## The experiment, which can refute the idea

Restrict the deep sweep to **one variant** and compare its table against the recorded baseline at eight
sites and all variants:

> YES set = `piano_roll_3`, `piano_roll_4`, `verify_depth`, `verify_types` — **4 moved out of
> undetected.**

**Killability requires a variant on which the reference behaves differently.** With one variant fewer
mutants may qualify, so the sweep could report more inert and find fewer. **That is a real loss if it
happens**, and it is exactly what the comparison is for.

| outcome | disposition |
|---|---|
| YES set unchanged | the variant sweep was redundant here; **restore to the gate** |
| YES set shrinks | depth genuinely needs variants; **stays opt-in**, and I say so |

## Wrong turns to avoid

- **Do not restore it without the comparison.** Last increment removed a guard on an unmeasured cost;
  restoring one on an unmeasured loss is the same error mirrored.
- **Do not accept a smaller YES set as "close enough".** The set is the instrument's output. If it
  shrinks, the cheaper sweep is a different, weaker instrument.
- **Do not re-time on a loaded machine.** Record the load average with the timing, as the last
  measurement did.
- **Do not treat the cost hypothesis as established.** That variants dominate is a *guess*; the timing
  after the change is what settles it. This line has twice guessed wrong about where cost sat.
- **Do not silently change what the printed table means.** If it sweeps one variant, the header must
  say so, or a later reader compares two different measurements.
- **Do not reduce the site cap further to buy time.** Sites are this sweep's whole purpose.

## Also, a stale line

The table header prints a duplicated fragment — *"at three sites, re-swept at up to 8"* — left by an
edit that changed the census sample. It should read once and correctly, because a header describing a
measurement that no longer happens is the quiet form of the defect this line keeps finding.
