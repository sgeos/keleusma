//! Wire format v2, stage 1: the flattened constant table.
//!
//! The round-trip tests establish that the flattening is lossless. The ordering
//! tests establish the property the flattening exists to create — that a
//! composite's range lies strictly after it, which is what lets the table be
//! walked bottom-up with no stack.

use keleusma::bytecode::ConstValue;
use keleusma::wire_schema::{
    ConstRecord, SchemaError, decode_constants, encode_constants, kind, tag,
};
use keleusma_wire::{WireRecord, WireView};

/// Structural equality INCLUDING the enum discriminant.
///
/// `ConstValue`'s own `PartialEq` deliberately ignores `discriminant` (the `..`
/// in its `Enum` arm), so `assert_eq!` on a round trip is **blind** to whether
/// the discriminant survived. Every enum round-trip test here would have passed
/// with the field dropped entirely. This comparison exists so they do not.
fn deep_eq(a: &ConstValue, b: &ConstValue) -> bool {
    match (a, b) {
        (
            ConstValue::Enum {
                type_name: na,
                variant: va,
                discriminant: da,
                fields: fa,
            },
            ConstValue::Enum {
                type_name: nb,
                variant: vb,
                discriminant: db,
                fields: fb,
            },
        ) => {
            na == nb
                && va == vb
                && da == db
                && fa.len() == fb.len()
                && fa.iter().zip(fb).all(|(x, y)| deep_eq(x, y))
        }
        (ConstValue::Tuple(x), ConstValue::Tuple(y))
        | (ConstValue::Array(x), ConstValue::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| deep_eq(p, q))
        }
        (
            ConstValue::Struct {
                type_name: na,
                fields: fa,
            },
            ConstValue::Struct {
                type_name: nb,
                fields: fb,
            },
        ) => {
            na == nb
                && fa.len() == fb.len()
                && fa
                    .iter()
                    .zip(fb)
                    .all(|((kn, kv), (ln, lv))| kn == ln && deep_eq(kv, lv))
        }
        _ => a == b,
    }
}

fn round_trip(roots: &[ConstValue]) {
    let bytes = encode_constants(roots).expect("encode");
    let back = decode_constants(&bytes, roots.len()).expect("decode");
    assert_eq!(back.len(), roots.len());
    for (got, want) in back.iter().zip(roots) {
        assert!(
            deep_eq(got, want),
            "round trip differed\n got: {got:?}\nwant: {want:?}"
        );
    }
}

#[test]
fn scalars_round_trip() {
    round_trip(&[
        ConstValue::Unit,
        ConstValue::None,
        ConstValue::Bool(true),
        ConstValue::Bool(false),
        ConstValue::Int(0),
        ConstValue::Int(-1),
        ConstValue::Int(i64::MIN),
        ConstValue::Int(i64::MAX),
        ConstValue::Byte(0),
        ConstValue::Byte(255),
        ConstValue::Fixed(-12345),
        ConstValue::StaticStr("hello".into()),
        ConstValue::StaticStr(String::new()),
    ]);
}

#[cfg(feature = "floats")]
#[test]
fn floats_round_trip_bit_exactly() {
    // Through bits, not through a decimal round trip, so a signalling NaN or a
    // negative zero survives rather than being normalised.
    let cases = [0.0f64, -0.0, 1.5, -2.25, f64::MIN, f64::MAX, f64::INFINITY];
    for f in cases {
        let bytes = encode_constants(&[ConstValue::Float(f)]).unwrap();
        let back = decode_constants(&bytes, 1).unwrap();
        match back[0] {
            ConstValue::Float(g) => assert_eq!(g.to_bits(), f.to_bits(), "{f}"),
            ref other => panic!("expected Float, got {other:?}"),
        }
    }
}

#[test]
fn composites_round_trip() {
    round_trip(&[
        ConstValue::Tuple(vec![ConstValue::Int(1), ConstValue::Bool(true)]),
        ConstValue::Array(vec![
            ConstValue::Byte(1),
            ConstValue::Byte(2),
            ConstValue::Byte(3),
        ]),
        ConstValue::Tuple(vec![]),
        ConstValue::Array(vec![]),
    ]);
}

#[test]
fn nesting_is_lossless_at_depth() {
    // Depth is the case the superseded encoding needed a recursion cap for. Here
    // it is just more table entries, so the only thing that could break is the
    // ordering -- which the decoder re-validates.
    let mut v = ConstValue::Int(7);
    for _ in 0..64 {
        v = ConstValue::Tuple(vec![v, ConstValue::Byte(1)]);
    }
    round_trip(&[v]);
}

#[test]
fn structs_round_trip_including_repeated_field_names() {
    // Field names are interned WITHOUT sharing so a struct's names stay
    // contiguous for `field_names_first + i` addressing. Two structs sharing
    // field names is exactly the case where sharing would have broken the run.
    round_trip(&[
        ConstValue::Struct {
            type_name: "Point".into(),
            fields: vec![
                ("x".into(), ConstValue::Int(1)),
                ("y".into(), ConstValue::Int(2)),
            ],
        },
        ConstValue::Struct {
            type_name: "Point".into(),
            fields: vec![
                ("x".into(), ConstValue::Int(3)),
                ("y".into(), ConstValue::Int(4)),
            ],
        },
        ConstValue::Struct {
            type_name: "Empty".into(),
            fields: vec![],
        },
    ]);
}

#[test]
fn enums_round_trip_and_none_is_not_zero() {
    // `discriminant: None` must not decode as `Some(0)`. The flag carries that
    // distinction; without it the two would be indistinguishable on the wire.
    round_trip(&[
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "A".into(),
            discriminant: Some(0),
            fields: vec![],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "B".into(),
            discriminant: None,
            fields: vec![],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "C".into(),
            discriminant: Some(-5),
            fields: vec![ConstValue::Int(9), ConstValue::Bool(true)],
        },
    ]);

    // `Some(0)` and `None` must stay distinguishable. Compared by DESTRUCTURING,
    // not by `!=`: `ConstValue`'s `PartialEq` ignores the discriminant, so `!=`
    // reports these equal and the test would fail for a reason that has nothing
    // to do with the wire format.
    let src = [
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "A".into(),
            discriminant: Some(0),
            fields: vec![],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "A".into(),
            discriminant: None,
            fields: vec![],
        },
    ];
    let back = decode_constants(&encode_constants(&src).unwrap(), 2).unwrap();
    let disc = |c: &ConstValue| match c {
        ConstValue::Enum { discriminant, .. } => *discriminant,
        other => panic!("expected Enum, got {other:?}"),
    };
    assert_eq!(disc(&back[0]), Some(0));
    assert_eq!(disc(&back[1]), None, "Some(0) must not collapse into None");
}

#[test]
fn a_mixed_tree_round_trips() {
    round_trip(&[ConstValue::Struct {
        type_name: "Outer".into(),
        fields: vec![
            (
                "items".into(),
                ConstValue::Array(vec![
                    ConstValue::Enum {
                        type_name: "E".into(),
                        variant: "V".into(),
                        discriminant: Some(2),
                        fields: vec![ConstValue::StaticStr("deep".into())],
                    },
                    ConstValue::Tuple(vec![ConstValue::Int(1), ConstValue::Unit]),
                ]),
            ),
            ("tag".into(), ConstValue::Byte(7)),
        ],
    }]);
}

#[test]
fn roots_keep_their_positions() {
    // A chunk indexes its constants by position, so roots must occupy 0..n in
    // order regardless of how children are numbered around them.
    let roots = [
        ConstValue::Tuple(vec![ConstValue::Int(10)]),
        ConstValue::Int(20),
        ConstValue::Array(vec![ConstValue::Int(30), ConstValue::Int(40)]),
    ];
    let bytes = encode_constants(&roots).unwrap();
    let back = decode_constants(&bytes, 3).unwrap();
    assert_eq!(back[1], ConstValue::Int(20));
    assert_eq!(&back, &roots);
}

/// Reads the constant table out of an encoded artifact.
fn const_records(bytes: &[u8]) -> Vec<ConstRecord> {
    let view = WireView::parse(bytes).unwrap();
    let region = view.find_region(kind::CONSTS).unwrap();
    let table = view.typed_records::<ConstRecord>(&region).unwrap();
    (0..table.len())
        .map(|i| table.get_as::<ConstRecord>(i).unwrap())
        .collect()
}

#[test]
fn every_composite_range_points_strictly_forward() {
    // The invariant, checked on real encoder output rather than assumed from the
    // construction. This is what makes the reverse sweep valid.
    let roots = [
        ConstValue::Struct {
            type_name: "S".into(),
            fields: vec![(
                "a".into(),
                ConstValue::Array(vec![
                    ConstValue::Tuple(vec![ConstValue::Int(1), ConstValue::Int(2)]),
                    ConstValue::Int(3),
                ]),
            )],
        },
        ConstValue::Tuple(vec![ConstValue::Byte(1)]),
    ];
    let records = const_records(&encode_constants(&roots).unwrap());
    assert!(records.len() > roots.len(), "children must be emitted");

    for (i, r) in records.iter().enumerate() {
        if r.is_composite() {
            let (first, n) = r.as_range();
            assert!(
                first as usize > i,
                "record {i} references range starting at {first}, which is not forward"
            );
            assert!(
                (first + n) as usize <= records.len(),
                "record {i} range overruns the table"
            );
        }
    }
}

#[test]
fn a_backwards_range_is_rejected_rather_than_silently_miswalked() {
    // The failure this format's ordering invariant exists to prevent. Hand-build
    // a table whose composite points BACKWARDS and require rejection: under a
    // reverse sweep an unchecked decoder would read an uncomputed entry and
    // return a wrong answer rather than faulting.
    let roots = [
        ConstValue::Int(1),
        ConstValue::Tuple(vec![ConstValue::Int(2), ConstValue::Int(3)]),
    ];
    let mut bytes = encode_constants(&roots).unwrap();

    // Locate the constant region's payload and rewrite record 1's range to point
    // at index 0, which precedes it.
    let (base, stride) = {
        let view = WireView::parse(&bytes).unwrap();
        let region = view.find_region(kind::CONSTS).unwrap();
        (
            region.byte_offset().unwrap(),
            <ConstRecord as WireRecord>::STRIDE,
        )
    };
    let at = base + stride + ConstRecord::OFFSET_PAYLOAD;
    // (first = 0, count = 2). Written as an explicit pack so the intent is
    // visible; `first` is genuinely zero, which is the whole point.
    let first: u32 = 0;
    let count: u32 = 2;
    let packed = first as u64 | ((count as u64) << 32);
    bytes[at..at + 8].copy_from_slice(&packed.to_le_bytes());

    assert_eq!(
        decode_constants(&bytes, 2).unwrap_err(),
        SchemaError::BadRange
    );
}

#[test]
fn a_range_overrunning_the_table_is_rejected() {
    let roots = [ConstValue::Tuple(vec![ConstValue::Int(1)])];
    let mut bytes = encode_constants(&roots).unwrap();
    let (base, _stride) = {
        let view = WireView::parse(&bytes).unwrap();
        let region = view.find_region(kind::CONSTS).unwrap();
        (region.byte_offset().unwrap(), 0)
    };
    let at = base + ConstRecord::OFFSET_PAYLOAD;
    // first = 1 (forward), count = 999 (past the end)
    let packed = 1u64 | (999u64 << 32);
    bytes[at..at + 8].copy_from_slice(&packed.to_le_bytes());

    assert_eq!(
        decode_constants(&bytes, 1).unwrap_err(),
        SchemaError::BadRange
    );
}

#[test]
fn an_unknown_tag_is_rejected_rather_than_guessed() {
    let mut bytes = encode_constants(&[ConstValue::Int(1)]).unwrap();
    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(kind::CONSTS)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    let at = base + ConstRecord::OFFSET_TAG;
    bytes[at..at + 2].copy_from_slice(&999u16.to_le_bytes());

    assert_eq!(
        decode_constants(&bytes, 1).unwrap_err(),
        SchemaError::UnknownTag(999)
    );
}

#[test]
fn a_truncated_artifact_is_rejected_at_every_length() {
    let bytes = encode_constants(&[ConstValue::Tuple(vec![
        ConstValue::Int(1),
        ConstValue::StaticStr("x".into()),
    ])])
    .unwrap();
    for cut in 0..bytes.len() {
        assert!(
            decode_constants(&bytes[..cut], 1).is_err(),
            "truncation to {cut} must be rejected"
        );
    }
    assert!(decode_constants(&bytes, 1).is_ok());
}

#[test]
fn asking_for_more_roots_than_exist_is_rejected() {
    let bytes = encode_constants(&[ConstValue::Int(1)]).unwrap();
    assert_eq!(
        decode_constants(&bytes, 99).unwrap_err(),
        SchemaError::BadIndex
    );
}

#[test]
fn corruption_anywhere_never_panics() {
    let bytes = encode_constants(&[
        ConstValue::Struct {
            type_name: "S".into(),
            fields: vec![("f".into(), ConstValue::Array(vec![ConstValue::Int(1)]))],
        },
        ConstValue::StaticStr("abc".into()),
    ])
    .unwrap();

    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        // Any outcome is acceptable except a panic or a hang.
        let _ = decode_constants(&m, 2);
    }
}

#[test]
fn the_float_tag_is_reserved_even_without_the_feature() {
    // A floats-built artifact read by a no-floats build must fail loudly rather
    // than being misread as some other tag.
    assert_eq!(tag::FLOAT, 6);
    #[cfg(not(feature = "floats"))]
    {
        let mut bytes = encode_constants(&[ConstValue::Int(1)]).unwrap();
        let base = {
            let view = WireView::parse(&bytes).unwrap();
            view.find_region(kind::CONSTS)
                .unwrap()
                .byte_offset()
                .unwrap()
        };
        let at = base + ConstRecord::OFFSET_TAG;
        bytes[at..at + 2].copy_from_slice(&tag::FLOAT.to_le_bytes());
        assert_eq!(
            decode_constants(&bytes, 1).unwrap_err(),
            SchemaError::UnknownTag(tag::FLOAT)
        );
    }
}
