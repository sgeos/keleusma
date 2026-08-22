# BRIEF — two of the six unseeded stages can be seeded with no accessor at all

## Where the gate stands

Twelve stage sources; ten agree. Of those ten, **four are seeded** (`verify_depth`, `verify_typed`,
`verify_structural`, `reconstruct`) and three of the four now run five subjects each. **Six are
unseeded and vary NEITHER axis**: one run, sixty ticks, no input. That is now the weakest part of the
gate, and it is weaker than the part just strengthened.

## The opening, and why it is not blocked

`src/selfhost/mod.rs` exposes only five seed accessors, and it is **read-only to this line** — so the
obvious route to seeding more stages is closed. **It is not the only route.**

`stage_differential.rs` seeds `lexer.kel` and `parse.kel` with a **generic helper** that resolves a
shared-data slot by name from the module's own `data_layout` and writes raw bytes. No accessor, no
change to the other line's files:

- `lexer.kel` takes source bytes into a `len` / `bytes` slot pair at width 1.
- `parse.kel` takes the lexer's own token output into `len` / `packed` at width 8.

**So two of the six unseeded stages are seedable today**, by a technique already proven in this tree.

## State the gain honestly, because the tree has been burned by the opposite

**`lexer.kel` and `parse.kel` ARE ALREADY COMPARED** in `stage_differential.rs`, each against real
input, each with a vacuity control. **This does not add coverage from nothing.** What it adds is:

1. **Multiple subjects.** Both are currently driven at ONE hardcoded source string — the same
   single-subject residual the `verify_*` stages had before the last increment.
2. **Inclusion in the Order-1 gate's seeded accounting**, so the gate's own report stops describing
   them as varying neither axis when a sibling test varies one.

Claiming more than that would repeat the inflation this line has already recorded twice.

## Prior failures and the specific wrong turns to avoid

- **A SEED THAT DOES NOT ARRIVE LOOKS EXACTLY LIKE COVERAGE.** `stage_differential` guards this with
  an explicit vacuity control — the unseeded parser alternates 15 and -1, so two distinct values is
  the vacuous baseline and three is the floor that proves arrival. **Any stage added here needs the
  same kind of floor**, not merely a non-empty result.
- **DO NOT WRITE TO `src/selfhost/mod.rs`.** It is the other line's. The whole point of the generic
  slot route is that it needs nothing from them. If a stage cannot be seeded without an accessor,
  that is a finding to report, not a file to edit.
- **THE SOURCE MUST FIT THE SEGMENT.** The helper asserts it does. A subject too large fails loudly,
  which is correct — do not silently truncate the source to make it fit, since a truncated program
  lexes differently and the comparison would then be of a different thing than it claims.
- **`parse.kel` IS DRIVEN BY THE LEXER'S OUTPUT, not by raw source.** Its subject is therefore
  derived, and a source that lexes to too few tokens drives the parser on nearly nothing.
  `stage_differential` asserts more than fifteen tokens for exactly that reason. **Carry that check
  per subject**, or a small subject will quietly weaken the parser comparison while raising a count.
- **DO NOT claim the six became two.** Widening two of six leaves four unseeded, and `reconstruct`
  still sees one subject. Report the new position, not a finished one.
- **DO NOT declare the gate met.**
- **Watch the runtime.** It went 30 s to 72 s on the last widening. Two more stages times N subjects
  will add more. If the total becomes unreasonable, use fewer subjects and SAY so.

## What a good outcome looks like

`lexer.kel` and `parse.kel` are driven from the corpus at more than one subject each, with a
per-subject floor proving the seed arrived; the gate reports six seeded stages rather than four and
four unseeded rather than six; the report says plainly that both were already compared elsewhere and
what this adds. **And the remaining unseeded stages are named**, so the next increment starts from a
list rather than a count.
