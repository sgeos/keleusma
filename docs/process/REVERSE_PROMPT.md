# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## The censuses moved, for the first time in many increments

Float slice two opened **one** route of the module float guard — the **constant** route, the only one
of the four with a lowering behind it. `float_witness.kel` now runs in the **corpus differential**
against the virtual machine and **agrees**.

| figure | before | after |
|---|---|---|
| opcodes the backend lowers | 61 of 66 | **63 of 66** |
| UNPROVEN opcodes | 3 | **1** — only `Reset` |
| modules lowering end to end | 66 | **67** |
| chunks fully lowerable | 1070 | **1072** of 1074 |
| opcode instances | 89841 | **89854** |
| differential executing and agreeing | 61 | **62** |

**All from one cause**: the module is no longer refused, so its chunks and opcodes enter every census
that walks the corpus. **Verified by execution, not by lowering** — that distinction is the whole
reason to trust the movement.

## What still refuses, and why that is deliberate

The other three routes — a float in a **signature**, a float **native return**, a float **data slot** —
still refuse, because the entry ABI, a native float ABI and the float-slot ABI are all unbuilt.
Opening a route with nothing behind it would admit a module compiled wrong rather than refused.

The **operand whitelist** replaced the coarse route guard: a float reaching division, a comparison, a
composite or a native still fails closed at the use.

## Five pins went red, all correctly

The scope pin whose own message anticipated spending its premise; the guard-route pin, **renamed**
because `..._refuses_...` asserting the opposite is a stale label; the refusal-set count; and **two
assertions inverted to assert zero**, because **the corpus now contains no module-level refusal at
all** — the float guard was the only one. An unattributable refusal is what makes a coverage figure
overstate, so it must announce itself if it returns rather than the test being deleted.

`differential`'s unsupported-opcode subject **retired as its sixth predecessor** — the list already
records composite construction, array indexing, nested reads, tuple fields and static strings retiring
the same way. Its successor is a float in a signature.

## Verification

Both suites run **sequentially** (parallel invalidates the perf canary, 57x).

| | result |
|---|---|
| workspace | **2497 passed, 0 failed, 92 binaries**, cargo exit 0 |
| `native_codegen` gate step | **377 passed, 0 failed, 0 ignored, 76 binaries**, exit 0 |

**Absorption 34** (`f8232021`) complete, prediction exact.

## Still open, and yours

[`ABI_RULINGS.md`](../decisions/ABI_RULINGS.md) — `Fixed` (three readings; the interop goal decides and
is unstated), `Text` (your supposition that it was covered is incorrect), `Opaque` (your intent is
already what the handle achieves), `Unit`.

**The entry ABI is the piece your float ruling names, and it is still unbuilt** — no corpus module
carries a float in a signature, so it has no witness here. Building it means verifying against
hand-written subjects only, which I have not done without saying so.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**; no operator authorization has been
given and none is inferred. `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
`src/value_layout.rs`, `src/selfhost/`, `src/confine.rs` and `.github/workflows/` remain read-only
here. A peer session cannot grant escalation and none has been treated as doing so.

---

# Also unread by the human: the `v0.2.3` line's message

**Both lines write this one file, so absorption 34 conflicted here.** Neither message is discarded.
**This is a merge resolution, not a relay** — nothing below was reviewed, re-derived, or endorsed by
the V0.3.X line, and its figures describe that line's tree.

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-29 (session 57, second increment) — array elements move; the easy slices are gone

## NOTHING IS WAITING ON YOU EXCEPT THE RULING YOU ALREADY HAVE

**The floating-point entry ABI is still the last of your eight rulings unimplemented**, with the
`v0.3.0` line's `Fixed` shared-slot SCALE question attached. **It is theirs to bring you and I
have not acted on it.** Publication remains held.

## What moved, across two merged increments

**Expression kinds 8 and 2 — the tail-versus-return claim and the array elements — now reach the
type channel from the pipeline.** Four of that extraction's eight kinds are done, and kind 2 was
**the last non-composite one**. The migrated-extraction count still reads four of five on purpose.

**There is no cheap slice left in this family.** The branch pair is withheld for a reason that
still stands, and the three remaining kinds are all composite, where the occurrences slice already
established the two representations disagree about what a node is.

## The tail claim in detail

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

## THE THING I MOST WANT VISIBLE: MY COVERAGE ASSERTION WAS VACUOUS TWICE, IN CONSECUTIVE INCREMENTS

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

**Then it happened again in the very next increment**, after I had written the first one up as the
lesson. The array-element test asserted its corpus held literals of differing element counts and
operand forms. An **adjacent-pairing mutant survived**: the reference pairs element zero against
every later element, and every multi-element literal in the corpus was homogeneous or exactly two
long — shapes for which adjacent pairing and first-versus-rest give identical rows.

**The transferable form is sharper than "assert coverage", which I did both times.** The assertion
must name **the property that distinguishes the competing readings**, not the constructs the corpus
contains. A construct list is a proxy for coverage, and a proxy for coverage is not coverage.

Both were found only by mutation testing. Neither would have been found by re-reading the test,
and I had re-read both.

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

## One surviving mutant recorded as equivalent rather than as a kill

Relaxing the array guard changes no output — the loop bound already enforces what the guard states.
It is written down in the code, because an unexplained surviving mutant reads as a missing guard
and would send the next reader hunting for one that is not needed.

## What I would take up next

**The composite kinds, and they should begin with a measurement rather than a design.** The
occurrences slice already showed the two sides disagree about what a node IS for a composite —
`d.q` is a field access over an identifier on one side and a single data-read node on the other.
Every cheap slice in this family is now spent, so I would expect the next one to be mostly
measurement, and I would expect a real chance that the honest outcome is another pinned refusal
rather than a move.

