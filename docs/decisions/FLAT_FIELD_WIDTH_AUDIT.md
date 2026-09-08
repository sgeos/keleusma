# Were those the only ones? An audit of flat-field width sites

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: Complete against a stated method, with a clean result. Written 2026-09-08.

## Why this exists

A composite bearing an opaque field was built and read at two different widths, and the worst symptom
was a **silent wrong answer**: two structures differing in a `Word` field compared equal. The repair
is recorded in [`NARROW_WIDTH_FAILURE_CLASSIFICATION.md`](./NARROW_WIDTH_FAILURE_CLASSIFICATION.md).

**Those sites were found by following failures, not by looking.** Five tests failed at a skewed
width; chasing them reached the code that disagreed. That procedure finds what the test corpus
reaches, and says nothing about a site on a path no test drives.

**For a class that returns wrong values rather than faulting, "did we get them all" is the question
that matters**, and it had not been asked.

## The scope, and why it is finite

`ScalarKind::Opaque` is the **only** kind the address width sizes. Every other kind is a function of
the word width or the float width, so a site assuming a word is correct for them. The audit reduces
to: which sites determine the byte width of an **opaque** field, and does each take that width from
the layout or assume one.

## The method, in three searches

Stating the method is part of the result, because a search whose reach is unstated is the kind of
instrument this tree has already caught reporting clean results about things it never touched.

| # | search | what it is for |
|---|---|---|
| 1 | every line in `src/` mentioning `Opaque`, outside test modules, with its enclosing function | sites that name the kind |
| 2 | every generic offset advance in a flat-composite context (`off += …`) | sites that stride over fields without naming a kind |
| 3 | every width use inside an `impl` for an opaque host type | sites where the type is opaque but the line is not |

**Search 1 alone is insufficient, and that is not hypothetical.** Two of the repaired sites contain
no mention of `Opaque` on the offending line: the host decode reads inside
`impl KeleusmaType for Arc<dyn HostOpaque>`, and the arena packer had already received the value as
an `Int`. Searches 2 and 3 exist because of those two, not by symmetry.

### The method was validated against the tree before the repair

Not asserted — run. Against `802e72d3`, the commit preceding the repair:

- **Search 1** surfaces the flat scalar read: `if matches!(kind, ScalarKind::Opaque)` followed
  immediately by a read taking `word_bytes`.
- **Search 1** surfaces the construction path rewriting `OpaqueRef` to a one-word `Int`.
- **Search 3** surfaces the host decode: `buf[..word_bytes]` inside the opaque impl, whose address
  parameter was named `_addr_bytes`. **The unused-parameter underscore was the tell**: the code
  declared in its own signature that it did not need the address width, for a field the layout sizes
  by the address width.

A method that could not rediscover the known defects would be worthless for finding an unknown one.

## A correction to the account of the defect, which this audit forced

The repair's commit message and pull request describe **four sites, each assuming a word**. Reading
the pre-repair code for this audit shows that is imprecise, and the imprecision matters because it
mis-describes the mechanism.

| pre-repair site | what it actually did |
|---|---|
| the construction path | rewrote `OpaqueRef` to `Int`, **discarding the kind**. This is the defect |
| the field-size query | saw an `Int` and answered with a word. **Correct code, wrong input** |
| the arena packer | derived its stride from the kind it was handed, which was `Int`. **Correct code, wrong input** |
| the flat scalar read | named the kind `Opaque` and read a word anyway. **An independent width assumption** |
| the host decode | read a word inside the opaque impl. **An independent width assumption** |

So there were **two independent width assumptions and one representation collapse**, whose
consequences flowed through two sites that were themselves correct. All five edits were necessary,
because removing the collapse left the packer needing to handle the index form directly — but
"four sites each assuming a word" over-counted by treating consequences as independent causes.

**An explanation is a guess too.** That lesson is already recorded in `HANDOFF.md`, and this is an
instance of it: the repair was right and the story about it was not.

## The result: no further site found

Every site that determines an opaque field's width now takes it from
`ScalarKind::Opaque.size_in_bytes`. The population, by group, summing to the total stated:

| group | count | verdict |
|---|---|---|
| the layout itself | 1 | `Self::Opaque => addr_bytes`, the single authority |
| sites that ask the layout for the width | 5 | the field-size query, the arena packer, the flat scalar read, the host decode, and the construction path which no longer rewrites |
| sites that REFUSE an opaque rather than size it | 3 | the fixed-scalar codec, the host flat-buffer writer, and the shared-data layout builder |
| generic stride sites in flat contexts | 6 | three in the compiler and three in the marshalling tuple macro, all sizing through the layout |
| fixed for a stated reason | 1 | a flat enum body opens with a discriminant **word**, which is an integer and not a reference |
| sites that classify, tag, or carry an opaque without sizing it | 38 | registry interning and resolution, equality by pointer identity, type naming, tag mapping, const rejection |

1 + 5 + 3 + 6 + 1 + 38 = **54**, which is the count search 1 returns.

**No site was found that assumes a width where the layout specifies another.**

## What this does NOT establish

**It is "no further site found", never "no further site exists."** This tree already carries a
retraction on exactly that distinction: `Op::IsStruct` was declared producerless and another line
found four producers within the hour. The recorded rule is *no producer FOUND*.

**What the method cannot see.** A site that computes a flat field offset arithmetically from a
literal rather than from a kind; a site reached only through a helper that takes an already-computed
width; and any site outside `src/`. The wire format is deliberately out of scope, since its record
strides and field widths are fixed by that format and are independent of the target.

**It does not settle what an opaque field IS.** Whether the flat field holds a registry index
(arguing for a word) or a host handle (arguing for an address) remains open, and the repair
deliberately did not need it answered: the runtime follows the layout either way. **This audit does
not promote that into a decision.**
