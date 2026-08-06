# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `8391bfa`
- **Written**: 2026-08-06
- **Tree at write**: clean. **An unfinished red branch exists locally — see below.**
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

**Validity check — run on resume.** Compare the **Parent commit** above to `git rev-parse HEAD~1`.

- **Match → VALID.** Proceed per the resume prompt.
- **Mismatch → INVALID and STALE.** Report it, familiarize from the three channels and the git log,
  and wait for instruction.

## On resume — do these first

1. **Run the validity check.**
2. **Read `secret/notes/APPENDIX_B.md` before writing ANY tracked file.**
3. **Re-read the three channels** — [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md) has the full
   remaining-work list, [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) newest-first,
   [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).** Its step 1a
   (probe before planning) falsified something in **every** increment of this arc.
5. **In-flight verification: NONE.**

## THE ACTIVE WORK: an unfinished cutover on a LOCAL RED BRANCH

```
git checkout feat/wire-cutover-proper     # local only, NOT on origin
```

- **`v0.2.3` = `8391bfa`, green, pushed, in sync with origin.** Untouched by the cutover.
- **`feat/wire-cutover-proper` = `d3d459a`**, one commit, **red by construction**, **not pushed**.
  It cannot be pushed while red: the pre-push hook runs the full gate and bypassing it is prohibited.

**Done on that branch**: both `module_to_wire_bytes` sites encode via `encode_aux_body`; the cold
loader decodes via `decode_aux_body` (the 8-byte-aligned scratch copy is gone, the v2 format being
byte-addressed); **`BYTECODE_VERSION` is 2**, operator-authorised 2026-08-06.

**Why red**: `Vm::archived()` is still `rkyv::access_unchecked` and reinterprets v2 bytes as an rkyv
archive. 322 lib tests fail.

### THE WARNING THAT MATTERS MOST

**The build is GREEN and the compiler catches none of this.** `access_unchecked` type-checks against
any byte range, so a wrong or half-finished port produces no diagnostic. **Do not treat a clean
`cargo build` as progress on this task.** The oracles are: the lib test suite, `tests/wire_corpus.rs`
(all ten self-hosted stages must still round-trip), and the VM actually executing those stages.

### Remaining work, in order

1. **Add the `AuxView` accessors the call sites need**: `op_record_count(chunk)`, `native_count()` /
   `native_name_bytes(idx)`, `template_field_count(chunk, template)`, `enum_min_payload(index)`,
   `enum_variant_count(index)`.
2. **Store `AuxOffsets` on the `Vm`**, resolved once at construction; replace `archived()` with
   `fn aux(&self) -> AuxView<'_>` via `AuxView::from_offsets`. Offsets carry **no borrow**, which is
   why this works where caching an `AuxView` cannot (the `Vm` owns the image the view borrows).
3. **Port the 26 `archived()` sites in `src/vm.rs`** — line numbers in `REVERSE_PROMPT.md`.
4. **Port the zero-copy entry** at `src/bytecode.rs:3886`, and the alignment guard at 3879.
5. **Update `CLAUDE.md`**, which still says `BYTECODE_VERSION` stays 1.
6. **rkyv does NOT go away**: six `AlignedVec` uses remain, unrelated to the aux archive.

## What is DONE, merged, and green on `v0.2.3`

- **`keleusma-wire` + `keleusma-wire-derive`** — schema-free container: triplicated prologue and
  directory, fixed-stride records, pools, CRC-32, (72,64) SECDED plane, `#[derive(WireRecord)]`.
  Both `#![forbid(unsafe_code)]`; reader allocation-free; builds for `wasm32v1-none`.
- **`src/wire_schema.rs`** — the whole aux body encodes and round-trips. `SchemaBuilder` owns the
  shared state (name interner, shape table, constant table).
- **`AuxView`** + **`AuxOffsets`** — the runtime read surface, resolve-once/reconstruct-cheaply.
- **Validation** — 90 schema tests, corpus differential over all ten stages (287 chunks, 2192
  constants), randomised input testing.

## The narrow requirement governing the port (probed, not assumed)

Exactly **one** accessor must return image-aliasing bytes: `AuxView::chunk_const_str_bytes`, for a
**non-empty top-level** `StaticStr`. Empty strings are deliberately not aliased; a composite's string
leaves are **already copied today**. Asserted by address, with a control proving the predicate rejects
a copy. Do not over-constrain into a borrow-everything design; do not lose that one property.

## Also outstanding

- **Publication HELD.** Operator: "push, but do not yet publish" (2026-08-06). Neither crate is
  published. **Irreversible and outward-facing — confirm first.**
- **MSRV 1.85 declared, never verified.**
- **The corpus emits zero struct templates**; that table rests on hand-built cases. The corpus test
  asserts the zero so the caveat cannot go stale.

## Method rules this arc validated

- **Probe before planning.** Falsified a claim in every increment, including ones written by me an
  increment earlier and limitations in already-merged code.
- **Ask what a test would still pass with.** Three succeeded emptily before being caught:
  `ConstValue::PartialEq` ignores the enum discriminant; ECC tests compared counts not values; the
  fuzz suite reached the readers 0/2000 times.
- **Run clippy `--all-features`, the `-D warnings` doc build, and `--no-default-features` BEFORE the
  gate.** Both genuine gate reds this arc lived in those blind spots.
- **Measure, do not theorise.** Three wrong guesses about a performance cliff cost ~30 minutes;
  instrumentation found it in one run. Wall-clock includes laptop suspend and is not a work measure.
- **Do not touch the tree while a gate runs.** Four gates lost to this.

**Boundary counts** — **79 Ok / 4 Gap / 1 RefRejects**, unchanged. Recount with a grep.

**Git**: `v0.2.3` = `8391bfa` plus this handoff commit, in sync with origin. Local branches: `main`,
`v0.2.3`, `v0.2.3-prerebase-backup`, `feat/wire-cutover-proper`. **Do NOT delete
`v0.2.3-prerebase-backup`** (309 commits not in `v0.2.3`, a deliberate safety net).

**Guardrails**: no new opcode without authorization; run the FULL gate before claiming complete;
confirm before any irreversible or outward-facing action; never bypass the pre-push gate.
