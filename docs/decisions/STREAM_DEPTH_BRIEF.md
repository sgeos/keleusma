# BRIEF — how deep has a stream actually been driven?

## The finding

This line's central architectural claim is that **the arena IS the coroutine
instance**, with `Op::Reset` rewinding it between runs. A defect that accumulates
across ticks — a pointer that creeps, state that survives a rewind, an arena that
grows — would be invisible at shallow depth.

**Measured: the deepest any hand-written stream witness drives is SIX replies.**
The distribution across the suite is 2 to 6, with `&[11, 20, 31, 40, 55, 60]` the
longest.

The self-hosted stage differential does far better — 180 to 300 result comparisons
per stage, 2460 total — but those are stage sources with particular shapes, seeded
from the shared segment. **A general stream carrying a local across a yield, driven
for hundreds of ticks, is a different subject**, and it is the shape that already
carried one defect this line fixed: a local live across `yield`, wiped by the entry
preamble on every resume.

## What the measurement found

**No divergence.** Three general shapes — a plain two-yield, a local carried across
a yield, and a branching form — agree with the reference at depths 6, 50 and
**200**. The finite two-yield stream, which completes and is rewound on the next
call, agrees over 40 ticks with the cycle visible in its values.

That is not a defect found; it is **evidence raised from 6 ticks to 200** on the
claim this line rests on, which is worth pinning so it cannot silently regress.

## The degenerate/general split, learned by tripping over it

`loop main(t: Word) -> Word { yield t }` **panics the general driver**, and that is
correct behaviour. A single-yield stream lowers to a **degenerate** chunk — 1
parameter, no trailing pointers — while a two-yield stream lowers **general**, with
`declared + 3`. Measured, not assumed. The general driver asserts the three
trailing pointers precisely because it names that signature by hand and cannot
detect a change to it.

**Fourth time this session a probe implicated itself.** The others were a float
argument passed as `i64::MIN`, a float return read as an integer, and a hand-named
signature that took a SIGBUS.

## Wrong turns to avoid

- **Reading the degenerate panic as a backend fault.** It is a driver/shape
  mismatch, and the signature counts prove it.
- **Driving depth without varying the replies.** A constant reply can mask a
  resume path that ignores its input; the replies cycle.
- **Claiming the arena does not grow.** Nothing here measures memory. The claim
  proved is agreement over ticks, which is weaker and is all that was measured.
- **Pinning a depth so large the gate slows.** 200 ticks is fast; the point is the
  order of magnitude over 6, not a record.
