# Handoff: feat/selfhost-two-word-records (P11 Option E)

> Per-branch handoff per [PARALLEL_DEVELOPMENT.md](../PARALLEL_DEVELOPMENT.md) §3.
> Branch cut from `v0.2.3` at `1faf59f`.

## Date
2026-07-23 (session 29)

## Goal
Implement P11 Option E on the record stream: emit each parse record as an
independent `(tag, payload)` pair instead of `tag + payload*64`, removing the
single-word `i64` ceiling. Design in
[ENCODING_CAPACITY_BRIEF.md](../../decisions/ENCODING_CAPACITY_BRIEF.md); mechanical
plan in [P11_OPTION_E_PLAN.md](../../decisions/P11_OPTION_E_PLAN.md).

## State
- Design, brief, and the phased implementation plan are committed. The protocol is
  pinned: a backward-compatible sentinel (`emit_arg = -1`) via a **two-phase
  guarded yielding function** `emit_record` (the proven `codegen.kel` `emit_next`
  idiom — a `loop` body cannot hold two sequential yields), so emit sites migrate
  one at a time, each byte-identical.
- No code edits to `parse.kel` / `selfhost.rs` yet. The branch carries the plan only.

## Verification
- md-links green. Items 1/7/P11-design already merged to `v0.2.3` (`1faf59f`,
  pushed). This branch adds only the plan doc + this handoff so far.

## Exact next step (Increment 1 — behavior-neutral transport)
1. `parse.kel`: add `emit_phase`, `emit_arg`, `pending_arg` to `private data ps`
   (line 308); add the guarded `emit_record` yield-fn and repoint `loop main`
   (line 4938) per the plan's protocol block.
2. `selfhost.rs` `parse_functions` (561-630): read yields in pairs `(t, arg)`; if
   `arg == -1` use today's `(t%64, t/64)`, else `(t, arg)`; roughly double the
   iteration budget at line 561.
3. Verify: `scripts/fast-check.sh 'test(self_host_compiles_parse_kel_byte_identically)'`
   first, then the full self-host suite / `release-gate.sh` before merge.

## Concern
The byte-identity verification loop is slow and was confounded this session by an
unrelated CPU-saturating process; run Increment 1's verification on an idle machine
for a trustworthy signal. The change is on the hot path of every self-host compile —
keep the two-phase yield-fn minimal.
