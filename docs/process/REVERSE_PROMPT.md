# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-04 (session 37)

## Current state — the wire-format programme, steps 1 and 2 done

The work has moved from "replace rkyv" to designing a wire format against a stated requirement set,
with its own reusable crate. **Steps 1 and 2 are complete**: the prototype closed both
layout-sensitive gaps, and the `keleusma-wire` / `keleusma-wire-derive` crates exist, are tested, and
are covered by the gate. **No `keleusma` runtime code consumes them yet** — that is step 4.

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
- ~~The ECC plane is unexercised end to end.~~ **CLOSED**: the plane is implemented in
  `keleusma-wire`, and every single-bit fault across a protected payload (512 cases) is corrected and
  reported. The *prototype* artifacts in `secret/` still carry no ECC region, which now matters less
  since the Rust implementation exercises it.

## Step 2 landed: the `keleusma-wire` crate exists

Mechanism only, as resolved. It provides framing, a triplicated prologue and region directory,
fixed-stride record tables, byte pools, CRC-32, and the majority vote. It has **no dependency on the
Keleusma runtime** and hardcodes no schema — region kinds, record strides, and field offsets are all
the caller's. That is what makes it usable elsewhere, which was the stated point.

Written to be transliterable to Keleusma (step 6): no recursion, static loop bounds, no allocation on
the read path, fixed-size records, unrolled place-value field access, no traits or generics in the
codec core, state in explicit structs.

**Verification**: 12 unit tests, 11 integration tests, 1 doctest. Clippy clean at `-D warnings` both
with and without default features. Builds for `wasm32v1-none` with and without `alloc`, so the
`no_std` claim is tested rather than declared. Three tests carry most of the weight:

- **1536 single-bit fault injections** across the protected header, each required to be both
  corrected by the vote **and** reported by `needs_scrub()`.
- **Every truncation** of a valid artifact is rejected, and every single-bit corruption anywhere is
  required not to panic.
- **Aliasing asserted by address.** The read path must return slices *into* the caller's buffer. A
  test that only checked values would not notice an owned decode creeping in, which is precisely how
  P10 would be lost silently — so the pointer range is asserted directly.

### Two findings from writing it

1. **The prologue had to be split from the directory** — a bootstrapping problem invisible until a
   real reader existed. Voting the header needs the block stride, which needs `region_count`, which
   would itself be inside the block being voted; a bit flip there would desynchronise the search for
   the copies meant to repair it. A fixed-size prologue at fixed offsets is votable with no prior
   knowledge. **This also withdraws the "block check must be a trailer" correction from earlier
   today**: once the directory is out of the block, the check covers only fixed-size fields known
   before the first write, so no back-patching arises. The split subsumes the trailer.
2. **A totality hole in my own bounds checks.** They were written `at + n <= len`, which overflows
   for `at` near `usize::MAX` and panics in a debug build — in the functions whose entire contract is
   totality. Found by testing the extreme offset rather than by review. Now a subtraction on the
   length, which cannot overflow.

### Encoder strategy: RESOLVED

**Operator chose option (a)** (one buffer per region, leading directory), 2026-08-04, which is what
the crate implements. Option (b) stays reachable without touching any record layout, should
single-pass emission ever be required.

## The ECC plane and the derive have landed too

**The parity plane is in**, which is what makes the crate differentiated rather than "another
container". (72,64) SECDED, one check byte per 64-bit word, held in a region parallel to the data.
`builder.protect(id, kind)` generates it; `view.verify_region(&r)` scans it. Correction returns a
**value** and never writes to the caller's buffer — an in-place corrector would have needed `&mut`
and the allocation-free read path would have died to deliver the fault tolerance.

**`#[derive(WireRecord)]` is in**, in a separate `keleusma-wire-derive` crate behind an off-by-default
`derive` feature. It generates offset constants, a stride, and a total codec, removing the
hand-rolled-offsets adoption barrier. Fields pack with **no implicit padding** (`{u8, i16, i64,
[u8;5]}` → 0/1/3/11), which `repr(C)` would not produce, so the offsets must be generated rather than
taken from the type.

**A gate hole was found and closed.** `release-gate.sh` runs `cargo test --workspace` at DEFAULT
features and documents five crates BY NAME, so the `derive` feature would never have been tested and
neither new crate's docs would ever have been built under `-D warnings` — the same shape of hole that
let the broken `src/selfhost/` intra-doc links survive four releases. Four steps added.

## Publication readiness: PREPARED, but hold until internal use

The crate is prepared — LICENSE, README with four compiled doctests, `#![forbid(unsafe_code)]`,
`#[non_exhaustive]` on the growable types, docs.rs metadata on both crates, and gate coverage.

**It should NOT be published yet, and the reason is concrete.** Nothing consumes it. Its only users
are its own tests, and the first real consumer always finds something: `Region` gained a `covers`
field the moment the second requirement (ECC) arrived, which post-1.0 would have been a breaking
change. Publishing now freezes an API that no workload has exercised.

Known gaps, none blocking internal use:
- **MSRV 1.85 is declared but never verified** — no build against that toolchain.
- **No fuzzing.** Totality is tested exhaustively for single-bit faults and truncation, which is
  strong but is not the same as a fuzzer against a parser of untrusted bytes.
- **No size or timing numbers.** The "addressing is a shift" and "no allocation" claims are
  structural; the second is verified by construction and by address, the first is not measured.

## Step 4 stage 1 landed: the flattened constant table

`src/wire_schema.rs` supplies the schema the container deliberately omits — region kinds, record
meanings, and the flattening of a `ConstValue` tree into fixed-size records. Five regions:
`STRING_POOL`, `NAMES`, `CONSTS`, `STRUCT_AUX`, `ENUM_AUX`. 16 tests, all passing.

**The design claim is now implemented, not just asserted.** A composite references a RANGE that lies
strictly after it, produced by breadth-first numbering with roots pinned to `0..n` (a chunk indexes
its constants positionally). That is what makes the table walkable by a single reverse linear sweep
with no stack. The decoder RE-VALIDATES the ordering up front rather than trusting the encoder that
produced its input, and a hand-corrupted backwards range is a test.

**Side tables rather than wider records.** A struct needs a type name, field names and values; an enum
a type name, variant, optional discriminant and payload. Widening every record to the worst case would
cost 32 bytes for an `Int` needing 8, so those two kinds reference small side tables and the constant
record stays two words.

**Field names are interned WITHOUT sharing**, unlike everything else, because a struct's names must
stay contiguous for `field_names_first + i` addressing; a repeated name returning an earlier index
would break the run. Two structs sharing field names is the test.

### The finding worth carrying: a test suite that was blind

`ConstValue`'s hand-written `PartialEq` **deliberately ignores the enum discriminant** (the `..` in
its `Enum` arm). So `assert_eq!` on a round trip cannot see whether the discriminant survived, and
every enum round-trip test was passing **vacuously** with respect to it — they would have passed with
the field dropped entirely. The tests now use a `deep_eq` helper that compares it explicitly, and the
`Some(0)` vs `None` distinction is asserted by destructuring rather than by `!=`. **Anyone testing
round trips of `ConstValue` must not use `==`.**

### Not done, and not claimed

- **`decode_constants` returns OWNED values.** This is the tooling and test path, the analogue of the
  existing `decode_aux`. The **borrowed in-place accessor the VM needs is not written**, and that is
  the surface where P10 is preserved or lost.
- **Nothing is wired into the loader.** The `rkyv` path is untouched; this is parallel infrastructure
  alongside `debug_meta` and `value_layout`.
- **The rest of the aux body** — struct templates, param types, enum layouts, signatures, native
  return shapes, the scalar header block — is not encoded yet. Those are flat vectors following the
  same mechanical pattern.

## Next step

Stage 2: the remaining aux-body regions, then the borrowed accessor. The accessor is the
consequential one — see the P10 note above.

## Superseded next step (kept for context)

Step 4 proper: the Keleusma schema on top of the container — region kinds, chunk descriptors, the
constant table, the string pool. The revision-2 prototype validated those record layouts across three
languages, but no Rust code emits them yet. That work is also what would qualify the container for
publication, since it is the first real consumer.

## Standing method notes

The fourteen rules are consolidated in [HANDOFF.md](./HANDOFF.md). Two that earned their keep most
recently:

- **Cross-check across independent implementations.** A Keleusma/Python checksum disagreement (3968
  against 4016) localised a mistranscribed magic constant in one step. Build the cross-check before
  it is needed.
- **Run the FULL gate before landing.** Clippy `-D warnings` and `EXPECTED_SELF_COMPILE` fire only
  there, and the documented `cargo doc --workspace` command once disagreed with the gate's own doc
  step, hiding a real defect in published documentation.
