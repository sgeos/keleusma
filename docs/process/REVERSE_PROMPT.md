# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## TWO DEFECTS, BOTH SILENT, BOTH AGREEING WITH YOUR RUNTIME ON EVERY SUBJECT THAT EXISTED

I set out to make one prose premise checkable. The premise held. **The increment found two defects
that the check was not pointed at**, and each had agreed with your runtime everywhere it had been
driven.

### 1. A local live across a `yield` was wiped on the way back in

```
loop main(t: Word) -> Word { let keep = t + 100; let r = yield 1; yield r + keep }
   yours [1, 108]        mine [1, 3]
```

The entry preamble zeroes every non-parameter local so an unwritten slot reads as your `Unit` rather
than as `undef`. **Right for a function entered once; a resumable stream is entered once per
suspension.** The initialisation now sits on the first-entry edge of the dispatch.

**No existing stream subject had a local live across a suspension**, which is why the suite was green
over it. The parameter store stays unconditional, and that asymmetry is yours, not an inconsistency:
your resume writes the incoming value into slot 0, and a stream reading its parameter after a
suspension sees the resume value on both sides.

### 2. A composite in a data slot was stored as a POINTER, not a copy

`14_frame_log.kel` opens by saying a slot holds *"a COPY of the composite's bytes ... not a reference
to the ephemeral body"*. **My lowering stored the reference.** A data slot access is one word; for a
flat composite the operand is the address of a body in the ephemeral region.

It agreed with you anyway — that script reads its slot in the iteration that wrote it, so the aliased
bytes still hold the right values, and it yielded `[81, 84, 87, 90]` on both sides across four
cycles. Separating a copy from an alias takes a subject that writes the slot on one loop iteration and
then rebuilds the SAME site twice more before reading back:

```
   yours [0, 0]        mine [2, 2]
```

`2` is the last body built at that site.

**Refused rather than fixed, and I want the cost visible: `14_frame_log.kel` no longer lowers.** The
correct lowering copies the body to the offset `private_composite_layout` names, but that pool's base
is not pinned against your runtime in my ABI, and guessing it puts a wrong answer where a refusal
belongs. Your answer for the discriminating subject is pinned in `private_slot_composite.rs`
independently of my backend, so the eventual copy has a target that does not depend on me.

## THE `Op::Reset` PREMISE ITSELF: CHECKED, AND THE COMMENT WAS OVER-CLAIMING

The comment said every site is overwritten by the next iteration. **It is not** — an iteration that
skips a site's constructor leaves the previous body sitting in your buffer, and the test reads those
bytes back out and tells them apart from a poison and from a rebuilt body. What makes the retention
unobservable is provenance plus no pointer outliving its iteration; the five escape routes are
tabulated with their mechanisms, including the two closed only indirectly.

## HOW BOTH DEFECTS ARE THE SAME SHAPE

Neither was a wrong calculation. **Each was a correct operation applied across a boundary it does not
hold across**: zeroing locals is right on entry and wrong on re-entry; a word-sized store is right for
a scalar slot and wrong for a composite one. Both coincide with the correct behaviour in the easy
case, which is why every existing subject agreed.

## WHAT I GOT WRONG

- **I was about to file a mechanism I had not verified.** The escape-route table's persistent-storage
  row cited a test that measures the population of slot-homed composites, not copy semantics. Reading
  the citation before filing it is the only reason the second defect surfaced.
- **My own completion condition would have passed a lowering carrying both defects.** Eight clauses,
  seven met, and neither defect is described by any of them.
- **I ran a gate while still editing** and it correctly reported NOT FROZEN. Self-inflicted, and the
  instrument caught it.

## STILL WITH YOU, NEITHER ACTED ON

1. **A `confine.rs` index panic on a truncated op stream.** Three mutation kinds reach it, one guard
   closes all three. My sweep allows it BY ORIGIN FILE and asserts it still fires, so your fix will
   fail my test and delete the carve-out rather than let it outlive the defect.
2. **A multi-parameter stream faults after its first rewind.** Defect, intended consequence, or a
   shape the verifier should reject? Only the third needs no runtime change.

## THE NEXT INCREMENT, UNLESS YOU REDIRECT

Implement the persistent composite copy: pin the pool's base against your runtime, copy
`byte_size` bytes at the offset `private_composite_layout` names, and drive the discriminator through
the differential. That removes the corpus refusal this increment added.
