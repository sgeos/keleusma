# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `5bf71c7`
- **Written**: 2026-08-05
- **Tree at write**: clean. Everything merged to `v0.2.3` and pushed; no feature branch open.
- **Context**: the six-step wire-format programme. Steps 1, 2 and 4-stage-1/2a are DONE and merged.
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** It defines what must not
  appear in this repository. **Hard constraint, not a preference.**

**Validity check — run on resume, before trusting this handoff.** Compare the **Parent commit** above
to `git rev-parse HEAD~1`.

- **Match → VALID.** Proceed per the resume prompt below.
- **Mismatch → INVALID and STALE.** Report the mismatch, familiarize from `REVERSE_PROMPT.md`,
  `DESIGN_JOURNAL.md`, `TASKLOG.md`, and the git log, and wait for instruction.

## On resume after compaction — do these first

1. **Run the validity check above.** Mismatch → stop and report.
2. **Read `secret/notes/APPENDIX_B.md` before writing ANY tracked file**, commit message, or code
   comment for this work.
3. **Re-read the three channels fresh** — [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md) (bounded latest
   state and the next increment), [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest first),
   [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md)** — the operator
   asked for the work to continue under that protocol. Its step 1a (**probe before planning**) is not
   optional and has falsified a recorded claim three times in this arc, most recently one written in
   the immediately preceding increment.
5. **In-flight verification: NONE.** No gate, CI run, or background agent pending.

## Resume prompt — WHERE THE PROGRAMME STANDS

The operator's six steps: prototype → mechanism-only crate → document the what → implement in Rust →
port Keleusma → self-host in Keleusma (**the Rust must be Keleusma-like**, which is a constraint on
step 4, not a later concern).

**DONE and merged to `v0.2.3`:**
- **Step 1** — prototype locked in. Both layout-sensitive gaps closed; five layout findings.
- **Step 2** — `keleusma-wire` + `keleusma-wire-derive` crates. Schema-free container: triplicated
  prologue and directory, fixed-stride tables, pools, CRC-32, (72,64) SECDED parity plane, and a
  derive for record offsets. Both `#![forbid(unsafe_code)]`; reader is allocation-free.
- **Step 4 stage 1** — the flattened constant table (`src/wire_schema.rs`).
- **Step 4 stage 2a** — `ConstTable<'a>`, the borrowed accessor. `decode_constants` refactored onto
  it so the two readers share one parse-and-validate path.

**NEXT, in order** (see `REVERSE_PROMPT.md` for the full table): stage 2b is **four or five
increments, not one** — `WireShape`, then `ChunkSignature`, `StructTemplate`, `EnumLayout`,
`DataLayout`, then the scalar header and debug pool. Then **step 5**, routing the runtime through
`ConstTable`, which is where P10 is preserved or lost in practice.

### The narrow requirement that governs the accessor (probed, not assumed)

Exactly **one** accessor must return image-aliasing bytes: a **non-empty top-level** `StaticStr`.
An empty string is deliberately not aliased; a composite's string leaves are **already copied today**.
Do not over-constrain the accessor into a borrow-everything design, and do not lose that one property.

### Operator decisions on record

- **Encoder strategy: option (a)** — one buffer per region, leading directory.
- **Publication: PREPARED but HELD.** Nothing consumes the crate yet, and `Region` gained a field the
  moment the second requirement arrived. Publish only after the first real consumer (step 5) has
  exercised the API. **Publishing is an irreversible outward-facing action: confirm first.**
- Crate scope is **mechanism only**; step 6 covers **both** encoder and decoder.

### Method rules that earned their keep in this arc

- **Run `cargo clippy --all-features`, the `-D warnings` doc build, and `cargo test
  --no-default-features` BEFORE the full gate.** Both of this session's genuine gate reds were in
  exactly those two blind spots, which targeted tests structurally cannot see.
- **Do not touch the tree while a gate runs.** Four gates were lost to this in one session.
- **Assert a borrowed read BY ADDRESS, with a control** proving the predicate rejects an owned copy.
- **Check what a type's `PartialEq` actually compares** before trusting a round-trip test.
  `ConstValue`'s ignores the enum discriminant, which made a whole suite vacuous.
- **Counts agreeing is not a cross-check; values agreeing is.**

**Boundary counts** — **79 Ok / 4 Gap / 1 RefRejects**, unchanged (no `.kel` stage work in this arc).
Pinned by `self_hosted_construct_support_boundary` in `tests/selfhost_codegen.rs`. Recount with a grep.

**Git position**: `v0.2.3` = `5bf71c7` plus this handoff commit, in sync with origin, tree clean. Local
branches: `main`, `v0.2.3`, `v0.2.3-prerebase-backup`. **Do NOT delete `v0.2.3-prerebase-backup`.**

**Guardrails**: no new opcode and no `BYTECODE_VERSION` bump without operator authorization; the
byte-identical differential oracle is the correctness signal; run the FULL gate before claiming
complete; confirm before any irreversible or outward-facing action; never bypass the pre-push gate.

**Environment**: `nvc` 1.23-devel at `/usr/local/bin/nvc`. The prototype lives in `secret/`, which is
gitignored and therefore absent from every commit — rebuild from `secret/silicon-prototype/README.md`
if lost.
