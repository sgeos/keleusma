# Wire Format Version 2 — Word-Oriented, Correcting, Silicon-Friendly

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Design for the version-2 wire format. **Supersedes the flat-aux design in
[`WIRE_FORMAT_V2_FLAT_AUX.md`](./WIRE_FORMAT_V2_FLAT_AUX.md)**, which remains accurate on the rkyv
displacement and the P10 analysis but is **wrong on record structure**.

Status: **DESIGN, superseding.** Operator requirements stated 2026-08-04.

## Requirements

The format must be:

1. **Integrity-preserving under bit-level corruption** — correction, not merely detection.
2. **Optionally encryptable** — per-artifact, without forcing the cost on every artifact.
3. **Self-hostable** — emittable by a Keleusma stage that appends bytes, with no library.
4. **Durable without external reference** — interpretable from the artifact alone, over long
   retention, with no registry or authority available to consult.
5. **A good fit for direct usage in silicon** — decodable by hardware, not only by a CPU.

Plus the constraints already binding: deterministic encoding (the byte-identity oracle), in-place
reads (P10 — see the prior document), `no_std + alloc`, and statically bounded WCET and WCMU.

Additional requirements context is held outside this repository; see Appendix B.

## What the research changed

Supporting findings are held outside this repository; see Appendix B. The load-bearing result:

**Requirements 1, 3 and 5 all point at the same construct — fixed-size records — and all three
condemn the length-prefixed variable-length records the superseded design used.**

| Requirement | Why variable-length hurts |
|---|---|
| Silicon | A variable length makes the *next* field's position data-dependent. Hardware parsers then need dynamic multiplexer trees or shift registers; P4-16 has no first-class TLV support at all. Fixed offsets map to pipeline stages with compile-time positions. |
| Corruption tolerance | A bit flip inside a length prefix does not corrupt one field — it destroys the framing of everything after it. Fixed-size records confine damage to one record and never lose synchronisation. |
| Self-hostable | Fixed strides are `base + (i << log2(width))`. No back-patching, no two-pass writes. |

And **(72,64) SECDED** — 64 data bits, 8 parity bits, single-error-correct double-error-detect — is the
standard word-level correction primitive, implemented as a small combinational syndrome-and-correct
circuit. It is what ECC DIMMs use. A silicon implementation of this format gets correction essentially
for free because the corrector is a circuit it likely already has.

That convergence is the design.

## The design

### Principle: everything is a word

The unit is the **64-bit little-endian word**. Every region is an integral number of words; every
record is a fixed integral number of words; every offset is a **word index**, not a byte offset.

**[Rationale]** Word indices multiply by a power of two, so hardware address generation is a shift.
Byte offsets would force a multiply or a non-uniform stride. Word granularity is also the natural
granularity for SECDED.

Little-endian is fixed by specification **and** declared in-band (below), so a future reader need not
possess this document.

### Region directory, triplicated

```
aux := header_block ×3, region*
header_block := magic u32, bom u16, version u16,
                region_count u16, reserved u16, header_crc u32,
                region_dir[region_count]
region_dir entry := kind u16, flags u16, word_offset u32, word_length u32
```

The header block and its directory are written **three times** and read by majority vote per word.

**[Rationale]** The directory is the only structure whose loss is total — without it no region is
reachable. It is on the order of a hundred bytes. Triplicating the single catastrophic-failure point
is the cheapest reliability purchase available, and majority-vote-of-three is trivial in hardware
(three-input bitwise majority is one gate per bit).

`bom` is a fixed byte-order marker. A reader that sees it byte-swapped knows the artifact's endianness
without external documentation — a durability requirement, two bytes.

`flags` per region carries `ENCRYPTED`, `ECC_PRESENT`, and `OPTIONAL`. Reserved now precisely because
retrofitting a flag word later is expensive.

### Records are fixed-size; variable data lives in pools

No record contains a length-prefixed payload. Variable-length data is referenced by
`(word_offset, length)` into a **pool region**, and the pools are byte-addressed leaf regions that
contain no framing to corrupt.

- **String pool** — a flat byte region. A string is `(byte_offset, byte_length)`. Contiguous, so a
  string is a **direct subslice**, which is what keeps `KStr` aliasing the image (P10).
- **Chunk table** — fixed-size chunk descriptors, one per chunk, each a fixed word count carrying the
  name reference, constant range, template range, counts, block type, and op stream location.
- **Constant table** — fixed-size constant records: a tag word plus a payload word. Composite
  constants (tuple, array, struct, enum) hold a **range reference** into the same table rather than
  nesting inline.

**[Rationale, and a second win]** Making composite constants reference a range instead of nesting
**removes the recursion**. The superseded design needed a recursive decoder with a depth cap to guard
against stack exhaustion on hostile input; a range-referencing table is walked iteratively with an
explicit stack. That also removes the R4 problem flagged for the eventual Keleusma emitter — it no
longer needs a recursion workaround, because there is no recursion.

### ECC as a parallel plane, not interleaved

An `ECC` region holds one parity byte per data word of the region it covers, identified in its own
header.

**[Rationale]** Interleaving parity with data — 9 bytes per 8-byte word — would break contiguity and
destroy in-place string aliasing, undoing P10. A parallel plane keeps data contiguous and in-place
readable, lets hardware fetch data and syndrome concurrently, and is purely additive: an artifact
without the region is simply uncorrected, and readers that ignore the region still work.

**[Consequence]** Correction is *optional per artifact and per region*. A flash-constrained target can
omit it; a long-retention artifact carries it. Same format either way.

### Encryption is per region

Encryption applies to a region, flagged in its directory entry, never to the whole body.

**[Rationale]** An encrypted region cannot be read in place. Whole-body encryption would force a
decrypt-and-copy on every load — an allocation per load and a WCMU regression, silently undoing P10.
Per-region encryption keeps the header, directory, and chunk table in the clear so loading and
verification stay in place, and pays the copy only for regions that actually carry secrets.

**[Consequence for the oracle]** A deterministic nonce would be required for byte-identical comparison,
and a deterministic nonce is a cryptographic hazard under key reuse. Therefore **the differential
oracle compares plaintext artifacts**; encryption is applied outside the compared boundary. This must
be stated in the test harness, not assumed.

### Self-description

A `SCHEMA_DESCRIPTOR` region carries a compact in-band description of the region kinds and record
layouts: for each record type, its word width and its field positions and widths.

**[Rationale]** The format is otherwise schema-external — meaningless without this source tree. Over
long retention the source tree is the thing least likely to survive. A few hundred bytes converts
the artifact from "needs its specification" to "carries its specification", without touching the fast
path: nothing reads the descriptor at load time.

**[Uncertainty]** This is the least-proven element of the design. It is cheap and additive, so the
recommendation is to reserve the region kind now and specify the descriptor's own layout carefully —
it is the one structure that must be interpretable without a schema, so it should be the simplest and
most redundantly encoded thing in the artifact.

## The Keleusma-expressibility test (validated 2026-08-04)

**Criterion, operator-stated:** a good wire format should have a producer/consumer pair that can be
expressed *gracefully in Keleusma*. This is not merely a convenience requirement. Keleusma's
constraints — totality, no recursion (R4), bounded `for … limit` loops, statically bounded memory —
are very nearly the constraints a hardware decoder and a corruption-tolerant format also live under.
So "graceful in Keleusma" and "good for silicon and durable artifacts" may be the same property seen
from two directions, which makes the test cheap evidence about the design rather than a style preference.

**It was tested, not assumed.** `secret/kel-format-probe/wirefmt.kel` implements an encoder and
decoder for the region directory and a fixed-size chunk-descriptor table, compiles under the real
compiler, and round-trips every field (3/3 directory entries, 3/3 chunk descriptors).

### What came out graceful

- **Fixed-size records.** Element *i* is `base + i * STRIDE` — a single expression, no state, no
  scan. The same arithmetic that is a shift in hardware.
- **Forward-only emission.** One `cursor` in `private data`, append only, **no back-patching** — but
  only because regions can be emitted in dependency order (pools before the tables referencing them).
  That ordering is now a design rule, not an accident.
- **No stack anywhere.** The codec needs no explicit stack, because the format has no nesting. This is
  the property doing the real work, and it is a direct consequence of composite constants referencing
  a range rather than nesting inline.

### The finding that changed the design

The first version of the probe assembled multi-byte fields with a loop over computed place values
(`pow256(i)`), which required two extra `private data` accumulator fields because **Keleusma has no
`let mut`** — mutable state must live in a data block.

Rewriting the primitives **unrolled**, with literal place values (`1, 256, 65536, 16777216`), removed
both accumulators and every loop: the state block went from three fields to one, and the readers
became pure expressions. Same result (round-trip still exact).

**That is the convergence, concretely.** The more graceful Keleusma form is also the more
hardware-like form — in silicon each byte of a word is a fixed slice, which is wiring rather than
arithmetic — and it is also the lower-WCMU form, since it holds less state. Three requirements
agreeing on one implementation detail is the strongest evidence so far that the word-oriented
direction is right.

### Design rules this establishes

1. **Every field width is fixed and small enough to unroll** (≤ 8 bytes). No loops in the primitives.
2. **Regions are emitted in dependency order**, so encoding is pure forward append with no
   back-patching.
3. **The format must be walkable without a stack.** This is the crisp form of the whole criterion: if
   a codec needs an explicit stack or a pile of accumulators, the format has nesting or
   data-dependent structure it should not have.

**Rule 3 is also the test to re-apply whenever the format changes**: write the producer and consumer
in Keleusma first. If they strain, the format is wrong, and it will be wrong in silicon and under
corruption for the same reason.

## What this costs

Honest accounting, because fixed-size records are not free:

- **Size.** Fixed-size records waste space on short values. **[Estimate, unverified]** the constant
  table is the worst case, at two words per constant regardless of payload. Mitigated by pooling
  strings rather than inlining them.
- **A rewrite.** `src/wire_aux.rs` as committed encodes variable-length records throughout. The
  primitive readers and writers, the leaf-enum tag assignments, the bounds-checking discipline, and
  the whole test approach survive; the record structure does not.
- **Losing the recursion guard.** Not a cost — the range-reference design removes the recursion that
  made the guard necessary. `MAX_CONST_DEPTH` becomes unnecessary rather than merely satisfied.

## Staging

1. **Restructure the codec** to fixed-size word records with pools. Reuse the primitive layer and the
   totality tests; replace the record layer.
2. **Accessor layer** as a borrowed view over `&'a [u8]` — unchanged in intent from the superseded
   plan, and still the stage where P10 is preserved or lost.
3. **Cut over the VM and loader**, delete rkyv, bump `BYTECODE_VERSION` to 2, update the
   no-public-adoption policy text in `CLAUDE.md`.
4. **Additive hardening**: triplicated header, ECC region, schema descriptor. Each independently
   landable because the directory makes them additive.
5. **Self-host the emitter.**

## Open questions for the operator

1. ~~**Word size.**~~ **RESOLVED 2026-08-04: fixed 64-bit universally**, independent of the 8/16/32/64
   target word. Artifacts stay portable across targets, which cross-target reuse requires, and (72,64)
   SECDED is defined at 64.
2. ~~**Is silicon decode a near-term target?**~~ **RESOLVED 2026-08-04: yes, near-term, not started.**
   Preliminary VHDL prototyping lives in `secret/silicon-prototype/`. **Consequence: the record layouts
   must be reviewed against a concrete fetch pipeline BEFORE they are frozen.** The prototype so far
   covers the SECDED codec, the TMR vote, and directory addressing; it does NOT cover the end-to-end
   path (walk directory, fetch chunk descriptor, resolve a constant), which is exactly the part that
   would expose a bad record layout. Do not freeze the constant-table or chunk-descriptor layout until
   that path is sketched.
3. **Cross-generational authenticity** — reasoned below rather than left open.

## Authenticity provisions

The format reserves, additively through the region directory:

- A **signature region** carrying an explicit algorithm identifier and room for several signatures, so
  a k-of-n policy across independent primitives is expressible. Algorithm diversity is the hedge
  against any single primitive being broken later.
- A **provenance region** carrying a hash chain, so an artifact's consistency with a known root can be
  checked locally, without contacting an external authority.
- A reserved **`AUTH_TIER`** field, so an artifact declares the assurance level it requires and a
  consumer can refuse an under-authenticated change rather than having to infer intent.

The signed extent is explicit and canonical. That was already required — determinism is what the
byte-identity oracle needs, and signing needs the same property.

**Recommendation: reserve the regions and the tier field now; implement only the hash-based signature
tier.** Reserving costs nothing and is awkward to retrofit. Rationale for the tiering, and for what
the reserved tiers are for, is in Appendix B.

**Residual, stated plainly:** none of this defends against an adversary with physical access to the
artifact or its host. That is outside what any encoding can address.
