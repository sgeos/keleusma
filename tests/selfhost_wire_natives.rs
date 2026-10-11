//! Native names and return shapes retain independent lengths and shared indices.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{ChunkSignature, Module, WireShape};
use keleusma::selfhost::{wire_auxiliary_from_module_via_kel, wire_module_artifact_via_kel};
use keleusma::wire_schema::{ModuleTable, SignatureTable};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}

fn compare(module: &Module) -> Vec<u8> {
    let reference = module.to_bytes().unwrap();
    let offset = u32::from_le_bytes(reference[48..52].try_into().unwrap()) as usize;
    let length = u32::from_le_bytes(reference[52..56].try_into().unwrap()) as usize;
    let actual = wire_auxiliary_from_module_via_kel(module).unwrap();
    assert_eq!(actual, reference[offset..offset + length]);
    assert_eq!(wire_module_artifact_via_kel(module).unwrap(), reference);
    let table = ModuleTable::parse(&actual).unwrap();
    assert_eq!(table.native_count(), module.native_names.len());
    assert_eq!(
        table.native_return_count(),
        module.native_return_shapes.len()
    );
    for (i, name) in module.native_names.iter().enumerate() {
        assert_eq!(table.native_name(i), Some(name.as_bytes()));
    }
    let shapes = SignatureTable::parse(&actual).unwrap();
    for (i, shape) in module.native_return_shapes.iter().enumerate() {
        assert_eq!(
            shapes.shape(table.native_return_shape(i).unwrap()),
            Some(*shape)
        );
    }
    actual
}

#[test]
fn compiled_native_calls_preserve_complete_artifacts() {
    for source in [
        "use host::tick(Word) -> Word\nfn main() -> Word { host::tick(7) }",
        "use host::mk() -> [Word; 3]\nfn main() -> Word { host::mk()[1] }",
    ] {
        let module = compile(source);
        assert!(!module.native_names.is_empty());
        assert!(!module.native_return_shapes.is_empty());
        compare(&module);
    }
}

#[test]
fn native_names_share_with_chunks_enums_and_later_data_slots() {
    let mut module = compile("enum E { A, B } shared data d { x: Word } fn main() -> Word { d.x }");
    module.chunks[0].name = "E".into();
    module.native_names = ["E", "A", "A", "d.x", "outside", "", "λ"]
        .into_iter()
        .map(Into::into)
        .collect();
    let bytes = compare(&module);
    let table = ModuleTable::parse(&bytes).unwrap();
    assert_eq!(table.native_name(0), Some(&b"E"[..]));
    assert_eq!(table.native_name(1), table.native_name(2));
}

#[test]
fn native_returns_reuse_the_chunk_shape_interner_and_each_other() {
    let mut module = compile("fn main() -> Word { 7 }");
    let scalar = WireShape::Scalar { kind: 1 };
    let flat = WireShape::Flat { kind: 2, size: 16 };
    let added = WireShape::Flat { kind: 3, size: 32 };
    module.signatures = vec![ChunkSignature {
        params: vec![WireShape::Top, scalar, WireShape::Top],
        ret: WireShape::Top,
        resume: flat,
    }];
    module.native_names = vec!["first".into(), "second".into()];
    module.native_return_shapes = vec![WireShape::Top, scalar, flat, added, added];
    let bytes = compare(&module);
    let table = ModuleTable::parse(&bytes).unwrap();
    let indices: Vec<_> = (0..5)
        .map(|i| table.native_return_shape(i).unwrap())
        .collect();
    assert_eq!(indices, [0, 1, 3, 4, 4]);
}

#[test]
fn native_vectors_preserve_independent_empty_and_unequal_lengths() {
    for (names, shapes) in [(0, 3), (3, 0), (3, 1), (1, 3), (0, 0)] {
        let mut module = compile("fn main() -> Word { 7 }");
        module.chunks.clear();
        module.signatures.clear();
        module.entry_point = None;
        module.native_names = (0..names).map(|i| format!("native{i}")).collect();
        module.native_return_shapes = vec![WireShape::Top; shapes];
        compare(&module);
    }
}

#[test]
fn native_names_cross_former_event_input_and_pool_limits() {
    let mut module = compile("fn main() -> Word { 7 }");
    module.chunks.clear();
    module.signatures.clear();
    module.entry_point = None;
    module.native_names = (0..64)
        .map(|i| format!("{i:04}{}", "a".repeat(252)))
        .collect();
    compare(&module); // Exactly 16,384 distinct name bytes.
    module.native_names.push("overflow".into());
    compare(&module);
    module.native_names = vec!["a".repeat(257)];
    compare(&module);
    module.native_names = vec!["a".into(); 1025];
    compare(&module);
    module.native_names = vec!["a".repeat(256); 200];
    compare(&module);
}
