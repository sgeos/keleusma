# BRIEF — do this session's eleven new assertions actually fire?

## The standard this line already holds

*"A clean guard proves its reach first."* A passing check is evidence about the
checker before it is evidence about the tree. This session added **eleven new
test functions across two files** and every one passed. Some of that passing is
established; most of it is not.

**What is already established, by accident rather than design:**

- `the_spill_reservation_is_nearly_all_unused` — its first version claimed the
  block was untouched entirely and **its own assertion refuted that on the first
  run**. Reach demonstrated.
- `the_reservation_share_across_the_corpus_is_pinned` — the first version pinned
  three streaming modules against an actual 28, and **failed**. Reach
  demonstrated.
- `every_streaming_module_reserves_the_same_flat_block` — the share half of its
  earlier form fired. Reach demonstrated for the partition it walks.
- The shared-segment canary — shown to fire when the buffer is shrunk to zero.

**What is NOT established:**

- `the_arena_extent_does_not_grow_with_tick_count` — the central claim of the
  whole memory increment, and **nothing has shown it would notice growth**.
- `the_named_terms_account_for_the_published_figure` — the check that licenses
  every share figure beside it, never seen to fail.
- `the_touched_extent_stays_within_the_planned_arena_bound`, the two instrument
  controls, the fidelity check, and the two native-call witnesses.

## Why this is the right increment rather than more coverage

**The value of a new instrument is entirely in its ability to produce a
finding**, and this session has now twice found that a plausible-looking guard
could not. The private-region canary sat in the file looking like coverage until a
stream that writes a private slot was added; the shared one was documented as
unreachable for weeks. **Adding a twelfth assertion whose reach is unknown is
worth less than establishing the reach of the eleven that exist.**

## Method, and the trap in it

Perturb the SUBJECT, not the assertion. Shrinking a buffer, changing a pinned
constant, or editing the expected value tests the harness; what must be shown is
that a plausible *defect* in the thing measured produces a failure.

**⚠ THE RECORDED TRAP, TWICE PAID FOR.** This line has already logged two
mutations that tested nothing: changing a `BoundsCheck` witness index from `[1][1]`
to `[0][0]` when both emit the opcode, and making stream replies constant when
with no defect present the implementations agree either way. **A mutation that
cannot distinguish the two outcomes is not a weaker test, it is no test**, and
writing it down as reach would be worse than leaving the reach unknown.

Where a property genuinely cannot be mutation-tested — as `stream_depth.rs`
already records of its varying replies — **say so in the file and state why**,
rather than inventing a perturbation that passes for one.

## Specific wrong turns to avoid

- **Leaving a perturbation in the tree.** Every mutation is temporary; the gate
  must be green on the unmutated tree, and a stray edit is the "run whose subject
  was edited while it was in flight" failure this project has already paid for.
- **Claiming reach from a compile error.** A mutation that fails to build has
  demonstrated nothing about the assertion.
- **Reporting eleven of eleven.** If some assertions cannot be made to fire, that
  is the finding. A census that returns a perfect score is the shape this line
  distrusts most.
- **Mutating the reference implementation.** `src/` and `tests/` at the repository
  root belong to the other line and are read-only here. Perturb this package, or
  the test's own subject, and nothing else.
