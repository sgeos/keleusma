# Self-Hosting Wire-Format Serialization — Scoping

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Scoping for the Order-1 residual "self-host wire-format serialization". Probed 2026-08-03,
**re-scoped 2026-08-08**.

Status: **STEP 6 COMPLETE (2026-08-09). All seven slices are done.** The wire format is
expressible in Keleusma end to end: CRC-32, the container primitives and prologue vote, the region
directory, record tables and byte pools, the twenty region kinds and seventeen record shapes, the
opcode stream and operand pool, and the framing header with its CRC trailer.

`src/selfhost/kel/wire.kel` is the implementation and `tests/selfhost_wire.rs` is the differential,
80 tests. **What remains before the artifact is produced by the self-hosted path is wiring, not
invention**: `wire.kel` is not yet driven by the pipeline, and it is deliberately absent from
`read_stage`.

## 2026-08-08: the blocker is gone. Read this before the 2026-08-03 text.

The analysis below concluded that full self-hosting was out of reach because the auxiliary body was
an rkyv archive, that reproducing rkyv's byte layout in Keleusma was disproportionate and fragile,
and that proceeding needed an operator decision to change the wire format so the auxiliary body used
an encoding the self-hosted compiler could produce.

**That decision was taken and executed.** The auxiliary body is now the wire format v2 container,
specified in [`../spec/WIRE_FORMAT.md`](../spec/WIRE_FORMAT.md) and implemented by the standalone
`keleusma-wire` crate. It was designed for this purpose: no recursion, statically bounded loops, no
allocation on the read path, fixed-size records, unrolled place-value field access, no traits or
generics in the codec core, and state in explicit structs. `BYTECODE_VERSION` moved to 2 with
operator authorization on 2026-08-06.

So the recommendation below to defer this item and prefer the monomorphizer and type checker is
**withdrawn**. The row reading "Auxiliary body — `rkyv::to_bytes` — NO" is obsolete; every region is
now self-hostable in principle.

### Re-slicing against the v2 container

Smallest first, each independently verifiable against the Rust implementation by byte identity.
Slice 1 is unchanged from the original plan and is still the right place to start, because it
establishes the byte-emission harness the rest depends on.

1. ~~**CRC-32.**~~ **DONE 2026-08-09.** `src/selfhost/kel/wire.kel` plus the differential in
   `tests/selfhost_wire.rs`. See [Slice 1 as built](#slice-1-as-built-2026-08-09) below.
2. ~~**Container primitives and the prologue.**~~ **DONE 2026-08-09.** See
   [Slice 2 as built](#slice-2-as-built-2026-08-09) below.
3. ~~**The region directory.**~~ **DONE 2026-08-09.** Emission, lookup, and the triplicated vote, with the prologue-to-directory bootstrap tested by damaging the region count in each prologue copy in turn.
4. ~~**Record tables and byte pools.**~~ **DONE 2026-08-09.** Fixed-stride addressing and pool
   access, located from the voted directory. `divides` uses real division rather than a mask,
   because a stride is only required to be a word multiple, not a power of two; and the
   zero-stride guard depends on `andalso` short-circuiting, since division by zero traps.
5. ~~**The schema layer.**~~ **DONE 2026-08-09**, as 5a to 5e. Twenty region kinds and seventeen
   record shapes, every transcribed offset pinned against the derive's generated constant.
6. ~~**The opcode stream and operand pool.**~~ **DONE 2026-08-09**, as 6a and 6b. Verified against
   `encode_op` and the four `OperandPoolEntry` constructors by byte identity.
7. ~~**The framing header.**~~ **DONE 2026-08-09.** Sixty-four fixed bytes plus the CRC trailer,
   which is validated by a residue rather than by recomputation.

### Constraints to carry into the implementation

- **Both directions are in scope.** The operator resolved on 2026-08-04 that self-hosting the wire
  format covers the encoder **and** the decoder.
- **`ConstTable::value` is NOT transliterable as written.** Added 2026-08-08, it uses `BTreeSet` and
  `BTreeMap` to walk one constant's reachable set. That is correct for the Rust VM and unavailable
  in Keleusma. The Keleusma decoder needs a bounded array-based walk instead. The forward-ordering
  invariant is what makes such a walk terminating, so the shape exists; it simply has to be written
  differently rather than transliterated.
- **Only a composite constant record carries a child range.** A scalar overlays its payload on those
  bytes. Getting this backwards reads an integer constant's value as a list of child indices, which
  has already happened once in the Rust implementation.
- **First thing to probe**, not yet established: how a `.kel` stage addresses a byte buffer for
  emission and for reading. The `secret/` prototype used a data segment. Settle this before slice 1,
  since every later slice inherits it.

### Slice 1 as built (2026-08-09)

`src/selfhost/kel/wire.kel` holds the Keleusma implementation; `tests/selfhost_wire.rs` holds the
differential. Eleven tests, 0.67 s. The file is **not** in `read_stage`'s table and the driver does
not run it, because it does not yet emit an artifact.

**The oracle is a published constant, not our own code.** `crc32("123456789") == 0xCBF43926` is the
standard CRC-32/ISO-HDLC check value, and both Rust implementations are already independently pinned
to it (`keleusma-wire/src/crc.rs`, `src/vm.rs:11696`). The test compares against
`keleusma_wire::crc32` rather than the `crate::bytecode::crc32` this plan named, only because the
latter is `pub(crate)` and unreachable from an integration test. Same algorithm, same polynomial.

**What the probe settled**, each executed against the reference rather than reasoned:

| Question | Answer |
|---|---|
| Are locals immutable? | Yes, rejected at **parse**: "assignment is only supported for data block fields" |
| Does a runtime-range `for` need `limit`? | Yes, rejected at **verify**, not at parse |
| Is `lsr` a logical shift over the full word? | Yes: `(0 - 1) lsr 1` is `2^63 - 1` |
| Does `Byte as Word` sign-extend? | No, it zero-extends: `0xFF` reads as 255 |
| Bounded `for` inside a `fn`? | Yes, including nested, and across a call boundary |
| Call in statement position? | Yes |

**No masking is required, and this corrects a recorded design note.** The handoff expected the
accumulator to need `band 0xFFFFFFFF` after each step. It does not. The accumulator is always in
`[0, 2^32)`: it starts at `2^32 - 1`, a folded byte xors in under 256, a logical shift right leaves
it under `2^31`, and the polynomial is under `2^32`. The invariant holds without help, so a mask
would be dead work.

**`require word >= 64`, not the `>= 32` every stage declares.** Copying the stages' directive by
analogy would have been a silent defect. A 32-bit signed `Word` cannot hold either the initial value
or the polynomial, and — verified against the reference — **a source carrying those literals compiles
for a 32-bit target without complaint when no `require` is present.** Nothing else catches it.

**One inherent blind spot, enumerated rather than estimated.** A polynomial mutation is undetectable
on the empty buffer and on the single byte `0xFF`, and on nothing else. `0xFFFFFFFF xor 0xFF` is
`0xFFFFFF00`, whose low eight bits are clear, so all eight iterations take the else branch and the
polynomial is never consulted; exhausting all 256 single-byte inputs confirms `0xFF` is unique. The
test asserts the blind set **exactly**, so a case that joins it fails loudly.

**A consequence worth carrying forward:** the range invariant makes `asr` and `lsr` compute the same
values here, so swapping them is *not* caught by the differential. That equivalence is pinned by its
own test so it reads as understood rather than as an untested assumption.

**Both control directions are encoded**, not run by hand: three independent must-fire mutations
(polynomial, initial value, inner iteration count), a must-not-fire pass over a corpus whose coverage
is itself asserted, a `mutate` helper that requires its anchor to occur exactly once so a stale
anchor cannot silently test the unmutated source, and hostile-input cases for a length beyond the
array capacity (traps, does not truncate) and a length shorter than the buffer.

### Slice 2 as built (2026-08-09)

Place-value writers and readers, prologue emission, and the majority-of-three vote, all in
`wire.kel`. The suite is now 23 tests, 0.97 s.

**The oracle is byte identity**, not a single value: the 48 bytes Keleusma emits must equal the
first 48 bytes `keleusma-wire` emits for the same region count, checked at 0, 1, 2, 7, 255, 256,
1023 and 1024 regions. Two complementary directions are also asserted, because byte identity alone
would pass if both sides were wrong in the same way. `WireView::parse` must **accept** what Keleusma
emitted, and on a damaged artifact the Keleusma reader and the reference reader must **agree**.

**What the probe settled:**

| Question | Answer |
|---|---|
| Can a stage write into a shared byte array? | Yes, at literal and runtime indices, and from a `fn` |
| Does a `[Byte]` element accept a `Word`? | No — the type checker demands an explicit `as Byte` |
| Does `as Byte` fault on an out-of-range value? | **No, it truncates silently**: `300 as Byte` is 44 |
| Does a place-value writer round-trip? | Yes, and it reproduces `MAGIC` as the bytes `X U A K` |

The silent truncation is why the writers keep an explicit `band 255` that is arithmetically
redundant with the cast. It states the narrowing where a reader can see it, rather than leaving it
to an implicit conversion.

**Two details of the reference that a transliteration would get wrong by default.**

1. **`maj3` is a per-BIT majority**, `(a & b) | (a & c) | (b & c)`, not "pick the value that appears
   at least twice". Where all three copies differ it synthesises a byte no copy contains, which is
   the stronger behaviour: independent single-bit faults in three different copies are all repaired.
   The distinction is invisible unless a case with three distinct bytes is exercised, so the suite
   constructs one rather than hoping for it.
2. **The prologue checksum is taken over the VOTED record, not the raw first copy.** So a vote that
   repaired a byte is confirmed rather than merely trusted. Checksumming the raw copy would reject
   an artifact the vote had already fixed. `wire.kel` keeps `crc_voted` separate from `crc_range`
   for exactly this reason, and the 48-case single-bit injection test is what holds it in place.

**Coverage.** Every one of the 48 single-bit positions across the three copies is injected and
required both to be outvoted and to be reported as needing a scrub. Each malformed-field rejection
carries its own code, and the order of the checks follows the reference exactly, because the code a
caller sees for a doubly-malformed artifact is observable behaviour. The region-ceiling case
recomputes a valid checksum over the oversized count, or it would be rejected at the checksum check
first and would be testing the wrong thing.

**A structural change to `wire.kel`.** `main` now dispatches on its argument, since the host drives
the module through a one-argument entry point and there are four operations. Command 0 is slice 1's
checksum, and a test re-pins its behaviour rather than assuming it survived the refactor. An
unrecognised command returns a distinct code instead of a plausible value. New shared scalars are
appended **after** the byte array so `bytes[i]` stays at slot `1 + i`; prepending one would silently
shift every seeding site.

**Still at a 4096-byte buffer.** Ample for the 48-byte prologue. The directory reaches 49200 bytes
at the 1024-region ceiling, so slice 3 has to grow it.

### Slice 5, decomposed

Designed 2026-08-09, **not started**. Slice 5 is not one increment: `src/wire_schema.rs` defines
**twenty region kinds** and **seventeen record types**, and doing them in one pass would be a large
unreviewable change with a single all-or-nothing oracle.

**The offsets must be pinned, not transliterated.** `#[derive(WireRecord)]` generates each field's
offset by packing with **no implicit padding**, then rounds the stride up to a word multiple
(`STRIDE = PACKED_BYTES.next_multiple_of(8)`). That is deliberately not what `repr(C)` produces, so
the offsets cannot be derived by eye from the field types and must not be guessed.

Two consequences shape the design:

- **Keleusma hardcodes the offsets**, because it has no derive and no reflection.
- **A test asserts each hardcoded number equals the generated constant.** Rust can read
  `OFF_<field>` and `STRIDE` at compile time, so the comparison is exact and free. This is the
  slice's real oracle: it catches a mistranscribed offset immediately, and it catches DRIFT later
  if a record gains a field — which the byte-identity corpus would only catch if some test
  happened to exercise that record.

**Order, smallest first and by dependency**, mirroring how the Rust side was built:

| Sub-slice | Records | Why here |
|---|---|---|
| 5a | `NameRef`, `ShapeRecord` | Two words or fewer, no side tables, nothing depends on them |
| 5b | `ConstRecord`, `StructAux`, `EnumAux` | The constant table and its two side tables |
| 5c | `SignatureRecord`, `StructTemplateRecord`, `EnumVariantRecord`, `EnumLayoutRecord` | Range-addressed runs; reuse 5a's name machinery |
| 5d | `DataSlotRecord`, `SharedSlotRecord`, `PrivateCompositeRecord`, `DataInitRecord` | The data segment; absence versus emptiness is semantic here |
| 5e | `ChunkRecord`, `NativeRecord`, `NativeReturnRecord`, `HeaderRecord` | The module level, which references everything above |

**Two traps already paid for on the Rust side**, recorded so they are not rediscovered:

- **`DATA_SLOTS` presence is semantic.** An absent region means `None`; an empty one means `Some`
  with no slots. A module with no `data` block and one whose data block is empty are different
  programs, and collapsing them is a silent wrong answer.
- **`NATIVES` and `NATIVE_RETURNS` are separate regions on purpose.** They were first paired in one
  record on the reasoning that parallel vectors fall out of step — but they are already allowed to
  differ in length, and pairing silently DROPPED the surplus instead of preventing it. Independent
  regions carry both lengths.

### Slices 5 to 7 as built (2026-08-09)

**The offsets are transcribed and pinned, which is the design that made 5a to 5e safe.**
`#[derive(WireRecord)]` packs with no implicit padding then rounds the stride to a word, so the
numbers cannot be recomputed by eye. Every constant in `wire.kel` is asserted against the derive's
generated `OFFSET_*` and `STRIDE` **by parsing them back out of the Keleusma source**; restating
them in the test would only prove the test agrees with itself. That protection was exercised
immediately: a field-extraction pattern of mine excluded digits and silently dropped
`DataSlotRecord::reserved2`, and the pinning is what would have caught it had the offsets moved.

**Three places where the value domain left no spare sentinel**, each resolved the same way — by
splitting the bound from the value rather than inventing an unrepresentable marker.

| Case | Why `0 - 1` will not serve | Resolution |
|---|---|---|
| An enum variant's discriminant | a discriminant of -1 is legal | `elay_variant_in_range` asked first |
| `DATA_SLOTS` presence | absent and empty are different programs | `data_layout_present`, then counts |
| A chunk's debug pool | absent differs from present-but-empty | the `ABSENT` sentinel, `u32::MAX` |

**Two parity schemes, deliberately different, and easy to conflate.** An opcode record carries one
BIT, the even parity of the popcount of its four bytes. A pool entry carries one BYTE, the
exclusive-or of the tag and the six payload bytes. The record's parity is computed in `wire.kel` as
a three-step fold rather than a bit count, and the equivalence is measured against an independently
written popcount definition over all 128 identifiers rather than argued.

**The CRC trailer is validated by a residue.** A reader checksums the whole artifact, trailer
included, and compares against a fixed constant, because appending a message's own CRC makes the
extended checksum invariant. The test derives that constant from four sealed messages rather than
restating the runtime's private one.

**A hard language limit surfaced and shaped the module.** The parser rejects an expression nested
more than 24 deep, so an `if / else if / ...` chain caps at about two dozen arms. The command
dispatch is nine chains, and a test sweeps every command below the ceiling to assert none falls
through to a chain default — the inverse hazard, where a drifting threshold silently routes live
commands to the default.

### The wiring increment: prep, 2026-08-09, BEFORE any code

Step 6 made the format expressible. Making the self-hosted path actually EMIT an artifact is the
next increment, and probing it first changed its shape — so this is recorded before anything is
written.

**A whole-artifact-in-one-buffer emitter cannot work for the real corpus.** The shared segment's
ceiling is `MAX_DATA_ADDR`, `1 << 24` = 16,777,216 bytes. Measured artifact sizes:

| Stage | Artifact | Share of the ceiling |
|---|---|---|
| `lexer` | **16,124,636** | **96.1%** |
| `parse` | 2,696,820 | 16.1% |
| `verify_typed` | 2,300,060 | 13.7% |
| the other seven | 107 KB to 784 KB | under 5% each |

`lexer.kel` alone leaves **652,580 bytes** of headroom, and the emitter's own input tables must live
in shared data too. So the largest stage does not fit its artifact and its inputs simultaneously,
and a stage slightly larger would not fit at all.

**Where the bytes go, measured rather than assumed.** For `lexer.kel` the opcode stream is 8,948
bytes and the operand pool 48, while the **auxiliary body is 16,115,568 — 99.94% of the artifact.**
The cause is the data segment: `lexer.kel` declares `bytes: [Byte; 393216]` plus two 1280-element
arrays, and the module reports **395,784 data slots**. **Every array element becomes its own data
slot with its own interned name**, so the aux body carries a `DataSlotRecord`, a `SharedSlotRecord`,
a `NameRef` and a pool string per element. The artifact therefore scales with DECLARED ARRAY SIZE,
not with program complexity — which is also why the `Names::intern` linear scan was catastrophic
rather than merely slow when it was found.

**Consequences for the increment, in order of how much they change it:**

1. **Emission must be staged, not buffered whole.** The natural shape is two passes: compute every
   region's length, write the leading directory from those lengths, then emit region by region with
   the host appending. That is compatible with the operator's chosen encoder strategy — buffer per
   region, leading directory — and it is what the `secret/` prototype's streaming stage did across
   yields.
2. **The emitter's buffer is a per-region working area, not the artifact.** Sizing it is then a
   question about the largest single region rather than the largest artifact, which is a far smaller
   number for every stage except possibly `lexer`.
3. **`lexer.kel` may need measuring per region before it is treated as in scope.** If one region
   exceeds a workable buffer, that stage needs the region itself chunked, and it would be better to
   know that now than to discover it after the driver exists.

**A separate question this raises, for the operator and not for the loop.** One slot and one interned
name per array ELEMENT is what makes a 21 KB source produce a 16 MB artifact. Whether an array should
instead occupy one slot with a length is a wire-format and data-layout design question with WCMU
implications, well outside a wiring increment. Recorded here because the measurement surfaced it, not
because it blocks anything: the format works as specified, it is merely much larger than the source
suggests.

**The largest single REGION, measured 2026-08-09.** This is the number that sizes a per-region
working buffer, and it is far kinder than the whole-artifact number.

| | bytes | share of the ceiling |
|---|---|---|
| Largest artifact (`lexer`) | 16,124,636 | 96.1% |
| **Largest single region** (`lexer`, `STRING_POOL`) | **6,609,960** | **39.4%** |
| Next largest region (`parse`, `STRING_POOL`) | 1,071,928 | 6.4% |

So a per-region working buffer of about 8 MB covers every stage with roughly 10 MB left for the
emitter's inputs. **The staged design is viable, and the whole-artifact design is not** — which is
the whole point of having measured before writing.

> **CORRECTED 2026-08-09 by the measurement in the next subsection.** The sentence above is wrong
> about the headroom, and wrong in the direction that matters. It treats the largest region as a
> **transient** working buffer, so it subtracts one region from the ceiling and calls the rest free.
> Two of the regions are **accumulators** that stay resident until the end, and for `lexer` they
> total 9,776,392 bytes rather than 6,609,960. The real remainder is about 7.0 MB, not 10 MB. The
> conclusion that the staged design is viable and the whole-artifact design is not still holds.

### The resident set, not the largest region, is the binding constraint (measured 2026-08-09)

The prep above asked how big the largest region is. That is the wrong question, and reading
`SchemaBuilder` is what shows why.

**`STRING_POOL` and `NAMES` are written LAST**, at the end of `SchemaBuilder::finish`
(`src/wire_schema.rs:833-837`), after every other region. Interning happens throughout: chunks
intern their names, struct templates intern a type name and every field name at
`src/wire_schema.rs:787-791`, and `flatten` interns while walking the constant forest. So the
content of those two regions is not final until every other contributor has run, which makes them
**accumulators held across the whole emission**, not buffers reused per region.

Per-region sizes across all ten stages, measured by encoding each stage's auxiliary body and
enumerating the container's region directory. The six largest regions per stage, `lexer` in full:

| Region | `lexer` | `parse` | `verify_typed` |
|---|---|---|---|
| `STRING_POOL` | 6,609,960 | 1,071,928 | 861,800 |
| `NAMES` | 3,166,432 | 464,424 | 468,808 |
| `DATA_SLOTS` | 3,166,272 | 462,408 | 468,632 |
| `SHARED_LAYOUT` | 3,166,224 | 329,800 | 449,072 |
| `CONSTS` | 4,176 | 278,256 | 40,352 |
| `CHUNKS` | 960 | 4,512 | 1,056 |
| **auxiliary body** | **16,114,608** | 2,616,320 | 2,290,408 |

Three things follow, and only the first was anticipated.

1. **The resident floor for `lexer` is 9,776,392 bytes**, `STRING_POOL` plus `NAMES`, which is
   **58.3% of the 16,777,216-byte ceiling**. Every other stage is under 10%, the next highest being
   `parse` at 9.2%. `lexer` is the only stage where this is tight, and it is tight by a wide margin.
2. **Four regions carry essentially the whole artifact.** For `lexer` the top four sum to 16,108,888
   of 16,114,608, so 99.96%. Optimising anything else is wasted effort.
3. **Three of those four are per-slot tables of identical length.** `NAMES` is 395,804 records,
   `DATA_SLOTS` 395,784, and `SHARED_LAYOUT` 395,778, each at an 8-byte stride. They scale one for
   one with data slots, which is the same per-array-element root cause recorded above. The artifact
   has three parallel copies of that count plus the pool of names they point into.

**What this changes about the increment.** Peak residency is the accumulator plus whichever region
is being built, so for `lexer` roughly 9.78 MB plus 3.17 MB, about 12.9 MB or 77% of the ceiling,
before the emitter's own inputs and before any dedup structure. That is feasible and it is not
comfortable, and it is a different design target from the one the prep set.

**A dedup index is required and is not free.** `Names::intern` is backed by a `BTreeMap` at
`src/wire_schema.rs:305`. A Keleusma emitter needs an equivalent, and a linear scan is known to be
catastrophic here rather than merely slow: `tests/wire_corpus.rs` records the corpus taking 782
seconds against about two and a half seconds once the quadratic interner was repaired. Whatever
structure replaces it consumes part of the remaining 7.0 MB.

**One constraint from the prep is softer than stated.** "Compute every region's length, write the
leading directory" assumed the directory must be written before the regions it describes. The host
owns the output buffer and can patch the directory after the fact, so lengths need not be known in
advance. The accumulator finding is independent of that and stands either way.

**Unverified.** The residency arithmetic above is derived from measured region sizes plus the
ordering read out of `SchemaBuilder`. No Keleusma emitter has been run against a real stage, so the
peak figure is a projection of the Rust encoder's structure onto a design that does not exist yet.
Treat 77% as an estimate to be confirmed by the first driver, not as a measurement.

### Records per region, which sizes every remaining slice (measured 2026-08-09)

The emitter receives a record's fields through `wire.fin`, a 1024-word batch buffer. Whether a slice
must implement batching is therefore `records * fields_per_record` against 1024, per region. Worst
case across all ten stages:

| Region | stride | fields | max records | worst stage | one batch? |
|---|---|---|---|---|---|
| `NAMES` | 8 | 2 | 395,804 | `lexer` | no, 774 batches |
| `DATA_SLOTS` | 8 | 4 | 395,784 | `lexer` | no, 1547 batches |
| `SHARED_LAYOUT` | 8 | 4 | 395,778 | `lexer` | no, 1547 batches |
| `CONSTS` | 16 | 4 | 17,391 | `parse` | no, 68 batches |
| `CHUNKS` | 48 | 14 | 94 | `parse` | **no, 2 batches** |
| `ENUM_VARIANTS` | 16 | 3 | 155 | `parse` | yes |
| `SHAPES` | 8 | 4 | 102 | `parse` | yes |
| `SIGNATURES` | 16 | 4 | 94 | `parse` | yes |
| `ENUM_LAYOUTS` | 16 | 4 | 3 | `parse` | yes |
| `DATA_INIT` | 8 | 2 | 1 | `lexer` | yes |
| `HEADER` | 32 | 11 | 1 | any | yes, done |

**`CHUNKS` is the right next slice for a reason the measurement supplies.** It is the **smallest
region that forces batching**, at two batches, so the mechanism gets built and exercised where the
failure is legible rather than inside a 1547-batch region. `ChunkRecord` also has **14 fields**,
more than `HeaderRecord`'s 11, so it is the widest record in the format and stresses the field
marshalling as well.

### Seven of twenty region kinds get NO emitter coverage from the corpus

This is the more consequential half of the measurement, and it enlarges a gap this document
previously recorded as a single region.

| Kind | State in the corpus |
|---|---|
| `DEBUG_POOL` | region never emitted at all (`emit_debug` defaults false) |
| `STRUCT_AUX` | emitted, **zero records** |
| `ENUM_AUX` | emitted, **zero records** |
| `STRUCT_TEMPLATES` | emitted, **zero records** |
| `PRIVATE_COMPOSITE` | emitted, **zero records** |
| `NATIVES` | emitted, **zero records** |
| `NATIVE_RETURNS` | emitted, **zero records** |

For a reader, an empty region and an absent one are different cases and both are covered. **For an
emitter they are the same problem**: no record of that kind is ever written, so a differential
validated only against these ten stages cannot see a mistranscribed offset in six of the seventeen
record shapes. Each needs a hand-built emitter case, exactly as `DEBUG_POOL` does.

The earlier statement that `DEBUG_POOL` is "the one region kind the corpus never emits" is true as
written and misleading as read. It is the only kind whose REGION is absent; it is one of seven whose
RECORDS never exist.

### `fin` is always the binding constraint, and the output window never is

There are two capacities in play and it was not obvious which binds. The input side is
`wire.fin`, 1024 words. The output side is `wire.bytes`, 65,536 bytes. Worst case per batch, over
every record kind:

| | records per batch | bytes of output |
|---|---|---|
| `ENUM_AUX` and `ENUM_VARIANTS` (the worst) | 341 | **5,456** |
| `NAMES`, `CONSTS`, `SIGNATURES`, others | 256–512 | 4,096 |
| `CHUNKS` | 73 | 3,504 |
| `DATA_SLOTS`, `SHARED_LAYOUT` | 256 | 2,048 |

**The largest batch any record kind can produce is 5,456 bytes, 8.3% of the output buffer, a
12-fold margin.** The reason is structural rather than lucky: a field occupies a whole word in
`fin` and at most four bytes in the record, so input is always at least twice the output and
usually four times.

**This collapses the batching design to one loop.** A slice needs input batching only. It does not
need to chunk its output within a batch, and it cannot overflow `wire.bytes` by emitting one
batch's worth. That is a materially smaller mechanism than the staged design implied.

### Slice 2's positioning does NOT generalise, and slice 3 must fix it

`emit_header_record` locates its record through `rec_at`, which is
`region_base(i) + rec * stride + off`, an **absolute artifact offset**. That works only because the
test emits a one-region artifact where HEADER sits at byte 96. In a real layout `CHUNKS` sits after
`STRING_POOL` and the others, so `region_base` is in the millions and lands far outside the
65,536-byte buffer.

**The staged emitter must therefore take a WINDOW BASE rather than derive position from the
directory.** The host emits a region's payload into a low window, appends it at the true offset, and
patches the directory afterwards, which the residency section already established it can do. Slice 2
is a correct special case, not a template, and the record-emitter signature should grow a window
base and a first-record index at slice 3 rather than later, while there is exactly one caller to
change.

Three further facts worth carrying:

- **The largest region is `STRING_POOL` for ALL TEN stages**, without exception. It is dominated by
  the interned per-element slot names, which is the same root cause as the artifact size.
- **`STRING_POOL` is a byte pool, not a record table**, so it is the region that streams most
  naturally: bytes are appended as names are interned, with no stride to respect. If even 6.6 MB
  proves awkward, this particular region is the easiest one to chunk further.
- **Every stage emits 19 regions**, out of the twenty kinds the schema defines, and the missing one
  is **`DEBUG_POOL`** — identified rather than left as "one of them". It is absent because
  `CompileOptions::emit_debug` defaults to false, so a release-style compile records no strippable
  debug annotations and the region is never created. Nothing is wrong; the corpus simply cannot
  reach that path.

  **A Keleusma emitter validated only against these ten stages would therefore never emit a
  `DEBUG_POOL` region.** The reader side is already covered — slice 5e's fixture includes a chunk
  with a real `debug_first`/`debug_len` and one with the `ABSENT` sentinel — but the emitter needs
  an explicit case, either a hand-built module or one compiled with `emit_debug` on.

  **Correcting an earlier claim in this document.** A previous revision said the gap "matches the
  recorded coverage caveat that the corpus emits zero struct templates". That is wrong:
  `STRUCT_TEMPLATES` is emitted by **all ten** stages. The struct-template caveat belongs to a
  different and much smaller hand-built round-trip corpus, and connecting the two was inference by
  plausibility rather than measurement.

  > **AND THAT CORRECTION IS ITSELF WRONG, measured 2026-08-09.** `STRUCT_TEMPLATES` is emitted by
  > all ten stages **and contains zero records in every one of them.** I conflated the region being
  > PRESENT with the region being POPULATED, and on that basis "corrected" a caveat in
  > `tests/wire_corpus.rs` that was substantively right. The original wording, "the corpus emits
  > zero struct templates", is accurate about the thing that matters for coverage. Two rounds of
  > correction, the first replacing a true statement with a false one, and both times the missing
  > step was counting records rather than looking for the region.

### The driver: prep, measured 2026-08-10, BEFORE any code

Every region kind now has an emitter driven by real compiler output. What remains is the driver,
where values stop being decoded from the reference and start being computed. Probing it first
changed its size, so this is recorded before anything is written.

**A WHOLE small artifact fits in the buffer, so the first slice is end-to-end rather than
per-region.** Measured auxiliary bodies:

| Program | aux body | share of the 65,536-byte buffer | regions |
|---|---|---|---|
| `fn main() -> Word { 42 }` | **912** | **1.4%** | 15 |
| two chunks | 1,008 | 1.5% | 15 |
| one local constant | 928 | 1.4% | 15 |
| a two-field shared data block | 1,152 | 1.8% | 19 |

The residency problem that governs `lexer` does not arise at this size at all. **The first driver
slice can therefore emit a COMPLETE artifact and compare it byte for byte**, which is a far smaller
and more decisive step than "reimplement `SchemaBuilder`".

For the minimal module the header area is 48 + 48x15 = 768 bytes and the payloads total 144, which
sums exactly to the measured 912. The driver's arithmetic is checkable by hand at this scale.

**THE REGION ORDER, MEASURED FROM THE DIRECTORY RATHER THAN INFERRED FROM THE CODE.** Byte identity
depends on it entirely, and it is not the schema's numeric order:

```
SIGNATURES, ENUM_VARIANTS, ENUM_LAYOUTS, NATIVES, NATIVE_RETURNS,
[DATA_SLOTS, SHARED_LAYOUT, PRIVATE_COMPOSITE, DATA_INIT],
HEADER,
CONSTS, STRUCT_AUX, ENUM_AUX, STRUCT_TEMPLATES, PARAM_TYPES, SHAPES, CHUNKS,
[DEBUG_POOL],
STRING_POOL, NAMES
```

It is the order the `add_*` calls run in, followed by everything `finish` defers. **Most regions are
present with length ZERO rather than absent**, which is why the count is 15 and not 7. Only two
groups are conditional:

- the four data-layout regions, present only when the module has a data layout (15 -> 19)
- `DEBUG_POOL`, present only under `emit_debug` (19 -> 20)

**What the driver must COMPUTE rather than copy**, which is the whole of the remaining work:

1. **Name interning with dedup**, feeding `STRING_POOL` and `NAMES`. `Names::intern` is a
   `BTreeMap`; a linear scan is known to be catastrophic here rather than merely slow, so the
   Keleusma side needs an index structure of its own.
2. **Constant flattening**, breadth-first with roots pinned to `0..n`, feeding `CONSTS` plus the
   `STRUCT_AUX` and `ENUM_AUX` side tables, and preserving the forward-ordering invariant that lets
   a reader walk the table by one reverse sweep with no stack.
3. **Per-chunk ranges** — `consts_first/count`, `templates_first/count`, `param_types_first/count` —
   which are allocation results of the order the contributors ran in.
4. **Region lengths and the directory**, which the host may patch afterwards rather than the emitter
   computing up front.

A minimal module needs one interned name, one constant root, and no templates, so **items 1 to 3 are
nearly trivial at that size** while still being the real code paths. That is the first slice.

#### The minimal module's complete input surface (measured 2026-08-10)

Everything the encoder consumes for `fn main() -> Word { 42 }`, so the first slice knows exactly
what the driver must marshal and nothing is discovered mid-implementation:

| Input | Value |
|---|---|
| chunks | **1** — name `"main"`, `local_count` 0, `param_count` 0, `block_type` `Func` |
| param types | none |
| constants | **one root**, `Int(42)` |
| struct templates | none |
| signatures | **1** — no params, `ret` `Scalar{kind:3}`, `resume` `Top` |
| enum layouts, natives, native returns, data layout | all absent |
| entry point | `Some(0)` |
| widths | `word`/`addr`/`float` all log2 = 6 |

**The arithmetic closes exactly, which is the check that the list is complete.** Those inputs produce
`STRING_POOL` 8, `NAMES` 8, `CONSTS` 16, `SHAPES` 16, `SIGNATURES` 16, `CHUNKS` 48, `HEADER` 32 —
**144 bytes of payload** — and 48 + 48x15 = 768 of header area, totalling **912**, which is the
measured artifact size to the byte. Nothing is unaccounted for.

**One detail worth carrying**: a single signature with no parameters still produces **two** `SHAPES`
records, for `ret` and `resume`. Shapes are interned and shared between signatures and native
returns, so the driver's shape table is its own small interner rather than a per-signature array.

#### The interner has TWO modes, and a deduping-only port would be wrong (measured 2026-08-10)

I was about to build the driver's interner slice on an oracle that does not work, and probing killed
it before any code existed.

**The intended oracle**: recover the interner's input from a real artifact — `NAMES` gives the
distinct names in order, `STRING_POOL` is measured to be a **pure concatenation** in that order
(`non_append = 0` for every stage checked) — then feed that list to a Keleusma interner and require
byte identity.

**It fails, and the failure is informative.** Duplicate entries in `NAMES`, by stage:

| stage | names | distinct | duplicate entries |
|---|---|---|---|
| `lexer` | 395,804 | 395,804 | 0 |
| `analyze` | 20,147 | 20,147 | 0 |
| `verify_yield` | 7,954 | 7,954 | 0 |
| `verify_datalayout` | 3,086 | 3,086 | 0 |
| **`parse`** | **58,053** | 58,033 | **20** |

`parse` carries twenty `NAMES` entries whose bytes duplicate an earlier entry. Feeding the recovered
distinct list through a deduping interner would produce 58,033 records where the reference produced
58,053, so the artifacts could not match.

**The cause is `intern_fresh`, and it exists for CONTIGUITY rather than freshness.** Two sites
deliberately append even when the bytes already exist:

- struct-constant field names (`wire_schema.rs:417`), so field `i` is at `field_names_first + i`
- enum-variant names (`:730`), so a layout's variants are one contiguous run

Deduping either would collapse a run and break `first + i` addressing. `parse` is the only corpus
stage with enum layouts populated, which is exactly why it is the only one with duplicates.

**Two consequences for the driver:**

1. **The Keleusma interner needs both modes**, `intern` and `intern_fresh`, and the caller chooses.
   A port that only dedups is not a simplification, it is a defect that would surface only on a
   program with enum layouts or struct constants — neither of which the smallest test cases have.
2. **The interner cannot be validated in isolation against the corpus.** Its input is not a list of
   names but a list of (name, mode) pairs, and that sequence is a property of the caller, not
   recoverable from the output. The interner therefore has to be tested as part of a whole-artifact
   differential rather than as a standalone unit — which is another argument for the minimal
   end-to-end first slice recorded above.

#### The flattener's composite path is unreachable from the corpus (measured 2026-08-10)

Every constant in every stage is a **scalar**. Measured over all ten sources:

| | |
|---|---|
| constant nodes | **2,192** |
| composite nodes | **0** |
| maximum tree depth | **0** |

So the breadth-first walk never enqueues a child, and **the corpus cannot distinguish a correct
flattener from one that appends the roots and stops.** The forward-ordering invariant, the child
numbering after the roots, and the `STRUCT_AUX` and `ENUM_AUX` side tables are all unexercised —
which also explains why those two regions measured empty in every stage.

The flattener therefore needs hand-built constant trees, oracled against `encode_aux_body` on a
constructed module, exactly as the slice-8 record kinds were oracled against the derive.

#### `Shapes` is a second interner with the SAME two modes, and opposite performance needs

`Shapes::append` keeps a contiguous run and `Shapes::intern` reuses an identical entry — the same
pair as `Names`. Two things follow, and they differ:

- **`Shapes::intern` is a LINEAR SCAN** over the existing records. That is the shape of the defect
  that made `Names::intern` take 782 seconds on this corpus before it became a `BTreeMap`.
- **It is fine here, and the measurement says why**: shape counts peak at **102**, against 395,804
  names. A Keleusma port should copy the linear scan for shapes and must not for names.

#### THE PATTERN, WHICH IS NOW THREE FOR THREE

The ten stage sources are the largest real Keleusma programs that exist, and this arc has now found
three separate paths they cannot reach: six record kinds emitted empty, every composite constant,
and the second interning mode. They are **large but semantically narrow** — no generics, no struct
or enum constants, no natives, no struct templates, almost no composites of any kind.

**Real compiler output is a strong oracle for VOLUME and a weak one for VARIETY.** Volume is what
caught the quadratic interner and what deep batching needs; variety needs constructed cases. A slice
should say which of the two it is buying, because "validated against the corpus" reads like both and
is only ever one.

### On the prototype

`secret/kel-format-probe/wirefmt.kel` proves the encoder and decoder are expressible in Keleusma, but
it **predates format lock-in** and encodes a 12-byte directory entry. The shipped entry is 16 bytes,
and the triplicated prologue postdates it entirely. Treat it as evidence of feasibility, not as a
starting implementation.

---

## The 2026-08-03 analysis, superseded above but retained

The correction it makes to the roadmap's cost estimate is still accurate and worth keeping. Only its
conclusion — that the item is blocked — has been overtaken.

## The correction

`V0_2_X_ROADMAP.md` describes the item as: "The framing header, operand-pool encoding, parity, and
CRC trailer must move into Keleusma so the emitted artifact is produced end to end by the self-hosted
path." That enumeration omits the dominant cost.

`module_to_wire_bytes` (`src/wire_format.rs` ~1591) produces four regions:

| Region | Encoding | Self-hostable? |
|---|---|---|
| Framing header | 64 fixed bytes | Yes, mechanical |
| Opcode stream | 4-byte records via `encode_op` (~206 lines) | Yes, mechanical |
| Operand pool | 8-byte flat entries | Yes, mechanical |
| CRC trailer | CRC-32, reflected, poly in `crc32` | Yes, ~15 lines |
| **Auxiliary body** | **`rkyv::to_bytes`** | **NO — see below** |

**The auxiliary body is rkyv-archived**, and it carries everything except the opcode stream and
operand pool: per-chunk metadata (name, constants, struct templates, local/param counts, block type,
param types, debug pool), enum layouts, signatures, native return shapes, native names, entry point,
data layout, the word/addr/float width fields, the WCET/WCMU header, flags, shared and private data
sizes, and the schema hash.

rkyv is a ZERO-COPY ARCHIVE format: relative pointers, alignment and padding rules, a resolver
protocol, and its own versioning. Reproducing its byte layout in Keleusma is not "serialization" in
the sense the roadmap implies — it is reimplementing a third-party archival format byte-for-byte, with
the byte-identical oracle demanding exact agreement including padding. That is disproportionate, and
it is also FRAGILE: an rkyv upgrade would silently invalidate the Keleusma implementation.

**Therefore "self-host wire-format serialization" as literally stated is NOT a bounded increment, and
it is not the cheapest Order-1 item.** The earlier recommendation to start here (recorded in
`REVERSE_PROMPT.md` and `HANDOFF.md` on 2026-08-03) was based on the roadmap's enumeration and is
withdrawn.

> **Superseded 2026-08-08.** True while the auxiliary body was rkyv. It no longer is. See the
> re-scoping at the top of this document.

## What IS bounded

The four non-rkyv regions are mechanical and have a clean byte-identity oracle: emit them from
Keleusma and compare against the same regions of the reference's buffer. Suggested slicing, smallest
first, each independently verifiable:

1. **CRC-32.** ~15 lines, a pure function over a byte range. Trivially oracle-checked against
   `crate::bytecode::crc32` on random and edge-case buffers. Good first slice to establish the
   byte-emission harness.
2. **The opcode stream and operand pool.** The meaty part, and exactly what the roadmap calls
   "operand-pool encoding". codegen.kel already carries an internal op encoding whose tag values ARE
   the opcode ids (`getfield` 47, `getindex` 49, `gettuplefield` 53, ...), so the mapping to 4-byte
   records is close to what it already computes. Verify against `encode_op` over every op form,
   including the pool-spill cases.
3. **The framing header.** 64 fixed bytes; needs the region lengths from 1 and 2 plus the aux length.

After those three, the artifact is Keleusma-produced EXCEPT the aux body, which remains host-supplied
as an opaque byte block. That is an honest partial result and should be described that way — it does
NOT meet the Order-1 gate's "no Rust scaffold borrow" wording.

## The open question for the operator — ANSWERED 2026-08-08

Fully self-hosting the artifact requires a decision this plan cannot make: either reimplement rkyv's
layout in Keleusma (disproportionate and fragile), or **change the wire format** so the aux body uses
an encoding the self-hosted compiler can produce — which is a format change, and therefore a
`BYTECODE_VERSION` question and an operator decision under the standing rules.

Until that is decided, treat Order 1 as reachable only in part, and prefer the other two remainders
(the monomorphizer, then the type checker) for closing it.

> **Answered.** The operator chose the second option. The wire format was changed, the auxiliary body
> now uses the v2 container, and `BYTECODE_VERSION` moved to 2 under authorization granted
> 2026-08-06. Order 1 is reachable in full, and the deferral advice above no longer applies.

## Note on the monomorphizer

Probed at the same time: the `.kel` stage sources use no generics (the four `impl`/`trait` hits in
parse.kel are its own parser code for those keywords, not uses). Monomorphization is therefore
IDENTITY over the self-hosting subset, which is why the pipeline omits the pass entirely and still
matches the reference byte-for-byte. Self-hosting it would tick the box without changing any output.
Its real cost arrives only with full-language generics (Workstream F), so its value here is formal
rather than functional — worth knowing before it is picked as "the cheapest".
