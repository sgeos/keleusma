//! Complete serialized artifacts from the production compiler, across the stage inventory.
//! This checks scaffold fields as well as chunk bodies. The all-stage comparison uses
//! Keleusma serialization for pipeline output and the Rust encoder for its reference.
#![cfg(all(
    feature = "self-host",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

const STAGES: &[(&str, &str)] = &[
    ("lexer", include_str!("../src/selfhost/kel/lexer.kel")),
    ("parse", include_str!("../src/selfhost/kel/parse.kel")),
    (
        "reconstruct",
        include_str!("../src/selfhost/kel/reconstruct.kel"),
    ),
    ("codegen", include_str!("../src/selfhost/kel/codegen.kel")),
    ("analyze", include_str!("../src/selfhost/kel/analyze.kel")),
    (
        "verify_structural",
        include_str!("../src/selfhost/kel/verify_structural.kel"),
    ),
    (
        "verify_yield",
        include_str!("../src/selfhost/kel/verify_yield.kel"),
    ),
    (
        "verify_depth",
        include_str!("../src/selfhost/kel/verify_depth.kel"),
    ),
    (
        "verify_typed",
        include_str!("../src/selfhost/kel/verify_typed.kel"),
    ),
    (
        "verify_datalayout",
        include_str!("../src/selfhost/kel/verify_datalayout.kel"),
    ),
    (
        "verify_types",
        include_str!("../src/selfhost/kel/verify_types.kel"),
    ),
    ("wire", include_str!("../src/selfhost/kel/wire.kel")),
    (
        "wire_debug",
        include_str!("../src/selfhost/kel/wire_debug.kel"),
    ),
    (
        "wire_schema",
        include_str!("../src/selfhost/kel/wire_schema.kel"),
    ),
];

#[test]
fn complete_artifact_corpus_covers_every_stage_source() {
    let actual: std::collections::BTreeSet<_> = std::fs::read_dir("src/selfhost/kel")
        .expect("stage directory")
        .map(|entry| entry.expect("stage entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "kel"))
        .map(|path| {
            path.file_stem()
                .expect("stage name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let listed: std::collections::BTreeSet<_> =
        STAGES.iter().map(|(name, _)| (*name).to_owned()).collect();
    assert!(!actual.is_empty());
    assert_eq!(listed, actual);
}

/// Loop scopes and enum scrutinee admission must preserve artifacts and execution.
#[test]
fn loop_scope_and_match_admission_preserve_artifacts() {
    use keleusma::Arena;
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for source in [
        "enum E { A, B } fn main() -> Word { match E::A() { E::A => 7, E::B => 2 } }",
        "enum E { A(Word), B } fn main() -> Word { match E::A(7) { E::A(n) => n, E::B => 2 } }",
        "enum E { A, B } fn make() -> E { E::A() } fn main() -> Word { match make() { E::A => 7, E::B => 2 } }",
        "enum E { A(Word), B } fn make() -> E { E::A(7) } fn choose(e: E) -> Word { match e { E::A(n) => n, E::B => 2 } } fn main() -> Word { choose(make()) }",
        "fn main() -> Word { let i = 7; for i in 0..2 limit 2 {} i }",
        "fn main() -> Word { let i = 7; for i in 0..2 {} i }",
        "fn main() -> Word { let x = 7; for i in 0..2 limit 2 { let x = i; } x }",
        "fn main() -> Word { let x = 7; for i in 0..2 { let x = i; } x }",
        "fn main() -> Word { let x = 7; for i in 0..0 limit 2 { let x = i; } x }",
        "fn main() -> Word { let x = 7; for x in 0..2 limit 2 { for x in 0..2 limit 2 {} let y = x; } x }",
        "struct P { x: Word } fn main() -> Word { let p = P { x: 7 }; for i in 0..2 limit 2 { let p = 1; } p.x }",
        "private data d { n: Word } fn main() -> Word { let i = 2; for i in 0..i limit 2 { d.n = d.n + 1; } d.n + 5 }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let reference = keleusma::compiler::compile(&ast).expect("valid reference control");
        let actual = keleusma::selfhost::self_host_compile_scratch(source);
        for (pipeline, expected) in actual.chunks.iter().zip(&reference.chunks) {
            assert_eq!(pipeline.ops, expected.ops, "{source}");
        }
        if actual.to_bytes().unwrap() != reference.to_bytes().unwrap() {
            eprintln!("pipeline {actual:#?}\nreference {reference:#?}");
        }
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
        for module in [reference, actual] {
            let persistent = required_persistent_capacity_for(&module);
            let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
            arena.resize_persistent(persistent).unwrap();
            let entry = module.chunks.iter().position(|c| c.name == "main").unwrap();
            let mut vm = Vm::new(module, &arena).expect("verified loop control");
            assert!(
                matches!(
                    vm.call_function(entry, &[]).unwrap(),
                    VmState::Finished(Value::Int(7))
                ),
                "{source}"
            );
        }
    }
}

fn reference_record_bytes(record: &keleusma::selfhost::WireSchemaRecord) -> Vec<u8> {
    use keleusma::selfhost::WireSchemaRecord;
    fn encode<T: keleusma_wire::WireRecord>(value: &T) -> Vec<u8> {
        let mut out = vec![0; T::STRIDE];
        value.write_record(&mut out).unwrap();
        out
    }
    match record {
        WireSchemaRecord::NameRef(value) => encode(value),
        WireSchemaRecord::ConstRecord(value) => encode(value),
        WireSchemaRecord::StructAux(value) => encode(value),
        WireSchemaRecord::EnumAux(value) => encode(value),
        WireSchemaRecord::ShapeRecord(value) => encode(value),
        WireSchemaRecord::SignatureRecord(value) => encode(value),
        WireSchemaRecord::StructTemplateRecord(value) => encode(value),
        WireSchemaRecord::EnumVariantRecord(value) => encode(value),
        WireSchemaRecord::EnumLayoutRecord(value) => encode(value),
        WireSchemaRecord::DataSlotRecord(value) => encode(value),
        WireSchemaRecord::SharedSlotRecord(value) => encode(value),
        WireSchemaRecord::PrivateCompositeRecord(value) => encode(value),
        WireSchemaRecord::DataInitRecord(value) => encode(value),
        WireSchemaRecord::ChunkRecord(value) => encode(value),
        WireSchemaRecord::NativeRecord(value) => encode(value),
        WireSchemaRecord::HeaderRecord(value) => encode(value),
        WireSchemaRecord::NativeReturnRecord(value) => encode(value),
    }
}

#[test]
fn production_compiler_matches_every_complete_stage_artifact() {
    let mut differences = Vec::new();
    for &(name, source) in STAGES {
        let tokens = keleusma::lexer::tokenize(source).expect("reference lex");
        let ast = keleusma::parser::parse(&tokens).expect("reference parse");
        let reference = keleusma::compiler::compile(&ast).expect("reference compile");
        let actual = keleusma::selfhost::self_host_compile_scratch(source);
        let expected_bytes = reference.to_bytes().expect("reference artifact");
        let actual_bytes = keleusma::selfhost::wire_module_artifact_via_kel(&actual)
            .unwrap_or_else(|e| panic!("{name}: pipeline wire emission failed: {e:?}"));
        let raw_auxiliary = keleusma::wire_format::parse_wire_sections(&actual_bytes)
            .unwrap()
            .aux_body;
        let directory = keleusma::selfhost::wire_auxiliary_directory_via_kel(raw_auxiliary)
            .unwrap_or_else(|e| panic!("{name}: auxiliary directory failed: {e:?}"));
        let expected_directory = keleusma_wire::WireView::parse(raw_auxiliary).unwrap();
        assert!(!directory.regions.is_empty(), "{name}: empty directory");
        assert_eq!(
            directory.regions.len(),
            usize::from(expected_directory.region_count())
        );
        assert_eq!(directory.needs_scrub, expected_directory.needs_scrub());
        let mut constant_ranges = Vec::new();
        for (i, region) in directory.regions.iter().enumerate() {
            let expected = expected_directory.region_at(i as u16).unwrap();
            assert_eq!(
                (region.kind, region.flags, region.covers),
                (expected.kind, expected.flags, expected.covers),
                "{name}"
            );
            assert_eq!(
                region.byte_offset,
                expected.byte_offset().unwrap(),
                "{name}"
            );
            let expected_bytes = expected_directory.region_bytes(&expected).unwrap();
            assert_eq!(region.bytes, expected_bytes, "{name}");
            assert_eq!(
                region.bytes.as_ptr(),
                expected_bytes.as_ptr(),
                "{name}: payload copied"
            );
            use keleusma::wire_schema::kind;
            if !matches!(
                region.kind,
                kind::STRING_POOL | kind::PARAM_TYPES | kind::DEBUG_POOL
            ) {
                let records =
                    keleusma::selfhost::wire_schema_records_via_kel(region.kind, region.bytes)
                        .unwrap_or_else(|e| {
                            panic!("{name}: record kind {} failed: {e:?}", region.kind)
                        });
                assert_eq!(
                    records.is_empty(),
                    region.bytes.is_empty(),
                    "{name}: kind {}",
                    region.kind
                );
                let mut reconstructed = Vec::new();
                for record in &records {
                    assert_eq!(record.kind(), region.kind);
                    if let keleusma::selfhost::WireSchemaRecord::ChunkRecord(chunk) = record {
                        constant_ranges.push((chunk.consts_first, chunk.consts_count));
                    }
                    reconstructed.extend_from_slice(&reference_record_bytes(record));
                }
                assert_eq!(
                    reconstructed, region.bytes,
                    "{name}: record kind {}",
                    region.kind
                );
            }
        }
        assert_eq!(constant_ranges.len(), reference.chunks.len());
        let decoded_constants =
            keleusma::selfhost::wire_decode_constant_pools_via_kel(raw_auxiliary, &constant_ranges)
                .unwrap_or_else(|e| panic!("{name}: constant semantics failed: {e:?}"));
        assert_eq!(decoded_constants.len(), reference.chunks.len());
        for (actual_pool, expected_chunk) in decoded_constants.iter().zip(&reference.chunks) {
            assert_eq!(actual_pool.len(), expected_chunk.constants.len());
            assert_eq!(
                keleusma::wire_schema::encode_constants(actual_pool).unwrap(),
                keleusma::wire_schema::encode_constants(&expected_chunk.constants).unwrap(),
                "{name}: constant values"
            );
        }
        let decoded_layouts =
            keleusma::selfhost::wire_decode_layout_metadata_via_kel(raw_auxiliary)
                .unwrap_or_else(|e| panic!("{name}: layout semantics failed: {e:?}"));
        let expected_templates: Vec<_> = reference
            .chunks
            .iter()
            .flat_map(|c| c.struct_templates.iter().cloned())
            .collect();
        assert_eq!(
            decoded_layouts.templates.as_ref(),
            Some(&expected_templates),
            "{name}: templates"
        );
        assert_eq!(
            decoded_layouts.enum_layouts.as_ref(),
            Some(&reference.enum_layouts),
            "{name}: enum layouts"
        );
        let decoded_data = keleusma::selfhost::wire_decode_data_layout_via_kel(raw_auxiliary)
            .unwrap_or_else(|e| panic!("{name}: data layout semantics failed: {e:?}"));
        match (&decoded_data, &reference.data_layout) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!(a.slots, b.slots, "{name}: data slots");
                assert_eq!(a.shared_layout, b.shared_layout, "{name}: shared offsets");
                assert_eq!(
                    a.private_composite_layout, b.private_composite_layout,
                    "{name}: private placements"
                );
                assert_eq!(
                    keleusma::wire_schema::encode_constants(&a.private_init).unwrap(),
                    keleusma::wire_schema::encode_constants(&b.private_init).unwrap(),
                    "{name}: initializers"
                );
            }
            _ => panic!("{name}: data layout presence disagrees"),
        }
        let decoded_signatures =
            keleusma::selfhost::wire_decode_signature_metadata_via_kel(raw_auxiliary)
                .unwrap_or_else(|e| panic!("{name}: signature semantics failed: {e:?}"));
        assert!(
            decoded_signatures.parse_valid,
            "{name}: signature parsing was skipped"
        );
        assert_eq!(
            decoded_signatures.signatures.len(),
            reference.chunks.len(),
            "{name}: every compiled chunk must have a decoded signature"
        );
        assert_eq!(
            decoded_signatures.signatures, reference.signatures,
            "{name}: signatures"
        );
        assert_eq!(
            decoded_signatures.native_return_shapes, reference.native_return_shapes,
            "{name}: native return shapes"
        );
        assert!(!reference.chunks.is_empty(), "{name} has no chunks");
        let sections = keleusma::selfhost::wire_opcode_sections_via_kel(&actual)
            .unwrap_or_else(|e| panic!("{name}: opcode emission failed: {e:?}"));
        let decoded = keleusma::selfhost::wire_decode_opcode_sections_via_kel(
            &sections.opcodes,
            &sections.operands,
        )
        .unwrap_or_else(|e| panic!("{name}: opcode decoding failed: {e:?}"));
        let expected_ops: Vec<_> = reference
            .chunks
            .iter()
            .flat_map(|chunk| chunk.ops.iter().cloned())
            .collect();
        assert!(!expected_ops.is_empty(), "{name} has no opcodes");
        assert_eq!(decoded, expected_ops, "{name}: decoded opcode semantics");
        if actual_bytes != expected_bytes {
            let first = actual_bytes
                .iter()
                .zip(&expected_bytes)
                .position(|(a, b)| a != b);
            differences.push((name, actual_bytes.len(), expected_bytes.len(), first));
        }
        eprintln!(
            "{name}: {} chunks, pipeline {} bytes, reference {} bytes",
            actual.chunks.len(),
            actual_bytes.len(),
            expected_bytes.len()
        );
    }
    assert!(
        differences.is_empty(),
        "complete artifact differences {differences:?}"
    );
}

/// Branch-local names and shapes must not alter a sibling branch's bindings.
#[test]
fn branch_scope_restoration_preserves_complete_artifacts() {
    for source in [
        "fn main() -> Word { let x: Word = 1; if true { let x: bool = false; if x { 0 } else { 1 } } else { x } }",
        "struct P { x: Word } fn main() -> Word { if true { let p = P { x: 3 }; p.x } else { let p: Word = 4; p + 1 } }",
        "fn main() -> Word { let x: Word = 1; if true { let x: bool = false; if x { 0 } else { 1 } }; x }",
    ] {
        let tokens = keleusma::lexer::tokenize(source).expect("reference lex");
        let ast = keleusma::parser::parse(&tokens).expect("reference parse");
        let reference = keleusma::compiler::compile(&ast).expect("reference compile");
        let actual = keleusma::selfhost::self_host_compile_scratch(source);
        assert_eq!(
            actual.to_bytes().expect("pipeline artifact"),
            reference.to_bytes().expect("reference artifact"),
            "branch scope {source}"
        );
    }
}

/// Guards execute after pattern binding and before the result, in source arm order.
#[test]
fn match_guards_preserve_complete_artifacts_and_execution() {
    use keleusma::Arena;
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

    for (source, expected) in [
        (
            "fn main() -> Word { match 1 { 1 when false => 10, 1 when true => 20, _ => 30 } }",
            20,
        ),
        (
            "fn main() -> Word { match 1 { _ when false => 10, 1 => 20, _ => 30 } }",
            20,
        ),
        (
            "enum E { A, B } fn main() -> Word { let e = E::A(); match e { E::A when false => 10, E::A when true => 20, _ => 30 } }",
            20,
        ),
        (
            "enum E { A, B } fn main() -> Word { let e = E::A(); match e { _ when false => 10, E::A() when true => 20, _ => 30 } }",
            20,
        ),
        (
            "enum E { A(Word), B } fn main() -> Word { let e = E::A(3); match e { E::A(n) when n > 4 => 10, E::A(n) when n == 3 => n + 1, _ => 30 } }",
            4,
        ),
        (
            "fn main() -> Word { match 1 { 1 when if true { false } else { true } => 10, _ => 20 } }",
            20,
        ),
        (
            "fn main() -> Word { match 1 { 1 when match 2 { 2 => false, _ => true } => 10, _ => 20 } }",
            20,
        ),
        (
            "fn add(a: Word, b: Word) -> Word { a + b } fn main() -> Word { match 1 { 1 when true => if false { 10 } else { add(12, 8) }, _ => 30 } }",
            20,
        ),
        (
            "fn main() -> Word { match 2 { 1 when true => 10, _ when true => 20, _ => 30 } }",
            20,
        ),
        (
            "enum E { A(Word), B } fn main() -> Word { let n = 5; let e = E::A(3); let r = match e { E::A(n) when false => n, _ => n }; r + n }",
            10,
        ),
        (
            "fn choose(c: bool) -> Word { match 1 { 1 when c => 10, _ => 20 } } fn main() -> Word { choose(false) }",
            20,
        ),
        (
            "fn main() -> Word { 2 + match 1 { 1 when true => 18, _ => 30 } }",
            20,
        ),
        (
            "fn main() -> Word { match 1 { 1 when false => 1 / 0, _ => 20 } }",
            20,
        ),
        (
            "private data d { n: Word } fn guard() -> bool { d.n = d.n + 1; false } fn main() -> Word { let r = match 1 { 2 when guard() => 10, 1 when guard() => 20, _ => 30 }; r + d.n }",
            31,
        ),
    ] {
        eprintln!("guard control {source}");
        let tokens = keleusma::lexer::tokenize(source).expect("reference lex");
        let ast = keleusma::parser::parse(&tokens).expect("reference parse");
        let reference = keleusma::compiler::compile(&ast).expect("reference compile");
        let actual = keleusma::selfhost::self_host_compile_scratch(source);
        assert_eq!(actual.chunks.len(), reference.chunks.len());
        for (left, right) in actual.chunks.iter().zip(&reference.chunks) {
            assert_eq!(left.ops, right.ops, "guard ops {source}");
        }
        assert!(
            actual.to_bytes().expect("pipeline artifact")
                == reference.to_bytes().expect("reference artifact"),
            "guard artifact {source}\npipeline {actual:#?}\nreference {reference:#?}"
        );
        for module in [reference, actual] {
            let persistent = required_persistent_capacity_for(&module);
            let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
            arena
                .resize_persistent(persistent)
                .expect("persistent arena");
            let entry = module
                .chunks
                .iter()
                .position(|c| c.name == "main")
                .expect("main");
            let mut vm = Vm::new(module, &arena).expect("verified guard program");
            assert!(
                matches!(vm.call_function(entry, &[]).expect("execute guard"), VmState::Finished(Value::Int(n)) if n == expected),
                "guard value {source}"
            );
        }
    }
}

/// A final wildcard keeps its compact trailer, preserving the prior twelve-arm capacity.
#[test]
fn trailing_wildcard_preserves_enum_match_table_capacity() {
    let variants = (0..12)
        .map(|i| format!("V{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let arms = (0..12)
        .map(|i| format!("E::V{i} => {i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "enum E {{ {variants} }} fn main() -> Word {{ let e = E::V0(); match e {{ {arms}, _ => 12 }} }}"
    );
    let tokens = keleusma::lexer::tokenize(&source).expect("reference lex");
    let ast = keleusma::parser::parse(&tokens).expect("reference parse");
    let reference = keleusma::compiler::compile(&ast).expect("reference compile");
    let actual = keleusma::selfhost::self_host_compile_scratch(&source);
    assert!(
        actual.to_bytes().expect("pipeline artifact")
            == reference.to_bytes().expect("reference artifact")
    );
}
