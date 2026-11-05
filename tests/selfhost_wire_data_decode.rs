//! Data-layout codec checks are distinct from later verifier invariants.
//!
//! Every case decodes one artifact through the self-hosted decoder and the reference
//! decoder and requires the two to agree, on the decoded layout when both accept and on the
//! refusal when either refuses. Encoding F, chosen by the operator on 2026-10-31, gives the
//! decoders one `DATA_FIELDS` record per declared field to validate.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{
    ConstValue, DataField, DataInit, DataLayout, MAX_DATA_ADDR, SHARED_SLOT_COMPOSITE_FLAG,
};
use keleusma::selfhost::wire_decode_data_layout_via_kel;
use keleusma::wire_schema::{self as schema, ABSENT, DataFieldRecord, NameRef, init_form, kind};
use keleusma_wire::{WireBuilder, WireRecord};

fn bytes<T: WireRecord>(values: &[T]) -> Vec<u8> {
    let mut result = vec![0; values.len() * T::STRIDE];
    for (value, out) in values.iter().zip(result.chunks_exact_mut(T::STRIDE)) {
        value.write_record(out).unwrap();
    }
    result
}
/// An artifact of one name, `text`, an optional field table and an optional retired region.
fn raw(fields: Option<&[u8]>, retired: Option<u16>, text: &[u8]) -> Vec<u8> {
    let mut b = WireBuilder::new();
    let r = b.region(kind::NAMES, 0).unwrap();
    b.push_record(
        r,
        &NameRef {
            offset: 0,
            length: text.len() as u32,
        },
    );
    let r = b.region(kind::STRING_POOL, 0).unwrap();
    b.push(r, text);
    if let Some(data) = fields {
        let r = b.region(kind::DATA_FIELDS, 0).unwrap();
        b.push(r, data);
    }
    if let Some(k) = retired {
        let r = b.region(k, 0).unwrap();
        b.push(r, &[0; 8]);
    }
    b.finish().unwrap()
}
fn equal(a: &DataLayout, b: &DataLayout) {
    assert_eq!(a, b);
    // `ConstValue`'s equality skips an enum discriminant; the encoded form does not.
    for (x, y) in a.fields.iter().zip(&b.fields) {
        if let (DataInit::Const(p), DataInit::Const(q)) = (&x.init, &y.init) {
            assert_eq!(
                schema::encode_constants(core::slice::from_ref(p)).unwrap(),
                schema::encode_constants(core::slice::from_ref(q)).unwrap()
            );
        }
    }
}
fn compare(data: &[u8]) -> bool {
    match (
        wire_decode_data_layout_via_kel(data),
        schema::decode_data_layout(data),
    ) {
        (Ok(Some(a)), Ok(Some(b))) => {
            equal(&a, &b);
            true
        }
        (Ok(None), Ok(None)) => true,
        (Err(_), Err(_)) => false,
        (a, b) => panic!("data layout mismatch {a:?} versus {b:?}"),
    }
}
/// A shared scalar field record both decoders accept: name 0, two elements eight bytes apart.
fn base() -> DataFieldRecord {
    DataFieldRecord {
        name: 0,
        first_slot: 0,
        count: 2,
        offset: 0,
        stride: 8,
        elem_len: 0,
        kind: 3,
        visibility: 1,
        init: init_form::ZERO,
        reserved0: 0,
        reserved1: 0,
        init_const: ABSENT,
    }
}

#[test]
fn fields_and_initializer_forests_match_reference() {
    let layout = DataLayout {
        fields: vec![
            DataField::shared("λ", 0, 31, SHARED_SLOT_COMPOSITE_FLAG | 2, 7, 0, 9),
            DataField::private(
                "p.init",
                31,
                4,
                SHARED_SLOT_COMPOSITE_FLAG,
                DataInit::Const(ConstValue::Tuple(vec![
                    ConstValue::StaticStr("init".into()),
                    ConstValue::Byte(7),
                ])),
            ),
            DataField {
                offset: Some(u32::MAX - 16),
                elem_len: 16,
                ..DataField::private(
                    "p.pool",
                    35,
                    1,
                    SHARED_SLOT_COMPOSITE_FLAG | 2,
                    DataInit::Unit,
                )
            },
            DataField::private("p.zero", 36, 9, 2, DataInit::Zero),
        ],
    };
    let mut b = schema::SchemaBuilder::new();
    b.add_data_layout(&layout).unwrap();
    let data = b.finish().unwrap();
    equal(
        wire_decode_data_layout_via_kel(&data)
            .unwrap()
            .as_ref()
            .unwrap(),
        &layout,
    );
    assert!(compare(&data));
}

#[test]
fn absent_empty_and_retired_regions_preserve_scope() {
    let data = raw(None, None, b"x");
    assert!(compare(&data));
    assert!(wire_decode_data_layout_via_kel(&data).unwrap().is_none());
    let data = raw(Some(&[]), None, b"x");
    assert!(compare(&data));
    assert!(
        wire_decode_data_layout_via_kel(&data)
            .unwrap()
            .unwrap()
            .fields
            .is_empty()
    );
    // A retired region is refused beside an absent field table and beside a present one,
    // because its absence from the new reader would otherwise read a module that has a data
    // block as one without.
    let good = bytes(&[base()]);
    for retired in kind::RETIRED_DATA_LAYOUT {
        assert!(!compare(&raw(None, Some(retired), b"x")), "{retired:#06x}");
        assert!(
            !compare(&raw(Some(&good), Some(retired), b"x")),
            "{retired:#06x}"
        );
    }
}

#[test]
fn field_validation_matches_reference() {
    let b = base();
    let private = DataFieldRecord {
        visibility: 2,
        offset: ABSENT,
        stride: 0,
        ..b
    };
    for (record, text, accepts) in [
        (b, &b"x"[..], true),
        (private, &b"x"[..], true),
        (
            DataFieldRecord {
                init: init_form::UNIT,
                ..private
            },
            &b"x"[..],
            true,
        ),
        (DataFieldRecord { count: 0, ..b }, &b"x"[..], false),
        (DataFieldRecord { visibility: 0, ..b }, &b"x"[..], false),
        (DataFieldRecord { visibility: 3, ..b }, &b"x"[..], false),
        (DataFieldRecord { name: 1, ..b }, &b"x"[..], false),
        (DataFieldRecord { reserved0: 1, ..b }, &b"x"[..], false),
        (DataFieldRecord { reserved1: 1, ..b }, &b"x"[..], false),
        (DataFieldRecord { first_slot: 1, ..b }, &b"x"[..], false),
        (
            DataFieldRecord {
                count: MAX_DATA_ADDR,
                ..b
            },
            &b"x"[..],
            true,
        ),
        (
            DataFieldRecord {
                count: MAX_DATA_ADDR + 1,
                ..b
            },
            &b"x"[..],
            false,
        ),
        (
            DataFieldRecord {
                offset: ABSENT,
                ..b
            },
            &b"x"[..],
            false,
        ),
        (
            DataFieldRecord {
                count: 1,
                stride: 8,
                ..b
            },
            &b"x"[..],
            false,
        ),
        (
            DataFieldRecord {
                offset: u32::MAX - 8,
                ..b
            },
            &b"x"[..],
            true,
        ),
        (
            DataFieldRecord {
                offset: u32::MAX - 7,
                ..b
            },
            &b"x"[..],
            false,
        ),
        (DataFieldRecord { elem_len: 4, ..b }, &b"x"[..], false),
        (
            DataFieldRecord {
                kind: SHARED_SLOT_COMPOSITE_FLAG | 2,
                elem_len: 4,
                ..b
            },
            &b"x"[..],
            true,
        ),
        (
            DataFieldRecord {
                kind: SHARED_SLOT_COMPOSITE_FLAG | 4,
                ..b
            },
            &b"x"[..],
            false,
        ),
        (DataFieldRecord { kind: 8, ..b }, &b"x"[..], false),
        (
            DataFieldRecord {
                init: init_form::UNIT,
                ..b
            },
            &b"x"[..],
            false,
        ),
        (DataFieldRecord { init: 3, ..private }, &b"x"[..], false),
        (DataFieldRecord { kind: 6, ..private }, &b"x"[..], false),
        (
            DataFieldRecord {
                init_const: 0,
                ..private
            },
            &b"x"[..],
            false,
        ),
        (
            DataFieldRecord {
                init: init_form::CONST,
                init_const: 0,
                ..private
            },
            &b"x"[..],
            false,
        ),
        (
            DataFieldRecord {
                stride: 8,
                ..private
            },
            &b"x"[..],
            false,
        ),
        (b, &b"\xff"[..], false),
    ] {
        let data = raw(Some(&bytes(&[record])), None, text);
        assert_eq!(compare(&data), accepts, "{record:?}");
    }
    // A shared field after a private one breaks the prefix.
    let after = DataFieldRecord { first_slot: 2, ..b };
    assert!(!compare(&raw(Some(&bytes(&[private, after])), None, b"x")));
    assert!(compare(&raw(
        Some(&bytes(&[
            b,
            DataFieldRecord {
                first_slot: 2,
                ..private
            }
        ])),
        None,
        b"x"
    )));
}

#[test]
fn decoded_layout_drives_shared_access_and_persistent_initialization() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    let source = "shared data io { values: [Word; 3] } private data d { value: Word = 7 } fn main(i: Word) -> Word { d.value = d.value + io.values[i]; d.value }";
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    let mut module = keleusma::compiler::compile(&ast).unwrap();
    let artifact = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
    let auxiliary = keleusma::wire_format::parse_wire_sections(&artifact)
        .unwrap()
        .aux_body;
    let expected = module.data_layout.take().unwrap();
    let decoded = wire_decode_data_layout_via_kel(auxiliary).unwrap().unwrap();
    equal(&decoded, &expected);
    module.data_layout = Some(decoded);
    let persistent = required_persistent_capacity_for(&module);
    let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
    arena.resize_persistent(persistent).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 2, Value::Int(5)).unwrap();
    for expected in [12, 17] {
        assert!(
            matches!(vm.call_with_shared(&mut shared,&[Value::Int(2)]).unwrap(),VmState::Finished(Value::Int(n)) if n==expected)
        );
    }
}

#[test]
fn maximal_counts_and_many_fields_decode_alike() {
    // One field spanning the whole slot space, and three hundred one-element fields, which
    // cross the stage's transport batches.
    let wide = DataFieldRecord {
        count: MAX_DATA_ADDR,
        stride: 1,
        kind: 2,
        ..base()
    };
    let data = raw(Some(&bytes(&[wide])), None, b"x");
    assert!(compare(&data));
    let layout = wire_decode_data_layout_via_kel(&data).unwrap().unwrap();
    assert_eq!(layout.slot_count(), MAX_DATA_ADDR as usize);
    assert_eq!(
        layout
            .shared_slot(MAX_DATA_ADDR as usize - 1)
            .unwrap()
            .offset,
        MAX_DATA_ADDR - 1
    );

    let many: Vec<DataFieldRecord> = (0..300u32)
        .map(|i| DataFieldRecord {
            first_slot: i,
            count: 1,
            offset: i * 8,
            stride: 0,
            ..base()
        })
        .collect();
    let data = raw(Some(&bytes(&many)), None, b"x");
    assert!(compare(&data));
    assert_eq!(
        wire_decode_data_layout_via_kel(&data)
            .unwrap()
            .unwrap()
            .fields
            .len(),
        300
    );
}
