//! Directory decoding retains borrowed payloads beyond the stage byte window.
#![cfg(feature = "self-host")]

use keleusma::selfhost::wire_auxiliary_directory_via_kel;
use keleusma_wire::{WireBuilder, WireView};

fn assert_directory(bytes: &[u8]) {
    let actual = wire_auxiliary_directory_via_kel(bytes).unwrap();
    let reference = WireView::parse(bytes).unwrap();
    assert_eq!(actual.needs_scrub, reference.needs_scrub());
    assert_eq!(actual.regions.len(), usize::from(reference.region_count()));
    for (index, region) in actual.regions.iter().enumerate() {
        let expected = reference.region_at(index as u16).unwrap();
        assert_eq!(
            (region.kind, region.flags, region.covers),
            (expected.kind, expected.flags, expected.covers)
        );
        assert_eq!(region.byte_offset, expected.byte_offset().unwrap());
        let payload = reference.region_bytes(&expected).unwrap();
        assert_eq!(region.bytes, payload);
        assert_eq!(region.bytes.as_ptr(), payload.as_ptr());
    }
}

#[test]
fn empty_and_maximum_directories_preserve_every_entry() {
    assert_directory(&WireBuilder::new().finish().unwrap());
    let mut builder = WireBuilder::new();
    for kind in 0..1024 {
        builder.region(kind, kind).unwrap();
    }
    let bytes = builder.finish().unwrap();
    assert_eq!(bytes.len(), 49200);
    assert_eq!(WireView::parse(&bytes).unwrap().region_count(), 1024);
    assert_directory(&bytes);
}

#[test]
fn large_payloads_and_parity_planes_remain_borrowed() {
    let mut builder = WireBuilder::new();
    let data = builder.region(100, 0).unwrap();
    builder.push(data, &vec![0xA5; 200_003]);
    builder.protect(data, 50000).unwrap();
    let tail = builder.region(101, 0).unwrap();
    builder.push(tail, &[1, 2, 3]);
    let bytes = builder.finish().unwrap();
    assert!(bytes.len() > 200_003);
    assert_directory(&bytes);
    let mut unaligned = vec![0; 3];
    unaligned.extend_from_slice(&bytes);
    assert_directory(&unaligned[3..]);
    let result = wire_auxiliary_directory_via_kel(&bytes).unwrap();
    assert_eq!(result.regions.len(), 3);
    assert!(result.regions.iter().any(|r| r.covers == 100));
    assert!(result.regions.iter().any(|r| r.byte_offset > 65536));
}

#[test]
fn overlapping_header_spans_and_reserved_fields_keep_reference_acceptance() {
    let mut builder = WireBuilder::new();
    builder.region(5, 65535).unwrap();
    builder.region(6, 0).unwrap();
    let mut bytes = builder.finish().unwrap();
    for copy in 0..3 {
        for region in 0..2 {
            let at = 48 + copy * 32 + region * 16;
            bytes[at + 4..at + 8].copy_from_slice(&0u32.to_le_bytes());
            bytes[at + 8..at + 12].copy_from_slice(&6u32.to_le_bytes());
            bytes[at + 12..at + 14].copy_from_slice(&65535u16.to_le_bytes());
            bytes[at + 14..at + 16].copy_from_slice(&65535u16.to_le_bytes());
        }
    }
    assert_directory(&bytes);
}
