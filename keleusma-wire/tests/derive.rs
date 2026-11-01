//! `#[derive(WireRecord)]`: offsets, stride, and the round trip.
//!
//! The point of the derive is that a layout is declared once instead of twice.
//! These tests therefore check the generated offsets against hand-computed ones —
//! if the derive and the hand count ever disagree, the derive is what would be
//! silently wrong, since nothing else would notice.

#![cfg(all(feature = "alloc", feature = "derive"))]

use keleusma_wire::{WireBuilder, WireRecord, WireView};

#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
struct ChunkDesc {
    name_off: u32,
    name_len: u32,
    const_first: u32,
    const_count: u32,
}

/// Deliberately not a whole word packed (2 + 1 + 4 = 7), to exercise padding.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
struct Odd {
    tag: u16,
    flags: u8,
    value: u32,
}

/// Mixed widths, a signed field, and a byte array.
#[derive(WireRecord, Debug, Clone, Copy, PartialEq, Eq)]
struct Mixed {
    a: u8,
    b: i16,
    c: i64,
    d: [u8; 5],
}

const RECORDS: u16 = 1;

#[test]
fn offsets_and_stride_match_a_hand_count() {
    assert_eq!(ChunkDesc::OFFSET_NAME_OFF, 0);
    assert_eq!(ChunkDesc::OFFSET_NAME_LEN, 4);
    assert_eq!(ChunkDesc::OFFSET_CONST_FIRST, 8);
    assert_eq!(ChunkDesc::OFFSET_CONST_COUNT, 12);
    assert_eq!(ChunkDesc::PACKED_BYTES, 16);
    assert_eq!(<ChunkDesc as WireRecord>::STRIDE, 16);
}

#[test]
fn fields_are_packed_with_no_implicit_padding() {
    // The container is byte-addressed and inserts no alignment padding, so a u8
    // between two wider fields must NOT push the next field to an aligned offset.
    // Rust's own layout rules would; the wire's do not.
    assert_eq!(Odd::OFFSET_TAG, 0);
    assert_eq!(Odd::OFFSET_FLAGS, 2);
    assert_eq!(Odd::OFFSET_VALUE, 3);
    assert_eq!(Odd::PACKED_BYTES, 7);

    assert_eq!(Mixed::OFFSET_A, 0);
    assert_eq!(Mixed::OFFSET_B, 1);
    assert_eq!(Mixed::OFFSET_C, 3);
    assert_eq!(Mixed::OFFSET_D, 11);
    assert_eq!(Mixed::PACKED_BYTES, 16);
}

#[test]
fn the_record_is_padded_to_a_whole_word() {
    // 7 packed bytes must round up to 8, or table addressing stops being a shift.
    assert_eq!(<Odd as WireRecord>::STRIDE, 8);
    assert_eq!(<Mixed as WireRecord>::STRIDE, 16);
    assert_eq!(<ChunkDesc as WireRecord>::STRIDE % 8, 0);
}

#[test]
fn records_round_trip_through_the_container() {
    let want = [
        ChunkDesc {
            name_off: 0,
            name_len: 4,
            const_first: 0,
            const_count: 3,
        },
        ChunkDesc {
            name_off: 4,
            name_len: 4,
            const_first: 3,
            const_count: 3,
        },
        ChunkDesc {
            name_off: 8,
            name_len: 5,
            const_first: 6,
            const_count: 2,
        },
    ];

    let mut b = WireBuilder::new();
    let t = b.region(RECORDS, 0).unwrap();
    for r in &want {
        b.push_record(t, r);
    }
    let img = b.finish().unwrap();

    let view = WireView::parse(&img).unwrap();
    let region = view.find_region(RECORDS).unwrap();
    let table = view.typed_records::<ChunkDesc>(&region).unwrap();

    assert_eq!(table.len(), 3);
    for (i, w) in want.iter().enumerate() {
        assert_eq!(table.get_as::<ChunkDesc>(i).as_ref(), Some(w));
    }
    assert_eq!(table.get_as::<ChunkDesc>(3), None);
}

#[test]
fn signed_and_array_fields_survive_the_round_trip() {
    let want = Mixed {
        a: 0xFE,
        b: -300,
        c: -1_234_567_890_123,
        d: *b"hello",
    };

    let mut b = WireBuilder::new();
    let t = b.region(RECORDS, 0).unwrap();
    b.push_record(t, &want);
    let img = b.finish().unwrap();

    let view = WireView::parse(&img).unwrap();
    let region = view.find_region(RECORDS).unwrap();
    let table = view.typed_records::<Mixed>(&region).unwrap();
    assert_eq!(table.get_as::<Mixed>(0), Some(want));
}

#[test]
fn a_table_opened_with_the_wrong_record_type_refuses_to_decode() {
    // Without the stride check this would read plausible-looking values from the
    // wrong offsets -- a wrong answer rather than an error, which is the failure
    // mode worth engineering against.
    let mut b = WireBuilder::new();
    let t = b.region(RECORDS, 0).unwrap();
    b.push_record(
        t,
        &ChunkDesc {
            name_off: 1,
            name_len: 2,
            const_first: 3,
            const_count: 4,
        },
    );
    let img = b.finish().unwrap();

    let view = WireView::parse(&img).unwrap();
    let region = view.find_region(RECORDS).unwrap();

    let table = view.typed_records::<ChunkDesc>(&region).unwrap();
    assert!(table.get_as::<ChunkDesc>(0).is_some());
    // `Odd` has stride 8, the table has stride 16.
    assert_eq!(table.get_as::<Odd>(0), None);
}

#[test]
fn reading_a_short_slice_yields_none_rather_than_panicking() {
    let full = [0u8; 16];
    for cut in 0..16 {
        assert_eq!(ChunkDesc::read_record(&full[..cut]), None, "cut {cut}");
    }
    assert!(ChunkDesc::read_record(&full).is_some());
}

#[test]
fn writing_into_a_short_slice_reports_rather_than_truncating() {
    let r = ChunkDesc {
        name_off: 1,
        name_len: 2,
        const_first: 3,
        const_count: 4,
    };
    let mut small = [0u8; 8];
    assert_eq!(r.write_record(&mut small), None);
    let mut ok = [0u8; 16];
    assert_eq!(r.write_record(&mut ok), Some(()));
    assert_eq!(ChunkDesc::read_record(&ok), Some(r));
}

#[test]
fn the_generated_offsets_agree_with_the_generated_bytes() {
    // The derive emits both the offset constants and the codec. If they ever
    // disagreed, a caller mixing in-place reads with `read_record` would get
    // inconsistent answers -- so they are checked against each other.
    let r = ChunkDesc {
        name_off: 0xAABB,
        name_len: 7,
        const_first: 9,
        const_count: 11,
    };
    let mut buf = [0u8; 16];
    r.write_record(&mut buf).unwrap();

    let at = ChunkDesc::OFFSET_NAME_OFF;
    assert_eq!(
        u32::from_le_bytes(buf[at..at + 4].try_into().unwrap()),
        0xAABB
    );
    let at = ChunkDesc::OFFSET_CONST_COUNT;
    assert_eq!(u32::from_le_bytes(buf[at..at + 4].try_into().unwrap()), 11);
}
