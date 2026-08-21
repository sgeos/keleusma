# BRIEF — a decline of MINE, made this session on a claim I never checked

## What this is

Two increments ago I declined to lower `Fixed` arithmetic, and wrote the reason into the handoff,
the commit message and a test:

> *"Widening `width_of_tag` would also newly admit composites carrying `Fixed` fields, a packing
> change with its own risk that does not belong bundled with arithmetic."*

**The coupling is real. The risk was asserted and never measured.** Measured now:

| declared type | reference packs |
|---|---|
| `struct { a: Word, b: Word }` | `byte_size: 16` |
| `struct { a: Fixed<16>, b: Fixed<16> }` | **`byte_size: 16`** |
| `struct { a: Byte, b: Byte }` | `byte_size: 2` |

**A `Fixed` field packs at eight bytes, exactly like a `Word`.** So `width_of_tag(Fixed) =
Scalar(8)` is not a guess — it is what the reference already does. The "packing risk" I named was
the risk of guessing, and there is nothing to guess.

## How the wrong exclusion survived: a bundled assertion

`a_composite_tag_has_no_known_width` asserts three things:

```
width_of_tag(Composite) == Unknown
width_of_tag(Float)     == Unknown
width_of_tag(Fixed)     == Unknown
```

Its doc comment justifies exactly one of them — *"a composite parameter's body length is not carried
on its type tag"*. That is true of `Composite` and says nothing about the other two. `Float` has a
good reason living elsewhere (no representation at all). **`Fixed` has no stated reason anywhere.**

**Three members, one rationale, and it covers one member.** That is the same shape as the two
records disagreeing about `Op::Add`, and as the float guard's four-route enumeration where one route
was closed somewhere else entirely. A bundle inherits the credibility of its best-justified member.

## What changes, and what does NOT

Widening the tag unlocks two things at once, and they should be reported separately:

1. **`Fixed` arithmetic** — `Add`, `Sub`, `Neg` on a matched `Fixed` pair. The arm already handles
   `Scalar(8)`; it only ever saw `Unknown`.
2. **Composites carrying `Fixed` fields** — previously refused with "operand of unknown packed
   width".

**The opcode census will NOT move.** `Add`, `Sub` and `Neg` already count as lowered via `Byte`, so
60 of 66 stays 60 of 66. **Say that plainly**: this widens what lowers without moving the headline,
and quoting a headline that did not change as evidence of a change would be its own small
dishonesty.

## Prior failures and specific wrong turns

- **`Fixed` must NOT be masked.** The arithmetic arm masks only `Scalar(1)`. Masking a `Fixed` would
  truncate it to eight bits while every downstream field offset still looked right.
- **Do not change the bundled assertion into a weaker one.** Split it so each tag carries its own
  stated reason. An assertion that stops testing `Composite` to let `Fixed` through would be worse
  than the bug.
- **`Float` stays `Unknown`.** It is excluded by the module guard, not by its width, but leaving the
  width unknown is a second line of defence and costs nothing.
- **Verify by EXECUTION.** The differential drives real composites; a corpus module that newly
  lowers must also newly agree, or the widening bought a refusal turning into a wrong answer.
- **Measure the DELTA, not just the end state.** How many chunks or modules newly lower is the
  honest figure for this change, and it is not the opcode count.

## What a good outcome looks like

`Fixed` arithmetic and `Fixed`-carrying composites lower and agree with the virtual machine; the
bundled assertion is split so each tag states its own reason; and the report says plainly that the
opcode headline did not move.

**If a newly-lowered module disagrees, revert the widening and report the disagreement** — that
would mean eight bytes is right for the declaration and wrong for something else, which is worth
more than the coverage.
