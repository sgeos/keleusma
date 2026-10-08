//! Templates preserve name-table order and empty field-run boundaries.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{ConstValue, Module, StructTemplate};
use keleusma::selfhost::{wire_auxiliary_from_module_via_kel, wire_module_artifact_via_kel};
use keleusma::wire_schema::{LayoutTable, ModuleTable};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}

fn template(name: &str, fields: &[&str]) -> StructTemplate {
    StructTemplate {
        type_name: name.into(),
        field_names: fields.iter().map(|s| (*s).into()).collect(),
    }
}

fn compare(module: &Module) -> Vec<u8> {
    let reference = module.to_bytes().unwrap();
    let at = u32::from_le_bytes(reference[48..52].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(reference[52..56].try_into().unwrap()) as usize;
    let bytes = wire_auxiliary_from_module_via_kel(module).unwrap();
    assert_eq!(bytes, reference[at..at + len]);
    assert_eq!(wire_module_artifact_via_kel(module).unwrap(), reference);
    bytes
}

#[test]
fn templates_preserve_fresh_fields_and_zero_length_runs_across_chunks() {
    let mut module = compile("fn main() -> Word { 7 }");
    let mut second = module.chunks[0].clone();
    second.name = "second".into();
    second.struct_templates = vec![template("T", &[])];
    module.chunks.push(second);
    module.native_names = vec!["T".into(), "f".into()];
    module.chunks[0].struct_templates = vec![
        template("T", &[]),
        template("T", &["f", "f"]),
        template("f", &[]),
        template("Z", &[]),
        template("T", &["T", "g"]),
    ];
    let bytes = compare(&module);
    let layout = LayoutTable::parse(&bytes).unwrap();
    assert_eq!(layout.template_count(), 6);
    for (i, (name, first, count)) in [
        (2, 4, 0),
        (2, 4, 2),
        (5, 6, 0),
        (6, 7, 0),
        (2, 7, 2),
        (7, 9, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let row = layout.template(i).unwrap();
        assert_eq!(
            (row.type_name, row.field_names_first, row.field_count),
            (name, first, count)
        );
    }
    let table = ModuleTable::parse(&bytes).unwrap();
    let first = table.chunk(0).unwrap();
    let second = table.chunk(1).unwrap();
    assert_eq!((first.templates_first, first.templates_count), (0, 5));
    assert_eq!((second.templates_first, second.templates_count), (5, 1));
}

#[test]
fn empty_and_unicode_template_names_remain_literal_bytes() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks[0].struct_templates = vec![template("", &["", "λ", "λ"]), template("λ", &[])];
    compare(&module);
}

#[test]
fn compiled_boxed_struct_reaches_the_template_encoder() {
    let module = compile(
        "use host::mk() -> [Word; 8192]\nstruct Big { field: [Word; 8192] }\nfn main() -> Word { let b = Big { field: host::mk() }; 0 }",
    );
    assert!(module.chunks.iter().any(|c| !c.struct_templates.is_empty()));
    compare(&module);
}

#[test]
fn template_event_capacity_includes_empty_run_checkpoints() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks[0].struct_templates = vec![StructTemplate {
        type_name: "T".into(),
        field_names: vec!["f".into(); 1021],
    }];
    compare(&module); // One chunk, one type, one checkpoint and 1021 fields.
    module.chunks[0].struct_templates[0]
        .field_names
        .push("f".into());
    assert!(wire_auxiliary_from_module_via_kel(&module).is_err());
    module.chunks[0].struct_templates = vec![template("T", &[]); 511];
    compare(&module);
    module.chunks[0].struct_templates.push(template("T", &[]));
    assert!(wire_auxiliary_from_module_via_kel(&module).is_err());
}

#[test]
fn scalar_transport_preserves_all_scalar_payloads() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks[0].constants = vec![
        ConstValue::Unit,
        ConstValue::None,
        ConstValue::Bool(false),
        ConstValue::Bool(true),
        ConstValue::Int(i64::MIN),
        ConstValue::Int(i64::MAX),
        ConstValue::Byte(0),
        ConstValue::Byte(255),
        ConstValue::Fixed(i64::MIN),
        ConstValue::Fixed(i64::MAX),
    ];
    #[cfg(feature = "floats")]
    module.chunks[0].constants.extend([
        ConstValue::Float(-0.0),
        ConstValue::Float(f64::INFINITY),
        ConstValue::Float(f64::from_bits(0x7FF8_1234_5678_9ABC)),
    ]);
    compare(&module);
}
