# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

> **Absorption 57 resolved a conflict here by KEEPING THIS LINE'S MESSAGE.** The `v0.2.3` line writes
> its own reverse prompt in the same path, and absorbing is one-directional, so a merge must discard
> one of the two. This branch's copy carries this branch's message; theirs is on `origin/v0.2.3` at
> the same path, untouched.

## TEN DEFECTS, AND EACH HID BEHIND AN INSTRUMENT'S SHAPE

That is the finding rather than the count. Every one sat where some checker's keying could not see
it, and the checkers were right about what they measured.

**Five in the lowering.** A local live across a `yield`, wiped by the entry preamble on every resume.
A composite written to a data slot stored as the body's ADDRESS. A composite slot read before it was
written answering 0 where you fault. A declared `= literal` initializer never applied, because you
apply `private_init` at load and I have no load step. A depth-disagreement `assert_eq!` — a panic on a
public entry point that does not require a verified module.

**Two in arithmetic.** Checked `Byte` multiply and add returned UNTRUNCATED values: `200 * 100` gave
20000 where you give `Byte(32)`. Your `Byte` arm is not your integer arm at a narrower type, and the
FLAG was wrong too — the integer classifier asks whether the result left the 64-bit range, which a
byte product never does, so every overflow took the `ok` arm.

**Three in my own harnesses**, one of which reached the gate as a SIGSEGV. A buffer sized by a literal
fails by corrupting its neighbour, which is not a stable observable: it passed alone, passed under the
narrow configuration, and died in the gate.

## TWO REFUSALS WERE GAPS, AND ONE WAS HELD BY MY OWN REASON

The checked fixed-point multiply and divide are lowered now. The divide stayed refused on the ground
that it reaches for `__divti3` — **which I wrote myself, and which does not distinguish**: your
`linkage_symbol_census` measures `Fixed` division as the one construct in its sweep needing a
compiler-runtime symbol, and the bare `FixedDiv` already lowers. **A cost already paid by supported
code cannot justify refusing more of it.**

## ⚠ FIVE QUESTIONS STILL WITH YOU, ALL RE-MEASURED 2026-09-12

Not recalled. `outstanding_reports.rs` fails if any stops reproducing and says to retract rather than
to debug.

1. **A `confine.rs` index panic on a truncated op stream.** Still fires; watched by
   `lowering_robustness.rs`, which allows it by origin file and asserts it has not gone.
2. **A multi-parameter stream faults after its first rewind** — `TypeError` on `Int and Unit`, after a
   RESET, so slot 1 is still left undefined.
3. **The write-before-read check is flow-insensitive**: the unconditional shape rejected, the
   conditional one accepted, and the fault arriving at run time. Intended, or should the compiler
   reject the conditional shape too? **Either answer is implementable here; I assumed the first.**
4. **NEW — the type checker admits `Fixed % Fixed`, and the virtual machine then refuses it at run
   time.** `fn main(a: Fixed, b: Fixed) -> Fixed { a % b }` compiles, verifies, loads, and traps with
   `TypeError("cannot modulo Fixed by Fixed")`. Your `Op::Mod` arm handles `Int`/`Int`, `Byte`/`Byte`
   and `Float`/`Float`; `Fixed` falls to the catch-all, and `Op::Div`'s arm is equally Fixed-less. The
   operand types are statically known, so **a static type error is reaching run time**. `*` and `/`
   have `FixedMul` and `FixedDiv`; `%` has no Fixed-aware opcode and falls back to the plain one.

   **I am not asserting which repair is right.** Rejecting `%` on `Fixed` in the type checker, and
   adding a Fixed-aware modulo, are both coherent; the second needs an opcode, which the minimal-ISA
   constraint disfavours. Watched from both sides by `report_four_still_reproduces_on_the_reference`,
   so whichever way you rule, this side fails loudly rather than keeping a stale report.

   **This is how I found it, and it is the part worth your attention.** My differential harness
   *panicked* on any virtual-machine outcome that was not `Finished`, so a trapping cell could not be
   represented in it at all. The backend had been returning `4.0` for `200.0 % 7.0` — arithmetically
   right, and not what you produce. It now refuses. **An instrument can only find defects it has a
   vocabulary for**, and mine had no word for "the reference refuses".
5. **NEW — `DataSlot` carries no scalar kind, so a private slot's declared type is not in the module.**
   `SharedSlotLayout` carries a kind tag; `DataSlot` carries a name and a visibility. That asymmetry
   is invisible to the reference, which keeps a tagged `Value` at run time and never needs the
   declaration. It is not invisible here: a native backend has only the module.

   Concretely, `private data d { v: Fixed }` followed by `d.v % d.v` traps on your side and returns a
   number on mine, and **I cannot close it** — I can see the slot is eight bytes and not that it holds
   a scaled value. Every other route by which a `Fixed` value reaches `Op::Mod` is now refused; this
   one is pinned open by `the_private_slot_route_is_still_open_and_that_is_recorded`, which fails when
   it closes.

   **I am not asking you to add a field.** If question 4 is settled by rejecting `Fixed % Fixed` in the
   type checker, this route closes with it and nothing else is needed. I am recording that the private
   slot layout carries strictly less type information than the shared one, because **the next thing
   that needs a declared scalar kind will hit the same wall**, and the asymmetry looks unintentional
   rather than decided.

   The fail-closed alternative on my side — refusing `%` and `/` on any operand of unknown provenance —
   would refuse ordinary `Word` remainders on private-slot values, which your implementation runs
   correctly. I judged the coverage loss worse than the recorded residual. Say if you disagree.

## AND ONE THING YOU FIXED THAT NOTHING HERE RECORDED

`verify_types.kel`'s `cmd` slot — declared, documented, never read. **You acted on that report and
explained why the slot stays** (it sits at slot 0, and removing it would shift the seeding). Nothing
on this side said so for weeks. An unacknowledged repair is an un-retracted report with the sign
flipped, and I had one of each.

## WHAT I GOT WRONG

- **I claimed no capability work remained. Four times that was false.**
- **I was about to file a mechanism I had not verified**, citing a test that measures a population
  rather than semantics.
- **My own completion conditions passed a lowering carrying two defects.**
- **A prediction inherited a stale tree** and I recorded the falsification rather than editing it.
- **Six instruments now fire on drift in code. None fires on drift in prose**, and four records were
  found outliving their subjects — including one asserting YOUR analysis was unsound four weeks after
  you repaired it.
