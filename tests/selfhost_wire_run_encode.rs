//! Data-field records, their names and their initialiser numbering are Keleusma decisions.
//!
//! This file held the run-grouping tests for the `DATA_SLOTS` and `SHARED_LAYOUT` run
//! records. Encoding F, which the operator chose on 2026-10-31, replaced both with one
//! `DATA_FIELDS` record per declared field, so no grouping remains to decide. What remains
//! is that the self-hosted encoder emits the same field records as the reference, with names
//! from its own interner, at every width and count the record carries.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{
    ConstValue, DataField, DataInit, DataLayout, MAX_DATA_ADDR, Module, SHARED_SLOT_COMPOSITE_FLAG,
};
use keleusma::selfhost::{compile_src, wire_auxiliary_from_module_via_kel};
use keleusma::wire_schema::{DataFieldRecord, kind};
use keleusma_wire::{WireRecord, WireView};

fn module(fields: Vec<DataField>) -> Module {
    let mut m = compile_src("fn main() -> Word { 7 }");
    m.data_layout = Some(DataLayout { fields });
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
/// `n` one-element shared `Int` fields with the given names, eight bytes apart.
fn named(names: &[String]) -> Vec<DataField> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| DataField::shared(n.clone(), i as u32, 1, 3, 0, (i * 8) as u32, 0))
        .collect()
}

#[test]
fn field_names_are_interned_by_the_stage_with_sharing() {
    // Equal names share one interned entry and distinct ones do not, whatever their
    // length, including names that differ only in a late byte.
    for mismatch in [0, 63, 64, 65, 127, 128, 129] {
        let a = "x".repeat(130);
        let mut b = a.clone().into_bytes();
        b[mismatch] = b'y';
        let b = String::from_utf8(b).unwrap();
        let names = vec![a.clone(), b.clone(), a.clone(), b.clone(), b];
        let bytes = compare(&module(named(&names)));
        let rows = records::<DataFieldRecord>(&bytes, kind::DATA_FIELDS);
        assert_eq!(rows.len(), 5, "one record per field, byte {mismatch}");
        assert_eq!(rows[0].name, rows[2].name, "equal names share an index");
        assert_ne!(rows[0].name, rows[1].name, "byte {mismatch}");
        assert_eq!(rows[1].name, rows[3].name);
    }
    // Empty names, and a long name of multi-byte characters.
    compare(&module(named(&vec![String::new(); 17])));
    compare(&module(named(&vec!["λ".repeat(5000); 3])));
}

#[test]
fn one_record_carries_any_element_count() {
    for count in [1u32, 2, 65534, 65535, 65536, 131071, MAX_DATA_ADDR - 1] {
        let stride = if count > 1 { 8 } else { 0 };
        let bytes = compare(&module(vec![DataField::shared(
            "shared.array",
            0,
            count,
            3,
            0,
            0,
            stride,
        )]));
        let rows = records::<DataFieldRecord>(&bytes, kind::DATA_FIELDS);
        assert_eq!(rows.len(), 1, "count {count}");
        assert_eq!((rows[0].count, rows[0].stride), (count, stride));
    }
}

#[test]
fn every_field_width_reaches_the_record() {
    let pooled = DataField {
        offset: Some(u32::MAX - 1),
        elem_len: u16::MAX,
        stride: u32::MAX,
        ..DataField::private(
            "p.wide",
            u32::MAX - 2,
            2,
            SHARED_SLOT_COMPOSITE_FLAG | 3,
            DataInit::Unit,
        )
    };
    let fields = vec![
        DataField::shared(
            "s.t",
            0,
            1,
            SHARED_SLOT_COMPOSITE_FLAG | 2,
            65535,
            u32::MAX - 2,
            0,
        ),
        DataField::private("p.z", 1, 7, 2, DataInit::Zero),
        DataField::private("p.c", 8, 1, 3, DataInit::Const(ConstValue::Int(-5))),
        pooled,
    ];
    let bytes = compare(&module(fields));
    let rows = records::<DataFieldRecord>(&bytes, kind::DATA_FIELDS);
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[3].first_slot, u32::MAX - 2);
    assert_eq!(rows[3].stride, u32::MAX);
    assert_eq!(rows[3].elem_len, u16::MAX);
}

#[test]
fn mixed_fields_match_reference_across_transport_batches() {
    let mut state = 7u32;
    let mut fields = Vec::new();
    let mut slot = 0u32;
    for i in 0..513u32 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let count = 1 + (state >> 20) % 5;
        let name = ["a", "b", "λ", ""][((state >> 8) & 3) as usize];
        let field = if i < 200 {
            DataField::shared(name, slot, count, (state & 3) as u8, 0, i * 64, 8)
        } else {
            let init = match state % 3 {
                0 => DataInit::Zero,
                1 => DataInit::Unit,
                _ => DataInit::Const(ConstValue::Int(i64::from(state))),
            };
            DataField::private(name, slot, count, 3, init)
        };
        slot += count;
        fields.push(field);
    }
    compare(&module(fields));
}

#[test]
fn encoded_layout_executes_on_both_sides_of_the_former_record_boundary() {
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
