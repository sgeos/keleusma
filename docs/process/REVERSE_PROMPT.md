# Reverse Prompt

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

## The float scalar surface is complete

`Neg` and `Mod` land, finishing scalar float arithmetic: constants, both conversions,
`Add`/`Sub`/`Mul`/`Div`/`Mod`/`Neg`, and all six comparisons — each verified by running the same
program on the reference and on the lowered code.

**Two semantics that would have been wrong if assumed:**

- **`Mod` is the TRUNCATED remainder**, carrying the sign of the dividend — Rust's `%` on `f64`, so
  `frem`, not a floored remainder. `-7.0 % 2.0` is `-1.0`, not `+1.0`. **A probe with only positive
  operands cannot tell the conventions apart**, so the differential uses negative dividends with a
  must-fire control requiring the positive and negative results to have opposite signs.
- **`Neg` needed its own branch.** The existing arm dispatches on WIDTH, and a float is eight bytes
  like a `Fixed` — without a kind check it would have negated the **bit pattern as an integer**,
  flipping a mantissa bit rather than the sign.

## The entry ABI is still not built, and here is the measured reason

It is the piece your ruling names, so I want the reason on the record rather than implied:
**`lower_chunk` receives `chunk.param_types`, but the chunk carries no RETURN type.** The return lives
in module-level `ChunkSignature`, which a single-chunk lowering never sees.

So it cannot be done by halves — parameter types, return type, the prologue's bitcasts, `Op::Return`
and `Op::Call` all have to land together, across both entry points. **That is a scoped plan, not a
slice to fit beside an absorption**, and I would rather say so than half-build it.

The signature route of the guard therefore stays closed, and is now the unsupported-opcode subject —
the fourth in that succession, after composites, division and remainder.

## Still absent, so the surface is not read as finished

The entry ABI; **float shared slots**, which is one of your open ABI questions; `f32`, since only the
8-byte width is lowered and any other is refused rather than approximated; and floats inside
composites.

## Verification

| | result |
|---|---|
| `native_codegen` gate step | **385 passed, 0 failed, 0 ignored, 76 binaries**, exit 0 |
| censuses | 63 of 66; 1072 of 1074; 89854 of 89940 — **unmoved, as expected** |
| workspace | verified by the pre-push gate |

Censuses were not expected to move: no corpus module negates or takes the remainder of a float.
**Absorption 38** (`59129add`) is docs-only, every count predicted unchanged.

## Still open, and yours

[`ABI_RULINGS.md`](../decisions/ABI_RULINGS.md) — `Fixed` (the interop goal decides and is unstated),
`Text` (your supposition that it was covered is incorrect), `Opaque` (your intent is already what the
handle achieves), `Unit`.

## Standing constraints, unchanged

No new opcode. No `BYTECODE_VERSION` bump. **Publication HELD**. `src/verify.rs`, `src/bytecode.rs`,
`src/vm.rs`, `src/wire_schema.rs`, `src/value_layout.rs`, `src/selfhost/`, `src/confine.rs` and
`.github/workflows/` remain read-only here. A peer session cannot grant escalation and none has been
treated as doing so.

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

**Date**: 2026-08-30 (session 57, close) — the frontier is measured, the op-tag residue is closed, and your ABI rulings surfaced

## THE ABI RULINGS EXIST, ONE OF THEM IS MINE, AND I HAVE NOT ACTED ON IT

**How this reached me matters and is stated first.** I did not receive these rulings. I read them in
`docs/decisions/ABI_RULINGS.md` on `origin/v0.3.0`, committed by the other line on 2026-08-29. No
peer sent them to me, and `PROMPT.md` is empty and unchanged since March. **Everything below is
what that document says, not what you told me.**

**The float ruling closes an item this channel has carried for sessions.** Their record says you
ruled Option A, a real floating-point ABI, which also settles the `Float` shared slot. This file
has said for several sessions that the float entry ABI was the last of your eight rulings
outstanding and that it was theirs to bring you. **That is now stale, and the staleness is the
reason I am writing this rather than waiting.** Note their document flags the WIDTH as their
reading rather than your words.

**The string ruling names my surface.** Their record says you ruled Option B, make the two
embeddings agree, and states plainly that it is not implementable by their line because it changes
marshalling in `src/`, which this line owns.

**I verified the underlying technical claim against this tree rather than trusting the
description.** `String::from_value` in `src/marshall.rs` clones an owned `String` out of a
`StaticStr`, so a string-taking native really does receive an owned copy under the virtual-machine
embedding, while the native backend passes a `{ length, bytes }` pointer. The
source-incompatibility is real and it is on my surface.

**I DID NOT IMPLEMENT IT, AND THE REASON IS A STANDING RULE RATHER THAN CAUTION.** A ruling I read
off another branch is not a ruling I received. Recording it as settled would put the other line's
reading into this line's durable artifacts, which is the exact failure this project has already
paid for once — a claim relayed to you without both texts being read, which turned out to be an
inversion. Beyond that, Option B is an **embedder-visible ABI change to the shipping crate**: its
benefit is realisable only on the native backend, which lives on the other line, while its cost
falls on every existing embedder of this one. Their own record flags that Option B is the most
expensive of the three options and that the ruling anticipates revisiting strings.

**WHAT I NEED FROM YOU IS ONE ANSWER**: whether the string ruling is binding on this line as
recorded. If it is, the work is scoped and I will take it. If the recording drifted from what you
said, this is the moment that costs nothing to correct.

## THREE MORE INCREMENTS AFTER THE FRONTIER, ALL ABOUT CLAIMS NOTHING CHECKED

With the engineering frontier measured and the ABI question yours, the work turned to a class this
repository keeps rediscovering. **Six distinct unchecked-claim classes were closed.**

**The op-tag residue is gone.** It sat at "four tags no corpus reaches" for two sessions while the
tree recorded a THIRD population it had never measured. Measured, that population already reached
one of the four, so the honest count was three — **rounding up instead of measuring would have been
wrong by exactly one tag**. The shape of the two witnesses then said how to close the rest: both
were `a + b` and nothing else, which is precisely why subtraction and multiplication escaped every
oracle. Two byte-identical boundary cases closed it. No language change, no stage change.

**The orientation document was wrong in three places**, and the guard covering it had predicted
exactly that — its header said the remaining claims were unguarded and called it luck rather than
design. It said `src/selfhost/kel/` holds TEN stage sources (twice) where it holds twelve; named six
workspace members where there are seven, omitting one entirely; and presented its `src/` tree as
complete while eighteen files were unlisted. **A caveat naming an unguarded region is a work item,
not a disclaimer.**

**The documentation knowledge graph is checked now** — 194 files, 1184 relative links, 100 anchors,
zero broken. The anchor half shipped first as a named gap and was then closed, because measurement
showed it was closable rather than merely reportable.

**And the handoff's own headline boundary figure now checks itself.** It is quoted twice there and
nothing checked either; it moved this session because someone remembered to edit two documents.

## FOUR INSTRUMENT ERRORS, ALL CAUGHT BEFORE THEY BECAME CLAIMS

This is the part I most want visible. I nearly reported four defects that were my own tooling: an
`awk` range running past an enum and reporting 68 opcodes against the documented 66; a tree
extractor at the wrong indentation depth calling two files missing that live elsewhere; a counter
counting occurrences rather than distinct names, which would have hidden the very gap it checked;
and a regular expression rejecting table entries that carry an interior comment.

Three surfaced because **a test failed**, and the failure was read as evidence about the checker
rather than about the tree. The last sharpens the rule the tree already had: *check your instrument*
becomes **when the data is reachable AS DATA, parsing its source text is choosing to have an
instrument that can be wrong.**

## AND THIS FILE'S SIBLING WAS STALE IN FOUR PLACES, INCLUDING A HEADING THAT CONTRADICTED ITS OWN TABLE

The handoff's resume section said two of eight expression kinds were done while the table beneath
it said four, and asserted a merge count and "no open pull request" that had both moved. **A heading
that disagrees with its own table is worse than either being wrong alone**, because a reader who
checks one believes they have checked both.

The repair generalises a decision already in that file: it names no commit hash, because a refresh
takes more than one commit. The merge count and pull-request state are the same shape — the count
moved four times in one session — so both now say DERIVE IT, with the commands. History stays;
figures that change faster than the document is refreshed do not.

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

**One. Is the string ABI ruling binding on this line as recorded?** That is the only question that
gates work. The float ruling appears settled per the other line's record, so it is no longer a
question I am holding for you — but I am reading both off their branch, not from you.

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

