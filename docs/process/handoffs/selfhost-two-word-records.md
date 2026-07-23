# Handoff: feat/selfhost-two-word-records (P11 Option E)

> Per-branch handoff per [PARALLEL_DEVELOPMENT.md](../PARALLEL_DEVELOPMENT.md) §3.
> Branch cut from `v0.2.3` at `1faf59f`.

## Date
2026-07-23 (session 29)

## Goal
P11 Option E: emit each parse record as an independent `(tag, payload)` pair instead
of `tag + payload*64`, removing the single-word `i64` ceiling. Design in
[ENCODING_CAPACITY_BRIEF.md](../../decisions/ENCODING_CAPACITY_BRIEF.md); mechanical
plan in [P11_OPTION_E_PLAN.md](../../decisions/P11_OPTION_E_PLAN.md).

## State — consolidation (path B) COMPLETE
Implementation surfaced that the parse-record host reader was duplicated across six
sites in four files. The operator chose path B: consolidate first, then change the
protocol once. Done and verified:

- `e822f06` — shared `drive_parse_records` driver in `keleusma`
  (`src/selfhost_host.rs`, `compile`+`verify`-gated); it owns the record-reading loop.
- `d71d1a2` — both subproject drivers (`compiler/src/selfhost.rs` `parse_functions`,
  `compiler/src/main.rs`) routed through it. Subproject tests green, clippy clean.
- `06003d4` — all four test-file drivers (`selfhost_codegen.rs` ×2,
  `selfhost_parse.rs`, `selfhost_pipeline.rs`) routed. 89 tests green (curated
  whole-stage byte-identity + the full parse/pipeline binaries), clippy clean.

Six copies of the loop are now one. The behavior is byte-identical (pure refactor).

## Exact next step — the two-word transport change (now a 2-edit payoff)
Because every caller routes through `drive_parse_records`, the protocol change is
localized:

1. `src/selfhost_host.rs` `drive_parse_records`: pair-read. On a `Yielded(t)`, resume
   once to read the payload word `arg`; then `(code,val) = if arg == -1 { (t%64, t/64) }
   else { (t, arg) }`. The `for _ in 0..budget` still bounds records (each iteration
   now consumes two yields), so **caller budgets do not change**.
2. `compiler/kel/parse.kel`: the two-phase `step()` wrapper. Add `emit_phase`,
   `emit_arg`, `pending_arg` to `private data ps` (line ~308); rename the existing
   `fn step()` to `fn step_body()`; add a `fn step()` wrapper that on phase 0 sets
   `emit_arg = 0 - 1`, calls `step_body()`, stashes `pending_arg = emit_arg`, sets
   `emit_phase = 1`, returns the tag word; on phase 1 returns `pending_arg` and clears
   the phase. The `loop main { yield step() }` is untouched.

With the `-1` sentinel, every record still travels the old path (`arg == -1`), so the
change is behavior-neutral and byte-identical. Later increments migrate specific emit
sites (the fat array-of-composite records, high tags) to a full-word payload / raw tag.

## Verification
- Consolidation: subproject `cargo test` green; 89 main-workspace tests green; clippy
  `-D warnings` clean on both.
- For the two-word change, verify with the curated subset plus the full
  `selfhost_parse`/`selfhost_pipeline` binaries plus the subproject, then
  `scripts/release-gate.sh` before merge.

## Concern
The byte-identity verification loop is slow and was confounded all session by an
unrelated CPU-saturating process (EVE Online at ~85%). The two-word change is on the
hot path of every self-host compile; verify on an idle machine for a trustworthy signal.
