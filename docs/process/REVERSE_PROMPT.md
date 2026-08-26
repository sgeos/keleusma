# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-26 (session 54) — I recorded the wrong cause for `wire.kel`, and the tree
now names the right one

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

`#278` merged: `origin/v0.2.3` is at `1627e65b`, **150 merges**. The bare-`for` support is in.
Publication remains held.

**The floating-point entry ABI is still the last of your eight rulings that is not
implemented**, and the `v0.3.0` line still has a second question attached to it — where a
`Fixed` shared slot's SCALE lives, since `Fixed<16>` and `Fixed<8>` differ by 256x and compile
to byte-identical host-visible layouts. **It is theirs to bring you and I have not acted on
it.** Nothing else needs you.

## The correction I would put in front of you

**I recorded `wire.kel`'s failure as a capacity bound. It was not, and the reading was wrong
in the way the handoff warns against three paragraphs above where I wrote it.**

The failure was `IndexOutOfBounds(-1, 1024)`. I read the `1024` and inferred a node-array
bound. But `-1` is *below the start*, not past the end — the number in an unnamed message
identifies an array's size and says nothing about why the index was bad.

**The true cause is a third thing, in a third place.** A record range leaves **two** nodes
where it must leave one, so the record stream carries an unfolded operand. The `-1` trap
fired several steps downstream of that, on state that was already wrong. Diagnosing the
failure directly would have sent me to investigate the work stack, which was innocent.

**That is why the instrument came before the diagnosis**, and it is the argument for the
whole increment.

## What landed

**`reconstruct.kel`'s failure modes are named.** The stage had none. Derived from the source
it declares **26 arrays in six size classes**, so **25 of the 26 shared a failure message
with at least one sibling** — the same defect `parse.kel` carried until thirteen causes were
named, where tracing one such failure cost seven increments.

Five causes now report by name. `tests/reconstruct_failure_modes.rs` provokes four of them
with real inputs and pins the fifth's unreachability.

**Two things surfaced that were not failures at all before they were named:**

- `reconstruct_range` read slot zero unconditionally. An **empty** range returned a *stale*
  node index left behind by the previous range; an **over-full** one *silently discarded*
  every node but the first. Neither trapped. This is what caught `wire.kel`.
- A record range longer than the input arrays trapped `LoopLimitExceeded`, a
  virtual-machine message naming no cause whatsoever.

**Two of my own guards could not fire as first written, and only running them showed it.**
One sat inside a walk whose `limit` aborted a full iteration before the check; the other is
unreachable by construction because `push` has one caller and the node guard fires first. The
second is kept with its *invariant* pinned rather than deleted, so a future second caller
fails a test instead of silently making the guard live.

**A regression I caused, and why I did not relax the test for it.** The float program that
used to be mis-reconstructed and caught downstream by the byte-comparison oracle is now
refused at source — but the refusal did not name the chunk, which is the operator-facing
value of that path. The chunk name is threaded through instead, so the earlier refusal keeps
the later guarantee.

## What I got wrong inside this increment

**I wrote "seven arrays" when the family was 26.** Seven is what the failure in front of me
pointed at. Seventh recorded instance of deriving a set from the part of the system I was
thinking about rather than from the system. The correction is left standing in the brief.

**The citation guard caught me citing two tests before they existed** — third time it has
done so, and it was right each time.

## What is next, and what it is NOT

`wire.kel` now fails with a named cause: a range leaving two nodes. **That is a `parse.kel`
emission defect, not a bound**, so raising a capacity would be the wrong repair.

**Naming a cause and repairing it are two claims with two evidence bars**, and this increment
makes only the first. The repair is the next increment and it is deliberately not attempted
here.

**Scope stated so the gap is visible**: guards cover the 1024-wide class only. The other
nineteen arrays are named in a register that fails if the stage grows an array without either
a guard or an entry.

---

# Previous session (53)

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
