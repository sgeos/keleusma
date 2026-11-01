# keleusma-wire-derive

`#[derive(WireRecord)]` for [`keleusma-wire`](https://crates.io/crates/keleusma-wire).

**Depend on `keleusma-wire` with the `derive` feature rather than on this crate
directly:**

```toml
[dependencies]
keleusma-wire = { version = "0.1", features = ["derive"] }
```

## What it generates

From a struct of fixed-width fields, it emits the offset constants, the stride,
and a total codec — so a record's layout is declared once instead of twice.

```rust
use keleusma_wire::WireRecord;

#[derive(WireRecord)]
struct ChunkDesc {
    name_off: u32,
    name_len: u32,
    const_first: u32,
    const_count: u32,
}

// Generated:
assert_eq!(ChunkDesc::OFFSET_NAME_OFF, 0);
assert_eq!(ChunkDesc::OFFSET_CONST_COUNT, 12);
assert_eq!(ChunkDesc::PACKED_BYTES, 16);
assert_eq!(<ChunkDesc as WireRecord>::STRIDE, 16);
```

Both halves are emitted deliberately. The codec (`read_record` / `write_record`)
covers the common case; the `OFFSET_*` constants let a caller read one field in
place without materialising the whole record.

## Layout rules

- **Fields are packed in declaration order with no implicit padding.** The
  container is byte-addressed, so a `u8` between two wider fields does *not* push
  the next field to an aligned offset. This differs from Rust's own layout rules,
  which is why the offsets are generated rather than taken from `repr(C)`.
- **The record is padded to a whole 64-bit word**, so element *i* of a table sits
  at a power-of-two stride and addressing stays a shift.

## Permitted field types

`u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, and `[u8; N]`.

Anything else is a compile error naming the field. The restriction is deliberate:
a type whose in-memory size differs from its wire width — anything with alignment
padding, a pointer, or a platform-dependent size — would produce offsets that
silently disagree with the bytes on the wire.

## Licence

0BSD.
