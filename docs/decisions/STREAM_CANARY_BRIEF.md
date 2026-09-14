# BRIEF — the stream driver has slack but no canary

## The finding

`common::general_native_sequence` is the driver behind every general-stream
comparison in this package, including the new 200-tick depth test. It sizes three
buffers:

- the private region as `required + supplement + 64`,
- the shared segment as a flat `4096`,
- the arena region as `host_arena_supplement_bytes(&m) + 4096`.

**Every one of those carries slack, and none carries a canary.** A write past the
published bound lands in the slack and is invisible; a write past the slack
corrupts whatever the allocator placed next.

`vm_and_native_two_arg` — the scalar driver — **does** plant canaries, and its own
comment says why: buffers there were once sized by literals, *"the corpus harness
had the same defect and was repaired hours earlier; this one was left and produced
a SIGSEGV within the day"*. **The stream driver was never given the same
treatment.**

## Why this matters more now

The depth test drives 200 ticks where the previous maximum was six. **If the arena
crept by a few bytes per tick, 4096 bytes of slack would absorb it silently for
tens of ticks and then corrupt** — and the sequences would keep agreeing right up
until they did not, because both implementations would still be computing the same
values.

This line's value proposition is definitive worst-case memory usage. A stream that
stays inside its published bound across many ticks is precisely the evidence that
claim needs, and the driver currently cannot tell.

**What is NOT claimed:** that the arena is bounded. That is what the measurement
is for, and until it runs, the honest position is that nothing here checks it.

## What to build

1. Canaries after each of the three buffers in the stream driver, checked after
   the run, naming which buffer was overrun.
2. A deep-run assertion that the canaries survive 200 ticks, so a per-tick creep
   is caught rather than absorbed.
3. Evidence that the canary actually fires — a check whose reach is demonstrated,
   not assumed.

## Wrong turns to avoid

- **Removing the slack instead of instrumenting it.** The slack may be load-
  bearing for an alignment or a write the bound legitimately excludes. Measure
  first; a canary tells you whether it is needed, removing it only tells you that
  something broke.
- **Claiming a bounded arena from a surviving canary.** A canary proves nothing
  was written past THAT point over THOSE ticks. It is evidence, not a proof, and
  the WCMU claim is a static one this test cannot establish.
- **Sizing the canary region by a literal.** That is the defect being guarded
  against; derive every figure from the published contract.
- **Assuming the scalar driver's canary covers the stream path.** They are
  different functions with different buffers; that assumption is the shape of the
  original defect.
