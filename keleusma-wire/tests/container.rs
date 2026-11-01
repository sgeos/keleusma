//! Container-level tests: round-trip, totality under malformed input, the
//! majority vote, and the in-place-read property.

#![cfg(feature = "alloc")]

use keleusma_wire::layout::{COPIES, DIR_ENTRY_BYTES, DIRECTORY_OFFSET, PROLOGUE_BYTES, WORD};
use keleusma_wire::{WireBuilder, WireError, WireView};

const STRINGS: u16 = 1;
const RECORDS: u16 = 2;
const EMPTY: u16 = 7;

/// A small artifact with a pool, a record table, and an empty region.
fn build() -> Vec<u8> {
    let mut b = WireBuilder::new();
    let pool = b.region(STRINGS, 0).unwrap();
    let table = b.region(RECORDS, 0).unwrap();
    let _ = b.region(EMPTY, 0).unwrap();

    let mut refs = Vec::new();
    for s in [&b"main"[..], b"tick", b"reset"] {
        let at = b.len_of(pool) as u32;
        b.push(pool, s);
        refs.push((at, s.len() as u32));
    }
    for (at, len) in refs {
        let mut r = [0u8; 8];
        r[0..4].copy_from_slice(&at.to_le_bytes());
        r[4..8].copy_from_slice(&len.to_le_bytes());
        b.push(table, &r);
    }
    b.finish().unwrap()
}

fn read_back(view: &WireView<'_>) -> Vec<Vec<u8>> {
    let pool = view.pool(&view.find_region(STRINGS).unwrap()).unwrap();
    let table = view
        .records(&view.find_region(RECORDS).unwrap(), 8)
        .unwrap();
    (0..table.len())
        .map(|i| {
            let r = table.get(i).unwrap();
            let off = u32::from_le_bytes(r[0..4].try_into().unwrap());
            let len = u32::from_le_bytes(r[4..8].try_into().unwrap());
            pool.slice(off, len).unwrap().to_vec()
        })
        .collect()
}

#[test]
fn round_trips_every_region() {
    let img = build();
    assert_eq!(
        img.len() % WORD,
        0,
        "artifact must be a whole number of words"
    );

    let view = WireView::parse(&img).unwrap();
    assert_eq!(view.region_count(), 3);
    assert!(!view.needs_scrub());
    assert_eq!(
        read_back(&view),
        vec![b"main".to_vec(), b"tick".to_vec(), b"reset".to_vec()]
    );

    // An empty region is addressable and yields an empty payload rather than an
    // error, so a schema can declare a region it did not populate.
    let empty = view.find_region(EMPTY).unwrap();
    assert_eq!(view.region_bytes(&empty).unwrap().len(), 0);
    assert!(view.records(&empty, 8).unwrap().is_empty());

    // An undeclared kind is absent, not garbage.
    assert!(view.find_region(999).is_none());
}

#[test]
fn reads_alias_the_input_buffer_rather_than_copying() {
    // The load-bearing property: an accessor must hand back a slice INTO the
    // caller's bytes. If someone ever routes these through an owned decode, the
    // values would still be right and every other test would still pass -- so
    // the aliasing is asserted directly, by address.
    let img = build();
    let view = WireView::parse(&img).unwrap();

    let base = img.as_ptr() as usize;
    let end = base + img.len();

    let pool = view.pool(&view.find_region(STRINGS).unwrap()).unwrap();
    let s = pool.slice(0, 4).unwrap();
    let at = s.as_ptr() as usize;
    assert!(
        at >= base && at + s.len() <= end,
        "pool slice must alias the artifact"
    );

    let table = view
        .records(&view.find_region(RECORDS).unwrap(), 8)
        .unwrap();
    let r = table.get(1).unwrap();
    let at = r.as_ptr() as usize;
    assert!(
        at >= base && at + r.len() <= end,
        "record must alias the artifact"
    );
}

#[test]
fn a_truncated_artifact_is_rejected_at_every_length() {
    let img = build();
    for cut in 0..img.len() {
        // Never a panic, and never an accidental success: every proper prefix is
        // either too short for the header or leaves a region out of bounds.
        assert!(
            WireView::parse(&img[..cut]).is_err(),
            "truncation to {cut} bytes must be rejected"
        );
    }
    assert!(WireView::parse(&img).is_ok());
}

#[test]
fn one_corrupted_prologue_or_directory_byte_is_outvoted_and_reported() {
    let img = build();
    let protected = DIRECTORY_OFFSET + 3 * DIR_ENTRY_BYTES * COPIES;

    for pos in 0..protected {
        for bit in 0..8u8 {
            let mut m = img.clone();
            m[pos] ^= 1 << bit;

            let view = WireView::parse(&m).unwrap_or_else(|e| {
                panic!("single fault at byte {pos} bit {bit} not survived: {e}")
            });

            // Recovering the value is not enough. Damage that goes unreported
            // accumulates until a second fault in the same position defeats the
            // vote, so the scrub signal is asserted too.
            assert!(
                view.needs_scrub(),
                "damage at byte {pos} bit {bit} unreported"
            );
            assert_eq!(
                read_back(&view),
                vec![b"main".to_vec(), b"tick".to_vec(), b"reset".to_vec()]
            );
        }
    }
}

#[test]
fn two_faults_in_the_same_position_are_not_silently_wrong() {
    // Majority-of-three corrects one fault by construction. The limit is
    // documented rather than defended against -- but the artifact must still not
    // be read as if it were intact, and must not panic.
    let img = build();
    let mut m = img.clone();
    m[8] ^= 0x01; // region_count, copy 0
    m[PROLOGUE_BYTES + 8] ^= 0x01; // and copy 1: the vote now carries the fault

    match WireView::parse(&m) {
        // The prologue CRC is the backstop once the vote is defeated.
        Err(_) => {}
        Ok(v) => assert!(
            v.needs_scrub(),
            "a defeated vote must still report disagreement"
        ),
    }
}

#[test]
fn corruption_anywhere_never_panics() {
    // Beyond the triplicated header there is no correction, so a payload fault
    // may well be read back as a wrong value. What must NOT happen is a panic or
    // an out-of-bounds read. Every byte, one bit each, is exercised.
    let img = build();
    for pos in 0..img.len() {
        let mut m = img.clone();
        m[pos] ^= 0x80;
        if let Ok(view) = WireView::parse(&m) {
            if let Some(r) = view.find_region(STRINGS) {
                let _ = view.region_bytes(&r);
                if let Ok(p) = view.pool(&r) {
                    let _ = p.slice(0, 4);
                    let _ = p.slice(u32::MAX, u32::MAX);
                }
            }
            for i in 0..view.region_count() {
                let _ = view.region_at(i);
            }
        }
    }
}

#[test]
fn a_duplicate_region_kind_is_rejected_by_both_halves() {
    // The builder refuses to create one...
    let mut b = WireBuilder::new();
    let _ = b.region(STRINGS, 0).unwrap();
    assert_eq!(
        b.region(STRINGS, 0),
        Err(WireError::DuplicateRegion { kind: STRINGS })
    );

    // ...and the reader refuses to accept one, since a hand-built artifact never
    // went through the builder. Rewrite the second entry's kind in all three
    // directory copies so the vote does not simply repair it.
    let img = build();
    let mut m = img.clone();
    let dir_span = 3 * DIR_ENTRY_BYTES;
    for copy in 0..COPIES {
        let entry = DIRECTORY_OFFSET + copy * dir_span + DIR_ENTRY_BYTES;
        m[entry] = STRINGS as u8;
        m[entry + 1] = (STRINGS >> 8) as u8;
    }
    assert_eq!(
        WireView::parse(&m).err(),
        Some(WireError::DuplicateRegion { kind: STRINGS })
    );
}

#[test]
fn a_bad_record_stride_is_rejected() {
    let img = build();
    let view = WireView::parse(&img).unwrap();
    let table = view.find_region(RECORDS).unwrap();

    assert_eq!(view.records(&table, 0), Err(WireError::BadRecordStride));
    // Not a whole number of words.
    assert_eq!(view.records(&table, 12), Err(WireError::BadRecordStride));
    // A whole number of words, but does not divide the region.
    assert_eq!(view.records(&table, 32), Err(WireError::BadRecordStride));
    assert!(view.records(&table, 8).is_ok());
}

#[test]
fn the_forward_ordering_invariant_is_checked_not_assumed() {
    let img = build();
    let view = WireView::parse(&img).unwrap();
    let table = view
        .records(&view.find_region(RECORDS).unwrap(), 8)
        .unwrap();
    assert_eq!(table.len(), 3);

    // Forward and in range.
    assert!(table.range_is_forward(0, 1, 2));
    assert!(table.range_is_forward(1, 2, 1));

    // Backward: the case whose violation is SILENT under a reverse sweep, which
    // is exactly why it must be rejected rather than trusted.
    assert!(!table.range_is_forward(2, 0, 2));
    // Self-reference is backward too.
    assert!(!table.range_is_forward(1, 1, 1));
    // Past the end.
    assert!(!table.range_is_forward(0, 2, 5));
    // Overflowing count.
    assert!(!table.range_is_forward(0, 1, u32::MAX));
}

#[test]
fn an_empty_builder_produces_a_valid_artifact() {
    // Degenerate but legal: no regions at all. It must round-trip rather than
    // being a special case the reader mishandles.
    let img = WireBuilder::new().finish().unwrap();
    let view = WireView::parse(&img).unwrap();
    assert_eq!(view.region_count(), 0);
    assert!(!view.needs_scrub());
    assert!(view.find_region(0).is_none());
    assert!(view.region_at(0).is_none());
}

#[test]
fn a_region_pointing_outside_the_buffer_is_rejected() {
    let img = build();
    let mut m = img.clone();
    let dir_span = 3 * DIR_ENTRY_BYTES;
    // Push the first region's word offset far past the end, in every copy.
    for copy in 0..COPIES {
        let entry = DIRECTORY_OFFSET + copy * dir_span;
        m[entry + 4..entry + 8].copy_from_slice(&0xFFFF_u32.to_le_bytes());
    }
    assert!(matches!(
        WireView::parse(&m),
        Err(WireError::RegionOutOfBounds { .. })
    ));
}
