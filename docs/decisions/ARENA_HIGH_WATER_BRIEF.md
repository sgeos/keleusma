# BRIEF — does the arena grow across ticks?

## The gap, in the tree's own words

`native_codegen/tests/stream_depth.rs` closes its header with:

> **Nothing about memory.** Whether the arena grows across ticks is not measured
> here; only that the two implementations produce identical sequences. A bounded
> arena is this line's claim elsewhere and is not evidenced by this file.

The session-close handoff repeats it as pickup item 3: *"Nothing here measures
memory. Streams agree over 200 ticks and no buffer canary is disturbed, but
whether the arena stays bounded is a static claim this line has not evidenced."*

**This is the largest unevidenced claim on the line.** The ecosystem's value
proposition is definitive worst-case memory usage. "The arena IS the coroutine
instance" is the sentence the whole region design rests on, and the only thing
measured about it so far is that a sentinel 16 bytes past a generously sized
buffer is undisturbed.

## What a canary does and does not say

The canaries added on 2026-09-14 answer **"did anything write past the bound?"**
They say nothing about **"how much of the bound is used, and does that amount
depend on the tick count?"** A stream that consumed four more bytes per tick
would pass every canary in the package for the first several hundred ticks and
then corrupt — **while the sequences kept agreeing right up until they did not**,
both implementations computing the same values from a buffer one had overrun.

The missing measurement is the **high-water mark**: the largest extent of the
arena region the lowered stream ever touches. If that number is the same after
one tick as after two hundred, the arena is being reused rather than consumed,
which is precisely what `Op::Reset` is supposed to guarantee.

## Method, and why this particular one

**Fill the region with a known pattern, drive the stream, then measure the
highest index that differs from the pattern.** That is the touched extent,
cumulative over the whole run, requiring no knowledge of the backend's internal
bookkeeping — which is the point, because a measurement that reads the backend's
own cursor would be reporting what the backend believes rather than what it did.

**A single fill pattern is unsound and must not be used.** A byte legitimately
written with the same value as the fill is invisible, so the extent is
under-reported and the test reads as a stronger result than it is. **Two runs
with two complementary patterns, taking the larger extent, removes that hole
entirely**: a byte can equal one pattern or the other, never both.

**The control is what makes it a measurement rather than a ritual.** Two
requirements, and a version without them is not worth writing:

1. **Non-vacuity.** The extent must be greater than zero, or the instrument is
   reporting that nothing was written and passing for that reason.
2. **Discrimination.** Two stream shapes with genuinely different arena appetites
   must produce genuinely different extents. An instrument that returns the same
   number for every subject is measuring the buffer, not the program.

## Prior failures this must not repeat

- **A guard with no reach, sitting in the file looking like coverage.** The
  private-region canary could never have fired until a stream that writes a
  private slot was added. **Check that this instrument can see growth before
  claiming it saw none.**
- **Keying an instrument by the wrong thing and trusting its zero.**
  `backend_support_census.rs` reported 0 refused while four variants were broken,
  because it keyed by opcode NAME where the OPERAND decides. Here the analogous
  error is keying on the canary tail and calling it a memory measurement.
- **A probe implicating itself.** Four times this session: a float argument passed
  as `i64::MIN`, a float return read as an integer, a hand-named signature taking
  a SIGBUS, a degenerate stream handed to the general driver. **A single-yield
  stream lowers degenerate and the general driver refuses it correctly** — any
  subject here must be a two-yield or deeper shape.
- **Recording a conclusion the run did not reach.** If the pattern fill changes
  the program's OUTPUT, that is a finding about the backend depending on
  zero-initialised host memory, and it is reported as such rather than worked
  around by quietly reverting to a zero fill.

## The honest boundary to state in the file

Whatever this finds, it is evidence about **these shapes over these tick counts**,
not a static bound. A dynamic high-water measurement cannot establish worst-case
memory usage; it can only refute the specific hypothesis that usage grows with
tick count. **Say that in the file**, because the failure mode this package keeps
producing is a record that outlives, or overstates, what its subject supports.
