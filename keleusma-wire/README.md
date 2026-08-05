# keleusma-wire

A word-oriented binary container format: fixed-size records, in-place reads, and a
triplicated directory — with **no dependency on what the records mean**.

This crate is the container only: framing, a region directory, fixed-stride record
tables, byte pools, and the integrity primitives. It does not know what your data
is. You choose the region kinds, the record strides, and the field offsets. That
separation is what makes it reusable outside the project it was written for.

## When this is the right tool

It suits artifacts that are **written once and read many times, possibly much
later, on hardware that may be small or hostile**: compiled program images,
firmware payloads, lookup tables shipped to embedded targets, anything you would
otherwise mmap and read directly.

It is **not** a general serialization framework. There is no schema language and
no versioned struct evolution. If you want to serialize arbitrary Rust types with
minimal ceremony, use `postcard`, `bincode`, or `rkyv`.

## What it guarantees

- **Reads cost no allocation.** Every accessor returns a slice aliasing your
  buffer. A string is a direct subslice; a record table is addressed by
  arithmetic, not materialised. The `alloc` feature gates only the *encoder*, so
  a read-only target needs no allocator at all. Verified by building for
  `wasm32v1-none`, and asserted by address in the test suite — a test that only
  checked values would not notice an owned decode creeping in.
- **Single-bit faults are corrected in the header *and* in your data.** The
  prologue and directory are stored three times and read by majority vote; any
  region may additionally carry a (72,64) SECDED parity plane that corrects a
  single-bit fault per 64-bit word and detects a double. The test suite injects
  every single-bit fault across the protected header (1536 cases) and every
  single-bit fault across a protected payload (512 cases), and requires each to be
  both corrected *and* reported. The code construction itself is verified
  exhaustively: all 432 single-bit and all 15336 double-bit patterns.
  **This is the thing most serialization crates do not do at all.**
- **Corruption cannot destroy framing.** No record carries a length prefix, so a
  flipped bit corrupts one field rather than desynchronising everything after it.
- **Decoding is total.** Malformed input is rejected, never panicked on. Every
  truncation of a valid artifact is tested, as is every single-bit corruption.
- **Addressing is a shift.** Offsets are word indices and every record is a whole
  number of 64-bit words, so element *i* is at `base + i * stride` with a
  power-of-two stride.

## Example

```rust
use keleusma_wire::{WireBuilder, WireView};

const STRINGS: u16 = 1;
const RECORDS: u16 = 2;

let mut b = WireBuilder::new();
let pool = b.region(STRINGS, 0)?;
let table = b.region(RECORDS, 0)?;

// Record the offset before appending, so nothing is ever back-patched.
let at = b.len_of(pool) as u32;
b.push(pool, b"alpha");

let mut record = [0u8; 8];
record[0..4].copy_from_slice(&at.to_le_bytes());
record[4..8].copy_from_slice(&5u32.to_le_bytes());
b.push(table, &record);

let artifact = b.finish()?;

let view = WireView::parse(&artifact)?;
let pool = view.pool(&view.find_region(STRINGS).unwrap())?;
let table = view.records(&view.find_region(RECORDS).unwrap(), 8)?;

let r = table.get(0).unwrap();
let off = u32::from_le_bytes(r[0..4].try_into().unwrap());
let len = u32::from_le_bytes(r[4..8].try_into().unwrap());
assert_eq!(pool.slice(off, len), Some(&b"alpha"[..]));
# Ok::<(), keleusma_wire::WireError>(())
```

### Protecting a region against bit rot

```rust
use keleusma_wire::{WireBuilder, WireView, WordStatus};

const DATA: u16 = 1;
const DATA_ECC: u16 = 2;

let mut b = WireBuilder::new();
let d = b.region(DATA, 0)?;
b.push(d, b"payload that should outlive its storage medium");
b.protect(d, DATA_ECC)?;          // one check byte per 8 payload bytes
let mut artifact = b.finish()?;

// Something flips a bit in the payload.
let at = WireView::parse(&artifact)?.find_region(DATA).unwrap()
    .byte_offset().unwrap();
artifact[at + 3] ^= 0x08;

let view = WireView::parse(&artifact)?;
let data = view.find_region(DATA).unwrap();

// The scan finds it, and says so rather than quietly fixing it.
let report = view.verify_region(&data).unwrap();
assert_eq!(report.corrected, 1);
assert!(report.needs_scrub());

// And the corrected word is available without mutating the artifact.
let plane = view.ecc_for(&data).unwrap();
match plane.word(view.region_bytes(&data)?, 0).unwrap() {
    WordStatus::Corrected(w) => { let _ = w; }
    other => panic!("expected a correction, got {other:?}"),
}
# Ok::<(), keleusma_wire::WireError>(())
```

Correction returns a **value**; it never writes to your buffer. That keeps the
read path borrow-only, and it means a caller decides whether to rewrite the
artifact — the scrub is yours to schedule.

### Declaring a record layout once

With the `derive` feature, a struct definition generates its own offsets, stride,
and codec, so the layout is not written twice and cannot drift.

```rust
use keleusma_wire::{WireBuilder, WireRecord, WireView};

#[derive(WireRecord, Debug, PartialEq, Eq)]
struct ChunkDesc {
    name_off: u32,
    name_len: u32,
    const_first: u32,
    const_count: u32,
}

const CHUNKS: u16 = 1;

let mut b = WireBuilder::new();
let t = b.region(CHUNKS, 0)?;
b.push_record(t, &ChunkDesc { name_off: 0, name_len: 4, const_first: 0, const_count: 3 });
let artifact = b.finish()?;

let view = WireView::parse(&artifact)?;
let table = view.typed_records::<ChunkDesc>(&view.find_region(CHUNKS).unwrap())?;
assert_eq!(table.get_as::<ChunkDesc>(0).unwrap().const_count, 3);

// The offset constants are emitted too, for reading one field in place.
assert_eq!(ChunkDesc::OFFSET_CONST_COUNT, 12);
# Ok::<(), keleusma_wire::WireError>(())
```

Fields are packed in declaration order with **no implicit padding** — the
container is byte-addressed, so a `u8` between two wider fields does not push the
next one to an aligned offset. That differs from Rust's own layout rules, which is
why the offsets are generated rather than taken from `repr(C)`. The record as a
whole is padded to a word so table addressing stays a shift.

Opening a table with the wrong record type is caught: `get_as` checks the stride,
so a mismatch returns `None` instead of reading plausible values from the wrong
offsets.

## Two design points worth knowing

**The prologue is separate from the directory**, which looks redundant until you
try to write the reader. Voting the directory requires knowing its stride, which
depends on the region count — and if the region count lived inside the block being
voted, a single bit flip in it would desynchronise the search for the very copies
that exist to repair it. A fixed-size prologue at fixed offsets is votable with no
prior knowledge, and its voted region count then makes the directory votable in
turn.

**Range references must point forward.** If a record references a range of other
records in the same table, that range must lie strictly after it. Under that
ordering the whole table can be walked bottom-up by a single reverse linear sweep
— no stack, static trip count. `RecordTable::range_is_forward` checks it, and you
should call it while validating rather than trusting your encoder, because the
violation is **silent**: a backwards range makes a reverse sweep read entries it
has not computed yet, producing a wrong answer rather than a fault.

## What this crate does not do

- **No encryption.** A region carries an `ENCRYPTED` flag so a reader knows the
  payload cannot be read in place, but this crate neither encrypts nor decrypts.
- **No multi-bit correction.** SECDED corrects one fault per 64-bit word and
  detects two; three or more in one word may be missed or mis-corrected. That is
  inherent to the code, not an implementation limit. The defence against
  accumulation is scrubbing, which is why `EccReport` reports corrected words
  instead of silently fixing them.
- **A parity plane is opt-in per region.** An unprotected region is unprotected;
  `protect()` is a call you make.
- **No schema evolution.** A record layout is fixed. Adding a field changes the
  stride, and old artifacts do not migrate themselves — reserve space up front, or
  version your region kinds.
- **No schema.** Which regions exist and what they mean are yours; the derive only
  computes offsets for a layout you declare.

## Features

- `alloc` (default on): the encoder, `WireBuilder`. The reader does not need it.
- `derive` (default off): `#[derive(WireRecord)]`. Off by default so a consumer
  that hand-writes its offsets pays no proc-macro build cost.

`WireError` implements `core::error::Error` unconditionally, so `no_std` consumers
get the standard error interface without a feature flag.

## Licence

0BSD.
