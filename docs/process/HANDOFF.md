# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than
misleading a resuming agent.

> **Rewritten whole, 2026-08-13**, not patched. A handoff that contradicts itself is worse than a
> stale one, because a reader cannot tell which half to trust. Overwrite this file; do not append.

## Validity

- **Branch**: `v0.2.3`, or a feature branch cut from it.
- **Parent commit**: `fe0e66f2`
- **Written**: 2026-08-13
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint. It governs
  documentation, commit messages, code comments, and anything drafted for publication.

**Check both.** `git rev-parse --abbrev-ref HEAD` is `v0.2.3` or a branch off it, and
`git rev-parse HEAD~1` equals the parent above. The branch half is not redundant: `v0.3.0` carries
parallel native-codegen work and can satisfy the commit check while describing a different
workstream. If you are on `v0.3.0`, read `docs/process/handoffs/v0.3.0.md` and **do not overwrite
this file**.

- **Both match → VALID.** **Commit mismatch → INVALID and STALE.** **Branch mismatch → NOT YOURS.**

## On resume, before doing anything

1. **Read `secret/notes/APPENDIX_B.md`.**
2. **Read the other session's mailbox**: `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.
   It has no wake; poll at increment boundaries. **Read it to the end** — the last request in it was
   missed once by stopping early.
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md)
   (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).**

## FIRST ACTION: the tree is quiet. Confirm it, then pick from THE NEXT WORK.

**Nothing of this line is in flight.** No pull request open, no branch of mine unmerged, no monitor
armed, tree clean and in sync. Confirm with `git status`, `git branch --list`, and
`gh pr list --state open`. **Anything based on `v0.3.0` is the OTHER session's — both lines share a
GitHub account, so `--author @me` matches theirs too. Tell them apart by BASE BRANCH.**

**`git branch --list` is not optional even now.** The other line has left uncommitted work and
branches in this checkout before, on 2026-08-12, after a compaction left it believing it owned
`v0.2.3`. Its changes survived a `git checkout` only because a tracked file was dirty, then vanished,
and were recovered from a scratchpad copy. **A branch survives what a dirty working tree does not.**

## THE WORKFLOW: CI GATES FEATURE BRANCHES

**Do not run `scripts/release-gate.sh` to gate a merge.** Operator decision, 2026-08-11.

1. Cut the feature branch **as the first action of an increment**. The moment to guard is just after
   a merge or a docs commit has left you on the version branch; that is exactly when code has been
   written directly onto `v0.2.3` twice, once caught only before push.
2. Verify locally as you go.
3. Push, open a **draft PR to `v0.2.3`**.
4. **Merge on CI green, at the commit CI ran, without rebasing.** Push. Delete the branch.

**Reproduce the gate's invocation; do not approximate it.** This cost four defects in one day, each
from a different narrowing:

| what was run | what it missed |
|---|---|
| default features | a `compile`-feature gate miss, failing `--no-default-features` |
| `--features signatures` | a `signatures`-gate miss, failing the **default** build |
| `cargo doc` with default features | a rustdoc error only under `--features signatures,encryption,shell` |
| `clippy --tests --all-features` | a `collapsible_if` only under `--all-targets` |

The gate's own invocations are in `.cargo-husky/hooks/pre-push`. Read them rather than remembering.

**`git push origin --delete` runs the full pre-push test tier, once per branch.** Deleting 32 refs in
a loop timed out after ten minutes. Use one push naming every branch, or
`gh api -X DELETE repos/sgeos/keleusma/git/refs/heads/<branch>`.

## THE STATE

`v0.2.3` is at `fe0e66f2`. Eight pull requests merged on 2026-08-13, each 22 of 22 CI jobs green.

| merged | |
|---|---|
| the checked-arithmetic push order, corrected at eight sites | reported as one |
| `SHARED_LAYOUT` run-length encoded | `codegen` aux body 154,880 → **111,864** bytes |
| byte-identity coverage for the five `verify_*.kel` stages | **ten of ten stages** now |
| the SECDED plane emitted and verified end to end | off by default |
| the plane-inside-the-signature property pinned | was inherited from layout, not enforced |
| the scrub / signature ORDER settled by execution | `ECC_SIGNATURE_ORDERING.md` holds nothing open |
| report and scrub as separate optional verbs | scheduling is the host's |
| in-flight CI in the status line | the display had shown a 66-hour-dead gate |

**Boundary counts: 79 Ok / 4 Gap / 1 RefRejects, 84 cases.** Recounted 2026-08-13 from the case table
of `self_hosted_construct_support_boundary` with comment lines stripped. **Recount it the same way
rather than trusting this number** — it has been found stale twice, and two of my own three
extraction attempts returned zero because they read the wrong `let cases` table and the wrong enum
spelling. The right one is the `&[(&str, Support, &str)]` table inside that function.

**73 branches were pruned**, 42 local and 31 remote. What remains is the other line's four local and
three remote `native`/`llvm` branches, `feat/selfhost-wire-data` held by a worktree, and
`v0.2.3-prerebase-backup`. Recovery manifest at `tmp/branch-prune-manifest-20260813.txt`.

## THE NEXT WORK, RANKED, WITH THE TRAP NAMED FOR EACH

**The ECC programme is finished.** This is a genuine choice among bounded roadmap tasks, which the
loop document says is yours to make without prompting the operator.

**1. A second stage through the whole-artifact capstone under the new encoding.**

> **The trap is that the corpus shrank under you.** Artifacts fell twice this session, and the
> capstone lost `verify_yield` and `verify_typed` because their whole bodies now fit one window.
> **Only `parse` (304,432), `codegen` (111,864) and `verify_structural` (102,256) still exceed it.**
> Its size-span control was lowered from 4x to 2x for that reason, and if a further reduction takes
> it below 2x the property has stopped being testable on real output and must move to a synthetic
> artifact rather than shrink again. Do not lower it a second time.

**2. The Order-1 type checker.** Scoped in
[`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md) at about fifteen
rejection shapes, sized by execution rather than counted from 163 `TypeError` sites.

> **The oracle is verdict agreement, not message agreement.** Do not chase identical diagnostics.

**3. Load-time ECC policy**, which is now only "should a host scrub, and when". The verbs make both
answers expressible and nothing forces either.

> **The trap is re-deciding what is decided.** The ORDER is fixed and recorded. Only scheduling is
> open, and the operator has already said a host may scrub on its own schedule.

**Two standing traps.** Do not replace the linear dedup scan (no early exit in a total language;
inputs capped at 256). Do not compute the chunk record's name index (`map[j] == j` always).

## THE RULE THAT MATTERED MOST TODAY

**Measurement overturned two conclusions AFTER they had been written down, and both were mine.**

**Writing a condition as an equation is what falsified the first.** The ordering decision said
verify-then-scrub is a hole outright. Written formally, it is not: a verifying artifact IS the
original, and scrubbing an undamaged artifact is the identity, so at a single instant the order is
safe. The real defect is that **verification is a statement about a moment** — a system verifies at
load and scrubs later, and the assumption that order needs is that no fault occurs in the window,
which is exactly what the parity plane exists because is false. **A design cannot rest on the
negation of its own motivation.**

**Enumerating a small space falsified the second.** Six hand-chosen triple-bit faults all
mis-corrected, reading as 100 percent. All 41,664 give **23,364, or 56.08 percent**, and the six sat
inside byte 0 where the rate genuinely is 100. A biased sample presented as a measurement, over a
space small enough that sampling was never justified.

## FACTS THAT COST REAL EFFORT

- **A (72,64) SECDED code reports 23,364 of 41,664 triple-bit faults as a SUCCESSFUL repair while
  producing the wrong word, and 5,133 of 635,376 four-bit faults as CLEAN.** Both are structural, not
  implementation defects. **A clean report is not an integrity check**; only a signature is.
- **`keleusma_wire::scrub` takes a wire CONTAINER, not a framed module.** Handing it the framed buffer
  makes the parse fail on the magic and the scrub silently repair nothing. Use
  `wire_format::scrub_module_bytes`, which slices the auxiliary body first.
- **A test that reimplements the shipped API tests the reimplementation.** The ordering test carried
  its own scrub and left the real verb unexercised; wiring it to the real one exposed the container
  mismatch above immediately.
- **The plane overhead is 12.5% asymptotically and 20.0% at 680 payload bytes**, because each plane is
  padded to a whole word and every artifact carries the same nineteen regions.
- **`MAX_PARSE_DEPTH` is a DEPTH BUDGET OF 24 SHARED between chain position and arm-body nesting**,
  not an arm count. In the TEST HARNESS, which binds because that is where `wire.kel` compiles,
  `dispatch_driver` holds 20 arms with a no-argument body and 18 with a nested-call body. **Do not
  size a chain from a CLI measurement** — that reads two to three arms too generous, and the harness
  SIGABRTs where the CLI reports a clean `ParseError`.
- **`Op::Reset` is a path exit.** A `loop` chunk has no `Loop` op and ends in `Reset`.
- **Shared data is re-seeded on every VM call**, so a multi-call artifact is carried forward as bytes.
- **A faulted VM is unusable for later calls.**
- **Chained tuple indexing `k.t.0.1` is not admitted.** Pass the nested tuple to a function.
- **The checked-arithmetic opcodes push `(low, high, flag)`**, while the surface form `overflow(h, l)`
  binds high first. Both orders are real and six sites state the binding order correctly.
- **On macOS `timeout` does not exist**; it is `gtimeout`.

## METHOD RULES THIS ARC PAID FOR

- **Reproduce the gate's invocation, do not approximate it.** Four defects, four narrowings, one day.
- **Enumerate a small space instead of sampling it**, and say so when a sample is hand-chosen.
- **Call the shipped API from a test, never a copy of it.**
- **A defect report names where a reader happened to look, not where the defect is.** One reported
  site was eight, five of them in `src/*.rs`.
- **Do not truncate output you intend to quote.** Piping a verification through `tail` destroyed the
  evidence twice in one day.
- **Put a control on the guard, not only on the detector.** A threshold passing by four orders of
  magnitude cannot report anything.
- **Check whether a file is generated before editing it.** `book/src/INSTRUCTION_SET.md` is generated
  from the spec and gated by `git diff --exit-code` in CI.
- **`git branch --list` before writing on a shared surface**, and **back up someone else's
  uncommitted work the moment you find it**.

## Order-1 status

- **Monomorphizer: EMPTY.** Identity on all ten stage sources, pinned with a must-fire control.
- **Type checker: ~15 rejection shapes**, verdict agreement is the oracle.
- **Wire format: emittable end to end**, all five driver-owed values computed, and now with an
  optional SECDED plane the encoder can emit and the reader can verify and repair.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **`v0.2.3-prerebase-backup`**, 309 commits ahead, local only. A deliberate pre-rebase safety copy.
  **Do not delete it without being asked.**
- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.** On a 2 MB thread the stack blows
  before the guard fires, so an embedder parsing untrusted source gets a SIGABRT rather than a
  `ParseError`. An availability failure at a trust boundary. Lowering the constant narrows the
  admitted language surface, so it is not changed unilaterally.
- **`CHANGELOG.md:340` states the checked-arithmetic push order wrongly and describes a PUBLISHED
  release.** `TASKLOG.md:320,331` likewise. Rewriting already-published text is a separate call.
- **A local gate quiet for 68 hours is still shown in the status line.** `gate-status.sh` is the other
  session's instrument and suppressing it would change their semantics.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Parallel development

`v0.3.0` carries native code generation on the same CI-gated workflow. **Three notes are waiting for
them** in `docs/process/handoffs/v0.2.3.md`: the `SharedSlotRecord` move with its accessor split
(`shared_count` is gone, replaced by `shared_record_count` and `shared_slot_count`), the status-line
change with the reasoning for leaving `gate-status.sh` untouched, and the branch prune with what
remains that is theirs.

They hold `src/wire_schema.rs` and `src/bytecode.rs` read-only and announce before widening. Extend
the same courtesy: **announce a change to their read surface before making it.**

## Untracked artifacts a fresh session cannot see

`tmp/` is gitignored, so none of this is in the repository:

- **`tmp/2026-08-10-when_error_correction_meets_a_signature.markdown`** — research spike A373 on
  ECC-and-signature composition, 4.8 MB, 13,796 references, passing the blog corpus checker with **0
  findings**. Identifiers came from Crossref by title-and-author query, never from memory; a bare
  title query returned the WRONG work for 11 of 74, and eight remain unregistered and are cited as
  plain text.
- **`tmp/a373/`** — the harvest pipeline: `harvest.py`, `select.py`, `gate.py`, `resolve_hand.py`,
  `refine_hand.py`, `gen_refs.py`, `assemble.py`, and the four Crossref rounds.
- **`tmp/a373_instrument_*.rs`** — the two measurement instruments behind the article.
- **`tmp/branch-prune-manifest-20260813.txt`** — the ONLY record of 73 deleted branches, with recovery
  commands in its header.
