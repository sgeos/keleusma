# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## Your ABI rulings are recorded, and measuring one of them changed the plan

**Settled**: float = Option A (which also settles the `Float` shared slot), string = Option B. The
string one **cannot be implemented by this line** — it changes marshalling in `src/`, owned by the
`v0.2.3` line.

**Measuring the float ruling before building to it was worth the one test it cost.** You named the
**entry ABI**. The corpus's only float-carrying module is blocked by a float **constant**, and **no
corpus module has a float in a signature at all** — so the entry-ABI change has **zero corpus
witnesses** and, built alone, could not be verified against the corpus. Option A as you wrote it covers
both, so the ruling is not wrong; but planning from the phrase "entry ABI" builds the wrong piece
first. Gain when built: **66 → 67 modules**, plus the two conversion opcodes the census lists as
UNPROVEN.

## Two things in the record are mine, not yours, and are labelled so

- **Float width.** `Float` is `f32` **or** `f64` under `narrow-float-32`, so "double" is incoherent in
  a build with no `f64`. I am proceeding on **the FP type matches the runtime's float width**.
- **`Unit`.** You asked what it is — a question, not a ruling. It is the empty type, **0 bytes**; a
  zero-byte slot conveys nothing. My inference is a permanent refusal.

## Three still need you

- **`Fixed`** — three readings, and one of them (a distinct slot tag per `N`) contradicts your own
  *"without needing to store"*. Your phrasing points at the reading that **is** the recorded Option B,
  but you framed it as distinct from the listed options, so I have not assumed it. **What you describe
  is already exactly how `Fixed` works in-module**; the open question is only at the host boundary,
  where the compiler cannot bake anything into a separately compiled host. **The interop goal —
  convention-based or self-describing — still governs and is still unstated.**
- **`Text`** — your supposition that it was covered is **incorrect**, and I have preserved that in the
  record rather than silently correcting it. The string ruling settles static literals; the `Text` slot
  is a two-word handle.
- **`Opaque`** — your stated intent is **already what the existing handle achieves**. A literal raw
  pointer would not fit under `narrow-word-8` or `-16`, where a word is 1–2 bytes and a pointer is 8.

**Nothing was implemented on an ambiguous ruling.**

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2491 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **373 passed, 0 failed, 0 ignored, 75 binaries**, exit 0 |
| censuses | 61 of 66; `["Len"]`; 1070 of 1074; 89841 of 89940 — all unmoved |

The native gate first aborted in 1s on `cargo fmt --check` for a new file; that pass is not reported as
a result, and the figures above are from the re-run.

**No absorption was needed**: already zero unabsorbed.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**; no operator authorization has been
given and none is inferred. `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
`src/value_layout.rs`, `src/selfhost/`, `src/confine.rs` and `.github/workflows/` remain read-only
here. A peer session cannot grant escalation and none has been treated as doing so.

---

# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 33 conflicted here.** Neither message is discarded:
the V0.3.X account is above, and the `v0.2.3` line's own account follows verbatim. **This is a merge
resolution, not a relay** — nothing below was reviewed, re-derived, or endorsed by the V0.3.X line, and
its figures describe that line's tree rather than this one's.

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-29 (session 57, first increment) — the tail-versus-return claim moves

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I
have not acted on it.** Publication remains held.

## What moved

Expression kind 8 — the tail-versus-return claim — now reaches the type channel from the
pipeline, joining the binary operator and the condition. Three of that extraction's eight kinds
are done. The migrated-extraction count still reads four of five on purpose; naming a partial
migration after the extraction would defeat the pin silently.

This is the row that refuses a function whose body yields something its signature does not
promise. Both halves were already on the wire, so no stage changed and no record was added.

## The hazard that killed the branch pair was present here, and it was discharged

Kind 8 is an equality kind, so a row emitted where the reference emits none could make the stage
**reject a correct program**. A body with no tail expression reconstructs with a **synthesised
payload-0 unit**, which is the same shape as the synthesised else arm that made the branch pair
unshippable.

What separates them is measurable rather than argued: the only source expression that would also
land on a payload-0 unit is a written `()`, and the pipeline refuses that outright. **I pinned
the refusal in the failing direction**, so if `()` ever becomes admissible the test breaks rather
than the descent quietly going wrong.

## THE THING I MOST WANT VISIBLE: MY OWN COVERAGE ASSERTION ASSERTED NOTHING

The new agreement test asserted that its corpus contained three distinct statement forms before a
tail — the discipline this family adopted after an earlier slice shipped blind to three of four
forest kinds.

**It was vacuous, and only mutation testing showed it.** Removing two of the six continuation
kinds from the descent left the entire suite **green**. Those two corpus cases ended in a data
read, which neither side can type; stopping the descent early lands on a node that is also
untypable, so both readings produced the identical unknown row.

The corpus now ends those cases in a literal and the assertion demands a **typable** tail. All six
continuation kinds fire under mutation, each mutant confirmed to compile before its result was
believed.

I am recording this prominently because it is the sixth-plus instance of one defect — a check
built from the same model as the thing it checks — and this time it appeared **inside the guard
written specifically to prevent that defect**.

## A doc in the same file was claiming a row that was deliberately not emitted

The condition agreement test's heading read "the condition **and branch-pair** rows agree", with a
section describing a branch's statement chain, while the test compares the condition kind alone.
The prose was written while the branch pair was still expected to ship and survived the decision
to withhold it. Corrected in place, with the history left visible.

## A second gap found by asking what else the reference calls a function

**A multiheaded function contributes no tail row at all.** The reference walks each head as its
own function with its own tail, so a three-headed `f` gives three rows; the pipeline reconstructs
the whole group into one fused body and can offer at most one.

I suppressed the group's row rather than emit it. The fused root is a dispatch structure that
typed as unknown on every program I measured — and "unknown on the programs I tried" is not the
property required. If a fused root ever types to a tag, that tag is not one any particular head
promises, and this row feeds an equality predicate. **Emitting nothing costs a check; emitting
the wrong thing costs a valid program.**

The loss is pinned in both directions by `a_multiheaded_function_contributes_no_tail_row`, and
the agreement test's doc says its corpus is single-headed, because a cap that is not written down
reads as coverage.

## One gap named rather than closed

The pipeline's type-name-to-tag table has no `Float` arm where the reference's does. The
direction is the safe one — an unmapped type reports the type channel's unknown, and unknown
accepts — so it costs a check and cannot cause a rejection. Float arithmetic diverges at the
construct-support boundary anyway. It is now named in the tree instead of being left for a reader
to guess about.

## Three questions that remain yours

**One. The floating-point entry ABI**, as above.

**Two. Should a shipped example demonstrate `Byte`?** None of the fifteen does, and it would close
three of the four op tags no corpus reaches.

**Three. Should `01_arithmetic.kel` be enriched?** I corrected its index downward, which is the
conservative direction; enriching the example is the other.

And the two-pass parser work that would make the twelfth stage self-compile remains **yours to
call**. I have not started it.

## What I would take up next

The remaining five kinds. Array elements is the only non-composite one left; the other four are
the branch pair, which is pinned as withheld for a reason that still stands, and the three
composite kinds where the two representations are already known to disagree about what a node is.

