# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt. Unlike the three resume channels it is **not** kept
always-current, so it must be able to report itself stale rather than mislead a resuming agent.

> **REFRESHED 2026-08-20 (late) against `f091a668`**, every pinned value re-measured. **NINE COMMITS
> since the previous refresh, which was itself the same day** — this session ran long and the tree
> moved under its own handoff twice.
>
> **THE SESSION'S SUBJECT WAS NOT WHAT IT SET OUT TO BE, TWICE.** It began on Order 1 item 3 and spent
> most of its length on **five silent miscompiles** in the self-hosted compiler, four fixed and one
> specified-but-not-fixed. Read "WHAT THE SWEEP FOUND" and "THE THREE UNREACHED-CODE FINDINGS" before
> planning anything.
>
> **THE OPERATOR'S QUEUE IS THE FIRST THING TO CLEAR.** Two pull requests and two decisions are
> waiting, listed under "Open, held by the operator". The `ParsedFn` accessor decision is the one that
> unblocks the most.

## Validity

- **Branch**: `v0.2.3`, or a branch cut from it. If you are on `v0.3.0`, read
  `docs/process/handoffs/v0.3.0.md` and **do not overwrite this file**.
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

**Validate by ANCESTRY and by CONTENT, never by a hash match.** A stamp requiring `HEAD~1` to equal a
recorded parent is a claim that nothing else ever lands, and it has failed twice.

```sh
git merge-base --is-ancestor f091a668 HEAD    # must succeed

# Content. If ANY of these differ, say so rather than acting on the state below.
grep -c '^\s*#\[test\]' tests/selfhost_typecheck.rs         # 16
grep -c '^\s*#\[test\]' tests/selfhost_wire.rs              # 173
grep -c '^\s*#\[test\]' tests/selfhost_parse.rs             # 89
grep -c '^\s*#\[test\]' tests/selfhost_codegen.rs           # 139
grep -c '^\s*#\[test\]' tests/selfhost_declared_bounds.rs   # 5
grep -c '^\s*#\[test\]' tests/opcode_reachability.rs        # 2   (new this session)
grep -c '^\s*#\[test\]' tests/block_form_statements.rs      # 11
grep -c '^\s*#\[test\]' tests/consts_region_composition.rs  # 7
grep -c '^\s*#\[test\]' tests/operand_stack_model.rs        # 6

# `tests/stage_command_reach.rs` is NOT in this list on purpose: it lands with PR
# #210, which is open at the time of writing. If it exists, #210 merged.

# THE STAGE BOUNDS. `wire.kel` unchanged; `verify_depth.kel`'s cap is new and
# REPLACED A SILENT DROP.
grep -oE 'fn (nm_max_names|mi_max_nodes|fl_max_nodes|ck_max|highest_command)\(\) -> Word \{ [0-9]+ \}' \
    src/selfhost/kel/wire.kel     # 1024, 1365, 170, 90, and highest_command 181
grep -oE 'fn max_nesting\(\) -> Word \{ [0-9]+ \}' src/selfhost/kel/verify_depth.kel   # 32

# THE MARGIN PINS. Moved twice this session, both times for a NAMED reason.
grep -oE 'assert_eq!\(worst_(names|blob), [0-9]+' tests/selfhost_wire.rs   # 672, 35233

# THE PARSER'S CAPS. Unchanged; the token cap now binds only the COLLECTING feed.
grep -rhoE 'pub const PARSE_[A-Z_]+: usize = [0-9]+;' src/ | sort

# THE CONSTRUCT-SUPPORT BOUNDARY. **THE ENUM HAS FOUR VARIANTS, NOT THREE**: `Gap`
# split into `Refuses` and `Diverges` because it conflated an honest refusal with a
# silent miscompile. Expect 88 SOk / 1 Refuses / 5 Diverges / 1 RefRejects.
awk '/let cases: &\[\(&str, Support, &str\)\] = &\[/{f=1;next} f&&/^    \];/{f=0} f' \
    tests/selfhost_codegen.rs \
  | sed 's://.*::' | grep -oE '\b(SOk|Refuses|Diverges|RefRejects)\b' | sort | uniq -c
```

**A CHECK THAT PASSES IS NOT A CURRENT DOCUMENT.** IS NOT A CURRENT DOCUMENT.** The last one passed every check six merges after
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

**A CANCELLED CI RUN IS NOT A GREEN ONE, AND IT LOOKS LIKE ONE IN A SUMMARY.** The version-branch
run for `52cbb6c4` completed as `cancelled`, with no failure and nothing pushed over it for eight
hours; the cause is unknown and guessing at one would be worse than recording that. **The commit it
merged was 22/22 green on its own branch**, and the next version-branch run covered the same content,
so the gap closed -- but only because someone looked. Read the `conclusion` field, and treat anything
that is not `success` as unverified.

**VERIFY THE REF AFTER A PUSH, NOT THE HOOK OUTPUT.** `git ls-remote --heads origin <branch>`.
**Never pipe a push through `tail`** — it truncates the hook log, which happened twice in one session.

**WHAT "WITHOUT REBASING" PROTECTS.** The invariant is *merged at the commit CI ran* — do not move a
branch out from under a green result. It is not a ban on `git rebase`. If a sequential branch
conflicts, rebasing BEFORE its first push is safe (CI runs once, on the final commit); leaving it
conflicting produces **no CI run at all, silently**, which is the outcome the rule exists to prevent.

**A PULL REQUEST BASED ON A FEATURE BRANCH GETS NO CI AT ALL, SILENTLY.** Measured 2026-08-21.
`ci.yml` filters `pull_request` on the **base** branch (`main` or `v*`), so a stacked pull request
whose base is another feature branch triggers **no workflow**: no failure, no queue, no run to
read. `gh pr checks` reports "no checks reported", which is indistinguishable from a slow start and
was left unnoticed through two pull requests.

Re-targeting the base is not enough on its own -- a base change emits `edited`, which is not one of
the default `pull_request` types -- so the run must be provoked, by closing and reopening the pull
request (`reopened` IS a default type) or by pushing to it. **Prefer basing on the version branch
from the start** and describing the stack in the body; the diff is noisier and the verification is
real.

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
| `lexer` into `parse` | **FUSED**, one-token window, byte-identical |
| `parse` into `reconstruct` | **FUSED at function granularity, 3.4x to 41.1x residency** |
| **`wire.kel`** | **PARSES, 486 functions.** The last excluding cap is gone |
| **`parse.kel` failure modes named** | **THIRTEEN**, across **ELEVEN** guarded counters |
| shared-slot layouts | **nine copies collapsed to two definitions**, in `selfhost_host` |
| architecture | one binary, selectable phases -- see `../decisions/PIPELINE_THEN_MONOLITH.md` |
| construct-support boundary | **79 SOk / 4 Gap / 1 RefRejects**, 84 cases |
| operand-stack models | **agree on every one of the 66 opcodes**; the known list is EMPTY |

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

## WHAT THE SWEEP FOUND, AND WHY IT COULD NOT HAVE BEEN FOUND BY READING

**FIVE SILENT MISCOMPILES.** Four fixed; one specified and deliberately not fixed.

| construct | symptom | state |
|---|---|---|
| `true` / `false` | emitted `GetLocal(0)` where the reference emits `PushImmediate(1)` | **FIXED** |
| `x as Byte` | emitted `ByteToWord`; the target type was discarded at parse time | **FIXED** |
| `Named("Bool")` as a type | tagged as the boolean primitive; a false accept | **FIXED** |
| nested array LITERAL | outer composite sized 16 where the reference computes 32 | **FIXED** |
| nested array INDEX | `a[0][1]` — the second `[1]` parses as an **ArrayLit** | **SPECIFIED, NOT FIXED** |

**THE METHOD IS THE DELIVERABLE.** Compile small programs through both compilers and compare
**BYTES**, classifying THREE ways: identical, refuses loudly, DIFFERS. Only the third is dangerous; a
loud refusal is an honest gap. An ops-only comparison calls the string-literal case clean, and it is
not.

**WHY THE ORACLE WAS BLIND.** Self-hosting is validated by compiling the twelve stage sources
byte-identically. **Those sources use no boolean literal and no `Byte` cast**, so the oracle cannot
see either. Any construct the corpus does not contain is unverified BY CONSTRUCTION.

**PROPORTIONALITY, AND STATE IT EVERY TIME.** `self_hosted_compile` cross-checks ops, constant pool
and local count against the reference and refuses on divergence. **Every defect above gave a user a
loud error, never a wrong artifact.** The exposure is to direct callers of `self_host_compile`.
Omitting that sentence overstates any of these badly.

**THE INDEX DEFECT IS NOT TRUNCATION, contrary to this document's own earlier claim.** `parse.kel`
emits records and they are WRONG records. Chained indexing is unsupported: `ps.aa_phase` arms only
after a let-bound array `Local` and never re-arms. **`let b = a[0]; b[1]` diverges too**, so the chain
is not the trigger. A fix needs a binding record for an array-typed element, a nested-variant postfix
phase, and chain re-arming — a FEATURE, not a defect fix. The boundary carries the specification.

## THE THREE UNREACHED-CODE FINDINGS, WHICH ARE ONE CLASS

**PRESENCE, DISPATCH, AND EVEN AN ANNOUNCEMENT ARE NOT EVIDENCE THAT CODE RUNS.**

1. The `v0.3.0` line found **`Op::Reset` never lowered anywhere**, credited only because a CHUNK
   containing it lowered. A mutation crediting it moved their figure to 57 of 66 **with every test
   still green**.
2. **`Op::IsStruct` has no witness.** Emitted only when a scrutinee's type is unknown; nine
   constructs tried, none reaches it. **Recorded as "not found", NOT as unreachable.**
3. **Commands 176/177 (`fl_stream_begin`/`fl_stream_step`) are dispatched and driven by nothing** —
   written, dispatched, and announced to the other line. This **changes the cost of `CONSTS`**: the
   route is written but never executed, so taking it means writing the driver AND validating
   never-run code.

**The cheap check is to search for callers before costing work that depends on code.** That was
learned three times before being written down.

**`Op::Len` IS reachable** — an `if` expression as a `for`-in source — **and the witnessing program
cannot be given a memory bound**. `verify()` accepts it; `module_wcmu` refuses it, and the same
missing `Expr::If` case defeats both the static-length lookup and the bound extractor. Both arms at
length two are still refused, so it is the SECOND category of conservative rejection. On a language
whose value proposition is definitive WCET and WCMU, **"reachable" needed qualifying and both framings
are asserted**.

## THE MACRO POSITION## THE MACRO POSITION

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

## THE ONE DEFECT THIS SESSION KEPT FINDING

**I derived a set from the part of the system I was thinking about, rather than from the system.**
Six times, and it was always mine rather than the code's:

| what I derived | what it actually was |
|---|---|
| 2 local-binding arrays | **8** -- the trap did not move |
| the one array the chunk cap is named after | a family of **6**, plus **2** loop limits |
| one copy of the shared layout | **five**, and my test checked only the driver's |
| a guard walking `src/` and `tests/` | the class spans the repo; a LIVE copy in `compiler/` |
| `grep '#[test]' compiler/src/` -> "zero tests" | **86**, in `compiler/tests/` |
| a probe against the REFERENCE tokenizer | the cap governs the STAGE's lexer |

**THE FIX IS ALWAYS THE SAME: derive the set from the source, and assert the derivation is
non-vacuous.** Two of those assertions fired on their first run -- the family test found ZERO arrays
because the walk hit a `[` first, and the no-copies guard flagged itself -- so without them both
guards would have passed while checking nothing.

**Live examples to copy rather than reinvent**: `the_parse_guard_caps_match_their_arrays` (eleven
counters, families derived), `every_chunk_indexed_array_admits_the_chunk_cap`,
`no_other_file_restates_the_shared_layout` (walks the tree, asserts `compiler/` was reached).

## THE PARSER'S CAPS, ALL NAMED

Thirteen failure modes report their own cause. **Four groups shared a message before this**, which is
the defect the whole programme exists to remove:

| shared message | constructs that gave it |
|---|---|
| `IndexOutOfBounds(64, 64)` | local bindings, operator nesting |
| `IndexOutOfBounds(32, 32)` | parameters, `if` nesting |
| `IndexOutOfBounds(8, 8)` | `for` nesting, array-literal nesting, **call nesting** |
| `IndexOutOfBounds(256, 256)` | statements, enum variants |

Each group is held distinct by an encoded test. **Two bounds are WHOLE-PROGRAM totals whose array
size misleads**: enum variants (256) and data-block fields (512). 128 enums of two variants refuse
exactly where one enum of 257 does.

**THE GUARD IS ON THE POINTER AND EACH GUARDED ARRAY CARRIES ONE SPARE SLOT.** The write precedes the
increment, so a guard on the increment fires one write too late, and clamping at the last usable slot
would REFUSE the exactly-full program that parses today. **Do not "simplify" that away.**

**NAMING A CAUSE COSTS ABOUT THREE NAMES** -- an error code, a capacity, a guard. The programme has
spent 39 of the 1,024-name budget, leaving 65% margin at 666. The margin pin has moved SIX times and
**not once for a reason its author was thinking about**.

**SWEPT AND FOUND CLEAR**, so the next sweep skips them: data blocks and `use` declarations through
64, tuple elements through 32, array-literal ELEMENTS through 1,025 (a different quantity from
array-literal NESTING, capped at 8), integer-literal match arms through 128, pending statements past
40.

**WHEN A GENERATED PROGRAM FAILS, CONFIRM THE REFERENCE ACCEPTS IT** before concluding anything about
the stage. Five of my probes measured something other than what I intended: a token-count mismatch, a
call-argument confound (a call cannot exceed its callee's arity, so the parameter cap fires first), a
malformed nested `match`, a malformed else-if chain, and an enum-pattern `match` where the corpus only
ever matches integer literals.

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
- **AN ITEM IS ITS ATTRIBUTES AND DOC BLOCK, NOT ITS `fn` LINE.** Inserting a helper before
  `fn parse_functions_impl(` put it between `#[allow(clippy::type_complexity)]` and the function that
  attribute applies to. Clippy caught it; two further splices trying to repair it made it worse.
  **Restoring from `HEAD` and reapplying beat a third correction stacked on two bad ones.**
- **ROOT `cargo fmt --all` DOES NOT REACH `compiler/`**, which declares its own `[workspace]`. A gate
  covering four feature sets still could not see the file just edited. Anything touching `compiler/`
  needs a `cd compiler` pass, and CI runs `fmt --check`, `clippy --all-targets -D warnings`, `test`
  there.
- **READ THE FEATURE MATRIX OUT OF `ci.yml`, NOT FROM MEMORY.** Publishing a constant from a module
  gated on `self-host` broke three CI jobs while every local check passed, because every local check
  had that feature enabled. Four `cargo check --tests` runs, about a minute:
  `--no-default-features`, `--features signatures`, `--features self-host`, `--features signatures,shell`.
- **A GUARD WITH A SCOPE NARROWER THAN ITS CLASS IS THE DEFECT IT PREVENTS.** The no-copies guard
  walked `src/` and `tests/` and missed a live fifth copy in `compiler/src/main.rs`.
- **A mechanical transform applied by pattern needs the compiler to confirm it** — a regex rebinding
  `&vm` missed every multi-line form; clippy found six.

## Open, held by the operator

**FOUR ITEMS. TWO PULL REQUESTS AND TWO DECISIONS.** Nothing else is blocked on the operator, and the
unblocked queue is genuinely thin.

**1. THE `ParsedFn` ACCESSOR DECISION — the one that unblocks the most.**

**THREE read-only accessors**, not the six an earlier revision of this file claimed. `ParsedFn` has
nine private fields and four public accessors; `tests/selfhost_codegen.rs` needs `name` (read 71
times), `param_types` (4) and `return_type` (2), and carries a full copy of the struct AND the driver
because it cannot reach them.

**THE DUPLICATE IS EVIDENCED, NOT ARGUED.** It has cost four separate things:
- blocks stage two of the token-residency work;
- required the boolean-literal shared slots to be seeded a second time;
- **is the compiler the construct-support table actually measures**;
- **has measurably diverged from the shipping compiler** — on a string literal the reference and the
  copy both emit `StaticStr("hi")` while `keleusma::selfhost::self_host_compile` emits `Int(3)`.

So the support table reports `Ok` for a construct the SHIPPING compiler gets wrong. Pinned by
`the_two_self_hosted_compilers_disagree_on_a_string_literal`.

*Options*: widen the three accessors and delete the copy (recommended); widen them gated on
`self-host`; or leave it and accept that the table describes a compiler that is not the one shipping.

**2. TWO PULL REQUESTS OPEN AGAINST `v0.2.3`:**
- **#201**, one clause in the `CHANGELOG.md` V0.2.0 entry that CONTRADICTS another statement in the
  same release section. Flagged for review because editing historical release text is a judgment call
  rather than a repair. One clause, trivially revertible.
- **#210**, pinning that commands 176/177 are dispatched and unreached. Its guard is TEXTUAL — it
  greps driver source for two function names — which is the shape argued against all session, and it
  passes vacuously if those functions are renamed. Stated in the pull request rather than hidden.

**3. THE DEAD `native@1c1ffb1e` GATE RECORD.** `scripts/gate-status.sh` reports it STALLED at 34%,
step 5, zero failures, **last write over 227 hours ago**, with its predecessor marked ABANDONED and
"a wait on it will never end". No process runs against that worktree and it is clean. The `v0.3.0`
line has confirmed nothing of theirs waits on it. Untouched because it is their worktree.

**4. THE RULINGS OF 2026-08-19 ARE ALL IMPLEMENTED OR RECORDED. Do not re-ask them.**

## WHAT A RESUMING SESSION SHOULD DO FIRST

**Clear the operator queue before starting anything**, because two of the four items change what the
tree means rather than what it does.

Then, if development continues, the honestly-costed options are:

- **`CONSTS`, Order 1 item 1.** The flattener already emits a byte-identical region and batching is
  the route, BUT commands 176/177 have never run — budget for validating them, and drive them from a
  test first so the stage side is proven independently of the driver.
- **Chained array indexing.** Specification is in the boundary table; it is a parser feature.
- **`Op::IsStruct` reachability.** Nine constructs tried and recorded. The target is making inference
  FAIL, and note that the trick which works for `Op::Len` does NOT work here — necessary but not
  sufficient.

**Do not resume by sweeping for more miscompiles.** The yield fell from two in twenty probes, to one
in twenty-two, to zero; the shallow layer is worked out.

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
