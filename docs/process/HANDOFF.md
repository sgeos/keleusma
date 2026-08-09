# Handoff Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

The self-contained, imperative resume prompt, written **before a planned compaction** and validated
on resume. Unlike the three resume channels it is **not** kept always-current. It is a snapshot
stamped with the commit it describes, so a stale handoff self-reports as stale rather than misleading
a resuming agent.

## Validity

- **Branch**: `v0.2.3`
- **Parent commit** (the repository state this handoff describes): `917da62`
- **Written**: 2026-08-08
- **Tree at write**: clean apart from this handoff commit. **Nothing unmerged, nothing unpushed.**
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
   (probe before planning) falsified a recorded claim in **every** increment of this arc, including a
   plan document that said the current work was blocked when it was not.
5. **In-flight verification: NONE.** No background run survives a session boundary here.

## THE STATE: everything is merged, pushed, and green

`v0.2.3` = `917da62`, in sync with origin, full gate green, CI green. **There is no unmerged work and
no local-only branch.** This is a clean resume point, unlike the previous handoff.

The six-step wire-format programme:

| Step | State |
|---|---|
| 1. Prototype to lock-in | done |
| 2. Standalone `keleusma-wire` crate | done, merged |
| 3. Document the format | done, merged |
| 4. Implement in Rust | done, merged |
| 5. Port Keleusma to it | done, merged |
| **6. Self-host it in Keleusma** | **not started — this is the work** |

`BYTECODE_VERSION` is **2**, operator-authorised 2026-08-06 because the substrate changed. A
version-1 artifact is now rejected on the version check. Any further bump needs authorization.

## THE NEXT INCREMENT: step 6, slice 1 — CRC-32 in Keleusma

**Read `docs/decisions/WIRE_FORMAT_SELFHOST_PLAN.md` from the top.** It was stale until 2026-08-08:
it concluded self-hosting was blocked on the rkyv auxiliary body and advised doing the monomorphizer
and type checker instead. That blocker was removed by changing the wire format, which is what this
programme did. It is now re-scoped, with seven slices against the v2 container.

**Slice 1 is CRC-32.** Bitwise, table-free, a pure function over a byte range, so it transliterates
directly. Oracle: `crate::bytecode::crc32` on random and edge-case buffers. It is small on purpose —
its job is to establish the byte-emission harness every later slice needs.

**Probe before writing any of it**, because it is unsettled and every slice inherits it: how a `.kel`
stage addresses a byte buffer, for emission and for reading. The `secret/kel-format-probe/` prototype
used a data segment. That prototype **predates format lock-in** — 12-byte directory entries where the
shipped entry is 16, and no triplicated prologue at all. Feasibility evidence, not a starting point.

**Carry this constraint:** `ConstTable::value` uses `BTreeSet`/`BTreeMap`, which Keleusma does not
have. The decoder needs a bounded array-based walk; the forward-ordering invariant is what makes one
terminate. Write it, do not transliterate it.

## The performance guard, and why it exists

`tests/perf_canary.rs` is new. The cutover merged green on twelve gate steps **and would also have
merged green while forty times slower**, because nothing else measures time.

- It is a **tripwire, not a benchmark.** The ceiling is deliberately slack.
- **If it fails, profile before touching the ceiling.** The class it catches is a hot-path read that
  has become proportional to the whole module. Correctness tests will keep passing.
- It was validated by reverting the repair: 1.7 s becomes 67.3 s and it trips.

Reference points, debug, uncontended, 200k iterations: rkyv 6.4 s, v2 as first committed 67.3 s, v2
repaired 1.2 s. **The v2 read path is 5.2× faster than rkyv.**

## Verification process — operator-directed, now binding

Full gate before every **merge**, not after every **change**. See
[`PROCESS_STRATEGY.md`](./PROCESS_STRATEGY.md#tiered-verification).

- **Tier 0**, per edit: `scripts/fast-check.sh 'test(<filter>)'`.
- **Tier 1**, per increment: `clippy --workspace --all-targets -D warnings`,
  `cargo test -p keleusma --no-default-features`, and the `-D warnings` doc build. These three catch
  what targeted tests structurally cannot.
- **Tier 2**, per merge: `scripts/release-gate.sh`, **batching three or four increments**.

A full gate is about 1h40m. **Run it in the background and monitor the log**; it survives fine when
launched that way, contrary to the previous handoff's advice. The feature matrix was deliberately not
narrowed — the reasoning is in the process doc.

**Reap orphans before any gate or measurement.** An interrupted gate leaves its test binary
reparented to PID 1 at full CPU; one was found burning four cores for ten hours. `release-gate.sh`
now does this as a preflight.

## Method rules this arc validated

- **Probe before planning.** It has now falsified a claim in every increment, including a persisted
  plan document and a published specification.
- **Ask what a test would still pass with.** Five have succeeded emptily this arc. The most recent
  was a differential test written for a bug already in hand, which passed *with that bug present*
  because every integer in it was small.
- **Never measure performance on a build you have not just re-verified.** A 3.7× "speedup" this
  session came from a build where constant loads were erroring out early.
- **A green build proves nothing during a format cutover**, and a green suite proves nothing about
  speed.
- **Do not touch the tree while a gate runs.**

## Also outstanding

- **Publication HELD.** Nothing is published. **Irreversible and outward-facing — confirm first.**
- **MSRV 1.85 declared, never verified.**
- **Fifteen `self.aux()` sites remain, audited, none hot** — a legitimate follow-up, not a blocker.
- **The corpus emits zero struct templates**; the corpus test asserts the zero so the caveat cannot
  go stale.

**Boundary counts** — **79 Ok / 4 Gap / 1 RefRejects**. Recount with a grep rather than trusting this.

**Git**: `v0.2.3` = `917da62` plus this handoff commit, in sync with origin. Local branches: `main`,
`v0.2.3`, `v0.2.3-prerebase-backup`. **Do NOT delete `v0.2.3-prerebase-backup`** (309 commits not in
`v0.2.3`, a deliberate safety net).

**Guardrails**: no new opcode or `BYTECODE_VERSION` bump without authorization; full gate before any
merge; confirm before any irreversible or outward-facing action; never bypass the pre-push gate.
