//! Constant forests must retain breadth-first indexing and name order.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{
    ConstValue, DataLayout, DataSlot, Module, SlotVisibility, StructTemplate,
};
use keleusma::selfhost::wire_module_artifact_via_kel;

fn module() -> Module {
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 7 }").unwrap())
            .unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}

fn compare(module: &Module) -> Vec<u8> {
    let bytes = wire_module_artifact_via_kel(module).unwrap();
    assert_eq!(bytes, module.to_bytes().unwrap());
    let decoded = Module::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.to_bytes().unwrap(), bytes);
    bytes
}

#[test]
fn nested_constant_names_follow_breadth_first_order() {
    let mut module = module();
    module.chunks[0].constants = vec![
        ConstValue::Tuple(vec![
            ConstValue::Tuple(vec![
                ConstValue::StaticStr("deep".into()),
                ConstValue::Int(1),
            ]),
            ConstValue::StaticStr("shallow".into()),
        ]),
        ConstValue::StaticStr("root".into()),
    ];
    compare(&module);
}

#[test]
fn struct_and_enum_side_tables_preserve_names_ranges_and_discriminants() {
    let mut module = module();
    module.native_names = vec!["T".into(), "field".into(), "E".into()];
    module.chunks[0].constants = vec![
        ConstValue::Struct {
            type_name: "T".into(),
            fields: vec![
                ("field".into(), ConstValue::Int(8)),
                ("field".into(), ConstValue::StaticStr("nested".into())),
            ],
        },
        ConstValue::Struct {
            type_name: "field".into(),
            fields: vec![],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "V".into(),
            discriminant: None,
            fields: vec![],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "V".into(),
            discriminant: Some(0),
            fields: vec![ConstValue::Int(9)],
        },
        ConstValue::Enum {
            type_name: "E".into(),
            variant: "V".into(),
            discriminant: Some(i64::MIN),
            fields: vec![ConstValue::Tuple(vec![])],
        },
    ];
    module.chunks[0].struct_templates = vec![StructTemplate {
        type_name: "field".into(),
        field_names: vec!["nested".into(), "field".into()],
    }];
    compare(&module);
}

#[test]
fn initializer_roots_share_the_same_forest_with_chunk_roots() {
    let mut module = module();
    module.chunks[0].constants = vec![ConstValue::Tuple(vec![ConstValue::StaticStr(
        "child".into(),
    )])];
    module.data_layout = Some(DataLayout {
        slots: vec![DataSlot {
            name: "d.value".into(),
            visibility: SlotVisibility::Private,
        }],
        shared_layout: vec![],
        private_composite_layout: vec![],
        private_init: vec![ConstValue::Array(vec![
            ConstValue::Int(4),
            ConstValue::StaticStr("initializer".into()),
        ])],
    });
    module.private_data_bytes = 32;
    let bytes = compare(&module);
    let at = u32::from_le_bytes(bytes[48..52].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(bytes[52..56].try_into().unwrap()) as usize;
    let layout = keleusma::wire_schema::DataLayoutTable::parse(&bytes[at..at + len])
        .unwrap()
        .unwrap();
    assert_eq!(layout.private_init_range(), (1, 1));
    module.chunks.clear();
    module.signatures.clear();
    module.entry_point = None;
    compare(&module);
}

#[test]
fn forests_cross_former_node_and_name_caps() {
    let mut module = module();
    for count in [170, 171, 1024, 1365, 1366, 5000] {
        module.chunks[0].constants = vec![ConstValue::Array(vec![ConstValue::Int(7); count - 1])];
        compare(&module);
    }
    module.chunks[0].constants = vec![ConstValue::Array(vec![ConstValue::Int(7); 1365])];
    compare(&module);
    module.chunks[0].constants = vec![ConstValue::StaticStr("x".repeat(257))];
    compare(&module);
}

#[test]
fn compiled_constant_forests_execute_after_decoding() {
    for (source, expected) in [
        (
            "const data d { values: [Word; 3] = [4, 5, 6] } fn main() -> Word { d.values[1] }",
            5,
        ),
        (
            "const data d { value: ((Word, Word), Word) = ((1, 2), 3) } fn main() -> Word { (d.value.0).0 + d.value.1 }",
            4,
        ),
        (
            "fn main() -> Word { if \"tree\" == \"tree\" { 7 } else { 0 } }",
            7,
        ),
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let module = keleusma::compiler::compile(&ast).unwrap();
        let bytes = compare(&module);
        let decoded = Module::from_bytes(&bytes).unwrap();
        let need = keleusma::vm::required_persistent_capacity_for(&decoded);
        let mut arena = keleusma::Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = keleusma::vm::Vm::new(decoded, &arena).unwrap();
        assert!(
            matches!(vm.call(&[]).unwrap(), keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(n)) if n == expected)
        );
    }
}
