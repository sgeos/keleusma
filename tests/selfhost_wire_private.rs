//! Private layout and initialization metadata must survive Keleusma encoding.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{
    ConstValue, DataLayout, DataSlot, Module, PrivateCompositeSlot, SlotVisibility, Value,
};
use keleusma::selfhost::wire_module_artifact_via_kel;

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

fn with_initializers(values: Vec<ConstValue>) -> Module {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks[0].constants = vec![ConstValue::Int(11), ConstValue::Int(22)];
    module.private_data_bytes = u32::try_from(values.len() * 32).unwrap();
    module.data_layout = Some(DataLayout {
        slots: vec![
            DataSlot {
                name: "d.value".into(),
                visibility: SlotVisibility::Private
            };
            values.len()
        ],
        shared_layout: vec![],
        private_composite_layout: vec![],
        private_init: values,
    });
    module
}

fn compare_layout(module: &Module) -> (u32, u32) {
    let bytes = wire_module_artifact_via_kel(module).unwrap();
    assert_eq!(bytes, module.to_bytes().unwrap());
    let at = u32::from_le_bytes(bytes[48..52].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(bytes[52..56].try_into().unwrap()) as usize;
    keleusma::wire_schema::DataLayoutTable::parse(&bytes[at..at + len])
        .unwrap()
        .unwrap()
        .private_init_range()
}

#[test]
fn elision_is_exactly_nonempty_int_zero_and_spans_batches() {
    assert_eq!(compare_layout(&with_initializers(vec![])), (2, 0));
    for count in [1, 512, 513, 2000] {
        assert_eq!(
            compare_layout(&with_initializers(vec![ConstValue::Int(0); count])),
            (u32::MAX, count as u32)
        );
        let mut values = vec![ConstValue::Int(0); count];
        values[count - 1] = ConstValue::Int(9);
        assert_eq!(
            compare_layout(&with_initializers(values)),
            (2, count as u32)
        );
    }
    for value in [
        ConstValue::Unit,
        ConstValue::None,
        ConstValue::Bool(false),
        ConstValue::Byte(0),
        ConstValue::Fixed(0),
        ConstValue::Int(i64::MIN),
    ] {
        assert_eq!(
            compare_layout(&with_initializers(vec![ConstValue::Int(0), value])),
            (2, 2)
        );
    }
    #[cfg(feature = "floats")]
    for value in [0.0, -0.0, f64::from_bits(0x7FF8_1234_5678_9ABC)] {
        assert_eq!(
            compare_layout(&with_initializers(vec![ConstValue::Float(value)])),
            (2, 1)
        );
    }
}

#[test]
fn initializer_ranges_follow_all_chunk_roots_and_handle_no_chunks() {
    let mut module = with_initializers(vec![ConstValue::Int(9)]);
    let mut second = module.chunks[0].clone();
    second.name = "second".into();
    second.constants = vec![ConstValue::Int(1); 3];
    module.chunks.push(second);
    assert_eq!(compare_layout(&module), (5, 1));
    module.chunks.clear();
    module.signatures.clear();
    module.entry_point = None;
    assert_eq!(compare_layout(&module), (0, 1));
    module.data_layout.as_mut().unwrap().private_init = vec![];
    assert_eq!(compare_layout(&module), (0, 0));
}

#[test]
fn private_composite_fields_preserve_their_full_widths() {
    let mut module = with_initializers(vec![ConstValue::Unit]);
    module
        .data_layout
        .as_mut()
        .unwrap()
        .private_composite_layout = vec![
        PrivateCompositeSlot { slot: 0, offset: 0 },
        PrivateCompositeSlot {
            slot: u16::MAX,
            offset: u32::MAX,
        },
    ];
    assert_eq!(compare_layout(&module), (2, 1));
}
