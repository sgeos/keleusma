# BRIEF — I derived a decisions list from a coverage measurement

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## The defect, and its mechanism

`OPERATOR_DECISIONS_OPEN.md` claims to be the page the operator can act from. Asked directly whether
the ABI issues were resolved, I found it **lists three decisions and does not mention the `Fixed`
shared-slot ABI at all** — an open item with its own decision document, on which the operator had
already ruled it should be settled alongside the float ABI.

**The mechanism matters more than the omission.** The page says:

> *"There is no fourth thing to fix. The three are below."*

That sentence is derived from the module-lowering census — 4 unlowerable chunks sitting in 3 refused
modules — and then written as exhaustive over **decisions**. It is not. **A coverage census can only
surface a decision that blocks a corpus module.** Verified: no corpus source declares a `Fixed`,
`Float` or `Text` shared slot, so those refusals block nothing, appear in no figure, and were
invisible to a list built from figures.

**This is the session's recurring defect in a new place** — a measurement answering a narrower question
than the claim built on it — and this time the claim was the operator-facing summary rather than a
test.

## What else the same mechanism hides

Anything host-visible the corpus does not exercise. Known: the **string ABI** (ratified nowhere; the VM
and native embeddings are not source-compatible for a string-taking native), and the `Unit`, `Float`,
`Text` and `Opaque` shared-slot kinds, each refused for a different recorded reason.

## The code change, which is not optional tidying

`FIXED_SHARED_SLOT_ABI.md` carries an ACTION for *whichever line owns the message*:

> the refusal text should say **the host-visible fraction-bit scale is unspecified** rather than
> *fixed-point representation is unsettled*. The current wording sends a reader looking for a
> representation decision that was made long ago.

`alloc_format_kind` is in `native_codegen/src/lib.rs`. **This line owns it**, so the action is mine and
has been sitting unclaimed.

## Wrong turns to avoid

- **Do not wait for the interop answer before listing `Fixed`.** I said last turn I would hold the
  amendment pending it. That is backwards: the page exists to prompt the answer, so withholding the
  item guarantees the answer never comes.
- **Do not re-argue the cases on the page.** It is an index. Each item has a record; the page says what
  is open, what it costs, and what happens by default.
- **Do not recommend where the underlying record declined to.** `FIXED_SHARED_SLOT_ABI.md` states a
  preference *conditional on an unanswered question* — the page must carry the condition, not collapse
  it to the preference.
- **Do not replace one over-broad claim with another.** The fix is to say what the figure is exhaustive
  over — corpus-blocking work — not to delete the figure or to assert the new list is complete.
- **Do not change the refusal's behaviour**, only its wording. The refusal was already correct.
- **Do not touch `src/value_layout.rs` or `src/wire_schema.rs`.** They are the `v0.2.3` line's.

## What good looks like

A page that distinguishes *what blocks a corpus module today* from *what is undecided*, lists every
known open ABI item with its options and its default, and states plainly that its completeness rests on
my search rather than on a measurement.
