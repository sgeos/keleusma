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

## Where things stand, 2026-08-10

**The wire format is emittable end to end from Keleusma, and the driver has started.**

| | |
|---|---|
| `v0.2.3` | wiring slices 1–8 merged, **WCMU verifier fix merged** (local, unpushed) |
| `feat/selfhost-wire-debugpool` | slices 9–10 plus five probe write-ups, **local only** |
| Machine | held by the `v0.3.0` session, gating `3d36feb` |

**Every one of the twenty region kinds now has an emitter**, the populated kinds driven by real
compiler output, the six the corpus leaves empty oracled against the derive, and `DEBUG_POOL` from
a real `emit_debug` compile. Slice 10 began the **driver**: region lengths are derived from record
counts, so all seventeen strides live on the Keleusma side.

**A WCMU soundness hole is closed.** `verify()` used to admit a chunk that can run off the end of
its instructions, which leaks `local_count + k - 1` operand slots per call and breaks the attested
bound. Reported by the `v0.3.0` session, reproduced here first, fixed, gated GREEN over 13 steps.

### Nothing is pushed, and that is deliberate

Six commits are local-only. The pre-push hook runs the routine test tier, `perf_canary` was
executing inside the other session's gate, and EVE Online was at 133% CPU. Firing a test tier into
their canary window is what I asked them to spare me, twice. **Push when their gate clears** — that
also re-runs my own canary quiet, which is the "re-run alone" step it asked for when it tripped.

### The driver's next piece, already scoped by four probes

1. **A minimal artifact is 912 bytes, 1.4% of the buffer**, so the first slice emits a COMPLETE
   artifact and compares byte for byte. The residency problem that governs `lexer` does not arise
   at that size.
2. **Region order is measured, not inferred**, and is not the schema's numeric order. Most regions
   are present with length zero; only the data-layout group and `DEBUG_POOL` are conditional.
3. **The interner needs BOTH modes.** `intern_fresh` exists for contiguity, not freshness, and a
   dedup-only port is a defect that surfaces only on enum layouts or struct constants — neither of
   which small test cases have.
4. **The flattener's composite path is unreachable from the corpus**: 2,192 constant nodes, zero
   composite, depth zero. It needs hand-built constant trees.

### The standing lesson from all of it

**Real compiler output is a strong oracle for VOLUME and a weak one for VARIETY.** The ten stages
are large but semantically narrow, and this arc found three separate paths they cannot reach. A
slice should say which of the two it is buying.

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
2. ~~The six record shapes with no corpus coverage.~~ **DONE in slice 8**, oracled against the
   derive's `write_record` since the corpus cannot reach them. `DEBUG_POOL` remains: its region is
   never emitted at all, and it is a byte pool, so slice 4's emitter already covers the mechanism —
   what is missing is a case, not code.
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
- **Wire-format serialization: expressible end to end**, and **every one of the seventeen record
  shapes now has an emitter** — the populated kinds driven by real compiler output, the rest
  oracled against the derive. What is left is the driver.

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
