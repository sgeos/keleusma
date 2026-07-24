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

## State — two-word transport LANDED (`6852176`, behavior-neutral)
The payoff of consolidating first: the transport change was a single edit to the
shared driver plus the `parse.kel` emit.

- `parse.kel`: two-phase `step()` wrapper (`emit_phase`/`emit_arg`/`pending_arg` on
  `ps`). Phase 0 computes the record and yields the tag word, stashing the payload;
  phase 1 yields the payload word. `ps` is private data, so the phase state persists
  across the productive loop's per-iteration RESET — the one subtlety the design had
  missed.
- `src/selfhost_host.rs` `drive_parse_records`: pair-reads the `(tag, payload)`,
  **skipping the RESET the loop emits between yields**, then `(code,val) = if arg == -1
  { (t%64, t/64) } else { (t, arg) }`. The `-1` sentinel keeps every record on the old
  path, so byte-identity holds; caller budgets are unchanged (each iteration still
  bounds one record).

Verified byte-identical: 89 main-workspace tests + 83 subproject tests; clippy clean.

## Next — capacity exploitation (the actual gain; each a small verified increment)
The transport removes the ceiling but is behavior-neutral until emit sites use it:

1. **Migrate the fattest record** (`parse.kel` `ArrayOfEnumEqBuild`, ~2457 in the
   original): set `ps.emit_arg` to the (now unpacked) payload and return the raw kind
   instead of `... * 64`. Same value/tag for now, so still byte-identical — proves a
   real record round-trips the two-word path.
2. **Retire a split-tag workaround**: give a record that currently reuses a low tag for
   a high node kind (for example `bnot` -> record 48 -> node 65) its NATIVE high tag
   (>= 64), adding the matching dispatch arm in `reconstruct.kel` `step_assembly`. First
   use of the newly unbounded tag space; verify byte-identical.
3. Later, the token and wire-op streams get the same two-word shape for uniformity.

## Verification protocol
- Curated subset + the full `selfhost_parse`/`selfhost_pipeline` binaries + the
  subproject, then `scripts/release-gate.sh` before merge.

## Concern
The byte-identity loop is slow and was confounded all session by an unrelated
CPU-saturating process (EVE Online ~85%). Verify on an idle machine for a clean signal.
The branch is NOT merged to `v0.2.3` yet; the transport + consolidation are a coherent
mergeable unit (behavior-neutral, drift-hazard-retiring) if the operator wants it in
before the capacity increments.
