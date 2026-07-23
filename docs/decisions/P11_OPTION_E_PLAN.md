# P11 Option E — Implementation Plan (two-word record stream)

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: APPROVED (operator, 2026-07-23) — "implement E, record stream first".
Branch: `feat/selfhost-two-word-records`. Design rationale is in
[`ENCODING_CAPACITY_BRIEF.md`](./ENCODING_CAPACITY_BRIEF.md); this file is the
mechanical plan.

## What the investigation established

- Parse emits records at a **single yield site**: `parse.kel:4939 yield step()`.
  `step()` returns one packed word `kind + payload*64` (kind &lt; 64).
- The host splits it (`selfhost.rs:563`, `(w % 64, w / 64)`) and stores the record
  as a decoupled `(code, val)` pair in `ParsedFn.body`/`data_records`/`enum_records`.
- The host then writes those pairs into reconstruct's `rec_kind[i]` / `rec_arg[i]`
  as **full i64 values** (`selfhost.rs:726-731`), and reconstruct reads the two
  arrays separately (`reconstruct.kel:828`).

So tag and payload are already de-coupled everywhere except parse's single packed
yield. Removing that packing is the whole change; reconstruct is untouched.

## The protocol: a backward-compatible sentinel (enables incremental site migration)

Add a parse global `emit_arg`, reset to a sentinel `-1` each iteration. The main
loop yields the record as **two words** — the tag word `t`, then `emit_arg`:

```
loop main(resume: Word) -> Word {
    ps.emit_arg = -1;      // sentinel: "this record is still old-packed"
    let t = step();
    yield t;
    yield ps.emit_arg;
}
```

Host reads the pair `(t, arg)`:
- `arg == -1` — an un-migrated (old-packed) record: `code = t % 64`, `val = t / 64`
  (exactly today's behavior).
- `arg >= 0` — a migrated (two-word) record: `code = t` (a full-word tag, may be
  &ge; 64), `val = arg` (a full-word payload).

Real payloads are always `>= 0`, so `-1` is an unambiguous sentinel. Control codes
(5, 15, 16, ...) are returned raw by `step()` with no payload, so they ride the
`arg == -1` path unchanged (`code < 64`, `val = 0`).

This lets emit sites migrate one at a time, each landing byte-identical, instead of
a big-bang rewrite of all ~40 `* 64` sites.

## Increments (each ends green; verify with the byte-identity corpus)

**Increment 1 — install the transport, migrate zero sites (behavior-neutral).**
- `parse.kel`: add `emit_arg` to the `ps` state; change the `main` loop to the
  two-yield form above.
- `selfhost.rs`: restructure the yield-reading loop (`parse_functions`, 561-630) to
  read pairs `(t, arg)` and apply the sentinel rule, producing the same `(code, val)`
  it produces today. No emit site changes yet.
- Verify: parse.kel still self-compiles byte-identically and the whole self-host
  suite is green. Because every record still takes the `arg == -1` path, the data
  handed to reconstruct is bit-for-bit identical, so the final Module bytes are
  unchanged. This increment only proves the two-word transport.
- If the `loop` body cannot hold two `yield`s, fall back to a helper or a small
  state machine in `step()`; confirm on a one-test `fast-check.sh` before the full run.

**Increment 2 — migrate the fattest record to the two-word form (proof of headroom).**
- `parse.kel:2457` `ArrayOfEnumEqBuild`: instead of `... * 64`, set `ps.emit_arg`
  to the (now unpacked) payload and return the raw kind. Keep the SAME kind value
  and payload contents for now, so it is still byte-identical, but it now travels
  the `arg >= 0` path.
- Verify byte-identical. This proves a real record round-trips through two words.

**Increment 3 — retire a split-record/node-kind workaround (the payoff).**
- Pick one record that currently reuses a low tag for a high node kind (for
  example `bnot` -> record 48 -> node 65) and give it its **natural high tag**
  (&ge; 64), adding the matching dispatch arm in `reconstruct.kel`'s
  `step_assembly`. This is the first use of the newly unbounded tag space and
  removes one workaround. Verify byte-identical (the final ops are unchanged; only
  the internal record tag differs).

**Later — token and wire-op streams.** Apply the same two-word shape to the token
stream (lexer emit / parse consume) and the wire-op stream (codegen emit /
`decode_op`) for uniform future-proofing. Out of scope for the record-stream-first
milestone; track as follow-ups.

## Verification per increment

- Inner loop: `scripts/fast-check.sh 'test(self_host_compiles_parse_kel_byte_identically)'`
  and the relevant construct test.
- Before merge: `scripts/release-gate.sh` (the full self-host suite + feature matrix
  + subproject), per the merge protocol in
  [`PARALLEL_DEVELOPMENT.md`](../process/PARALLEL_DEVELOPMENT.md).
- No `Vm::new_unchecked`. The record encoding is internal, so byte-identity of the
  final Module is the invariant that guards every increment.

## Risk notes

- The `main` loop and `step()` are on the hot path of every self-host compile;
  keep the two-yield change minimal.
- The sentinel `-1` must never collide with a real payload. Real payloads are
  non-negative by construction (offsets, counts, interned ids); assert this in the
  host when taking the `arg >= 0` branch during bring-up.
- Increment 1 changes parse.kel's own bytecode, so the reference and self-host must
  agree on compiling the new parse.kel; that is exactly what the byte-identity test
  checks.
