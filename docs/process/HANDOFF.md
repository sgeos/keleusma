# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than
misleading a resuming agent.

## Validity

- **Branch**: `v0.2.3`, or a feature branch cut from it.
- **Parent commit**: `bb95ce4`
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
   **I failed to do this for most of one session** and it held a demonstrated defect on my surface.
   Poll it at increment boundaries; it has no wake.
3. **Read this branch's mailbox** [`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md) and the three
   channels: [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md)
   (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).**

## THE STATE: six commits are LOCAL ONLY. Push them first.

**Nothing below is on origin.** `origin/v0.2.3` is at `cbf00c6`; local `v0.2.3` is `bb95ce4`.

| Ref | Local | Contains |
|---|---|---|
| `v0.2.3` | `bb95ce4` | the WCMU verifier fix merged at `fefd761`, plus mailbox and channels |
| `feat/selfhost-wire-debugpool` | 7 commits over `v0.2.3` | slices 9–10 and five probe write-ups |
| `fix/verify-terminal-depth` | `11c5d9d` | pushed, gated GREEN, now merged |

**Why nothing is pushed.** The pre-push hook runs the routine test tier. `perf_canary` was
executing inside the `v0.3.0` session's gate with EVE Online at 133% CPU, and my own canary had
already tripped once under that load (38.6 s against a 30 s tripwire) on a branch that changes **no
`src/` file at all**. Pushing into their canary window is what I asked them to spare me, twice.

**Push as soon as their gate clears.** That also re-runs my canary on a quiet machine, which is the
"re-run alone" step its own failure message asks for. **Do not raise the ceiling and do not
`--no-verify`.**

## WHAT WAS FINISHED: the wire format is emittable end to end

**All twenty region kinds have emitters**, and `tests/selfhost_wire.rs` is **114 tests**.

| Slices | Content |
|---|---|
| 1–4 | container header, `HEADER`, `CHUNKS` with batching, `PARAM_TYPES` pool |
| 5–8 | the accumulators, the per-slot tables, the remaining populated kinds, the six empty kinds |
| 9 | `DEBUG_POOL` from a real `emit_debug` compile — the twentieth kind |
| 10 | **the driver begins**: region lengths derived from record counts |

**A WCMU soundness hole is closed** (`fefd761`, gated GREEN over 13 steps). `verify()` admitted a
chunk that can run off the end of its instructions; `Return` truncates the operand stack and
falling off the end does not, so each call leaked `local_count + k - 1` slots and the attested
bound was wrong. Reported by `v0.3.0`, reproduced here first, then fixed.

## THE NEXT INCREMENT: the driver, scoped by four probes

Read the driver sections of
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md) **before
planning**. Each probe changed the plan before code existed:

- **A minimal artifact is 912 bytes, 1.4% of the buffer.** The first slice emits a COMPLETE artifact
  and compares byte for byte. Its full input surface is enumerated and the arithmetic closes to the
  byte.
- **Region order is measured, not inferred**, and is not the schema's numeric order. Most regions
  are present with length zero; only the data-layout group and `DEBUG_POOL` are conditional.
- **The interner needs BOTH modes.** `intern_fresh` is for contiguity, not freshness. A dedup-only
  port fails only on enum layouts or struct constants, which small cases lack. It also **cannot be
  unit-tested against the corpus**, because its input is (name, mode) pairs owned by the caller.
- **The flattener's composite path is unreachable from the corpus**: 2,192 constant nodes, zero
  composite, depth zero. It needs hand-built constant trees.

## Order-1: all three blockers closed or sized

- **Monomorphizer: EMPTY**, identity on all ten stages, pinned with a must-fire control.
- **Wire format: emittable end to end**, driver started.
- **Type checker: ~15 rejection shapes**, measured by execution rather than counted from 163
  `TypeError` sites. Every subset rejection lands in one pass; all but one carry the `type error:`
  prefix, the exception being the V0.2.0 restriction on calling a local. **The oracle is verdict
  agreement, not message agreement.**

## Facts that cost real effort

- **The parser depth ceiling is NINETEEN arms for a dispatch chain**, not the 24 I had recorded.
  Exceeding it presents as a **stack overflow with SIGABRT in the test binary**, not a parse error.
  Three occurrences in `wire.kel`.
- **A faulted VM is unusable for later calls.** The fall-through sweep deliberately faults commands,
  so any test reusing that VM afterwards needs a fresh one.
- **`Op::Reset` is a path exit**; a `loop` chunk contains no `Loop` op and ends in `Reset`. "The
  reference compiler always emits a trailing `Return`" is FALSE, and asserting it broke 37 tests.
- **Real compiler output is a strong oracle for VOLUME and a weak one for VARIETY.** The ten stages
  are large but semantically narrow; three separate paths are unreachable from them.

## Method rules this arc paid for

- **Check `$?` explicitly.** I masked an exit code by piping a background command to `tail`; the 0
  was `tail`'s. Propagate with `rc=$?; …; exit $rc`.
- **Make a textual patch ASSERT its anchor.** A `replace` that matches nothing changes nothing and
  reports success. I hit this twice, the second time one increment after recording the first.
- **A bound in your own tooling is a by-name enumeration.** My gate-progress regex capped headers at
  70 characters and silently never saw the 71-character twelfth step.
- **Measure the failure instead of reasoning about it.** Three hypotheses about the `verify()`
  regression were each disproved by checking; dumping the ops settled it in one step.
- **Controls catch errors in the CORPUS, not just the subject.** A case I mislabelled as ill-typed
  was caught only because well-typed controls sat beside it.

## Gating

`scripts/gate-in-worktree.sh <commit>` with **`KEL_GATE_NAME` and `KEL_GATE_TARGET` set** — mine is
`wire-corpus` / `.gate-target-wire`, theirs is `native` / `.gate-target-native`. Neither session owns
the unnamed default any more.

- **Gate the tip you intend to merge**, and **merge the gated commit by name, without rebasing** —
  a rebase rewrites the hash the result rests on. That correction is now in the loop document.
- **Stopping a gate is PATH-SCOPED, always.**
- **A `pgrep -f "release-gate.sh"` matches any shell whose command line contains that string**,
  including a waiter loop. That deadlocked the other session for hours.

## Open, held by the operator

- **Publication remains HELD.**
- **Gate cost.** Slice 5 adds ~9 minutes across the feature matrix; slice 10 may add more, but the
  clean figure is **unmeasured** — the suite read 600 s under contention against ~237 s before.
  Measure it quiet before quoting it.
- **The (72,64) SECDED plane is entirely unexercised by the shipping encoder.** `SchemaBuilder`
  declares every region with flags 0 and builds no parity plane. Deliberate cost choice or unwired
  capability is not mine to settle.
- **Per-element data slots**, why a 21 KB source makes a 16 MB artifact, paid three times over in
  parallel tables.
- **MSRV 1.85 declared, never verified.**

## Parallel development

`v0.3.0` is gating `3d36feb` in `keleusma-worktrees/native`. **The next gate slot after theirs is
mine**, but they have yielded to the release line twice and had none since `9ac2be3`, so do not
queue on top of them. Their mailbox carries two findings they raised on this surface, both now
fixed, and a warning that `verify()` is stricter for anything their backend emits.
