# One wire slot per array

> **Navigation** [Decisions](./README.md) | [Wire format specification](../spec/WIRE_FORMAT.md) | [Wire format self-hosting plan](./WIRE_FORMAT_SELFHOST_PLAN.md)

## The ruling and its provenance

The session that commissioned this document reported that the operator ruled first-hand on
2026-10-30 to design now a change in which an array occupies one wire slot plus a length,
instead of one slot and one interned name per element. This document received that ruling
through the commissioning session and does not reproduce the operator's own wording. The
ruling authorizes a design. It does not authorize a wire-format change, and every item that
would need that separate authorization is listed under [Decisions that remain the
operator's](#decisions-that-remain-the-operators).

The measurement that raised the question is recorded in
[the self-hosting plan](./WIRE_FORMAT_SELFHOST_PLAN.md) under the paragraph that begins "A
separate question this raises, for the operator and not for the loop". It reports that the
21 KB `lexer.kel` of 2026-08-09 produced a 16,124,636-byte artifact, 99.94 percent of it in
the auxiliary body, because every array element carried its own data slot record, shared
layout record, name record and pool string.

No code is changed by this document. `BYTECODE_VERSION` remains 2 and the instruction set
remains at 66 opcodes.

## Summary

1. **The premise is largely superseded on the wire, and this is a measured fact.** Three
   changes that landed after the measurement removed most of the per-element wire cost. Commit
   `96139a76` of 2026-08-12 gave every element of an array the array's name, so the name is
   interned once. Commit `32af6da5` of the same day run-length encoded `DATA_SLOTS`, and commit
   `4c2598d1` did the same for `SHARED_LAYOUT`. Commit `b6d9cec3` of 2026-08-17 elided a
   wholly zero private initialiser pool. A fresh measurement on the current tree gives
   `lexer.kel` at 32,497 source bytes and a 28,212-byte artifact, against 16,124,636 bytes in
   the original measurement. For a shared array of either `Word` or `Byte`, the artifact size
   is now independent of the element count.
2. **Four element-proportional costs remain, and each was measured or read directly.** A
   private array whose zero is not `ConstValue::Int(0)`, which covers `Byte`, `bool`, `Fixed`,
   `Text` and every composite element, still carries one sixteen-byte constant record per
   element. A private array of composites still carries one eight-byte `PRIVATE_COMPOSITE`
   record per element. Every private element occupies one 32-byte `Value` slot in the arena
   whatever its element type. The owned decoder, the hot-swap schema hash and the self-hosted
   emitter all iterate logical element slots.
3. **A latent defect surfaced and is recorded as a fact with an unverified consequence.** The
   reference compiler silently omits a private composite element from the placement table when
   its slot index exceeds 65,535, because the table's slot field is a `u16`. A probe declaring
   `[P; 70000]` emitted 65,536 placement records for 70,000 elements without a diagnostic.
4. **Two encodings are proposed, and the choice between them is the operator's.** The first
   extends the existing run records so that every remaining per-element cost becomes per-run.
   It keeps the stage corpus byte-for-byte the same size. The second replaces four regions with
   one record per declared field, which is the literal form of the ruling and the natural base
   for packed private storage. It grows the stage corpus by an estimated 4.1 percent.
5. **The worst-case memory benefit lies in storage, not in the wire.** Packing private arrays
   at element width would reduce the private data segment of the fifteen stage modules from
   4,271,552 bytes to an estimated 1,091,216 bytes, about 74.5 percent less. That figure is an
   inference from measured slot counts, and it requires a change to runtime storage and to the
   declared private data size that is distinct from the wire encoding.
6. **`BYTECODE_VERSION` would not have to change under the operator policy of 2026-09-01**,
   which freezes it at 2 and relies on the per-release `FORMAT_FINGERPRINT`. Within one release
   cycle the fingerprint does not move, so the change would need either a fingerprint roll at
   landing or a structural refusal of the old regions, and that choice is the operator's.

## The current encoding, located

### The source of per-element slots

| Concern | Location | What it does |
|---|---|---|
| Slot expansion | `src/compiler.rs:3608` | `slots_for_data_type` gives an array field one logical slot per element, multiplied through nested arrays |
| One name per array | `src/compiler.rs:3635` to `src/compiler.rs:3664` | Every element slot carries the array's name, so `Names::intern` stores it once |
| Slot count function | `src/compiler.rs:5157` | Product of element counts, capped at `MAX_DATA_ADDR` |
| Private initialisers | `src/compiler.rs:5172`, `src/compiler.rs:5204` | One `ConstValue` per private element, `Int(0)` for `Word`, `Byte(0)`, `Bool(false)`, `Fixed(0)`, `Float(0.0)`, and `Unit` for `Text` and composites |
| Private composite placement | `src/compiler.rs:3881` to `src/compiler.rs:3934` | One `PrivateCompositeSlot` per element of a private array of composites, silently skipped when `u16::try_from(slot)` fails |
| Declared private size | `src/compiler.rs:4159` | `private_count * VALUE_SLOT_SIZE_BYTES`, so 32 bytes per private element |
| Array access | `src/compiler.rs:885`, `src/compiler.rs:5564`, `src/compiler.rs:5748`, `src/compiler.rs:11459` | Arrays are reached only through `GetDataIndexed` and `SetDataIndexed` with the array's first slot and its flat element count |

### The in-memory model

| Type | Location | Granularity |
|---|---|---|
| `DataSlot` | `src/bytecode.rs:3346` | One per logical slot, each holding an owned `String` name |
| `SharedSlotLayout` | `src/bytecode.rs:3388` | One per shared logical slot |
| `PrivateCompositeSlot` | `src/bytecode.rs:3427` | One per private composite element, with `slot: u16` |
| `DataLayout` | `src/bytecode.rs:3445` | Holds the four per-slot vectors above |
| `compute_typed_schema_hash` | `src/bytecode.rs:4049` | Builds one canonical line per logical slot into a buffer, then takes its cyclic redundancy check |

### The wire records

| Record | Location | Granularity on the wire |
|---|---|---|
| `DataSlotRecord` | `src/wire_schema.rs:1963` | One per run of consecutive slots sharing a name and visibility, eight bytes, `run: u16` |
| `SharedSlotRecord` | `src/wire_schema.rs:2022` | One per run of equal kind, equal length and constant stride, sixteen bytes, `run: u16` |
| `PrivateCompositeRecord` | `src/wire_schema.rs:2063` | One per private composite element, eight bytes, `slot: u16` |
| `DataInitRecord` | `src/wire_schema.rs:2074` | One range into `CONSTS`, or the `ABSENT` sentinel when elided |
| Elision predicate | `src/wire_schema.rs:2097` | Elides only when every private initialiser is `ConstValue::Int(0)` |
| Encoder | `src/wire_schema.rs:2187` | `SchemaBuilder::add_data_layout` forms the runs and chunks them at `u16::MAX` |
| Borrowed view | `src/wire_schema.rs:2350` | `DataLayoutTable::parse` validates the run tables in place |
| Shared lookup | `src/wire_schema.rs:2461` | Binary search on `first_slot`, bounded by the logarithm of the record count |
| Owned decoder | `src/wire_schema.rs:2524` | `decode_data_layout` expands every run back to one `DataSlot` with a cloned `String` per element |

The container itself, `keleusma-wire/`, assigns no meaning to these regions. Its relevant
constants are the eight-byte word at `keleusma-wire/src/layout.rs:56` and the sixteen-byte
directory entry at `keleusma-wire/src/layout.rs:72`. Region kinds `0x001A` to `0x001D` are
listed at `docs/spec/WIRE_FORMAT.md:290` to `docs/spec/WIRE_FORMAT.md:293`.

**The specification describes the logical tables and not the run records, which is a fact
established by reading it.** `docs/spec/WIRE_FORMAT.md:200` describes `shared_layout` as one
entry per shared slot and `private_composite_layout` as one entry per private composite slot,
and the file contains no description of the `run`, `first_slot` or `stride` fields. Any
reimplementation from the specification alone would therefore not reproduce the current bytes.

### The runtime

| Concern | Location | Behaviour |
|---|---|---|
| Private storage | `src/vm.rs:1158` to `src/vm.rs:1167` | `private_slot_count` values of `GenericValue`, 32 bytes each, in the arena persistent region |
| Load-time initialisation | `src/vm.rs:2704` | Writes one `Value` per private slot from the decoded initialiser pool |
| Private composite lookup | `src/vm.rs:3048` to `src/vm.rs:3063` | Linear scan over every placement record on each flat composite private write |
| Private write | `src/vm.rs:3161` | A composite with no placement record is stored directly instead of being persisted into the pool |
| Indexed read | `src/vm.rs:5791` | Resolves `base + index` as a logical slot after checking the span against the data length |
| Typed layout validation | `src/verify_typed.rs:435` | Iterates logical slots |

### The self-hosted pipeline

| Concern | Location | Behaviour |
|---|---|---|
| Data records from the parser | `src/selfhost/kel/parse.kel`, consumed at `src/selfhost/mod.rs:7150` | Already one record per field, with code 8 carrying the element count |
| Layout assembly | `src/selfhost/mod.rs:6944` to `src/selfhost/mod.rs:7160` | `assemble_data_slots`, `assemble_shared_layout`, `assemble_private_init` and `private_composite_pool` expand fields to elements in Rust |
| Run grouping driver | `src/selfhost/wire_run_encode.rs:39`, `src/selfhost/wire_run_encode.rs:158` | Feeds logical slots to the Keleusma stage in batches |
| Run grouping stage | `src/selfhost/kel/wire_schema.kel:1613` to `src/selfhost/kel/wire_schema.kel:1760` | Groups logical slots through byte-comparison windows over names |
| Data decode stage | `src/selfhost/kel/wire_schema.kel:621` to `src/selfhost/kel/wire_schema.kel:765` | Reads the run records back |
| Layout verification | `src/selfhost/mod.rs:6600` to `src/selfhost/mod.rs:6680` | Feeds the shared layout to `verify_datalayout.kel` in batches per slot |
| Artifact comparison | `src/selfhost/artifact_compare.rs:23` to `src/selfhost/artifact_compare.rs:102` | Compares the decoded `DataLayout` and `schema_hash` field by field |

## A fresh measurement

### Method

The command-line frontend was built with `RUSTC_WRAPPER=` set empty and
`CARGO_TARGET_DIR` set to `target-measure` inside the worktree, by
`cargo build --release -p keleusma-cli`, on branch `docs/v02-array-wire-slot-design` at
`c1380cee`. Each of the fifteen stage sources under `src/selfhost/kel/` was compiled with
`keleusma compile <source> -o <artifact>` at the default host target. A Python script read the
64-byte framing header for the auxiliary body offset, walked the first directory copy at
offset 48, and decoded the four data-layout regions with the record layouts listed above. The
script is a measurement aid held outside the repository and is not a tracked test.

**Coverage limits.** One target only. The script reads the first directory copy without
voting, which is sound for an uncorrupted artifact and says nothing about a corrupted one.
Private element types were inferred from the elision flag, since every stage's initialiser
pool is elided and the compiler emits `Int(0)` only for `Word`.

### The stage corpus

| Stage | Source bytes | Artifact bytes | Logical slots | `DATA_SLOTS` records | Data regions bytes | Private elements in arrays | Private data bytes |
|---|---|---|---|---|---|---|---|
| `analyze` | 43,807 | 44,852 | 132,845 | 143 | 1,184 | 9,856 | 316,928 |
| `codegen` | 165,659 | 118,332 | 46,599 | 104 | 856 | 21,968 | 704,512 |
| `lexer` | 32,497 | 28,212 | 528,248 | 30 | 424 | 104 | 3,776 |
| `parse` | 483,715 | 328,948 | 96,214 | 698 | 5,608 | 51,688 | 1,666,080 |
| `reconstruct` | 74,582 | 65,908 | 35,480 | 93 | 768 | 5,214 | 168,416 |
| `template_scan` | 7,651 | 11,340 | 1,374 | 32 | 280 | 14 | 864 |
| `verify_datalayout` | 4,714 | 6,340 | 3,085 | 16 | 152 | 0 | 0 |
| `verify_depth` | 14,768 | 14,220 | 41,432 | 29 | 256 | 462 | 15,008 |
| `verify_structural` | 18,051 | 15,212 | 90,130 | 29 | 256 | 32,768 | 1,048,800 |
| `verify_typed` | 27,165 | 33,964 | 157,666 | 74 | 648 | 5,064 | 162,656 |
| `verify_types` | 48,080 | 29,740 | 16,548 | 81 | 672 | 2,560 | 82,240 |
| `verify_yield` | 12,940 | 15,196 | 35,608 | 43 | 368 | 2,816 | 90,464 |
| `wire_debug` | 9,184 | 15,716 | 9,242 | 29 | 304 | 0 | 768 |
| `wire_schema` | 67,992 | 108,660 | 1,263 | 218 | 1,784 | 0 | 6,880 |
| `wire` | 265,296 | 180,828 | 179,337 | 155 | 1,376 | 0 | 4,160 |
| **Total** | 1,276,101 | 1,017,468 | 1,375,071 | 1,774 | 14,936 | 132,514 | 4,271,552 |

"Data regions bytes" is the sum of `DATA_SLOTS`, `SHARED_LAYOUT`, `PRIVATE_COMPOSITE` and
`DATA_INIT`. Three cross-checks hold exactly. For `lexer`, private data bytes equal 118 private
slots times 32. Shared data bytes for `lexer`, 555,024, equal the packed sum of
`[Byte; 524288]`, three `[Word; 1280]` arrays and two scalar words. For `parse`, the private
array elements plus the 377 private scalars equal the 52,065 private logical slots.

**What the table establishes, as fact.** On the current tree the data-layout regions are 1.5
percent of the corpus artifact bytes. Artifact size no longer tracks declared array size for
any stage. The old 16 MB figure is not reproducible on this tree.

### Probes of element kind and visibility

Each probe declares one array field `a` and one scalar `w: Word` in a single data block. The
private probes write `d.w` so that the compiler admits them.

| Probe | Elements | Artifact bytes | `CONSTS` bytes | `PRIVATE_COMPOSITE` bytes | Private data bytes |
|---|---|---|---|---|---|
| `shared [Word; N]` | 1 and 10,000 | 3,892 and 3,892 | absent | 0 | 0 |
| `shared [Byte; N]` | 1 and 10,000 | 3,908 and 3,908 | absent | 0 | 0 |
| `private [Word; N]` | 1 and 10,000 | 3,916 and 3,916 | elided | 0 | 64 and 320,032 |
| `private [Byte; N]` | 1 and 10,000 | 3,948 and 163,932 | 160,032 at 10,000 | 0 | 64 and 320,032 |
| `private [bool; N]` | 1 and 10,000 | 3,948 and 163,932 | 160,032 at 10,000 | 0 | 64 and 320,032 |
| `private [P; N]`, `P` two `Word` fields | 1,000 | 27,932 | 16,032 | 8,000 | 32,032 |
| `private [P; N]` | 70,000 | 1,648,228 | 1,120,032 | 524,288 | 2,240,032 |

**What the probes establish, as fact.** A shared array and a private `Word` array cost the
artifact nothing per element. A private `Byte` or `bool` array costs sixteen artifact bytes
per element. A private array of composites costs 24 artifact bytes per element. Every private
element costs 32 bytes of private data whatever its type, so a private `[Byte; 10000]` reserves
320,032 bytes for 10,000 bytes of content. The `[P; 70000]` probe emitted 524,288 bytes of
placement records, which is 65,536 records for 70,000 elements.

## What still scales with the element count

| Cost | Kind | Scales with | Status |
|---|---|---|---|
| Private initialiser constants for non-`Int(0)` zeros | Artifact bytes | Private elements of `Byte`, `bool`, `Fixed`, `Float`, `Text` or composite type | Measured |
| `PRIVATE_COMPOSITE` records | Artifact bytes, linear scan per write | Private composite elements | Measured for bytes, read for the scan |
| Private `Value` slots | Runtime memory | Every private element, at 32 bytes | Measured |
| Owned decode | Load-time heap | Every logical slot, one `String` clone each | Read, not measured |
| Typed schema hash | Compile-time heap and time | Every logical slot, one canonical line each | Read, not measured |
| Layout validation | Load-time and verification time | Every logical slot | Read, not measured |
| Self-hosted run grouping | Stage execution time | Every logical slot, with a name comparison | Read, not measured |
| Self-hosted layout assembly | Driver heap | Every logical slot | Read, not measured |

**The placement truncation, stated precisely.** The fact is that the compiler skips the
placement record when `u16::try_from(slot_idx + k)` fails at `src/compiler.rs:3918`, while it
continues to add the element's body to `persistent_composite_bytes`. The runtime lookup at
`src/vm.rs:3051` likewise returns `None` for any slot above 65,535. The write path at
`src/vm.rs:3181` then stores the flat composite value directly instead of persisting it into
the pool. **The consequence is a hypothesis.** A directly stored flat composite may hold a
handle into the ephemeral arena region, which would not survive RESET. No program was run to
confirm or refute this. The repair is independent of this design and is listed as stage 0.

## The proposed encoding

The ruling asks that an array occupy one wire slot plus a length. The current run records
already achieve this for the name and shared-layout tables whenever an array has at most 65,535
elements. Two encodings complete it, and they differ in how far they go.

### Encoding R, extended runs

Encoding R keeps the four regions and their order, and removes the remaining per-element
records.

1. `DataSlotRecord` keeps its eight bytes. Its `reserved` byte at offset 5 becomes an
   initialiser form for a private run, with `0` meaning the zero of the element kind, `1`
   meaning `Unit` under the write-before-read contract, and `2` meaning an explicit constant.
   A shared run keeps `0`. The private element kind travels in a second new byte only if the
   decoder cannot recover it from the constant form, and the design prefers that it can.
2. The elision predicate at `src/wire_schema.rs:2097` generalises from "every initialiser is
   `Int(0)`" to "every initialiser is the zero of its kind or `Unit`". The pool is then elided
   for every array the compiler can currently produce, because only scalar private fields admit
   an explicit `= literal` initialiser.
3. `PrivateCompositeRecord` becomes a sixteen-byte run record with `first_slot: u32`,
   `run: u32`, `offset: u32` and `stride: u32`, which removes both the per-element record and
   the `u16` slot cap. Lookup becomes a binary search on `first_slot`, matching
   `DataLayoutTable::shared_slot`.

Encoding R leaves the corpus artifacts byte-for-byte the same size, because no stage declares
a private array of non-`Word` scalars or composites. It is the smaller change.

### Encoding F, one record per declared field

Encoding F replaces `DATA_SLOTS`, `SHARED_LAYOUT`, `PRIVATE_COMPOSITE` and `DATA_INIT` with one
new region, provisionally `DATA_FIELDS`, holding one 32-byte record per declared field. A
non-array field is a field with a count of one. A multi-dimensional array is one field whose
count is the product of its dimensions, which is what `slots_for_data_type` already computes.

| Offset | Width | Field | Meaning |
|---|---|---|---|
| 0 | u32 | `name` | `NAMES` index of `block.field` |
| 4 | u32 | `first_slot` | Logical slot index of element zero |
| 8 | u32 | `count` | Element count, never zero, with `first_slot + count` at most `MAX_DATA_ADDR` |
| 12 | u32 | `offset` | Byte offset of element zero in the shared buffer or the persistent composite pool, or `ABSENT` |
| 16 | u32 | `stride` | Byte distance between consecutive elements, zero when `count` is one |
| 20 | u16 | `elem_len` | Flat composite body length of one element, zero for a scalar |
| 22 | u8 | `kind` | Scalar or composite kind tag in the space `SharedSlotRecord::kind` uses |
| 23 | u8 | `visibility` | `SHARED` or `PRIVATE` tag |
| 24 | u8 | `init` | `0` zero of kind, `1` `Unit`, `2` explicit constant |
| 25 | u8 | `reserved` | Zero, refused otherwise |
| 26 | u16 | `reserved` | Zero, refused otherwise |
| 28 | u32 | `init_const` | `CONSTS` index when `init` is `2`, `ABSENT` otherwise |

The stride is stored rather than derived, for the reason `SharedSlotRecord` gives at
`src/wire_schema.rs:2015` to `src/wire_schema.rs:2020`. Deriving it would put a kind-to-size
table on both the Rust and the Keleusma side, and duplication of that shape produced three
defects on 2026-08-12. The thirty-two-byte width is a power of two, as the container requires
of a record stride.

**The cost of Encoding F on the corpus is an estimate and is stated plainly.** The corpus
carries 1,774 `DATA_SLOTS` records today, which bounds the field count from above because a
run splits only at `u16::MAX`. At 32 bytes each the new region would hold at most 56,768 bytes
against 14,936 bytes in the four regions it replaces, an increase of about 41,832 bytes or 4.1
percent of the 1,017,468-byte corpus. Encoding F is therefore a pessimisation for the existing
stages. Its benefit is that every data-layout cost, wire and decode alike, becomes a function
of the field count for every element kind and visibility, and that one lookup returns
everything a packed private array needs.

### Storage S1, packed private arrays

Neither encoding by itself changes runtime memory. Storage S1 is the change that does. It
places each private array as a contiguous body at element width in the persistent region,
exactly as a shared array already lies in the host buffer, and keeps private scalars as
`Value` slots. A private `Word` element on a 64-bit word target would then take 8 bytes rather
than 32, and a `Byte` or `bool` element 1 byte rather than 32. A private array of composites
would already have its bodies in the persistent composite pool and would drop its per-element
`Value` handles.

### Addressing, options A and B

| | Option A, logical element slots kept | Option B, one logical slot per field |
|---|---|---|
| `GetData` and `SetData` operands | Unchanged | Renumbered, since an array takes one slot |
| `GetDataIndexed(base, len)` meaning | Unchanged | `base` names the field, the index selects the element body |
| Opcode stream of an existing program | Byte-identical under R or F without S1 | Changes wherever a slot index follows an array |
| Resolution cost with S1 | Binary search over field records on every private access | One indexed table read |
| Twenty-four-bit slot space counts | Elements | Fields |
| Authorization | Wire change only | Wire change and an operand-semantics change |

Option A with either encoding and without S1 changes nothing that executes. Option B changes
the meaning of existing operands without adding an opcode, which the minimal instruction set
constraint permits and which still needs authorization because it changes what a given byte
sequence means.

## Interactions

**Flat composites.** A field record carries the element's composite kind and `elem_len`, so a
private array of structs becomes one record with a pool offset and a stride, and the runtime
derives each element's pool offset by one multiplication. The bound check at
`src/vm.rs:3073`, which today searches for the next placement offset, becomes the field's own
`offset + count * stride`. That is an inference from the code, and the check's tests would have
to confirm it.

**Shared and private data.** Shared arrays need no storage change, since their bodies are
already packed. The shared prefix invariant that `validate_data_layout` enforces at
`src/verify_typed.rs:452` becomes a property of field records, all shared fields preceding all
private fields, and it remains checkable at load in one pass over the fields.

**The hot-swap schema hash.** `compute_typed_schema_hash` emits one line per logical slot, and
its type token is the element type after peeling arrays, so an array's length is expressed only
by the repetition of its line. Two forms are possible.

- **H1, the current form computed by streaming.** The hash value is unchanged for every
  module, and the cyclic redundancy check is fed line by line without the buffer. Time remains
  proportional to elements, at compile time only.
- **H2, a per-field form.** One line per field whose token carries the full array type, such as
  `[Word;1280]`, so that resizing an array still changes the hash. Every hash for a module with
  an array changes. The operator ruled on 2026-10-28 that the hash cover slot types, and H2
  preserves that coverage, but it changes a canonical form the operator ruled on, so it is the
  operator's call.

**Error-correcting code planes.** A plane is a companion region holding a single-error-correcting,
double-error-detecting code of eight check bits per 64-bit word. Planes are per-region and
resolved by kind, so a new or reshaped region acquires a plane through the existing
`encode_aux_body_with_ecc` path with no format change. A smaller region shrinks its plane in
proportion. Planes are off by default and remain so.

**Signing and encryption.** The signature covers the whole framed buffer with the signature
bytes and the trailer zeroed, and encryption covers the whole body. Neither inspects regions,
so neither mechanism changes. Every signed or encrypted artifact must be recompiled and
re-signed after the change, which is the same consequence a release fingerprint roll already
has.

**In-place reads.** `DataLayoutTable` stays a borrowed, allocation-free view. Under Encoding F
the field table is a fixed-stride record table searched by `first_slot`, the same shape as
`SharedSlotRecord` today, with a static bound of the logarithm of the field count. Under
Encoding R the private composite lookup gains the same bound and loses its linear scan.

## Worst-case memory usage and worst-case execution time

Worst-case memory usage is abbreviated WCMU below, and worst-case execution time WCET.

| Quantity | Today | R | F | F with S1 |
|---|---|---|---|---|
| Artifact data-layout bytes | Runs plus 24 bytes per private composite element plus 16 bytes per non-`Int(0)` private element | Runs only | 32 bytes per field | 32 bytes per field |
| Private data segment, declared in the header and charged to WCMU | 32 bytes per private slot | Unchanged | Unchanged | 32 bytes per private scalar plus packed array bodies |
| Owned decode heap | One `String` per logical slot | Unchanged unless `DataLayout` changes | Per field if `DataLayout` changes | Per field |
| Private composite write | Linear in placement records | Logarithmic in runs | Logarithmic in fields | Logarithmic or constant, by addressing option |
| Private scalar access | Constant | Constant | Constant | Constant under B, logarithmic under A |
| Shared access | Logarithmic in shared runs | Unchanged | Logarithmic in fields | Unchanged from F |

**The WCMU effect of S1 on the corpus is an inference from measured counts.** The fifteen stage
modules declare 132,514 private array elements, all `Word` by the elision argument above, in
4,271,552 bytes of private data. Packing those elements at eight bytes would save 24 bytes each,
3,180,336 bytes in total, leaving about 1,091,216 bytes, a reduction of about 74.5 percent. For
`parse` alone the estimate is 1,666,080 bytes reduced to 425,568. These figures assume the
64-bit host target and that `VALUE_SLOT_SIZE_BYTES` stays at 32, and no packed build exists to
measure them.

**WCET.** The per-iteration bound the verifier proves covers instruction execution. Whether the
present linear private composite scan at `src/vm.rs:3056` is charged at its worst case in the
cost model was not established, and that question deserves an answer whatever this design's
fate, since the scan is linear in a quantity the program's declarations control. Load-time work,
which is initialisation of private data, layout validation and decode, is inferred to lie
outside the per-iteration bound and is linear in elements today. Under S1 the private array
initialisation becomes a byte fill of a known length.

## The self-hosted pipeline and the differential oracle

The byte-identical differential oracle compares the self-hosted compiler's artifact with the
Rust reference's. Any change to the data-layout encoding must therefore land in both encoders
in the same increment, or the oracle fails on every program with a data block. That failure
would show disagreement and not which side is wrong.

- **`parse.kel` needs no change**, since its data records are already per field and carry the
  element count.
- **The driver's assembly functions** at `src/selfhost/mod.rs:6944` to
  `src/selfhost/mod.rs:7160` would emit field records instead of expanding to elements. That
  removes the per-element heap in the driver.
- **The run grouping stage** at `src/selfhost/kel/wire_schema.kel:1613` to
  `src/selfhost/kel/wire_schema.kel:1760`, and its driver `src/selfhost/wire_run_encode.rs`,
  would be replaced under Encoding F by a per-field emitter that needs no name comparison
  windows. Under Encoding R it would need the new initialiser byte and the private composite
  run record. Either way the stage's execution time stops depending on element count, which is
  an inference from reading the stage.
- **The data decode stage** at `src/selfhost/kel/wire_schema.kel:621` to
  `src/selfhost/kel/wire_schema.kel:765` and its tests under `tests/selfhost_wire_data_decode.rs`
  would follow the new record shape.
- **The self-hosted subset does not currently support a private array of structs**, as the
  comment at `src/selfhost/mod.rs:7162` to `src/selfhost/mod.rs:7167` records. The private
  composite run record would be implemented on the reference side first and refused by the
  cross-check on the self-hosted side until that subset grows.
- **The construct-support boundary counts** pinned by `self_hosted_construct_support_boundary`
  in `tests/selfhost_codegen.rs` are expected not to move, because the change is in encoding
  and not in admitted constructs. That expectation was not verified.

## Compatibility and migration

**Old artifacts would not load, and they should be refused rather than misread.** A region
reshaped in place, or a region replaced by another, makes every artifact of the old shape
unreadable. The hazard is a silent misread rather than a refusal. Under Encoding F an old
artifact carries `DATA_SLOTS` and no `DATA_FIELDS`, and a reader that looks only for
`DATA_FIELDS` would read it as a module with no data block, because region absence currently
means `None`. The new reader must therefore refuse any artifact that carries the retired region
kinds. Under Encoding R the reshaped `PrivateCompositeRecord` changes stride, which the record
table length check would detect only when the region length is not a multiple of both strides.

**`BYTECODE_VERSION` need not change, as a matter of current policy.** `docs/process/RELEASE_PROCESS.md`
records that the operator froze `BYTECODE_VERSION` at 2 on 2026-09-01 until a hardware compiler
exists or the language sees notable adoption, and that the per-release `FORMAT_FINGERPRINT` at
`src/bytecode.rs:3900` distinguishes releases instead. An artifact from a released version is
already refused by fingerprint. The precedent is commit `32af6da5`, which changed the
`DataSlotRecord` shape under version 2 and records that the record shape change was authorized.

**The within-cycle hazard needs an explicit answer.** Two builds of the same release cycle share
a fingerprint, so an artifact compiled before the change and loaded after it is not refused by
that mechanism. Three answers exist, each the operator's to choose.

- Roll the fingerprint at landing with `scripts/fingerprint.sh --new`, as well as at release.
- Rely on the structural refusal of retired region kinds described above.
- Raise `BYTECODE_VERSION` to 3, which the current policy forbids without new authorization.

No migration tool is proposed. Artifacts are recompiled from source, which is the documented
consequence of every release under the frozen version.

## Staged implementation plan

Each stage is a separate feature branch and pull request with green continuous integration
before merge, and each stage after stage 0 waits on the authorization named in it.

**Stage 0, the placement truncation.** Not a wire change. First, a test that writes, resets and
reads back a private composite element above slot 65,535, which settles the hypothesis above.
Then a compile-time refusal or a repair, with a negative test at slots 65,535 and 65,536.

**Stage 1, the reference encoding.** Encoding R or F in `src/wire_schema.rs`, the compiler and
the runtime view, with `DataLayout` changed or left as the operator decides.

- Round-trip tests for every element kind and visibility at counts of one, two, 65,535, 65,536
  and the `MAX_DATA_ADDR` boundary.
- A size test, generalising the probe matrix above, asserting that the artifact size is
  independent of element count for every kind and visibility.
- Negative tests for a zero count, `first_slot + count` overflow, `count * stride` overflow,
  non-zero reserved bytes, an unknown `init` form, overlapping or descending fields, and an
  artifact carrying a retired region kind.
- Updated golden wire bytes, `tests/wire_slot_layout.rs`, `tests/shared_layout_runs.rs`,
  `tests/consts_region_composition.rs` and `tests/wire_fuzz.rs`, with the pinned figures
  re-measured rather than edited.
- A fingerprint roll if the operator chooses that answer.

**Stage 2, the self-hosted twin.** The same encoding in `src/selfhost/kel/wire_schema.kel` and
its driver. Obligations are the byte-identical oracle across the stage corpus and the
`tests/selfhost_wire_*` suites, and unchanged construct-support boundary counts.

**Stage 3, the schema hash.** H1 or H2 in both compilers, with `tests/typed_schema_hash.rs`
extended to show that resizing, retyping and reordering an array each change the hash, and the
hot-swap suites `tests/hot_swap_trust_matrix.rs` and `tests/hot_swap_working_set.rs` green.

**Stage 4, packed private storage.** S1 with addressing option A or B. Obligations are a WCMU
test asserting the declared private size per element kind, `tests/wire_private_state_accounting.rs`,
RESET persistence of every element kind, Miri on the VM tests with Tree Borrows, and a
re-measurement of the corpus private data bytes against the estimate above.

**Stage 5, the specification.** `docs/spec/WIRE_FORMAT.md` gains the record layouts of the data
regions. This closes the existing gap as well as describing the new form, and it could land
first for the current run records without any authorization, since it changes no bytes.

## Decisions that remain the operator's

Each question lists lettered options with the recommendation first.

1. **Which encoding.**
   - A. Encoding F if S1 is intended, since S1 then needs one lookup.
   - B. Encoding R if S1 is not intended, since it costs the corpus nothing.
   - C. Neither, recording the remaining costs as accepted.
2. **Whether to authorize the wire-format change itself**, meaning the record shapes and region
   kinds above. This is required for either encoding.
   - A. Authorize the chosen encoding.
   - B. Withhold authorization, leaving stage 0 and the specification work of stage 5.
3. **How to close the within-cycle hazard.**
   - A. A fingerprint roll at landing.
   - B. Structural refusal of the retired regions only.
   - C. `BYTECODE_VERSION` raised to 3, which would reverse the 2026-09-01 policy.
4. **The schema hash form.**
   - A. H2, the per-field form.
   - B. H1, the current form streamed.
5. **Packed private storage.** S1 changes the declared private data size and therefore every
   admitted program's WCMU figure.
   - A. Authorize S1 as a separate stage after the encoding.
   - B. Defer it.
6. **Addressing under S1.**
   - A. Option B, for constant-time resolution.
   - B. Option A, for an unchanged opcode stream.
7. **The public `DataLayout` type.**
   - A. Change it to fields, a breaking change to a public type within the V0.2.x line.
   - B. Keep it and add a field-level view beside it, which leaves the owned decode per element.
8. **Stage 0.**
   - A. Refuse a private composite slot above 65,535 at compile time now, before any encoding
     change, once the stage 0 test has settled the consequence.
   - B. Wait for the encoding change to remove the cap.

   Stage 0 landed on 2026-10-30 in pull request 652, as option A, as an obvious defect repair
   under the operator's instruction to address open items. `[P; 65536]` compiles with one record
   per element, and `[P; 65537]` is refused naming the field, pinned by
   `tests/private_composite_slot_cap.rs`.

## Operator rulings, 2026-10-31

Received first-hand in the V0.2.X session.

> C3.1. Explain encodings F and R.
> C3.2. A. Authorize without version bump once a decision has been made.
> C3.3. Fingerprints (and bytecode versions) are only updated when public releases are cut. We
> are approaching a new public release, so a bumps are premature.
> C3.4. A.
> C3.5. A.
> C3.6. A, option B.
> C3.7. A.

After the explanation of the two encodings, "Proceed with encoding F."

| Decision | Ruling |
|---|---|
| 1, encoding | Encoding F, one record per declared field |
| 2, the wire change | Authorized at `BYTECODE_VERSION` 2 |
| 3, the within-cycle hazard | No fingerprint roll and no version change at landing. The release step rolls the fingerprint when the release is cut |
| 4, the schema hash | H2, the per-field form |
| 5, packed private storage | S1, authorized as a stage after the encoding |
| 6, addressing under S1 | Option B, one logical slot per field |
| 7, `DataLayout` | Changed to fields, a breaking change within the V0.2.x line |
| 8, stage 0 | Done in pull request 652 |

**What decision 3 leaves open until the release.** Between landing and the release, an artifact
built under the old layout could be read by a runtime built under the new one, and the
fingerprint will not tell them apart. Nothing is published, so the exposure is confined to
artifacts and caches inside the repository. Those are regenerated in the same change, and the
self-hosted stage artifacts are rebuilt by their tests.

## What was not verified

- The runtime consequence of the placement truncation above slot 65,535.
- Whether the cost model charges the linear private composite scan at its worst case.
- The owned decode heap, the schema hash buffer and the self-hosted stage time, which were read
  from code and not measured.
- Every estimate for Encoding F's size and for S1's WCMU effect, since neither exists.
- Any target other than the default host target.
- Whether the `SharedSlotRecord` kind space can name every private element type, `Text` in
  particular, without extension.
- The construct-support boundary counts under any of the proposed changes.
