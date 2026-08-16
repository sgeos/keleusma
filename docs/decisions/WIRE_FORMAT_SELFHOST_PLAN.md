# Self-Hosting Wire-Format Serialization — Scoping

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Scoping for the Order-1 residual "self-host wire-format serialization". Probed 2026-08-03,
**re-scoped 2026-08-08**.

Status: **STEP 6 COMPLETE (2026-08-09). All seven slices are done.** The wire format is
expressible in Keleusma end to end: CRC-32, the container primitives and prologue vote, the region
directory, record tables and byte pools, the twenty region kinds and seventeen record shapes, the
opcode stream and operand pool, and the framing header with its CRC trailer.

`src/selfhost/kel/wire.kel` is the implementation and `tests/selfhost_wire.rs` is the differential.
~~`wire.kel` is not yet driven by the pipeline, and it is deliberately absent from `read_stage`.~~

> ## CURRENCY BANNER (2026-08-15). READ THIS BEFORE ANY FIGURE BELOW.
>
> **Three claims in this document went stale and each produced a wrong conclusion downstream.**
> They are struck rather than deleted, because the reasoning around them is still worth reading and
> a deleted claim leaves a reader unable to tell what changed.
>
> **1. `wire.kel` IS in `read_stage` and IS driven.** It joined when `wire_names_via_kel` gained a
> path that emits through it, and that driver now takes a `Module` and builds its own input. The
> struck sentence above, and every "wiring remains" statement below, describes a state that ended.
>
> **2. THE `395,804` FIGURE IS NOT A NAME COUNT, and this banner governs every appearance of it
> below.** It is a REGION RECORD count belonging to `CONSTS`, and it was true of `NAMES` only
> before the run-length encoding collapsed the per-array-element slot names. Measured across all
> ten stages on 2026-08-15: **the largest `NAMES` region is 627 records, in `parse`, from a
> 33,395-byte blob.** Where the figure appears in a table of measurements taken at the time, it is
> a historical measurement and is left standing under this banner. Where it drives a CONCLUSION
> about work that remains, it is corrected in place.
>
> **This figure has now produced a wrong conclusion twice.** It made a 2.5x problem look like a
> 1500x one, and it invented a dependency between the interning producer and the residency
> staging — work the plan said "are the same increment, and doing either alone is wasted" when in
> fact no stage in the corpus needs staging at all. **A stale figure does not merely misstate a
> size; it manufactures dependencies between pieces of work.**
>
> **A note on this sweep, since it is the same failure in miniature.** The sweep was scoped as
> "five sites". There are about sixteen appearances, of which roughly ten are stale. The count of
> the sites of a figure whose lesson is checking figures was itself unchecked.
>
> **3. Test counts are not restated here.** This document said 80; the file holds 163. Derive it:
> `grep -c '^\s*#\[test\]' tests/selfhost_wire.rs`.

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

#### OPTION C PART TWO IS NOT PART ONE AGAIN: `SHARED_LAYOUT` IS ON A HOT PATH (2026-08-13)

Part one run-length encoded `DATA_SLOTS` and was nearly free at runtime: the only
consumer, `src/vm.rs`, merely COUNTED slots, so summing runs replaced 91,183
iterations with about 60 and was faster.

**`SHARED_LAYOUT` has a different consumer and the difference is categorical.**
`Vm::shared_layout_entry` calls `dl.shared_slot(slot)` with a LOGICAL slot index,
reading the artifact in place, and it runs on every `get_shared` and `set_shared`
— every host access to a shared slot. Run-length encoding turns that O(1) array
index into a scan over runs.

**So the two-word `run + stride` record described earlier is incomplete.** It is
right about size and silent about cost. A correct design needs one of:

- **A `first_slot` field per record**, so a logical index resolves by binary
  search over runs. `offset: u32, kind: u8, reserved: u8, len: u16, first_slot:
  u32, run: u16, stride: u16` fits two words and keeps lookup O(log runs).
- **Expansion at decode time into an owned `Vec`**, which contradicts the in-place
  zero-copy reading the container was built for (P10) and is therefore the worse
  answer here, not merely the slower one.

**The stride is derivable but should not be derived.** A scalar run advances by
`kind.size_in_bytes()` and a composite run by `len`. Deriving it would put a
kind-to-size table on BOTH the Rust and Keleusma sides, and cross-language
duplication of exactly this shape produced three separate defects on 2026-08-12
(the multihead predicate in three copies, the `DATA_SLOTS` row decoder reading a
`u16` as a byte, and the naming convention encoded as a lookup index in three
test helpers). Store it.

**Expected saving, for judging whether the complexity earns its place**:
`SHARED_LAYOUT` is 43,032 bytes of `codegen`'s 154,880 after part one, so roughly
27% of what remains. Part one delivered more for less.

##### MEASURED 2026-08-13: THE RUNS ARE ENORMOUS, AND A CONCERN I RAISED IS REFUTED

**The section above is silent on the one number that decides the increment.** A
run record needs `first_slot`, `run` and `stride` on top of the existing fields,
which takes `SharedSlotRecord` from ONE word to TWO. So the encoding is a
**pessimisation** unless the mean run exceeds 2, and neither the plan nor the
27% figure measured the distribution that depends on. I raised that as a blocker
before writing any code. **It is refuted, by four orders of magnitude.**

Measured across all eleven stage sources (`tests/shared_layout_runs.rs`), with
the `u16` `run` field's 65,535 chunking accounted for:

| | slots | runs | mean run | table now | run-length | |
|---|---|---|---|---|---|---|
| `lexer` | 395,778 | 3 | 131,926 | 3,166,224 | 144 | |
| `verify_typed` | 56,134 | 1 | 56,134 | 449,072 | 16 | |
| `wire` | 91,143 | 6 | 15,190 | 729,144 | 112 | |
| `codegen` | 5,379 | 1 | 5,379 | 43,032 | 16 | |
| **all eleven** | **643,276** | **18** | **35,738** | **5,146,208** | **400** | |

**Break-even is a mean run of 2. The measurement is 35,738.** The table does not
shrink by 27%, it effectively disappears: eighteen records stand for the entire
shared layout of every stage this project compiles.

**Two consequences for the design, one of which simplifies it.**

- **The `first_slot` binary search is still right, but it is no longer the
  interesting cost.** With one to six records per stage, even a linear scan
  would be fast. Binary search is kept anyway, because "fast in practice on
  today's corpus" is not the property this project sells — a scan's bound is
  data-dependent, and a `log2(records)` bound is static. The hot-path warning
  above stands as reasoning even though the magnitude collapsed.
- **The `u16` `run` field is load-bearing rather than incidental.** `lexer`'s
  largest run is 393,216, so it chunks into seven records exactly as
  `DATA_SLOTS` does. Counting logical runs rather than emitted records
  understates `lexer` sevenfold, which is the kind of error that would make a
  projection look better than the artifact.

**Why the measurement is now a test rather than a note.** The payoff is a
property of how stages DECLARE shared data — a few large arrays — not a property
of the encoder. A stage that declared many small distinct shared slots would
fragment the runs and silently turn the encoding into a size regression that
nothing else would report. `tests/shared_layout_runs.rs` fails first, and it
carries a control on **its own guard**: a fully fragmented synthetic layout is
asserted to be rejected by the same threshold, because a check passing by four
orders of magnitude is otherwise indistinguishable from one that can no longer
report anything.

#### CORRECTION: THE PROJECTION STANDS. WHAT THE PLAN OMITTED IS ARTIFACT SIZE (2026-08-11)

**Two earlier commits on this branch claimed the 77% projection was "refuted by about a factor of
forty" and derived a ~321,000-slot budget from it. That was wrong, and the error is worth naming
precisely because the wrong number looked authoritative:** the budget divided a **byte-addressing
ceiling** by a figure measured in **bytes of artifact per slot**. Those are different quantities, and
the spurious factor of forty was the units mistake itself.

**`MAX_DATA_ADDR` does not bound the artifact.** It is 2^24 and bounds a data-segment byte offset, a
unified data-slot index, and an indexed-array length (`src/bytecode.rs:3548-3556`). The wire
container addresses regions with **u32 word offsets and lengths** (`keleusma-wire/src/layout.rs`,
`WORD = 8`), so an artifact may reach roughly 34 GB. There is no 16 MB artifact ceiling to exhaust.

Against the ceilings that do bind, `lexer`'s 9,776,392-byte accumulator needs:

| Ceiling | Needed | Share |
|---|---|---|
| Shared segment, 16,777,216 bytes | 9,932,096 with today's declarations | **59.2%** |
| Slot index, 2^24 | 9,867,573 with today's 91,181 slots | 58.8% |

**Which is the 58.3% the section above already recorded.** The original projection was right, and the
peak figure of roughly 77% including the region under construction stands unchallenged. The
per-stage accumulator table that followed the retracted claim is deleted with it: every stage clears
the real ceilings, so a fits-or-not column measured against a phantom budget said nothing.

**What the measurement DID establish, and what the plan genuinely omitted**, all still valid:

1. **A data slot per ARRAY ELEMENT.** Exact deltas: `65,536 - 8,192 = 57,344` slots added, and
   `1,000,000 - 8,192 = 991,808`. The per-element cost the operator holds, now with a number on it.
2. **About 40.7 bytes of ARTIFACT per slot**, stable to within 1% across a fourfold range (91,181 to
   345,133 slots), because each element carries a slot record, a `SHARED_LAYOUT` record, an interned
   name and that name's pool bytes.
3. **Compile time of roughly 2.4 seconds per megabyte declared**: 1.30 s at 64 KB, 3.54 s at 1 MB,
   17.24 s at 6.6 MB, paid on every build rather than once.

**So declaring `lexer`'s accumulator costs a `wire.kel` whose own auxiliary body runs to the order of
400 MB and which takes roughly 25 seconds to compile.** That is a serious practical cost and it is
what the residency analysis did not price. It is **not** a limit violation and the increment is not
blocked on it. Those three figures are what the per-element representation costs; fixing that
representation is what makes them go away.

**Method note, since this section now carries two corrections in a row.** The first over-generalised
from `lexer` to every stage. The second divided two incompatible units and produced a confident,
authoritative-looking budget that was 40x too small. Both were caught by asking what a number
actually BOUNDS rather than reusing one of the right order of magnitude. **A constant that appears in
several places for several reasons is exactly where this goes wrong**: 2^24 is a byte offset, and a
slot index, and coincidentally close to `lexer`'s artifact size, which is what made the mistake look
self-consistent from three directions at once.


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

**MEASURED 2026-08-11, and the threshold is sharper than "a real layout": EVERY stage fails, the
smallest one included.** Region byte offsets against `wire.bytes`, which is 65,536:

| Stage | artifact | `CHUNKS` at | `STRING_POOL` at | `NAMES` at |
|---|---|---|---|---|
| `verify_datalayout` | 105,848 | 50,464 | 50,560 | **81,160** |
| `verify_yield` | 303,464 | **143,096** | **143,480** | **239,832** |
| `codegen` | 480,416 | **236,216** | **239,864** | **391,160** |

`verify_datalayout` is the smallest of the ten and its `NAMES` region already starts 15,624 bytes
past the end of the buffer. **Absolute positioning therefore works only for artifacts under 65,536
bytes, which is no stage at all** — it holds for the constructed corpus and for nothing else. This is
a hard prerequisite for driving any real stage, not an optimisation, and it is independent of
batching: batching fixes how many records reach the emitter per call, and the window base fixes where
they land.

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

Every region kind now has an emitter. **Fourteen of the twenty are driven by real compiler
output; six are oracled against the derive with constructed values**, because the corpus emits them
empty. What remains is the driver,
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

#### SLICE 12 — the interner is DONE, and it found a third mode-like trap

Commands 136 to 140 in `wire.kel`. Keleusma now computes `STRING_POOL` and `NAMES` from a sequence
of (name, mode) pairs instead of re-emitting decoded offsets, and the two regions are byte-identical
inside a complete artifact across six constructed sources. **This is the first value the driver
computes rather than copies.**

**A LAST MATCH WINS, AND A FORWARD SCAN WOULD HAVE BEEN SILENTLY WRONG.** `intern_fresh` does
`index.insert`, which OVERWRITES, so when the same bytes have been appended twice a later `intern`
resolves to the SECOND index. Measured rather than reasoned: for
`enum A { X, P } enum B { X, Q } enum X { R }` the reference's third layout cites name index **5**,
while indices 2 and 5 both hold `"X"`.

The consequence is what makes it worth recording. **First-match and last-match produce
byte-identical `NAMES` and `STRING_POOL` regions.** The divergence appears only in a record that
cites an index — here `ENUM_LAYOUTS.type_name`. Had the scan been written the obvious way, every
test in this slice would have passed and the defect would have surfaced only once some later slice
computed a record's name field.

**That forced a design change, and the trade was worth taking.** The rule was untestable through the
two regions the slice emits, so the interner now also produces an **input-to-index map** in the upper
half of `wire.fin`, which halves the admissible name count from 512 to 256. Logic whose test cannot
fail is worse than a lower cap. The map is also the piece downstream records need, so the next slice
does not have to reach back for it.

**The linear scan is a KNOWN DEBT with a measured cost, not an oversight.** The reference used one
and encoding a mid-sized stage took 782 seconds before it became a `BTreeMap`. At the sizes this
slice drives — ten names — a scan is correct, and the note sits in `wire.kel` where the next reader
will be, not only here. ~~**It must be replaced before the interner is driven by a real stage**, where the count reaches
395,804.~~ **CORRECTED 2026-08-15**: the interner IS driven by real stages and the count reaches
**627**, not 395,804. The scan was not replaced and did not need to be. Separately, the scan has no
real-module coverage at all: making `nm_find` report "not found" unconditionally leaves all ten
stages byte-identical, so **the cap this scan's cost justifies is priced on a path no stage
reaches**.

**Caps are stated and enforced with codes, not by truncation**: 256 names (`-230`), 256 bytes per
name (`-231`), the `wire.bin` capacity (`-232`), and an out-of-range map query (`-235`). All four
have a negative test.

**Two incidental measurements**, both narrowing later slices:

- A bare `enum` declaration populates `enum_layouts` with **no use site required**, so a duplicate
  case costs three lines of source and about 1 KB of artifact.
- A plain struct literal does **not** populate `struct_templates`. That path stays unreachable from
  ordinary source, consistent with `STRUCT_TEMPLATES` measuring empty in every stage.

**`emit_in_region` covered exactly the minimal module's eight kinds** and refused `ENUM_VARIANTS`
with `-222` on the first constructed source that declared an enum. The guard slice 11 added did its
job on its first real test: a refusal, not a mis-sized region. Both enum kinds are now wired.

**WHAT IS STILL NOT COMPUTED, stated so it is not discovered mid-implementation.** The (name, mode)
SEQUENCE is generated by a Rust model of the encoder's call order (`interner_input` in
`tests/selfhost_wire.rs`), restricted to chunk names and enum layouts and guarded by
`assert_no_other_contributors`, which refuses a module with natives, a data layout, or struct
templates rather than silently under-generating. Producing that sequence from the AST is the
self-hosted driver's job and remains open.

#### SLICE 13 — the breadth-first reordering is DONE (scalars, tuples, arrays)

Command 141. Keleusma reorders a depth-first preorder into the breadth-first `CONSTS` table and
writes it into its region, byte-identical to `encode_aux_body` across six constructed sources. The
input is deliberately depth-first: a breadth-first input would make the test vacuous.

**A vacuity control caught that the test was four-fifths empty.** The differential passed on its
first run; the accompanying assertion that at least two cases distinguish the two walks did not.
Two causes, both invisible from the green differential:

- **A composite in LAST position makes the walks coincide.** `(1, (2, 3))` is identical under both,
  and four of the five original cases had that shape. Fixed with `((1, 2), 3)`.
- **Tag sequences are too coarse a discriminator.** `((1, 2), 3)` gives 8, 8, 3, 3, 3 either way
  while visiting the scalars in different orders, so the check compares (tag, payload) pairs.

**A corpus-level control is a distinct instrument from a must-fire mutation.** A mutation asks
whether the check can report a defect; this asks whether the INPUTS can tell two answers apart at
all. The differential here was strong against the reference and weak against the corpus, and only
the second kind of control said so.

**Two places a total language cost nothing.** There is no `while`, but the queue provably ends at
exactly `nnodes` entries, so `for head in 0..nnodes` walks it exactly — the bound the language
demanded was already known. And the reference's `next_index` is provably equal to the queue length
at every step, so the Keleusma side keeps one field instead of two that could disagree.

**Validation precedes sizing because `limit` TRAPS rather than reports.** `for k in 0..n limit 341`
aborts the VM when the runtime range exceeds the cap, so a malformed child count must be rejected
before it is ever used as a bound; a sticky flag would be set too late. Cursors are clamped as well
as flagged, so a refusal is issued from a memory-safe state.

**Scope and codes.** Scalars, tuples and arrays. `STATIC_STR`, `STRUCT` and `ENUM` intern names as
they walk, coupling the flattener to the interner and the two side tables — the next slice. Caps:
341 nodes (`-240`), an impossible child count (`-241`), a cursor out of range (`-242`), a queue
overrun (`-244`), an out-of-scope tag (`-245`), `nroots > nnodes` (`-246`). Each has a negative test.

#### SLICE 13b — DESIGNED, NOT BUILT: the flattener drives the interner

Scoped while the machine was held by the other session, so implementation is mechanical rather than
exploratory. **Everything here marked (measured) was checked against the reference; everything else
is a design choice and may not survive contact.**

**THE COUPLING IS THE WHOLE POINT.** `STATIC_STR`, `STRUCT` and `ENUM` intern names *as `flatten`
walks*, so the name sequence is a function of the BREADTH-FIRST order, not of the input order. The
interner therefore cannot run as a separate pass over a host-supplied list the way slice 12 does —
the flattener has to drive it. That is also what makes the slice worth doing: it is the first place
two computed values interact.

**It is genuinely testable, which was not obvious** (measured). A string can sit *inside* a
composite: `const_value_from_literal_for_field` maps `(Literal::String, PrimType::Text)` to
`StaticStr` and recurses through tuple, array and struct initialisers, so
`const data k { t: (Text, Word) = ("hi", 1) }` puts a `StaticStr` at a child position. A child is
interned at its breadth-first index, not its preorder one, so depth-first and breadth-first produce
different `STRING_POOL` bytes — the reordering and the interning are observably coupled rather than
merely adjacent.

**Per-node interning order** (measured, `wire_schema.rs:412-442`), and the middle line is the one
that is easy to get wrong:

| tag | sequence |
|---|---|
| `STATIC_STR` | `intern(s)` -> `aux` |
| `STRUCT` | `intern(type_name)`, **then** capture `field_names_first` **after** it, then `intern_fresh(field)` per field |
| `ENUM` | `intern(type_name)`, `intern(variant)` |

`field_names_first` is read *after* the type name is interned, so a port that captures it first is
off by one on every struct — and only on structs whose type name is not already present.

**Channels.** The preorder grows to six words per node — tag, payload, child count, `names_first`,
flags, discriminant — because an enum needs `FLAG_HAS_DISCRIMINANT` and a signed discriminant that
`payload` cannot carry (a composite's payload is its child range). 1024 words is then a **170-node
cap**, to be stated and enforced with a code. Per-node name groups arrive in PREORDER in a new
`wire.nin`, with bytes in `wire.bin`; `names_first` indexes that group.

**Outputs go straight into the regions, not into scratch.** `NAMES`, `STRING_POOL`, `CONSTS`,
`STRUCT_AUX` and `ENUM_AUX` are written where the directory says they go, and the dedup scan reads
the `NAMES` and `STRING_POOL` regions back out of `wire.bytes`. **The interner's state IS the
artifact**, so "what I think I emitted" and "what is in the artifact" cannot drift apart — which is
the failure a separate scratch buffer would make possible.

**Two walks, because the region lengths are RESULTS.** The name count, pool length and both aux
counts are outcomes of the walk, but the directory has to be laid before anything can be written
into a region. So a counting walk runs first into a scratch window high in `wire.bytes` (artifacts
are about a kilobyte against 65,536, so the space is free), the host relays the figures back, and the
emitting walk runs again. Deterministic, so the second walk is the same answer rather than a second
answer — the same argument slice 12 already rests on.

**THE FLATTENER'S INTERNER STARTS PART-WAY THROUGH, NOT EMPTY** (measured). `flatten` is called
inside `SchemaBuilder::finish` (`wire_schema.rs:765`), after every `add_chunk` and after
`add_enum_layouts`. So constant-interned names are appended to a table that already holds the chunk
names and the enum-layout names, and the indices a `STATIC_STR` or `STRUCT` record cites depend on
what came before.

The consequence for the command shape is concrete: since shared data is re-seeded on every call, one
call must both seed the prefix (the chunk and layout names, as slice 12 already handles) **and** run
the walk that continues interning from there. A slice that interned from empty would produce a
correct-looking `CONSTS` table citing indices that are wrong by the size of the prefix — and on the
minimal module the prefix is one name, so the error would be a quiet off-by-one rather than an
obvious break.

**IN-PLACE POOL COMPACTION DIES HERE, AND THE ARGUMENT IS SHORT ENOUGH TO CHECK.** Slice 12
compacts the pool over its own input in `bin`, justified by the output cursor never overtaking the
input cursor. That holds only while names are interned **in input order**. The walk interns in
BREADTH-FIRST order, and then it fails:

```
nin = [ A(10 bytes), B(10 bytes) ]      bin = AAAAAAAAAABBBBBBBBBB
BFS reaches B first  ->  copy input 10..19 to output 0..9   bin = BBBBBBBBBBBBBBBBBBBB
BFS reaches A next   ->  read input 0..9                    reads B, not A
```

So the pool needs its **own output buffer**, `wire.bout`. Input in `bin` is then never written, and
the dedup scan compares an emitted name in `bout` against a candidate in `bin` — different arrays, so
the aliasing question disappears rather than being argued. This is a change to slice 12's mechanics,
not only an addition to them.

**A second consequence: interning out of order needs each input name's BYTE OFFSET**, which slice 12
never materialised because it walked `nin` sequentially with a running cursor. Two ways to get it,
and the cheaper one is a prefix-sum prepass over `nin` into the spare upper quarter of `nout`
(`[768, 1024)`, exactly the 256-name cap), rather than widening `nin` to (offset, length, mode)
triples — which would change slice 12's input format and every test that feeds it.

**This was recorded the other way round an hour earlier**, as "the pool is still compacted in place
in `bin`, which is sound for the reason recorded at `intern_run`". That sentence is true of slice 12
and false of slice 13b, and the difference is exactly the property the new slice changes. Worth
noting as a pattern: **a justification carried forward with the code it justified is the easiest kind
of stale documentation to produce**, because nothing about the move looks like an edit.

**ALL FOUR ASSUMPTIONS ARE NOW MEASURED, AND ALL FOUR HOLD (2026-08-11).** Probed before writing
any of the slice, which is the discipline the flattener error bought:

| # | Assumption | Verdict |
|---|---|---|
| 1 | a string can sit at a CHILD position | **YES** — `const data k { t: (Text, Word) = ("hi", 1) }` gives `Tuple[Str("hi"), Int(1)]`, and `(Word, Text)` puts it second |
| 2 | `Text` is admissible in `const data` | **YES**. The two cases that produced nothing were simply UNREFERENCED, so never reached a chunk pool — not a `Text` restriction |
| 3 | a struct interns its type name before its fields | **YES** — `names = ["main","take","Zed","alpha","beta"]`, `type_name = 2`, `field_names_first = 3` |
| 4 | child-position strings are constructible | **YES**, including `Struct P{s:Str,n:Int}` |

**AND THE VACUITY CHECK, APPLIED BEFORE writing the case list rather than after.** `Tuple[Str, Int]`
visits the string first under BOTH walks, so it proves nothing about interning ORDER — the same trap
four of the flattener's five cases fell into. The discriminating case needs two strings at different
depths:

```
const data k { t: ((Text, Word), Text) = (("aaa", 1), "bbb") }
  ->  Tuple[Tuple[Str("aaa"), Int(1)], Str("bbb")]
  breadth-first: outer, inner, "bbb", "aaa", 1   ->  pool "bbbaaa"
  depth-first:   outer, inner, "aaa", 1, "bbb"   ->  pool "aaabbb"
```

**Different `STRING_POOL` bytes, so the coupling is observable** and the slice can start at
`STATIC_STR` as planned rather than falling back to `STRUCT`.

**Two incidental grammar findings.** Chained tuple indexing `k.t.0.1` is NOT admitted ("expected
field name or tuple index after '.'"); reference a nested tuple by passing it to a function instead.
And an unreferenced `const data` field never reaches a chunk's constant pool at all, which is why
two probe cases looked like `Text` failures and were not.

~~**THE UNMEASURED ASSUMPTIONS IN THIS DESIGN, listed so they are probed rather than built on.**~~ The
flattener's "needs hand-built constant trees" error came from treating a reading-derived inference as
a measurement, and this design contains four more inferences of the same kind. Each is cheap to
settle and none has been:

| # | Assumption | Basis | Consequence if wrong |
|---|---|---|---|
| 1 | `const data k { t: (Text, Word) = ("hi", 1) }` compiles and yields `Tuple[StaticStr, Int]` | read from `const_value_from_literal_for_field`, which maps `(Literal::String, PrimType::Text)` and recurses through tuple initialisers | **the whole reason 13b's coupling is testable disappears** — a `StaticStr` only ever at root position means depth-first and breadth-first interning coincide, and the slice's central property becomes unobservable, exactly like the flattener's four vacuous cases |
| 2 | `Text` is admissible in a `const data` field at all | `PrimType::Text` exists and `flat_byte_size` treats it as flat at a 64-bit word | as above; `Text` may be a retired surface even though the type survives (the V0.1.x `text` DSL is gone) |
| 3 | A struct node interns `type_name`, THEN captures `field_names_first`, THEN each field fresh | read at `wire_schema.rs:412-442` | every struct whose type name is new is off by one, and only those — a corpus with familiar type names would hide it |
| 4 | A `STATIC_STR` can appear at a child position often enough to matter | follows from 1 | if it is rare, the case list needs constructing rather than sampling |

**Probe 1 first and let its answer size the slice.** If a string cannot sit inside a composite, the
`STATIC_STR` step buys almost nothing on its own and the decomposition should start at `STRUCT`,
where `STRUCT_AUX` and the contiguous field-name run make the coupling observable regardless — that
path is already measured (`STRUCT_AUX` is 1 at depth 1 and 2 at depth 2).

~~**A smaller thing to fix on the next touch**: `emit_pool_bytes_from_bout` guards with
`n > bin_capacity()`.~~ **DONE 2026-08-11.** `bout` has its own `bout_capacity()`, and the guard
returns its own code `-255` rather than sharing slice 4's `-201` — two guards behind one code leave a
caller unable to say which buffer overflowed, and these are reached from different directions.

**Both guards are UNREACHABLE BY CONSTRUCTION and were kept anyway**, which is worth stating because
it looks like dead code. `nm.ocur` is the sum of the EMITTED names' lengths, bounded by the sum of
all input lengths, which `intern_run` already refuses above `bin_capacity()`. The guard exists so the
emitter does not depend on a caller's invariant. It correspondingly has **no negative test**: nothing
this corpus can build reaches it, and a test asserting an unreachable code would be theatre rather
than evidence. That is the honest counterpart to the vacuity rule — a control that cannot fire is
worthless, and so is a test written to make an unfirable control look covered.

**A LATENT DEFECT IN 13b-i, FOUND BY READING IT BACK (2026-08-11).** The counting pass writes
throwaway `CONSTS` records at `fx_scratch()` = 32768, and `fx_emit_names`/`fx_emit_pool` run it
**while an artifact is already seeded in `wire.bytes`**. Nothing stopped those records landing inside
a live artifact once one reached 32768 bytes.

**It cannot fire on the current corpus**, whose artifacts run to about a kilobyte — which is the
reason it needed a guard rather than a note. A hazard that the present tests cannot reach is one
that ships. `wire.len` carries the seeded length, so the check costs one comparison and returns
`-252`.

That is the third defect this arc that reading found and a green suite could not: the unvalidated
node count, a guard placed where its own test could not reach it, and now this. **The common shape
is that all three are about inputs the corpus does not produce** — which is the same lesson as
"real compiler output is a strong oracle for volume and a weak one for variety", arriving from the
other direction.

#### DONE (2026-08-15): the driver is wired to a module, and the staging half was never needed

**Read this before the section below it, which specified the increment and is now history.**

`wire_names_via_kel` takes a `Module` and builds its own input with `selfhost::module_input`.
Before this it accepted a pre-built blob and opened with `let _ = module;` — the module was in the
signature and unused, and the only producer of the blob was a Rust function in the test harness. A
path that cannot be driven from a `Module` is not a compile path, however byte-identical its output.
Byte identity against the reference is unchanged.

**THE TWO HALVES WERE NOT ONE INCREMENT, AND THE COUPLING CAME FROM THE WRONG FIGURE.** The section
below states that the producer and the residency staging "are the same increment, and doing either
alone is wasted". That followed from sizing the problem at 395,804 names. Measured across all ten
stages, the worst case is `parse` at **627 names from a 33,395-byte blob**, against caps of 1024 and
49,152 — **61% and 68%**. No stage in the corpus requires staging at all, and the conclusion is now
an assertion (`every_stage_fits_the_driver_caps_with_margin`) so a stage that grows past either bound
fails with the number rather than surfacing as an `Unsupported` at some later call site.

**The producer was ALREADY self-hosted** and this section did not say so: `mi_chunk_names`,
`mi_enum_names`, `mi_slot_names` and `mi_const_nodes` in `wire.kel` produce the interning sequence
from the blob, joined by `mi_join`. What was host-side was the ENCODER, not the producer.

**`wire.kel` was already in `read_stage`.** This document and `src/selfhost/mod.rs`'s module comment
both still said it was "deliberately absent" while the entry sat fifty lines below that comment. The
comment is corrected; a file contradicting itself is worse than one that omits the detail.

**What the name count is, and is not.** `module_input` returns the blob and an UPPER BOUND on the
names interned, not the record count. The reference dedups, and by how much is order-dependent
because `Names::intern_fresh` records its entry so a later `intern` can share it. Reproducing the
exact count host-side would mean replicating the reference's interning ORDER — a second model of the
thing under test. The bound is exact on all ten stages and demonstrably loose on an enum constant
(9 derived against 4 emitted), and both facts are pinned. The direction is the safe one: the previous
caller passed `interner_input(&module).len()`, which omits the data-slot contributor and reported
**252 for `parse` where the module interns 627** — an under-count feeding a cap check whose whole
purpose is to refuse a module that would overrun the interner.

#### THE ACTUAL NEXT INCREMENT: wire the driver to a module, not to a model (SUPERSEDED, see above)

With four of five values computed and the coverage matrix at 19 REAL / 1 DERIVE, what remains is
**not another emitter slice**. It is the step this document has called "wiring, not invention" since
step 6 closed, and it is now the only thing left of any size.

**What is still modelled.** The interning SEQUENCE — chunk names, then enum-layout names, then the
constant tree's names — is produced by Rust functions in the test file (`interner_input`,
`preorder_13b`, `chunk_inputs`), guarded by `assert_no_other_contributors` so it cannot silently
under-generate. The ORDER is measured and recorded; what is absent is a Keleusma-side producer of it.

**Why it is a different KIND of work.** Every slice so far took input the host had already decoded
and made Keleusma compute a value from it. This one needs the MODULE itself to reach Keleusma —
chunk names as bytes, enum layouts, the constant forest with its names inline — which means defining
a module-input encoding in shared data and, eventually, having `codegen.kel` produce it directly.
`wire.kel` is still deliberately absent from `read_stage`; this is the increment that changes that.

**Sizing, honestly.** The order is known and the emitters are done, so there is no discovery left in
the format. The work is an input encoding, a producer, and the residency staging that a real stage's
395,804 names force — which is the same batching problem the scan note above defers to. **Those two
are the same increment, and doing either alone is wasted.**

#### THE CHILD-POSITION SLICE, SPECIFIED BEFORE IT IS BUILT (2026-08-14)

Roots are done: names (slice 14c) and the six-word node table (slice 14d). What remains of the
constant contributor is the nesting, and it is **more intricate than the root case in three separate
ways**. Each was verified against `preorder_13b` rather than inferred, and each is a way to produce a
plausible artifact that is wrong.

**1. Two orders are in play at once, and they are not the same order.** The name sequence follows a
DEPTH-FIRST preorder, because that is the order `go()` descends. The record emission follows a
BREADTH-FIRST queue, which is what `fq` holds. A walk that used one order for both would agree with
the reference on every single-level constant and diverge at depth two, which is exactly where the
corpus has no coverage to catch it.

**2. `STRUCT` and `ENUM` intern in DIFFERENT MODES, and the difference is load-bearing.**

| tag | names interned | modes |
|---|---|---|
| `STRUCT` | type name, then every field name | type name DEDUP, **each field name FRESH** |
| `ENUM` | type name, then the variant name | **both DEDUP** |

The struct's field run is fresh because the layout addresses a field by `first + i`, so the run has
to stay contiguous and a dedup hit would break the contiguity. Nothing addresses a variant that way,
so the enum has no contiguity to protect. **A single "composite interns its names" rule would be
wrong for one of the two**, and would only show up where a name repeats.

**3. Field NAMES are not children; field VALUES are.** The child count and the name count come from
the same `fields` list and are unrelated quantities. Writing one where the other belongs produces a
node table whose subtree sizes are self-consistent and wrong.

**The corpus reaches nesting only through `const data`**, which `assert_no_other_contributors`
refuses because it introduces a data layout. So the child-position cases cannot extend the slice-14
table in place; they belong with `FX_CASES`, which already carries `str-in-tuple`,
`two-strings-depth-2`, `one-struct` and the enum cases, and whose model (`fx_input`) already covers
the constant contributor.

**The must-fire this slice needs** is a case with a name repeated across a struct field run, where
dedup and fresh give different answers. `two-strings-depth-2` discriminates depth; it does not
discriminate mode.

#### `read_stage` IS NOT A SMALL INDEPENDENT ITEM (checked 2026-08-14)

`src/selfhost/mod.rs` states the criterion precisely: `wire.kel` "joins the stage table when it
produces bytes rather than a checksum". **It now produces bytes**, byte-identically, for whole
artifacts. So the criterion reads as met and the item reads as a one-line change.

**It is not.** `read_stage` feeds the DRIVER, and the driver still has no role for `wire.kel`: the
other ten sources are pipeline stages that take a source and produce a stage output, while
`wire.kel` is an emitter library driven by commands from a host that already holds a module. Adding
it to the table without a driver that calls it would record a capability the system does not have.

**The item is therefore blocked behind "wire the driver to a module", not beside it.** Checked so
that a later session does not spend an increment rediscovering it as an easy win.

#### THE JOIN COVERS TEN STAGES, AND THE DEDUP BRANCH COVERS NONE (2026-08-15)

All ten stage sources now emit `NAMES` and `STRING_POOL` byte-identically through `mi_join`, pinned
by `the_join_holds_across_every_stage`. They passed on the first run, so the increment's substance is
the measurement of what that is worth.

**NINE OF THE TEN REACH NO NEW MAXIMUM.** `parse` is the largest in every dimension measured --
chunks (94), enum names (158), slot runs (375), constant names, constant nodes (815), constant depth
-- and nothing else exceeds it anywhere. The widening is a REGRESSION NET over nine real shapes, not
additional scale. The dominance is asserted rather than described, so a stage growing past `parse`
reports itself instead of quietly making the test worth more. What it genuinely adds is nine
ZERO-ENUM modules: `parse` is the only stage with enum layouts, so that path had no real module
behind it in the join.

**THE DEDUP BRANCH HAS NO REAL-MODULE COVERAGE, AND THAT BEARS ON THE CAP'S JUSTIFICATION.**
Making `nm_find` report "not found" unconditionally leaves all ten stages byte-identical. Every
dedup-mode name in every stage is distinct, so the matching branch has never been taken by real
input.

That is not a curiosity. `nm_find` is the quadratic scan whose cost is the stated reason the name
count is capped at all -- the raise from 256 to 1024 multiplies its static bound by sixteen -- and
the branch that cost pays for is exercised only by synthetic cases. **The cap is priced on a path no
stage reaches.**

**Reading the counts would not have established it.** Input and output name counts being equal is
consistent with dedup firing and finding nothing to merge; only disabling the branch and observing no
change distinguishes "never collides" from "collides and is handled". The counts suggested it, the
mutation established it, and the encoded proxy is the asserted equality between the blob's input name
count and the `NAMES` record count.

**TWO FURTHER GAPS PINNED AS ASSERTIONS THAT A LIMITATION STILL HOLDS**: no stage contributes a
constant-interned name, and no stage nests a constant past depth one. The constant contributor's name
and child-position paths are exercised by `FX_CASES` and by nothing real. Both are written so that
firing means coverage was GAINED, and the response is to record that rather than restore the zero.

#### A GUARD PINNED TO A LITERAL, CAUGHT BY CI AND NOT BY THE BENCH (2026-08-15)

`the_driver_refuses_more_names_than_one_call_can_intern` spelled `257` against a cap of 256. The
ceiling raise took the cap to 1024, so the case silently stopped being over the bound: the driver
accepted where the test demanded a refusal. **It was the only thing in the suite that caught the
raise's loose end.**

It reached CI rather than the bench because it sits behind the `self-host` feature, and neither
`cargo test --workspace` nor `cargo test --features compile` enables it. Both were run and both were
green. The gate is `cargo nextest run --profile ci` across a five-entry feature matrix
(`--workspace`, `self-host`, `signatures`, `--no-default-features`, and the `keleusma-wire` pair),
and a default-feature run is an APPROXIMATION of it -- the thing the workflow already says not to
substitute. Every cap-pinned test now derives from a named `NAME_CAP` rather than a literal.

#### THE CEILING IS RAISED, AND FOUR OF ITS FIVE PREMISES WERE WRONG (2026-08-15)

**Done.** `parse.kel` -- 627 names, a 33,395-byte module blob -- now emits `NAMES` and
`STRING_POOL` byte-identically to `encode_aux_body`, pinned by
`the_join_holds_on_the_largest_real_stage` in `tests/selfhost_wire.rs`. The section below records
what the raise actually cost, because the estimate under it was wrong in four separate ways and each
one pointed the work somewhere it did not need to go.

**Its fifth premise was right, and it is the one that mattered**: "it is not two constants" held
exactly as written, and the loop it warned about is the loop that trapped. A document can be wrong
about every figure and still correct about the hazard.

**1. "The hard ceiling is 512" was a guard naming the wrong buffer.** `emit_name_records_from_nout`
reads `wire.nout`. It was bounded by `fin_capacity()`, copied from its sibling
`emit_name_records`, which genuinely reads `wire.fin`. At 1024 words and two fields per record that
guard refuses 512 names -- a number with no relationship to the buffer the function touches. It is
now bounded by `nout_capacity()` with its own code `-256`, because two guards sharing `-202` leave a
caller unable to say which buffer overflowed.

**2. The binding ceiling was never the name count.** Measured across all ten stages:

| stage | NAMES records | pool bytes | module blob |
|---|---|---|---|
| `parse` | **627** | 7,680 | **33,395** |
| `codegen` | 152 | 1,840 | **21,225** |
| `reconstruct` | 96 | 1,120 | **8,849** |
| `lexer` | 31 | 248 | 7,963 |

`bin` was 8,192 bytes, so **three stages did not fit, not one**, and `lexer` sat at 97% of it. The
name count was the ceiling the plan named and the third-largest of the four that bind.

**3. "`parse`'s 304,432-byte artifact does not fit the single-window join harness" is true and
irrelevant.** The join writes exactly two regions, and what places them is the DIRECTORY, not the
artifact's total size. A directory holding only `NAMES` and `STRING_POOL` is 12,840 bytes for
`parse`, comfortably inside the existing 65,536-byte window. **No windowed join variant and no second
harness were built**; the decision the plan framed as a fork did not have to be taken.

**4. "`NIN_SLOT` and `NIN_CAPACITY` are the only constants that move" is true of the harness's slot
arithmetic and false as a sizing claim.** What moved: `nm_max_names` 256 -> 1024, `nin` and `nout`
1024 -> 2048 words, `bin` 8,192 -> 49,152 bytes, `bout` 8,192 -> 16,384, twelve name-count loop
limits 256 -> 1024, `nm_find` 512 -> 1024, `emit_pool_bytes` 8,192 -> 49,152,
`emit_pool_bytes_from_bout` 8,192 -> 16,384, and two new arrays.

**The map and the offsets are now their own arrays.** They were the upper two thirds of `nout`,
reached through `nm_map_base()` and `nm_off_base()`, tiling 1024 words exactly with no slack. The old
comment named that as a fragility and offered two repairs; the second was taken, so raising the cap
again means changing three array lengths and no base constants.

#### THE TRAP THE GOAL NAMED, OBSERVED (2026-08-15)

**A loop left behind aborts on exactly the inputs the raise was for**, and one was left behind.
`emit_pool_bytes` guards `n > bin_capacity()` and then loops `limit 8192`. Raising `bin` to 49,152
left the guard admitting six times what the loop would run, so three whole-artifact tests died with
`LoopLimitExceeded` -- past a guard that had already said yes. The limit is now `bin_capacity()`'s
value, with the coupling stated where a later editor will meet it.

**Enumerating by the literal `256` was not sufficient.** `nm_find` was at `limit 512`, so a sweep for
`limit 256` missed the one loop whose bound is quadratic in the cap. The enumeration has to be by
WHAT BOUNDS THE LOOP: two loops at `limit 256` are bounded by a name's BYTE LENGTH, not by a name
count, and raising those would have inflated the innermost loop of three for nothing.

#### TWO DEFECTS THE CONTROL FOUND THAT THE RAISE DID NOT CAUSE (2026-08-15)

Both were invisible to a green suite, and both needed real stage input to reach.

**`mi_chunk_names` ignored `nm.mode`.** It wrote its output copy through `put_u32` directly rather
than through `mi_pair`, which honours the mode -- the one producer on that path that did not.
`mi_join` runs the walk in SILENT mode precisely because the artifact is already in `wire.bytes`, so
the copy landed on the artifact at byte `8 * i`: the seventh chunk overwrote the directory at byte
48, and every later chunk overwrote a region payload. **It needs seven chunks to fire and the join
corpus topped out at three.** `parse` has 94 and refused with `-233`, which reads as "no NAMES
region" and meant "the directory that named it has been overwritten with name lengths".

**`mi_join` summed its three emitter results.** A sum is not a conjunction: `-202` from the names
emitter plus 7,680 from the pool emitter reported **7,478 -- positive, therefore success** -- with
the `NAMES` region left entirely zero. Each result is now checked separately. Any caller of the join
before this fix could have accepted a half-written artifact as a good one.

#### WHAT THE RAISE COST, MEASURED (2026-08-15)

**`shared_data_bytes` 155,704 -> 237,624, up 52.6%.** The composition is exact and reproduces the
figure: 13,319 words at 8 bytes (106,552) plus three byte arrays (65,536 + 49,152 + 16,384).

**The WCET bound rises faster than the memory does, and that is the real price.** `nm_find` scans
every name interned so far with no early exit -- a total language has none -- so the interning
phase's static worst case is quadratic in `nm_max_names`. Going 256 -> 1024 multiplies that bound by
**sixteen**, from 65,536 name comparisons to 1,048,576. It costs nothing on real input, where the
scan runs to `cnt`; it is the BOUND that moves, and the bound is what this project sells.

**`-255` became live and has no negative test.** `emit_pool_bytes_from_bout`'s overflow guard was
argued unreachable because `intern_run` refuses above `bin_capacity()` -- sound only while `bin` and
`bout` were both 8,192. They are now 49,152 and 16,384, so an input whose names total between the two
passes the input guard and overflows the output buffer. Reaching it needs a module with more than
16 KB of distinct name bytes, which the corpus cannot build (`parse` is the largest at 7,680), so
this is a coverage GAP rather than a justified omission.

#### WHAT IS LEFT OF THE CEILING, AND WHY IT IS NOT A ONE-LINE RAISE (2026-08-14)

> **SUPERSEDED 2026-08-15** by the four sections above. Kept for the reasoning it carries and because
> three of its five premises were wrong in instructive ways. The measurements in it are stale; the
> ones above were taken against the code.


The data-slot contributor is built, so the producer's sequence now matches the reference's `NAMES` on
every corpus case tested. **One thing remains: `parse` needs 627 names and the hard ceiling is 512.**

`nin` holds two words per name in a 1024-word array, and `nm_max_names` is 256. Raising the ceiling
looks like two constants and is not:

1. **Every `for .. limit 256` in the interner and the producer must rise with it.** `for .. limit`
   TRAPS on entry when the runtime range exceeds the static cap, so a loop left at 256 does not
   degrade -- it aborts. A missed loop is a runtime trap on exactly the inputs the raise was for.
2. **Verifying it needs a module with more than 256 names, and the only one in the corpus is
   `parse`**, whose artifact is 304,432 bytes. The join test asserts `total < CAPACITY` because it
   uses the single-window path, so the verifying case does not fit the harness that would verify it.

**So the raise and the staged path are the same decision after all** -- not for the reason the
395,804 figure suggested, but because the only input that exercises either is one the current test
path cannot hold. Either the join grows a windowed variant, or the ceiling rises and a new harness
carries `parse`.

**Growing the buffer is narrow in SLOT terms**, which is worth recording so the next attempt does not
over-estimate it: `nin` is followed by `nout` and `bout`, and the test harness addresses none of them
by slot. `NIN_SLOT` and `NIN_CAPACITY` are the only constants that move.

#### RESIDENCY STAGING WAS SIZED FROM THE WRONG REGION (measured 2026-08-14)

This document, the roadmap and my own goal statement all said residency staging is forced by **"a
real stage's 395,804 names"**. Measured across all ten stages, that figure describes **no name count
at all**.

| stage | `NAMES` records | of which DATA_SLOTS | largest region |
|---|---|---|---|
| `parse` | **627** | 375 | `CONSTS`, 34,782 units |
| `codegen` | 152 | 76 | `CONSTS`, 12,676 |
| `verify_structural` | 42 | 28 | `CONSTS`, 12,404 |
| `lexer` | 31 | 17 | `CONSTS`, 522 |
| `verify_datalayout` | 17 | 15 | `STRING_POOL`, 19 |

**The largest NAMES region in the corpus is 627 records.** The 395,804 figure is a REGION record
count, and the largest region is `CONSTS`. The number was carried from one context into another
where it was never true, and it made a two-and-a-half-times problem look like a fifteen-hundred-times
one.

**What actually remains, restated from the measurement.**

1. **The DATA_SLOTS name contributor is not in the producer at all.** It is the difference between
   the producer's sequence and the reference's `NAMES`: 252 against 627 for `parse`. This is the
   real gap, and it is a new contributor rather than a scaling problem.
2. **The sequence then exceeds one call, but barely.** The interner admits 256 names, and `nin`
   holds two words per name in a 1024-word array, so **512 is the hard ceiling before any staging**
   and 627 is past it. Two calls, or wider arrays, or both.

**Why the old framing would have produced the wrong design.** Staging for 395,804 names means
carrying an interner's dedup state across hundreds of batches with a pool far larger than `bin`,
which is a genuine architecture problem. Staging for 627 means two batches with the emitted pool
still resident. **The first design would have been built and then not needed.**

**The slot contributor's ORDER and SPELLING, measured rather than assumed.** For
`shared data s { alpha: Word, beta: Word }` with functions `zulu` and `main`, the reference `NAMES`
region is:

```
main, zulu, s.alpha, s.beta
```

So slot names **append after the chunk names**, and each is spelled `<block>.<field>` rather than the
bare field name. Both facts are needed to extend the producer and neither is guessable from the
declaration: a producer emitting `alpha` in declaration order would be wrong twice over, and the
first name would still dedup against nothing and look plausible.

**The remaining ceiling, stated precisely.** `nin` holds two words per name in a 1024-word array, so
512 names is the hard limit before any staging and `parse` needs 627. Two batches suffice if the host
carries `bout` and the running counts between calls and re-seeds them, because shared data is
re-seeded on every call. The emitted pool stays resident: at roughly 6 KB it fits `bout`'s 8192
bytes, which is what makes two batches enough and is the fact the old 395,804 framing hid.

**The pre-run-length-encoding state is where the large figure came from**, when `SHARED_LAYOUT` held
one record per array element and `lexer.kel` alone expanded to roughly 76,000 shared slots. Run-length
encoding took `SHARED_LAYOUT` to between one and nine records per stage. The figure outlived the
representation it described.

#### THE END-TO-END SLICE, AND THE ONE OBSTACLE IN IT (specified 2026-08-14)

The producer is complete: Keleusma derives the interning SEQUENCE and the constant node table from a
module blob, for chunk names, enum layouts with both intern modes, and the constant forest including
child positions. The Rust models `interner_input` and `preorder_13b` are no longer the source of
either.

**What is NOT yet closed is that the produced sequence has never been FED INTO the emitters.** The
producer is verified against the Rust models; the emitters are verified byte-identical against
`encode_aux_body`. Those are two chains, and nothing yet joins them. Until they are joined, "the
sequence is Keleusma's" and "the artifact is byte-identical" are true separately and unproven
together.

**The obstacle is one assumption inside the interner.** `nm_offsets` computes each name's byte
offset as a CUMULATIVE SUM of the lengths in `nin`, which is correct exactly when `bin` holds the
names concatenated with nothing between them. **The module blob interleaves a two-byte length prefix
before every name**, so the cumulative sum is wrong by two bytes per preceding name. Feeding the
producer's output straight into `intern_run` would read each name shifted, and the failure would look
like a corrupt pool rather than like an offset convention.

**The fix is additive rather than a modification, which is why it should be done in one pass by
someone with the room to verify it.**

1. The producer already knows where each name's bytes begin: `nm.icur + 2`, at the moment it reads
   the length. Record that into `wire.nout[nm_off_base() + k]` as it walks.
2. Add `intern_run_preoffset(n)`, identical to `intern_run` except that it does NOT call
   `nm_offsets`, since the offsets are already present. **Additive, not a change to `intern_run`** --
   the sequential path keeps its own offset computation and cannot regress.
3. One command that runs the producer, then `intern_run_preoffset`, then the existing
   `intern_emit_names` and `intern_emit_pool`, all within a single VM call, because shared data is
   re-seeded on every call and `nin` does not survive a return.
4. Compare the emitted `NAMES` and `STRING_POOL` regions against `encode_aux_body` for a real
   compiled module. **That comparison is the thing that joins the two chains.**

**Then, and only then, `read_stage`.** The driver produces no auxiliary body today; it runs pipeline
stages and nothing else. `wire.kel` joining the stage table is meaningful once the driver can emit
through it, and the step above is what makes that possible.

#### THE TWO DEDUP SCANS ARE DIFFERENT SCANS (settled 2026-08-14)

Two statements in this repository read as a contradiction, and they are not one. They name
**different sites**, and acting on the wrong reading either wastes an increment or undoes a
deliberate decision.

| site | bound | verdict |
|---|---|---|
| `intern_run`, `wire.kel` | `for j in 0..n limit 256`, batch-local | **Do not replace.** |
| the walk-nested scan through `NAMES` via `rec_u32`, once per interned name | unbounded at stage scale | **Measure, do not assume.** |

**The first is the one the standing trap protects**, and the arithmetic above settles it: a total
language has no early exit, so a 1024-slot table costs 1024 probes where the linear scan costs about
256 comparisons. The driver's inputs are capped at 256 because `nin`, `nout` and `bin` are sized for
a batch rather than for a stage. The table only wins past roughly a thousand entries.

**The second is the site the reference's 782-second lesson actually bears on.** It is nested inside
another walk and reads the `NAMES` region per interned name. ~~Raising the cap to a stage's 395,804
names is not a tuning change but the staged-batching problem, so the ordering stands: batching
first, index second.~~

> **CORRECTED 2026-08-15.** The cap was raised to 1024 and a stage's real name count is **627**, so
> there is no staged-batching problem to order against: the worst stage fits in one call at 61% of
> the cap. The "batching first, index second" ordering was derived from the 395,804 figure and
> ordered two pieces of work of which one turned out not to exist. The measurement is now an
> assertion — `every_stage_fits_the_driver_caps_with_margin` — so a stage that grows past the bound
> fails with the number rather than reviving this argument by surprise.
>
> **What is NOT settled**: the scan's cost at stage scale is still unmeasured, and the corpus cannot
> measure it, because making `nm_find` report "not found" unconditionally leaves all ten stages
> byte-identical. The scan is on a path no stage reaches. That is a fact about the corpus, not
> about the scan.

**The roadmap's Order-1 cell is stale on this point.** It lists "replacing a linear dedup scan" among
the remaining work, which was written before the 2026-08-11 analysis reversed it. Corrected there to
point here rather than restating the reasoning in two places.

#### TWO NEXT-INCREMENTS THAT DO NOT SURVIVE INSPECTION (2026-08-11)

Both were on the list. Neither is worth doing, and the reasons are worth more than the increments
would have been.

**1. Replacing the linear dedup scan is PREMATURE, and would make things worse at current sizes.**
The scan is recorded as "the shape that cost the reference 782 seconds", which is true — at 395,804
names. The arithmetic at the sizes this driver actually handles goes the other way:

| | linear scan | 1024-slot hash table |
|---|---|---|
| lookup at n = 256 | ~256 length comparisons | **1024 probes** |
| lookup at n = 395,804 | ~395,804 comparisons | 1024 probes |

**A total language has no early exit**, so `for p in 0..1024 limit 1024` runs all 1024 iterations
whether or not the slot is found on the first probe. The table only wins once n exceeds roughly a
thousand — and the driver's inputs are capped at **256 names**, because `nin`, `nout` and `bin` are
sized for a batch, not for a stage. Raising that cap to 395,804 is not a tuning change; it is the
staged-batching problem the residency measurement already governs.

So the ordering is: **batching first, index second.** Replacing the scan before the structures can
hold a real stage optimises a path nothing takes, and slows every path something does take.

**2. Computing the chunk record's NAME INDEX would be vacuous.** Slice 14 left it coming from the
reference row and flagged it for a later increment. Checking before writing: chunk names are the
FIRST entries of the interner's prefix, they are interned in order, and function names within a
module are distinct — so the interner's map satisfies `map[j] == j` for every chunk, always.

A driver that simply wrote the loop counter would produce a byte-identical artifact on every source
this corpus can construct. There is no case that separates the computed answer from the trivial one,
which makes the increment untestable rather than merely easy. **Caught before writing it, by asking
the question the last four vacuity controls trained me to ask** — the first time in this programme
that check has run early enough to cancel work rather than repair it.

**Suggested decomposition**, smallest first, since the whole thing is larger than any slice so far:

1. `STATIC_STR` alone — one intern per node, no side table, no contiguous run. Establishes the
   drive-the-interner mechanism.
2. `STRUCT` — adds `STRUCT_AUX` and the contiguous field-name run, which is where `intern_fresh`'s
   contiguity requirement finally has a consumer.
3. `ENUM` — adds `ENUM_AUX`, the discriminant, and the flag.

**The linear dedup scan becomes a real problem here and should be measured, not assumed.** Slice 12
scans a list of at most 256 names. This scans the `NAMES` region through `rec_u32`, once per
interned name, inside the walk. It is still correct and still small at these sizes, but it is now
nested inside another walk, and the reference's 782-second lesson was about exactly this shape.

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

~~The flattener therefore needs hand-built constant trees, oracled against `encode_aux_body` on a
constructed module, exactly as the slice-8 record kinds were oracled against the derive.~~

**THAT CONCLUSION WAS WRONG, AND IT WAS MY OWN. Corrected 2026-08-10.** The measurement above is
sound — the corpus really does contain 2,192 scalar nodes and no composite. The inference from it
was not: "the corpus cannot reach this" does not establish "no source can reach this", and I wrote
the second as though it followed from the first.

**Ordinary source reaches the composite path through `const data`.** There are three data
visibilities, not two. `shared` fields admit no initializer at all and `private` fields admit only
scalar ones (`compiler.rs:3199-3211`), which is what thirteen probes of tuple, array, struct,
nested-struct and enum-payload literals all ran into. **`const data` is the third**, and it is the
only caller of `const_value_from_literal_for_field` with no scalar guard. Referenced from a function
so the value reaches a chunk's pool rather than the compiler's `const_fields` map, it produces real
composite constants:

| source | artifact | `CONSTS` | `STRUCT_AUX` | `ENUM_AUX` | depth |
|---|---|---|---|---|---|
| `(Word, Word)` | 944 | 3 | 0 | 0 | 1 |
| `[Word; 3]` | 976 | 5 | 0 | 0 | 1 |
| `struct P { x, y }` | 1,072 | 3 | **1** | 0 | 1 |
| `struct P { q: Q, y }` | 1,112 | 4 | **2** | 0 | **2** |
| `[(Word, Word); 2]` | 1,112 | 8 | 0 | 0 | **2** |
| `enum E { A(Word) }` | 1,128 | 3 | 0 | **1** | 1 |

**So the flattener slice keeps the REAL oracle** — `encode_aux_body` on a genuinely compiled module —
rather than dropping to hand-built trees, and every artifact is about a kilobyte against a
65,536-byte buffer. The breadth-first walk, the child numbering after the roots, and the
forward-ordering invariant are all exercised by depth-2 cases.

**Two of the six DERIVE rows in the coverage matrix below are now upgradable.** `STRUCT_AUX` and
`ENUM_AUX` are reachable from real compiler output, so their emitter tests can be re-oracled from
constructed values to real ones — taking the split from 14 REAL / 6 DERIVE to 16 / 4. **That work is
NOT done**, and the matrix still reads 14/6 because that is what the tests currently do. Recording
the opportunity, not claiming the upgrade.

**The method lesson, which is the same one twice in one day.** The interner slice found that a
constructed SOURCE beats a hand-built input, because it keeps the reference encoder as the oracle.
Applying that same question here overturned a conclusion this document had already committed to.
**A "the corpus cannot reach X" measurement is a fact about the corpus; the reachability of X is a
separate question that has to be asked separately.** Three earlier findings in this arc are phrased
the same way and are worth re-asking rather than trusting:

- the six record kinds emitted empty by every stage (asked and partly answered here — two of them
  are reachable);
- the second interning mode (asked in slice 12 — reachable, via two enums sharing a variant name);
- generics and floats, recorded as "the deferred tail".

**Incidental:** the repeat-array form `[7; 64]` is not admitted (`expected RBracket`), so a wide
constant array has to be written out elementwise.

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

### Emitter coverage matrix: which oracle backs which region kind

Written after correcting an over-claim of my own, and kept because a roll-up sentence dropped a
qualifier that every individual slice had recorded correctly. **A table cannot drop a qualifier.**

**REAL** means driven by real compiler output — a strong oracle for volume, a weak one for variety.
**DERIVE** means constructed values checked against `#[derive(WireRecord)]`'s `write_record` — the
reverse. Both are legitimate; conflating them is not.

> **REACHABILITY SWEEP, 2026-08-10: FIVE OF THE SIX DERIVE ROWS ARE UPGRADABLE.**
>
> Every DERIVE row was justified by "emitted empty by every stage". That is a fact about the corpus
> and says nothing about whether a source can reach the kind. Having been wrong about exactly that
> twice in one day, I asked it of all six rather than of the one in front of me.
>
> | Region kind | Reachable from source? | Smallest trigger found | Artifact |
> |---|---|---|---|
> | `STRUCT_AUX` | **YES** | `const data k { p: P = P { .. } }`, referenced | 1,072 B |
> | `ENUM_AUX` | **YES** | `const data k { e: E = E::A(7) }`, referenced | 1,128 B |
> | `NATIVES` | **YES** | `use beep` | 936 B |
> | `NATIVE_RETURNS` | **YES** | `use beep` | 936 B |
> | `PRIVATE_COMPOSITE` | **YES** | `private data d { p: P }`, written | 1,168 B |
> | `STRUCT_TEMPLATES` | **NO, structurally, in this configuration** | — | — |
>
> **`STRUCT_TEMPLATES` is settled by construction rather than by sampling, which is why it is a
> stronger statement than the other five.** The template is added only on the BOXED
> struct-construction path (`compiler.rs:9479`), taken when `flat_alloc_bytes` returns `None`. That
> has two routes and both are closed here:
>
> - **A non-flat type.** `flat_byte_size` (`value_layout.rs:493`) returns `None` in exactly one
>   case: a `Text` field where `word_bytes` is below the host pointer width — a **narrow-word
>   build**. `tests/selfhost_wire.rs` is gated out of every narrow-word configuration and `wire.kel`
>   declares `require word >= 64`, so this cannot occur where the wire tests run. Unknown names are
>   rejected by the type checker and generics are monomorphized away before codegen.
> - **A flat size above the sixteen-bit operand**, i.e. a struct over 65,535 bytes. Constructed and
>   **rejected by the typed operand-stack verifier**, so no module results.
>
> So `STRUCT_TEMPLATES` stays DERIVE for a stated structural reason, not for want of a corpus case.
> That is a better justification than the one it had.
>
> **The matrix below still reads 14 REAL / 6 DERIVE, and that is deliberate.** Upgrading a row means
> rewriting its emitter test to use these sources; none of that is done. The achievable split is
> **19 REAL / 1 DERIVE**. Recording the opportunity and its size — not the upgrade.
>
> **Two incidental rejections worth not rediscovering.** A path-qualified `use audio::beep(Word) ->
> Word` interns the name as `audio::beep`, so a bare `beep(1)` call fails with "undefined function".
> And `private data d { xs: [Word; 3] }` produces **no** `PRIVATE_COMPOSITE` record: the table holds
> composite slots, and an array of scalars is not one. An array of structs produces one per element.

| Region kind | Slice | Oracle | Note |
|---|---|---|---|
| `HEADER` | 2 | REAL | first schema emitter |
| `CHUNKS` | 3 | REAL | widest record, 14 fields; forces batching |
| `PARAM_TYPES` | 4 | REAL | byte pool; pad residues 0, 3, 4, 5, 7 |
| `STRING_POOL` | 5 | REAL | 807 batches on `lexer` |
| `NAMES` | 5 | REAL | 774 batches on `lexer` |
| `DATA_SLOTS` | 6 | REAL | **capped at 2048 records**, stated in the test |
| `SHARED_LAYOUT` | 6 | REAL | capped likewise |
| `SHAPES` | 7 | REAL | |
| `SIGNATURES` | 7 | REAL | |
| `ENUM_VARIANTS` | 7 | REAL | only `parse` populates it |
| `ENUM_LAYOUTS` | 7 | REAL | only `parse` populates it |
| `DATA_INIT` | 7 | REAL | |
| `CONSTS` | 7 | REAL | **scalars only** — no composite constant exists in the corpus |
| `DEBUG_POOL` | 9 | REAL | needs `emit_debug`; the twentieth kind |
| `STRUCT_AUX` | 8, **13b-ii** | **REAL** | driven by a real `const data` struct constant |
| `ENUM_AUX` | 8, **13b-iii** | **REAL** | driven by a real `const data` enum constant |
| `STRUCT_TEMPLATES` | 8 | DERIVE | **structurally unreachable here**, see below |
| `PRIVATE_COMPOSITE` | 8, **upgrade** | **REAL** | driven by a written private composite field |
| `NATIVES` | 8, **upgrade** | **REAL** | driven by a bare `use beep` |
| `NATIVE_RETURNS` | 8, **upgrade** | **REAL** | driven by a `use` with a signature |

**19 REAL / 1 DERIVE as of 2026-08-11**, up from 14 / 6, which is the split the
reachability sweep predicted was achievable. The two upgrades are
bookkeeping on work already done rather than new tests: slice 13b's differential
compiles real modules whose constant trees contain a struct and an enum, and
compares the whole artifact byte for byte — so `STRUCT_AUX` and `ENUM_AUX` are
now driven by real compiler output, not by constructed values checked against
the derive.

**THE UPGRADE COST FOUR DEFECTS IN THE TEST HARNESS, none of them in the emitters.** Driving these
regions from real sources for the first time found: `emit_in_region` missing arms for six kinds
(refused with `-222`, correctly); `rows_for_kind`'s eight-byte stride list missing four kinds, so
`records()` errored and the caller emitted a region with ZERO rows — a wrong artifact rather than a
refusal; no decoders for `NATIVES`, `NATIVE_RETURNS` or `PRIVATE_COMPOSITE`; and `DATA_SLOTS`,
`SHARED_LAYOUT` and `STRUCT_AUX` needing raw decoding, because they carry trailing reserved bytes the
emitters take as separate inputs and a struct-shaped decode returns fewer fields than the emitter
consumes.

**Every one of those was a BY-NAME ENUMERATION in the test harness** — a `match` listing the kinds
someone had needed so far. That is the sixth through ninth instance of the defect this project has
now catalogued, and the first time the enumerations were in test code rather than in a build script
or an ignore file. The failure mode is identical: silent, and reading as success.

**A region-level diff located all four in one sitting.** Comparing whole artifacts reports "2,182
bytes differ"; comparing region by region names the kind. That diagnostic is now permanent in the
test rather than scaffolding, because the next person will need it for the same reason.

**Recording it late is itself the point.** The upgrade was earned when 13b-ii and
13b-iii landed, and the matrix went on saying DERIVE because nobody revisited it.
A coverage table is exactly the kind of document that decays silently: it is read
to decide what needs work, so a stale row misdirects effort rather than merely
being wrong. The remaining three DERIVE rows are **measured reachable** and
awaiting a test that drives them; `STRUCT_TEMPLATES` is the one that is not, for
the structural reason recorded above — its only non-flat type is `Text` under a
narrow word, and this suite is gated out of narrow-word builds.

**Fourteen REAL, six DERIVE.** Two REAL entries carry stated limits worth remembering: the per-slot
tables are compared over their first 2048 records, and `CONSTS` sees scalars only, so the
flattener's composite path is untested by any of this.

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
