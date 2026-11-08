# BRIEF — the canary that cannot fire, and the two opcodes nothing drives

## Why these two, together

Both are the same defect class, which is this line's most frequently recurring
one: **an instrument that looks like coverage and cannot produce a finding.**

- The **shared-segment canary** in `general_native_sequence` is documented in its
  own comment as *"NOT PROVEN, and cannot be by this helper"*. It sits beside two
  canaries whose reach was proven by shrinking the buffer until they fired. It is
  not evidence of anything today, and says so.
- **`CallVerifiedNative` and `CallExternalNative` have no driven witness.** Sixty
  two of sixty six opcodes have a program that emits them, runs on both
  implementations, and agrees. These two do not — not because they are broken,
  but because the native-registration machinery lives inside `corpus_differential`
  rather than in a reusable helper. **That is a fact about where code sits, not
  about the backend.**

## The cause of the first, precisely

`general_vm_sequence` builds its `Vm` and calls `vm.call(..)`. The runtime also
offers `call_with_shared` and `resume_with_shared`, which hand the interpreter a
host-owned `&mut [u8]` shared segment. **Because the reference driver uses the
plain form, a stream declaring a shared slot is refused by the reference**, so no
shared-slot stream is drivable, so nothing ever writes to the native side's shared
buffer, so its canary cannot fire.

The fix is to drive the reference through the `*_with_shared` pair, with the same
buffer contract the native side already receives. **That is a change to a test
helper, not to the backend.**

## Prior failures to avoid repeating

- **The private-region canary had exactly this shape** and was closed last session
  by adding a stream that writes a private slot. The lesson recorded then: a
  canary is not proven by existing, only by being made to fire. **Shrink the
  shared buffer and watch it fire, or do not claim reach.**
- **Four probe self-implications this month**: a float argument passed as
  `i64::MIN`, a float return read as an integer, a hand-named signature taking a
  SIGBUS, and a degenerate single-yield stream handed to the general driver. **The
  general driver asserts `declared + 3` parameters by hand.** Any new shape must
  be a two-yield-or-deeper stream, or the assertion fires and the probe is the
  thing at fault.
- **A guard with a patched constant is not a repair.** If the witness count moves,
  the pinned figure moves with a stated reason.
- **`vm_and_native_two_arg` is the scalar driver**; the native-call witnesses need
  whichever driver can register a native on BOTH sides. Registering on one side
  only produces a comparison between a program and itself.

## The specific wrong turn available here

**Making the shared segment reachable by weakening the reference driver.** If the
reference refuses a shared-slot stream for a reason other than the missing
segment, supplying a segment must not become a way to drive a program the
reference would otherwise reject. **Verify the refusal disappears for the right
reason** — the same shape must fail without the segment and pass with it, and the
yielded sequences must still be compared against the reference rather than merely
produced.

## What success is not

Not "the canary exists". Not "the tests pass". **The shared canary must be shown
to fire when the buffer is too small**, and the two opcodes must have a program
that emits them, runs on both implementations, and agrees — the same standard the
other sixty two meet.
