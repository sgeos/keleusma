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

**To probe before writing the driver:** the largest single REGION across the ten stages, which is the
number that actually sizes the working buffer.

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
