//! Wire format v2, stage 1: the flattened constant table.
//!
//! The round-trip tests establish that the flattening is lossless. The ordering
//! tests establish the property the flattening exists to create — that a
//! composite's range lies strictly after it, which is what lets the table be
//! walked bottom-up with no stack.

use keleusma::bytecode::ConstValue;
use keleusma::wire_schema::{
    ConstRecord, ConstTable, SchemaError, decode_constants, encode_constants, kind, tag,
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

// ---------------------------------------------------------------------------
// The borrowed accessor. The runtime reads through this; `decode_constants` is
// the tooling path.
// ---------------------------------------------------------------------------

#[test]
fn a_string_constant_aliases_the_artifact_rather_than_copying() {
    // THE load-bearing property. A probe of the live runtime showed a non-empty
    // top-level string constant is loaded by minting a handle directly over the
    // image's bytes; that is only possible if the accessor hands back a slice
    // INTO the artifact. A test comparing values would pass just as well against
    // an owned copy, so this asserts by ADDRESS.
    let bytes =
        encode_constants(&[ConstValue::StaticStr("aliased".into()), ConstValue::Int(1)]).unwrap();

    let t = ConstTable::parse(&bytes).unwrap();
    let s = t.str_bytes(0).expect("string constant");
    assert_eq!(s, b"aliased");

    let base = bytes.as_ptr() as usize;
    let at = s.as_ptr() as usize;
    assert!(
        at >= base && at + s.len() <= base + bytes.len(),
        "string bytes must alias the artifact, not be copied out of it"
    );

    // And as `&str`, still borrowed.
    let as_str = t.str(0).unwrap();
    assert_eq!(as_str, "aliased");
    assert_eq!(as_str.as_ptr() as usize, at);

    // Control: the address predicate must actually discriminate. An owned copy
    // of the same bytes has the same VALUE and a different address, so if this
    // predicate accepted it the test above would prove nothing.
    let copy = b"aliased".to_vec();
    let copy_at = copy.as_ptr() as usize;
    assert!(
        !(copy_at >= base && copy_at + copy.len() <= base + bytes.len()),
        "predicate must reject a copy, or the aliasing assertion is vacuous"
    );
}

#[test]
fn str_bytes_reports_kind_rather_than_guessing() {
    let bytes = encode_constants(&[
        ConstValue::Int(5),
        ConstValue::StaticStr("x".into()),
        ConstValue::Tuple(vec![ConstValue::Int(1)]),
    ])
    .unwrap();
    let t = ConstTable::parse(&bytes).unwrap();

    assert!(t.str_bytes(0).is_none(), "an Int is not a string");
    assert_eq!(t.str_bytes(1), Some(&b"x"[..]));
    assert!(t.str_bytes(2).is_none(), "a Tuple is not a string");
    assert!(t.str_bytes(999).is_none(), "out of range");
}

#[test]
fn an_empty_string_constant_yields_an_empty_slice_not_none() {
    // The runtime deliberately does NOT alias an empty string, so that it need
    // not rest on a non-null guarantee for a zero-length pointer. That is the
    // runtime's decision; the accessor's job is to report the bytes faithfully,
    // and an empty string is a string.
    let bytes = encode_constants(&[ConstValue::StaticStr(String::new())]).unwrap();
    let t = ConstTable::parse(&bytes).unwrap();
    assert_eq!(t.str_bytes(0), Some(&[][..]));
    assert_eq!(t.str(0), Some(""));
}

#[test]
fn scalar_and_range_accessors_agree_with_the_owned_decode() {
    // The two readers must not drift. They share a parse path now; this pins
    // that they also agree on what they report.
    let roots = [
        ConstValue::Int(-77),
        ConstValue::Byte(9),
        ConstValue::Bool(true),
        ConstValue::Array(vec![ConstValue::Int(1), ConstValue::Int(2)]),
    ];
    let bytes = encode_constants(&roots).unwrap();
    let t = ConstTable::parse(&bytes).unwrap();

    assert_eq!(t.tag(0), Some(tag::INT));
    assert_eq!(t.payload(0).map(|p| p as i64), Some(-77));
    assert_eq!(t.tag(1), Some(tag::BYTE));
    assert_eq!(t.payload(1).map(|p| p as u8), Some(9));
    assert_eq!(t.tag(2), Some(tag::BOOL));
    assert_eq!(t.payload(2), Some(1));

    let (first, n) = t.range(3).expect("array has a range");
    assert_eq!(n, 2);
    assert!(first as usize > 3, "range must point forward");
    assert_eq!(t.payload(first as usize).map(|p| p as i64), Some(1));
    assert_eq!(t.payload(first as usize + 1).map(|p| p as i64), Some(2));

    // A scalar has no range.
    assert!(t.range(0).is_none());

    let owned = decode_constants(&bytes, roots.len()).unwrap();
    assert_eq!(owned.len(), 4);
}

#[test]
fn composite_side_tables_are_reachable_without_materialising() {
    let roots = [
        ConstValue::Struct {
            type_name: "P".into(),
            fields: vec![
                ("x".into(), ConstValue::Int(1)),
                ("y".into(), ConstValue::Int(2)),
            ],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "V".into(),
            discriminant: Some(3),
            fields: vec![],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "W".into(),
            discriminant: None,
            fields: vec![],
        },
    ];
    let bytes = encode_constants(&roots).unwrap();
    let t = ConstTable::parse(&bytes).unwrap();

    let sa = t.struct_aux(0).expect("struct aux");
    assert_eq!(t.name_bytes(sa.type_name), Some(&b"P"[..]));
    assert_eq!(t.name_bytes(sa.field_names_first), Some(&b"x"[..]));
    assert_eq!(t.name_bytes(sa.field_names_first + 1), Some(&b"y"[..]));

    let (ea, disc) = t.enum_aux(1).expect("enum aux");
    assert_eq!(t.name_bytes(ea.type_name), Some(&b"E"[..]));
    assert_eq!(t.name_bytes(ea.variant), Some(&b"V"[..]));
    assert_eq!(disc, Some(3));

    // `None` must not read back as `Some(0)`.
    let (_, disc) = t.enum_aux(2).expect("enum aux");
    assert_eq!(disc, None);

    // Kind checks, not guesses.
    assert!(t.struct_aux(1).is_none(), "an enum is not a struct");
    assert!(t.enum_aux(0).is_none(), "a struct is not an enum");
}

#[test]
fn the_accessor_rejects_a_backwards_range_at_parse_time() {
    // Same malformed input the owned decoder rejects. Validating once at parse
    // is what lets every later accessor be total without re-checking.
    let roots = [
        ConstValue::Int(1),
        ConstValue::Tuple(vec![ConstValue::Int(2), ConstValue::Int(3)]),
    ];
    let mut bytes = encode_constants(&roots).unwrap();
    let (base, stride) = {
        let t = ConstTable::parse(&bytes).unwrap();
        let _ = t.len();
        let view = keleusma_wire::WireView::parse(&bytes).unwrap();
        let region = view.find_region(kind::CONSTS).unwrap();
        (
            region.byte_offset().unwrap(),
            <ConstRecord as WireRecord>::STRIDE,
        )
    };
    let at = base + stride + ConstRecord::OFFSET_PAYLOAD;
    // (first = 0, count = 2). `first` is genuinely zero -- that is the defect
    // being injected -- so the pack is written out rather than folded away.
    let first: u32 = 0;
    let count: u32 = 2;
    let packed = first as u64 | ((count as u64) << 32);
    bytes[at..at + 8].copy_from_slice(&packed.to_le_bytes());

    assert_eq!(
        ConstTable::parse(&bytes).unwrap_err(),
        SchemaError::BadRange
    );
}

#[test]
fn the_accessor_is_total_on_a_truncated_artifact() {
    let bytes = encode_constants(&[ConstValue::StaticStr("abc".into())]).unwrap();
    for cut in 0..bytes.len() {
        assert!(
            ConstTable::parse(&bytes[..cut]).is_err(),
            "truncation to {cut} must be rejected"
        );
    }
    assert!(ConstTable::parse(&bytes).is_ok());
}

#[test]
fn the_accessor_never_panics_under_corruption() {
    let bytes = encode_constants(&[
        ConstValue::Struct {
            type_name: "S".into(),
            fields: vec![("f".into(), ConstValue::StaticStr("v".into()))],
        },
        ConstValue::Array(vec![ConstValue::Int(1)]),
    ])
    .unwrap();

    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        if let Ok(t) = ConstTable::parse(&m) {
            for i in 0..t.len().saturating_add(2) {
                let _ = t.tag(i);
                let _ = t.payload(i);
                let _ = t.range(i);
                let _ = t.str_bytes(i);
                let _ = t.str(i);
                let _ = t.struct_aux(i);
                let _ = t.enum_aux(i);
            }
            let _ = t.name_bytes(u32::MAX);
        }
    }
}

// ---------------------------------------------------------------------------
// Shapes and signatures.
// ---------------------------------------------------------------------------

use keleusma::bytecode::{ChunkSignature, WireShape};
use keleusma::wire_schema::{
    ShapeRecord, SignatureRecord, SignatureTable, decode_signatures, encode_signatures, shape_tag,
};

fn sig(params: Vec<WireShape>, ret: WireShape, resume: WireShape) -> ChunkSignature {
    ChunkSignature {
        params,
        ret,
        resume,
    }
}

#[test]
fn signatures_round_trip_across_every_shape_variant() {
    let want = vec![
        sig(
            vec![
                WireShape::Top,
                WireShape::Scalar { kind: 3 },
                WireShape::Flat { kind: 2, size: 48 },
            ],
            WireShape::Scalar { kind: 1 },
            WireShape::Top,
        ),
        sig(vec![], WireShape::Top, WireShape::Top),
        sig(
            vec![WireShape::Flat {
                kind: 7,
                size: u32::MAX,
            }],
            WireShape::Flat { kind: 7, size: 0 },
            WireShape::Scalar { kind: 255 },
        ),
    ];

    let bytes = encode_signatures(&want).unwrap();
    let back = decode_signatures(&bytes).unwrap();
    assert_eq!(back.len(), want.len());
    for (g, w) in back.iter().zip(&want) {
        assert_eq!(g.params, w.params);
        assert_eq!(g.ret, w.ret);
        assert_eq!(g.resume, w.resume);
    }
}

#[test]
fn parameter_runs_are_contiguous_and_singles_are_shared() {
    // The two admission modes. A parameter run must be addressable as
    // `params_first + i`, so it cannot share; `ret`/`resume` may share, and
    // `Top` dominates real modules so the sharing is worth having.
    let sigs = vec![
        sig(
            vec![WireShape::Top, WireShape::Top],
            WireShape::Top,
            WireShape::Top,
        ),
        sig(vec![WireShape::Top], WireShape::Top, WireShape::Top),
    ];
    let bytes = encode_signatures(&sigs).unwrap();
    let t = SignatureTable::parse(&bytes).unwrap();

    // Contiguity: each parameter is reachable at its own offset.
    for (i, s) in sigs.iter().enumerate() {
        for p in 0..s.params.len() {
            assert_eq!(
                t.param_shape(i, p),
                Some(WireShape::Top),
                "sig {i} param {p}"
            );
        }
        assert_eq!(t.param_shape(i, s.params.len()), None, "past the end");
    }

    // Sharing: three parameter entries are appended unshared, and the singles
    // reuse an existing `Top` rather than adding four more.
    let view = WireView::parse(&bytes).unwrap();
    let region = view
        .find_region(keleusma::wire_schema::kind::SHAPES)
        .unwrap();
    let shapes = view.typed_records::<ShapeRecord>(&region).unwrap();
    assert_eq!(
        shapes.len(),
        3,
        "3 unshared parameter entries, singles shared onto one of them"
    );

    let r0 = t.record(0).unwrap();
    let r1 = t.record(1).unwrap();
    assert_eq!(r0.params_first, 0);
    assert_eq!(r0.params_count, 2);
    assert_eq!(r1.params_first, 2);
    assert_eq!(r1.params_count, 1);
}

#[test]
fn distinct_shapes_are_not_collapsed() {
    // Sharing must key on the whole record, not just the tag: two `Flat`s of
    // different size are different shapes.
    let sigs = vec![sig(
        vec![],
        WireShape::Flat { kind: 1, size: 8 },
        WireShape::Flat { kind: 1, size: 16 },
    )];
    let bytes = encode_signatures(&sigs).unwrap();
    let t = SignatureTable::parse(&bytes).unwrap();
    assert_eq!(t.ret_shape(0), Some(WireShape::Flat { kind: 1, size: 8 }));
    assert_eq!(
        t.resume_shape(0),
        Some(WireShape::Flat { kind: 1, size: 16 })
    );
    assert_ne!(t.record(0).unwrap().ret, t.record(0).unwrap().resume);
}

#[test]
fn an_empty_signature_table_is_legal() {
    let bytes = encode_signatures(&[]).unwrap();
    let t = SignatureTable::parse(&bytes).unwrap();
    assert!(t.is_empty());
    assert_eq!(t.record(0), None);
    assert_eq!(decode_signatures(&bytes).unwrap().len(), 0);
}

#[test]
fn a_shape_record_is_one_word() {
    // Fixed-size records are the format's premise; a shape that outgrew a word
    // would need a side table like struct and enum constants do.
    assert_eq!(<ShapeRecord as WireRecord>::STRIDE, 8);
    assert_eq!(<SignatureRecord as WireRecord>::STRIDE, 16);
}

#[test]
fn an_out_of_range_parameter_run_is_rejected() {
    let sigs = vec![sig(vec![WireShape::Top], WireShape::Top, WireShape::Top)];
    let mut bytes = encode_signatures(&sigs).unwrap();
    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::SIGNATURES)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    let at = base + SignatureRecord::OFFSET_PARAMS_COUNT;
    bytes[at..at + 4].copy_from_slice(&999u32.to_le_bytes());
    assert_eq!(
        SignatureTable::parse(&bytes).unwrap_err(),
        SchemaError::BadIndex
    );
}

#[test]
fn an_out_of_range_single_shape_is_rejected() {
    let sigs = vec![sig(vec![], WireShape::Top, WireShape::Top)];
    let mut bytes = encode_signatures(&sigs).unwrap();
    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::SIGNATURES)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    let at = base + SignatureRecord::OFFSET_RET;
    bytes[at..at + 4].copy_from_slice(&50u32.to_le_bytes());
    assert_eq!(
        SignatureTable::parse(&bytes).unwrap_err(),
        SchemaError::BadIndex
    );
}

#[test]
fn an_unknown_shape_tag_is_reported_rather_than_guessed() {
    let sigs = vec![sig(vec![WireShape::Top], WireShape::Top, WireShape::Top)];
    let mut bytes = encode_signatures(&sigs).unwrap();
    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::SHAPES)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    let at = base + ShapeRecord::OFFSET_TAG;
    bytes[at..at + 2].copy_from_slice(&77u16.to_le_bytes());

    // The table still parses -- bounds are fine -- but the shape does not decode.
    let t = SignatureTable::parse(&bytes).unwrap();
    assert_eq!(t.shape(0), None);
    assert!(decode_signatures(&bytes).is_err());
}

#[test]
fn a_zeroed_shape_record_is_not_a_valid_top() {
    // Tags start at one precisely so a zeroed region does not read as a
    // well-formed table of "shape unknown" entries.
    assert_ne!(shape_tag::TOP, 0);
    let zeroed = ShapeRecord {
        tag: 0,
        kind: 0,
        reserved: 0,
        size: 0,
    };
    assert_eq!(zeroed.to_shape(), None);
}

#[test]
fn a_truncated_signature_artifact_is_rejected_at_every_length() {
    let sigs = vec![sig(
        vec![WireShape::Scalar { kind: 1 }],
        WireShape::Top,
        WireShape::Top,
    )];
    let bytes = encode_signatures(&sigs).unwrap();
    for cut in 0..bytes.len() {
        assert!(
            SignatureTable::parse(&bytes[..cut]).is_err(),
            "truncation to {cut} must be rejected"
        );
    }
    assert!(SignatureTable::parse(&bytes).is_ok());
}

#[test]
fn signature_corruption_never_panics() {
    let sigs = vec![
        sig(
            vec![WireShape::Top, WireShape::Flat { kind: 1, size: 24 }],
            WireShape::Scalar { kind: 2 },
            WireShape::Top,
        ),
        sig(vec![], WireShape::Top, WireShape::Top),
    ];
    let bytes = encode_signatures(&sigs).unwrap();
    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        if let Ok(t) = SignatureTable::parse(&m) {
            for i in 0..t.len().saturating_add(2) {
                let _ = t.record(i);
                let _ = t.ret_shape(i);
                let _ = t.resume_shape(i);
                for p in 0..4 {
                    let _ = t.param_shape(i, p);
                }
            }
            let _ = t.shape(u32::MAX);
        }
        let _ = decode_signatures(&m);
    }
}
