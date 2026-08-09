# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

Prior sessions' blocks were removed here rather than allowed to accrete, which is what this
file's spec asks for and why it once reached 362 KB. Nothing was discarded: the reasoning is
in the design journal, and the two items it did not already hold were carried into its newest
entry before this overwrite.

---

## Last Updated

**Date**: 2026-08-09 (session 40)

## Completed this session — step 6, slices 1 and 2

**The six-step wire-format programme stands at 1 to 5 done and merged; step 6 slices 1 and 2
done, slices 3 to 7 open.** Both are on `feat/selfhost-wire-crc32`, two commits, unmerged.

Two new files:

- **`src/selfhost/kel/wire.kel`** — CRC-32/ISO-HDLC written in Keleusma. It is deliberately
  **not** in `read_stage`'s table, because the self-hosted compile driver does not run it: it
  does not yet emit an artifact. It joins the table when it produces bytes rather than a
  checksum. The module doc in `src/selfhost/mod.rs` says so, since the directory's contract
  was previously "the ten stage sources".
- **`tests/selfhost_wire.rs`** — the differential. Twenty-three tests, 0.97 s.

**Slice 1** is CRC-32/ISO-HDLC. **Slice 2** is the little-endian place-value primitives, the
16-byte prologue, and the majority-of-three vote, and its oracle is stronger: **byte identity**
against what `keleusma-wire` emits, at 0, 1, 2, 7, 255, 256, 1023 and 1024 regions.

**Tier 1 green**: `fmt`, `clippy --workspace --all-targets` and `--all-features` at
`-D warnings`, `cargo test -p keleusma --no-default-features`, and the `-D warnings` doc build.
**The full gate has NOT been run for this branch.** Per the tiered-verification policy it runs
once per merge, batching three or four increments, so it is owed before this merges.

### The oracle is a published constant, not our own code

`crc32("123456789") == 0xCBF43926` is the standard CRC-32/ISO-HDLC check value, and both Rust
implementations are already independently pinned to it. The test compares against
`keleusma_wire::crc32` rather than the `crate::bytecode::crc32` the plan named, only because
the latter is `pub(crate)` and unreachable from an integration test. Same algorithm and
polynomial.

### Three recorded claims were falsified by probing, one of them mine

1. **No masking is required.** The handoff expected `band 0xFFFFFFFF` after each step because
   `Word` is signed. The accumulator is always in `[0, 2^32)` by construction, so a mask would
   be dead work. That design note is now corrected in the plan document.
2. **`require word >= 32` would have been a silent defect.** Every pipeline stage declares it,
   so the reflex is to match, but a 32-bit signed `Word` holds neither the initial value nor the
   polynomial — and, verified against the reference, **a source carrying those literals compiles
   for a 32-bit target with no complaint when no `require` is present.** `wire.kel` declares
   `>= 64`, which the reference confirms rejects both narrow targets.
3. **The two constraints the `v0.3.0` session reported are both real**, and now confirmed here
   rather than taken on trust. Locals are immutable, rejected at **parse**; a runtime-range
   `for` needs `limit`, rejected at **verify**. Also settled: `Byte as Word` zero-extends, `lsr`
   is logical over the full word, and a bounded `for` works inside a `fn` and across a call.

### The must-fire control earned its keep by failing on its first run

A mutated polynomial does **not** change the answer for the single byte `0xFF`:
`0xFFFFFFFF xor 0xFF` is `0xFFFFFF00`, whose low eight bits are clear, so all eight iterations
take the else branch and the polynomial is never consulted. Enumerating all 256 single-byte
inputs shows `0xFF` is the only such case.

The response was **not** to relax the assertion to a count. The suite asserts the blind set
**exactly**, so a case that joins it later fails loudly and has to be explained.

A corollary that is itself a coverage gap, so it is pinned rather than left implicit: because
the accumulator is never negative, **`asr` and `lsr` compute identical values here**, and
swapping them is invisible to the differential. That equivalence has its own test.

### The probe apparatus failed before the code did

The first probe run reported six constructs rejected at `Vm::new`. All six were my arena
carrying zero persistent capacity, so every module with a `private data` block failed for a
reason unrelated to the language. Taken at face value it would have been recorded as a language
restriction and would have redesigned the slice. The trivial must-not-fire case in the probe is
what exposed it. **A probe needs its own control, exactly as a test does.**

### Slice 2 — two details of the reference a transliteration would get wrong

Both were found by reading the reference before writing, and neither is something a passing test
would have surfaced afterwards.

1. **`maj3` is a per-BIT majority**, `(a & b) | (a & c) | (b & c)`, not "pick the value that
   appears at least twice". Where all three copies differ it synthesises a byte no copy contains,
   which repairs three independent single-bit faults in three different copies. The distinction
   is invisible unless a case with three distinct bytes is exercised, so the suite builds one.
2. **The prologue checksum is taken over the VOTED record, not the raw first copy**, so a vote
   that repaired a byte is confirmed rather than trusted. Checksumming the raw copy would reject
   an artifact the vote had already fixed — a failure that appears only on damaged input, so it
   would have shipped clean and failed exactly when the fault tolerance was needed.

Also: **`as Byte` truncates silently** (`300 as Byte` is 44). The type checker does demand the
cast, so the narrowing is visible at the site, and the writers keep an arithmetically redundant
`band 255` to state it. This hazard recurs as records widen.

**Byte identity alone would be vacuous in two ways**, so both are closed: `WireView::parse` must
accept what Keleusma emitted, the two readers must agree on a damaged artifact, and the emitted
record must not be all zeroes. Every one of the 48 single-bit positions across the three copies
is injected and required both to be outvoted and to be reported as needing a scrub.

## The next increment — step 6, slice 3

**The region directory**: emission and lookup, triplicated and voted, word-indexed offsets. The
vote machinery from slice 2 generalises to it, but the directory's stride depends on the voted
region count, so the two-stage bootstrap is the thing to get right.

**The buffer has to grow.** It is 4096 bytes, ample for the 48-byte prologue; the directory
reaches 49200 bytes at the format's 1024-region ceiling. Growing it raises the `limit` on
`crc_range` too, so the worst-case bound moves with it.

## Open, and held by the operator

- **Publication remains HELD.** Nothing is published. Irreversible and outward-facing.
- **Trimming the gate's feature matrix**, worth roughly 34 minutes, measured. Not done
  unilaterally because it weakens verification.
- **MSRV 1.85 declared, never verified.**
- **Fifteen `self.aux()` sites remain, audited, none hot.** A real follow-up, not a blocker.

## Parallel development

`v0.3.0` carries native code generation in a separate session and worktree. Its mailbox is
`git show origin/v0.3.0:docs/process/handoffs/v0.3.0.md`; this branch's is
[`handoffs/v0.2.3.md`](./handoffs/v0.2.3.md). Slice 1 touched no file on their read surface —
`src/wire_schema.rs` and `src/bytecode.rs` are unmodified — and added no gate step.
