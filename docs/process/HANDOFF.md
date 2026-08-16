# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt. Unlike the three resume channels it is **not** kept
always-current, so it must be able to report itself stale rather than mislead a resuming agent.

> **Rewritten whole, 2026-08-16**, not patched. A handoff that contradicts itself is worse than a
> stale one, because a reader cannot tell which half to trust. Overwrite this file; do not append.

## Validity

- **Branch**: `v0.2.3`, or a branch cut from it. If you are on `v0.3.0`, read
  `docs/process/handoffs/v0.3.0.md` and **do not overwrite this file**.
- **Written**: 2026-08-16, describing the tree at `e5c1afbe`.
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint. It governs
  documentation, commit messages, code comments, and anything drafted for publication.

**Validate by ANCESTRY and by CONTENT, never by a hash match.** A stamp requiring `HEAD~1` to equal a
recorded parent is a claim that nothing else ever lands; the previous one failed its own check while
its contents were still true.

```sh
git merge-base --is-ancestor e5c1afbe HEAD    # must succeed

cargo test --features compile,verify --test block_form_statements    # 8 passed

# The construct-support boundary, found by its own declaration rather than by
# line number, because a hardcoded range is the defect this file warns about.
awk '/let cases: &\[\(&str, Support, &str\)\] = &\[/{f=1;next} f&&/^    \];/{f=0} f' \
    tests/selfhost_codegen.rs \
  | sed 's://.*::' | grep -oE '\b(SOk|Gap|RefRejects)\b' | sort | uniq -c
# expect: 4 Gap, 1 RefRejects, 79 SOk
```

Strip comments before counting. The aliases are `SOk`, `Gap`, `RefRejects` — only the first carries
the `S` — and a previous session reported three wrong numbers extracting them by hand. **If the
counts differ, say so rather than acting on the state below.**

## Derive numbers; do not copy them forward

This project has been bitten by stale figures **seven** times now, most recently a roadmap cell
reading "125 tests" against a file holding 163, and the `395,804` that invented a dependency between
two pieces of work.

```sh
grep -c '^\s*#\[test\]' tests/selfhost_wire.rs      # the wire differential
git log --oneline -1 v0.2.3                          # where the version branch is
gh pr list --state open                              # in flight, BY BASE BRANCH
gh run list --branch v0.2.3 --limit 1                # is the tip verified
```

## On resume, before doing anything

1. **Read `secret/notes/APPENDIX_B.md`.**
2. **Read the other line's mailbox**: `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.
   No wake; poll at increment boundaries. **Read it to the end** — the item you already know about is
   usually not the one that matters. Reading only as far as a familiar heading is how a live
   `Vm::set_breakpoint` panic sat unnoticed.
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three channels:
   [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest
   first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).**

## THE WORKFLOW: CI GATES FEATURE BRANCHES

**Do not run `scripts/release-gate.sh` to gate a merge.** Operator decision, 2026-08-11. CI is a
verified strict superset and runs in ~48 minutes against ~2h30m.

1. Cut the feature branch **as the first action**, and `git status` before committing.
2. **Cut sequential branches ONE AT A TIME.** `DESIGN_JOURNAL.md`, `REVERSE_PROMPT.md` and
   `TASKLOG.md` are prepended to by every increment, so two branches cut in parallel conflict by
   construction. See "WHAT 'WITHOUT REBASING' PROTECTS" below.
3. Verify locally as you go; the gate's invocations are in `.cargo-husky/hooks/pre-push`.
4. Push, open a **draft PR to `v0.2.3`**, merge on green **at the commit CI ran**.

**VERIFY THE REF AFTER A PUSH, NOT THE HOOK OUTPUT.** `git ls-remote --heads origin <branch>`.
**Never pipe a push through `tail`** — it truncates the hook log, which happened twice in one session.

**WHAT "WITHOUT REBASING" PROTECTS.** The invariant is *merged at the commit CI ran* — do not move a
branch out from under a green result. It is not a ban on `git rebase`. If a sequential branch
conflicts, rebasing BEFORE its first push is safe (CI runs once, on the final commit); leaving it
conflicting produces **no CI run at all, silently**, which is the outcome the rule exists to prevent.

**A default-feature run is not the gate.** `cargo test --workspace` and `--features compile` both miss
`self-host`. The gate is a five-entry feature matrix.

**`ci.yml` now supersedes pull-request runs.** Grouped on `github.ref` for a pull request and on the
unique `run_id` otherwise, so branch verification runs are untouched. Verified by execution: a second
push cancelled run `31932202253` and `31932359730` replaced it.

## THE STATE

Tip `e5c1afbe`, CI green, tree clean, nothing of this line open.

| | |
|---|---|
| construct-support boundary | **79 Ok / 4 Gap / 1 RefRejects**, 84 cases |
| the driver | wired to a `Module`; builds its own input via `selfhost::module_input` |
| stage seed accessors | **five**, public under `self-host`, one encoding shared with the driver |
| `analyze_class` / `analyze_opk` | **exhaustive over `Op`**; a new opcode fails to build until classified |
| residency staging | **not needed** — worst stage `parse` at 627 names / 33,395 bytes vs caps 1024 / 49,152 |

## THE MACRO POSITION, WHICH IS FURTHER OFF THAN RECENT INCREMENTS SUGGEST

**V0.2.x completes when the five success criteria in
[`../roadmap/V0_2_X_ROADMAP.md`](../roadmap/V0_2_X_ROADMAP.md) hold. None do.** Even **Order 1**, the
first of six milestones, is not met, and two things block it:

1. **The self-hosted path emits TWO region kinds, not the artifact.** `wire_names_via_kel` is the only
   driver emit entry and the byte-identity check covers `[kind::NAMES, kind::STRING_POOL]`. The schema
   defines about twenty. Everything landed recently — the module-input encoding, the interning
   producer, the caps — feeds those two. **This is the largest single gap and it is invisible from the
   increment titles.**
2. **Self-hosted type rejection is started, not done.** `tests/selfhost_typecheck.rs` holds 7 tests
   against a plan sizing the obligation at roughly 15 rejection shapes.

**The roadmap's Order 1 cell is itself stale**: it states "`tests/selfhost_wire.rs` is 125 tests"
against a file holding 163, and lists as remaining several items that are done (the child-position
constant walk; `wire.kel` in `read_stage`; residency staging). Correct it before sizing from it.

## OPEN CORRECTNESS ITEMS, HIGHEST FIRST

**1. `wcmu_region` reports 2 where both peak models and the emitter say 3.** Reported by the `v0.3.0`
line on `06_multiheaded::classify` and `rogue_bestiary::corpse_fill`. **An UNDERSTATED bound on
shipped chunks, which is the one thing this project sells.** Same family as `manhattan_norm`, but the
accessor repair cannot reach it: **neither chunk contains a `GetField`**. They eliminated the emitter
and their own harness pairing by measurement; what remains is the bound. **What they could not
establish, and it is inside our function**: why `wcmu_region` returns 2 when the same peak model
walked by hand reaches op 18 and returns 3 — its `If` arm recurses and `Op::Return` falls through the
catch-all rather than terminating the walk. Start there.

**2. `Op::Yield`'s peak-model net.** `stack_growth` 0 / `stack_shrink` 1 gives net −1;
`verify::op_depth_effect` gives `(1, 0)`, net 0, above a comment saying the resume pushes the input
back. The walk goes negative: `analyze::main` and `verify_depth::main` reach −1 at `PopN(1)`.
Confirmed on both trees; `v0.3.0` measures 8 of 958 chunks. **Derive the corrected pair from the
virtual machine, not by analogy with the `GetField` repair**, and note that on a small yielding chunk
both models return peak 3 — **a peak that agrees is not evidence the net is right.**

**3. The control that cannot reach either.** `the_peak_model_agrees_with_the_depth_model` compares the
two models over five hand-written cases, none of which yields. **Its coverage is a property of its
case list, not of the opcode set.** The fix is a check RANGING OVER `Op`, not another case — adding a
case closes one instance and leaves the next invisible, which is exactly how `Yield` survived a repair
made the day before.

**4. `Op::cost()` disagrees with measurement.** Two findings pinned, not repaired. Only 17 opcodes of
66 were ever measured; every other emitted value is an unchecked bucket assignment.

## THE META-DEFECT THIS LINE KEEPS FINDING

**A suite whose coverage is a property of its case list, mistaken for a property of the thing under
test.** Four instances in two days: the enum intern mode, the constant-name branch, the peak/depth
control, and — on the other line, the same day — a green `Trap` observable whose subjects emit no
`Op::Trap` at all. **In every case the code was reachable and the evidence was not, and in every case
a mutation or a corpus walk found what green did not.** When a comment states a property, check that
the suite beside it tests that property; twice it did not.

## FACTS THAT COST REAL EFFORT

- **CHECK A FIGURE AGAINST THE THING IT CLAIMS TO MEASURE.** `395,804` is a `CONSTS` region record
  count; read as a name count it made a 2.5x problem look like 1500x **and invented a dependency**
  between the interning producer and residency staging. A wrong figure does not merely misstate a
  size.
- **A COUNT OF ERRORS IS NOT A COUNT OF DEFECTS.** "Four unresolved doc links" was three plus
  rustdoc's aggregate line, counted by `grep -cE "^error"`.
- **THE PLAN IS NOT THE TREE.** Five items across two days were listed as remaining and were already
  done. Check each against the code before building.
- **A JOB BUILDING THE UNION OF FEATURES CANNOT CATCH A FEATURE-GATED REFERENCE** — both gates are
  satisfied, so the link resolves. Only a lean set reports it; three had accumulated behind that.
- **A diagnostic names where the parser stopped, not what it objected to.** The `break` report was a
  stray `;` after a `for` block.
- **APPEND TO A SLOT-ADDRESSED BLOCK, NEVER INSERT.**
- **`emit_at` is at EIGHTEEN arms**, the measured parse-depth ceiling for that shape in the harness.
- **`highest_command()` is a real guard**; a new command returns `0 - 99` until the ceiling rises.
- **Private data PERSISTS across VM calls; shared data is RE-SEEDED.**
- **On macOS `timeout` does not exist**; it is `gtimeout`.

## METHOD RULES THIS LINE PAID FOR

- **Mutate to test a control.** Green proves nothing about a branch no case reaches.
- **A control removes the suspected cause and checks the failure survives.**
- **Assert WHICH failure fired**, not merely that one did.
- **Say what a green suite does NOT establish**, in the source, where a reader will meet it.
- **Instrument rather than grep** when asking whether anything ever does X.
- **PIN rather than repair when the change is a judgment call**, and say so.
- **A mechanical transform applied by pattern needs the compiler to confirm it** — a regex rebinding
  `&vm` missed every multi-line form; clippy found six.

## Open, held by the operator

- **Publication remains HELD.**
- **The `ci.yml` concurrency FORM.** The `v0.3.0` line asked three times for
  `group: ${{ github.workflow }}-${{ github.ref }}`. What landed scopes cancellation to pull requests,
  to preserve per-tip branch verification. **They have not answered the mailbox note**, and the edit
  is one line if they or the operator prefer the simpler form.
- **The `for` trailing-semicolon asymmetry**, pinned; widening is the operator's call.
- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.**
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- **`-255` is live and has no negative test**; the corpus tops out at 7,680 distinct name bytes.
- **`v0.2.3-prerebase-backup`**, local only. Do not delete without being asked.
- **MSRV**: CI checks 1.85 for `keleusma-arena`, 1.88 for `keleusma`.

## A NOTE ON THE `/goal` MECHANISM, IF THE OPERATOR USES IT

It is a Stop hook judged by a model **against the session transcript, not the tree** — every finding
across a dozen iterations quoted prose, never a file. Consequences worth knowing:

- **Conditions must be state-based.** Anything about ordering or process (branch structure, merge
  sequence) can become permanently unsatisfiable and loop.
- **Do not embed a literal artifact** unless that exact text is mandatory; a condition containing both
  a snippet and permission to deviate will be read as requiring the snippet.
- **Shorter is safer.** Every sub-clause is another thing to fail on.
- **Candour is penalised** — honest self-reports become the evidence cited against completion. Record
  evidence in the tree, where it survives; keep the self-assessment for the operator.

## Parallel development

`v0.3.0` carries native code generation. Their mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries. They hold
`src/wire_schema.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/verify.rs` and `.github/workflows/`
read-only and announce before widening. **Extend the same courtesy, and announce BEFORE landing on a
shared file, not simultaneously** — that was got wrong on `ci.yml` this session.

Owed to them: nothing outstanding. Owed by them: an answer on the concurrency form.

## Untracked artifacts a fresh session cannot see

`tmp/` is gitignored:

- **`tmp/2026-08-10-when_error_correction_meets_a_signature.markdown`** — research spike A373, 4.8 MB.
- **`tmp/a373/`** — the harvest pipeline.
- **`tmp/branch-prune-manifest-20260813.txt`** and **`-20260815.txt`** — the only record of deleted
  branches, and the substance of the second is duplicated in this file's history for that reason.
