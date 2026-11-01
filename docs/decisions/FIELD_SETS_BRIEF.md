# Brief — the third type-channel extraction, and why it is not the size the handoff says

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: brief for the increment. Written 2026-08-28, session 56.

## THE HANDOFF'S GUIDANCE IS OPTIMISTIC, AND I ESTABLISHED THAT BEFORE PLANNING FROM IT

The handoff says the third extraction is a settled pattern — *"take `field_sets` (80 lines) or
`occurrence_rows` (100)"* — with the implication, carried from the two slices that already moved,
that it is a **re-projection of data the driver already holds**. `decl_call_rows_from_pipeline`
says so in as many words: *"The driver already holds every input ... Nothing here re-walks the
source."*

**THAT IS NOT TRUE OF EITHER REMAINING SLICE.** Both need something the driver currently
**discards**, and I found this by reading the driver rather than by trusting the plan.

| slice | what the pipeline already has | what it discards |
|---|---|---|
| `field_sets` | `parse.kel` holds `sd_fstart`, `sd_fcount`, `sd_fname` — **exactly** the first three of the four values the extraction returns | the driver's record loop maps codes `18..=20` to `in_skip_decl = true`, so struct declarations never reach the host at all |
| `occurrence_rows` | function names, call sites | struct and newtype names (skipped), `use` imports (only a flag survives), and ident occurrences keyed by SLOT rather than name |

**No host command exposes the struct table either.** `PARSE_FIELDS_CAP` is 512 in
`src/selfhost_host.rs`, matching `sd_fname`'s width, so the host knows the CAP and has no reader
for the contents. Searched before assuming; the standing rule is to look for callers before costing
work that depends on code.

## THE RECOMMENDATION IS `field_sets`, AND THE REASON IS THE SHAPE OF ITS GAP

`field_sets` returns `(first, count, flat, accesses)`. The first three correspond **one to one**
with a contiguous table `parse.kel` already computes and maintains for its own layout work. The
gap is not "derive something new"; it is "surface a table that exists". `occurrence_rows` by
contrast needs four different declaration kinds, two of them skipped, plus a slot-to-name
resolution that is only partly solved.

**AND THERE IS IN-TREE PRECEDENT FOR THE EXACT MECHANISM.** The driver already collects
declaration-level records for two other kinds: `data_records` on code 9 and `enum_records` on code
12, both accumulated in the same loop that skips 18..=20. Struct records follow that established
pattern rather than inventing one. **Do not design a new mechanism.** This line has now recorded
six instances of building what already existed, and one of them reached the tree.

## PRIOR FAILURES THIS INCREMENT MUST NOT REPEAT

**COMPARE BY NAME ON BOTH SIDES. NEVER BY INDEX.** The reference numbers functions in DECLARATION
order; the pipeline numbers chunks by SORTED name. **Both** slices that already moved hit this, and
the recorded escape is the same each time: *carrying a string removes the question rather than
answering it.* `field_sets` makes this sharper than usual, because it returns **two independent
index spaces** — a type index and a field-name intern index — and neither is shared with the
pipeline's. Compare `(type name, field name)` pairs as strings.

**AND CHECK THE CORPUS SEPARATES THE TWO ORDERS.** If every corpus source declared its structs and
fields in sorted order, a name comparison would be indistinguishable from an index comparison and
the test would pass while establishing nothing. That vacuity was caught last slice only by asking
for it deliberately. Ask for it again.

**REUSE WHAT EXISTS RATHER THAN DESIGNING AROUND IT.** The last brief reasoned carefully about
avoiding a casing hazard in the type-tag rule, and `tag_of` in the driver had already handled it
correctly with the earlier mistake documented in place. **Read the tree before designing around a
hazard it has already handled.**

**PARSE.KEL IS IN THE BYTE-IDENTITY CORPUS.** Any change to it must leave all eleven stages
compiling byte-identically. That is the correctness signal and it is not negotiable. Run the
corpus, and remember the split-by-test-name form: a truncated run with no exit status is a lower
bound on coverage, not a pass, and it has looked like a pass twice.

**STATE WHAT DOES NOT MOVE.** `decl_call_rows_from_pipeline` moved two of three returned values and
said plainly that the third needs an expression classifier and was therefore left. Do the same
here: if the ACCESSES half needs body-walk work the declared-sets half does not, move what moves
and say what did not, rather than implying the extraction is fully migrated.

**THE COUNT IS DERIVED, NEVER RESTATED.** `the_moved_extraction_count_is_four_of_five` counts the
analogues in the driver. A hand-written count is a second definition that goes stale, which is
precisely how the handoff came to assert an already-closed gap was open.

## THE WRONG TURNS SPECIFICALLY

- **Do not add an opcode.** The rad-hard minimal-ISA constraint is a stage-record and host-command
  question here, not an instruction-set one, but the constraint is easy to trip over by reflex.
- **Do not bump `BYTECODE_VERSION`.** It requires operator authorization and none is in hand.
- **Do not widen the driver's skip to a silent collect.** The skip state exists because struct,
  trait and impl declarations once faulted the driver on 29 boundary cases. Collecting struct
  records must not re-admit trait and impl records by accident.
- **Do not claim the extraction is moved if only the declared sets are.** Three of five, or two and
  a half, are both honest; "three of five" when the accesses still walk the reference AST is not.
- **Do not gate a source-text guard behind an off-by-default feature**, and check the feature sets
  that LACK the feature being worked on. That went red on four continuous-integration jobs, then
  three, in one session.

---

## CORRECTION, WRITTEN AFTER THE WORK: THIS BRIEF WAS WRONG IN THE CHEAP DIRECTION

**Left standing rather than edited away**, per this line's practice of keeping a retraction beside
the claim it retracts.

The table above is right that the driver discards struct declarations. **It is wrong about where
the data would have to come from.** It named `parse.kel`'s internal `sd_fstart`, `sd_fcount` and
`sd_fname`, which pointed at surfacing a stage-internal table — new emission from a source that is
itself in the byte-identity corpus, and a correspondingly larger and riskier increment.

**`parse.kel` was already emitting all of it.** A STRUCTSTART carries the type's name id and each
field arrives as its own record, in declaration order, under the same record code a function
parameter uses. The driver received them and mapped the run to skip state. The increment was
therefore **driver-side and touched no stage source at all**.

**The transferable part is not "read more before planning".** I did read, and I read the producer's
data structures. The record stream is the interface, and it already carried the answer. **Reading
the producer's internals told me about the producer, not about what crosses the boundary.**

The brief's other guidance held. The mechanism had in-tree precedent and was followed rather than
reinvented; the comparison carried strings rather than either numbering; the accesses did not move
and that is said plainly; and the demand that something check the trait/impl split turned out to be
the most valuable line in the document, because the agreement test **cannot** see that split and
only a mutation revealed it.
