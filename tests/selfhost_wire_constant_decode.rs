//! Owned constant reconstruction preserves reference values and ownership rules.
#![cfg(feature = "self-host")]

use keleusma::bytecode::ConstValue;
use keleusma::selfhost::wire_decode_constant_pools_via_kel;
use keleusma::wire_schema::{self as schema, ConstRecord, EnumAux, NameRef, StructAux, kind};
use keleusma_wire::WireBuilder;

fn compare(bytes: &[u8], ranges: &[(u32, u32)]) -> bool {
    let expected = schema::decode_constant_pools(bytes, ranges);
    let actual = wire_decode_constant_pools_via_kel(bytes, ranges);
    match (actual, expected) {
        (Ok(a), Ok(b)) => {
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(&b) {
                assert_eq!(a.len(), b.len());
                // Re-encoding compares floating-point payload bits, including NaNs.
                assert_eq!(
                    schema::encode_constants(a).unwrap(),
                    schema::encode_constants(b).unwrap()
                );
            }
            true
        }
        (Err(_), Err(_)) => false,
        (a, b) => panic!("constant decoder disagreement: {a:?} versus {b:?}"),
    }
}

fn raw(
    nodes: &[ConstRecord],
    names: &[NameRef],
    pool: &[u8],
    structs: &[StructAux],
    enums: &[EnumAux],
) -> Vec<u8> {
    let mut b = WireBuilder::new();
    let r = b.region(kind::CONSTS, 0).unwrap();
    for value in nodes {
        b.push_record(r, value);
    }
    let r = b.region(kind::NAMES, 0).unwrap();
    for value in names {
        b.push_record(r, value);
    }
    let r = b.region(kind::STRING_POOL, 0).unwrap();
    b.push(r, pool);
    let r = b.region(kind::STRUCT_AUX, 0).unwrap();
    for value in structs {
        b.push_record(r, value);
    }
    let r = b.region(kind::ENUM_AUX, 0).unwrap();
    for value in enums {
        b.push_record(r, value);
    }
    b.finish().unwrap()
}

fn node(tag: u16, first: u32, count: u32) -> ConstRecord {
    ConstRecord {
        tag,
        flags: 0,
        aux: 0,
        payload: u64::from(first) | (u64::from(count) << 32),
    }
}

#[test]
fn every_constant_form_and_nested_named_tree_matches_reference() {
    let mut roots = vec![
        ConstValue::Unit,
        ConstValue::None,
        ConstValue::Bool(true),
        ConstValue::Bool(false),
        ConstValue::Int(i64::MIN),
        ConstValue::Byte(255),
        ConstValue::Fixed(i64::MAX),
        ConstValue::StaticStr("λ\0\u{10FFFF}".into()),
        ConstValue::Tuple(vec![]),
        ConstValue::Array(vec![ConstValue::Int(9)]),
        ConstValue::Struct {
            type_name: "S".into(),
            fields: vec![
                (
                    "same".into(),
                    ConstValue::Tuple(vec![ConstValue::StaticStr("leaf".into())]),
                ),
                (
                    "same".into(),
                    ConstValue::Enum {
                        type_name: "E".into(),
                        variant: "V".into(),
                        discriminant: Some(i64::MIN),
                        fields: vec![ConstValue::Byte(7)],
                    },
                ),
            ],
        },
        ConstValue::Struct {
            type_name: "Empty".into(),
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
            fields: vec![],
        },
    ];
    #[cfg(feature = "floats")]
    roots.extend([
        ConstValue::Float(-0.0),
        ConstValue::Float(f64::from_bits(0xFFF8000000001234)),
    ]);
    // Keep mutation meaningful in the no-floats configuration too.
    roots.push(ConstValue::Int(17));
    let bytes = schema::encode_constants(&roots).unwrap();
    assert!(compare(&bytes, &[(0, roots.len() as u32)]));
}

#[test]
fn invalid_ranges_and_duplicate_ownership_are_refused() {
    let scalar = node(schema::tag::INT, 123, 456);
    for first in [0, 2, u32::MAX] {
        let bytes = raw(
            &[node(schema::tag::TUPLE, first, 1), scalar],
            &[],
            &[],
            &[],
            &[],
        );
        assert!(!compare(&bytes, &[(0, 1)]));
    }
    let bytes = raw(
        &[node(schema::tag::TUPLE, 1, 1), scalar],
        &[],
        &[],
        &[],
        &[],
    );
    assert!(compare(&bytes, &[(0, 1)]));
    assert!(!compare(&bytes, &[(1, 1)]));
    assert!(!compare(&bytes, &[(0, 1), (0, 1)]));
    let bytes = raw(
        &[
            node(schema::tag::TUPLE, 2, 1),
            node(schema::tag::ARRAY, 2, 1),
            scalar,
        ],
        &[],
        &[],
        &[],
        &[],
    );
    assert!(!compare(&bytes, &[(0, 2)]));
    let bytes = raw(&[scalar], &[], &[], &[], &[]);
    assert!(compare(&bytes, &[(0, 1)]));
    assert!(compare(&bytes, &[(0, 1)]));
    assert!(!compare(&bytes, &[(0, 1), (0, 1)]));
    assert!(!compare(&bytes, &[(u32::MAX, 0)]));
    assert!(compare(&bytes, &[(1, 0)]));
}

#[test]
fn empty_composites_unused_names_and_scalar_payloads_keep_reference_behavior() {
    let names = [
        NameRef {
            offset: 0,
            length: 1,
        },
        NameRef {
            offset: u32::MAX,
            length: 1,
        },
    ];
    let bytes = raw(
        &[node(schema::tag::STRUCT, 1, 0)],
        &names,
        b"S",
        &[StructAux {
            type_name: 0,
            field_names_first: u32::MAX,
        }],
        &[],
    );
    assert!(compare(&bytes, &[(0, 1)]));
    let bytes = raw(&[node(schema::tag::TUPLE, 0, 0)], &[], &[], &[], &[]);
    assert!(!compare(&bytes, &[(0, 1)]));
    let bytes = raw(&[node(schema::tag::TUPLE, 1, 0)], &[], &[], &[], &[]);
    assert!(compare(&bytes, &[(0, 1)]));
    for tag in [
        schema::tag::UNIT,
        schema::tag::NONE,
        schema::tag::BOOL,
        schema::tag::INT,
        schema::tag::BYTE,
        schema::tag::FIXED,
    ] {
        let bytes = raw(
            &[ConstRecord {
                tag,
                flags: u16::MAX,
                aux: u32::MAX,
                payload: u64::MAX,
            }],
            &[],
            &[],
            &[],
            &[],
        );
        assert!(compare(&bytes, &[(0, 1)]));
    }
    let bytes = raw(&[node(65535, 0, 0)], &[], &[], &[], &[]);
    assert!(!compare(&bytes, &[]));
    assert!(!compare(&bytes, &[(0, 0)]));
}

#[test]
fn float_tag_admission_matches_the_feature_configuration() {
    let bytes = raw(
        &[ConstRecord {
            tag: schema::tag::FLOAT,
            flags: 0,
            aux: 0,
            payload: 0x7FF8000000001234,
        }],
        &[],
        &[],
        &[],
        &[],
    );
    assert_eq!(compare(&bytes, &[(0, 1)]), cfg!(feature = "floats"));
    assert_eq!(compare(&bytes, &[]), cfg!(feature = "floats"));
}

#[test]
fn names_auxiliary_bounds_and_required_tables_are_checked() {
    let invalid_text = raw(
        &[ConstRecord {
            tag: schema::tag::STATIC_STR,
            flags: 0,
            aux: 0,
            payload: u64::MAX,
        }],
        &[NameRef {
            offset: 0,
            length: 2,
        }],
        b"\xc0\x80",
        &[],
        &[],
    );
    assert!(!compare(&invalid_text, &[(0, 1)]));
    let error = wire_decode_constant_pools_via_kel(&invalid_text, &[(0, 1)]).unwrap_err();
    assert!(
        matches!(error, keleusma::selfhost::SelfHostError::Unsupported { detail } if detail.contains("refused with -298"))
    );
    let names = [NameRef {
        offset: 0,
        length: 1,
    }];
    for (tag, structs, enums) in [
        (schema::tag::STRUCT, vec![], vec![]),
        (
            schema::tag::STRUCT,
            vec![StructAux {
                type_name: 1,
                field_names_first: 0,
            }],
            vec![],
        ),
        (schema::tag::ENUM, vec![], vec![]),
        (
            schema::tag::ENUM,
            vec![],
            vec![EnumAux {
                type_name: 0,
                variant: 1,
                discriminant: 0,
            }],
        ),
    ] {
        assert!(!compare(
            &raw(&[node(tag, 1, 0)], &names, b"S", &structs, &enums),
            &[(0, 1)]
        ));
    }
    let empty = raw(&[], &[], &[], &[], &[]);
    assert!(compare(&empty, &[]));
    assert!(compare(&empty, &[(0, 0)]));
    for missing in [
        kind::CONSTS,
        kind::NAMES,
        kind::STRING_POOL,
        kind::STRUCT_AUX,
        kind::ENUM_AUX,
    ] {
        let view = keleusma_wire::WireView::parse(&empty).unwrap();
        let mut builder = WireBuilder::new();
        for i in 0..view.region_count() {
            let r = view.region_at(i).unwrap();
            if r.kind != missing {
                builder.region(r.kind, r.flags).unwrap();
            }
        }
        assert!(!compare(&builder.finish().unwrap(), &[]));
    }
}

#[test]
fn large_pools_and_long_nested_names_do_not_inherit_encoder_caps() {
    let roots: Vec<_> = (0..5000).map(ConstValue::Int).collect();
    let bytes = schema::encode_constants(&roots).unwrap();
    let view = keleusma_wire::WireView::parse(&bytes).unwrap();
    assert!(
        view.find_region(kind::CONSTS)
            .unwrap()
            .byte_length()
            .unwrap()
            > 65536
    );
    assert!(compare(&bytes, &[(0, 2500), (2500, 2500)]));
    let text = "λ".repeat(40000);
    let roots = vec![ConstValue::Tuple(vec![ConstValue::StaticStr(text)])];
    let bytes = schema::encode_constants(&roots).unwrap();
    assert!(bytes.len() > 65536);
    assert!(compare(&bytes, &[(0, 1)]));
}

#[test]
fn raw_deep_trees_and_malformed_required_record_lengths_match_reference() {
    let mut nodes: Vec<_> = (0..128)
        .map(|i| node(schema::tag::TUPLE, i + 1, 1))
        .collect();
    nodes.push(ConstRecord {
        tag: schema::tag::INT,
        flags: 0,
        aux: 0,
        payload: 7,
    });
    let bytes = raw(&nodes, &[], &[], &[], &[]);
    assert!(compare(&bytes, &[(0, 1)]));
    for bad_kind in [kind::CONSTS, kind::ENUM_AUX] {
        let view = keleusma_wire::WireView::parse(&bytes).unwrap();
        let mut builder = WireBuilder::new();
        for i in 0..view.region_count() {
            let r = view.region_at(i).unwrap();
            let id = builder.region(r.kind, r.flags).unwrap();
            builder.push(id, view.region_bytes(&r).unwrap());
            if r.kind == bad_kind {
                builder.push(id, &[0; 8]);
            }
        }
        assert!(!compare(&builder.finish().unwrap(), &[(0, 1)]));
    }
}
#[test]
fn decoded_constants_and_separate_initializer_pools_execute() {
    use keleusma::bytecode::{Op, Value};
    use keleusma::selfhost::{
        WireSchemaRecord, wire_auxiliary_directory_via_kel, wire_schema_records_via_kel,
    };
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for (source, expected, composite) in [
        (
            "const data d { values: [Word; 3] = [4, 5, 6] } fn main(i: Word) -> Word { d.values[i] }",
            5,
            true,
        ),
        (
            "const data d { values: [(Word, Word); 2] = [(1, 2), (3, 4)] } fn main(i: Word) -> Word { let p = d.values[i]; p.0 + p.1 }",
            7,
            true,
        ),
        (
            "private data d { value: Word = 7 } fn main() -> Word { d.value = d.value + 1; d.value }",
            8,
            false,
        ),
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let mut module = keleusma::compiler::compile(&ast).unwrap();
        if composite {
            assert!(module.chunks.iter().any(|c| c.ops.iter().any(|op| matches!(op,
                Op::Const(index) if matches!(c.constants.get(usize::from(*index)), Some(ConstValue::Array(_) | ConstValue::Tuple(_)))))));
        }
        let artifact = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
        let raw = keleusma::wire_format::parse_wire_sections(&artifact)
            .unwrap()
            .aux_body;
        let directory = wire_auxiliary_directory_via_kel(raw).unwrap();
        let chunks = directory
            .regions
            .iter()
            .find(|r| r.kind == kind::CHUNKS)
            .unwrap();
        let records = wire_schema_records_via_kel(chunks.kind, chunks.bytes).unwrap();
        let ranges: Vec<_> = records
            .into_iter()
            .map(|r| match r {
                WireSchemaRecord::ChunkRecord(r) => (r.consts_first, r.consts_count),
                _ => panic!("unexpected chunk record"),
            })
            .collect();
        for chunk in &mut module.chunks {
            chunk.constants.clear();
        }
        let pools = wire_decode_constant_pools_via_kel(raw, &ranges).unwrap();
        assert_eq!(pools.len(), module.chunks.len());
        for (chunk, pool) in module.chunks.iter_mut().zip(pools) {
            chunk.constants = pool;
        }
        if !composite {
            let region = directory
                .regions
                .iter()
                .find(|r| r.kind == kind::DATA_INIT)
                .unwrap();
            let records = wire_schema_records_via_kel(region.kind, region.bytes).unwrap();
            let WireSchemaRecord::DataInitRecord(init) = &records[0] else {
                panic!("initializer record")
            };
            assert_ne!(
                init.first,
                u32::MAX,
                "control must reach a stored initializer"
            );
            module.data_layout.as_mut().unwrap().private_init.clear();
            module.data_layout.as_mut().unwrap().private_init =
                wire_decode_constant_pools_via_kel(raw, &[(init.first, init.count)])
                    .unwrap()
                    .pop()
                    .unwrap();
        }
        let need = required_persistent_capacity_for(&module);
        let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(module, &arena).unwrap();
        let args = if composite {
            vec![Value::Int(1)]
        } else {
            vec![]
        };
        assert!(
            matches!(vm.call(&args).unwrap(), VmState::Finished(Value::Int(n)) if n == expected)
        );
    }
}
