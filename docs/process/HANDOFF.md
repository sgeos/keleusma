# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than
misleading a resuming agent.

> **Rewritten whole, 2026-08-11**, not patched. Incremental edits had left it asserting 14 REAL / 6
> DERIVE, 125 tests and 116 tests in three separate places, alongside a "next increment" section
> describing work finished hours earlier and a Gating section for a workflow that no longer exists.
> **A handoff that contradicts itself is worse than a stale one**: a reader cannot tell which half to
> trust. Overwrite this file; do not append to it.

## Validity

- **Branch**: `v0.2.3`, or a feature branch cut from it.
- **Parent commit**: `23943578`
- **Written**: 2026-08-11
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

**Check both.** `git rev-parse --abbrev-ref HEAD` is `v0.2.3` or a branch off it, and
`git rev-parse HEAD~1` equals the parent above. The branch half is not redundant: `v0.3.0` carries
parallel native-codegen work and can satisfy the commit check while describing a different
workstream. If you are on `v0.3.0`, read `docs/process/handoffs/v0.3.0.md` and **do not overwrite
this file**.

- **Both match → VALID.** **Commit mismatch → INVALID and STALE.** **Branch mismatch → NOT YOURS.**

## On resume, before doing anything

1. **Read `secret/notes/APPENDIX_B.md`.**
2. **Read the other session's mailbox**: `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`.
   It has no wake; poll at increment boundaries.
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md)
   (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).**

## FIRST ACTION: read the retraction before trusting the plan's residency numbers

**Nothing is in flight.** PRs #9-#13, #15, #17, #19 and #21 all merged on 22/22 CI green, each at the commit CI ran. Even-numbered
PRs from #14 on are the other session's; **tell us apart by BASE BRANCH, not author** -- both sessions
use the same account, so `--author @me` matches theirs too. Confirm with `gh pr list --state open`; if `gh run list --branch v0.2.3 --limit 1` is red, read
its log first.

**Then read the CORRECTION section in
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md).** Two
commits on this branch, `db700212` and `5bec2df8`, published a residency "refutation" and a
~321,000-slot budget that are **withdrawn** by `69a32862`. If you have those numbers in context from
a summary rather than from the file, they are wrong. The plan's original 77% projection stands.

## THE WORKFLOW CHANGED TODAY. CI GATES FEATURE BRANCHES.

**Do not run `scripts/release-gate.sh` to gate a merge.** Operator decision, 2026-08-11: gate time
was the project's bottleneck and two sessions were serialising on one machine.

1. Feature branch cut from `v0.2.3`.
2. Verify locally as you go — the suite and tier 1 are cheap and catch things before CI does.
3. Push, open a **draft PR to `v0.2.3`**. `pull_request: branches: [main, 'v*']` triggers the full
   23-job matrix on hosted runners.
4. **Merge on CI green, at the commit CI ran, without rebasing.** Push. Confirm CI on the merge.

**CI is a verified strict superset of the local gate**, checked job by job rather than assumed. Every
one of the twelve local steps has a CI job, including `keleusma-wire` in *both* configurations. CI
also runs Miri, two MSRV checks, `no_std`, the RTOS `thumbv8m` cross-build, `keleusma-bench`, SDL3
examples, the LSP, the extension and the WASM playground. **~48 minutes contending for nothing,
against ~2h30m exclusive.**

**The `perf_canary` courtesy is retired.** Once neither session holds the machine, neither has a
canary window to protect. **Run builds freely.** `scripts/gate-status.sh` still works and still
reports an abandoned run on a `previous:` line; it is now an occasional check, not a scheduling
instrument.

**The local gate keeps two uses**: a pre-publication run with `--miri`, and working offline.

**It worked for both sessions.** `v0.3.0` abandoned three local gates today and has since merged
three PRs (#2, #3, #6).

## THE STATE

| Ref | Commit | Status |
|---|---|---|
| `v0.2.3` | `23943578` | nine PRs merged in, pushed |
| PR #9 | `ae01441f` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #10 | `3b93e351` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #11 | `eaf95524` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #12 | `ad0a1bff` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #13 | `fa4badb5` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #15 | `af980528` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #17 | `7edbd767` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #19 | `2cd653dc` | **MERGED**, 22/22 green, at the commit CI ran |
| PR #21 | `c2700d45` | **MERGED**, 22/22 green — **the capstone** |
| `v0.3.0` | — | same workflow; their last local gate is STALLED and irrelevant |

Eight PRs merged on this line today, every one CI-gated, **with the local machine idle throughout**.

## WHERE THE DRIVER IS

`tests/selfhost_wire.rs` is **148 tests**. Keleusma computes **all five** of the values the driver
owed: the name table with both interning modes, the breadth-first constant ordering, the names
interned **during** the walk for all three interning tags with `STRUCT_AUX` and `ENUM_AUX` alongside,
the per-chunk ranges, and now the interning SEQUENCE itself, derived from a module description that
is grouped by kind rather than pre-ordered.

**The emitter coverage matrix is 19 REAL / 1 DERIVE**, up from 14 / 6. The one remaining DERIVE row
is `STRUCT_TEMPLATES`, and it is **structural rather than pending**: the boxed construction path
needs a non-flat type, the only one is `Text` under a narrow word, and this suite is gated out of
narrow-word builds.

## THE MECHANICAL ARC IS COMPLETE. WHAT IS LEFT IS NOT MECHANISM.

**PR #21 closed it.** Keleusma's own output builds `verify_datalayout`'s entire 105,848-byte
auxiliary body — header area, directory and every region — byte-identical to `encode_aux_body`. The
driver has all five computed values, batching on both paths, window positioning across all seventeen
record kinds, multi-window assembly, and now whole-artifact composition.

**Do NOT reach for another emitter slice. There is no obvious one left**, and inventing one is how a
programme starts producing mechanisms that work and were not needed. Three candidates, in the order
I would take them:

1. **A second stage through the capstone.** `verify_yield` at 303,464 bytes exercises multi-window
   assembly inside whole-artifact composition, which `verify_datalayout` does not — its regions all
   fit one window. Cheap, and it tests composition at a size the current test cannot.
2. **The residency question, which is the operator's.** About **40.7 bytes of artifact per data
   slot**, one slot per array element, ~2.4 s of compile time per megabyte declared. That is what
   makes `lexer` expensive rather than impossible, and it is a representation decision rather than an
   increment.
3. **`parse` and `verify_typed` end to end**, which need the accumulator question answered first.

**FOUR CONSECUTIVE GAPS HERE NEEDED A CALLER, NOT AN EMITTER.** Generic batching, multi-window
assembly, the capstone, and `DEBUG_POOL` before them. **Check first, every time.** The cost of not
checking is not a wrong answer; it is a mechanism that works, passes its tests, and did not need to
exist. On the capstone the check was one grep — the artifact's only checksum covers twelve prologue
bytes, not the body — and had it gone the other way the increment would have needed an incremental
CRC carried across windows.

**Do NOT do these two.** Reasoning in
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md):

- **Replacing the linear dedup scan.** No early exit in a total language; inputs capped at 256.
- **Computing the chunk record's name index.** `map[j] == j` always.

**Constraints to carry into any new command.** `dispatch_driver` holds 18 arms, `dispatch_driver2`
13, `emit_at` 17 with flat bodies. The cap is a depth budget of 24 shared between chain position and
arm-body nesting: 20 arms with a no-argument body, 18 with a nested call. Exceeding it in the test
harness is a stack overflow and SIGABRT, not a parse error.

**CUT THE FEATURE BRANCH AS THE FIRST ACTION**, before any edit. PR #15's code went straight onto
`v0.2.3`, caught only because nothing had been pushed. The moment to guard is just after a merge.

## THE ONE RULE THAT MATTERED MOST TODAY

**A differential's failure mode is not a wrong answer. It is a corpus that cannot tell right from
wrong.** Byte identity against a strong oracle reads like proof and is only ever as strong as the
inputs behind it.

Four vacuity controls were needed this arc and **three would have passed while measuring nothing**:

- the flattener went green while four of five cases could not distinguish breadth-first from
  depth-first, because a composite in LAST position makes the walks coincide;
- that control compared tags alone, which cannot separate `((1,2),3)` under the two walks;
- the struct version counted only strings, so every struct case looked non-discriminating;
- with one chunk every range starts at zero, so a driver emitting a constant `0` would have passed.

**The corollary, which bit later**: a guard whose triggering input the corpus cannot generate is
untested by construction, and writing a test to make it *look* covered is theatre. Two pool-capacity
guards are unreachable and deliberately untested; that is recorded at the code.

## FACTS THAT COST REAL EFFORT

- **`const data`, referenced from a function, emits real composite constants** to depth 2 in ~1 KB.
  This overturned a committed plan conclusion. There are **three** data visibilities, not two.
- **A struct interns its type name, THEN captures `field_names_first`, THEN interns fields FRESH.**
  Capturing it first is off by one on every struct whose type name is new — invisible on a corpus of
  familiar names.
- **An enum's two names both DEDUP**, unlike a struct's field run, and the discriminant flag cannot
  be derived from the value: `Some(0)` and `None` both present as zero.
- **A LAST match wins** in the interner: `intern_fresh` inserts into the reference's index, so a
  later `intern` yields the second occurrence. A first-match scan gives byte-identical `NAMES` and
  `STRING_POOL` and a wrong `ENUM_LAYOUTS`.
- **In-place pool compaction is unsound** once interning order differs from input order; two
  ten-byte names break it.
- **A dispatch chain's cap is a DEPTH BUDGET OF 24 SHARED between chain position and arm-body
  nesting, not an arm count** — measured 2026-08-11, superseding the "nineteen arms" figure that
  stood here. Every level an arm body nests costs one arm off the chain, which is why earlier
  sessions recorded 19 and 23 and both were right for their shape. In the TEST HARNESS, which is the
  binding context because that is where `wire.kel` is compiled, `dispatch_driver` holds **20 arms**
  with a no-argument call body and **18** with a nested-call body. It is at 18 today: two arms of
  headroom, or none, depending on what the arm calls. All three figures measured against the real
  chain, not a synthetic one: 20 / 19 / 18 arms for a no-argument body, `emit_in_region(a, b)`, and a
  nested-call body respectively.
- **The failure mode differs by context, and the test harness gets the worse one.** A 2 MB test
  thread overflows its stack and SIGABRTs before `MAX_PARSE_DEPTH` (`src/parser.rs:98`) can report;
  the CLI, on the main thread's larger stack, rejects the same source cleanly at 23 arms with a
  `ParseError` naming the limit. **So do not size a chain from a CLI measurement** — that reads two
  to three arms too generous. Flagged for the operator below.
- **`Op::Reset` is a path exit.** A `loop` chunk has no `Loop` op and ends in `Reset`.
- **Shared data is re-seeded on every VM call**, so a multi-call artifact is carried forward as bytes.
- **A faulted VM is unusable for later calls.**
- **An enum discriminant takes a literal with an optional unary minus**; `A = 0 - 5` is rejected with
  "expected type name" — right column, wrong explanation.
- **Chained tuple indexing `k.t.0.1` is not admitted.** Pass the nested tuple to a function.
- **`verify()` rejects a chunk that can run off its end.** Every path must exit via `Return`, `Trap`
  or `Reset` — a constraint on anything a backend emits.

## METHOD RULES THIS ARC PAID FOR

- **"The corpus cannot reach X" is a fact about the corpus.** Whether a SOURCE can reach X is a
  separate question. Asking it overturned two committed conclusions.
- **Read back what you just wrote.** Three defects this arc were in code whose full targeted suite
  was green: an unvalidated node count, a guard placed where its own test could not reach it, and a
  scratch pass that could overwrite a live artifact.
- **A test can catch what reading cannot.** The `fl_tag_in_scope` coupling was found by a negative
  test written one slice earlier for a different tag. Neither instrument subsumes the other.
- **A hand-written list or bound is a by-name enumeration.** Nine instances catalogued; four found
  today in TEST harness `match` arms. All silent, all reading as success.
- **A region-level diff names the kind; a byte diff names nothing.** It located four harness defects
  in one sitting and is kept in the test rather than removed as scaffolding.
- **Fixing a failure mode in one tool does not fix it in the tool that shares the assumption.** The
  abandoned-run display was fixed in the morning; the same bug survived in the waiter beside it.
- **Check `$?` explicitly.** A command piped to `tail` reports *tail's* status. Hit again today.
- **Make a textual patch ASSERT its anchor**, and beware `replace(..., 1)` when two tests share a
  line shape — it silently patched the wrong function.
- **A roll-up drops the qualifier the detail records.** Prefer a table.

## Order-1 status

- **Monomorphizer: EMPTY.** Identity on all ten stage sources, pinned with a must-fire control.
- **Type checker: ~15 rejection shapes**, sized by execution rather than counted from 163 `TypeError`
  sites. **The oracle is verdict agreement, not message agreement.** See
  [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md).
- **Wire format: emittable end to end**, driver computing four of five values.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **Per-element data slots.** One slot and one interned name per array element is why a 21 KB source
  makes a 16 MB artifact, paid three times over in parallel tables plus the pool they index.
- **The (72,64) SECDED plane is entirely unexercised** by the shipping encoder.
- **`MAX_PARSE_DEPTH` does not do its stated job on a small stack, and this is a runtime concern
  rather than a workflow one.** The constant is 24 (`src/parser.rs:98`) and its message says deep
  nesting is "rejected to prevent stack overflow". Measured 2026-08-11: on a 2 MB thread the stack
  blows BEFORE the guard fires, so the process aborts with SIGABRT instead of returning a
  `ParseError`. The limit is evidently calibrated for a main thread's larger stack. An embedder that
  parses untrusted source on a small-stack thread therefore gets an abort, not a rejection, which is
  an availability failure at a trust boundary the guard was written to hold. **Not changed
  unilaterally**: lowering the constant narrows the admitted language surface and would need a
  reason beyond one measurement, so this is the operator's call.
- **MSRV**: CI checks 1.85 for `keleusma-arena` and 1.88 for `keleusma`.

## Parallel development

`v0.3.0` carries native code generation and is on the same CI-gated workflow. Their measurement that
matters here: **ten of eleven stage modules refuse native lowering on `Stream`, not on composites**,
so Order 1's native path is gated on sub-coroutines. Their caveat stands — `lower_module` refuses on
the first unsupported opcode, so `Stream` is necessary, not provably sole.

**A courtesy worth asking them for again**: announce a gate start in the mailbox. Restarting silently
reopens a canary window, and that cost a disclosed overlap today.
