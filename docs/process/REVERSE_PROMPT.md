# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

> **Rewritten rather than prepended, 2026-08-09.** Six slices of prepending had produced a file
> whose newest section was accurate and whose oldest sections contradicted it — the git-state block
> still announced "everything is merged, nothing in flight" while six unmerged commits sat on a
> feature branch. That is the accretion this file's own spec exists to prevent, and prepending is
> not overwriting. The per-slice reasoning is in the design journal, which is where it belongs.

---

## Last Updated

**Date**: 2026-08-09 (session 40, continued)

## THE MERGE BOUNDARY — read before merging anything

> **The gate that ran covers `3ad895e` ONLY.** I launched it on the slice-4 tip and then continued
> developing in the free tree, which is exactly the trap `HANDOFF.md` records: **gate the tip you
> intend to merge.**
>
> **Merge only up to `3ad895e`.** Then rebase the remainder, gate it, and merge that separately.
>
> This will keep recurring, because a free tree during a gate is the entire point of
> `gate-in-worktree.sh`. The discipline therefore has to live at MERGE time, not at launch time.

## Git state

| | |
|---|---|
| Version branch | `v0.2.3`, one unpushed mailbox commit `8fc802e` |
| Feature branch | `feat/selfhost-wire-real-corpus`, pushed |
| Gated commit | `3ad895e` (slice 4) — **everything after it is ungated** |
| Suite | `tests/selfhost_wire.rs`, **106 tests**, Tier 1 green throughout |

The mailbox commit on `v0.2.3` is deliberately unpushed: the pre-push hook runs the test suite and
I preferred not to risk a false `perf_canary` trip on my own running gate. Push it when convenient.

**Do not remove anything under `keleusma-worktrees/`** without checking — the other session's tree
and gate target live there.

## What the wiring increment has built

`wire.kel` can now emit, from real compiler output and byte-identically to the Rust encoder:

| Slice | What | Note |
|---|---|---|
| 1 | Container header for real region sets | needed **no** Keleusma change |
| 2 | `HEADER` record | first schema emitter; `wire.fin` input channel |
| 3 | `CHUNKS`, batched, window-addressed | widest record, 14 fields; batching mechanism |
| 4 | `PARAM_TYPES` byte pool | `wire.bin` channel; the pad is the whole risk |
| 5 | `NAMES` + `STRING_POOL` | the two accumulators; first deep batching, 774/807 |
| 6 | `DATA_SLOTS` + `SHARED_LAYOUT` | completes the four regions that are 99.96% of `lexer` |
| 7 | `SHAPES`, `SIGNATURES`, `ENUM_VARIANTS`, `ENUM_LAYOUTS`, `DATA_INIT`, `CONSTS` | **every populated kind now has an emitter**; `put_u64` for the two 64-bit fields |

**Both region shapes are covered** — record table and byte pool — and the batching mechanism is
built and exercised. What remains is coverage breadth and the driver, not new mechanism.

## Two things that are yours to decide, not mine

**1. Slice 5 costs about nine minutes of gate time.** The accumulator test is **201 s** measured,
taking the suite from ~23 s to ~224 s, and the gate runs the suite once per feature configuration.
That time is not inefficiency to optimise away: it is roughly 7.4 million `set_shared`/`get_shared`
calls in a debug build, which is what driving 6.6 MB through the public API costs, and batching
depth is the property under test. Restricting it to `parse` would still give 226 and 131 batches for
about a third of the time. **Kept at full coverage**, because that is a gate-scope trade in the same
class as trimming the feature matrix, and "probably safe" narrowing is how two coverage holes were
made here before.

**2. The SECDED plane is entirely unexercised by the shipping encoder.** `SchemaBuilder` declares
every region as `region(kind, 0)` and builds no parity plane anywhere, so real artifacts carry flags
0 and covers 0 throughout. Whether that is a deliberate cost choice or an unwired capability is not
mine to settle. It is pinned in the firing direction, and it reduces emitter scope: no ECC support
is needed for byte identity with the encoder as it stands.

## Next, in order

1. ~~The remaining populated record tables.~~ **DONE in slice 7.**
2. **The six record shapes with no corpus coverage**, needing hand-built emitter cases: `STRUCT_AUX`,
   `ENUM_AUX`, `STRUCT_TEMPLATES`, `PRIVATE_COMPOSITE`, `NATIVES`, `NATIVE_RETURNS`, plus
   `DEBUG_POOL` whose region is never emitted at all. **These do not block the driver** — a region
   with zero records is declared with length zero and needs no record emitter. They are a generality
   concern for programs that use natives or struct templates.
3. **The driver**, where values stop being decoded from the reference and start being computed. That
   is the real remaining work, and the residency measurement governs it.

~~A debt worth paying early in the next slice.~~ **PAID in slice 7, as a mechanism.** `wire.kel`
declares `highest_command()`, `main` refuses anything above it, and the sweep reads the value from
the source. A command added past the number is unreachable and fails its own test; a control on
`highest + 1` stops the bound drifting below the real top.

## Order-1: integration, not invention

- **Monomorphizer: EMPTY** for the first pass. Identity on all ten stage sources, pinned by
  `tests/selfhost_monomorphize_identity.rs` with a must-fire control.
- **Type checker: REJECTION ALONE.** Clearing `program.fn_expr_types` leaves every stage module
  byte-identical. Three controls, in
  [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md).
- **Wire-format serialization: expressible end to end**, and **every region kind the corpus
  populates now has an emitter driven by real data**. What is left is the six kinds the corpus
  leaves empty, and the driver.

## Open, held by the operator

- **Publication remains HELD.** Nothing is published.
- **Trimming the gate's feature matrix.** Argued against by evidence: the non-`--all-features`
  clippy caught lints in five separate increments, and `--no-default-features` caught a stray
  `examples/` file.
- **Per-element data slots.** One slot and one interned name per array element is why a 21 KB source
  produces a 16 MB artifact, and the cost is paid **three times over** in parallel tables plus the
  pool they index. A format and data-layout question with WCMU implications.
- **MSRV 1.85 declared, never verified.**

## Parallel development

`v0.3.0` carries native code generation. **Their gate on `9ac2be3` went GREEN**; they had rebased
onto my exact tip `78a5bc1`, so their run validated my step-6 merge too. I then took the machine
with `KEL_GATE_NAME=wire-corpus` and a separate target directory, leaving the default gate worktree
theirs. Their mailbox is `git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; mine is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Poll at increment boundaries — there is no wake.

## Method rules this stretch paid for

- **A probe establishes what it measured, not the question it was aimed at.** The wiring prep
  measured region sizes and answered "how big is a region" when the design needed "what must be
  resident at once".
- **Check `$?` explicitly; never read success off output.**
- **A first-try pass is a signal to check for vacuity, not to celebrate.**
- **Write the test that can fail.** A pad test that dirties the buffer in a separate call proves
  nothing, because every call starts from a fresh buffer.
- **State a coverage cap; never take one silently.** Slice 6 caps at 2048 records and says so, with
  the reason and the residual batch depth asserted.
- **Measure the failure instead of reasoning about it.** Slice 7's control failed and my first three
  hypotheses — a mis-parsed constant, an unbalanced brace, a misplaced guard — were each disproved
  by checking. The brace count I had eyeballed as wrong was balanced. The real cause was that a
  faulted VM is unusable for any later call, which no amount of reading would have suggested.
