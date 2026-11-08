//! Private layout and initialization metadata must survive Keleusma encoding.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{
    ConstValue, DataField, DataInit, DataLayout, MAX_DATA_ADDR, Module, SHARED_SLOT_COMPOSITE_FLAG,
    Value,
};
use keleusma::selfhost::wire_module_artifact_via_kel;
use keleusma::wire_schema::{ABSENT, init_form};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}

fn compare_and_run(source: &str, expected: i64) {
    let module = compile(source);
    let bytes = wire_module_artifact_via_kel(&module).unwrap();
    assert_eq!(bytes, module.to_bytes().unwrap());
    let decoded = Module::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.to_bytes().unwrap(), bytes);
    let need = keleusma::vm::required_persistent_capacity_for(&decoded);
    let mut arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let mut vm = keleusma::vm::Vm::new(decoded, &arena).unwrap();
    assert!(
        matches!(vm.call(&[]).unwrap(), keleusma::vm::VmState::Finished(Value::Int(n)) if n == expected)
    );
}

#[test]
fn explicit_private_scalar_initializers_round_trip_and_execute() {
    compare_and_run(
        "private data d { total: Word = 42 } fn main() -> Word { let n = d.total; d.total = 0; n }",
        42,
    );
}

#[test]
fn private_composite_placement_round_trips_and_executes() {
    compare_and_run(
        "struct Point { x: Word, y: Word } private data d { p: Point } fn main() -> Word { d.p = Point { x: 3, y: 4 }; d.p.x + d.p.y }",
        7,
    );
}

/// A module whose data layout is one private `Int` field per initialiser, each of `count`
/// elements, every initialiser in the canonical form the compiler would choose.
fn with_initializers(values: Vec<ConstValue>, count: u32) -> Module {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks[0].constants = vec![ConstValue::Int(11), ConstValue::Int(22)];
    module.private_data_bytes = u32::try_from(values.len() * 32).unwrap() * count;
    let fields = values
        .into_iter()
        .enumerate()
        .map(|(i, v)| {
            DataField::private(
                format!("d.f{i}"),
                i as u32 * count,
                count,
                INT_KIND,
                DataInit::canonical(INT_KIND, v),
            )
        })
        .collect();
    module.data_layout = Some(DataLayout { fields });
    module
}

/// The `(init, init_const)` of every field record in the self-hosted artifact, after checking
/// the artifact byte for byte against the reference.
fn compare_layout(module: &Module) -> Vec<(u8, u32)> {
    let bytes = wire_module_artifact_via_kel(module).unwrap();
    assert_eq!(bytes, module.to_bytes().unwrap());
    let at = u32::from_le_bytes(bytes[48..52].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(bytes[52..56].try_into().unwrap()) as usize;
    let t = keleusma::wire_schema::DataLayoutTable::parse(&bytes[at..at + len])
        .unwrap()
        .unwrap();
    (0..t.field_count())
        .map(|i| {
            let r = t.field(i).unwrap();
            (r.init, r.init_const)
        })
        .collect()
}

const INT_KIND: u8 = 3;
const ZERO: (u8, u32) = (init_form::ZERO, ABSENT);

/// The zero form is chosen by the kind, never by the element count, and the explicit
/// constants are numbered after the two chunk roots in field order, across the stage's
/// sixteen-field transport batches.
#[test]
fn the_zero_form_is_per_field_and_numbering_spans_batches() {
    assert_eq!(compare_layout(&with_initializers(vec![], 1)), vec![]);
    for fields in [1usize, 16, 17, 40] {
        for count in [1u32, 513] {
            assert_eq!(
                compare_layout(&with_initializers(vec![ConstValue::Int(0); fields], count)),
                vec![ZERO; fields],
                "{fields} zero fields of {count} elements"
            );
            let mut values = vec![ConstValue::Int(0); fields];
            values[fields - 1] = ConstValue::Int(9);
            let mut want = vec![ZERO; fields];
            want[fields - 1] = (init_form::CONST, 2);
            assert_eq!(
                compare_layout(&with_initializers(values, count)),
                want,
                "{fields} fields of {count} elements, the last explicit"
            );
        }
    }
    // Every explicit value is a constant, numbered in field order.
    let explicit = vec![
        ConstValue::Int(i64::MIN),
        ConstValue::Int(1),
        ConstValue::Int(0),
        ConstValue::Int(-1),
    ];
    assert_eq!(
        compare_layout(&with_initializers(explicit, 3)),
        vec![
            (init_form::CONST, 2),
            (init_form::CONST, 3),
            ZERO,
            (init_form::CONST, 4)
        ]
    );
}

#[test]
fn initializer_constants_follow_all_chunk_roots_and_handle_no_chunks() {
    let mut module = with_initializers(vec![ConstValue::Int(9)], 1);
    let mut second = module.chunks[0].clone();
    second.name = "second".into();
    second.constants = vec![ConstValue::Int(1); 3];
    module.chunks.push(second);
    assert_eq!(compare_layout(&module), vec![(init_form::CONST, 5)]);
    module.chunks.clear();
    module.signatures.clear();
    module.entry_point = None;
    assert_eq!(compare_layout(&module), vec![(init_form::CONST, 0)]);
    module.data_layout.as_mut().unwrap().fields[0].init = DataInit::Unit;
    assert_eq!(compare_layout(&module), vec![(init_form::UNIT, ABSENT)]);
}

/// A field's widest values reach the artifact unchanged: a first slot near the top of the
/// twenty-four-bit slot space, a pool offset near `u32::MAX` and the largest body length.
#[test]
fn private_composite_fields_preserve_their_full_widths() {
    let mut module = with_initializers(vec![ConstValue::Int(0)], MAX_DATA_ADDR - 2);
    module.data_layout.as_mut().unwrap().fields.push(DataField {
        offset: Some(u32::MAX - u32::from(u16::MAX)),
        elem_len: u16::MAX,
        ..DataField::private(
            "d.p",
            MAX_DATA_ADDR - 2,
            2,
            SHARED_SLOT_COMPOSITE_FLAG | 3,
            DataInit::Unit,
        )
    });
    module.data_layout.as_mut().unwrap().fields[1].stride = 0;
    module.data_layout.as_mut().unwrap().fields[1].count = 1;
    assert_eq!(
        compare_layout(&module),
        vec![ZERO, (init_form::UNIT, ABSENT)]
    );
}
