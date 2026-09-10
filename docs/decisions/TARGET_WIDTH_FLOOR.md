# The target width floor, and the argument that was applied to one width of three

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: closed for the word and address widths. The float width was already closed. What
remains unchecked is named at the end.

## The finding

`Target::validate_against_runtime` checked that the word, address and float widths did not EXCEED
what the runtime implements. Nothing checked the other end. A target could declare a width narrower
than any format the runtime hosts, and it compiled without complaint.

The floor argument was already written down in this tree, twice, and applied to the float width
only.

- `src/target.rs`, in `validate_program_for_target`, refuses `float_bits_log2` below 5 on the
  grounds that such widths "are not formats, and the runtime's implemented-width predicate does not
  admit them, so a target declaring one produces bytecode nothing will run".
- `tests/float_arith_width.rs` records the provenance. The V0.3.0 line observed that its backend
  collapses `float_bits_log2` 0, 1 and 2 to ZERO BYTES. Checking whether the same conflation was
  harmless here showed it was not, and the measurement is in that file: a program declaring
  `float_bits_log2 = 0` with `has_floats = true` compiled, loaded, and returned 3.75, computed in
  `f64` while declaring a zero-bit float.

Neither statement is about floats. `1u32 << bits_log2` divided by eight is zero for every
`bits_log2` below 3, whichever field it names. `Word` is implemented for `i8`, `i16`, `i32` and
`i64`; `Address` for `u8`, `u16`, `u32` and `u64`. Three is the narrowest of either that any
runtime hosts.

## The consequence was measured, not supposed

A target with `addr_bits_log2 = 2` compiles. The layout sizes `ScalarKind::Opaque` by the ADDRESS
width, four bits is zero bytes, and the failure surfaces much later, at run time, as

```text
InvalidBytecode("NewComposite flat operand on non-flat values")
```

That message names neither the width nor the target that produced it. A reader given it would look
for a defect in composite construction, which is the one place there is not one.

## How it was found, which is the uncomfortable part

It was found by a derivation that produced such a target. `tests/composite_width_skew.rs` derives a
skewed target by taking the build's word width and clamping one step below it. **Its floor was 2.**
Under `narrow-word-8` the word is 3, one step below is 2, and the clamp handed back a four-bit
address.

That clamp was written in this session's own narrow-width work, in a file whose subject is
disagreement between the word and address widths, while repairing hard-coded widths elsewhere.
**This is the sixth time in the session that the class under repair has appeared inside the
repair.** It is recorded because the pattern is now frequent enough to be evidence about the work
rather than about any one edit.

## What changed

- `Target::validate_against_runtime` now refuses a word or address width below the narrowest
  implemented one, naming the field. Both floors are taken from the trait impls
  (`<i8 as Word>::BITS_LOG2`, `<u8 as Address>::BITS_LOG2`) rather than written as literals, so
  widening or narrowing either family moves the floor with it.
- The float floor stays in `validate_program_for_target`, because it is conditional on
  `has_floats`, a program-facing capability flag, rather than a property of the width alone. The
  split is deliberate, and it is also how the other two floors came to be missing for as long as
  they were. A reader looking at the width checks does not see the float one beside them, and the
  documentation on each now says so.
- `tests/target_width_floor.rs` pins the check in four parts: the narrowest implemented pair is
  ACCEPTED, so the floor is not a ceiling in disguise; a word and an address below it are each
  refused with the field named; and every width below the floor is separately shown to round to
  zero bytes, which is the mechanism that makes refusal necessary rather than merely tidy.
- `tests/composite_width_skew.rs` takes its clamp floor from `Address` and gains a test asserting
  that the skew it depends on is NOT DEGENERATE.

## The premise guard is the transferable part

Every test in the skew corpus is about a runtime whose word and address widths differ. Both targets
are derived from the build, and **a derivation can collapse**. If the address came out equal to the
word, every test would still pass while exercising nothing. That is the quietest failure available
to a test suite, and no assertion in the file would have reported it.

`the_skew_this_file_depends_on_is_not_degenerate` states the premise. It is expected to FAIL under
`narrow-word-8`, and that failure is the correct report: with the word already at the narrowest
implemented width there is no narrower address to pair it with, so the file has no subject in that
build. **A build the file cannot cover is a different thing from a defect it has found**, and the
message says which one it is.

## Reach, demonstrated

- Removing the two floor checks fails exactly the two refusal tests and leaves the other three
  passing, which is the correct discrimination for a check that only those two depend on.
- The skew corpus was mutation-probed at a narrow build to establish that it still reaches there.
  Two of the four runtime sites that ask the layout for the opaque width were reverted to asking
  for a word. Each failed at the default build, which makes the probe VALID, and each also failed
  under `narrow-word-16`. One of the two failed STRICTLY MORE tests at the narrow width than at the
  default, so that build is not a degraded copy of the default configuration.

The earlier attempt at this probe was invalid, failing at neither width, and it was nearly reported
as evidence that the corpus had gone vacuous. A mutation that fails nowhere measures the mutation.

## What is NOT established

- **Only `narrow-word-16` was probed for reach.** The corpus fails at `narrow-word-8` and at
  `narrow-address-8` for reasons enumerated below, and no reach claim is made for any build other
  than the default and `narrow-word-16`.
- At `narrow-word-8` the corpus loses two tests, both inadmissible by construction rather than
  defective. One expects the value 135, which does not exist in an eight-bit word. The other is the
  degenerate-skew report described above. **The clamp repair removed a third**: before it,
  `every_corpus_shape_is_actually_built_flat` failed there with the `InvalidBytecode` message
  quoted earlier, which was the zero-byte opaque and not a property of the corpus at all.
- At `narrow-address-8` eight of the ten tests fail because both targets the file drives declare a
  SIXTEEN-BIT address, which that build cannot host. This is the same category as the seven
  self-hosted stage sources that declare `require word >= 32`. Making them pass would weaken a
  stated requirement.
- The floor refuses widths below the narrowest implemented one. It does NOT check that the declared
  width corresponds to a type the runtime actually instantiates for THIS module, which the load
  path checks separately. The two checks are not merged and no claim is made that together they are
  exhaustive.
