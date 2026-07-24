# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning and frontier assessments live in
[DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-07-24 (session 30)

## Current state

- **`v0.2.3` (release line) at `72a648e`, CI-GREEN.** The P11 encoding-capacity change
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
- **`main` at `7494435`** is diverged behind `v0.2.3` (see Concerns).

## Verification

- The full CI gate is green on `v0.2.3` `72a648e`: feature matrix, the entire self-host
  suite, the subproject, doc, miri, clippy, MSRV, no_std, LSP, WASM.
- Enabling CI on the release line immediately caught two real defects that curated local
  runs (confounded by a CPU-saturating process all session) had missed — subproject fmt
  drift, and a token-radix site in `tests/selfhost_lexer.rs` the 8-bit widening had omitted.
  Both fixed and green. This is the concrete value of gating the release line.

## Process-audit residual status

- **Items 2–5, 7**: done and merged.
- **Item 4 (gate blind spot)**: closed in both places — `release-gate.sh` and now a CI job.
- **Item 6 (encoding capacity)**: implemented, merged, CI-green (Option E, above).
- **Item 1 (nextest cap): DONE.** The tier split (routine `quick` vs full) is merged. The
  audit-mandated measurement finally ran on the idle 10-core box and **refuted the cap**: the
  heavy self-host suite ran 1127s at `max-threads=2` versus 632s uncapped (1.8x slower — the
  cap serialized the tests below the core count). The `SLOW [>960s]` figure was a
  loaded-machine confound. Per the audit's "keep only if wall-clock drops", the
  `heavy-selfhost` cap was REMOVED; `test-threads` remains the concurrency/memory bound.

## Concerns / operator decisions pending

- **`main` catch-up.** `main` is diverged behind `v0.2.3`, and its own `ci.yml` does not yet
  carry the `v*` trigger (so a version branch cut from `main` would not be gated until it
  picks up v0.2.3's workflow). The release line IS gated now, so this is no longer urgent,
  but reconciling `main` (catch it up to the release line, or at least land the `v*` trigger
  there) is an operator decision. Offered to prepare that PR.

## Next step

All process-audit items are addressed. `main`'s `ci.yml` gets the `v*` trigger + a
`selfhost-compiler` job (operator chose to update main's CI config, not a full catch-up).
Then await operator direction on the next roadmap target — the nested-composite-equality
frontier (enum-in-struct, tuple-of-struct, 2+-level) is now unblocked by the freed encoding
capacity.
