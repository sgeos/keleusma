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

1. **DONE (`c431ffd`) — migrated the fattest record.** `parse.kel`
   `ArrayOfEnumEqBuild` (line ~2467) now sets `ps.emit_arg` to the payload and returns
   the raw kind 63, so its payload (which reached bit 55, one below the ceiling) rides
   its own word. Byte-identical; 89 + 83 tests green. This is the first emit site on the
   `emit_arg >= 0` full-word path.
2. **DONE (`4796746`) — retired the `bnot` split-tag.** Word `bnot` now yields its
   native record kind 65 (the first record kind >= 64, using the unbounded tag space).
   `parse.kel` `bnot_record()` helper sets `emit_arg = 0` and returns 65 (a match arm
   takes an expression, not a `{...}` block — the block form fails to parse, which cost
   one verify cycle to learn). `reconstruct.kel`: the 40..63 routing gate now also admits
   65, and `step_assembly`/`step_bnot` dispatch 65. The Rust `reconstruct_into` needed NO
   change — it never processes bnot; only `reconstruct.kel` does. Byte-identical: 92 + 83
   tests. The remaining split-tag reuses (record 59 -> node 68 for eager and/or; record 54
   -> node 67 for array-of-array-eq) can be retired the same way when convenient.
3. **DONE — token and wire-op streams widened.** `4047a21` (wire-op) and `20ae58f`
   (token). Engineering finding: these two did NOT need two-word — unlike the record
   stream's fat payload, their payloads have headroom, so an 8-bit RADIX WIDENING (tags
   0..255) is safe and far simpler than two-word (no pair-reads, no layout shifts). Wire-op
   is `wire.radix` + two `decode_op` splits; token is 17 sites of `64 -> 256` across the
   lexer emit, host reads/writes, and parse reads. Both byte-identical (92 + 83 tests). So
   all three inter-stage encodings now have ample tag headroom: record unbounded (two-word),
   token and wire-op at 256.

## Remaining Option E follow-ups (future, not started)
- The record stream's other split-tag reuses can be retired like `bnot` did (record 59 ->
  node 68 for eager and/or; record 54 -> node 67 for array-of-array-eq) when convenient.
- Precedence P1 (renumber the self-host precedence scale to match the reference, fixing the
  `a xor b == c` / `a and b xor c` faithfulness defects) is an independent change, still
  open per the brief.

## Verification protocol
- Curated subset + the full `selfhost_parse`/`selfhost_pipeline` binaries + the
  subproject, then `scripts/release-gate.sh` before merge.

## Concern
The byte-identity loop is slow and was confounded all session by an unrelated
CPU-saturating process (EVE Online ~85%). Verify on an idle machine for a clean signal.
The branch is NOT merged to `v0.2.3` yet; the transport + consolidation are a coherent
mergeable unit (behavior-neutral, drift-hazard-retiring) if the operator wants it in
before the capacity increments.
