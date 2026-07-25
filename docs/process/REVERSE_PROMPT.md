# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-25 (session 31)

## Current state

- **`v0.2.3` (release line), CI-GREEN.** The P11 encoding-capacity change
  (Option E) is complete and merged: the record stream uses a two-word `(tag, payload)`
  transport (single-word `i64` ceiling removed), the token and wire-op streams an 8-bit
  radix, every split-tag workaround is retired with native `>= 64` record kinds, and
  precedence P1 fixed the `xor`/`and` folding defects (`xor` became its own opcode that
  still lowers to `CmpNe`). The six duplicated parse-record host drivers were consolidated
  into one `keleusma::selfhost_host::drive_parse_records` first. See
  [`docs/decisions/P11_OPTION_E_PLAN.md`](../decisions/P11_OPTION_E_PLAN.md) and
  [`ENCODING_CAPACITY_BRIEF.md`](../decisions/ENCODING_CAPACITY_BRIEF.md) (both RESOLVED).
- **CI now gates the release line.** It triggers on `main` and any `v*` branch and includes
  a `selfhost-compiler` job for the detached subproject. The construct-support boundary is
  now **47 Ok / 7 Gap / 1 RefRejects** (the two precedence Gaps closed).
- **`main` at `4483f43`** is now a clean ancestor of `v0.2.3`, which was rebased to sit purely
  ahead of it (see Branching model).

## Verification

- The full CI gate is green on `v0.2.3`: feature matrix, the entire self-host
  suite, the subproject, doc, miri, clippy, MSRV, no_std, LSP, WASM.
- Enabling CI on the release line immediately caught two real defects that curated local
  runs (confounded by a CPU-saturating process all session) had missed — subproject fmt
  drift, and a token-radix site in `tests/selfhost_lexer.rs` the 8-bit widening had omitted.
  Both fixed and green. This is the concrete value of gating the release line.

## Process-audit residual status

- **Items 2–5**: done and merged. Item 5's last residual (the `TASKLOG.md` Active Milestone
  narrative) was relocated into the append-only `DESIGN_JOURNAL.md` on 2026-07-24, so `TASKLOG`
  is now a bounded current-state file. Item 3's memoization is now IMPLEMENTED in the
  complete-key form (`tests/common/mod.rs`, 2026-07-24): the expensive whole-stage
  self-compile and analyze tests are memoized on a key of the test-binary identity (hence
  the whole Rust reference compiler, VM, and wire format) plus every `.kel` input the test
  reads, active only under `KEL_SELFHOST_CACHE=1` (the fast lane, never a gate). Verified
  that gate mode never caches, a `.kel` or binary change forces a miss, and the eight
  heaviest self-host tests drop from ~102s to ~0.02s on a warm cache. Follow-ups (2026-07-25)
  extended it to the assembled/scaffold family (~291s to ~0.024s) and the parse_into_codegen
  bridge (~58s to ~5.2s, 14 of 16 via a shared cached helper), so every test category that
  cost time is now cached, all CI-green.
- **Item 4 (gate blind spot)**: closed in both places — `release-gate.sh` and now a CI job.
- **Item 6 (encoding capacity)**: implemented, merged, CI-green (Option E, above).
- **Item 1 (nextest cap): DONE.** The tier split (routine `quick` vs full) is merged. The
  audit-mandated measurement finally ran on the idle 10-core box and **refuted the cap**: the
  heavy self-host suite ran 1127s at `max-threads=2` versus 632s uncapped (1.8x slower — the
  cap serialized the tests below the core count). The `SLOW [>960s]` figure was a
  loaded-machine confound. Per the audit's "keep only if wall-clock drops", the
  `heavy-selfhost` cap was REMOVED; `test-threads` remains the concurrency/memory bound.

## Branching model (resolved)

- **`v0.2.3` was rebased to sit purely ahead of `main`** (2026-07-24). `main` (`4483f43`) is now
  a clean ancestor of `v0.2.3`, which is 307 ahead and 0 behind with a linear history, so the two
  lines no longer diverge. The rebase preserved a byte-identical tree. `main`'s duplicate
  frontend-fix was dropped as already-applied and the only conflict was ci.yml comment wording.
  A local `v0.2.3-prerebase-backup` retains the pre-rebase tip pending cleanup.
- **`main`'s `ci.yml`** (commit `4483f43`) carries the `v*` trigger and the `selfhost-compiler`
  job, so both `main` and `v0.2.3` are CI-gated and any future version branch cut from either is
  gated from the start.

## Item 7 (autonomy and parallelism)

- **Parallelism**: infrastructure complete — worktree isolation, serialized merge, shared
  caches (sccache plus the item-3 test memoization), CI gating, and a workstream-ownership map
  with an honest coupling analysis in [PARALLEL_DEVELOPMENT.md](./PARALLEL_DEVELOPMENT.md).
- **Autonomy**: the substrate is now written,
  [AUTONOMOUS_IMPLEMENTATION_LOOP.md](./AUTONOMOUS_IMPLEMENTATION_LOOP.md) (2026-07-25). It
  encodes the keep-going default (proceed on obvious increments without re-issue), the
  increment cycle, the byte-identical oracle as the hard signal, and the explicit stop
  conditions. Authorizing the loop to run remains the operator's call.

## Next step

All actionable process-audit items (1–6) are addressed and the branching finding is resolved;
item 7's prep is now complete on both halves, pending only the operator's go. The frontier was
re-scouted post-P11 (2026-07-25, in `DESIGN_JOURNAL.md`), closing the last prep residual: the
stale 2026-07-22 recipe is superseded, **tuple-of-struct** is the confirmed smallest-bounded
first increment (step 1 `tup_estruct` already in `v0.2.3`), and it needs no new opcode (the
nested extract reuses op 53). The frontier is now drivable by the autonomy loop substrate.
