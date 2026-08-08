# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `702c90f`
- **Written**: 2026-08-07
- **Tree at write**: clean. **The active work is on an UNPUSHED branch — see below.**
- **Before writing anything tracked, read `secret/notes/APPENDIX_B.md`.** Hard constraint.

**Validity check — run on resume.** Compare the **Parent commit** above to `git rev-parse HEAD~1`.

- **Match → VALID.** Proceed per the resume prompt.
- **Mismatch → INVALID and STALE.** Report it, familiarize from the three channels and the git log,
  and wait for instruction.

## On resume — do these first

1. **Run the validity check.**
2. **Read `secret/notes/APPENDIX_B.md` before writing ANY tracked file**, commit message, or comment.
3. **Re-read the three channels** — [`REVERSE_PROMPT.md`](./REVERSE_PROMPT.md),
   [`DESIGN_JOURNAL.md`](./DESIGN_JOURNAL.md) (newest first), [`TASKLOG.md`](./TASKLOG.md).
4. **Read [`AUTONOMOUS_IMPLEMENTATION_LOOP.md`](./AUTONOMOUS_IMPLEMENTATION_LOOP.md).** Step 1a
   (probe before planning) falsified something in **every** increment of this arc.
5. **In-flight verification: NONE.** Any background run from the previous session is dead.

## THE ACTIVE WORK: a complete but UNVERIFIED cutover on an unpushed branch

```
git checkout feat/wire-cutover-proper     # local only, NOT on origin
```

- **`v0.2.3` = `702c90f`** plus this handoff commit. Green, pushed, in sync with origin.
- **`feat/wire-cutover-proper` = `c29abcf`.** The wire-format v2 cutover, **functionally complete**,
  **committed**, **not pushed**, and **not fully verified**.

### What the cutover did

`BYTECODE_VERSION` is **2** (operator-authorised 2026-08-06: "the substrate itself has changed").
The auxiliary body is the v2 format end to end; rkyv no longer touches it (the dependency stays —
six `AlignedVec` uses are unrelated). Five rkyv paths were ported: both encode sites, the cold
loader decode, the 26 `Vm::archived()` reads, the zero-copy validation entry, and `decode_all_ops`.

`Module::access_bytes` became `Module::validate_bytes` — the old signature returned
`&ArchivedWireAuxBody`, a type that no longer describes anything. The v2 body is byte-addressed, so
the 8-byte alignment requirement is gone everywhere.

### VERIFICATION STATUS — the gate has NOT passed

**Verified green**: format, clippy `--all-features -D warnings`, docs `-D warnings`, 1231 lib tests,
92 schema tests, the ten-stage corpus differential, the randomised suite, `--no-default-features`.
The default-features workspace pass reached 1963 log lines with zero failures before its session died.

**NOT verified**: `signatures`, `signatures,shell`, `self-host`, both `keleusma-wire` feature
configurations, the markdown-link check, and the detached `compiler/` subproject.

**Do not merge until a full `scripts/release-gate.sh` is green.** No failure has been seen anywhere,
but absence of evidence is not verification.

### TWO THINGS TO PUT TO THE OPERATOR

1. **The golden-bytes test was regenerated** (`bytecode_golden_bytes_for_main_returning_one`).
   Judged legitimate: the test's own message directs updating it deliberately after a version bump,
   the bump is authorised, the round-trip half still EXECUTES the bytes, and the other failure was
   diagnosed separately rather than blanket-updating. Regenerating an expectation is nevertheless how
   real drift gets laundered, so it is flagged rather than buried. One-line revert if unwanted.
2. **`CLAUDE.md` still says `BYTECODE_VERSION` stays 1** under the no-public-adoption policy. That
   text is now wrong and must be updated. The hazard it records — an old artifact accepted-then-
   mis-read — is *resolved* for v1 artifacts, which now fail the version check.

## RUNNING THE GATE: the operator should do it

Long background runs **do not survive** this environment. The operator closes the laptop and leaves
WiFi range; every such run this session was killed mid-flight, with zero failures in the surviving
logs. Elapsed-time readings are meaningless for the same reason (they include suspend).

**Ask the operator to run `! scripts/release-gate.sh` at merge points.** It survives a dropped
session in a way an agent-launched background task does not.

## FASTER ITERATION — the lane that exists and was not used

`scripts/fast-check.sh 'test(<filter>)'` is a seconds-scale inner loop (fmt, clippy on the touched
crate, one test filter). `cargo-nextest` and `sccache` are installed and configured.

**One caveat, learned the hard way**: the `KEL_SELFHOST_CACHE=1` memoization is a COMPLETE KEY — it
reuses a result only when the test binary *and* every `.kel` input are byte-identical. Any Rust edit
invalidates all of it. It accelerates `.kel` stage work, **not** Rust-side work like this cutover.

The process that would actually have helped, and should be used from here:

- **Inner loop**: run only the tests for the work in hand, not full suites.
- **Pre-gate** (minutes): clippy `--all-features`, the `-D warnings` doc build, and
  `--no-default-features`. This caught four defects that targeted tests structurally cannot see.
- **Gate**: once per merge, and **batch three or four increments per merge**. Roughly twenty full
  gates were run this session, one per increment; the output would have been identical batched.

## What is DONE, merged, and green on `v0.2.3`

- **`keleusma-wire` + `keleusma-wire-derive`** — schema-free container: triplicated prologue and
  directory, fixed-stride records, pools, CRC-32, (72,64) SECDED plane, `#[derive(WireRecord)]`.
  Both `#![forbid(unsafe_code)]`; reader allocation-free; builds for `wasm32v1-none`.
- **`src/wire_schema.rs`** — the whole aux body encodes and round-trips; `SchemaBuilder` owns the
  shared state (name interner, shape table, constant table).
- **`AuxView` / `AuxOffsets`** — the runtime read surface, resolve-once/reconstruct-cheaply.
- **Validation** — 92 schema tests, the ten-stage corpus, randomised input testing.

## The narrow requirement that still governs

Exactly **one** accessor returns image-aliasing bytes: `AuxView::chunk_const_str_bytes`, for a
**non-empty top-level** `StaticStr`. Empty strings are deliberately not aliased; a composite's string
leaves are **already copied**. Asserted by address, with a control proving the predicate rejects a
copy. Do not over-constrain into a borrow-everything design; do not lose that one property.

## Also outstanding

- **Publication HELD.** Operator: "push, but do not yet publish" (2026-08-06). Neither crate is
  published. **Irreversible and outward-facing — confirm first.**
- **MSRV 1.85 declared, never verified.**
- **The corpus emits zero struct templates**; that table rests on hand-built cases. The corpus test
  asserts the zero so the caveat cannot go stale.

## Method rules this arc validated

- **Probe before planning.** Falsified a claim in every increment, including ones written by me one
  increment earlier, and a limitation in already-merged code.
- **Ask what a test would still pass with.** Four succeeded emptily before being caught:
  `ConstValue::PartialEq` ignores the enum discriminant; ECC tests compared counts not values; the
  fuzz suite reached the readers 0/2000 times; and pairing `native_return_shapes` with
  `native_names` silently dropped the surplus.
- **A green build proves nothing during a format cutover.** `access_unchecked` type-checks against
  any byte range, so the compiler was blind for the whole port — 320 tests failed while it compiled.
- **Measure, do not theorise.** Three wrong guesses about a performance cliff cost ~30 minutes;
  instrumentation found it in one run.
- **Do not touch the tree while a gate runs.** Four gates lost to this.

**Boundary counts** — **79 Ok / 4 Gap / 1 RefRejects**, unchanged. Recount with a grep.

**Git**: `v0.2.3` = `702c90f` plus this handoff commit, in sync with origin. Local branches: `main`,
`v0.2.3`, `v0.2.3-prerebase-backup`, `feat/wire-cutover-proper`. **Do NOT delete
`v0.2.3-prerebase-backup`** (309 commits not in `v0.2.3`, a deliberate safety net).

**Guardrails**: no new opcode without authorization; run the FULL gate before claiming complete;
confirm before any irreversible or outward-facing action; never bypass the pre-push gate.
