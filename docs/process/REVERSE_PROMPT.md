# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-04 (session 36)

## Current state — the wire-format programme, at the design/prototype stage

The work has moved from "replace rkyv" to designing a wire format against a stated requirement set,
with its own reusable crate. **No product code has changed for this yet**; the branch carries design
documents only.

**Before writing anything tracked — documentation, commit message, or code comment — read
`secret/notes/APPENDIX_B.md`.** It defines what must not appear in this repository. Tracked material
was sanitized against it on 2026-08-04. This is a hard constraint.

## The programme (operator-stated, six steps, in order)

1. Prototype the wire format until it can be locked in.
2. Add a new wire-format crate, usable by other projects as an alternative to `rkyv`, as
   `keleusma-arena` is nominally useful outside this repository.
3. Document the WHAT of the format, without the internal reasoning about the WHY.
4. Implement the wire format in Rust.
5. Port Keleusma to it.
6. Self-host the wire format in Keleusma — which implies **the Rust must be Keleusma-like**.

Operator resolutions, 2026-08-04:
- **Crate is MECHANISM ONLY**, named `keleusma-wire`. It must not depend on the Keleusma runtime nor
  hardcode `WireChunk` / `ConstValue`; Keleusma's schema layers on top in the `keleusma` crate.
- **Step 6 covers BOTH encoder and decoder.**
- **Lock-in is a judgement call.** A proof of concept need only be good enough to decide and move on;
  do not gold-plate it.

## Design state

The current design is [`../decisions/WIRE_FORMAT_V2_WORD_ORIENTED.md`](../decisions/WIRE_FORMAT_V2_WORD_ORIENTED.md):
word-oriented with a 64-bit unit, word-indexed offsets, fixed-size records with variable data in
byte-addressed pools, a (72,64) SECDED plane held parallel to the data, per-region encryption, and a
triplicated header and region directory.

It **supersedes** [`../decisions/WIRE_FORMAT_V2_FLAT_AUX.md`](../decisions/WIRE_FORMAT_V2_FLAT_AUX.md)
on record structure. That document's **P10 analysis still governs and is not repeated**: string
constants materialise as `KStr` aliasing the bytecode image, so the accessor layer must be a
**borrowed view, never an owned decode**. Routing the runtime through an owned decode would allocate
per load and silently undo P10 with no test failing.

`src/wire_aux.rs` on `v0.2.3` implements the **superseded** variable-length design. Its primitive
layer, explicit tag discipline, and totality tests are reusable; **its record structure is not.**

## Prototype state (all in `secret/`, gitignored, reproducible)

- `kel-format-probe/wireimage.kel` — Keleusma **producer** emitting a 160-byte artifact.
- `kel-format-probe/image.py` — independent reference emitter. **Checksums agree at 4016.**
- `silicon-prototype/wire_decode.vhd` + `tb_wire.vhd` — VHDL **consumer** of those exact bytes.
  **PASS** on magic, region count, both regions, absent-region not-found, all chunk descriptors.
- `silicon-prototype/tb_wire_corrupt.vhd` — one corrupted header copy outvoted **and** flagged. PASS.
- `silicon-prototype/secded_*` — (72,64) SECDED validated in Python and simulated in VHDL:
  432/432 single-bit corrected, 15336/15336 double-bit detected.
- Toolchain: `nvc` 1.23-devel at `/usr/local/bin/nvc`. Build notes in
  `secret/silicon-prototype/README.md` — MacPorts needs `--enable-static-llvm`, and `make` will not
  relink an existing `bin/nvc` after reconfiguring.

## Two gaps that could still move the record layouts

Not lock-in gates (lock-in is judgement), but each is cheaper to find now than after step 4:

1. **The fetch path stops at the chunk descriptor.** It does not follow `const_first`/`const_count`
   into a constant table, nor resolve a string slice out of the pool.
2. **Emission is only tested from a terminating `fn`.** A real stage is `loop main` yielding
   incrementally, which is where forward-only emission either pays off or does not.

## Next step

Continue step 1, then step 2. The immediate technical question is whether constant and string-slice
resolution survives the fixed-size-record layout unchanged.

## Standing method notes

The fourteen rules are consolidated in [HANDOFF.md](./HANDOFF.md). Two that earned their keep most
recently:

- **Cross-check across independent implementations.** A Keleusma/Python checksum disagreement (3968
  against 4016) localised a mistranscribed magic constant in one step. Build the cross-check before
  it is needed.
- **Run the FULL gate before landing.** Clippy `-D warnings` and `EXPECTED_SELF_COMPILE` fire only
  there, and the documented `cargo doc --workspace` command once disagreed with the gate's own doc
  step, hiding a real defect in published documentation.
