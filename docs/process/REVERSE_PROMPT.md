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
| `v0.2.3` | `f1fc5ffc`, pushed, tree clean, in sync with origin |
| PRs merged this session | **#42, #43, #44, #46, #47, #50, #51, #53** — each 22 of 22 green, merged at the commit CI ran |
| Open PRs of this line | **none** |
| Branches | 73 merged branches pruned; what remains is the other line's, a worktree, and a backup |
| Boundary counts | **79 Ok / 4 Gap / 1 RefRejects, 84 cases** — recounted from the table with comments stripped, matches the recorded figure |

**The parity-plane arc is complete and `docs/decisions/ECC_SIGNATURE_ORDERING.md` holds nothing
open.** Self-hosted byte-identity coverage reached ten of ten stages. Artifact sizes fell again:
`codegen`'s auxiliary body 154,880 to **111,864** bytes, `lexer`'s to **7,456**.

## Two conclusions that measurement overturned after I had written them down

**The ordering decision was wrong in its first form.** I wrote that verify-then-scrub is a hole
outright. Writing the soundness condition as an equation showed it is not: an adversary without the
key cannot produce a verifying artifact other than the original, and scrubbing an undamaged artifact
is the identity, so **at a single instant the order is safe**. The real defect is that verification is
a statement about a moment. A system verifies at load and scrubs later, and the assumption that order
needs is that no fault occurs in the window, **which is exactly what the parity plane exists because
is false**. The corrected argument is stronger and it connects the problem to
time-of-check-to-time-of-use, which the first version had no reason to reach for.

**A sampled measurement reported 100 percent where the truth is 56.08 percent.** Six hand-chosen
triple-bit faults all mis-corrected. Enumerating all 41,664 gives 23,364, and the six sat inside byte
0 where the rate genuinely is 100 percent. **A biased sample presented as a measurement**, wrong by
nearly a factor of two, over a space small enough that sampling was never justified. The enumeration
also produced the result the design turns on: **5,133 of 635,376 four-bit patterns are reported
CLEAN**, because the error pattern is itself a codeword. A clean report is not an integrity check, and
`EccReport::is_clean` now says so.

## A design correction that came from the operator, not from me

I had concluded the fix for the ordering problem was a mutable **load path**. That would have pushed
`&mut` into the common path and cost the zero-copy and worst-case-memory properties the reader exists
for. **Report and scrub as separate verbs is the right shape**: report already existed and only the
mutating counterpart was missing. `scrub` returns counts rather than an artifact, so there is nothing
to load without re-authenticating, and `&mut [u8]` makes the unsound order unrepresentable wherever
the reader borrows the buffer. Scheduling is the host's by operator decision.

## The mistake I made four times in one day

**I approximated the gate's invocation instead of reproducing it**, and each narrowing hid a different
failure:

| what I ran | what it missed |
|---|---|
| default features | a `compile`-feature gate miss, failing `--no-default-features` |
| `--features signatures` | a `signatures`-gate miss, failing the **default** build |
| `cargo doc` default features | a rustdoc error visible only under the docs.rs feature set |
| `clippy --tests --all-features` | a `collapsible_if` visible only under `--all-targets` |

All four were caught by the pre-push gate or CI, so nothing unsound shipped. **The local signal was
worth less than it appeared each time**, which is the same shape as the `$?`-after-a-pipeline defect
recorded on 2026-08-12.

## Two smaller findings worth not rediscovering

**A reimplementation hid an interface mismatch.** The ordering test carried its own copy of a scrub,
exercising a private reimplementation and leaving the shipped verb untested. Wiring it to the real one
failed at once: `keleusma_wire::scrub` takes a wire **container** and the test handed it a **framed**
module. The parse failed on the magic, the scrub returned `None`, and nothing was repaired, silently.
`scrub_module_bytes` exists so no host repeats it.

**`git push origin --delete` runs the full pre-push test tier, once per branch.** Deleting 32 refs in
a loop timed out after ten minutes having spent all of it running tests in order to delete pointers.
One push naming every branch, or `gh api -X DELETE .../git/refs/heads/<b>`, avoids it.

## Concerns raised, not acted on

- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.** Unchanged and still yours. The
  constant is 24 (`src/parser.rs:98`); on a 2 MB thread the stack blows before the guard fires, so an
  embedder parsing untrusted source on a small-stack thread gets a SIGABRT rather than a `ParseError`.
  An availability failure at the trust boundary the guard exists to hold.
- **`CHANGELOG.md:340` states the checked-arithmetic push order wrongly and describes a published
  release.** `TASKLOG.md:320,331` likewise. Left unchanged: rewriting already-published text is a
  separate call from correcting a live specification.
- **A local gate quiet for 68 hours is still shown in the status line.** That is `gate-status.sh`'s
  call and it is honestly labelled; suppressing it would change the other session's semantics, so it
  is raised in their mailbox instead.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **`v0.2.3-prerebase-backup`**, 309 commits ahead, local only. A deliberate safety copy of pre-rebase
  history, not deleted and not to be without being asked.
- **`MAX_PARSE_DEPTH` on small stacks**, above.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Next intended step

The ECC programme is finished, so the next increment is a genuine choice among bounded roadmap tasks
rather than a continuation. In order of my preference:

1. **A second stage through the whole-artifact capstone under the new encoding.** Artifacts shrank
   twice this session and the capstone corpus lost two stages to that; only three still exceed one
   window. Worth confirming the composition still holds where it can.
2. **The Order-1 type checker**, scoped in
   [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md) at about 15
   rejection shapes, sized by execution. The oracle is verdict agreement, not message agreement.
3. **Load-time ECC policy**, which is now purely a question of whether a host should scrub and on what
   schedule. The verbs make both answers expressible and nothing forces either.

## Parallel development

`v0.3.0` carries native code generation on the same CI-gated workflow. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries, since there is no wake.
**Tell the two lines apart by BASE BRANCH, not by author**: we share one GitHub account.

Three notes are waiting for them: the `SharedSlotRecord` move with its accessor split, the status-line
change with the reasoning for not touching `gate-status.sh`, and the branch prune with what remains
that is theirs.

## Method rules this session paid for

- **Reproduce the gate's invocation, do not approximate it.** Four defects, four narrowings, one day.
- **Enumerate a small space instead of sampling it.** 41,664 triple-fault patterns, and the sample was
  wrong by nearly a factor of two.
- **Write the condition as an equation before deciding it holds.** That is what showed the first
  ordering conclusion was wrong.
- **Call the shipped API from the test, not a copy of it.** A reimplementation hid a real interface
  mismatch.
- **A defect report names where a reader happened to look, not where the defect is.** One reported
  site was eight.
- **Do not truncate the output of a run whose result you intend to quote.** Done twice today.
