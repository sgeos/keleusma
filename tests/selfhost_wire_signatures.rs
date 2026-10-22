//! Signature numbering must preserve both append and intern semantics.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{ChunkSignature, WireShape};
use keleusma::selfhost::wire_signature_sections_via_kel;
use keleusma::wire_schema::{ShapeRecord, SignatureRecord, signature_tables};
use keleusma_wire::WireRecord;

fn compare(sigs: &[ChunkSignature]) -> (Vec<ShapeRecord>, Vec<SignatureRecord>) {
    let actual = wire_signature_sections_via_kel(sigs).unwrap();
    let (shapes, signatures) = signature_tables(sigs);
    let mut shape_bytes = vec![0; shapes.len() * ShapeRecord::STRIDE];
    for (r, out) in shapes
        .iter()
        .zip(shape_bytes.as_chunks_mut::<{ ShapeRecord::STRIDE }>().0)
    {
        r.write_record(out).unwrap();
    }
    let mut signature_bytes = vec![0; signatures.len() * SignatureRecord::STRIDE];
    for (r, out) in signatures.iter().zip(
        signature_bytes
            .as_chunks_mut::<{ SignatureRecord::STRIDE }>()
            .0,
    ) {
        r.write_record(out).unwrap();
    }
    assert_eq!(actual.shapes, shape_bytes);
    assert_eq!(actual.signatures, signature_bytes);
    (shapes, signatures)
}

#[test]
fn parameters_append_while_returns_share_the_earliest_matching_record() {
    let top = WireShape::Top;
    let scalar = WireShape::Scalar { kind: 1 };
    let flat = WireShape::Flat {
        kind: 2,
        size: 0x12345678,
    };
    let (_, sigs) = compare(&[
        ChunkSignature {
            params: vec![top, top, scalar, flat],
            ret: top,
            resume: flat,
        },
        ChunkSignature {
            params: vec![scalar, top],
            ret: flat,
            resume: scalar,
        },
        ChunkSignature {
            params: vec![],
            ret: WireShape::Flat {
                kind: 255,
                size: u32::MAX,
            },
            resume: WireShape::Scalar { kind: 255 },
        },
        ChunkSignature {
            params: vec![],
            ret: top,
            resume: top,
        },
    ]);
    assert_eq!(
        (
            sigs[0].params_first,
            sigs[0].params_count,
            sigs[0].ret,
            sigs[0].resume
        ),
        (0, 4, 0, 3)
    );
    assert_eq!(
        (
            sigs[1].params_first,
            sigs[1].params_count,
            sigs[1].ret,
            sigs[1].resume
        ),
        (4, 2, 3, 2)
    );
    assert_eq!(
        (sigs[2].params_first, sigs[2].ret, sigs[2].resume),
        (6, 6, 7)
    );
    assert_eq!(
        (sigs[3].params_first, sigs[3].ret, sigs[3].resume),
        (8, 0, 0)
    );
}

#[test]
fn signature_search_crosses_the_transport_window_without_a_table_cap() {
    let unique = WireShape::Flat {
        kind: 4,
        size: 65537,
    };
    let mut params = vec![WireShape::Top; 6144];
    params.push(unique);
    let (shapes, sigs) = compare(&[ChunkSignature {
        params,
        ret: unique,
        resume: WireShape::Top,
    }]);
    assert_eq!(shapes.len(), 6145);
    assert_eq!((sigs[0].ret, sigs[0].resume), (6144, 0));
}

#[test]
fn empty_signatures_and_zero_parameter_signatures_remain_distinct() {
    let (shapes, sigs) = compare(&[]);
    assert!(shapes.is_empty() && sigs.is_empty());
    let (shapes, sigs) = compare(&[ChunkSignature::default(), ChunkSignature::default()]);
    assert_eq!(shapes.len(), 1);
    assert_eq!((sigs[0].params_first, sigs[1].params_first), (0, 1));
    assert!(
        sigs.iter()
            .all(|s| s.params_count == 0 && s.ret == 0 && s.resume == 0)
    );
}
