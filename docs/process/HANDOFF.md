# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt. Unlike the three resume channels it is **not** kept
always-current, so it must be able to report itself stale rather than mislead a resuming agent.

> **Rewritten whole, 2026-08-15**, not patched. A handoff that contradicts itself is worse than a
> stale one, because a reader cannot tell which half to trust. Overwrite this file; do not append.

## Validity

- **Branch**: `v0.2.3`, or a branch cut from it. If you are on `v0.3.0`, read
  `docs/process/handoffs/v0.3.0.md` and **do not overwrite this file**.
- **Written**: 2026-08-15, describing the tree at `0de6d6d0`.
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint. It governs
  documentation, commit messages, code comments, and anything drafted for publication.

**THE PREVIOUS STAMP WAS A HASH MATCH AND IT SELF-INVALIDATED.** It required `git rev-parse HEAD~1`
to equal a recorded parent, so the first unrelated merge made a otherwise-accurate handoff report
itself stale. That is what happened on 2026-08-15: three merges landed and the file failed its own
check while its contents were still largely true. The `v0.3.0` line hit the same defect and fixed it
the same way. **Validate by ancestry and by content, never by a hash match.**

```sh
git merge-base --is-ancestor 0de6d6d0 HEAD    # must succeed
```

**Then validate the CONTENT, which is what actually matters:**

```sh
cargo test --features compile,verify --test block_form_statements    # 8 passed

# The construct-support boundary. This finds the table by its own declaration
# rather than by line number, because a hardcoded range is the defect this file
# spends a section warning about.
awk '/let cases: &\[\(&str, Support, &str\)\] = &\[/{f=1;next} f&&/^    \];/{f=0} f' \
    tests/selfhost_codegen.rs \
  | sed 's://.*::' | grep -oE '\b(SOk|Gap|RefRejects)\b' | sort | uniq -c
# expect: 4 Gap, 1 RefRejects, 79 SOk
```

**Strip the comments before counting.** The aliases are `SOk`, `Gap`, and `RefRejects` — only the
first carries the `S` — and a previous session reported three wrong numbers extracting them by hand.

If the counts differ, the boundary has moved and the state below is stale. **Say so rather than
acting on it.**

## Derive numbers, do not copy them forward

This file used to carry a `selfhost_wire` test count. It said 157 while the tree held 161. Every
hand-maintained restatement of something a mechanism can derive is a defect waiting for the
maintenance to lapse, and this project has now been bitten by that class **six** times. So:

```sh
grep -c '^\s*#\[test\]' tests/selfhost_wire.rs        # the wire differential
git log --oneline -1 v0.2.3                           # where the version branch is
gh pr list --state open                               # what is in flight, BY BASE BRANCH
git log -1 --format=%cd v0.2.3 -- docs/process/handoffs/v0.2.3.md   # when my mailbox last moved
```

## On resume, before doing anything

1. **Read `secret/notes/APPENDIX_B.md`.**
2. **Read the other session's mailbox**: `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.
   No wake; poll at increment boundaries. **Read it to the end.**
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md)
   (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).**

## FIRST ACTION: confirm the tree is quiet

`git status`, `git branch --list`, `gh pr list --state open`. **Anything based on `v0.3.0` is the
OTHER session's — both lines share a GitHub account, so tell them apart by BASE BRANCH, not by
author.**

## THE WORKFLOW: CI GATES FEATURE BRANCHES

**Do not run `scripts/release-gate.sh` to gate a merge.** Operator decision, 2026-08-11. CI is a
verified strict superset of the local gate and runs in about 48 minutes against roughly 2h30m.

1. Cut the feature branch **as the first action of an increment**, and `git status` before
   committing. A previous session left a slice's changes on the wrong branch by not switching.
2. Verify locally as you go. **Reproduce the gate's invocation, do not approximate it.** The
   invocations are in `.cargo-husky/hooks/pre-push`.
3. Push, open a **draft PR to `v0.2.3`**.
4. **Merge on CI green, at the commit CI ran, without rebasing.** Push. Delete the branch.

**VERIFY THE REF AFTER A PUSH, NOT THE GATE OUTPUT.** `git ls-remote --heads origin <branch>` is the
check. A push once printed "pre-push: all checks passed" and never created the ref. **Do not pipe the
push through `tail`** — it truncates the very evidence you meant to read, which happened again on
2026-08-15 and cost the hook log on an otherwise clean push.

**A default-feature run is not the gate.** `cargo test --workspace` and `--features compile` both
miss `self-host`, and a cap-pinned test escaped both while being caught by CI. The gate is a
five-entry feature matrix.

**Check the failing STEP before believing a CI failure.** Two failures were infrastructure, not the
diff, and both cleared on `gh run rerun <id> --failed`. A docs-only change failing Clippy and MSRV
together is a runner, not a defect.

## THE STATE

**Everything of this line is merged; nothing of mine is open.** Confirm with `gh pr list --state
open` and treat anything based on `v0.3.0` as the other session's.

| | |
|---|---|
| the name ceiling | RAISED. `parse` joins byte-identically at 627 names, 33,395-byte blob |
| the join corpus | all ten stages byte-identical through `mi_join` |
| the operand-stack model | **repaired**; the understated WCMU bound is closed |
| the three remaining host models | **checked against independent sources**, two findings pinned |
| the reported `break` discrepancy | **answered**; it was a stray semicolon, not `break` |
| construct-support boundary | **79 Ok / 4 Gap / 1 RefRejects, 84 cases** |

## WHAT THE LAST TWO INCREMENTS ESTABLISHED, AND WHAT THEY DID NOT

**A differential against the model under test cannot detect that the model is wrong.** `analyze.kel`
self-hosts the control-flow ALGORITHM, not the models; it receives `Op::cost()`, the stack-effect
pair, `Op::heap_alloc()` and the class tables from the host, so the self-hosted differential agrees
**by construction**. One of those four inputs was unsound while every differential in the tree was
green. All four are now checked against sources that are not themselves.

**`Op::cost()` disagrees with measurement and is PINNED, NOT REPAIRED.** The nominal tier boundary
separating `{Div, Mod}` from `{CmpEq, CmpLt}` is unsupported — measured at the same
`ops_per_pattern`, all four sit within seven cycles with `Div` the cheapest, and the same inversion
appears on `thumbv8m`. Changing a calibration is a judgment call, not a correctness fix.

**Only 17 opcodes of 66 were ever measured.** Every other value in the emitted cost model is a bucket
assignment, checked by nothing. Do not read the model's ordering as evidence outside those 17.

~~**A live structural hazard remains open**: `analyze_class` ends in `_ => (0, 0)`.~~ **CLOSED
2026-08-15.** `analyze_class` and `analyze_opk` are exhaustive over `Op`, so the compiler refuses a
new opcode until someone decides its class. Verified by adding a variant to `Op` and observing
`E0004` at both sites. **The classification is unchanged** — every opcode the catch-all matched
still maps to the plain group; what changed is that the decision is now forced rather than defaulted.

**What the compiler still cannot guarantee is that a classification is RIGHT.** Exhaustiveness is
satisfied just as well by putting a new control-flow opcode in the plain group, which is the same
silent-edge defect wearing a different hat. The nine-class count stays pinned by test for that
reason.

## FACTS THAT COST REAL EFFORT

- **CHECK A FIGURE AGAINST THE THING IT CLAIMS TO MEASURE.** "395,804 names" was a region record
  count belonging to `CONSTS` and survived three documents, making a 2.5x problem look like a 1500x
  one. Separately, "the hard limit is 512" was a guard reading `wire.nout` while bounded by
  `fin_capacity()`, a number with no relationship to the buffer it touches. **Twice in two sessions,
  in one document.**
- **A DIAGNOSTIC NAMES WHERE THE PARSER STOPPED, NOT WHAT IT OBJECTED TO.** The `v0.3.0` line
  reported `GRAMMAR.md` documenting a `break;` the parser rejects. The documented form parses. The
  rejection came from a stray `;` after a `for` block, and `unexpected token Semicolon in expression`
  named the semicolon. **The control settles it**: remove `break` entirely, keep the stray semicolon,
  and the failure is identical.
- **A DEFECT REPORT NAMES WHERE A READER HAPPENED TO LOOK, NOT WHERE THE DEFECT IS.** That was the
  other line's lesson to me about `GRAMMAR.md`, and it came back the other way within a week.
- **APPEND TO A SLOT-ADDRESSED BLOCK, NEVER INSERT.** Two off-by-one defects came from ignoring the
  convention the file states: once shifting every later field and failing four tests at once, once
  stepping over a scratch word so `calling-a-local` was silently ACCEPTED.
- **Say which fact a green suite does NOT establish**, in the source, where a reader of the code will
  meet it. The slot-name intern mode is unverified by the corpus. The dedup branch has no real-module
  coverage: making `nm_find` report "not found" unconditionally leaves all ten stages byte-identical,
  and that is the scan whose cost is the stated reason the name count is capped at all.
- **When the question is "does anything ever do X", INSTRUMENT, do not grep.** Seven hits out of
  seven proved nothing; instrumenting every emit command gave 16 of 17 and named the missing one.
- **`git checkout <file>` to undo a bad edit discards everything else in that file.**
- **`emit_at` is at EIGHTEEN arms**, the measured parse-depth ceiling for that shape in the test
  harness, which binds because that is where `wire.kel` compiles. A nineteenth needs the chain
  restructured, not extended.
- **`highest_command()` is a real guard.** A new command returns `-99` until the ceiling is raised.
- **Private data PERSISTS across VM calls; shared data is RE-SEEDED.**
- **A struct template is written only on the BOXED path**, so a struct wider than 65,535 bytes
  reaches it — about 8,300 `Word` fields.
- **On macOS `timeout` does not exist**; it is `gtimeout`.
- **`git push origin --delete` runs the full pre-push tier, once per branch.** Use one push naming
  every branch, or `gh api -X DELETE`, which skips the hook.

## METHOD RULES THIS LINE PAID FOR

- **A control removes the suspected cause and checks the failure survives.** It is the cheapest
  discriminator there is and it settled the `break` report in one probe.
- **A control runs in one direction only**, so a must-fire case and a must-not-fire case are both
  required.
- **Instrument rather than grep** when asking whether anything ever does X.
- **Verify the ref after a push**, not the gate output, and never through `tail`.
- **A guard refusing loudly is the guard working.** `-99`, `-222`, and a compiler rejecting
  `if <Word>` each surfaced a real gap as a refusal rather than a wrong artifact.
- **Assert WHICH failure fired**, not merely that one did.
- **A commit message is a claim.** One said six collectors were deleted; two remained.
- **PIN rather than repair when the change is a judgment call**, and say so in the source.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **The `analyze_class` catch-all**, above. Closing it changes a `match`, not a bound.
- **The `for` trailing-semicolon asymmetry.** `if`, `match`, and `loop` accept one; `for` does not.
  Accepting it widens the admitted language, so it is pinned rather than repaired.
- **`v0.2.3-prerebase-backup`**, local only, a deliberate pre-rebase safety copy. Do not delete it
  without being asked.
- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.** An availability failure at a
  trust boundary. Lowering the constant narrows the admitted language, so it is not changed
  unilaterally.
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- **`-255` is live and has no negative test.** Reaching it needs more than 16 KB of distinct name
  bytes; the corpus tops out at 7,680.
- **`bin` was raised, not fixed.** 49,152 covers `parse` at 1.47x; a stage half again as large breaks
  it.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Parallel development

`v0.3.0` carries native code generation on the same CI-gated workflow. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries. They hold
`src/wire_schema.rs` and `src/bytecode.rs` read-only and announce before widening; extend the same
courtesy. **Both items they were awaiting from this line are now answered.**

## Untracked artifacts a fresh session cannot see

`tmp/` is gitignored:

- **`tmp/2026-08-10-when_error_correction_meets_a_signature.markdown`** — research spike A373, 4.8 MB.
- **`tmp/a373/`** — the harvest pipeline.
- **`tmp/branch-prune-manifest-20260813.txt`** — the ONLY record of 73 deleted branches.
- **`tmp/branch-prune-manifest-20260815.txt`** — the record of the one branch deleted on 2026-08-15.
  Its substance is duplicated in the section below **because a manifest in a gitignored directory is
  a single point of failure**, which the 2026-08-13 entry already flagged and did not act on.

## Housekeeping settled on 2026-08-15, so it is not re-investigated

**`feat/selfhost-wire-data` is DELETED** (`35bd458f`, recoverable with `git branch <name> <commit>`),
and its worktree `keleusma-worktrees/wire-directory` is removed. The `v0.3.0` line had flagged it for
my decision. It was one commit ahead and 283 behind, touching only
`docs/decisions/WIRE_FORMAT_SELFHOST_PLAN.md`.

**Verified by reading the text, not by trusting `git cherry`.** Its `-` marker says "already
upstream", but the file still differed, because `v0.2.3` had moved. The check that settled it was
finding both of its claims present on `v0.2.3` — the `DEBUG_POOL` identification and the
struct-templates correction — **and a later refinement the branch does not have**. `git branch -d`
refused, as it should, because the commit is not an ancestor even though its content is. `-D` was
taken only after that verification.

**The `wire-corpus` worktree is KEPT, deliberately.** It is 4.4 GB and its HEAD `79fc97d1` is fully
contained in `v0.2.3`, so it looks like an obvious prune. It is not. Disk is at 186 GB free, the
cache represents hours of compilation, and the local gate's one remaining use is the pre-publication
`--miri` run. **Do not delete it to tidy up.** The `wire-corpus-*.log` files beside it are the
cost-model calibration record and are cited in the mailbox; they stay for the same reason.

**Everything else under `keleusma-worktrees/` belongs to the `v0.3.0` line** — `arena-composites`,
`gate`, `llvm-backend-spike`, `native` — as do every remaining local `feat/native-*` and
`feat/llvm-backend-spike` branch. Tell them apart by base branch, never by author. The two worktrees
under `projects/blog/tmp/` are not this project's.

**`v0.2.3` is 806 ahead of `main` and that is correct.** `main` holds releases, publication is HELD,
and merging to it is a release action rather than housekeeping.
