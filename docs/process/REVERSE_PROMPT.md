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

- `kel-format-probe/wireimage.kel` — Keleusma **producer and consumer**, 408-byte artifact, 12/12.
- `kel-format-probe/image.py` — independent reference emitter. **Checksums agree at 5093.** It also
  generates both hardware image packages, so the clean and corrupt images cannot drift — they had,
  before this revision.
- `kel-format-probe/stream.kel` — Keleusma **streaming stage**, emitting across yields. 9/9.
- `silicon-prototype/wire_decode.vhd` + `tb_wire.vhd` — hardware **consumer** of those exact bytes.
  **PASS** on the header vote, block trailer, all three regions, absent-region not-found, every chunk
  descriptor, every constant record, a string constant resolved to real pool bytes, and the
  reverse-sweep aggregate.
- `silicon-prototype/tb_wire_corrupt.vhd` — one corrupted header copy outvoted **and** flagged, and it
  now asserts the damaged copy actually differs from the voted value so it cannot pass vacuously. PASS.
- `silicon-prototype/secded_*` — (72,64) SECDED validated in Python and simulated in VHDL:
  432/432 single-bit corrected, 15336/15336 double-bit detected.
- Toolchain: `nvc` 1.23-devel at `/usr/local/bin/nvc`. Build notes in
  `secret/silicon-prototype/README.md` — MacPorts needs `--enable-static-llvm`, and `make` will not
  relink an existing `bin/nvc` after reconfiguring.

## Both layout-sensitive gaps are now CLOSED (revision 2, 2026-08-04)

The fetch path runs past the chunk descriptor into the constant table and out into the string pool,
and emission is tested from a yielding stage. Results, each expected value taken from an independent
implementation rather than from the code under test:

| Implementation | Result |
|---|---|
| Keleusma producer + consumer (`wireimage.kel`) | 12/12 |
| Reference emitter (`image.py`) | byte-identical, checksum 5093 |
| Hardware decoder, simulated (`wire_decode.vhd`) | 24 checks |
| Keleusma streaming stage (`stream.kel`) | 9/9 across suspensions |

Both hardware testbenches were checked against a **negative control** (mutate an expected value,
confirm the failure fires), since a testbench that passes first try has not been shown able to fail.

**It found five things, which is why the design document required it before freezing.**

1. **The directory entry was 12 bytes** — one and a half words, contradicting the format's own rule
   that every record is an integral number of words. Now 16.
2. **The block check cannot be a header field.** Its input is the directory written after it, so a
   leading position requires back-patching. Moved to a trailer.
3. **The composite-range ordering invariant is load-bearing and must be CHECKED.** A composite's range
   must lie strictly after the composite; that is what makes a bottom-up walk a single reverse linear
   sweep with no stack. Violating it yields a **wrong answer rather than a fault**, so it replaces
   `MAX_CONST_DEPTH` as the hostile-input check rather than simply removing it.
4. **A leading directory and globally contiguous regions are both incompatible with streaming
   emission.** This forces an encoder choice — buffer per region (keeps the leading directory), or a
   trailing directory with per-unit segments (true single pass). Option (b) was implemented and works;
   **the recommendation is (a)**. Now an explicit open question in the design document.
5. **Language finding**: a resumed `yield` block continues from the suspension point with its
   parameter still bound to the original argument, so an `if tick == n` ladder runs once and falls
   through. The first streaming probe did exactly that and emitted one segment instead of three; the
   byte count caught it. Streaming stages want straight-line yields.

## Open questions for the operator

- **Encoder strategy (item 4 above).** Blocks nothing today — the record layouts are identical either
  way — but it decides whether the directory leads or trails, so it wants settling before the crate.
- **The ECC plane is still unexercised end to end.** The codec is validated in isolation; no prototype
  artifact carries an ECC region. Additive through the directory, so deferrable without a format change.

## Next step

Step 2, the mechanism-only `keleusma-wire` crate. The chunk-descriptor and constant-record layouts are
candidates for freezing; the directory entry and block trailer changed in this revision and should be
treated as settled only as of it.

## Standing method notes

The fourteen rules are consolidated in [HANDOFF.md](./HANDOFF.md). Two that earned their keep most
recently:

- **Cross-check across independent implementations.** A Keleusma/Python checksum disagreement (3968
  against 4016) localised a mistranscribed magic constant in one step. Build the cross-check before
  it is needed.
- **Run the FULL gate before landing.** Clippy `-D warnings` and `EXPECTED_SELF_COMPILE` fire only
  there, and the documented `cargo doc --workspace` command once disagreed with the gate's own doc
  step, hiding a real defect in published documentation.
