# BRIEF — consolidate the 2026-09-18 session

**Filed 2026-09-18, against `97156846`, backlog 0, suite 648, five commits unpushed.**

> ⚠ **STATUS, ADDED BY THE ARTIFACT AUDIT 2026-09-18.** A BRIEF DESCRIBES THE TREE AT FILING, BEFORE ITS OWN WORK LANDS. Every gap it states in the present tense was, by construction, still open when written. **LANDED `246ddbc2`.** ⚠ Its count is wrong: it says this session's records overstated the tree *"three times in its own"*. The session-close block it produced lists **four**, and the artifact audit that followed found a **fifth class** — briefs whose gap-claims read as current fact after the gap closed, which is what these status lines exist to fix.


## Why consolidation is the right increment, not another instrument

Eight increments have landed. The handoff's newest NARRATIVE section is still
headed *SESSION CLOSE, 2026-09-17*, while its state table, its pickup list and
five commits have moved past it.

**That is the exact failure this session found five times in other people's
records and three times in its own**: a document describing a tree that no longer
exists. Leaving it because the work is more interesting than the write-up would be
the least defensible version of it, because I have spent the day correcting the
same shape elsewhere.

Five commits sitting unpushed is the second reason. They are gate-verified in both
float configurations; the only thing between them and durability is a push.

## What the session actually established, to be written down accurately

**Increments, all gate-verified in both float configurations with the script run
end to end and its own exit status read:**

1. Absorption 64 — four predictions, all hit.
2. `Float` in the scalar-operator matrix — 28 cells, no defect class.
3. `Float` in the mixed-operand matrix — 42 to 84 cells, all refused.
4. The float yield arm — capability landed, differential switched on.
5. Generated float expression trees — 600 programs.
6. Generated float streams — 240 programs, 1440 yielded values.
7. A float on the spill slice — kind witnessed, width measured as unwitnessed.
8. The float spill width has no witness; a capability gap pinned instead.

**The backend produced no incorrect result under any instrument.** What it
produced was one capability gap found (a float reply cannot be packed into a
composite; the same shape at `Byte` can) and one refusal surface characterised.

## ⚠ The half that must not be softened in the write-up

**Four times this session a record of mine claimed more than the tree supported,
and every one was green when written:**

- an exactness guard that watched results and not operands, letting a phantom
  divergence through under `narrow-float-32`;
- a degenerate-yield test that early-returned on a shape the reference refuses,
  establishing nothing;
- a generated-stream header claiming to exercise the operand spill when all 240
  subjects carried values in locals;
- three subjects labelled width witnesses that do not detect a lost width.

**Plus one inherited premise**: the scope document's claim that admitting the
yield without the reply kind would produce a silent wrong number. It does not —
it refuses loudly or passes the right bits. I repeated that claim in my own brief
without measuring it.

**And one red gate**: a new file was green under every targeted run and turned the
gate red in a census in a different binary. `-E binary(X)` cannot see a census in
binary Y.

## Wrong turns, named

1. **Do not write the session block as a list of wins.** The four overstatements
   and the red gate are the more useful half, and a reader resuming this line
   needs the failure shapes more than the counts.
2. **Do not add a rival "RESUME HERE" section.** This file once carried seven, with
   the one labelled *LAST* above the one labelled *LATEST*. Write into the existing
   newest section.
3. **Do not quote a figure the guards can re-derive without letting them.** Run the
   record guards after editing, not before.
4. **Do not report the push landed on the strength of `pre-push: all checks
   passed`.** That line has accompanied a silently unlanded push five times on this
   project. `ls-remote` is the instrument.
5. **Do not restate a figure in the narrative that the state table owns.** A guard
   fails on exactly that, and it exists because the two drifted apart.
