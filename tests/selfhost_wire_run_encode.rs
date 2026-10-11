//! Logical-slot grouping and split records are Keleusma decisions.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{DataLayout, DataSlot, Module, SharedSlotLayout, SlotVisibility};
use keleusma::selfhost::{compile_src, wire_auxiliary_from_module_via_kel};
use keleusma::wire_schema::{DataSlotRecord, SharedSlotRecord, kind};
use keleusma_wire::{WireRecord, WireView};

fn module(slots: Vec<DataSlot>, shared_layout: Vec<SharedSlotLayout>) -> Module {
    let mut m = compile_src("fn main() -> Word { 7 }");
    m.data_layout = Some(DataLayout {
        slots,
        shared_layout,
        private_composite_layout: vec![],
        private_init: vec![],
    });
    m
}
fn compare(m: &Module) -> Vec<u8> {
    let reference = m.to_bytes().unwrap();
    let aux = keleusma::wire_format::parse_wire_sections(&reference)
        .unwrap()
        .aux_body;
    let actual = wire_auxiliary_from_module_via_kel(m).unwrap();
    assert_eq!(actual, aux);
    actual
}
fn records<T: WireRecord>(bytes: &[u8], kind: u16) -> Vec<T> {
    let view = WireView::parse(bytes).unwrap();
    let r = view.find_region(kind).unwrap();
    let table = view.typed_records::<T>(&r).unwrap();
    (0..table.len())
        .map(|i| table.get_as::<T>(i).unwrap())
        .collect()
}
fn slot(name: &str, visibility: SlotVisibility) -> DataSlot {
    DataSlot {
        name: name.into(),
        visibility,
    }
}
fn shared(offset: u32, kind: u8, len: u16) -> SharedSlotLayout {
    SharedSlotLayout { offset, kind, len }
}

#[test]
fn data_names_compare_across_windows_and_stop_at_the_first_difference() {
    use SlotVisibility::{Private, Shared};
    for boundary in [0, 1, 7, 8, 9, 15, 16, 17] {
        let mut slots = vec![slot("", Shared); boundary];
        slots.push(slot("", Private));
        slots.extend(vec![slot("", Shared); 9]);
        let bytes = compare(&module(slots, vec![]));
        let rows = records::<DataSlotRecord>(&bytes, kind::DATA_SLOTS);
        assert_eq!(rows.len(), if boundary == 0 { 2 } else { 3 });
    }
    for mismatch in [0, 63, 64, 65, 127, 128, 129] {
        let a = "x".repeat(130);
        let mut b = a.clone().into_bytes();
        b[mismatch] = b'y';
        let b = String::from_utf8(b).unwrap();
        let mut slots = vec![slot(&a, Shared); 9];
        slots.extend([
            slot(&b, Shared),
            slot(&a, Shared),
            slot(&b, Shared),
            slot(&b, Shared),
            slot(&b, Private),
        ]);
        let bytes = compare(&module(slots, vec![]));
        let rows = records::<DataSlotRecord>(&bytes, kind::DATA_SLOTS);
        assert_eq!(
            rows.iter().map(|r| r.run).collect::<Vec<_>>(),
            [9, 1, 1, 2, 1],
            "byte {mismatch}"
        );
    }
    let name = "λ".repeat(5000);
    compare(&module(vec![slot(&name, Shared); 17], vec![]));
}

#[test]
fn shared_runs_preserve_greedy_stride_kind_and_length_boundaries() {
    let cases = vec![
        vec![],
        vec![shared(77, 255, 65535)],
        vec![
            shared(0, 3, 0),
            shared(8, 3, 0),
            shared(16, 3, 0),
            shared(32, 3, 0),
            shared(48, 3, 0),
        ],
        vec![shared(9, 3, 0), shared(9, 3, 0), shared(9, 3, 0)],
        vec![
            shared(0, 1, 2),
            shared(65535, 1, 2),
            shared(131070, 1, 2),
            shared(196606, 1, 2),
        ],
        vec![shared(0, 1, 2), shared(65536, 1, 2), shared(65537, 1, 2)],
        vec![
            shared(0, 1, 2),
            shared(1, 2, 2),
            shared(2, 2, 3),
            shared(3, 2, 3),
        ],
        vec![shared(u32::MAX, 7, 9), shared(0, 7, 9), shared(1, 7, 9)],
    ];
    for layout in cases {
        compare(&module(vec![], layout));
    }
    let bytes = compare(&module(
        vec![],
        vec![
            shared(0, 3, 0),
            shared(8, 3, 0),
            shared(16, 3, 0),
            shared(32, 3, 0),
            shared(48, 3, 0),
        ],
    ));
    let rows = records::<SharedSlotRecord>(&bytes, kind::SHARED_LAYOUT);
    assert_eq!(
        rows.iter()
            .map(|r| (r.first_slot, r.run, r.stride))
            .collect::<Vec<_>>(),
        [(0, 3, 8), (3, 2, 16)]
    );
}

#[test]
fn u16_boundaries_preserve_split_offsets_and_tail_stride() {
    for count in [65534, 65535, 65536, 131071] {
        let slots = vec![slot("shared.array", SlotVisibility::Shared); count];
        let shared_layout = (0..count).map(|i| shared((i * 8) as u32, 3, 0)).collect();
        let bytes = compare(&module(slots, shared_layout));
        let names = records::<DataSlotRecord>(&bytes, kind::DATA_SLOTS);
        let rows = records::<SharedSlotRecord>(&bytes, kind::SHARED_LAYOUT);
        assert_eq!(names.len(), count.div_ceil(65535));
        assert_eq!(rows.len(), names.len());
        for (i, (name, row)) in names.iter().zip(&rows).enumerate() {
            let first = i * 65535;
            let take = (count - first).min(65535);
            assert_eq!(name.run as usize, take);
            assert_eq!(
                (row.offset, row.first_slot, row.run, row.stride),
                ((first * 8) as u32, first as u32, take as u16, 8)
            );
        }
    }
}

#[test]
fn mixed_metadata_batches_match_reference_without_assuming_sorted_offsets() {
    let mut state = 7u32;
    let mut slots = Vec::new();
    let mut layout = Vec::new();
    for i in 0..513 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        slots.push(slot(
            ["a", "b", "λ", ""][((state >> 8) & 3) as usize],
            if state & 1 == 0 {
                SlotVisibility::Shared
            } else {
                SlotVisibility::Private
            },
        ));
        layout.push(shared(
            if i % 7 < 4 { i * 8 } else { state },
            (state & 3) as u8,
            ((state >> 3) & 7) as u16,
        ));
    }
    compare(&module(slots, layout));
}

#[test]
fn encoded_split_layout_executes_on_both_sides_of_the_record_boundary() {
    use keleusma::bytecode::Value;
    use keleusma::selfhost::{
        self_hosted_compile, wire_decode_module_via_kel, wire_module_artifact_via_kel,
    };
    use keleusma::target::Target;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
    let source =
        "shared data io { values: [Byte; 65536] } fn main(i: Word) -> Byte { io.values[i] }";
    let original = self_hosted_compile(source, &Target::host()).unwrap();
    let bytes = wire_module_artifact_via_kel(&original).unwrap();
    assert_eq!(bytes, original.to_bytes().unwrap());
    drop(original);
    let module = wire_decode_module_via_kel(&bytes).unwrap();
    let arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    for (slot, value) in [(65534, 17u8), (65535, 23u8)] {
        vm.set_shared(&mut shared, slot, Value::Byte(value))
            .unwrap();
        assert!(
            matches!(vm.call_with_shared(&mut shared,&[Value::Int(slot as i64)]).unwrap(),VmState::Finished(Value::Byte(n)) if n==value)
        );
    }
}
