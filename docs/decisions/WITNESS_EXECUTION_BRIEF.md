# BRIEF — make the witness file EXECUTE instead of only lowering

## The goal, and why it is the sharpest thing available

The previous increment took backend support to 60 of 66 by lowering `Add`, `Sub`, `Mul` and `Neg`
for a matched `Byte` pair. **The corpus executes none of it.** `opcode_witness.kel` still refuses on
`FixedDiv`, `Len` and `IsStruct`, so the differential exempts the whole file and never drives
`byte_mix`. The verification is four hand-written boundary cases — real, mutation-checked, and
narrow.

**A lowering figure is not a correctness figure**, and that distinction is what this whole line keeps
turning on. Closing it needs no operator and no new mechanism: **the same corpus split that fixed the
float regression one increment ago.**

## The move

`opcode_witness.kel` mixes opcodes that lower with opcodes that refuse. One refusal exempts the
whole module, so every lowering opcode in it is verified by nothing. Split by outcome:

| stays | leaves |
|---|---|
| `byte_mix` — `Add`/`Sub`/`Mul`/`Neg` | `fixed_mix` — `FixedDiv` |
| `grid_at` — `BoundsCheck` | `len_witness` — `Len` |
| `checked_ratio` — `CheckedDiv`/`CheckedMod` | `is_struct_witness` — `IsStruct` |
| `host::tick` — `CallExternalNative` | |

Every opcode keeps a witness, so the coverage census stays at 66 of 66. What changes is that the
lowering half becomes **runnable**, and the differential drives it with its argument vectors.

## The part that is easy to get wrong

**Seeding.** `byte_mix` currently takes constants — `byte_mix(3 as Byte, 5 as Byte)`. Making the
file executable while its arithmetic is driven by literals produces **twenty-four comparisons of one
constant against itself**, which is precisely the inflation this line already recorded: half the
"executed and agreeing" count agreed on a single value, and twelve modules received twenty-four
vectors and never moved. **Drive the arithmetic from the entry argument**, or the execution claim is
theatre.

## Prior failures and specific wrong turns

- **Do not weaken a refusal to make a module executable.** The float half was split out rather than
  the guard relaxed, one increment ago. Same rule here.
- **A pinned assertion WILL fire and that is the design.**
  `the_witness_module_cannot_be_given_an_arena` asserts `opcode_witness.kel` has no arena bound,
  which is true only because `len_witness` is in it. Moving that function out makes the assertion
  false. **It fires as NEWS, exactly as its message says — update the claim, do not delete it.**
- **Check the exempt classification actually changes.** If the file leaves the exempt set, the
  differential's counts move. If they do not move, the split did not do what it claims and the
  execution is still not happening.
- **Byte values wrap at 255.** An argument vector supplies a `Word`; casting into `Byte` truncates.
  A comparison that only ever feeds small values exercises the mask no better than a constant does.
- **Do not add a second corpus walker or a parallel figure.** Every count here comes from the
  existing censuses and differential.

## What a good outcome looks like

`byte_mix` is driven by the differential with varying inputs, agreeing with the virtual machine;
every opcode still has a witness; the refusing opcodes are still refused; and the handoff no longer
has to carry the qualification that the arithmetic is verified only by hand-written cases.

**If the split does not move the executed count, say so** — that would mean the file is exempt for a
reason other than the one assumed, and the assumption is the finding.
