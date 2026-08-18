# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt. Unlike the three resume channels it is **not** kept
always-current, so it must be able to report itself stale rather than mislead a resuming agent.

> **Refreshed 2026-08-18** against the merge at `3f3735d2`, with every pinned count re-measured. The
> session that produced it converted the last two stages to coroutines, removed every emit-side cap,
> fused the lexer into the parser, and recorded the selectable-phase architecture. **Read
> `../decisions/PIPELINE_THEN_MONOLITH.md` before touching the pipeline**; it holds one open fork for
> the operator and the enumeration of whole-input facts that any boundary format depends on.
>
> **Previously refreshed 2026-08-17** against the merge at `81ddd260`, with every pinned count re-measured
> rather than carried forward. The state, macro position, correctness items and operator-held list
> changed; the workflow, method rules and hard-won facts below did not and were left alone. Four
> operator decisions are now RULED ON and two are DONE — read that section before asking anything.
>
> **Rewritten whole, 2026-08-16 (second rewrite that day).** The previous one was stamped six merges
> back and **passed all of its own validity checks while being wrong about every open item** — it
> named a repaired bound as the top concern and said the emit path covered two region kinds when it
> covered four. A document that certifies its own currency and is wrong is the worst case this file
> exists to avoid. Overwrite; do not append.

## Validity

- **Branch**: `v0.2.3`, or a branch cut from it. If you are on `v0.3.0`, read
  `docs/process/handoffs/v0.3.0.md` and **do not overwrite this file**.
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

**Validate by ANCESTRY and by CONTENT, never by a hash match.** A stamp requiring `HEAD~1` to equal a
recorded parent is a claim that nothing else ever lands, and it has failed twice.

```sh
git merge-base --is-ancestor 3f3735d2 HEAD    # must succeed

# Content. If ANY of these differ, say so rather than acting on the state below.
grep -c '^\s*#\[test\]' tests/selfhost_typecheck.rs         # 12
grep -c '^\s*#\[test\]' tests/selfhost_wire.rs              # 172
grep -c '^\s*#\[test\]' tests/selfhost_parse.rs             # 69
grep -c '^\s*#\[test\]' tests/block_form_statements.rs      # 11
grep -c '^\s*#\[test\]' tests/consts_region_composition.rs  # 7
grep -c '^\s*#\[test\]' tests/operand_stack_model.rs        # 6
grep -oE 'fn highest_command\(\) -> Word \{ [0-9]+ \}' src/selfhost/kel/wire.kel   # 181

# THE FIVE CAPS. Four are gone; the numbers that remain are LIVE BOUNDS.
grep -oE 'fn (nm_max_names|mi_max_nodes|fl_max_nodes|ck_max)\(\) -> Word \{ [0-9]+ \}' \
    src/selfhost/kel/wire.kel        # 1024 names, 1365 nodes, 170 flattener, 90 chunk batch
grep -n 'chunks: \[Word;' src/selfhost/kel/parse.kel   # [Word; 256] -- excludes `wire` at 475

awk '/let cases: &\[\(&str, Support, &str\)\] = &\[/{f=1;next} f&&/^    \];/{f=0} f' \
    tests/selfhost_codegen.rs \
  | sed 's://.*::' | grep -oE '\b(SOk|Gap|RefRejects)\b' | sort | uniq -c
# expect: 4 Gap, 1 RefRejects, 79 SOk
```

**A CHECK THAT PASSES IS NOT A CURRENT DOCUMENT.** The last one passed every check six merges after
it was written. If the counts hold but the dates below are old, read the three channels first and
trust them over this file.

## Derive numbers; do not copy them forward

**Bitten SEVEN times now**, most recently by a comment in `wire.kel` that governed a design decision
while citing region offsets an order of magnitude wrong.

```sh
git log --oneline -1 v0.2.3
gh pr list --state open                  # BY BASE BRANCH; the other line's appear here too
gh run list --branch v0.2.3 --limit 1
```

## On resume, before doing anything

1. **Read `secret/notes/APPENDIX_B.md`.**
2. **Read the other line's mailbox**: `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.
   No wake; poll at increment boundaries. **Read it to the end.**
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

| | |
|---|---|
| ALL TWELVE STAGES | **`loop main(...)` coroutines** |
| emit path | **11 of 11 stages**; every emit-side cap removed |
| `verify_types.kel` | **streams**, one row per resume, asserted by resume COUNT |
| `wire.kel` | SEVEN streaming commands; the rest answer in one yield |
| `lexer` into `parse` | **FUSED**, one-token window, byte-identical on four stages |
| architecture | one binary, selectable phases -- see `../decisions/PIPELINE_THEN_MONOLITH.md` |
| construct-support boundary | **79 Ok / 4 Gap / 1 RefRejects**, 84 cases |
| auxiliary body | **103,544 bytes** across eleven stages, down from 712,936 |
| stages fitting one 65,536-byte window | **11 of 11**, where three did not |
| chunk region emitted by the driver | **9 of 11 stages**, up from 7 |
| module-driven emit path | **four region kinds** of twenty, and they differ in strength |
| operand-stack models | **agree on every one of the 66 opcodes**; the known list is EMPTY |
| type rejection | **rules COMPLETE**; the stage RESOLVES names, not just compares tags |
| ill-typed corpus | **20** cases, 7 well-typed controls, both guards raised |
| `analyze_class` / `analyze_opk` | exhaustive over `Op`, and in `selfhost_host` so there is ONE copy |

**WHAT EACH EMITTED REGION OWES TO WHOM, because the distinction is the coverage claim.**
`NAMES` and `STRING_POOL` are **computed** — the stage walks the module blob and derives every byte.
`CHUNKS` is **mixed per field**: the stage computes the name index from its own interner and the
three range cursors by accumulation; ten fields per record come from the host. `HEADER` is **encoded
but not derived**. A region whose payload came from the harness or the reference is **not covered**.

**THREE DIFFERENT LIMITS, AND CONFLATING THEM IS HOW THE LAST STALE COMMENT HAPPENED.**

1. **Artifact offset past the 65,536 buffer** — GONE, twice over. Regions are emitted at window
   offset zero and placed by the host, AND the all-default elision took every stage's whole artifact
   under the buffer. `parse` is 39,216 bytes where it was 304,432.
2. **Chunk records past one batch of 90** — `parse` has 94 and `wire` 475. The only limit still
   excluding a real stage.
3. **TWO NODE CAPS, AND THEY ARE DIFFERENT CAPS.** I conflated them once and told the other line
   their figure was wrong when it was right; retracted in `50d949ab`. The **module-input walk**
   refuses past **1,024 nodes** (`nm_max_names`, error `-240`), which `wire.kel` hits at 1,148 chunk
   constants. The **flattener out of `wire.fin`** refuses past **170**, `fin` being 1,024 words at six
   words a node. Only the second is derived from a word count.

## THE MACRO POSITION

**V0.2.x completes when the five success criteria in
[`../roadmap/V0_2_X_ROADMAP.md`](../roadmap/V0_2_X_ROADMAP.md) hold. None do.** Order 1 needs:

1. **`CONSTS`, and BOTH RECORDED OBSTACLES TO IT WERE WRONG.** The interning-order conflict is
   **unreachable** for this corpus: the flattener interns only for `StaticStr`, `Struct` and `Enum`,
   and every corpus constant is `Int`. Pinned by `the_flattener_interns_no_name_for_any_stage`. The
   figures were wrong too — 645,312 measured against the 663,120 recorded, and all of it is now
   historical, because eliding the all-default initialiser pool removed 85% of the body. **What
   remains is the 170-node flattener cap**, needing about five batches for `parse` rather than a
   hundred and three. Derive figures from `tests/consts_region_composition.rs`, never from prose.
2. **The remaining region kinds**, which are the same shape as `CHUNKS`. Re-measure their sizes
   before sizing work from them: every figure recorded for them predates the elision. **`STRUCT_AUX`
   and `ENUM_AUX` are EMPTY in all eleven stages** — a byte identity for either passes while emitting
   nothing, and the reason is the same census as item 1: both are written only for `Struct` and `Enum`
   constants, and there are none.
3. **The type checker's INPUT.** Its rules are complete and its resolution is now in the stage, but
   the extraction is still Rust walking the REFERENCE parser's AST. Structure is available from
   `parse.kel` plus `reconstruct.kel`; **do not invent a second encoding.**

## OPEN CORRECTNESS ITEMS

**1 and 2 ARE CLOSED.** `Op::Yield`, `FixedMul` and `FixedDiv` are all repaired against the virtual
machine handlers, and the ranging check's known-disagreement list is **empty**: the two operand-stack
models agree on every one of the 66 opcodes. `Yield` was the unsound one — it accounted for the pop of
the yielded value and not for the resume pushing the reply back, so a bound understated by one value
slot per preceding yield. **Confirmed independently by the `v0.3.0` line**, which registered its
prediction before merging: chunks reaching negative operand depth went 8 to 0, every offender a stream
`main` whose `PopN(1)` went under. Bounds RISE for yield-bearing chunks, which is a changelog-visible
consequence.

**3. `Op::cost()` disagrees with measurement.** `OPCODE_SPECS` holds 17 entries covering **16 distinct
opcodes of 66**, so 50 carry estimates. Worst-case execution time is the project's headline claim, so
this is the largest gap between what is asserted and what is measured. **Operator's ruling: close it
sometime after Order 1.**

**4. Derived operands in type rejection.** A field read, an index or an arithmetic result is still
UNKNOWN and therefore accepted. Pinned by `the_rules_still_do_not_reach_a_derived_operand`.
Reaching them is a fixpoint, not a lookup.

## FIVE CAPS, FOUR REMOVED, AND HOW THEY WERE FOUND

| bound | status |
|---|---|
| artifact size against the 65,536 window | **gone** -- the all-default elision, 6.9x smaller body |
| chunk batch, 90 records | **gone** -- streaming; the carries existed because a function cannot remember |
| constant flattener, 170 nodes | **gone for a scalar forest** -- a composite needs the queue, and the queue IS the residency |
| module-input node walk, 1,024 | **gone** -- the guard was measuring the NAME arrays; the node table holds 1,365 |
| `toks.chunks`, 256 entries | **STANDS.** `wire.kel` cannot be PARSED at 475 functions |

**FOUR OF THE FIVE WERE FOUND BY SOMETHING OTHER THAN LOOKING FOR THEM.** The last surfaced while
measuring residency for an unrelated increment. That is an argument for measuring AROUND work rather
than only at it.

**Raising `toks.chunks` is a real fix and a separate increment**: `base` and `at` were appended after
it, so widening shifts them, and a mid-block insertion silently shifting every later field has already
broken four tests once.

## DIAGNOSTICS IN `parse.kel` POINT AWAY FROM THEIR CAUSES

Twice in one session, in one file, each costing real time:

- **`LoopLimitExceeded`** for a full chunk table. A reader investigates loop bounds, not the 257th
  function in their program.
- **`IndexOutOfBounds(-1, 64)`** for an unprimed token window. `packed` is 40,960 words; **64 is
  `opstack`**. The wrong token drove the shunting yard into draining an empty operator stack thousands
  of steps later, so the index named neither the array at fault nor the cause.

**Treat this as its own increment rather than adding a guard each time you trip over one.** Both were
diagnosed wrongly on the first attempt because the message was taken at face value.

## THE META-DEFECT THIS LINE KEEPS FINDING

**A suite whose coverage is a property of its case list, mistaken for a property of the thing under
test.** **SIX instances now**: the enum intern mode, the constant-name branch, the peak/depth
control, the other line's `Trap` observable, the WCMU corpus, and the type corpus — where every one
of sixteen ill-typed cases placed its operands as literals, so a rule that could only see literals
looked complete. **In every case the code was reachable and the evidence was not.**

## FACTS THAT COST REAL EFFORT

- **A GUARD THAT CANNOT FIRE IS WORSE THAN NONE.** I wrote one comparing `directory.len()` against
  the stage buffer; that length is the SHARED ARRAY's size, 65,536 for every module, so it was false
  by construction. **Before adding a check, construct the input that makes it fire.**
- **CHECK A FIGURE AGAINST THE THING IT CLAIMS TO MEASURE.** `395,804` was a `CONSTS` record count
  read as a name count and it INVENTED A DEPENDENCY between two unrelated pieces of work.
- **A COUNT OF TESTS IS NOT A COUNT OF SHAPES.** "7 tests against ~15 shapes" was repeated in four
  documents; the rules were complete and I nearly rewrote them.
- **THE PLAN IS NOT THE TREE.** Five instances, three of them mine.
- **A DUPLICATE WITH A STRUCTURAL CAUSE RETURNS UNLESS THE CAUSE IS REMOVED.** The drifted class
  table existed because the consumer could not reach the original.
- **THE DISPATCH CHAINS HAVE A PARSE-DEPTH CEILING** and it presents as a STACK OVERFLOW in the test
  binary, not a parse error. `dispatch_emit` hit it at twenty arms, `dispatch_driver2` at twenty-two.
  Split the group rather than hunting the ceiling.
- **`wire.fin` IS 1024 WORDS AND ITS USERS OVERLAP.** Chunk records take 0..990 at eleven each; the
  header rides 990..1001. `parse`'s 94 chunks overran it and silently rewrote the header.
- **APPEND TO A SLOT-ADDRESSED BLOCK, NEVER INSERT.**
- **`highest_command()` is a real guard**; it has moved 167 → 173 and a new command returns `0 - 99`
  until it moves again.
- **Private data PERSISTS across VM calls; shared data is RE-SEEDED.** Every region of one artifact
  must therefore be emitted in one call, or the host must place windows itself.
- **The interner is a PURE FUNCTION of its input**, so a re-walk is the same answer rather than a
  second one. Rely on that instead of carrying state between calls.
- **On macOS `timeout` does not exist**; it is `gtimeout`.
- **VERIFY A PUSH BY THE REF, NEVER THE HOOK OUTPUT.** A push printed "all checks passed" and did not
  land, on a dropped SSH connection.

## METHOD RULES THIS LINE PAID FOR

- **CUT THE FEATURE BRANCH BEFORE THE FIRST EDIT, not before the commit.** Demonstrated, not
  hypothetical: `be296d89` (the streaming chunk emit) went straight onto `v0.2.3` because the previous
  task had left the session on the version branch and nothing forced a branch before `wire.kel` was
  opened. Knowing the rule is not the same as having a habit that enforces it, and the enforcing habit
  is positional. Not reverted -- rewriting published history on a branch the `v0.3.0` line rebases
  from is worse than the violation -- so CI on the version branch was the only gate that change got.
  It passed 22 of 22, which is luck rather than process.
- **NEVER CLASSIFY A STATE AS FAILURE BY EXCLUSION. Enumerate the terminal failure states, or do not
  classify at all.** This is the narrow form of a rule that was written too broadly and therefore did
  not work: an earlier revision said "check what the source emits before filtering on it", the defect
  was fixed in a CI monitor, the lesson was written down -- **and then reproduced verbatim an hour
  later in the shell version of the same wait.** Fixing the instance is not fixing the habit, and a
  rule you cannot mechanically apply is a rule you will re-break.
  The working form: a wait that counts the literal string `pending` and stops at zero reads no field
  and assumes nothing. A filter saying `!= SUCCESS && != SKIPPED && != NEUTRAL` calls every running
  job a failure.
- **FIVE CONSTRUCTED STATUSES IN ONE SESSION, none caught by reading the code.** Every one was caught
  by output whose SHAPE did not match the expectation, which is the argument for instruments that
  show raw evidence over ones that show a verdict.
  | claimed | true |
  |---|---|
  | gate ran, exit 0 | `timeout` does not exist on macOS; it never executed |
  | `echo "CLIPPY OK"` | clippy was failing; the echo was unconditional |
  | `\| tail -1; echo $?` gave 0 | that is the PIPE's exit, not clippy's, which was 101 |
  | monitor: 2 jobs failed | 0 failed, 2 still running |
  | shell: 4 jobs failed | 0 failed, 4 still running -- the same bug again |
- **A test that measures a VERDICT cannot tell a streaming stage from a one-shot fold behind a
  coroutine shell.** Eleven verdict tests passed either way; only the resume count discriminated.
- **A REFUSAL PROVES WHICH LIMIT FIRED ONLY IF THE TEST NAMES THE ONE IT EXPECTED.** Third near-miss
  of the session: `wire` refusing `-240` was read as the chunk batch cap when it is the 1,024-node
  module-input walk.
- **Mutate to test a control.** Green proves nothing about a branch no case reaches.
- **A control removes the suspected cause and checks the failure survives.**
- **Assert WHICH failure fired**, not merely that one did.
- **Say what a green suite does NOT establish**, in the source, where a reader will meet it.
- **Instrument rather than grep** when asking whether anything ever does X.
- **PIN rather than repair when the change is a judgment call**, and say so.
- **A mechanical transform applied by pattern needs the compiler to confirm it** — a regex rebinding
  `&vm` missed every multi-line form; clippy found six.

## Open, held by the operator

**FOUR OF THESE ARE NOW RULED ON. Do not re-ask them; the ruling is the answer.**

- **Publication remains HELD.** Reaffirmed 2026-08-17.
- **`CONSTS` representation** — *ruled: Option A, elide the zeros, and no `BYTECODE_VERSION` bump,
  because no version-2 artifact has ever been published.* **DONE**, merged at `81ddd260`.
- **`Op::cost()`** recalibration — *ruled: close it sometime after Order 1.*
- **Derived operands in type rejection** — *ruled: before publishing V0.3.0.* Whether a V0.2.x version
  ships before V0.3.0 is itself undecided.
- **The Japanese FAQ entry** is stale and renders as English — *ruled: correct eventually.*
- **The `for` trailing-semicolon asymmetry** — *ruled: accept it, shape A.* **DONE**, merged at
  `1f0e5e19`. Both parsers implement the empty statement and agree byte-identically.
- **A WINDOWED COMPILER, in the Turbo Pascal sense, is a stated goal.** Measured: ten of the twelve
  stages are already `loop main(resume) -> Word` coroutines stepped one yield at a time, and bounded
  working set is forced by the language rather than by discipline. `wire.kel` and `verify_types.kel`
  are the two that are not. **I claimed a windowed verifier was blocked because a bound needs a whole
  chunk's control-flow graph; the operator challenged it and was right.** The analysis is a fold over
  a well-nested bracket structure with a stack of depth equal to the nesting level. What is not
  forward-only is the IMPLEMENTATION: the walk jumps on `Loop(target)`, the `If` arm peeks at
  `ops[target - 1]` for an `Else`, `trace_const_set_local` scans BACKWARD for a bound constant, and
  `loop_body_advances_induction` scans FORWARD to the body tail. Each has a bounded-state streaming
  equivalent — decide at `EndLoop` rather than at `Loop`, and carry a slot-to-last-constant map.
  **BOTH FORMERLY-OPEN QUESTIONS ARE NOW ANSWERED** by the `v0.3.0` line, by measurement over 985
  chunks in 64 modules, in their handoff rewrite (PR #159).
  * **The break fold holds.** 0 of 386 loops carrying a break disagree on operand depth, guarded by a
    must-fire control on a synthetic unbalanced loop. The assumption was safe.
  * **Nesting depth still has NO static cap, and 19 is NOT one.** The deepest observed is 19, in
    `parse.kel::body_step`. Their warning is the load-bearing part and is repeated here verbatim in
    spirit: **that is what the corpus contains, not a bound on what the language admits, and it must
    never be offered as one.** A verifier written in Keleusma needs a DECLARED cap with programs past
    it rejected, not a number read off today's sources.
  * They also warn against a second walker: one written independently for the break question invented
    its own `If`/`EndIf` handling and reported 365 of 386 loops disagreeing. `EndIf` RESTORES the
    depth saved at its `If` rather than restoring and then applying its own effect. **Do not compete
    with a validated walker.**
- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack.**
- **`CHANGELOG.md:340`** states the checked-arithmetic push order wrongly in published text.
- **`-255` is live and has no negative test.**
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
read-only and announce before widening. **Extend the same courtesy.**

**NEITHER OF US IS A RELIABLE NARRATOR ABOUT THE OTHER'S CODE, and we now have three instances.**
They reported `reconstruct` blocked on a `pub` when `parse_functions` was already public; I reported
`analyze.kel` free of a defect it had in three places; they retracted an inflated coverage figure
whose instrument called a seeded-but-unrun module non-vacuous. **Check the claim against the code
before acting on it, especially when it says someone else must act.**

Owed to them: nothing outstanding. Owed by them: nothing outstanding.

## Untracked artifacts a fresh session cannot see

`tmp/` is gitignored:

- **`tmp/2026-08-10-when_error_correction_meets_a_signature.markdown`** — research spike A373, 4.8 MB.
- **`tmp/a373/`** — the harvest pipeline.
- **`tmp/branch-prune-manifest-20260813.txt`** and **`-20260815.txt`** — the only record of deleted
  branches, and the substance of the second is duplicated in this file's history for that reason.
