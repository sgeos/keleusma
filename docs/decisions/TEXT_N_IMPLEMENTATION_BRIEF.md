# Brief: implement `Text<N>`

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

The design is settled in [`TEXT_CAPACITY_TYPE.md`](./TEXT_CAPACITY_TYPE.md). This brief covers only
how to build it and what will go wrong.

## First, a defect in the design record itself

`TEXT_CAPACITY_TYPE.md` contradicts itself. Its **Open questions** section lists the overflow rule
as undecided and belonging to the operator. Its later **SETTLED SEMANTICS** section settles it: a
statically-too-narrow assignment is a compile error, and runtime overflow truncates by default with
an optional arm following `CheckedArmKind`.

The settled section is newer and records a direct in-session ruling, so it wins. **Fix the stale
open question before building**, because a resuming session that reads the earlier list will believe
the central semantic choice is still open and may re-ask a question the operator has answered.

Open questions 2 and 3, the bundled-operation reversal and the surface syntax, remain genuinely open.

## What is being built

Two distinct types, not one family.

- **Static text** stays a pointer into `.rodata` with no capacity in its type. A literal is static.
- **Dynamic text** is `Text<N>`, a flat composite of a length word followed by `N` content bytes.
  `N` counts content bytes with no terminator, and B40 erases it to a literal at monomorphization,
  so the capacity is never stored.

The governing analogy is the operator's: `Text<N>` is `for .. limit <const>` applied to storage
instead of iteration. A runtime length under a static cap.

## Why flat, restated because it is the load-bearing constraint

An earlier draft made `Text<N>` an arena handle. That was wrong and the correction is the whole
reason the design works: a handle implies unbounded lifetime, which is why it needs an epoch, which
puts worst-case memory beyond static reach — contradicting the ecosystem's entire value proposition.

Flat gives four properties for free: worst-case memory static by construction, no epoch and no
staleness, no cross-yield prohibition since there is no pointer to reclaim, and no new shared-slot
kind because the composite machinery already carries bodies.

**If an implementation step reintroduces a pointer or a handle into `Text<N>`, it has broken the
design, not extended it.**

## Sequencing that costs real money if got wrong

`ScalarKind::Text` is currently `2 * word_bytes`, sized to hold either a rodata offset-and-length
pair or an arena handle-and-epoch pair. Once static text is a plain `.rodata` pointer and dynamic
text is a composite, that kind collapses to **one address**.

That collapse is a wire-format change. `BYTECODE_VERSION` is frozen at 2 by operator policy and
nothing has been published at 2, so it is **free before publication and impossible afterwards
without a version the operator has declined to spend**. The release plan puts publication right
after this work. Land the collapse with `Text<N>`, not after it.

## The fingerprint will move, and that is the mechanism working

The format fingerprint hashes the scalar size table. Changing `ScalarKind::Text` from two words to
one address **must** move it. Expect the pinned value to change and update it deliberately.

Do not treat the moving fingerprint as breakage. The one outcome that would be a real defect is the
fingerprint **failing** to move, which would mean the detector is broken.

## Specific wrong turns to avoid

**Do not let `Text<N>` and static text share one `ScalarKind`.** They have different sizes and
different residence rules. Collapsing them back into one kind to avoid touching the layout table is
how the current two-word compromise arose in the first place.

**Do not add an opcode.** The rad-hard minimal-ISA constraint is a standing high-priority design
rule and the instruction set is at 66. The design's own position is that text operations reuse the
composite machinery. If an operation seems to need an opcode, that is a signal to re-read the
composite path, not to spend one.

**Do not implement the overflow arm as an exception.** It follows `CheckedArmKind`, which already
exists for checked arithmetic. Reusing that shape keeps every operation total; inventing a second
mechanism for the same problem does not.

**Do not check three feature sets and call it verified.** This session produced that error four
times, most recently with a test that compiled under the three sets I chose and failed the gate's
no-default-features step, which was a compile failure and therefore invisible to a grep for failing
tests. `lexer`, `parser` and `compiler` are gated behind `compile`. The gate runs seven
configurations; for work touching feature-gated modules, the gate is the instrument.

**Do not trust a green self-host run that was not re-run.** The byte-identity comparisons are
excluded from the routine push tier. Changing the layout table will move the emitted bytes, and the
Keleusma emitter must be seeded from the same source as the reference rather than a copied literal,
or the two drift silently.
