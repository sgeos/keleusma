# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than
misleading a resuming agent.

## Validity

- **Branch**: `v0.2.3`, or a feature branch cut from it.
- **Parent commit**: `01a19293`
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
   **I failed to poll it for most of one session** and it held a demonstrated defect on my surface.
   It has no wake; poll at increment boundaries.
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md)
   (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).**
5. **Run `scripts/gate-status.sh`.** It answers "is a gate running, and where" in 0.23 s. Do not
   hand-roll a `pgrep` for it; see Gating.

## FIRST ACTION: a gate of MINE is running — read its verdict before anything else

**`scripts/gate-status.sh` first.** A gate was running on **`79fc97d1`** when this was written, the
tip of `feat/selfhost-wire-driver`, as `wire-corpus` in `.gate-target-wire`.

- **GREEN** -> merge `79fc97d1` into `v0.2.3` with `--no-ff`, **at the gated commit and without
  rebasing** — a rebase rewrites the hash the result rests on. Then push and confirm CI. `v0.2.3` is
  already a strict ancestor of that tip, so the merge cannot conflict and the gated tree is exactly
  the tree that lands.
- **RED** -> read the log the tool names before doing anything else.
- **ABANDONED** -> the tool now says so explicitly, on a `previous:` line. A run that stops without a
  verdict is neither GREEN nor RED, and waiting on one never ends. This line exists because I made
  that exact mistake.

**`perf_canary` runs in gate steps 3 to 7 only** — it is in the `keleusma` package, so steps 8 onward
(`keleusma-wire`, docs, links, the detached subprojects) cannot trip it. While a gate is inside that
window, **do not run a full test suite and do not push**: the pre-push tier runs the canary too.
Documentation, design and reading work is fine, and a brief targeted test is a judgement call rather
than a prohibition.

**THREE DOCUMENTATION COMMITS ON `v0.2.3` ARE UNPUSHED**, deliberately, for that reason. Push them
once the gate clears step 7.

## THE STATE

| Ref | Commit | Status |
|---|---|---|
| `v0.2.3` | `01a19293` | slices 1–11 merged; **3 docs commits unpushed** |
| `feat/selfhost-wire-driver` | `79fc97d1` | **gating when written**; slices 12–13 + prerequisites |
| `v0.3.0` | `357315b9` | gated GREEN, 13 steps; machine handed to me |

`v0.2.3` is a strict ancestor of the driver tip, so the merge back is a clean `--no-ff` bubble.

## WHAT CHANGED SINCE: the driver COMPUTES two of the three values it owed

**`tests/selfhost_wire.rs` is 125 tests.** Slice 12 computes `STRING_POOL`, `NAMES` and an
input-to-index map; slice 13 reorders a depth-first constant forest into the breadth-first `CONSTS`
table. Both byte-identical to `encode_aux_body` on real compiled modules.

**SLICE 13b's PREREQUISITES ARE IN AND ARE NOT VALIDATED BY ANY TEST.** The interner moved off `fin`
onto its own `nin`/`nout`, and the pool gained an output buffer `bout`. Both are changes to slice
12's MECHANICS, and **neither is distinguishable from what it replaced by anything in the suite**,
because the interner still walks its input sequentially. They rest on one argument: in-place
compaction is unsound once interning order differs from input order, which a breadth-first walk
guarantees. Two ten-byte names suffice to break it. **The test that separates them arrives with 13b**
— treat that as an obligation, not a nicety.

## THREE METHOD RULES THIS ARC PAID FOR, ALL ABOUT SEEING WHAT TESTS CANNOT

- **"The corpus cannot reach X" is a fact about the corpus.** Whether a SOURCE can reach X is a
  separate question that must be asked separately. Asking it overturned two committed conclusions:
  the flattener needs no hand-built constant trees (`const data` emits real composites to depth 2),
  and five of the six `DERIVE` coverage rows are reachable. **The matrix still reads 14 REAL / 6
  DERIVE** because upgrading a row means rewriting its emitter test; the achievable split is 19 / 1.
- **A green differential can be weak evidence.** The flattener's suite passed while four of its five
  cases could not tell breadth-first from depth-first, because a composite in LAST position makes the
  walks coincide. A corpus-level control is a different instrument from a must-fire mutation: the
  mutation asks whether the check can report a defect, this asks whether the inputs can tell two
  answers apart at all.
- **Read back what you just wrote.** Three defects this arc were in code whose full targeted suite
  was green: an unvalidated node count, a guard placed where its own test could not reach it, and a
  scratch field serving two roles. Tests confirmed the behaviour I thought to test.



`tests/selfhost_wire.rs` is **125 tests**. Slice 12 computes `STRING_POOL`, `NAMES` and an
input-to-index map from a (name, mode) sequence; slice 13 reorders a depth-first constant forest into
the breadth-first `CONSTS` table. Both are byte-identical to `encode_aux_body` on real compiled
modules. **Still not computed**: the (name, mode) sequence itself, which is a Rust model of the
encoder's call order; `STATIC_STR`/`STRUCT`/`ENUM` constants, which intern as they walk; per-chunk
ranges. **The dedup scan is linear**, the shape that cost the reference 782 seconds before it became
a `BTreeMap` — it must be replaced before a real stage drives it.

**The one idea to carry forward.** "The corpus cannot reach X" is a fact about the corpus; whether a
source can reach X is a separate question. Asking it properly overturned two conclusions this
project had committed to — the flattener does not need hand-built constant trees, and five of the six
`DERIVE` rows in the coverage matrix are reachable. **The matrix still reads 14 REAL / 6 DERIVE**
because upgrading a row means rewriting its emitter test; the achievable split is 19 / 1.

## WHAT WAS DONE EARLIER: the wire format is emittable end to end

`tests/selfhost_wire.rs` is **116 tests**. **All twenty region kinds have emitters**, and Keleusma
builds a **complete 912-byte auxiliary body byte-identical to `encode_aux_body`**.

**The qualifier is load-bearing: the driver RE-EMITS values decoded from the reference; it does not
COMPUTE them.** Interning, constant flattening and per-chunk range allocation remain. Do not let
"Keleusma builds a complete artifact" travel without that attached — a roll-up of mine dropped a
similar qualifier and had to be corrected in three places.

**Coverage is 14 REAL / 6 DERIVE**, and a matrix in
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md) says which
is which. **Real compiler output is a strong oracle for VOLUME and a weak one for VARIETY** — the
ten stages are large but semantically narrow, and three separate paths are unreachable from them.

## THE NEXT INCREMENT: make the driver compute what it borrows

Five probes scoped this before any code existed. Read the driver sections of the plan document.

1. **The interner needs BOTH modes.** `intern_fresh` exists for CONTIGUITY, not freshness, so
   `field_names_first + i` addressing holds. A dedup-only port fails only on enum layouts or struct
   constants, which small cases lack. It **cannot be unit-tested against the corpus**: its input is
   (name, mode) pairs owned by the caller. `parse` has 20 duplicate `NAMES` entries and four other
   stages have none — the corpus would look like a working oracle right up to the stage that
   matters.
2. **The flattener's composite path is unreachable from the corpus**: 2,192 constant nodes, **zero**
   composite, depth zero. It needs hand-built constant trees.
3. **`Shapes` is a second interner with the same two modes**, and its `intern` is a linear scan.
   That is correct here — shapes peak at 102 against 395,804 names — so **copy the linear scan for
   shapes and do not for names.**
4. **Region order is measured, not inferred**, and is not the schema's numeric order. Most regions
   are present with length zero; only the data-layout group and `DEBUG_POOL` are conditional.
5. **Order-1's last obligation is ~15 rejection shapes**, measured by execution rather than counted
   from 163 `TypeError` sites. All but one carry the `type error:` prefix, the exception being the
   V0.2.0 restriction on calling a local. **The oracle is verdict agreement, not message agreement.**

## Order-1: all three blockers closed or sized

- **Monomorphizer: EMPTY**, identity on all ten stages, pinned with a must-fire control.
- **Wire format: emittable end to end**, driver started.
- **Type checker: ~15 rejection shapes**, verdict-agreement oracle.

The `v0.3.0` session measured that **ten of eleven stage modules refuse native lowering on `Stream`,
not on composites**, so Order 1's *native* path is gated on sub-coroutines. Their caveat stands:
`lower_module` refuses on the first unsupported opcode, so `Stream` is necessary, not provably sole.

## Facts that cost real effort

- **A dispatch chain caps at NINETEEN arms**, not the 24 I had recorded, and exceeding it presents
  as a **stack overflow with SIGABRT in the test binary**, not a parse error. Three occurrences.
- **`Op::Reset` is a path exit.** A `loop` chunk contains no `Loop` op and ends in `Reset`. "The
  reference compiler always emits a trailing `Return`" is FALSE; asserting it broke 37 tests.
- **A faulted VM is unusable for later calls.** The fall-through sweep deliberately faults commands.
- **Shared data is re-seeded on every VM call**, so a multi-call artifact is carried forward as
  bytes. That is the staged design, and why slice 11 works as it does.
- **`verify()` now rejects a chunk that can run off its end.** Every path must exit via `Return`,
  `Trap` or `Reset` — a constraint on anything a backend emits.

## Method rules this arc paid for

- **Check `$?` explicitly.** A background command piped to `tail` reports *tail's* status.
  Propagate with `rc=$?; …; exit $rc`.
- **Make a textual patch ASSERT its anchor.** A `replace` matching nothing changes nothing and
  reports success. Hit twice, the second one increment after recording the first.
- **A bound in your own tooling is a by-name enumeration.** A 70-character regex never saw a
  71-character step; an anchored one never saw an ANSI-wrapped header.
- **Measure the failure instead of reasoning about it.** Three hypotheses about the `verify()`
  regression were each disproved by checking; dumping the ops settled it in one step.
- **Controls catch errors in the CORPUS, not just the subject.** A case I mislabelled ill-typed was
  caught only because well-typed controls sat beside it.
- **A roll-up drops the qualifier the detail records.** Prefer a table.

## Gating

`scripts/gate-in-worktree.sh <commit>` with **`KEL_GATE_NAME=wire-corpus` and
`KEL_GATE_TARGET=.gate-target-wire`**. The `v0.3.0` session uses `native` / `.gate-target-native`;
neither of us owns the unnamed default.

- **Gate the tip you intend to merge**, and **merge the gated commit BY NAME, without rebasing** — a
  rebase rewrites the hash the result rests on. The loop document was corrected for this.
- **Never hand-roll `pgrep -f "release-gate.sh"`**: it matches its own shell and deadlocked a
  session for hours. `scripts/gate-status.sh` decides liveness from log mtime instead.
- A gate is 1.5–3.5 h. The status line shows progress via `scripts/statusline-segment.sh`; the
  global half is `~/.claude/statusline.sh`, is user-global, and was backed up before editing.

## Open, held by the operator

- **Publication remains HELD.**
- **Gate cost.** Slice 5 adds ~9 minutes across the matrix; slices 10–11 add more. **The clean
  figure is unmeasured** — the suite read 600 s under contention against ~237 s before. Measure it
  quiet before quoting it.
- **The (72,64) SECDED plane is entirely unexercised by the shipping encoder.** `SchemaBuilder`
  declares every region with flags 0 and builds no parity plane.
- **Per-element data slots**, why a 21 KB source makes a 16 MB artifact, paid three times over in
  parallel tables.
- **MSRV 1.85 declared, never verified.**

## Parallel development

`v0.3.0` gated `3d36feb` GREEN and handed the machine back; they have adopted `gate-status.sh`.
They suggest dropping the "last synced" stamp from this branch's mailbox in favour of
`git rev-parse --short $(git merge-base v0.2.3 origin/v0.3.0)`, on the grounds that a stamp is only
as good as its maintenance and theirs went three syncs stale. **I agree and have not yet done it.**
