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

## The fingerprint will NOT move on its own, and this section used to say the opposite

**CORRECTED 2026-09-03, and the correction inverts the advice.** This section previously stated that
the format fingerprint hashes the scalar size table, that changing `ScalarKind::Text` must therefore
move it, and that a fingerprint failing to move would mean the detector is broken.

**That was true of a design the operator replaced.** `FORMAT_FINGERPRINT` in `src/bytecode.rs` is a
hand-rolled random constant, currently `0x4327_63E1`. It derives from nothing. A layout change moves
it exactly never.

The stale text is dangerous rather than merely out of date, because it inverts a diagnostic. A
session performing the `ScalarKind::Text` collapse would look for the fingerprint to move as evidence
the mechanism works, would observe that it did not, and would conclude the detector is broken while
it is behaving exactly as designed. **The correct expectation is the reverse: the value does not
move unless a person moves it.**

Rolling it is a release step, not a consequence of a layout edit. `scripts/fingerprint.sh` reads
this tree's value, reads any commit's or tag's, and rolls a new one; step 1b of the release process
does the rolling. Skipping that step produces no warning and no test failure, only two releases
silently accepting each other's bytecode.

**Why the operator chose a random constant over a derived one is worth keeping**, because the
argument against it looked strong. A hand-written constant fails by being forgotten, which is true.
But a derived value covers only what it hashes, so a release that changed an opcode's MEANING while
leaving every hashed size alone would leave it unmoved while genuinely differing. A value that is
rolled deliberately per release has no such blind spot.

## Where the work actually stands, 2026-09-03

Four increments are merged. **Do not re-plan them; read what they settled.**

| Increment | What it landed | The thing worth knowing |
|---|---|---|
| 1 | The type surface, refused everywhere below | A refusal is not a defect here; each increment removes one |
| 2 | The flat layout | `Tuple([Scalar(Int), Array{Byte, N}])`, sized exactly `word_bytes + N`, no new descriptor variant, no opcode |
| 3 | A distinct nominal type in the checker | `Type::TextN(ConstDim)`; **five** match sites, all in `typecheck.rs`, measured by the compiler |
| 4 | The zero value | A zero length word followed by `N` zero bytes, cross-checked against the layout |

**Three refusals remain, and all three are correct.** One in `check_composite_dimensions`, one in the
whole-program guard, one in data-field validation. They stay until code is generated.

## The next increment is EMISSION, and it is the gate for everything else

Nothing further can proceed without it. In particular **the `ScalarKind::Text` collapse cannot be
done first**: that kind is two words precisely because it must still hold the dynamic case, and its
own comment says the one-address form becomes correct only once `Text<N>` removes that case. The
collapse rides with emission or it does not happen before publication, and after publication it
costs a version the operator has declined to spend.

### EMISSION IS BLOCKED ON AN OPERATOR DECISION, measured 2026-09-03 by a spike

**Do not start emission expecting a mechanical increment.** A throwaway spike removed both
refusals and asked the compiler what happens. The answer:

```
type error: let binding declared as Text<8> but value has type Text
```

**That is increment 3 working exactly as designed, not an obstacle to route around.** A string
literal is STATIC text; `Text<8>` is dynamic text; the two are different types and deliberately do
not unify, which is the operator's ruling. So nothing can enter a `Text<N>` until there is a way to
put it there.

**And the silent path is closed by a language rule, not by taste.**
[`GRAMMAR.md`](../spec/GRAMMAR.md) states: *No implicit type coercion exists. Numeric conversion
requires the `as` keyword.* An implicit literal-to-`Text<N>` conversion at a `let` would contradict
that rule for every reader of the language, not merely for this feature.

So emission needs a surface form -- a cast, a constructor, or a method -- and **which one is
already an open question belonging to the operator**, recorded as open question 2 in
[`TEXT_CAPACITY_TYPE.md`](./TEXT_CAPACITY_TYPE.md). This line should not pick it unilaterally,
because the choice is visible in every program anyone writes with the type and is far more
expensive to change than the layout beneath it.

**What the spike also confirmed, which is the good news:** exactly two match arms had to change to
admit the type, both already known, and no other pass objected. The machinery below the surface is
in place; only the way in is missing.

**What is NOT blocked** and can proceed without the operator: anything that does not require a value
to exist. The refusals stay as they are, and each remains correct.

### What emission must decide, which is not what it must touch

The compiler enumerates what must be TOUCHED the moment a refusal is lifted; that is free
information and should not be estimated in advance. **Estimating it is how three wrong figures got
into this line's records.** What the compiler cannot tell you is what must be DECIDED:

- **How a literal becomes a `Text<N>`.** A string literal is static text. The settled semantics say
  a statically-too-narrow assignment is a compile error and runtime overflow truncates with an
  optional arm following `CheckedArmKind`. So the narrowing check is a compile-time comparison of
  the literal's byte length against `N`, and it is a REFUSAL rather than a truncation, because the
  length is known.
- **Where the bytes live.** The layout is flat and arena-resident. There is no handle and no epoch,
  and **an implementation step that reintroduces either has broken the design rather than extended
  it** -- that is the whole reason the bound is static.
- **Which operations ship first.** The design's position is that text operations reuse the composite
  machinery. If an operation appears to need an opcode, that is a signal to re-read the composite
  path, not to spend one: the instruction set is at 66 and the rad-hard constraint is standing.

### The verification that is already in place, and what it will do

Two guards will fire the moment emission is wrong rather than merely incomplete:

- `no_position_that_can_name_a_type_admits_an_unbuilt_one` enumerates fourteen positions by class
  and asserts every fixture reaches the compiler. **When a refusal is lifted, this test must be
  updated deliberately**, and the update is the place to state which positions are now admitted.
- The zero value is cross-checked against `layout_pass`. If emission changes the flat shape and only
  one of the two follows, that test fails rather than the two drifting silently.

### The trap that is armed and will bite this increment specifically

`Op::Len` was emitted on arrays and refused by the virtual machine. **The trap was disarmed on
2026-09-04**: the compiler has no emission site for the opcode, both former sites folding the length
or failing with a compile error. `Text<N>` will still want a length operation.

**Read [`OP_LEN_ROOT_REPAIR.md`](./OP_LEN_ROOT_REPAIR.md) before adding one**, for the rule rather
than the trap: a length must come from the operand's type where the type has it, and where it does
not the compiler must refuse rather than emit an opcode the runtime rejects. A `Text<N>` length is
NOT a folded constant -- `N` is the capacity and the length is a runtime value in the composite's
first word -- so a length operation on it reads that word and must not reach `Op::Len` at all. That
document also records that its own prediction about how many cases a type-inference fallback would
close was wrong by a factor of six, which is worth reading before trusting any estimate in it.

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
