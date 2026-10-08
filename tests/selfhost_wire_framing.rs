//! Module-driven section production is compared with the reference.
//! Broader auxiliary support, semantic decoding and shipping integration remain open.
#![cfg(feature = "self-host")]

use keleusma::bytecode::Module;
use keleusma::selfhost::{
    wire_artifact_from_sections_via_kel, wire_auxiliary_from_module_via_kel,
    wire_container_from_regions_via_kel, wire_framing_is_intact_via_kel,
    wire_opcode_sections_via_kel,
};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn section(bytes: &[u8], offset_field: usize) -> &[u8] {
    let at = u32_at(bytes, offset_field);
    &bytes[at..at + u32_at(bytes, offset_field + 4)]
}

#[test]
fn framing_matches_every_current_stage_artifact() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/kel");
    let mut paths: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "kel"))
        .collect();
    paths.sort();
    let names: std::collections::BTreeSet<_> = paths
        .iter()
        .map(|p| p.file_stem().unwrap().to_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "analyze",
            "codegen",
            "lexer",
            "parse",
            "reconstruct",
            "verify_datalayout",
            "verify_depth",
            "verify_structural",
            "verify_typed",
            "verify_types",
            "verify_yield",
            "wire",
            "wire_debug"
        ]
        .into_iter()
        .collect()
    );
    let mut large = 0;
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let module = compile(&source);
        let expected = module.to_bytes().unwrap();
        let auxiliary = wire_auxiliary_from_module_via_kel(&module)
            .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
        assert_eq!(
            auxiliary,
            section(&expected, 48),
            "auxiliary {}",
            path.display()
        );
        let instructions = wire_opcode_sections_via_kel(&module).unwrap();
        assert_eq!(
            instructions.opcodes,
            section(&expected, 32),
            "opcodes {}",
            path.display()
        );
        assert_eq!(
            instructions.operands,
            section(&expected, 40),
            "operands {}",
            path.display()
        );
        let actual = wire_artifact_from_sections_via_kel(
            &module,
            &instructions.opcodes,
            &instructions.operands,
            &auxiliary,
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", path.display());
        assert!(wire_framing_is_intact_via_kel(&actual).unwrap());
        assert!(Module::from_bytes(&actual).is_ok());
        if actual.len() > 65536 {
            large += 1;
        }
        eprintln!(
            "{}: {} exact bytes",
            path.file_stem().unwrap().to_str().unwrap(),
            actual.len()
        );
    }
    assert!(
        large >= 2,
        "the corpus must exercise multiple transport windows"
    );
}

#[test]
fn auxiliary_preserves_metadata_and_refuses_unintegrated_records() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.flags = 1;
    module.wcet_cycles = 123;
    module.wcmu_bytes = 456;
    module.shared_data_bytes = 789;
    module.private_data_bytes = 987;
    module.schema_hash = 0x12345678;
    let expected = module.to_bytes().unwrap();
    assert_eq!(
        wire_auxiliary_from_module_via_kel(&module).unwrap(),
        section(&expected, 48)
    );
    let mut native = module.clone();
    native.native_names.push("native".into());
    let expected_native = native.to_bytes().unwrap();
    assert_eq!(
        wire_auxiliary_from_module_via_kel(&native).unwrap(),
        section(&expected_native, 48)
    );
    let mut composite = module.clone();
    composite.chunks[0]
        .constants
        .push(keleusma::bytecode::ConstValue::Tuple(vec![]));
    assert!(wire_auxiliary_from_module_via_kel(&composite).is_err());
    module.chunks.clear();
    module.signatures.clear();
    module.entry_point = None;
    let expected = module.to_bytes().unwrap();
    assert_eq!(
        wire_auxiliary_from_module_via_kel(&module).unwrap(),
        section(&expected, 48)
    );
}

#[test]
fn auxiliary_refuses_oversized_inputs_without_packer_panics() {
    let module = compile("fn main() -> Word { 7 }");
    let mut long_name = module.clone();
    long_name.chunks[0].name = "a".repeat(65536);
    assert!(wire_auxiliary_from_module_via_kel(&long_name).is_err());
    let mut many_chunks = module.clone();
    many_chunks.chunks = vec![module.chunks[0].clone(); 1025];
    assert!(wire_auxiliary_from_module_via_kel(&many_chunks).is_err());
    let mut many_nodes = module.clone();
    many_nodes.chunks[0].constants = vec![keleusma::bytecode::ConstValue::Int(0); 65536];
    assert!(wire_auxiliary_from_module_via_kel(&many_nodes).is_err());
}

#[test]
fn framing_preserves_every_resource_field_and_padding() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.wcet_cycles = 0x12345678;
    module.wcmu_bytes = 0x23456789;
    module.shared_data_bytes = 0x34567890;
    module.private_data_bytes = 0x45678901;
    module.aux_arena_bytes = 0x56789012;
    module.persistent_composite_bytes = 0x67890123;
    module.flags = 1;
    let expected = module.to_bytes().unwrap();
    let actual = wire_artifact_from_sections_via_kel(
        &module,
        section(&expected, 32),
        section(&expected, 40),
        section(&expected, 48),
    )
    .unwrap();
    assert_eq!(actual, expected);
    for length in [0, 1, 7, 8, 65464, 65468, 65469, 65536, 131072] {
        let auxiliary = vec![0xA5; length];
        let artifact =
            wire_artifact_from_sections_via_kel(&module, &[0; 4], &[0; 8], &auxiliary).unwrap();
        assert_eq!(u32_at(&artifact, 32), 64);
        assert_eq!(u32_at(&artifact, 40), 72);
        assert_eq!(u32_at(&artifact, 48), 80);
        assert_eq!(&artifact[68..72], &[0; 4]);
        assert_eq!(section(&artifact, 48), auxiliary);
        assert_eq!(keleusma_wire::crc32(&artifact), 0x2144DF1C);
        assert!(wire_framing_is_intact_via_kel(&artifact).unwrap());
    }
}

#[test]
fn section_alignment_and_artifact_corruption_are_refused() {
    let module = compile("fn main() -> Word { 7 }");
    assert!(wire_artifact_from_sections_via_kel(&module, &[0; 3], &[], &[]).is_err());
    assert!(wire_artifact_from_sections_via_kel(&module, &[], &[0; 7], &[]).is_err());
    let bytes =
        wire_artifact_from_sections_via_kel(&module, &[0; 4], &[], &vec![0x5A; 131072]).unwrap();
    for at in [0, 4, 6, 8, 32, 40, 48, 64, 65535, 65536, bytes.len() - 1] {
        let mut corrupt = bytes.clone();
        corrupt[at] ^= 1;
        assert!(
            !wire_framing_is_intact_via_kel(&corrupt).unwrap(),
            "corruption at {at}"
        );
    }
    for length in [0, 63, 64, 67, bytes.len() - 1] {
        assert!(!wire_framing_is_intact_via_kel(&bytes[..length]).unwrap());
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(!wire_framing_is_intact_via_kel(&trailing).unwrap());
}

#[test]
fn malformed_layout_is_rejected_even_with_a_valid_checksum() {
    let module = compile("fn main() -> Word { 7 }");
    let bytes = module.to_bytes().unwrap();
    for at in [0, 4, 6, 8, 32, 36, 40, 44, 48, 52] {
        let mut corrupt = bytes.clone();
        corrupt[at] ^= 1;
        let last = corrupt.len() - 4;
        let crc = keleusma_wire::crc32(&corrupt[..last]);
        corrupt[last..].copy_from_slice(&crc.to_le_bytes());
        assert_eq!(keleusma_wire::crc32(&corrupt), 0x2144DF1C);
        assert!(
            !wire_framing_is_intact_via_kel(&corrupt).unwrap(),
            "layout field {at}"
        );
    }
}

#[test]
fn container_layout_matches_reference_at_region_and_window_boundaries() {
    fn compare(regions: &[(u16, &[u8])]) {
        let mut reference = keleusma_wire::WireBuilder::new();
        for &(kind, bytes) in regions {
            let id = reference.region(kind, 0).unwrap();
            reference.push(id, bytes);
        }
        let actual = wire_container_from_regions_via_kel(regions).unwrap();
        assert_eq!(actual, reference.finish().unwrap());
    }
    compare(&[]);
    compare(&[
        (7, &[]),
        (2, &[1]),
        (9, &[2; 7]),
        (1, &[3; 8]),
        (8, &[4; 9]),
    ]);
    let large = vec![0xA5; 131073];
    compare(&[(12, &large), (3, &[0xFF])]);
    let capacity: Vec<_> = (0..1024).map(|kind| (kind, &[][..])).collect();
    compare(&capacity);
    let overflow: Vec<_> = (0..1025).map(|kind| (kind, &[][..])).collect();
    assert!(wire_container_from_regions_via_kel(&overflow).is_err());
    assert!(wire_container_from_regions_via_kel(&[(1, &[]), (1, &[])]).is_err());
}
