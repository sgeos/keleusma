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

// ---------------------------------------------------------------------------
// Struct templates, enum layouts, and the shared name interner.
// ---------------------------------------------------------------------------

use keleusma::bytecode::{EnumLayout, EnumVariantDisc, StructTemplate};
use keleusma::wire_schema::{
    LayoutTable, SchemaBuilder, decode_enum_layouts, decode_struct_templates, encode_layouts,
};

fn tmpl(type_name: &str, fields: &[&str]) -> StructTemplate {
    StructTemplate {
        type_name: type_name.into(),
        field_names: fields.iter().map(|s| (*s).into()).collect(),
    }
}

fn layout(type_name: &str, variants: &[(&str, i64)], min_payload: u32) -> EnumLayout {
    EnumLayout {
        type_name: type_name.into(),
        variants: variants
            .iter()
            .map(|(n, d)| EnumVariantDisc {
                name: (*n).into(),
                disc: *d,
            })
            .collect(),
        min_payload,
    }
}

#[test]
fn struct_templates_round_trip() {
    let want = vec![
        tmpl("Point", &["x", "y"]),
        tmpl("Empty", &[]),
        // Same field names as the first, which must NOT be shared into it --
        // each template's run has to stay contiguous.
        tmpl("Other", &["x", "y", "z"]),
    ];
    let bytes = encode_layouts(&want, &[]).unwrap();
    let back = decode_struct_templates(&bytes).unwrap();

    assert_eq!(back.len(), want.len());
    for (g, w) in back.iter().zip(&want) {
        assert_eq!(g.type_name, w.type_name);
        assert_eq!(g.field_names, w.field_names);
    }
}

#[test]
fn enum_layouts_round_trip_including_negative_discriminants() {
    let want = vec![
        layout("Colour", &[("Red", 0), ("Green", 1), ("Blue", 2)], 0),
        layout("Signed", &[("Neg", -9_000_000_000), ("Max", i64::MAX)], 48),
        layout("Nullary", &[], 0),
    ];
    let bytes = encode_layouts(&[], &want).unwrap();
    let back = decode_enum_layouts(&bytes).unwrap();

    assert_eq!(back.len(), want.len());
    for (g, w) in back.iter().zip(&want) {
        assert_eq!(g.type_name, w.type_name);
        assert_eq!(g.min_payload, w.min_payload);
        assert_eq!(g.variants.len(), w.variants.len());
        for (gv, wv) in g.variants.iter().zip(&w.variants) {
            assert_eq!(gv.name, wv.name);
            assert_eq!(gv.disc, wv.disc, "discriminant must survive");
        }
    }
}

#[test]
fn one_artifact_carries_every_region_and_shares_one_name_pool() {
    // The architectural point. Constants, templates and enum layouts all
    // reference names; the pool and name table are single regions and the
    // container rejects duplicate kinds, so the interner has to be shared. This
    // builds all of them into ONE artifact and reads each back.
    let mut b = SchemaBuilder::new();
    b.add_constants(&[ConstValue::Struct {
        type_name: "Shared".into(),
        fields: vec![("a".into(), ConstValue::Int(1))],
    }])
    .unwrap();
    b.add_signatures(&[sig(vec![WireShape::Top], WireShape::Top, WireShape::Top)])
        .unwrap();
    b.add_struct_templates(&[tmpl("Shared", &["a"])]).unwrap();
    b.add_enum_layouts(&[layout("E", &[("V", 1)], 8)]).unwrap();
    let bytes = b.finish().unwrap();

    // Every reader finds what it needs in the same artifact.
    let ct = ConstTable::parse(&bytes).unwrap();
    assert_eq!(ct.tag(0), Some(tag::STRUCT));
    let st = SignatureTable::parse(&bytes).unwrap();
    assert_eq!(st.len(), 1);
    let lt = LayoutTable::parse(&bytes).unwrap();
    assert_eq!(lt.template_count(), 1);
    assert_eq!(lt.layout_count(), 1);

    assert_eq!(
        decode_struct_templates(&bytes).unwrap()[0].type_name,
        "Shared"
    );
    assert_eq!(decode_enum_layouts(&bytes).unwrap()[0].type_name, "E");

    // "Shared" is mentioned by a constant AND a template. It must be interned
    // once, so both point at the same name index.
    let const_type = ct.struct_aux(0).unwrap().type_name;
    let tmpl_type = lt.template(0).unwrap().type_name;
    assert_eq!(
        const_type, tmpl_type,
        "a type name used twice must be stored once and comparable by index"
    );
    assert_eq!(lt.name_bytes(tmpl_type), Some(&b"Shared"[..]));
}

#[test]
fn field_and_variant_runs_are_contiguous() {
    let bytes = encode_layouts(
        &[tmpl("A", &["p", "q"]), tmpl("B", &["p"])],
        &[
            layout("E", &[("X", 1), ("Y", 2)], 0),
            layout("F", &[("Z", 3)], 0),
        ],
    )
    .unwrap();
    let t = LayoutTable::parse(&bytes).unwrap();

    assert_eq!(t.template_field_name(0, 0), Some(&b"p"[..]));
    assert_eq!(t.template_field_name(0, 1), Some(&b"q"[..]));
    assert_eq!(t.template_field_name(0, 2), None, "past the end");
    assert_eq!(t.template_field_name(1, 0), Some(&b"p"[..]));

    assert_eq!(t.layout_variant(0, 0), Some((&b"X"[..], 1)));
    assert_eq!(t.layout_variant(0, 1), Some((&b"Y"[..], 2)));
    assert_eq!(t.layout_variant(0, 2), None, "past the end");
    assert_eq!(t.layout_variant(1, 0), Some((&b"Z"[..], 3)));
    assert_eq!(t.layout_variant(9, 0), None, "no such layout");
}

#[test]
fn layout_names_alias_the_artifact() {
    // Same load-bearing property as the constant table's strings.
    let bytes = encode_layouts(&[tmpl("Aliased", &["f"])], &[]).unwrap();
    let t = LayoutTable::parse(&bytes).unwrap();
    let n = t.template_field_name(0, 0).unwrap();

    let base = bytes.as_ptr() as usize;
    let at = n.as_ptr() as usize;
    assert!(
        at >= base && at + n.len() <= base + bytes.len(),
        "name bytes must alias the artifact"
    );

    let copy = b"f".to_vec();
    let copy_at = copy.as_ptr() as usize;
    assert!(
        !(copy_at >= base && copy_at + copy.len() <= base + bytes.len()),
        "predicate must reject a copy, or the assertion is vacuous"
    );
}

#[test]
fn an_out_of_range_field_run_is_rejected() {
    let mut bytes = encode_layouts(&[tmpl("A", &["p"])], &[]).unwrap();
    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::STRUCT_TEMPLATES)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    let at = base + keleusma::wire_schema::StructTemplateRecord::OFFSET_FIELD_COUNT;
    bytes[at..at + 4].copy_from_slice(&999u32.to_le_bytes());
    assert_eq!(
        LayoutTable::parse(&bytes).unwrap_err(),
        SchemaError::BadIndex
    );
}

#[test]
fn an_out_of_range_variant_run_is_rejected() {
    let mut bytes = encode_layouts(&[], &[layout("E", &[("V", 1)], 0)]).unwrap();
    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::ENUM_LAYOUTS)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    let at = base + keleusma::wire_schema::EnumLayoutRecord::OFFSET_VARIANTS_COUNT;
    bytes[at..at + 4].copy_from_slice(&999u32.to_le_bytes());
    assert_eq!(
        LayoutTable::parse(&bytes).unwrap_err(),
        SchemaError::BadIndex
    );
}

#[test]
fn a_truncated_layout_artifact_is_rejected_at_every_length() {
    let bytes = encode_layouts(&[tmpl("A", &["p"])], &[layout("E", &[("V", 1)], 0)]).unwrap();
    for cut in 0..bytes.len() {
        assert!(
            LayoutTable::parse(&bytes[..cut]).is_err(),
            "truncation to {cut} must be rejected"
        );
    }
    assert!(LayoutTable::parse(&bytes).is_ok());
}

#[test]
fn layout_corruption_never_panics() {
    let bytes = encode_layouts(
        &[tmpl("A", &["p", "q"])],
        &[layout("E", &[("X", -1), ("Y", 2)], 16)],
    )
    .unwrap();
    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        if let Ok(t) = LayoutTable::parse(&m) {
            for i in 0..t.template_count().saturating_add(2) {
                let _ = t.template(i);
                for f in 0..4 {
                    let _ = t.template_field_name(i, f);
                }
            }
            for i in 0..t.layout_count().saturating_add(2) {
                let _ = t.layout(i);
                for v in 0..4 {
                    let _ = t.layout_variant(i, v);
                }
            }
            let _ = t.name_bytes(u32::MAX);
        }
        let _ = decode_struct_templates(&m);
        let _ = decode_enum_layouts(&m);
    }
}

// ---------------------------------------------------------------------------
// Multi-contributor constant pools.
// ---------------------------------------------------------------------------

use keleusma::wire_schema::decode_constant_pool;

#[test]
fn several_pools_share_one_table_with_disjoint_ranges() {
    // A module has one constant pool per chunk, so the table has to serve many
    // contributors. Each gets a contiguous run it can address as first + i.
    let pool_a = [ConstValue::Int(10), ConstValue::StaticStr("a".into())];
    let pool_b = [ConstValue::Tuple(vec![
        ConstValue::Int(20),
        ConstValue::Int(21),
    ])];
    let pool_c = [ConstValue::Byte(3)];

    let mut b = SchemaBuilder::new();
    let ra = b.add_constant_pool(&pool_a);
    let rb = b.add_constant_pool(&pool_b);
    let rc = b.add_constant_pool(&pool_c);
    let bytes = b.finish().unwrap();

    assert_eq!(ra, (0, 2));
    assert_eq!(rb, (2, 1));
    assert_eq!(rc, (3, 1));

    let back_a = decode_constant_pool(&bytes, ra).unwrap();
    let back_b = decode_constant_pool(&bytes, rb).unwrap();
    let back_c = decode_constant_pool(&bytes, rc).unwrap();

    assert_eq!(back_a.len(), 2);
    assert!(deep_eq(&back_a[0], &pool_a[0]));
    assert!(deep_eq(&back_a[1], &pool_a[1]));
    assert!(deep_eq(&back_b[0], &pool_b[0]));
    assert!(deep_eq(&back_c[0], &pool_c[0]));
}

#[test]
fn the_forward_ordering_invariant_survives_multiple_pools() {
    // Children are numbered after ALL roots, not after their own pool's roots.
    // If that were wrong, a later pool's root could occupy an index a child of
    // an earlier pool already claimed, and the reverse sweep would read a value
    // that had not been computed.
    let mut b = SchemaBuilder::new();
    b.add_constant_pool(&[ConstValue::Tuple(vec![
        ConstValue::Int(1),
        ConstValue::Int(2),
    ])]);
    b.add_constant_pool(&[ConstValue::Array(vec![ConstValue::Int(3)])]);
    b.add_constant_pool(&[ConstValue::Int(4)]);
    let bytes = b.finish().unwrap();

    let t = ConstTable::parse(&bytes).unwrap();
    for i in 0..t.len() {
        if let Some((first, n)) = t.range(i) {
            assert!(
                first as usize > i,
                "record {i} range starts at {first}, not forward"
            );
            assert!((first + n) as usize <= t.len(), "record {i} range overruns");
        }
    }
}

#[test]
fn a_pool_range_outside_the_table_is_rejected() {
    let mut b = SchemaBuilder::new();
    let r = b.add_constant_pool(&[ConstValue::Int(1)]);
    let bytes = b.finish().unwrap();

    assert!(decode_constant_pool(&bytes, r).is_ok());
    assert_eq!(
        decode_constant_pool(&bytes, (0, 99)).unwrap_err(),
        SchemaError::BadIndex
    );
    assert_eq!(
        decode_constant_pool(&bytes, (99, 1)).unwrap_err(),
        SchemaError::BadIndex
    );
    // An empty range anywhere in bounds is legal and yields nothing.
    assert_eq!(decode_constant_pool(&bytes, (0, 0)).unwrap().len(), 0);
}

#[test]
fn an_artifact_with_no_constants_emits_no_constant_regions() {
    // A layout-only artifact should not carry three empty constant regions, and
    // ConstTable should say so rather than reporting an empty table.
    let bytes = encode_layouts(&[tmpl("A", &["p"])], &[]).unwrap();
    assert!(LayoutTable::parse(&bytes).is_ok());
    assert!(
        ConstTable::parse(&bytes).is_err(),
        "absent is not the same as empty"
    );
}

#[test]
fn an_empty_pool_still_yields_a_usable_range() {
    let mut b = SchemaBuilder::new();
    let empty = b.add_constant_pool(&[]);
    let full = b.add_constant_pool(&[ConstValue::Int(7)]);
    let bytes = b.finish().unwrap();

    assert_eq!(empty, (0, 0));
    assert_eq!(full, (0, 1));
    assert_eq!(decode_constant_pool(&bytes, empty).unwrap().len(), 0);
    assert_eq!(decode_constant_pool(&bytes, full).unwrap().len(), 1);
}

// ---------------------------------------------------------------------------
// Data-segment layout.
// ---------------------------------------------------------------------------

use keleusma::bytecode::{
    DataLayout, DataSlot, PrivateCompositeSlot, SharedSlotLayout, SlotVisibility,
};
use keleusma::wire_schema::{DataLayoutTable, decode_data_layout, visibility_tag};

fn sample_layout() -> DataLayout {
    DataLayout {
        slots: vec![
            DataSlot {
                name: "buffer".into(),
                visibility: SlotVisibility::Shared,
            },
            DataSlot {
                name: "counter".into(),
                visibility: SlotVisibility::Private,
            },
            DataSlot {
                name: "state".into(),
                visibility: SlotVisibility::Private,
            },
        ],
        shared_layout: vec![SharedSlotLayout {
            offset: 4096,
            kind: 0x80 | 2,
            len: 64,
        }],
        private_composite_layout: vec![PrivateCompositeSlot {
            slot: 2,
            offset: 128,
        }],
        private_init: vec![ConstValue::Int(0), ConstValue::Unit],
    }
}

#[test]
fn a_data_layout_round_trips() {
    let want = sample_layout();
    let mut b = SchemaBuilder::new();
    b.add_data_layout(&want).unwrap();
    let bytes = b.finish().unwrap();

    let got = decode_data_layout(&bytes).unwrap().expect("layout present");

    assert_eq!(got.slots.len(), want.slots.len());
    for (g, w) in got.slots.iter().zip(&want.slots) {
        assert_eq!(g.name, w.name);
        assert_eq!(g.visibility, w.visibility);
    }
    assert_eq!(got.shared_layout, want.shared_layout);
    assert_eq!(got.private_composite_layout, want.private_composite_layout);
    assert_eq!(got.private_init.len(), want.private_init.len());
    for (g, w) in got.private_init.iter().zip(&want.private_init) {
        assert!(deep_eq(g, w));
    }
}

#[test]
fn an_absent_data_layout_is_none_not_empty() {
    // Region presence is what encodes Option<DataLayout>. A module with no data
    // block must read back as None, and a module with an EMPTY data block must
    // read back as Some -- collapsing them would lose a real distinction.
    let bytes = encode_layouts(&[tmpl("A", &["p"])], &[]).unwrap();
    assert!(
        decode_data_layout(&bytes).unwrap().is_none(),
        "absent → None"
    );
    assert!(DataLayoutTable::parse(&bytes).unwrap().is_none());

    let mut b = SchemaBuilder::new();
    b.add_data_layout(&DataLayout {
        slots: vec![],
        shared_layout: vec![],
        private_composite_layout: vec![],
        private_init: vec![],
    })
    .unwrap();
    let bytes = b.finish().unwrap();
    let got = decode_data_layout(&bytes).unwrap();
    assert!(got.is_some(), "empty → Some, not None");
    assert_eq!(got.unwrap().slots.len(), 0);
}

#[test]
fn private_init_shares_the_constant_table_with_chunk_pools() {
    // private_init is a forest of constant trees, so it goes through the same
    // shared table rather than a parallel copy of the flattening machinery.
    let mut b = SchemaBuilder::new();
    let chunk = b.add_constant_pool(&[ConstValue::StaticStr("chunk".into())]);
    b.add_data_layout(&DataLayout {
        slots: vec![DataSlot {
            name: "s".into(),
            visibility: SlotVisibility::Private,
        }],
        shared_layout: vec![],
        private_composite_layout: vec![],
        private_init: vec![ConstValue::Tuple(vec![ConstValue::Int(5)])],
    })
    .unwrap();
    let bytes = b.finish().unwrap();

    let t = DataLayoutTable::parse(&bytes).unwrap().unwrap();
    let init = t.private_init_range();
    assert_ne!(init, chunk, "the two pools must occupy different ranges");

    // Both readable from the one table.
    let chunk_vals = decode_constant_pool(&bytes, chunk).unwrap();
    let init_vals = decode_constant_pool(&bytes, init).unwrap();
    assert!(deep_eq(
        &chunk_vals[0],
        &ConstValue::StaticStr("chunk".into())
    ));
    assert!(deep_eq(
        &init_vals[0],
        &ConstValue::Tuple(vec![ConstValue::Int(5)])
    ));

    // And the forward-ordering invariant still holds with a nested initialiser.
    let ct = ConstTable::parse(&bytes).unwrap();
    for i in 0..ct.len() {
        if let Some((first, _)) = ct.range(i) {
            assert!(first as usize > i, "record {i} range not forward");
        }
    }
}

#[test]
fn slot_names_alias_the_artifact() {
    let mut b = SchemaBuilder::new();
    b.add_data_layout(&sample_layout()).unwrap();
    let bytes = b.finish().unwrap();
    let t = DataLayoutTable::parse(&bytes).unwrap().unwrap();

    let n = t.slot_name(0).unwrap();
    assert_eq!(n, b"buffer");
    let base = bytes.as_ptr() as usize;
    let at = n.as_ptr() as usize;
    assert!(at >= base && at + n.len() <= base + bytes.len());

    let copy = b"buffer".to_vec();
    let copy_at = copy.as_ptr() as usize;
    assert!(!(copy_at >= base && copy_at + copy.len() <= base + bytes.len()));
}

#[test]
fn an_unknown_visibility_tag_is_rejected() {
    // Zero is deliberately not a valid visibility, so a zeroed record cannot
    // read as a well-formed shared slot.
    assert_ne!(visibility_tag::SHARED, 0);
    assert_ne!(visibility_tag::PRIVATE, 0);

    let mut b = SchemaBuilder::new();
    b.add_data_layout(&sample_layout()).unwrap();
    let mut bytes = b.finish().unwrap();

    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::DATA_SLOTS)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    bytes[base + keleusma::wire_schema::DataSlotRecord::OFFSET_VISIBILITY] = 0;
    assert!(matches!(
        DataLayoutTable::parse(&bytes),
        Err(SchemaError::UnknownTag(0))
    ));
}

#[test]
fn every_data_record_is_one_word() {
    use keleusma::wire_schema::{DataInitRecord, PrivateCompositeRecord, SharedSlotRecord};
    assert_eq!(
        <keleusma::wire_schema::DataSlotRecord as WireRecord>::STRIDE,
        8
    );
    assert_eq!(<SharedSlotRecord as WireRecord>::STRIDE, 8);
    assert_eq!(<PrivateCompositeRecord as WireRecord>::STRIDE, 8);
    assert_eq!(<DataInitRecord as WireRecord>::STRIDE, 8);
}

#[test]
fn a_truncated_data_layout_is_rejected_at_every_length() {
    let mut b = SchemaBuilder::new();
    b.add_data_layout(&sample_layout()).unwrap();
    let bytes = b.finish().unwrap();
    for cut in 0..bytes.len() {
        assert!(
            DataLayoutTable::parse(&bytes[..cut]).is_err(),
            "truncation to {cut} must be rejected"
        );
    }
    assert!(DataLayoutTable::parse(&bytes).is_ok());
}

#[test]
fn data_layout_corruption_never_panics() {
    let mut b = SchemaBuilder::new();
    b.add_data_layout(&sample_layout()).unwrap();
    let bytes = b.finish().unwrap();

    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        if let Ok(Some(t)) = DataLayoutTable::parse(&m) {
            for i in 0..t.slot_count().saturating_add(2) {
                let _ = t.slot(i);
                let _ = t.slot_name(i);
            }
            for i in 0..t.shared_count().saturating_add(2) {
                let _ = t.shared_slot(i);
            }
            for i in 0..t.private_composite_count().saturating_add(2) {
                let _ = t.private_composite(i);
            }
            let _ = t.private_init_range();
        }
        let _ = decode_data_layout(&m);
    }
}

// ---------------------------------------------------------------------------
// Per-chunk ranges: struct templates and parameter types.
// ---------------------------------------------------------------------------

use keleusma::bytecode::TypeTag;
use keleusma::wire_schema::{ParamTypeTable, type_tag_byte, type_tag_from_byte};

#[test]
fn struct_templates_are_per_chunk_ranges() {
    // Templates are declared per chunk, so the table serves many contributors
    // exactly as the constant table does.
    let mut b = SchemaBuilder::new();
    let a = b.add_struct_template_pool(&[tmpl("A", &["x"]), tmpl("B", &["y", "z"])]);
    let c = b.add_struct_template_pool(&[tmpl("C", &["w"])]);
    let bytes = b.finish().unwrap();

    assert_eq!(a, (0, 2));
    assert_eq!(c, (2, 1));

    let t = LayoutTable::parse(&bytes).unwrap();
    assert_eq!(t.template_count(), 3);
    // Each chunk's run is addressable at first + i, and field-name runs stayed
    // contiguous through the deferred interning.
    assert_eq!(t.template_field_name(a.0 as usize, 0), Some(&b"x"[..]));
    assert_eq!(t.template_field_name(a.0 as usize + 1, 0), Some(&b"y"[..]));
    assert_eq!(t.template_field_name(a.0 as usize + 1, 1), Some(&b"z"[..]));
    assert_eq!(t.template_field_name(c.0 as usize, 0), Some(&b"w"[..]));
}

#[test]
fn a_module_with_templates_but_no_enums_still_parses() {
    // Absent and empty mean the SAME thing for templates and enum layouts --
    // unlike Option<DataLayout>, "no struct templates" has only one reading. A
    // reader that demanded the enum regions would reject a perfectly ordinary
    // module.
    let mut b = SchemaBuilder::new();
    b.add_struct_template_pool(&[tmpl("Only", &["f"])]);
    let bytes = b.finish().unwrap();

    let t = LayoutTable::parse(&bytes).unwrap();
    assert_eq!(t.template_count(), 1);
    assert_eq!(t.layout_count(), 0, "absent enum regions read as empty");
    assert_eq!(t.layout_variant(0, 0), None);
    assert_eq!(decode_enum_layouts(&bytes).unwrap().len(), 0);
}

#[test]
fn parameter_types_round_trip_as_a_byte_pool() {
    let chunk_a = [TypeTag::Word, TypeTag::Byte, TypeTag::Bool];
    let chunk_b = [TypeTag::Composite];

    let mut b = SchemaBuilder::new();
    let ra = b.add_param_types(&chunk_a);
    let rb = b.add_param_types(&chunk_b);
    let bytes = b.finish().unwrap();

    assert_eq!(ra, (0, 3));
    assert_eq!(rb, (3, 1));

    let t = ParamTypeTable::parse(&bytes).unwrap().expect("present");
    assert_eq!(t.tags(ra).unwrap(), chunk_a);
    assert_eq!(t.tags(rb).unwrap(), chunk_b);
    assert_eq!(t.tags((0, 0)).unwrap(), Vec::<TypeTag>::new());
}

#[test]
fn parameter_type_bytes_alias_the_artifact() {
    let mut b = SchemaBuilder::new();
    let r = b.add_param_types(&[TypeTag::Word, TypeTag::Float]);
    let bytes = b.finish().unwrap();
    let t = ParamTypeTable::parse(&bytes).unwrap().unwrap();

    let raw = t.tag_bytes(r).unwrap();
    let base = bytes.as_ptr() as usize;
    let at = raw.as_ptr() as usize;
    assert!(at >= base && at + raw.len() <= base + bytes.len());

    let copy = raw.to_vec();
    let copy_at = copy.as_ptr() as usize;
    assert!(!(copy_at >= base && copy_at + copy.len() <= base + bytes.len()));
}

#[test]
fn every_type_tag_round_trips_and_zero_is_invalid() {
    for t in [
        TypeTag::Composite,
        TypeTag::Byte,
        TypeTag::Word,
        TypeTag::Fixed,
        TypeTag::Float,
        TypeTag::Bool,
        TypeTag::Unit,
        TypeTag::Text,
    ] {
        let b = type_tag_byte(t);
        assert_ne!(b, 0, "a zeroed byte must not be a valid tag");
        assert_eq!(type_tag_from_byte(b), Some(t));
    }
    assert_eq!(type_tag_from_byte(0), None);
    assert_eq!(type_tag_from_byte(200), None);
}

#[test]
fn an_out_of_range_or_corrupt_param_range_is_reported() {
    let mut b = SchemaBuilder::new();
    let r = b.add_param_types(&[TypeTag::Word]);
    let mut bytes = b.finish().unwrap();

    let t = ParamTypeTable::parse(&bytes).unwrap().unwrap();
    assert!(t.tags(r).is_ok());
    assert_eq!(t.tags((0, 99)).unwrap_err(), SchemaError::BadIndex);
    assert_eq!(t.tags((99, 1)).unwrap_err(), SchemaError::BadIndex);

    // A corrupted tag byte is reported, not guessed.
    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::PARAM_TYPES)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    bytes[base] = 0;
    let t = ParamTypeTable::parse(&bytes).unwrap().unwrap();
    assert_eq!(t.tags(r).unwrap_err(), SchemaError::UnknownTag(0));
}

#[test]
fn an_absent_param_type_pool_is_none() {
    let bytes = encode_layouts(&[tmpl("A", &["p"])], &[]).unwrap();
    assert!(ParamTypeTable::parse(&bytes).unwrap().is_none());
}

// ---------------------------------------------------------------------------
// The chunk table, natives, header, and debug pool.
// ---------------------------------------------------------------------------

use keleusma::bytecode::BlockType;
use keleusma::wire_schema::{ABSENT, ChunkMeta, HeaderRecord, ModuleTable, block_tag};

fn meta(consts: (u32, u32), templates: (u32, u32), params: (u32, u32)) -> ChunkMeta {
    ChunkMeta {
        constants: consts,
        templates,
        param_types: params,
        local_count: 7,
        param_count: 2,
        block_type: BlockType::Stream,
        op_byte_offset: 64,
        op_record_count: 12,
    }
}

#[test]
fn a_whole_module_assembles_into_one_artifact() {
    // Everything stage 2b built, in one place: chunks referencing constants,
    // templates and parameter types by range, plus natives, header, and debug.
    let mut b = SchemaBuilder::new();

    let c0 = b.add_constant_pool(&[ConstValue::Int(1), ConstValue::StaticStr("s".into())]);
    let t0 = b.add_struct_template_pool(&[tmpl("P", &["x"])]);
    let p0 = b.add_param_types(&[TypeTag::Word, TypeTag::Bool]);
    b.add_chunk("main", &meta(c0, t0, p0), Some(b"dbg0"))
        .unwrap();

    let c1 = b.add_constant_pool(&[ConstValue::Byte(9)]);
    let t1 = b.add_struct_template_pool(&[]);
    let p1 = b.add_param_types(&[]);
    b.add_chunk("tick", &meta(c1, t1, p1), None).unwrap();

    b.add_natives(
        &["host::log".to_string(), "host::now".to_string()],
        &[WireShape::Scalar { kind: 3 }],
    )
    .unwrap();

    b.add_header(&HeaderRecord {
        entry_point: 0,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        flags: 0x01,
        wcet_cycles: 1234,
        wcmu_bytes: 5678,
        shared_data_bytes: 64,
        private_data_bytes: 32,
        schema_hash: 0xDEADBEEF,
        reserved: 0,
    })
    .unwrap();

    let bytes = b.finish().unwrap();
    let m = ModuleTable::parse(&bytes).unwrap();

    assert_eq!(m.chunk_count(), 2);
    assert_eq!(m.chunk_name(0), Some(&b"main"[..]));
    assert_eq!(m.chunk_name(1), Some(&b"tick"[..]));
    assert_eq!(m.chunk_block_type(0), Some(BlockType::Stream));

    // The chunk's ranges address the shared tables it actually wrote into.
    let ch = m.chunk(0).unwrap();
    let vals = decode_constant_pool(&bytes, (ch.consts_first, ch.consts_count)).unwrap();
    assert_eq!(vals.len(), 2);
    assert!(deep_eq(&vals[0], &ConstValue::Int(1)));

    let lt = LayoutTable::parse(&bytes).unwrap();
    assert_eq!(
        lt.template_field_name(ch.templates_first as usize, 0),
        Some(&b"x"[..])
    );

    let pt = ParamTypeTable::parse(&bytes).unwrap().unwrap();
    assert_eq!(
        pt.tags((ch.param_types_first, ch.param_types_count))
            .unwrap(),
        vec![TypeTag::Word, TypeTag::Bool]
    );

    // Header and entry point.
    let h = m.header().expect("header present");
    assert_eq!(h.wcet_cycles, 1234);
    assert_eq!(h.schema_hash, 0xDEADBEEF);
    assert_eq!(h.word_bits_log2, 6);
    assert_eq!(m.entry_point(), Some(0));

    // Natives, with the second lacking a described return shape.
    assert_eq!(m.native_count(), 2);
    assert_eq!(m.native_name(0), Some(&b"host::log"[..]));
    assert_ne!(m.native(0).unwrap().ret_shape, ABSENT);
    assert_eq!(m.native(1).unwrap().ret_shape, ABSENT);
}

#[test]
fn a_chunk_with_no_debug_metadata_differs_from_one_with_empty_metadata() {
    // Option<Vec<u8>> again: None is a release build, Some(empty) is a debug
    // build that happened to emit nothing. ABSENT keeps them apart.
    let mut b = SchemaBuilder::new();
    let r = (0, 0);
    b.add_chunk("none", &meta(r, r, r), None).unwrap();
    b.add_chunk("empty", &meta(r, r, r), Some(b"")).unwrap();
    b.add_chunk("some", &meta(r, r, r), Some(b"xyz")).unwrap();
    let bytes = b.finish().unwrap();

    let m = ModuleTable::parse(&bytes).unwrap();
    assert_eq!(m.chunk_debug_bytes(0), None, "None stays None");
    assert_eq!(
        m.chunk_debug_bytes(1),
        Some(&[][..]),
        "Some(empty) stays Some"
    );
    assert_eq!(m.chunk_debug_bytes(2), Some(&b"xyz"[..]));
    assert_eq!(m.chunk(0).unwrap().debug_first, ABSENT);
    assert_ne!(m.chunk(1).unwrap().debug_first, ABSENT);
}

#[test]
fn an_absent_entry_point_is_none() {
    let mut b = SchemaBuilder::new();
    b.add_header(&HeaderRecord {
        entry_point: ABSENT,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        flags: 0,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
        reserved: 0,
    })
    .unwrap();
    let bytes = b.finish().unwrap();
    let m = ModuleTable::parse(&bytes).unwrap();
    assert_eq!(m.entry_point(), None);
    assert!(m.header().is_some(), "the header is still present");
}

#[test]
fn chunk_and_native_names_alias_the_artifact() {
    let mut b = SchemaBuilder::new();
    let r = (0, 0);
    b.add_chunk("aliased", &meta(r, r, r), None).unwrap();
    b.add_natives(&["nat".to_string()], &[]).unwrap();
    let bytes = b.finish().unwrap();
    let m = ModuleTable::parse(&bytes).unwrap();

    let base = bytes.as_ptr() as usize;
    for slice in [m.chunk_name(0).unwrap(), m.native_name(0).unwrap()] {
        let at = slice.as_ptr() as usize;
        assert!(at >= base && at + slice.len() <= base + bytes.len());
    }
    let copy = b"aliased".to_vec();
    let copy_at = copy.as_ptr() as usize;
    assert!(!(copy_at >= base && copy_at + copy.len() <= base + bytes.len()));
}

#[test]
fn an_unknown_block_type_is_rejected() {
    assert_ne!(block_tag::FUNC, 0);
    let mut b = SchemaBuilder::new();
    let r = (0, 0);
    b.add_chunk("c", &meta(r, r, r), None).unwrap();
    let mut bytes = b.finish().unwrap();

    let base = {
        let view = WireView::parse(&bytes).unwrap();
        view.find_region(keleusma::wire_schema::kind::CHUNKS)
            .unwrap()
            .byte_offset()
            .unwrap()
    };
    bytes[base + keleusma::wire_schema::ChunkRecord::OFFSET_BLOCK_TYPE] = 0;
    assert!(matches!(
        ModuleTable::parse(&bytes),
        Err(SchemaError::UnknownTag(0))
    ));
}

#[test]
fn module_records_are_whole_words() {
    use keleusma::wire_schema::{ChunkRecord, NativeRecord};
    assert_eq!(<ChunkRecord as WireRecord>::STRIDE % 8, 0);
    assert_eq!(<NativeRecord as WireRecord>::STRIDE, 8);
    assert_eq!(<HeaderRecord as WireRecord>::STRIDE, 32);
}

#[test]
fn a_truncated_module_artifact_is_rejected_at_every_length() {
    let mut b = SchemaBuilder::new();
    let r = (0, 0);
    b.add_chunk("c", &meta(r, r, r), Some(b"d")).unwrap();
    b.add_natives(&["n".to_string()], &[WireShape::Top])
        .unwrap();
    let bytes = b.finish().unwrap();
    for cut in 0..bytes.len() {
        assert!(
            ModuleTable::parse(&bytes[..cut]).is_err(),
            "truncation to {cut} must be rejected"
        );
    }
    assert!(ModuleTable::parse(&bytes).is_ok());
}

#[test]
fn module_corruption_never_panics() {
    let mut b = SchemaBuilder::new();
    let r = (0, 0);
    b.add_chunk("c0", &meta(r, r, r), Some(b"dbg")).unwrap();
    b.add_chunk("c1", &meta(r, r, r), None).unwrap();
    b.add_natives(&["n0".to_string()], &[WireShape::Top])
        .unwrap();
    b.add_header(&HeaderRecord {
        entry_point: 0,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        flags: 0,
        wcet_cycles: 1,
        wcmu_bytes: 2,
        shared_data_bytes: 3,
        private_data_bytes: 4,
        schema_hash: 5,
        reserved: 0,
    })
    .unwrap();
    let bytes = b.finish().unwrap();

    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        if let Ok(t) = ModuleTable::parse(&m) {
            for i in 0..t.chunk_count().saturating_add(2) {
                let _ = t.chunk(i);
                let _ = t.chunk_name(i);
                let _ = t.chunk_block_type(i);
                let _ = t.chunk_debug_bytes(i);
            }
            for i in 0..t.native_count().saturating_add(2) {
                let _ = t.native(i);
                let _ = t.native_name(i);
            }
            let _ = t.header();
            let _ = t.entry_point();
            let _ = t.name_bytes(u32::MAX);
        }
    }
}

#[test]
fn signatures_and_natives_can_coexist() {
    // REGRESSION. Both contribute shapes, and SHAPES is a single region the
    // container will not let two callers declare. This collided for one
    // increment because the only test exercised natives WITHOUT signatures --
    // the same blind spot that hid the NAMES collision earlier.
    let mut b = SchemaBuilder::new();
    b.add_signatures(&[sig(
        vec![WireShape::Scalar { kind: 1 }],
        WireShape::Top,
        WireShape::Top,
    )])
    .unwrap();
    b.add_natives(
        &["n0".to_string(), "n1".to_string()],
        &[WireShape::Flat { kind: 2, size: 16 }],
    )
    .unwrap();
    let bytes = b.finish().unwrap();

    // Both readers see their own entries, resolved through the one shape table.
    let st = SignatureTable::parse(&bytes).unwrap();
    assert_eq!(st.len(), 1);
    assert_eq!(st.param_shape(0, 0), Some(WireShape::Scalar { kind: 1 }));
    assert_eq!(st.ret_shape(0), Some(WireShape::Top));

    let m = ModuleTable::parse(&bytes).unwrap();
    assert_eq!(m.native_count(), 2);
    let nat0 = m.native(0).unwrap();
    assert_ne!(nat0.ret_shape, ABSENT);
    assert_eq!(
        st.shape(nat0.ret_shape),
        Some(WireShape::Flat { kind: 2, size: 16 }),
        "a native's return shape resolves in the shared table"
    );
    assert_eq!(m.native(1).unwrap().ret_shape, ABSENT);
}

#[test]
fn every_add_method_can_be_called_together() {
    // The general form of the collision above: exercise every contributor in one
    // builder, so a future add_* that claims an already-claimed region fails
    // here rather than in whichever combination nobody tested.
    let mut b = SchemaBuilder::new();
    let c = b.add_constant_pool(&[ConstValue::Int(1)]);
    let t = b.add_struct_template_pool(&[tmpl("T", &["f"])]);
    let p = b.add_param_types(&[TypeTag::Word]);
    b.add_signatures(&[sig(vec![], WireShape::Top, WireShape::Top)])
        .unwrap();
    b.add_enum_layouts(&[layout("E", &[("V", 1)], 0)]).unwrap();
    b.add_data_layout(&sample_layout()).unwrap();
    b.add_chunk("c", &meta(c, t, p), Some(b"d")).unwrap();
    b.add_natives(&["n".to_string()], &[WireShape::Top])
        .unwrap();
    b.add_header(&HeaderRecord {
        entry_point: 0,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        flags: 0,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
        reserved: 0,
    })
    .unwrap();

    let bytes = b.finish().expect("every contributor must coexist");

    // Every reader finds its regions in the one artifact.
    assert!(ConstTable::parse(&bytes).is_ok());
    assert!(SignatureTable::parse(&bytes).is_ok());
    assert!(LayoutTable::parse(&bytes).is_ok());
    assert!(DataLayoutTable::parse(&bytes).unwrap().is_some());
    assert!(ParamTypeTable::parse(&bytes).unwrap().is_some());
    assert!(ModuleTable::parse(&bytes).is_ok());
}

// ---------------------------------------------------------------------------
// The whole auxiliary body: the first real consumer.
// ---------------------------------------------------------------------------

use keleusma::wire_format::{WireAuxBody, WireChunk};
use keleusma::wire_schema::{decode_aux_body, encode_aux_body};

fn assert_aux_eq(got: &WireAuxBody, want: &WireAuxBody) {
    assert_eq!(got.chunks.len(), want.chunks.len(), "chunk count");
    for (g, w) in got.chunks.iter().zip(&want.chunks) {
        assert_eq!(g.name, w.name);
        assert_eq!(g.local_count, w.local_count, "{}", w.name);
        assert_eq!(g.param_count, w.param_count, "{}", w.name);
        assert_eq!(g.block_type, w.block_type, "{}", w.name);
        assert_eq!(g.param_types, w.param_types, "{}", w.name);
        assert_eq!(g.op_byte_offset, w.op_byte_offset, "{}", w.name);
        assert_eq!(g.op_record_count, w.op_record_count, "{}", w.name);
        assert_eq!(g.debug_pool_bytes, w.debug_pool_bytes, "{}", w.name);
        assert_eq!(g.constants.len(), w.constants.len(), "{} constants", w.name);
        for (gc, wc) in g.constants.iter().zip(&w.constants) {
            assert!(deep_eq(gc, wc), "{}: {gc:?} vs {wc:?}", w.name);
        }
        assert_eq!(
            g.struct_templates.len(),
            w.struct_templates.len(),
            "{}",
            w.name
        );
        for (gt, wt) in g.struct_templates.iter().zip(&w.struct_templates) {
            assert_eq!(gt.type_name, wt.type_name);
            assert_eq!(gt.field_names, wt.field_names);
        }
    }
    assert_eq!(got.native_names, want.native_names);
    assert_eq!(got.native_return_shapes, want.native_return_shapes);
    assert_eq!(got.entry_point, want.entry_point);
    assert_eq!(got.word_bits_log2, want.word_bits_log2);
    assert_eq!(got.addr_bits_log2, want.addr_bits_log2);
    assert_eq!(got.float_bits_log2, want.float_bits_log2);
    assert_eq!(got.wcet_cycles, want.wcet_cycles);
    assert_eq!(got.wcmu_bytes, want.wcmu_bytes);
    assert_eq!(got.flags, want.flags);
    assert_eq!(got.shared_data_bytes, want.shared_data_bytes);
    assert_eq!(got.private_data_bytes, want.private_data_bytes);
    assert_eq!(got.schema_hash, want.schema_hash);
    assert_eq!(got.signatures.len(), want.signatures.len());
    for (g, w) in got.signatures.iter().zip(&want.signatures) {
        assert_eq!(g.params, w.params);
        assert_eq!(g.ret, w.ret);
        assert_eq!(g.resume, w.resume);
    }
    assert_eq!(got.enum_layouts.len(), want.enum_layouts.len());
    for (g, w) in got.enum_layouts.iter().zip(&want.enum_layouts) {
        assert_eq!(g.type_name, w.type_name);
        assert_eq!(g.min_payload, w.min_payload);
        assert_eq!(g.variants.len(), w.variants.len());
        for (gv, wv) in g.variants.iter().zip(&w.variants) {
            assert_eq!(gv.name, wv.name);
            assert_eq!(gv.disc, wv.disc, "discriminant must survive");
        }
    }
    assert_eq!(got.data_layout.is_some(), want.data_layout.is_some());
}

fn chunk(name: &str, constants: Vec<ConstValue>, params: Vec<TypeTag>) -> WireChunk {
    WireChunk {
        name: name.into(),
        constants,
        struct_templates: vec![tmpl("T", &["a", "b"])],
        local_count: 4,
        param_count: params.len() as u8,
        block_type: BlockType::Func,
        param_types: params,
        op_byte_offset: 32,
        op_record_count: 9,
        debug_pool_bytes: None,
    }
}

fn rich_aux() -> WireAuxBody {
    WireAuxBody {
        chunks: vec![
            chunk(
                "main",
                vec![
                    ConstValue::Int(-1),
                    ConstValue::StaticStr("hello".into()),
                    ConstValue::Struct {
                        type_name: "P".into(),
                        fields: vec![("x".into(), ConstValue::Byte(2))],
                    },
                ],
                vec![TypeTag::Word, TypeTag::Bool],
            ),
            chunk("tick", vec![], vec![]),
            WireChunk {
                debug_pool_bytes: Some(b"debug-bytes".to_vec()),
                block_type: BlockType::Stream,
                ..chunk("stream", vec![ConstValue::Unit], vec![TypeTag::Composite])
            },
        ],
        native_names: vec!["host::a".into(), "host::b".into()],
        entry_point: Some(0),
        data_layout: Some(sample_layout()),
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        wcet_cycles: 4242,
        wcmu_bytes: 8484,
        flags: 0x03,
        shared_data_bytes: 256,
        private_data_bytes: 128,
        schema_hash: 0x1234_5678,
        enum_layouts: vec![layout("E", &[("A", 0), ("B", -7)], 24)],
        signatures: vec![
            sig(
                vec![WireShape::Scalar { kind: 3 }],
                WireShape::Top,
                WireShape::Top,
            ),
            sig(vec![], WireShape::Flat { kind: 1, size: 8 }, WireShape::Top),
            sig(vec![], WireShape::Top, WireShape::Top),
        ],
        native_return_shapes: vec![WireShape::Scalar { kind: 1 }, WireShape::Top],
    }
}

#[test]
fn a_whole_aux_body_round_trips() {
    let want = rich_aux();
    let bytes = encode_aux_body(&want).expect("encode");
    let got = decode_aux_body(&bytes).expect("decode");
    assert_aux_eq(&got, &want);
}

#[test]
fn a_minimal_aux_body_round_trips() {
    // The degenerate case: no chunks, no natives, no data layout, no entry point.
    let want = WireAuxBody {
        chunks: vec![],
        native_names: vec![],
        entry_point: None,
        data_layout: None,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        flags: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
        enum_layouts: vec![],
        signatures: vec![],
        native_return_shapes: vec![],
    };
    let bytes = encode_aux_body(&want).expect("encode");
    let got = decode_aux_body(&bytes).expect("decode");
    assert_aux_eq(&got, &want);
    assert!(got.data_layout.is_none(), "absent stays absent");
    assert!(got.entry_point.is_none());
}

#[test]
fn per_chunk_ranges_do_not_bleed_between_chunks() {
    // The failure this whole range design exists to prevent: chunk 1 seeing
    // chunk 0's constants, templates, or parameter types.
    let want = WireAuxBody {
        chunks: vec![
            chunk("a", vec![ConstValue::Int(1)], vec![TypeTag::Word]),
            chunk(
                "b",
                vec![ConstValue::Int(2), ConstValue::Int(3)],
                vec![TypeTag::Bool, TypeTag::Byte],
            ),
            chunk("c", vec![], vec![]),
        ],
        ..rich_aux()
    };
    let bytes = encode_aux_body(&want).unwrap();
    let got = decode_aux_body(&bytes).unwrap();

    assert_eq!(got.chunks[0].constants.len(), 1);
    assert!(deep_eq(&got.chunks[0].constants[0], &ConstValue::Int(1)));
    assert_eq!(got.chunks[1].constants.len(), 2);
    assert!(deep_eq(&got.chunks[1].constants[0], &ConstValue::Int(2)));
    assert!(deep_eq(&got.chunks[1].constants[1], &ConstValue::Int(3)));
    assert_eq!(got.chunks[2].constants.len(), 0);

    assert_eq!(got.chunks[0].param_types, vec![TypeTag::Word]);
    assert_eq!(
        got.chunks[1].param_types,
        vec![TypeTag::Bool, TypeTag::Byte]
    );
    assert_eq!(got.chunks[2].param_types, Vec::<TypeTag>::new());

    // Each chunk gets its own template run, not the concatenation.
    for c in &got.chunks {
        assert_eq!(c.struct_templates.len(), 1, "{}", c.name);
        assert_eq!(c.struct_templates[0].field_names, vec!["a", "b"]);
    }
}

#[test]
fn the_aux_body_codec_is_total_under_truncation() {
    let bytes = encode_aux_body(&rich_aux()).unwrap();
    for cut in 0..bytes.len() {
        assert!(
            decode_aux_body(&bytes[..cut]).is_err(),
            "truncation to {cut} must be rejected"
        );
    }
    assert!(decode_aux_body(&bytes).is_ok());
}

#[test]
fn the_aux_body_codec_never_panics_under_corruption() {
    let bytes = encode_aux_body(&rich_aux()).unwrap();
    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        let _ = decode_aux_body(&m);
    }
}

// The lexer, parser and compiler live behind the `compile` feature, so this test
// cannot exist in a runtime-only build. Gated rather than deleted: the realistic
// corpus is worth having wherever the pipeline is present.
#[cfg(feature = "compile")]
#[test]
fn a_real_compiled_module_round_trips() {
    // Realistic data rather than hand-built: constants, templates, param types
    // and block types as the compiler actually emits them.
    let src = r#"
        struct P { x: Word, y: Word }
        fn helper(a: Word, b: Byte) -> Word { a + (b as Word) }
        fn make() -> P { P { x: 1, y: 2 } }
        fn main(n: Word) -> Word { n |> helper(3Byte) }
    "#;
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    let module = keleusma::compiler::compile(&program).expect("compile");

    // Assert what this corpus actually covers, so the test cannot quietly become
    // vacuous if the compiler stops emitting one of these. Measured, not assumed:
    // this program yields 3 chunks, 3 constants, 3 parameter types and 3
    // signatures -- but ZERO struct templates and ZERO natives, which are
    // exercised only by the hand-built `rich_aux` case above.
    let consts: usize = module.chunks.iter().map(|c| c.constants.len()).sum();
    let ptypes: usize = module.chunks.iter().map(|c| c.param_types.len()).sum();
    assert!(module.chunks.len() >= 3, "expected several chunks");
    assert!(consts >= 3, "expected real constants, got {consts}");
    assert!(ptypes >= 3, "expected real parameter types, got {ptypes}");
    assert!(!module.signatures.is_empty(), "expected real signatures");

    let want = WireAuxBody {
        chunks: module
            .chunks
            .iter()
            .map(|c| WireChunk {
                name: c.name.clone(),
                constants: c.constants.clone(),
                struct_templates: c.struct_templates.clone(),
                local_count: c.local_count,
                param_count: c.param_count,
                block_type: c.block_type,
                param_types: c.param_types.clone(),
                op_byte_offset: 0,
                op_record_count: c.ops.len() as u32,
                debug_pool_bytes: None,
            })
            .collect(),
        native_names: module.native_names.clone(),
        entry_point: module.entry_point,
        data_layout: module.data_layout.clone(),
        word_bits_log2: module.word_bits_log2,
        addr_bits_log2: module.addr_bits_log2,
        float_bits_log2: module.float_bits_log2,
        wcet_cycles: module.wcet_cycles,
        wcmu_bytes: module.wcmu_bytes,
        flags: module.flags,
        shared_data_bytes: module.shared_data_bytes,
        private_data_bytes: module.private_data_bytes,
        schema_hash: module.schema_hash,
        enum_layouts: module.enum_layouts.clone(),
        signatures: module.signatures.clone(),
        native_return_shapes: module.native_return_shapes.clone(),
    };

    let bytes = encode_aux_body(&want).expect("encode");
    let got = decode_aux_body(&bytes).expect("decode");
    assert_aux_eq(&got, &want);
}

// ---------------------------------------------------------------------------
// AuxView: the runtime's read surface.
// ---------------------------------------------------------------------------

use keleusma::wire_schema::AuxView;

#[test]
fn the_runtime_view_serves_chunk_relative_indices() {
    // The VM addresses a chunk's constants from zero, not from the shared
    // table's base. Getting that mapping wrong would have each chunk reading
    // whichever constants happen to sit at its indices -- a wrong answer, not a
    // fault, since the reads would all be in bounds.
    let want = WireAuxBody {
        chunks: vec![
            chunk("a", vec![ConstValue::Int(10), ConstValue::Int(11)], vec![]),
            chunk("b", vec![ConstValue::Int(20)], vec![]),
            chunk("c", vec![], vec![]),
        ],
        ..rich_aux()
    };
    let bytes = encode_aux_body(&want).unwrap();
    let v = AuxView::parse(&bytes).unwrap();

    assert_eq!(v.chunk_count(), 3);
    assert_eq!(v.const_count(0), Some(2));
    assert_eq!(v.const_count(1), Some(1));
    assert_eq!(v.const_count(2), Some(0));

    // Chunk-relative index 0 means each chunk's OWN first constant.
    assert_eq!(v.const_record(0, 0).unwrap().payload as i64, 10);
    assert_eq!(v.const_record(0, 1).unwrap().payload as i64, 11);
    assert_eq!(v.const_record(1, 0).unwrap().payload as i64, 20);

    // A chunk cannot reach past its own pool into the next one's.
    assert!(
        v.const_record(0, 2).is_none(),
        "chunk 0 has only 2 constants"
    );
    assert!(v.const_record(1, 1).is_none(), "chunk 1 has only 1");
    assert!(v.const_record(2, 0).is_none(), "chunk 2 has none");
    assert!(v.const_record(99, 0).is_none(), "no such chunk");
}

#[test]
fn the_runtime_view_aliases_string_constants() {
    // The load-bearing property, at the granularity the VM actually uses.
    let want = WireAuxBody {
        chunks: vec![
            chunk("a", vec![ConstValue::Int(1)], vec![]),
            chunk(
                "b",
                vec![ConstValue::StaticStr("aliased-here".into())],
                vec![],
            ),
        ],
        ..rich_aux()
    };
    let bytes = encode_aux_body(&want).unwrap();
    let v = AuxView::parse(&bytes).unwrap();

    let s = v.chunk_const_str_bytes(1, 0).expect("chunk 1 constant 0");
    assert_eq!(s, b"aliased-here");

    let base = bytes.as_ptr() as usize;
    let at = s.as_ptr() as usize;
    assert!(
        at >= base && at + s.len() <= base + bytes.len(),
        "string bytes must alias the artifact"
    );

    // Control: the predicate must reject a copy, or the assertion proves nothing.
    let copy = b"aliased-here".to_vec();
    let copy_at = copy.as_ptr() as usize;
    assert!(!(copy_at >= base && copy_at + copy.len() <= base + bytes.len()));

    // Kind and range are checked, not guessed.
    assert!(
        v.chunk_const_str_bytes(0, 0).is_none(),
        "an Int is not a string"
    );
    assert!(v.chunk_const_str_bytes(1, 1).is_none(), "out of range");
}

#[test]
fn the_runtime_view_serves_every_read_the_vm_makes() {
    // Enumerated from the archived call sites in src/vm.rs: per-chunk constants,
    // struct templates and local_count; the word and float widths; schema hash;
    // shared data bytes; the data layout; and the enum layouts.
    let want = rich_aux();
    let bytes = encode_aux_body(&want).unwrap();
    let v = AuxView::parse(&bytes).unwrap();

    assert_eq!(v.local_count(0), Some(want.chunks[0].local_count));
    assert!(v.const_count(0).is_some());
    assert_eq!(v.template_count(0), Some(1));
    assert_eq!(v.template_type_name(0, 0), Some(&b"T"[..]));
    assert_eq!(v.template_field_name(0, 0, 0), Some(&b"a"[..]));
    assert_eq!(v.template_field_name(0, 0, 1), Some(&b"b"[..]));
    assert!(
        v.template_field_name(0, 0, 2).is_none(),
        "past the field run"
    );

    assert_eq!(v.word_bits_log2(), Some(want.word_bits_log2));
    assert_eq!(v.float_bits_log2(), Some(want.float_bits_log2));
    assert_eq!(v.schema_hash(), Some(want.schema_hash));
    assert_eq!(v.shared_data_bytes(), Some(want.shared_data_bytes));

    assert_eq!(v.enum_layout_count(), want.enum_layouts.len());
    assert_eq!(v.enum_type_name(0), Some(&b"E"[..]));
    assert_eq!(v.enum_variant(0, 1), Some((&b"B"[..], -7)));

    let dl = v.data_layout().expect("data layout present");
    assert_eq!(dl.slot_count(), sample_layout().slots.len());
    assert_eq!(dl.slot_name(0), Some(&b"buffer"[..]));
}

#[test]
fn the_runtime_view_parses_a_module_with_no_constants() {
    // A module carrying no constants emits no constant regions. That is absence,
    // not an error, and the view must still parse.
    let mut b = SchemaBuilder::new();
    b.add_chunk("only", &meta((0, 0), (0, 0), (0, 0)), None)
        .unwrap();
    b.add_header(&HeaderRecord {
        entry_point: ABSENT,
        word_bits_log2: 6,
        addr_bits_log2: 6,
        float_bits_log2: 6,
        flags: 0,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
        reserved: 0,
    })
    .unwrap();
    let bytes = b.finish().unwrap();

    let v = AuxView::parse(&bytes).expect("must parse without constant regions");
    assert_eq!(v.chunk_count(), 1);
    assert_eq!(v.const_count(0), Some(0));
    assert!(v.const_record(0, 0).is_none());
    assert!(v.chunk_const_str_bytes(0, 0).is_none());
    assert!(v.data_layout().is_none());
}

#[test]
fn the_runtime_view_never_panics_under_corruption() {
    let bytes = encode_aux_body(&rich_aux()).unwrap();
    for pos in 0..bytes.len() {
        let mut m = bytes.clone();
        m[pos] ^= 0x80;
        if let Ok(v) = AuxView::parse(&m) {
            for c in 0..v.chunk_count().saturating_add(2) {
                let _ = v.local_count(c);
                let _ = v.const_count(c);
                let _ = v.template_count(c);
                for i in 0..4 {
                    let _ = v.const_record(c, i);
                    let _ = v.chunk_const_str_bytes(c, i);
                    let _ = v.template_type_name(c, i);
                    let _ = v.template_field_name(c, i, 0);
                }
            }
            let _ = v.word_bits_log2();
            let _ = v.schema_hash();
            for i in 0..v.enum_layout_count().saturating_add(2) {
                let _ = v.enum_type_name(i);
                let _ = v.enum_variant(i, 0);
            }
            let _ = v.data_layout();
        }
    }
}
