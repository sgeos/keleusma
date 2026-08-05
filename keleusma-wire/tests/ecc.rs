//! The parity plane end to end: build a protected artifact, damage it, and
//! require the damage to be corrected or reported.
//!
//! The unit tests in `src/ecc.rs` prove the code construction exhaustively at the
//! word level. These prove the plane is wired to the container correctly — that
//! the right parity bytes cover the right payload words, which is the part a
//! correct codec can still get wrong.

#![cfg(feature = "alloc")]

use keleusma_wire::layout::FLAG_IS_ECC;
use keleusma_wire::{WireBuilder, WireError, WireView, WordStatus};

const DATA: u16 = 1;
const DATA_ECC: u16 = 2;
const PLAIN: u16 = 3;

const PAYLOAD: &[u8] = b"the quick brown fox jumps over the lazy dog, twice over!!";

fn build_protected() -> Vec<u8> {
    let mut b = WireBuilder::new();
    let d = b.region(DATA, 0).unwrap();
    b.push(d, PAYLOAD);
    b.protect(d, DATA_ECC).unwrap();

    // An unprotected region alongside, so the linkage must actually match on
    // `covers` rather than just finding the only plane present.
    let p = b.region(PLAIN, 0).unwrap();
    b.push(p, b"unprotected");

    b.finish().unwrap()
}

/// Byte offset of the protected payload inside the artifact.
fn payload_offset(view: &WireView<'_>) -> usize {
    view.find_region(DATA).unwrap().byte_offset().unwrap()
}

#[test]
fn a_protected_region_round_trips_and_reports_clean() {
    let img = build_protected();
    let view = WireView::parse(&img).unwrap();

    let data = view.find_region(DATA).unwrap();
    assert!(data.has_ecc(), "protected region must advertise its plane");
    assert!(!data.is_ecc_plane());

    let plane_region = view.find_region(DATA_ECC).unwrap();
    assert!(plane_region.is_ecc_plane());
    assert_eq!(plane_region.covers, DATA);
    assert_eq!(plane_region.flags & FLAG_IS_ECC, FLAG_IS_ECC);

    let report = view.verify_region(&data).expect("plane should be found");
    assert!(report.is_clean(), "undamaged artifact must verify clean");
    assert!(!report.needs_scrub());
    assert_eq!(report.words, PAYLOAD.len().div_ceil(8));

    // Payload still reads in place and unchanged.
    assert_eq!(&view.region_bytes(&data).unwrap()[..PAYLOAD.len()], PAYLOAD);
}

#[test]
fn an_unprotected_region_reports_no_plane_rather_than_failing() {
    let img = build_protected();
    let view = WireView::parse(&img).unwrap();
    let plain = view.find_region(PLAIN).unwrap();

    assert!(!plain.has_ecc());
    assert!(view.ecc_for(&plain).is_none());
    // Absence of a plane is not an error -- the plane is purely additive.
    assert!(view.verify_region(&plain).is_none());
}

#[test]
fn every_single_bit_fault_in_the_payload_is_corrected() {
    // This is the property that distinguishes the format: a flipped bit in the
    // DATA, not merely in the header, is recovered rather than silently served.
    let img = build_protected();
    let base = {
        let view = WireView::parse(&img).unwrap();
        payload_offset(&view)
    };
    let words = PAYLOAD.len().div_ceil(8);

    let mut checked = 0;
    for word in 0..words {
        for bit in 0..64 {
            let byte = base + word * 8 + bit / 8;
            let mut m = img.clone();
            m[byte] ^= 1 << (bit % 8);

            let view = WireView::parse(&m).unwrap();
            let data = view.find_region(DATA).unwrap();
            let plane = view.ecc_for(&data).unwrap();
            let stored = view.region_bytes(&data).unwrap();

            // The damaged word must decode to the ORIGINAL value.
            let want = {
                let clean = WireView::parse(&img).unwrap();
                let cd = clean.find_region(DATA).unwrap();
                let cb = clean.region_bytes(&cd).unwrap();
                u64::from_le_bytes(cb[word * 8..word * 8 + 8].try_into().unwrap())
            };
            assert_eq!(
                plane.word(stored, word),
                Some(WordStatus::Corrected(want)),
                "word {word} bit {bit} not corrected"
            );

            // And the scan must count exactly one repairable fault.
            let report = view.verify_region(&data).unwrap();
            assert_eq!(report.corrected, 1);
            assert_eq!(report.uncorrectable, 0);
            assert!(report.needs_scrub());
            checked += 1;
        }
    }
    assert_eq!(checked, words * 64);
}

#[test]
fn a_single_bit_fault_in_the_parity_plane_leaves_the_data_intact() {
    // A fault in the check byte must not be mistaken for a data fault and
    // "corrected" into corruption.
    let img = build_protected();
    let view = WireView::parse(&img).unwrap();
    let plane_region = view.find_region(DATA_ECC).unwrap();
    let base = plane_region.byte_offset().unwrap();

    for bit in 0..8 {
        let mut m = img.clone();
        m[base] ^= 1 << bit;

        let view = WireView::parse(&m).unwrap();
        let data = view.find_region(DATA).unwrap();
        let plane = view.ecc_for(&data).unwrap();
        let stored = view.region_bytes(&data).unwrap();

        let want = u64::from_le_bytes(stored[0..8].try_into().unwrap());
        assert_eq!(
            plane.word(stored, 0),
            Some(WordStatus::Corrected(want)),
            "parity-bit fault {bit} must leave data unchanged"
        );
    }
}

#[test]
fn a_double_bit_fault_is_reported_rather_than_miscorrected() {
    // Silent mis-correction is the dangerous outcome, so the status is asserted,
    // not merely that the value is wrong.
    let img = build_protected();
    let base = {
        let view = WireView::parse(&img).unwrap();
        payload_offset(&view)
    };

    for (b1, b2) in [(0usize, 1usize), (0, 7), (3, 4), (0, 63), (17, 40)] {
        let mut m = img.clone();
        m[base + b1 / 8] ^= 1 << (b1 % 8);
        m[base + b2 / 8] ^= 1 << (b2 % 8);

        let view = WireView::parse(&m).unwrap();
        let data = view.find_region(DATA).unwrap();
        let plane = view.ecc_for(&data).unwrap();
        let stored = view.region_bytes(&data).unwrap();

        assert_eq!(
            plane.word(stored, 0),
            Some(WordStatus::Uncorrectable),
            "double fault {b1},{b2} must be reported"
        );
        let report = view.verify_region(&data).unwrap();
        assert_eq!(report.uncorrectable, 1);
        assert!(!report.is_clean());
    }
}

#[test]
fn the_plane_covers_the_padding_too() {
    // The payload is 57 bytes, so the last word is part padding. A fault in the
    // padding must still be caught, or a region's tail would be unprotected.
    let img = build_protected();
    let view = WireView::parse(&img).unwrap();
    let data = view.find_region(DATA).unwrap();
    let base = data.byte_offset().unwrap();
    let stored_len = data.byte_length().unwrap();
    assert!(stored_len > PAYLOAD.len(), "test needs a padded region");

    let mut m = img.clone();
    m[base + stored_len - 1] ^= 0x01; // last padding byte

    let view = WireView::parse(&m).unwrap();
    let data = view.find_region(DATA).unwrap();
    let report = view.verify_region(&data).unwrap();
    assert_eq!(report.corrected, 1, "padding must be covered by the plane");
}

#[test]
fn protect_rejects_a_colliding_or_repeated_plane() {
    let mut b = WireBuilder::new();
    let d = b.region(DATA, 0).unwrap();

    // Collides with an existing region kind.
    assert_eq!(
        b.protect(d, DATA),
        Err(WireError::DuplicateRegion { kind: DATA })
    );

    b.protect(d, DATA_ECC).unwrap();
    // Already protected.
    assert!(b.protect(d, 99).is_err());
    // A later region cannot claim the plane's kind.
    assert_eq!(
        b.region(DATA_ECC, 0),
        Err(WireError::DuplicateRegion { kind: DATA_ECC })
    );
}

#[test]
fn an_empty_protected_region_is_legal() {
    let mut b = WireBuilder::new();
    let d = b.region(DATA, 0).unwrap();
    b.protect(d, DATA_ECC).unwrap();
    let img = b.finish().unwrap();

    let view = WireView::parse(&img).unwrap();
    let data = view.find_region(DATA).unwrap();
    let report = view.verify_region(&data).unwrap();
    assert_eq!(report.words, 0);
    assert!(report.is_clean());
}

#[test]
fn the_plane_costs_one_eighth_of_the_payload() {
    // The size claim, pinned so a change to the code rate is deliberate.
    let img = build_protected();
    let view = WireView::parse(&img).unwrap();
    let data = view.find_region(DATA).unwrap();
    let plane = view.find_region(DATA_ECC).unwrap();

    let data_words = data.word_length as usize;
    let plane_bytes = view.region_bytes(&plane).unwrap().len();
    assert!(plane_bytes >= data_words, "one check byte per data word");
    // Rounded up to a whole word, so at most seven bytes of slack.
    assert!(plane_bytes < data_words + 8);
}
