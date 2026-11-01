//! Semantic signature decoding preserves reference parsing and tag behavior.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{ChunkSignature, WireShape};
use keleusma::selfhost::{wire_decode_signature_metadata_via_kel, wire_decode_signatures_via_kel};
use keleusma::wire_schema::{self as schema, ShapeRecord, SignatureRecord, kind};
use keleusma_wire::{WireBuilder, WireRecord, WireView};

fn records<T: WireRecord>(values: &[T]) -> Vec<u8> {
    let mut bytes = vec![0; values.len() * T::STRIDE];
    for (value, out) in values.iter().zip(bytes.chunks_exact_mut(T::STRIDE)) {
        value.write_record(out).unwrap();
    }
    bytes
}

fn container(shapes: Option<&[u8]>, signatures: Option<&[u8]>, natives: Option<&[u8]>) -> Vec<u8> {
    let mut builder = WireBuilder::new();
    for (kind, bytes) in [
        (kind::SHAPES, shapes),
        (kind::SIGNATURES, signatures),
        (kind::NATIVE_RETURNS, natives),
    ] {
        if let Some(bytes) = bytes {
            let region = builder.region(kind, 0).unwrap();
            builder.push(region, bytes);
        }
    }
    builder.finish().unwrap()
}

#[test]
fn signature_semantics_preserve_kind_bytes_and_ignore_unused_shape_fields() {
    let shapes = records(&[
        ShapeRecord {
            tag: 1,
            kind: 255,
            reserved: 255,
            size: u32::MAX,
        },
        ShapeRecord {
            tag: 2,
            kind: 255,
            reserved: 255,
            size: u32::MAX,
        },
        ShapeRecord {
            tag: 3,
            kind: 255,
            reserved: 255,
            size: u32::MAX,
        },
        ShapeRecord {
            tag: 0,
            kind: 0,
            reserved: 0,
            size: 0,
        },
    ]);
    let signatures = records(&[SignatureRecord {
        params_first: 0,
        params_count: 3,
        ret: 1,
        resume: 2,
    }]);
    let bytes = container(Some(&shapes), Some(&signatures), None);
    let expected = schema::decode_signatures(&bytes).unwrap();
    assert_eq!(
        expected,
        vec![ChunkSignature {
            params: vec![
                WireShape::Top,
                WireShape::Scalar { kind: 255 },
                WireShape::Flat {
                    kind: 255,
                    size: u32::MAX
                }
            ],
            ret: WireShape::Scalar { kind: 255 },
            resume: WireShape::Flat {
                kind: 255,
                size: u32::MAX
            },
        }]
    );
    assert_eq!(wire_decode_signatures_via_kel(&bytes).unwrap(), expected);
    // The invalid fourth shape remains unused and must not reject the view.
    let bad = records(&[SignatureRecord {
        params_first: 3,
        params_count: 1,
        ret: 1,
        resume: 2,
    }]);
    let bytes = container(Some(&shapes), Some(&bad), None);
    assert!(schema::decode_signatures(&bytes).is_err());
    assert!(wire_decode_signatures_via_kel(&bytes).is_err());
}

fn rewrite(bytes: &[u8], replacements: &[(u16, Option<Vec<u8>>)]) -> Vec<u8> {
    let view = WireView::parse(bytes).unwrap();
    let mut builder = WireBuilder::new();
    for index in 0..view.region_count() {
        let region = view.region_at(index).unwrap();
        let replacement = replacements.iter().find(|(kind, _)| *kind == region.kind);
        let data = match replacement {
            Some((_, Some(bytes))) => bytes.as_slice(),
            Some((_, None)) => continue,
            None => view.region_bytes(&region).unwrap(),
        };
        let id = builder.region(region.kind, region.flags).unwrap();
        builder.push(id, data);
    }
    for (kind, replacement) in replacements {
        if let (None, Some(data)) = (view.find_region(*kind), replacement) {
            let id = builder.region(*kind, 0).unwrap();
            builder.push(id, data);
        }
    }
    builder.finish().unwrap()
}

#[test]
fn optional_table_failures_and_semantic_tag_errors_remain_distinct() {
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 7 }").unwrap())
            .unwrap();
    let mut module = keleusma::compiler::compile(&ast).unwrap();
    module.signatures = vec![ChunkSignature {
        params: vec![],
        ret: WireShape::Top,
        resume: WireShape::Top,
    }];
    let artifact = module.to_bytes().unwrap();
    let bytes = keleusma::wire_format::parse_wire_sections(&artifact)
        .unwrap()
        .aux_body
        .to_vec();
    let good_shapes = records(&[ShapeRecord {
        tag: 1,
        kind: 0,
        reserved: 0,
        size: 0,
    }]);
    let bad_shape = records(&[ShapeRecord {
        tag: 0,
        kind: 0,
        reserved: 0,
        size: 0,
    }]);
    let bad_bounds = records(&[SignatureRecord {
        params_first: 0,
        params_count: 0,
        ret: 1,
        resume: 0,
    }]);
    let native = records(&[schema::NativeReturnRecord {
        shape: 0,
        reserved: 0,
    }]);
    let cases = [
        (vec![(kind::SHAPES, None)], true),
        (
            vec![
                (kind::SHAPES, Some(good_shapes.clone())),
                (kind::SIGNATURES, Some(vec![0; 8])),
            ],
            true,
        ),
        (
            vec![
                (kind::SHAPES, Some(good_shapes.clone())),
                (kind::SIGNATURES, Some(bad_bounds.clone())),
            ],
            true,
        ),
        (
            vec![
                (kind::SHAPES, Some(bad_shape.clone())),
                (kind::SIGNATURES, Some(bad_bounds.clone())),
            ],
            true,
        ),
        (
            vec![
                (kind::SHAPES, Some(bad_shape.clone())),
                (
                    kind::SIGNATURES,
                    Some(records(&[
                        SignatureRecord {
                            params_first: 0,
                            params_count: 0,
                            ret: 0,
                            resume: 0,
                        },
                        SignatureRecord {
                            params_first: 0,
                            params_count: 0,
                            ret: 1,
                            resume: 0,
                        },
                    ])),
                ),
            ],
            true,
        ),
        (
            vec![
                (kind::SHAPES, Some(bad_shape.clone())),
                (
                    kind::SIGNATURES,
                    Some(records(&[
                        SignatureRecord {
                            params_first: 0,
                            params_count: 0,
                            ret: 1,
                            resume: 0,
                        },
                        SignatureRecord {
                            params_first: 0,
                            params_count: 0,
                            ret: 0,
                            resume: 0,
                        },
                    ])),
                ),
            ],
            true,
        ),
        (vec![(kind::SHAPES, Some(bad_shape.clone()))], false),
        (
            vec![
                (kind::SHAPES, Some(bad_shape.clone())),
                (kind::SIGNATURES, None),
                (kind::NATIVE_RETURNS, Some(native.clone())),
            ],
            false,
        ),
        (
            vec![
                (kind::SHAPES, None),
                (kind::NATIVE_RETURNS, Some(native.clone())),
            ],
            false,
        ),
        (
            vec![
                (kind::SHAPES, Some(good_shapes.clone())),
                (kind::SIGNATURES, Some(bad_bounds)),
                (kind::NATIVE_RETURNS, Some(native.clone())),
            ],
            false,
        ),
        (
            vec![
                (kind::SHAPES, Some(good_shapes)),
                (kind::SIGNATURES, None),
                (kind::NATIVE_RETURNS, Some(native)),
            ],
            true,
        ),
    ];
    for (index, (changes, accepts)) in cases.iter().enumerate() {
        let bytes = rewrite(&bytes, changes);
        let expected = schema::decode_aux_body(&bytes);
        assert_eq!(
            expected.is_ok(),
            *accepts,
            "reference case {index}: {expected:?}"
        );
        let actual = wire_decode_signature_metadata_via_kel(&bytes);
        match (actual, expected) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.signatures, b.signatures, "case {index}");
                assert_eq!(
                    a.native_return_shapes, b.native_return_shapes,
                    "case {index}"
                );
                assert_eq!(a.parse_valid, schema::SignatureTable::parse(&bytes).is_ok());
            }
            (Err(_), Err(_)) => {}
            (a, b) => panic!("case {index}: {a:?} versus {b:?}"),
        }
    }
}

#[test]
fn long_parameter_ranges_and_native_returns_preserve_order() {
    let params: Vec<_> = (0..3000)
        .map(|i| match i % 3 {
            0 => WireShape::Top,
            1 => WireShape::Scalar {
                kind: (i % 256) as u8,
            },
            _ => WireShape::Flat {
                kind: (i % 256) as u8,
                size: i,
            },
        })
        .collect();
    let expected = vec![ChunkSignature {
        params,
        ret: WireShape::Scalar { kind: 255 },
        resume: WireShape::Flat {
            kind: 255,
            size: u32::MAX,
        },
    }];
    let bytes = schema::encode_signatures(&expected).unwrap();
    assert_eq!(wire_decode_signatures_via_kel(&bytes).unwrap(), expected);
    let native = records(&[
        schema::NativeReturnRecord {
            shape: 2,
            reserved: u32::MAX,
        },
        schema::NativeReturnRecord {
            shape: 0,
            reserved: 255,
        },
        schema::NativeReturnRecord {
            shape: 1,
            reserved: 0,
        },
    ]);
    let bytes = rewrite(&bytes, &[(kind::NATIVE_RETURNS, Some(native))]);
    let decoded = wire_decode_signature_metadata_via_kel(&bytes).unwrap();
    assert!(decoded.parse_valid);
    assert_eq!(decoded.signatures, expected);
    assert_eq!(
        decoded.native_return_shapes,
        vec![
            WireShape::Flat { kind: 2, size: 2 },
            WireShape::Top,
            WireShape::Scalar { kind: 1 },
        ]
    );
    // A native error is outside the standalone signature reader's scope.
    let bad_native = records(&[schema::NativeReturnRecord {
        shape: u32::MAX,
        reserved: 0,
    }]);
    let bytes = rewrite(&bytes, &[(kind::NATIVE_RETURNS, Some(bad_native))]);
    assert_eq!(wire_decode_signatures_via_kel(&bytes).unwrap(), expected);
    assert!(wire_decode_signature_metadata_via_kel(&bytes).is_err());
}

#[test]
fn signature_table_presence_and_empty_range_boundaries_match_reference() {
    let shape = records(&[ShapeRecord {
        tag: 1,
        kind: 0,
        reserved: 0,
        size: 0,
    }]);
    let empty = Vec::new();
    let partial = vec![0; 8];
    let at_end = records(&[SignatureRecord {
        params_first: 1,
        params_count: 0,
        ret: 0,
        resume: 0,
    }]);
    let past_end = records(&[SignatureRecord {
        params_first: 2,
        params_count: 0,
        ret: 0,
        resume: 0,
    }]);
    let wide = records(&[SignatureRecord {
        params_first: u32::MAX,
        params_count: u32::MAX,
        ret: 0,
        resume: 0,
    }]);
    let mut accepted = 0;
    let mut refused = 0;
    for shapes in [None, Some(empty.as_slice()), Some(shape.as_slice())] {
        for signatures in [
            None,
            Some(empty.as_slice()),
            Some(partial.as_slice()),
            Some(at_end.as_slice()),
            Some(past_end.as_slice()),
            Some(wide.as_slice()),
        ] {
            let bytes = container(shapes, signatures, None);
            match (
                wire_decode_signatures_via_kel(&bytes),
                schema::decode_signatures(&bytes),
            ) {
                (Ok(a), Ok(b)) => {
                    assert_eq!(a, b);
                    accepted += 1;
                }
                (Err(_), Err(_)) => refused += 1,
                (a, b) => panic!("table boundary disagreement: {a:?} versus {b:?}"),
            }
        }
    }
    assert_eq!((accepted, refused), (5, 13));
}
