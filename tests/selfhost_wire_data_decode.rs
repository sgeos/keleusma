//! Data-layout codec checks are distinct from later verifier invariants.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{
    ConstValue, DataLayout, DataSlot, PrivateCompositeSlot, SharedSlotLayout, SlotVisibility,
};
use keleusma::selfhost::wire_decode_data_layout_via_kel;
use keleusma::wire_schema::{
    self as schema, DataInitRecord, DataSlotRecord, NameRef, PrivateCompositeRecord,
    SharedSlotRecord, kind,
};
use keleusma_wire::{WireBuilder, WireRecord};

fn bytes<T: WireRecord>(values: &[T]) -> Vec<u8> {
    let mut result = vec![0; values.len() * T::STRIDE];
    for (value, out) in values.iter().zip(result.chunks_exact_mut(T::STRIDE)) {
        value.write_record(out).unwrap();
    }
    result
}
fn raw(
    slots: Option<&[u8]>,
    shared: Option<&[u8]>,
    placements: Option<&[u8]>,
    init: Option<&[u8]>,
    text: &[u8],
) -> Vec<u8> {
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
    for (kind, data) in [
        (kind::DATA_SLOTS, slots),
        (kind::SHARED_LAYOUT, shared),
        (kind::PRIVATE_COMPOSITE, placements),
        (kind::DATA_INIT, init),
    ] {
        if let Some(data) = data {
            let r = b.region(kind, 0).unwrap();
            b.push(r, data);
        }
    }
    b.finish().unwrap()
}
fn equal(a: &DataLayout, b: &DataLayout) {
    assert_eq!(a.slots, b.slots);
    assert_eq!(a.shared_layout, b.shared_layout);
    assert_eq!(a.private_composite_layout, b.private_composite_layout);
    assert_eq!(
        schema::encode_constants(&a.private_init).unwrap(),
        schema::encode_constants(&b.private_init).unwrap()
    );
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

#[test]
fn named_runs_and_nondefault_initializer_forests_match_reference() {
    let layout = DataLayout {
        slots: (0..31)
            .map(|_| DataSlot {
                name: "λ".into(),
                visibility: SlotVisibility::Shared,
            })
            .collect(),
        shared_layout: (0..31)
            .map(|i| SharedSlotLayout {
                offset: i * 9,
                kind: 255,
                len: 7,
            })
            .collect(),
        private_composite_layout: vec![PrivateCompositeSlot {
            slot: 65535,
            offset: u32::MAX,
        }],
        private_init: vec![ConstValue::Tuple(vec![
            ConstValue::StaticStr("init".into()),
            ConstValue::Byte(7),
        ])],
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
fn absent_empty_and_elided_initializers_preserve_scope() {
    let zero = bytes(&[
        DataInitRecord {
            first: u32::MAX,
            count: 4,
        },
        DataInitRecord {
            first: 0,
            count: u32::MAX,
        },
    ]);
    let data = raw(None, Some(&[0; 8]), None, None, b"x");
    assert!(compare(&data));
    assert!(wire_decode_data_layout_via_kel(&data).unwrap().is_none());
    let data = raw(Some(&[]), Some(&[]), Some(&[]), Some(&zero), b"x");
    assert!(compare(&data));
    assert_eq!(
        wire_decode_data_layout_via_kel(&data)
            .unwrap()
            .unwrap()
            .private_init,
        vec![ConstValue::Int(0); 4]
    );
    for init in [None, Some(&[][..])] {
        assert!(!compare(&raw(Some(&[]), Some(&[]), Some(&[]), init, b"x")));
    }
    let stored = bytes(&[DataInitRecord { first: 0, count: 0 }]);
    assert!(!compare(&raw(
        Some(&[]),
        Some(&[]),
        Some(&[]),
        Some(&stored),
        b"x"
    )));
}

#[test]
fn run_validation_and_overflow_match_without_extra_kind_or_slot_rules() {
    let init = bytes(&[DataInitRecord {
        first: u32::MAX,
        count: 0,
    }]);
    let slot = DataSlotRecord {
        name: 0,
        visibility: 1,
        reserved: 255,
        run: 3,
    };
    let shared = SharedSlotRecord {
        offset: 10,
        kind: 255,
        reserved: 255,
        len: 65535,
        first_slot: u32::MAX,
        run: 3,
        stride: 2,
    };
    let placement = bytes(&[PrivateCompositeRecord {
        slot: 65535,
        reserved: 65535,
        offset: u32::MAX,
    }]);
    for (s, r, text, accepts) in [
        (slot, shared, &b"x"[..], true),
        (DataSlotRecord { run: 0, ..slot }, shared, &b"x"[..], false),
        (
            DataSlotRecord {
                visibility: 3,
                ..slot
            },
            shared,
            &b"x"[..],
            false,
        ),
        (DataSlotRecord { name: 1, ..slot }, shared, &b"x"[..], false),
        (
            slot,
            SharedSlotRecord { run: 0, ..shared },
            &b"x"[..],
            false,
        ),
        (
            slot,
            SharedSlotRecord {
                offset: u32::MAX,
                run: 2,
                stride: 1,
                ..shared
            },
            &b"x"[..],
            false,
        ),
        (
            slot,
            SharedSlotRecord {
                offset: u32::MAX,
                run: 1,
                stride: 65535,
                ..shared
            },
            &b"x"[..],
            true,
        ),
        (slot, shared, &b"\xff"[..], false),
    ] {
        let data = raw(
            Some(&bytes(&[s])),
            Some(&bytes(&[r])),
            Some(&placement),
            Some(&init),
            text,
        );
        assert_eq!(compare(&data), accepts);
    }
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
fn maximal_slot_runs_cross_batch_and_record_boundaries() {
    let slots = bytes(&[
        DataSlotRecord {
            name: 0,
            visibility: 1,
            reserved: 0,
            run: 65535,
        },
        DataSlotRecord {
            name: 0,
            visibility: 1,
            reserved: 0,
            run: 1,
        },
    ]);
    let shared = bytes(&[
        SharedSlotRecord {
            offset: 0,
            kind: 0,
            reserved: 0,
            len: 0,
            first_slot: 0,
            run: 65535,
            stride: 8,
        },
        SharedSlotRecord {
            offset: 65535 * 8,
            kind: 0,
            reserved: 0,
            len: 0,
            first_slot: 65535,
            run: 1,
            stride: 65535,
        },
    ]);
    let init = bytes(&[DataInitRecord {
        first: u32::MAX,
        count: 0,
    }]);
    let data = raw(Some(&slots), Some(&shared), Some(&[]), Some(&init), b"x");
    assert!(compare(&data));
    let layout = wire_decode_data_layout_via_kel(&data).unwrap().unwrap();
    assert_eq!(layout.slots.len(), 65536);
    assert_eq!(layout.shared_layout.len(), 65536);
    assert_eq!(layout.shared_layout[65535].offset, 65535 * 8);
}
