# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-09-08 (session 64) — a wrong answer found in the flat-composite core, repaired, and guarded in a build continuous integration already runs

## READ FIRST: THE RUNTIME RETURNED A WRONG ANSWER, AND IT IS REPAIRED

A composite bearing an **opaque** field was built and read at two different widths. The worst
outcome was not a fault:

```
P { h: h, n: 1 } == P { h: h, n: 2 }   evaluated to true
```

Two structures differing in a `Word` field compared **equal**. Nothing in that program is
unrepresentable at the width it ran at. **A silent wrong answer is worse than any refusal this tree
has catalogued**, and it is why this outranks everything else here.

**The mechanism was one field with two authorities.** `ScalarKind::Opaque` is sized by the ADDRESS
width, and the compiler bakes every field offset from that layout, the typed verifier sizes operands
from it, and the marshalling layer reports field sizes from it. The runtime disagreed in four places,
each assuming a WORD: the construction path rewrote the registry index to a one-word `Int`, the arena
packer advanced by that width, the flat scalar read read it back as a word, and the host decode asked
for a word's worth of bytes from a field the layout had already sized.

**The default target makes the two widths equal**, so all four agreed with the layout by coincidence.

**The repair did not have to choose a side, and deliberately did not.** Whether the field is a
registry INDEX (arguing for a word) or a host HANDLE (arguing for an address) is a real question. It
is left open. Three of the four subsystems already treat the layout as the authority, so each runtime
site now asks the layout for the width. If that question is ever settled differently, the runtime
follows without further change. **Do not promote it into a decision on my account.**

Repaired in #376, guarded in #378, recorded in
[`NARROW_WIDTH_FAILURE_CLASSIFICATION.md`](../decisions/NARROW_WIDTH_FAILURE_CLASSIFICATION.md).

## THE FINDING WITH THE LONGEST REACH: A WIDTH SKEW NEEDS NO FEATURE

I wrote in a merged document that the construction path **could not** be guarded without a
continuous-integration job in a configuration nothing builds. **That was wrong**, and finding out why
is worth more than the repair.

`GenericVm<W, A, F>` is generic over the word and address types independently, and every `Word`
(`i8`, `i16`, `i32`, `i64`) and every `Address` (`u8`, `u16`, `u32`, `u64`) is implemented
unconditionally. **A host-defined alias reaches any pair of widths in the default build.** The
`narrow-*` features only choose which pair the bundled `Vm` alias uses. A skewed pair also already
ships as a named target: `Target::embedded_8` is an eight-bit word with a sixteen-bit address, and
the `addr_bits_log2` field's own documentation names the 6502.

**So a width-dependent defect can be guarded at no standing per-push cost.** That is cheaper than the
job [`FEATURE_COMBINATION_SWEEP.md`](../decisions/FEATURE_COMBINATION_SWEEP.md) recommends and does
not adopt, and it was available the whole time.

**It does not extend to the feature axis.** A build omitting `floats` cannot be reached by an alias;
that is why the float hole below stayed unexercised. The sweep's central finding stands for features
and is qualified for widths.

## WHAT THE CLASSIFICATION SETTLED, AND WHAT IT DID NOT

Every failure of a finished `narrow-word-16` run now carries a verdict taken from the failing
assertion's own text rather than from the test's name. The previous grouping was by name and left
eighteen in an unexamined remainder; **its "opaque and flat-composite layout" row had guessed close
to the real mechanism and still filed it as a test premise.**

| | |
|---|---|
| distinct failures, before | 41 across 16 binaries |
| distinct failures, now | **33 across 11 binaries**, of 106 |
| repaired | 5 opaque-width, 3 hard-coded eight-byte harness reads |
| newly failing | **none** |

**The last figure was established by DIFFING the failing sets, not by comparing totals.** A total
falling by eight is equally consistent with fixing nine and breaking one.

The remaining 33 are seven groups whose tests declare a premise a sixteen-bit word voids: 14 + 6 + 8
+ 2 + 1 + 1 + 1. **The narrow widths are still not verified.** A suite that cannot run at a width
cannot vouch for it, and making it run there remains a project nobody has adopted.

## THE NEGATIVES, WHICH ARE PART OF THE RESULT

**No mutation is caught by the shape corpus alone.** I extended the skew guard to four composite
shapes, then shortened a nested child's stride by one byte specifically to demonstrate unique value.
**Seventeen tests across the suite caught it** — a broken nested stride is wrong at every width. The
corpus's case is that it drives the repaired sites through four construction paths, which is a
hypothesis about defects not yet found, not coverage of anything today.

**Two of the four repaired sites hang on a single test each**, and one test in that file is caught by
no mutation at all. All three facts are written beside the tests rather than left for a reader to
discover.

**My first attempt to measure the corpus used cargo's default fail-fast**, stopped after the first
failing binary, and showed only the corpus failing — which would have supported the opposite
conclusion. It was a lower bound, not a result. The tree already records this trap for the build
phase; it applies to the test phase too.

## THE FOUR DECISIONS ARE STILL YOURS, AND NONE MOVED

Stated in full at the top of [`HANDOFF.md`](./HANDOFF.md). Nothing in this session implements any of
them.

1. **How does a value enter a `Text<N>`?** It appears in every program anyone writes with the type.
2. **Is the width bundle worth a breaking change?** 33 signatures, 14 public, in a published crate.
3. **Should `verify()` refuse float opcodes when the feature is absent?** Evidence complete: about
   ten lines, prototyped, **zero new failures**, and the one semantic worry is moot. Unlanded only
   because I said it was your call in a merged document.
4. **Does any build configuration earn a continuous-integration job?** Note that finding 2 above
   makes this cheaper than it looked for the width axis, and unchanged for the feature axis.

## THE HOLE THAT IS STILL PINNED OPEN

A float-using module verifies, loads, and traps `InvalidBytecode` on a runtime built without the
`floats` feature — the class `verify()` exists to exclude. Pinned by
`tests/float_opcode_without_floats.rs`. **Unchanged this session**, and it is decision 3 above.

## THE INTENDED NEXT STEP

Nothing large without an answer to the four. The self-directed work with the clearest value left is
the remaining narrow-width groups, which are premise repairs and therefore a decision about whether
the suite should run at a narrow width at all — **that decision is not mine and I have not taken
it.** The two censuses (`InvalidBytecode` at 34 of 46 sites, discard arms at 17 of 19) remain open;
my own assessment is that their remainders are low value, and that assessment is recorded in the
documents rather than acted on silently.
