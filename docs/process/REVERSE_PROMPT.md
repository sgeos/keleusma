# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

Prior sessions' blocks were removed here rather than allowed to accrete, which is what this
file's spec asks for and why it once reached 362 KB. Nothing was discarded: the reasoning is
in the design journal, and the two items it did not already hold were carried into its newest
entry before this overwrite.

---

## Last Updated

**Date**: 2026-08-09 (session 40, continued)

## STEP 6 IS COMPLETE — the wire format is expressible in Keleusma end to end

All seven slices. `src/selfhost/kel/wire.kel` plus `tests/selfhost_wire.rs`, **80 tests**.

| Slice | Content |
|---|---|
| 1 | CRC-32/ISO-HDLC, oracle the published check value |
| 2 | Container primitives, prologue, majority-of-three vote |
| 3 | Region directory, with the prologue-to-directory bootstrap |
| 4 | Record tables and byte pools |
| 5a–5e | The schema layer: 20 region kinds, 17 record shapes |
| 6a–6b | Opcode records and the operand pool |
| 7 | Framing header and CRC trailer |

**What remains is wiring, not invention.** `wire.kel` is deliberately absent from `read_stage`
and is not yet driven by the pipeline; it emits and reads the format, but nothing calls it to
produce a real artifact yet. That is the next increment.

## Order-1 has moved a long way, and the roadmap now understates it

The roadmap names three blockers. Two were measured and shrank; the third is now done.

- **Monomorphizer: EMPTY for the first pass.** Identity on all ten stage sources, pinned by
  `tests/selfhost_monomorphize_identity.rs` with a must-fire control.
- **Type checker: reduces to REJECTION alone.** Clearing `program.fn_expr_types` leaves every
  stage module byte-identical, so the structural fallback covers the subset. Three controls, in
  [`../decisions/TYPECHECK_SELFHOST_PLAN.md`](../decisions/TYPECHECK_SELFHOST_PLAN.md).
- **Wire-format serialization: step 6 complete.**

**The roadmap's Order-1 row should be restated once this merges.** It currently implies three
comparable blockers; the accurate statement is that the remaining work is integration.

## Git state — READ THIS BEFORE RESUMING

**Everything is merged. Nothing of mine is in flight.** Both wire-format branches landed; the
stacked-branch table that stood here described a state that ended when they merged.

`v0.2.3` is the working line, in sync with origin, tree clean. The stale worktree
`keleusma-worktrees/wire-directory` still sits on the merged `feat/selfhost-wire-data`; harmless,
and **nothing under `keleusma-worktrees/` should be removed without checking**, because the other
session's tree lives there too.

**A gate is running, and it is not mine.** The `v0.3.0` session is gating `9ac2be3`, rebased onto
my exact tip `78a5bc1`. So I cannot start a gate, and no timing measurement is trustworthy until
it finishes. Development is unaffected, which is the whole point of the detached-worktree gate.

## Wiring slices 1 and 2 are done; slice 3 is a multi-record region

**On `feat/selfhost-wire-real-corpus`, Tier 1 green, NOT gated. 86 tests, up from 80.**

**Slice 2 added the first schema emitter.** `emit_header_record` writes a real record's real fields
at the transcribed offsets, byte-identical to the Rust encoder for all ten stages, with the
reference reader recovering all eleven fields from what Keleusma wrote. The must-fire control flips
one bit of each field in turn and requires every one to change the output, so "the offsets are
right" is asserted rather than hoped.

**The input channel is `wire.fin: [Word; 1024]`**, a record's fields in declaration order. It is a
**batch** buffer by design, not a region's worth: the largest real region holds about 395,784
records, so the host must feed fields in batches while appending output. The staged shape now lives
in the interface, not only in a document.

**The buffer constraint bound on the first record.** A real HEADER region's `region_base` lands far
outside the 65,536-byte `wire.bytes`, so the record is emitted into a one-region artifact and
compared against the payload extracted from the real one. Every later slice inherits this.

**Slice 3 is `CHUNKS`, and the prep has scoped it fully.** It is the **smallest region that forces
batching**, at two batches, so the mechanism gets built where a failure is legible rather than
inside the 1547-batch regions. `ChunkRecord` is the widest record in the format at **14 fields**,
and **all 14 offsets are already transcribed and pinned** in `wire.kel`, so the emitter itself is
mechanical: eleven `u32`, one `u16`, two `u8`.

Three prep results shape it, all measured:

- **`fin` is always the binding constraint; the output window never is.** The largest batch any
  record kind can produce is 5,456 bytes, 8.3% of `wire.bytes`, a 12-fold margin — structural, since
  a field is a whole word in `fin` and at most four bytes in the record. **So a slice needs input
  batching only**, which is a materially smaller mechanism than the staged design implied.
- **Slice 2's positioning does not generalise.** `emit_header_record` locates its record through
  `region_base`, an absolute artifact offset, which works only in a one-region artifact. A real
  `CHUNKS` region sits millions of bytes in. The signature must grow a **window base** and a
  first-record index, and slice 3 is the moment to do it, while there is exactly one caller.
- **Seven of twenty region kinds get no emitter coverage from the corpus**, not one: `DEBUG_POOL` is
  never emitted, and six more are emitted with zero records. Six of seventeen record shapes will
  need hand-built emitter cases.

## Slice 1, for context

**On `feat/selfhost-wire-real-corpus`, one commit, Tier 1 green, NOT gated.** The container header
is emitted by `wire.kel` for all ten stages' **real** region sets and matches the Rust encoder byte
for byte. 83 tests, up from 80. Seven-perturbation must-fire control plus the clean case.

**It needed no Keleusma change**, which is the finding. Commands 18-83 in `wire.kel` are readers,
and `emit_pattern_records` writes a synthetic pattern rather than a schema record, so the emit side
already covered everything the container header needs.

**Slice 2 is where new Keleusma code starts**: emit a real schema region's PAYLOAD. `HEADER` is the
smallest target, one record. The design question to settle first is **input marshalling** —
`HeaderRecord` has eleven fields and only five `warg` slots exist, so the host needs a word-array
input channel in shared data, the way `rkind`/`rflags`/`rlen`/`rcovers` already are for the
directory. That array generalises to every record kind and is the staged emitter's real interface,
so it is worth designing once rather than per region.

**An observation held for the operator.** `SchemaBuilder` declares every region as
`region(kind, 0)` and builds no parity plane, so the **(72,64) SECDED plane in `keleusma-wire` is
entirely unexercised by the shipping encoder**. Whether that is a deliberate cost choice or an
unwired capability is not mine to decide. It is pinned in the firing direction, and it reduces the
emitter's scope: no ECC support is needed for byte identity with the encoder as it stands.

## The wiring increment: the prep's sizing was wrong, and is corrected

Probing before planning caught it, as it has in nearly every increment of this arc.

The prep sized the emitter's buffer from **the largest single region**, 6,609,960 bytes. That is
the wrong quantity. `SchemaBuilder::finish` writes `STRING_POOL` and `NAMES` **last**, after every
other contributor has interned into them, so those two are **accumulators resident across the whole
emission**, not buffers reused per region. For `lexer` they total **9,776,392 bytes, 58.3% of the
ceiling**, leaving about **7.0 MB** rather than the 10 MB recorded.

Also measured: four regions carry **99.96%** of `lexer`'s auxiliary body, and three of those four
are per-slot tables of identical record count at an 8-byte stride. The per-array-element slot
explosion therefore appears three times over, plus the pool of names they index.

The prep's conclusion survives — staged emission is viable, whole-artifact is not — but the design
target moved. Full numbers and the unverified-projection caveat are in
[`../decisions/WIRE_FORMAT_SELFHOST_PLAN.md`](../decisions/WIRE_FORMAT_SELFHOST_PLAN.md).

## Method rules this stretch paid for

- **Transcribe, then pin.** Where a value must be copied from another language, assert it
  against the generating source rather than restating it in the test.
- **The sentinel technique fails silently** when the value domain has no spare value. Split the
  bound from the value instead: three separate cases hit this.
- **Check `$?`; never read success off the output.** A `| tail` hid a red gate earlier today.
- **An implausibly fast pass is the signal.** A five-minute "green" on a 2.5-hour gate.
- **A set difference is not a finding until the scope is established.** Two false alarms in the
  documentation audit, both rejected by reading the document's stated scope first.

## Open, and held by the operator

- **Publication remains HELD.** Nothing is published.
- **Trimming the gate's feature matrix.** Now argued against by evidence: the
  non-`--all-features` clippy caught lints in five separate increments today, and
  `--no-default-features` caught a stray `examples/` file. The matrix is finding defects at a
  steady rate.
- **MSRV 1.85 declared, never verified.**
- **Per-element data slots.** One slot and one interned name per array element is why a 21 KB
  source produces a 16 MB artifact, and the measurement above shows the cost is paid three times
  over in parallel tables. A format and data-layout question with WCMU implications, not a loop
  decision.

~~Two audit findings recorded but not applied.~~ **Both closed.** The `CheckedArithNoArm` finding
was **refuted by execution** — a checked construct with only an `ok` arm raises `DivisionByZero`,
so the proposed wording would have put a false statement into a normative spec. The corpus
enumeration hole is closed by a mechanism, and both directions of the guard were shown to fire.

## Parallel development

`v0.3.0` carries native code generation. **Their gate is running on `9ac2be3` and mine is not**;
they rebased onto my exact tip, so their run validates my step-6 merge too. The mailbox is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md), theirs is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`. **A gate no longer makes the main tree
look busy** — it runs in a detached worktree, so check `pgrep -f release-gate.sh` or the banner.
