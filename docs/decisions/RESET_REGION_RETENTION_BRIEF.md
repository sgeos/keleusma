# BRIEF — the `Op::Reset` region-retention premise, made checkable

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11.

## The premise, quoted from the code it justifies

`src/lib.rs`, the `Op::Reset` arm:

> **The composite region needs no explicit reset**: every site has a fixed offset, so the next
> iteration overwrites exactly the bytes a reset would have reclaimed.

The runtime resets the ephemeral region at `Op::Reset`. This backend does not. The sentence above is
the entire argument that the difference is unobservable, and **nothing in the tree checks it**.

## Why this one and not another

It is the last load-bearing premise in the stream lowering that lives only in a comment. The
handoff's own third carried-forward lesson is that a statement being *correct* is what makes it feel
discharged; seven completion-condition clauses were audited and seven had nothing connecting them to
the tree, two of those hiding live defects. This premise has the same shape as the one that cost the
most: the `Op::GetIndex` comment claiming *"the compiler emits `Op::BoundsCheck` before the index"*,
which was false and returned the caller's buffer filler as a value.

The operator also flagged the distinction explicitly, saying it may matter later. "Later" is not a
plan; a check is.

## What the premise actually rests on — the chain, stated so it can be attacked

The quoted sentence is **not** the real argument, and writing the check will be wrong if it encodes
the sentence rather than the chain:

- **P1 — provenance.** A site's bytes are reachable only through a pointer produced by that site's
  own `NewComposite`. Sites do not share storage (`region_nonreuse.rs`), and no other op manufactures
  a region pointer.
- **P2 — no pointer outlives its iteration.** Locals are cleared at `Reset`; the operand stack goes
  to depth zero; the spill slice is dead once depth is zero; a yielded composite is refused
  (`YIELD_ESCAPE_REFUSAL.md`); persistent storage takes a copy, not an alias.

P1 and P2 together, not the fixed offsets, are what make the retained bytes unobservable. **The
retained bytes are real.** The sentence in the code implies they get overwritten; they do not, when
an iteration takes a branch that does not run that site's constructor.

## The specific wrong turns to avoid

1. **Do not "fix" the lowering.** Emitting a region clear at `Reset` would cost real work per
   iteration to hide a difference that is unreachable. The increment is evidence, not a memset.
2. **Do not assert the premise as written.** A test that checks "the next iteration overwrites the
   site" will FAIL on the interesting shape — the conditional one — and the false conclusion is that
   the lowering is broken. Check P1 and P2 instead, and record that the sentence over-claims.
3. **Do not write a witness whose staleness is assumed.** A zero-filled region cannot distinguish
   *never written* from *written zero*. Poison the buffer, or the leg proves nothing. This is the
   mistake the earlier array-index defect exposed from the other side: `0xab` was informative
   precisely because it was not a plausible value.
4. **Do not infer the region layout.** Site offsets come from `plan_chunk_region`, which the emitter
   itself consults. A second computation of the same offset is free to drift; the hard-coded
   "+3 pointers" in `declared_float_width.rs` already had to be replaced for exactly this reason.
5. **Do not claim agreement about a program the backend refused.** Use the harness that checks
   refusals first. Asserting agreement about a module that never ran is the failure mode the common
   helper exists to prevent.
6. **Do not let the instrument be cast in the shape of one route.** The handoff's second lesson: every
   instrument so far was blind to the class next to the one that prompted it. Enumerate the escape
   routes as a set, and say in the file which ones the check cannot see.

## What "done" looks like

The retained bytes are demonstrated to exist, the yielded sequence is demonstrated to agree anyway,
each escape route names a live closing mechanism with a control that proves the check can fail, and
the code comment stops implying something the tree contradicts.

---

## CLOSURE — 2026-09-11

**The brief's own framing was right about the premise and wrong about where the risk was.**

It said the retention premise was "the last load-bearing premise in the stream lowering that lives
only in a comment". Making it checkable took one test file and confirmed the argument holds. **Both
defects found in the increment came from somewhere else:**

| found by | defect |
|---|---|
| a control subject added so a count could not pass by not moving | the entry preamble zeroed locals on every resume, wiping any local live across a `yield` |
| reading a citation before filing it | a composite data slot stored the body's ADDRESS where the runtime copies its bytes |

**Wrong turn 4 in this brief — "do not infer the region layout" — generalises further than it was
written.** It was about not recomputing an offset the emitter computes. The second defect came from
the same family one level up: a claim about a *mechanism*, sourced from a test that measures
something adjacent. The rule that catches both is: **take the fact from where the system states it,
and check that the place you cited actually states it.**

**Wrong turn 1 — "do not fix the lowering" — held.** No region clear was emitted. The composite slot
was refused rather than copied, for the reason that wrong turn gives: the pool's base is not pinned
against the runtime, and a guessed offset is a wrong answer where a refusal belongs.

### What is now open, and it was not open this morning

**The persistent composite copy.** `private_composite_layout` names the pool offset per slot, and
`persistent_composite_bytes` its size, but the pool's base in the native ABI is not pinned against
the runtime. Implementing it makes `14_frame_log.kel` lower again and removes a corpus refusal. The
discriminator that separates a copy from an alias already exists in `private_slot_composite.rs`, and
the runtime's answer for it is pinned there independently of the backend.

## AUDIT OF THE COMPLETION CONDITION I WROTE — 2026-09-11

Eight clauses. Seven met. **The finding is not in the seven.**

| clause | verdict |
|---|---|
| 1 retained bytes shown with a poison and three states, offset from the planner | met |
| 2 same program agrees with the runtime, refusal-checked | met |
| 3 escape routes enumerated with mechanisms, open ones stated | met — **and one row was WRONG when drafted** |
| 4 a must-fire control on at least one route | met, two |
| 5 the comment no longer over-claims | met |
| 6 no opcode, no `BYTECODE_VERSION`, no root `src/`/`tests/` edit | met |
| 7 green in both float configurations, counts re-derived | met at the stamp |
| 8 blindness written down | met |

### The gap the clauses could not have caught

**Neither defect this increment found is described by any clause.** The condition is about bytes
retained across `Op::Reset`; the first defect is about locals destroyed across a `yield`, one layer
in, and the second about a slot that is not in the region at all. A lowering carrying both defects
satisfies all eight clauses.

**That is the third carried-forward lesson arriving on schedule** — a correct statement, connected to
nothing that would notice its neighbour. The condition was not too weak for its subject; it was
pointed at the subject I had already decided was the risk.

### Clause 1 is weaker than it reads

The three-state discrimination checks the poison against the FIRST word of the body, not the whole
of it. A partial write leaving later bytes poisoned would pass. The bodies here are two words, so
the exposure is one word wide, and it is recorded rather than repaired because the repair is a
one-line strengthening whose absence is now written down.

### Clause 3's wrong row

The persistent-storage row cited `slot_homed_composites.rs` for "copied, not aliased". That test
measures the POPULATION of slot-homed composites. **The mechanism the row asserted did not exist** —
the store was an alias, and the row would have filed a defect as a closed route. The row now states
what is actually true: the route is refused.
