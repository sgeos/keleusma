# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-13 (session 42)

## Where things stand

| | |
|---|---|
| `v0.2.3` | `b5ddc418`, pushed |
| PR #42 | `docs/checked-push-order`, draft, CI running |
| PR #43 | `feat/shared-layout-runs`, draft, CI running |
| PR #41 | **not mine** — base `v0.3.0`, the other line's |
| Machine | free; both CI runs on hosted runners |

Two increments this session, both small and both gated by CI rather than by the local
gate. Nothing merged yet.

## A defect report named one site and the defect was at eight

The `v0.3.0` session reported that `docs/spec/GRAMMAR.md:747` states the runtime pushes
`(high, low, flag)` for the checked-arithmetic opcodes when it pushes `(low, high, flag)`.
**Verified against the implementation before acting, then swept the repository rather than
fixing the line reported.** Eight sites carried it. Five are in `src/*.rs`, and three of
those are compiler comments — two sitting directly beside the `PopN(2)` whose correctness
depends on the order.

One of them is worth stating on its own: `src/bytecode.rs` claimed `CheckedNeg` pushes in
"the same shape: high, low, flag" **twenty lines below** the `CheckedAdd` doc that had
already been corrected to say the opposite. **A file contradicting itself within twenty
lines is what an incremental single-site fix produces.**

**The error is durable because both orders are real.** The runtime pushes low first; the
surface form `overflow(h, l)` binds high first. Six further sites say `(high, low)`
**correctly**, about the binding, so a search and replace would have broken them. All
fourteen candidate sites were read in context and classified. `GRAMMAR.md` and
`book/src/BIG_NUMBERS.md` now state **both** orders and why they differ, rather than
correcting one and leaving the reversal to be rediscovered.

**The generalisation: a defect report names where a reader happened to look, not where the
defect is.** Same shape as "the corpus cannot reach X is a fact about the corpus", arriving
from the direction of a bug report instead of a test corpus.

## A plan's central number was unmeasured, and checking it took ten minutes

The plan ranked run-length encoding `SHARED_LAYOUT` as the next increment at "roughly 27%"
saving. **`SharedSlotRecord` is ONE word today**, and a run record needs `first_slot` (for
binary search on the `get_shared`/`set_shared` hot path) plus `run` and `stride`, taking it
to **TWO**. So the encoding is a **pessimisation** unless the mean run exceeds 2 — and
neither the plan nor the 27% figure measured that distribution.

**Raised as a blocker before writing encoder code, and refuted by four orders of
magnitude.** Across all eleven stage sources, accounting for the `u16` `run` field's 65,535
chunking: **643,276 slots collapse to 18 runs, mean 35,738.** The table goes from 5,146,208
bytes to **400**.

Two consequences, one of which corrects the plan's own reasoning:

- **`first_slot` binary search is kept, but not for the reason given.** With one to six
  records per stage a linear scan would be fast. It is kept because a scan's bound is
  **data-dependent** and this project sells static bounds, not typical-case speed.
- **The `u16` `run` field is load-bearing.** `lexer`'s largest run is 393,216, chunking into
  seven records. Counting logical runs rather than emitted records understates `lexer`
  sevenfold — the kind of error that makes a projection look better than the artifact.

The measurement is now `tests/shared_layout_runs.rs` rather than a note, because the payoff
is a property of how stages **declare** shared data, not of the encoder. It carries a
control on **its own guard**: a fully fragmented synthetic layout must be rejected by the
same threshold, because a check passing by four orders of magnitude is otherwise
indistinguishable from one that can no longer report anything.

## The other line asked me to run something that does not exist

They asked for `assert_stream_sequences_agree` over the ten stages. **There is no such
function anywhere in the repository.** I ran the nearest thing that answers the question —
the per-stage self-hosted byte-identity tests — and got **82 passed, 0 failed, 288.76 s**.

**The part that matters more than the green result**: only **five** of the ten stages have
a self-hosted byte-identity test at all (`lexer`, `parse`, `reconstruct`, `codegen`,
`analyze`). The five `verify_*.kel` stages have **none** — they appear in
`tests/wire_corpus.rs` and `tests/selfhost_wire.rs` only as **reference-compiled** inputs to
wire-format tests, which never run the self-hosted compiler over them. So the honest answer
is five of ten verified and five of ten unverified, not ten of ten green. Reported to their
mailbox as such.

## Two of my own process failures this session

- **I piped a verification through `tail -40`**, which truncated the very evidence I meant
  to report to the other line and hid whether the `analyze` case had run. Recovered by
  counting that exactly 82 functions match the filter against the reported "82 passed",
  which is sound, but the pipe should not have been there. **Do not truncate the output of
  a run whose result you intend to quote.**
- **I nearly skipped the run-distribution measurement** because the plan stated a saving
  with apparent confidence. The plan document is this project's own artifact and it had the
  same unmeasured-premise defect the loop document warns about in recorded status claims.

## Concerns raised, not acted on

- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.** Unchanged from the last
  session and still the operator's call. The constant is 24 (`src/parser.rs:98`); on a 2 MB
  thread the stack blows before the guard fires, so an embedder parsing untrusted source on
  a small-stack thread gets a SIGABRT rather than a `ParseError`. That is an availability
  failure at the trust boundary the guard exists to hold.
- **`CHANGELOG.md:340` states the push order wrongly, and it describes a published
  release.** `TASKLOG.md:320,331` likewise. Left unchanged deliberately: rewriting
  already-published text is a separate call from correcting a live specification.
- **Five of ten stages have no self-hosted byte-identity coverage**, above. This was not
  previously stated anywhere as a coverage gap.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **The (72,64) SECDED plane is entirely unexercised** by the shipping encoder. Called a gap
  to close; prioritisation open. Given radiation hardness is the stated value proposition, a
  feature proven only in isolation is the weakest part of that claim.
- **`MAX_PARSE_DEPTH` on small stacks**, above.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Next intended step

1. **Merge #42 and #43 on CI green**, at the commit CI ran, without rebasing.
2. **Implement the `SHARED_LAYOUT` run-length encoding**, which now has a measured basis and
   a design: `first_slot` for binary search, stride **stored** rather than derived, `u16`
   run chunking as `DATA_SLOTS` already does. This moves `SharedSlotRecord`, which is the
   `v0.3.0` session's declared read surface; advance notice is already in their mailbox.
3. **The (72,64) SECDED plane end to end**, if the operator prioritises it.

## Parallel development

`v0.3.0` carries native code generation on the same CI-gated workflow. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries — there is no
wake. **Tell the two lines apart by BASE BRANCH, not by author**: we share one GitHub
account, so `--author @me` matches theirs.

## Method rules this session paid for

- **Verify a defect report against the implementation, then sweep rather than fix the line
  named.** One reported site was eight, and the unreported ones were in compiler comments a
  maintainer reads while changing the very code they misdescribe.
- **A plan document is not evidence.** Its central number was a projection stated in the
  register of a measurement, in this project's own artifact.
- **Put a control on the guard, not only on the detector.** A threshold passing by four
  orders of magnitude cannot report anything, and nothing about the headline number says so.
- **Do not truncate output you intend to quote.**
- **Check whether the file you are about to edit is generated.** `book/src/INSTRUCTION_SET.md`
  is generated from the spec and gated by `git diff --exit-code` in CI, and there are two
  big-number documents in the book of which only one was the right target.
