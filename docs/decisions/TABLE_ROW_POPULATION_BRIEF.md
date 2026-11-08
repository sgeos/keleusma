# BRIEF — the guard's population was the rows I had just fixed

## The finding

Last increment built `handoff_figures.rs` to stop the state table drifting, and
its completion condition said:

> *Every figure in that table is either checked against the tree, or carries the
> commit at which it was measured. **No figure in that table is both underivable
> and unattributed.***

**That was not met, and I said it was.** Of six figure rows the guard checks
three and attributes one. The two left unchecked and unattributed are **both stale
right now**:

| row | says | truth |
|---|---|---|
| corpus | 69 modules | **74** |
| unabsorbed | 26 | **46** |

## Why this happened, which is the part worth keeping

The rows I checked — `test files`, `test functions`, `ISA` — are exactly the rows
I had just been correcting. **The guard's population was what I was already
thinking about, not the table.** The stale rows were, by construction, the ones
outside that set: if I had been attending to them they would not have been stale.

This is the session's recurring failure in its sharpest form. A census keyed to
the analyst's attention finds nothing the analyst had not already noticed. **The
population must be the container, enumerated, not the subset that prompted the
work.**

## What to build

1. **Enumerate the table's rows and require a disposition for each.** Every row is
   `checked`, or carries a `measured at` commit, or is explicitly `not a figure`.
   A row with none of those fails. This makes a newly added row fail closed rather
   than joining silently — which is the actual fix, the individual corrections
   being secondary.
2. Check `corpus`, which is derivable: sources built and modules refused.
3. `unabsorbed` says *"at the stamp, and MOVING — re-derive, never quote"*. **A
   number that must not be quoted should not be printed.** Replace the figure with
   the command that derives it. A self-aware hedge beside a wrong number is still
   a wrong number.
4. `absorption` is a process counter with no derivation from the tree. Attribute
   it, as the suite row already is.

## Prior failures to avoid repeating

- **Checking what is in front of you and calling the container covered.** That is
  this increment's whole subject; do not repeat it by enumerating rows but
  hard-coding which ones matter.
- **Deriving a figure approximately so it can be "checked".** The suite row was
  correctly left underivable and attributed instead. `absorption` deserves the
  same treatment, not an invented derivation.
- **Trusting a hedge.** *"Re-derive, never quote"* did not stop the number being
  wrong by twenty; it only made it defensible. Remove the number.
- **Counting the corpus by globbing.** It is the harness's own enumeration, with
  the rtos prelude composed in, that decides how many modules build. Use that, not
  a file count — an earlier increment recorded five scripts as compiler failures
  when the harness was the cause.
