# Reverse prompt

## Indexed-write handlers, 2026-10-21

Branch feat/v02-indexed-write-handlers, cut from feat/v02-order3-audit-followup
at 89aaba69, which merged into v0.2.3 as 3e4f4984 through pull request 484.

Both compilers now implement the index-first bare write and the single-index
handled write, per the operator rulings recorded in
docs/decisions/ORDER3_HANDLER_SURFACE_PLAN.md. The self-hosted compiler matches
the reference byte for byte for Word and Byte elements and refuses other
element types. The plan document's "Self-hosted mirror" section records the
mechanism, the capacity measurements and the residue.

Verification. A frozen copy of the tree ran the five feature sets in release.
The tree then changed only in the roadmap's boundary count and in this file.

| Feature set | Result |
|---|---|
| `--features self-host` | 3185 of 3186 passed |
| `--workspace` | 3444 of 3445 passed |
| `--no-default-features` | 331 of 331 passed |
| `--features signatures` | 2447 of 2447 passed |
| `--features signatures,shell` | 2464 of 2464 passed |

The one failure in both self-host and workspace was the guard requiring the
roadmap to state the boundary table's counts. The three new `data/` cases moved
them to 100 SOk over 105 cases. The roadmap now says so, and the guard was
rerun and passes. Clippy for the workspace and for self-host printed Checking
lines and is clean, and fmt is clean. The stages' self-compilation passes in
release with the index-first writes.

Also opened this session, on separate branches from v0.2.3.

- Pull request 485 cleans the library under no-default-features clippy.
- Pull request 486 derives the trap classifier's per-tag expectation from the
  Rust analysis, completion-claim item 13.
- Branch fix/v02-cast-byte-tracking fixes the cast divergence described in the
  plan document. Seven boundary cases diverged without it.
- Commit 7e37e81a on v0.2.3 records the CI receipt in both completion audits.

Concerns.

- Release is not debug. The local matrix ran in release for time. CI runs debug,
  where overflow checks and debug assertions are active, so the pull request's
  CI run is the debug evidence.
- The parse stage holds 1023 of the type stage's 1024 names. Any further parse
  feature must fold into existing functions, as this one did, or the name
  channel must be staged.
- `checked_open` now decides between a handled write and a handled read by the
  operator depth recorded at `=`. That rule is exercised by the parenthesized
  read case and the nested-block case. A deeper nesting shape that this does
  not anticipate would mis-attach the arms, and the cross-check would then
  refuse the program rather than emit a wrong module.

Next. Handled multi-index writes, then native-call handlers and handler-required
composite reads, then FixedDiv totalization.

## Opus 5.5 audit of the Codex V0.2.X work, 2026-10-20

Line v0.2.3. Branch feat/v02-order3-audit-followup, cut from
feat/v02-order3-trap-analysis at 36ebe64d, which it extends and should be merged
back into. The audit covered the 65 commits from origin/v0.2.3 at 89fc6aac,
covering the rest of Order 1, Order 2 and partial Order 3.

## Audit verdict

Facts established by direct inspection or by three read-only reviews.

- BYTECODE_VERSION is 2, FORMAT_FINGERPRINT is 0x4327_63E1 and the opcode count
  is 66 at both ends of the range. Test functions grew from 1448 to 1865 names.
  Nineteen names present at 89fc6aac are gone. Each was renamed or replaced in
  the same commit and file, mostly where a pinned limitation was closed and its
  test inverted. No silent deletion was found. An earlier count of removed
  test attributes reported one, because a rename keeps its attribute line.
  No Appendix B content appears in the diff or in any commit message.
  Every commit carries the Codex trailer except 3fefccc0, which has none.
- None of the 65 commits has run in remote continuous integration. No pull
  request exists for the branch.
- The last full Codex matrix, at 09e74c94, failed in all five feature sets. The
  repairs in 53b88f52 were confirmed only by targeted runs. That matrix and the
  handoff commands used `--release`, while continuous integration runs debug,
  where overflow checks and debug assertions are active.
- Proven execution had no concrete soundness defect, and its certificate is
  bound to the executed image and cannot be forged. It did depend entirely on
  verify::op_depth_effect matching each opcode, unchecked by any test.
- Six existing opcodes changed meaning in place under an unchanged version.
  These are Div, Mod, GetDataIndexed, SetDataIndexed, BoundsCheck and GetIndex on
  flat arrays. This matches the operator's intent that only Trap traps.
- The completion claims are broadly correct with caveats. The caveats are
  listed in docs/decisions/COMPLETION_CLAIM_STRENGTHENING.md.

## Operator rulings, 2026-10-20

First-party, recorded verbatim in docs/decisions/ORDER3_HANDLER_SURFACE_PLAN.md.

- The project rejects undefined behaviour and admits platform-specific
  behaviour. Keep a real length check on the fast path and add the tests.
- Only Trap traps. Host-contract faults and indexing forms outside the first
  pass report failure as an outcome, and the compiler defaults to a trap.
- Indexed-write handlers, native-call handlers and handler-required composite
  reads are in Order 3 scope, from grammar to code generation.
- A processor-style flags register replaces in-band outcome flags in V0.2.4.
- BYTECODE_VERSION and the fingerprint stay frozen until the next public
  version. A bump will follow evidence of traction.

## Done in this increment

- Operand-length checks restored in src/vm.rs. No unsafe operand access remains
  on the proven path.
- src/vm_operand_effect_tests.rs compares observed stack effects with the table
  for 37 source programs and 13 hand-built subjects, with exact opcode coverage.
- STANDARD.md and STRUCTURAL_ISA.md corrected. ORDER3_PROVEN_EXECUTION.md and
  the command-line help describe retained checks.
- The misplaced stage-count doc comment in tests/claimed_counts.rs moved to its
  test and no longer states a present-tense count of eleven.
- tests/comment_citations.rs failed two tests at 36ebe64d, because a source
  comment and four historical HANDOFF.md passages cited renamed tests. The
  comment now names the current test. Each historical passage keeps the old
  name and points to its replacement. Both tests pass.
- ORDER3_HANDLER_SURFACE_PLAN.md and COMPLETION_CLAIM_STRENGTHENING.md written.
  ORDER3_COMPLETION_PLAN.md records the scope resolution.

## Verification of this increment

Run in the worktree at ../keleusma-worktrees/v023-audit. Receipts are under the
ignored tmp/opus-audit-20261020 directory of the primary checkout.

| Check | Result |
|---|---|
| New test, default features and self-host | 1 passed each |
| Mutation of one net entry and one pop requirement | the new test failed both times, then the table was restored |
| proven_execution, trap_policy, indexed read and write outcomes, operand_stack_model, default features | 11, 9, 8, 6 and 6 passed |
| proven_execution and trap_policy with signatures and encryption | 12 and 11 passed |
| keleusma-cli proven_execution | 5 passed |
| claimed_counts | 13 passed |
| comment_citations, self-host | 6 passed, against 4 passed and 2 failed at 36ebe64d |
| proof_elision and verify_execution unit tests | 9 passed |
| `cargo clippy --workspace --all-targets -D warnings`, the CI command | passed with Checking output |
| `cargo fmt --check` | passed |

The full five-configuration matrix was NOT run on this branch. A CI-shaped debug
matrix ran on the unmodified 36ebe64d tree. Its workspace run took 14,901
seconds and passed 3425 of 3430 tests. The other five failed on all three
attempts. No-default-features, signatures and signatures,shell passed, with
their documentation tests.

The five failures came from 53b88f52. It added two functions to wire.kel, so
three chunk-count pins moved, and it pushed wire.kel two rows past the type
stage's 640-row name-occurrence channel. The second effect is a capacity
regression. A follow-up commit removes three redundant helpers rather than
raising any capacity. See the TASKLOG entry. The two capacity tests pass after
it, and the binaries that read wire.kel were rerun in debug.

## Concerns

- **Continuous integration time.** The test job sets no timeout, so GitHub's six
  hour default applies on a four-core runner. The debug self-host tests may not
  fit. This is a hypothesis until a remote run measures it.
- **FixedDiv.** Source Fixed division emits a bare FixedDiv, which raises
  DivisionByZero without a Trap. The analysis reports it as unsupported, so no
  unsound verdict results. The handler plan includes its totalization.
- **No-default-features clippy** fails on a pre-existing only-used-in-recursion lint
  in src/bytecode.rs. Continuous integration does not run that configuration.

## Intended next step

Pull request 484 into v0.2.3 was opened at the operator's request. Its first
CI run failed the book drift step and timed out both long Test jobs at six
hours. The catalogue is regenerated, and on operator direction the two long
jobs now run as four hash partitions each. The next CI run decides whether the
shards fit. The local self-host matrix later finished with the same five
failures, which the wire.kel commit fixes. The
handler surface implementation follows the plan document. Its acceptance rows
are listed there. Codex or Claude may take it.

---

# ► APPENDED BY THE V0.3.X LINE (worktree `arena-composites`, branch `v0.3.0`)

**This section is an addendum, not a rewrite.** Everything above is the `v0.2.3`
line's own channel and is untouched.

> ⚠ **THIS LINE RECORDED A COMMITMENT TO TOUCH NONE OF `REVERSE_PROMPT.md`,
> `DESIGN_JOURNAL.md` OR `TASKLOG.md`, AND BROKE IT.** Absorption 60 conflicted on
> this file because both lines overwrite it wholesale — 106 lines against 1988 —
> and that will recur at every absorption while both write here. The `v0.3.0` line
> has its own channel at `docs/process/handoffs/v0.3.0.md`. **Where should this
> line's outgoing reports live?** Appending is the least destructive answer
> available without a ruling; it is not proposed as the permanent one.

> AI to human communication. Overwritten each increment.

## Line

V0.3.X, worktree `arena-composites`, branch `v0.3.0`.

> **Absorption 57 resolved a conflict here by KEEPING THIS LINE'S MESSAGE.** The `v0.2.3` line writes
> its own reverse prompt in the same path, and absorbing is one-directional, so a merge must discard
> one of the two. This branch's copy carries this branch's message; theirs is on `origin/v0.2.3` at
> the same path, untouched.

## TEN DEFECTS, AND EACH HID BEHIND AN INSTRUMENT'S SHAPE

That is the finding rather than the count. Every one sat where some checker's keying could not see
it, and the checkers were right about what they measured.

**Five in the lowering.** A local live across a `yield`, wiped by the entry preamble on every resume.
A composite written to a data slot stored as the body's ADDRESS. A composite slot read before it was
written answering 0 where you fault. A declared `= literal` initializer never applied, because you
apply `private_init` at load and I have no load step. A depth-disagreement `assert_eq!` — a panic on a
public entry point that does not require a verified module.

**Two in arithmetic.** Checked `Byte` multiply and add returned UNTRUNCATED values: `200 * 100` gave
20000 where you give `Byte(32)`. Your `Byte` arm is not your integer arm at a narrower type, and the
FLAG was wrong too — the integer classifier asks whether the result left the 64-bit range, which a
byte product never does, so every overflow took the `ok` arm.

**Three in my own harnesses**, one of which reached the gate as a SIGSEGV. A buffer sized by a literal
fails by corrupting its neighbour, which is not a stable observable: it passed alone, passed under the
narrow configuration, and died in the gate.

## TWO REFUSALS WERE GAPS, AND ONE WAS HELD BY MY OWN REASON

The checked fixed-point multiply and divide are lowered now. The divide stayed refused on the ground
that it reaches for the compiler-runtime symbol __divti3 — **which I wrote myself, and which does
not distinguish**: the census at `native_codegen/tests/linkage_symbol_census.rs` measures `Fixed`
division as the one construct in its sweep needing such a symbol, and the bare `FixedDiv` already
lowers.

> ⚠ **TWO CORRECTIONS TO THE PARAGRAPH ABOVE, BOTH MADE BY YOUR OWN INSTRUMENT, 2026-09-16.** It
> read *"your linkage_symbol_census"*, with the name in backticks. **The census is MINE** — it lives in `native_codegen/`,
> which is this line's package entirely — so this section credited its own work to you, in the
> channel whose purpose is to keep the two lines' claims apart. And both names were written as bare
> backticked identifiers that your `comment_citations` scan resolves against the ROOT tree, where
> neither exists, because it does not walk the detached package. **A citation only I can check is
> not a citation**; they are written as a path and as plain prose now, so your scan can see what
> they are. **And the first version of this very correction failed your scan again**, by quoting the
> old wording with its backticks intact — the note describing the defect reproduced it. **A cost already paid by supported
code cannot justify refusing more of it.**

## ⚠ FIVE QUESTIONS STILL WITH YOU, ALL RE-MEASURED 2026-09-12

Not recalled. `outstanding_reports.rs` fails if any stops reproducing and says to retract rather than
to debug.

1. **A `confine.rs` index panic on a truncated op stream.** Still fires; watched by
   `lowering_robustness.rs`, which allows it by origin file and asserts it has not gone.
2. **A multi-parameter stream faults after its first rewind** — `TypeError` on `Int and Unit`, after a
   RESET, so slot 1 is still left undefined.
3. **The write-before-read check is flow-insensitive**: the unconditional shape rejected, the
   conditional one accepted, and the fault arriving at run time. Intended, or should the compiler
   reject the conditional shape too? **Either answer is implementable here; I assumed the first.**
4. **ANSWERED in absorption 82, 2026-10-22, from the runtime side.** Bare `Op::Mod` and `Op::Div`
   became total with a Fixed arm, so the program below now completes, and no opcode was added. This
   backend lowers the matched Fixed pair to the same result. The watch is now
   `report_four_is_answered_and_the_answer_holds`. The original report follows unchanged.
   **The type checker admits `Fixed % Fixed`, and the virtual machine then refuses it at run
   time.** `fn main(a: Fixed, b: Fixed) -> Fixed { a % b }` compiles, verifies, loads, and traps with
   `TypeError("cannot modulo Fixed by Fixed")`. Your `Op::Mod` arm handles `Int`/`Int`, `Byte`/`Byte`
   and `Float`/`Float`; `Fixed` falls to the catch-all, and `Op::Div`'s arm is equally Fixed-less. The
   operand types are statically known, so **a static type error is reaching run time**. `*` and `/`
   have `FixedMul` and `FixedDiv`; `%` has no Fixed-aware opcode and falls back to the plain one.

   **I am not asserting which repair is right.** Rejecting `%` on `Fixed` in the type checker, and
   adding a Fixed-aware modulo, are both coherent; the second needs an opcode, which the minimal-ISA
   constraint disfavours. It was watched from both sides by a reproduction test, so whichever way
   you ruled, this side would fail loudly rather than keep a stale report, and it did.

   **This is how I found it, and it is the part worth your attention.** My differential harness
   *panicked* on any virtual-machine outcome that was not `Finished`, so a trapping cell could not be
   represented in it at all. The backend had been returning `4.0` for `200.0 % 7.0` — arithmetically
   right, and not what you produce. It now refuses. **An instrument can only find defects it has a
   vocabulary for**, and mine had no word for "the reference refuses".
5. **NEW — `DataSlot` carries no scalar kind, so a private slot's declared type is not in the module.**
   `SharedSlotLayout` carries a kind tag; `DataSlot` carries a name and a visibility. That asymmetry
   is invisible to the reference, which keeps a tagged `Value` at run time and never needs the
   declaration. It is not invisible here: a native backend has only the module.

   Concretely, `private data d { v: Fixed }` followed by `d.v % d.v` traps on your side and returns a
   number on mine, and **I cannot close it** — I can see the slot is eight bytes and not that it holds
   a scaled value. Every other route by which a `Fixed` value reaches `Op::Mod` is now refused; this
   one was pinned open by a test that failed when it closed. **Since absorption 82 it no longer
   diverges**: your side completes it, and the integer remainder mine lowers has the same low bits,
   so `the_private_slot_route_agrees_without_the_kind` now asserts agreement. The slot's kind is still
   absent from the module, which is why report 5 stays open.

   **I am not asking you to add a field.** If question 4 is settled by rejecting `Fixed % Fixed` in the
   type checker, this route closes with it and nothing else is needed. I am recording that the private
   slot layout carries strictly less type information than the shared one, because **the next thing
   that needs a declared scalar kind will hit the same wall**, and the asymmetry looks unintentional
   rather than decided.

   The fail-closed alternative on my side — refusing `%` and `/` on any operand of unknown provenance —
   would refuse ordinary `Word` remainders on private-slot values, which your implementation runs
   correctly. I judged the coverage loss worse than the recorded residual. Say if you disagree.

## AND ONE THING YOU FIXED THAT NOTHING HERE RECORDED

`verify_types.kel`'s `cmd` slot — declared, documented, never read. **You acted on that report and
explained why the slot stays** (it sits at slot 0, and removing it would shift the seeding). Nothing
on this side said so for weeks. An unacknowledged repair is an un-retracted report with the sign
flipped, and I had one of each.

## WHAT I GOT WRONG

- **I claimed no capability work remained. Four times that was false.**
- **I was about to file a mechanism I had not verified**, citing a test that measures a population
  rather than semantics.
- **My own completion conditions passed a lowering carrying two defects.**
- **A prediction inherited a stale tree** and I recorded the falsification rather than editing it.
- **Six instruments now fire on drift in code. None fires on drift in prose**, and four records were
  found outliving their subjects — including one asserting YOUR analysis was unsound four weeks after
  you repaired it.

## ⚠ SIX — A DISCLOSURE ABOUT A FIGURE **I** PUBLISH, NOT A DEFECT IN YOURS (2026-09-17)

**This is a quantification, not a defect claim, and nothing here is broken.**
Over-provisioning errs in the safe direction, the reservation involved is fixed and static so a
worst-case bound moves by a known constant, and its stated rationale — that a predicate disagreeing
with the lowering's own would be a worse defect than unused bytes — stands. **The code is MINE**: it
lives at native_codegen/src/region.rs, in this line's detached package. I am telling you because your
hosts would consume the figure.

**The figure.** My backend publishes `host_arena_supplement_bytes`, the arena bytes a host must ADD
beyond `auto_arena_capacity_for` to run a module natively. Measured two ways:

- **Statically, across the corpus** by native_codegen/tests/region_composition.rs: **30 modules have
  a stream chunk on the entry path, and every one reserves the same flat block** of `MAX_STACK * 8` =
  512 bytes. That reservation is **at least half the published figure for 27 of the 30**, running from
  27% up to **98% for the self-hosted compiler stages**, which plan almost nothing else. Re-measured in
  absorption 82: the two added stages `wire_debug.kel` and `wire_schema.kel` are the whole change,
  each at 98%. Before it the figures were 28 modules and 25 of 28. The
  decomposition is checked against the published total rather than assumed, so the reading of the
  planner is verified.
- **Dynamically** by native_codegen/tests/arena_high_water.rs and
  native_codegen/tests/stage_differential.rs: three hand-written streams touch 16, 24 and 48 bytes of
  520-552 byte plans, and **the twelve stages touch ZERO** of 520-600. Instrument reach was proven
  before the numbers were believed — poking one byte into the region moves the measurement.
- **Dynamically across the CORPUS** by native_codegen/tests/streaming_arena_touch.rs, added
  2026-09-23: **15 of the 30 streaming modules are driven** since absorption 82, which added two
  stages that both drive and touch zero bytes; it was 13 of 28 before. The stages confirm ZERO by a
  second route, and `14_frame_log.kel` is **the first corpus module outside the stages ever measured
  this way — 48 bytes of a 600-byte plan, 8%.** The figure was checked stable across two independent
  runs before being published here.
  **The other 15 are NOT measured, and they are named rather than dropped**: eleven need host natives
  registered (`host::song_name`, `host::run_player_turn`), three take a **`Composite`** first argument, and
  one my backend refuses to lower — the yield-escape hazard already dispositioned. Reaching them
  needs the stub machinery in my corpus differential, which I declined to extract rather than
  disturb a tuned instrument. **So read this as 15 of 30, not as the corpus.**

**Why it may matter to you.** A host sizing an arena from that figure provisions roughly twenty times
what these streams use, and for the compiler stages provisions a region nothing writes. Whether that
is acceptable is an arena-accounting question **which that function's own documentation already says
belongs to the operator**, and I am not proposing a change: shrinking the reservation means making the
planner's predicate agree with the lowering's, which is the trade-off the current design deliberately
declined.

**What this does NOT establish.** The static half is about plans, the dynamic half about particular
modules on particular inputs. **Neither bounds what an arbitrary program would touch**, and no
worst-case claim should be read out of them.

Watched by the report-six test in native_codegen/tests/outstanding_reports.rs, which re-derives the figures rather than
trusting this text. If they move, this section is wrong and should be re-measured or withdrawn.



### V0.3.X handoff repair checkpoint of 2026-09-28

Absorption 78 incorporates the upstream guard repair and preserves all existing
reports above. The bounded V0.3.X brief now defines the resume procedure and the
required evidence. Read its archive and the machine-written native gate record
for the current state. This checkpoint does not close the outstanding reports or
start the nested-yield increment.


## V0.3.X review follow-up, 2026-09-28

Codex prepared `fix/v3-handoff-residuals` from `20decc75`, absorbing upstream `cb91b987`. Reporter exit status now reflects unverified evidence, and six-column provenance is parsed correctly. The diagnostic catalogue records the fifteenth occurrence. The operator-confirmed gate cadence applies. See `docs/process/handoffs/v0.3.0-BRIEF.md` for targeted checks and outstanding integration verification. The previous native addendum and upstream current block remain preserved.


## V0.3.X gate-boundary resolution, 2026-09-28

Codex absorbed upstream `e3634edb` into the feature branch. The operator requires full gates before publication or merging into `main`. Version-branch integration depends on green remote continuous integration and relevant targeted local checks. Historical full native records remain unchanged. The bounded brief now records the resolved policy rather than leaving the merge-cadence question pending.


## V0.3.X legacy re-review corrections, 2026-09-28

Codex absorbed upstream `89fc6aac`. The bounded brief explains the expected stale full-gate report before its first-action command. The reporter distinguishes valid recorded failure from malformed evidence, and the fixture suite has an independent test entry. The archive stamps completed absorption 80 at `aa4db05f`, with absorption 81 described separately through its rehearsal and merge receipt. The original native addendum remains preserved. The bounded brief records verification outcomes. No publication or merge into `main` is requested.


## V0.3.X integration receipt, 2026-09-28

Pull request 472 merged into `v0.3.0` as `2e4a5d36` after run `36518497064` succeeded on `b8269a86`. The final candidate had 27 successful checks, one skipped check and none pending or failing. The merge tree matches that candidate exactly. No push interrupted that final run. The bounded brief now includes the deferred absorption-81 pointer and the completed integration status. The original native addendum and upstream current block remain preserved. Historical full native gates remain stale and are not represented as current verification.


## V0.3.X conditional yield increment and review, 2026-09-29

Codex revalidated the existing lowering and strengthened its regression in
`ed212340`. All 25 lexer yields are conditional tails. The review follow-up
shortens the bounded brief, archives receipts and relocates the worktree under
`keleusma-worktrees/` for remote review. No lowering or next increment is added.
See the V0.3.X brief and archive for verification and integration status.


## V0.3.X roadmap sequencing correction, 2026-09-29

Codex selected the next bounded Workstream B slice from the roadmap rather than
requiring another operator ruling. The bounded brief remains 66 lines. A stale
absolute corpus claim in the historical codegen probe is corrected. This follow-up
adds no lowering and is separate from pull request 473 while its checks run.


## V0.3.X bytecode retcon increment, 2026-09-29

Codex implements scalar bytecode coroutine lowering with LLVM retcon, arena
frames and host start, resume and release. Nine new tests pass in both float
configurations. The initial broad phase found seven maintenance-guard failures,
now corrected and checked individually. Full native gates remain outstanding.
Pull request 473 is merged. Workstream B remains active, with delegated suspension
next. Resume from the bounded V0.3.X brief. Upstream current-state material is preserved.


## V0.3.X delegated coroutine increment, 2026-09-29

Codex extends LLVM retcon lowering to nested reentrant calls and guarded heads.
The default broad native phase passed 694 tests with frozen inputs. Fourteen
coroutine tests and both lint configurations pass. Full gates remain outstanding.
The branch is `feat/retcon-delegation`, stacked on pull request 474. Continue from
the bounded V0.3.X brief. Workstream B remains active, with nested stream callees
and composite suspension values still refused. Upstream material is preserved.


## V0.3.X nested stream increment, 2026-09-29

Codex extends LLVM retcon to nested streams and corrects narrow-float private
initialisation. Eighteen coroutine tests pass in both float configurations. The
frozen narrow broad phase passed 699 tests. Cleared non-Unit parameter reads and
differing delegated yield signatures remain refused based on virtual-machine checks. The branch is `feat/retcon-stream-calls`, stacked on
pull request 475. Workstream B remains active at the composite suspension
boundary. Complete gates remain outstanding. Resume from the bounded native brief.


## V0.3.X coroutine reply-type correction, 2026-09-29

A Byte reply produced a native value where the virtual machine faults, in both
direct and delegated yields. `fix/retcon-reply-types` adds finite scalar-tag
admission across calls, locals, branches and loop back edges. Four regression
functions cover the correction. All 22 coroutine tests and both lint modes pass.
Pull request 474 is merged. Keep 475 and 476 as
drafts until this correction integrates. Workstream B and complete gates remain
outstanding. Resume from the bounded native brief. Upstream material is preserved.


## V0.3.X coroutine inspection correction, 2026-09-29

Pull request 477 also corrects enum inspection of scalar replies. Converged type
facts fold false inspections and exclude impossible payload arms from emission.
Three regression functions cover scalar categories, payload arms, enum and struct
controls and the explicit mixed-receiver refusal. All 25 coroutine tests and
all-target lint checks pass in both float configurations. Complete native gates remain
outstanding. Workstream B remains active at the composite suspension boundary.
Resume from the bounded native brief. Upstream material is preserved.


## V0.3.X flat coroutine yield increment, 2026-09-29

`feat/retcon-composite-yields` adds flat body outputs and exact yield and return
extent checks. The host reads each body before the next resume or release.
Three regression functions cover direct and delegated bodies and incorrect
extent metadata. All 28 coroutine tests and all-target lints pass in both float
configurations. Composite inputs, replies and live operands remain refused.
Full gates for the preceding correction continue on frozen `15b96c2e` in
`retcon-reply-types`. Workstream B remains active. Resume from the bounded native
brief. Upstream material is preserved.


## V0.3.X live body operands, 2026-09-29

`feat/retcon-live-bodies` admits body operands beneath direct and delegated
suspensions while preserving virtual-machine aliases and existing confinement
checks. Three added regression functions pin lifetimes, private aliasing and
negative boundaries. All 31 coroutine tests and all-target lints pass in both
float configurations. Composite host inputs and replies remain unimplemented.
Both complete gates passed on frozen `15b96c2e`. The same tree is now the combined
head of pull request 475, with remote checks running. Keep that head unchanged.
Workstream B remains active. Resume from the bounded native brief. Upstream
material is preserved.


## V0.3.X stable arena handles, 2026-09-29

`feat/retcon-stable-handle` stores the current continuation inside a bounded
caller-owned slot through provisional start, resume and release entry points.
Existing VM differentials now exercise both interfaces. Two new tests cover
independent slots, release, reuse and C linkage. All 33 coroutine tests and
all-target lints pass in both float configurations. Composite host inputs and replies
remain outside admission. Pull request 475 continues remote checks unchanged.
Workstream B and combined native gates remain outstanding. Resume from the
bounded native brief. Upstream material is preserved.


## V0.3.X bounded flat host values, 2026-09-29

`feat/retcon-flat-inputs` copies host-derived bodies at bounded value transfers
while preserving private-data aliases. Its eight new tests cover buffer reuse,
delegation, nested packed fields, frame-resident yields and refusal boundaries.
All 41 coroutine tests and all-target lints pass in both float configurations.
Both complete gates passed on clean, frozen `360d3295`. The flat-input changes
still need complete native gates. Pull request 475 has two remote test jobs
running and its head remains unchanged. Workstream B remains active. Resume from
the bounded native brief. Upstream material is preserved.


## V0.3.X mutation registrations and integration, 2026-09-29

The default gate on `d487eb07` failed only on three stale mutation registrations.
Their original zero-result perturbations now place again, with checked recipe
replacements. The failure record is preserved. No lowering code changed in this
correction. Complete native gates still need rerunning. Pull request 475 merged
as `ff124db9` after green remote checks, with its tree identical to the tested
head. The canonical version worktree is synchronized. Pull requests 478 through
481 remain stacked. Workstream B remains active. Upstream material is preserved.


## V0.3.X Workstream B verification, 2026-09-29

Both complete native gates passed on clean, frozen `5a0fec1d`. The bounded
sub-coroutine implementation is verified. Pull request 478 tracks consolidation
of 478 through 481 and version integration after green remote checks. Preserve
the combined head during those checks and compare its tree with the merge.
The 64-line native brief holds the resume sequence and admission boundaries.
Upstream material is preserved.


## V0.3.X completion cycle of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion` in the prescribed sibling
worktree. The full Workstream B objective remains active. This increment adds
reentrant completion, general entry arity and bounded owned completion storage.
All 46 coroutine tests and test-target Clippy pass in both float configurations.
The [bounded native brief](./handoffs/v0.3.0-BRIEF.md) now records completed
integration in past tense. Resume from the [acceptance matrix](../decisions/WORKSTREAM_B_COMPLETION.md),
starting with explicit reply and yield contracts. Full gates and integration are
not claimed. Upstream current state and preceding native reports are preserved.


## V0.3.X dialogue completion cycle of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`. Per-yield contracts and
bounded site metadata now handle differing direct and delegated dialogue types.
An undersized reply regression exposed an accepted field overread, now closed by
read and callee-argument extent checks. All 53 coroutine tests and test-target
Clippy pass in both float configurations. Continue with the remaining admission
audit in the [acceptance matrix](../decisions/WORKSTREAM_B_COMPLETION.md). Full
Workstream B, final gates and integration remain open. Upstream state is preserved.


## V0.3.X ownership and reachability cycle of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`. Native snapshot and
instance-borrow results now have explicit lifetime contracts. Unused helpers no
longer impose coroutine contracts. Route and C linkage tests pass, including
independent instance completion. All 58 coroutine tests and test-target Clippy pass
in both float configurations. Continue with runtime kind tracking for the mixed
receiver case in the [acceptance matrix](../decisions/WORKSTREAM_B_COMPLETION.md).
Full native gates and integration remain open. Upstream current state is preserved.


## V0.3.X runtime kinds and extents of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`, following `61646c36`.
Mixed enum receivers now carry runtime kinds, and successful tests refine payload
reads only while their receiver bindings remain unchanged. Owned mixed values
retain their selected size inside bounded storage. Field reads use a minimum
extent, array bounds use the actual selected size, and Reset clears metadata.

All 66 coroutine tests and test-target Clippy with warnings denied pass in both
float configurations. The default non-corpus suite passes all 747 tests. This is
one broad phase, not a complete gate. The original broad run exposed metadata
underflow on malformed input and a census which mistook admission facts for
emission sites. Both are fixed. The documented upstream confinement-panic
exception remains unchanged. All 13 targeted root documentation checks pass.

The refusal audit found another valid source case. A private Boolean protects a
nested stream parameter so it is read only on the first iteration. The virtual
machine yields 5 then zeros, but native admission rejects the parameter read.
The tracked reproducer and [acceptance matrix](../decisions/WORKSTREAM_B_COMPLETION.md)
name that implementation gap. Complete gates, the final audit and integration
remain outstanding. Upstream current state and previous native reports are preserved.


## V0.3.X cleared-value consumers of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`, following `1fda7a04`.
Nested stream parameters may now be read on paths where their original values
remain valid. Bounded kind checks at typed consumers trap on cleared Unit values.
Equality and enum inspection preserve their valid Unit behavior. Equality also
retains floating-point NaN and signed-zero semantics. Native fault subprocesses
must execute the expected prefix before the trap.

All 71 coroutine tests pass in both float configurations. Test-target Clippy
with warnings denied passes in both. The default non-corpus native phase passes
all 752 tests with a frozen verdict. The 13 targeted root documentation checks
pass. These results do not constitute either complete native gate.

A further valid source case remains refused. Guarded tuple construction from a
nested parameter encounters a joined Word-or-Unit width. The tracked reproducer
and the [acceptance matrix](../decisions/WORKSTREAM_B_COMPLETION.md) identify it
as an implementation gap. Continue with value-driven composite packing. Complete
Workstream B, full final gates and version integration remain outstanding.
Upstream current state and earlier native reports are preserved.


## V0.3.X actual value transfers of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`, following `8c432401`.
Composite packing now follows actual operand sizes within a bounded reservation.
Internal calls preserve actual kinds and lengths through parameters and returns.
Private composite slots retain actual view lengths and Unit values. Older aliases
keep their captured lengths after shorter writes. Field and host boundary guards
check actual lengths before access.

All 80 coroutine tests pass in both float configurations. Test-target Clippy
with warnings denied passes in both. The frozen default non-corpus run completed
761 tests with 760 passing and one stale Return mutation template failing.
The template is re-registered without changing its zero-return mutation.
Both mutation-placement tests pass after that correction, along with 15 native
citation, handoff and population checks and 13 root documentation checks.
Neither complete gate is claimed.

Upstream runtime finding, kept separate from native behavior. Private composite
self-assignment reaches `persist_composite_body` in `src/vm.rs`, which invokes
`copy_nonoverlapping` with overlapping source and destination. The debug runtime
aborts on its unsafe precondition. Reproducer is the source in
`native_private_self_assignment_preserves_the_current_view` in the native tests.
That test checks the native overlap-safe copy against explicit expected values
and region sentinels. It deliberately does not execute the undefined runtime
operation as an oracle. Root runtime sources remain unchanged.

Continue the admission audit with private scalar kind changes and correlated
numeric kinds at dialogue joins. These remain audit questions, not proven
refusal boundaries. Full Workstream B, both final gates and version integration
remain outstanding. Upstream current state and preceding reports are preserved.


## V0.3.X private kinds and ordering of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`, following `335a939f`.
Private scalar slots now retain actual kinds in checked persistent metadata.
Kinds survive frame release and reuse with private payloads. Ordered comparisons
select the runtime numeric kind, reject unequal kinds and preserve the virtual
machine's NaN ordering. Transfer-time refusals were replaced with typed-consumer
checks where the actual kind can be tested safely.

All 87 coroutine tests and test-target Clippy with warnings denied pass in both
float configurations. A frozen default non-corpus phase passed all 767 tests
before the final indexed-placement guard. After that guard, all three lowering
robustness tests pass. Neither complete native gate is claimed.

The measured gap at that increment is now covered by
`correlated_numeric_arithmetic_uses_the_actual_kind`. Its operands are both Byte
or both Word depending on the same branch. The virtual machine added them
successfully, while native admission then rejected `CheckedAdd`. Continue with actual-kind arithmetic selection and the remaining
[acceptance matrix](../decisions/WORKSTREAM_B_COMPLETION.md). Full Workstream B,
final gates and version integration remain outstanding. Upstream current state
and preceding native reports are preserved.


## V0.3.X runtime arithmetic of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`, following `ad184d07`.
Checked and bare arithmetic now select actual numeric kinds after branch joins.
All 92 coroutine tests and test-target Clippy with warnings denied pass in both
float configurations. The former correlated-arithmetic refusal now executes.
The corrected default non-corpus run passed all 773 tests with frozen inputs.
The archive records two preceding guard failures and their corrections.
No complete native gate or full Workstream B completion is claimed.

Continue the acceptance-matrix audit with bounded differing composite stream
reply extents. The virtual machine rejects scalar reply type mismatches, but its
Composite category admits more than the current native dialogue constraint.
This is a source-level audit question requiring an executable differential.
Root runtime sources remain unchanged. The earlier private self-assignment
runtime finding remains recorded above. Upstream current state and preceding
native reports are preserved.


## V0.3.X composite stream replies of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`, following `b007a6dc`.
Composite stream parameters now accept bounded differing reply shapes with actual
kind, length and ownership preserved through direct yields, delegation and Reset.
The initial refusal was verified against a successful virtual-machine sequence.
All 95 coroutine tests and test-target Clippy pass in both float configurations.
The archive records the memory-site classification. Verification assertions remain
intact. Root runtime sources and earlier native reports are preserved.

The default non-corpus run passed all 776 tests with frozen inputs. The final
acceptance audit classifies remaining refusals and records the shared opcode and
representation dependencies. No further suspension-specific gap was identified.
Next, run both complete native gates separately on the final clean commit, then
complete green version integration. The earlier private self-assignment runtime
defect remains recorded above. This is not a full completion claim.


## V0.3.X Workstream B verified implementation of 2026-09-30

Attributed to Codex on `feat/workstream-b-completion`, following `5695ab1f`.
Both complete native gates passed separately at that clean, frozen commit.
Each ran formatting, all-target lint, 776 non-corpus tests, ten corpus tests and
776 optimized non-corpus tests. The generated gate record preserves provenance.
One default-phase object-linkage test passed with a non-failing nextest diagnostic.
The archive records its qualification. Subsequent receipt edits receive targeted
checks and must not be called the identical tested tree.

The acceptance matrix and roadmap record the implementation evidence and shared
backend dependencies. Pull request 482 tracks version integration. Read its live
merge and check state before taking action. The active goal remains open until
green version integration is verified. Publication remains held. Root runtime
sources, root tests, the upstream current block and older native reports remain
unchanged. The earlier private self-assignment runtime defect remains recorded.
The receipt diff passed 13 root and 23 native documentation and record checks,
with zero failures or skips and exit zero. No Rust source or tests changed.

## Native audit follow-up of 2026-10-01

Fixed both native gate shell findings on `fix/native-gate-shell`. ShellCheck, failure-path checks,
30 reporter fixtures, 13 root and 23 native documentation checks pass. Full gates were not repeated.
A scratch native workflow proposal awaits operator approval for its per-push cost. No job is activated.

## Native continuous integration activation of 2026-10-01

The operator approved the dedicated native workflow. It covers default-feature non-corpus
execution in ordinary and optimized modes. Both local runs passed all 776 selected tests
with unchanged tracked inputs. Formatting, ShellCheck and all-target Clippy also pass.
The complete corpus and narrow-float gates remain separate. Consult workflow runs at
the exact commit being integrated for hosted results.


## V0.3.X Claude Code handoff of 2026-10-22

Attributed to Codex on `v0.3.0`, describing `7d141fcc` plus this local, unpushed
documentation commit. Resume at [the bounded brief](./handoffs/v0.3.0-BRIEF.md)
in `keleusma-worktrees/arena-composites`. Pull requests 482 and 483 are merged.
The exact merged head has 28 successful hosted checks and one deployment skip.
All 150 historical ancestry anchors pass. The old handoff was invalid-and-stale.
Upstream `cc48dc61` is 78 commits ahead of the absorbed history. Absorb and
revalidate before further native implementation. Full-gate reach is `UNVERIFIED`
for both configurations although their records remain PASS at `5695ab1f`.
The reviewed interline checkpoint is included. No runtime code changed.
The archive records verification limits and pilot sampling omissions.
Verification passed 13 root and 23 native documentation and provenance tests.
Both commands exited zero. Full native gates were not repeated.


## V0.3.X absorption 82, 2026-10-22

Claude Code on `feat/v3-absorption-82`, cut from `v0.3.0` at `13c9ed11` after the
Codex handoff commit was pushed. Absorption 82 merged 78 upstream commits at
`bd62e6fd`, through `cc48dc61`. The receipt is in
`docs/decisions/ABSORPTION_82_BRIEF.md`.

Upstream made division and indexed access total. The native backend now lowers the
new contracts in both paths, computes checked division in 64 bits, and lowers a
matched Fixed `Div` or `Mod` now that report 4 is answered. One emitter defect of
the terminator-walk class was fixed in `certified_local_widths`, with four test
walks of the same class. The four audit findings of the same day are closed as the
operator directed. Report 4 is marked answered and report six is re-measured above.

Verification before the gate commit: 777 non-corpus native tests and the corpus
differential were run on intermediate trees while the repairs proceeded, and the
root documentation, citation and correspondence guards passed 12 of 12 on the final
edits. The complete native gates run on the committed tree, and their outcome is
recorded in the receipt that follows this section, not predicted here.

Open for the operator: whether a hosted run of the complete native gate satisfies
the full-gate requirement before publication or `main`. Next intended step:
reconcile the Workstream B acceptance matrix against the absorbed runtime.
