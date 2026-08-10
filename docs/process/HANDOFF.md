# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than
misleading a resuming agent.

## Validity

- **Branch**: `v0.2.3`, or a feature branch cut from it.
- **Parent commit**: `7fab0d3`
- **Written**: 2026-08-10
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

## FIRST ACTION: check the gate, then merge if it is green

**A gate was running on `9eb623d` when this was written** — `feat/selfhost-wire-debugpool`, one
commit over `v0.2.3`, carrying slice 11.

- **GREEN** → merge `9eb623d` into `v0.2.3` with `--no-ff`, **at the gated commit and without
  rebasing**. Push, then confirm CI.
- **RED or STALLED** → read the log the tool names before doing anything else.

**CI was in progress on the merge `7fab0d3`.** Confirm it went green; a red CI is remedied at once.

## THE STATE

| Ref | Commit | Status |
|---|---|---|
| `v0.2.3` | `7fab0d3` | pushed; slices 1–10 and the gate tooling merged |
| `feat/selfhost-wire-debugpool` | `9eb623d` | pushed; slice 11; **gating when written** |

Everything is on origin. Nothing is local-only.

## WHAT IS DONE: the wire format is emittable end to end

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
