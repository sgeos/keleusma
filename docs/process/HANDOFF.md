# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than
misleading a resuming agent.

> **Rewritten whole, 2026-08-14**, not patched. A handoff that contradicts itself is worse than a
> stale one, because a reader cannot tell which half to trust. Overwrite this file; do not append.

## Validity

- **Branch**: `v0.2.3`, or a feature branch cut from it.
- **Parent commit**: `b5b9c2b6`
- **Written**: 2026-08-14
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
   No wake; poll at increment boundaries. **Read it to the end.**
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md)
   (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).**

## FIRST ACTION: confirm the tree is quiet, then take the named increment

`git status`, `git branch --list`, `gh pr list --state open`. Thirteen pull requests merged on
2026-08-13/14, none open. **Anything based on `v0.3.0` is the OTHER session's — both lines share a
GitHub account, so tell them apart by BASE BRANCH, not by author.**

## THE WORKFLOW: CI GATES FEATURE BRANCHES

**Do not run `scripts/release-gate.sh` to gate a merge.** Operator decision, 2026-08-11.

1. Cut the feature branch **as the first action of an increment**. This session left one slice's
   changes sitting on a documentation branch by not switching after creating it; `git status` before
   committing is what caught it.
2. Verify locally as you go. **Reproduce the gate's invocation, do not approximate it.** The
   invocations are in `.cargo-husky/hooks/pre-push`.
3. Push, open a **draft PR to `v0.2.3`**.
4. **Merge on CI green, at the commit CI ran, without rebasing.** Push. Delete the branch.

**VERIFY THE REF AFTER A PUSH, NOT THE GATE OUTPUT.** A push this session printed
"pre-push: all checks passed" and **never created the ref**. `git ls-remote --heads origin <branch>`
is the check. The output had been truncated with `tail -3`, which cut the line that would have said
so.

**Two CI failures this session were INFRASTRUCTURE, not the diff.** Six jobs failing at `Set up job`,
and a lost runner after 59 minutes with no retrievable log. Both cleared on `gh run rerun <id>
--failed`. Check the failing STEP before believing a failure: a docs-only change failing Clippy and
MSRV together is a runner, not a defect.

## THE STATE

**Seventeen pull requests merged, each 22 of 22 CI jobs green.** `selfhost_wire` is at **156 tests**.

| | |
|---|---|
| the whole-artifact capstone gains a synthetic, erosion-proof case | #54 |
| record-shape coverage measured and closed at **17 of 17** | #57 |
| the module-input producer: chunk names | #58 |
| the module-input producer: enum layouts, both intern modes | #59 |
| the type checker's sliced implementation plan | #60 |
| the constant contributor: roots, names | #62 |
| the constant contributor: roots, six-word node table | #65 |
| the constant contributor: CHILD positions, names and node table -- **the contributor is COMPLETE** | #69 |
| roadmap and plan currency corrections | #61, #63, #64, #66, #67, #70 |

**Boundary counts: 79 Ok / 4 Gap / 1 RefRejects, 84 cases.** Recount from the
`&[(&str, Support, &str)]` table inside `self_hosted_construct_support_boundary`, comment lines
stripped. **Three of my own extraction attempts have returned a wrong number** — the enum aliases are
`SOk`, `Gap`, `RefRejects`, only the first carries the `S`.

## THE NEXT INCREMENT, SPECIFIED

**THE CONSTANT CONTRIBUTOR IS COMPLETE** (slice 14e, #69): roots and child positions, names and node
table, all four. The interning sequence is no longer a Rust model.

**A prediction that increment disproved, recorded because the reflex would inflate every remaining
estimate on this line.** The plan and this file both said the nesting walk "needs an explicit stack
because the language has no recursion". **It does not.** The blob carries the forest in PREORDER and
the reference pushes a node then descends, so the node table and the name sequence are BOTH in that
order and a linear scan reproduces both. A stack is needed only to reconstruct tree SHAPE, which the
producer does not do. **The cost of a total language was assumed rather than measured.**

What survives from that specification and still binds: **`STRUCT` interns its field names FRESH while
`ENUM` interns both names DEDUP**, and a single "a composite interns its names" rule is wrong for one
of the two, visibly only where a name repeats.

**Next, and they are ONE increment**: removing `wire.kel` from the `read_stage` exclusion and the
residency staging a stage's 395,804 names force. The plan is explicit that doing either alone is
wasted, and `read_stage` is **blocked behind the driver, not beside it** — its stated criterion
("when it produces bytes rather than a checksum") now reads as met, and it is still not a one-line
change.

**B, the self-hosted type checker, is blocked on the same question.** Its plan is merged
([`../decisions/TYPECHECK_IMPLEMENTATION_PLAN.md`](../decisions/TYPECHECK_IMPLEMENTATION_PLAN.md),
rejection only, six slices). It shares an input-encoding problem with the wire-format line, and
**neither should invent a second encoding**. Do not build the checker before its input encoding
exists.

## FACTS THAT COST REAL EFFORT

- **When the question is "does anything ever do X", INSTRUMENT, do not grep.** All seven
  previously-empty region kinds appear in the test file, seven hits out of seven, and that proves
  nothing: a kind can be named in a stride table or a negative test with no record of that shape ever
  written. Instrumenting every emit command gave 16 of 17, and named the missing one.
- **A missing capability can hide behind a coverage gap.** `STRUCT_TEMPLATES` had no decoder AND no
  dispatch arm; the emitter refused it with `-222`. A differential cannot see a mistranscribed offset
  in a shape it never reaches.
- **All six formerly-empty record shapes are reachable from REAL COMPILED MODULES**, including
  `STRUCT_AUX` and `ENUM_AUX` via `const data`. The plan expected hand-built artifacts; they are not
  needed.
- **A struct template is written only on the BOXED path.** `flat_alloc_bytes` returns `None` above the
  sixteen-bit operand bound, so a struct wider than 65,535 bytes reaches it — 8,300 `Word` fields.
- **THE TWO DEDUP SCANS ARE DIFFERENT SCANS.** `intern_run` is batch-local, capped at 256, and must
  NOT be replaced: a total language has no early exit, so a 1024-slot table costs 1024 probes against
  about 256 comparisons. The walk-nested scan through `NAMES` is the one the 782-second lesson bears
  on, and it is to be MEASURED at stage scale.
- **PER-CHUNK RANGES ARE ALREADY SELF-HOSTED.** `emit_chunks_batch` accumulates the cursors and writes
  each `*_first` before advancing. The roadmap listed them as remaining; it was stale.
- **Private data PERSISTS across VM calls; shared data is RE-SEEDED.** A flag left set by one call
  silently changes the next. `mi_pairs` sets `quiet` explicitly for this reason.
- **`emit_at` is at EIGHTEEN arms**, the measured parse-depth ceiling for that shape in the TEST
  HARNESS, which binds because that is where `wire.kel` compiles. A nineteenth needs the chain
  restructured, not extended.
- **`highest_command()` is a real guard.** A new command returns `-99` until the ceiling is raised.
- **On macOS `timeout` does not exist**; it is `gtimeout`.

## METHOD RULES THIS SESSION PAID FOR

- **Instrument rather than grep** when asking whether anything ever does X.
- **Verify the ref after a push**, not the gate output.
- **Write the encoding down before relying on it.** The enum count would have read correctly out of
  zero-filled memory whether or not the encoder wrote it.
- **A guard refusing loudly is the guard working.** `-99`, `-222`, and a compiler rejecting
  `if <Word>` each surfaced a real gap as a refusal rather than a wrong artifact.
- **Check an item against the code before repeating it.** Two roadmap items moved on inspection.
- **Assert WHICH failure fired**, not merely that one did.
- **A bound on a loop is not a bound on the damage.** Doubling puts the cost in the last attempt.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **`v0.2.3-prerebase-backup`**, local only, a deliberate pre-rebase safety copy. Do not delete it
  without being asked.
- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.** An availability failure at a
  trust boundary. Lowering the constant narrows the admitted language, so it is not changed
  unilaterally.
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Parallel development

`v0.3.0` carries native code generation on the same CI-gated workflow. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries. They hold
`src/wire_schema.rs` and `src/bytecode.rs` read-only and announce before widening; extend the same
courtesy.

## Untracked artifacts a fresh session cannot see

`tmp/` is gitignored:

- **`tmp/2026-08-10-when_error_correction_meets_a_signature.markdown`** — research spike A373, 4.8 MB,
  13,796 references, passing the blog corpus checker with 0 findings.
- **`tmp/a373/`** — the harvest pipeline.
- **`tmp/branch-prune-manifest-20260813.txt`** — the ONLY record of 73 deleted branches.
