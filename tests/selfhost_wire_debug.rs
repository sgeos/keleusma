//! Structured debug pools must sort and encode through Keleusma.
#![cfg(feature = "self-host")]

use keleusma::debug_meta::{DebugPool, DebugRecord, DebugRecordKind};
use keleusma::selfhost::wire_debug_pool_via_kel;

fn module(source: &str, debug: bool) -> keleusma::bytecode::Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile_with_options(
        &ast,
        &keleusma::target::Target::host(),
        &keleusma::compiler::CompileOptions { emit_debug: debug },
    )
    .unwrap()
    .0
}

#[test]
fn complete_debug_artifacts_match_for_compiled_source() {
    for source in [
        "fn main() -> Word { 7 }",
        "fn double(n: Word) -> Word { n * 2 } fn main() -> Word { let x = 4; double(x) }",
    ] {
        let module = module(source, true);
        assert!(
            module
                .chunks
                .iter()
                .any(|c| c.debug_pool.as_ref().is_some_and(|p| !p.records.is_empty()))
        );
        let actual = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
        assert_eq!(actual, module.to_bytes().unwrap());
        let decoded = keleusma::bytecode::Module::from_bytes(&actual).unwrap();
        assert_eq!(decoded.to_bytes().unwrap(), actual);
    }
}

#[test]
fn absent_and_empty_debug_pools_have_distinct_chunk_spans() {
    let mut module = module("fn main() -> Word { 7 }", false);
    let original = module.chunks[0].clone();
    for i in 0..3 {
        let mut chunk = original.clone();
        chunk.name = format!("chunk{i}");
        module.chunks.push(chunk);
    }
    module.chunks[1].debug_pool = Some(DebugPool::default());
    module.chunks[3].debug_pool = Some(DebugPool {
        string_pool: vec!["x".into()],
        ..DebugPool::default()
    });
    let bytes = keleusma::selfhost::wire_auxiliary_from_module_via_kel(&module).unwrap();
    let table = keleusma::wire_schema::ModuleTable::parse(&bytes).unwrap();
    let spans: Vec<_> = (0..4)
        .map(|i| {
            let c = table.chunk(i).unwrap();
            (c.debug_first, c.debug_len)
        })
        .collect();
    assert_eq!(spans, [(u32::MAX, 0), (0, 16), (u32::MAX, 0), (16, 21)]);
    assert_eq!(
        keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap(),
        module.to_bytes().unwrap()
    );
}

fn record(op: u32, kind: u8, operands: Vec<u16>) -> DebugRecord {
    DebugRecord {
        op_index: op,
        kind: DebugRecordKind::from_u8(kind).unwrap(),
        operands,
    }
}

fn compare(pool: &DebugPool) -> Vec<u8> {
    let actual = wire_debug_pool_via_kel(pool).unwrap();
    assert_eq!(actual, pool.encode());
    assert_eq!(DebugPool::decode(&actual).unwrap().encode(), actual);
    actual
}

#[test]
fn debug_records_sort_by_op_kind_and_lexical_operands() {
    let pool = DebugPool {
        string_pool: vec!["".into(), "λ\0file.kel".into()],
        span_pool: vec![(u16::MAX, u32::MAX, 0), (0, 12, 34)],
        type_pool: vec![vec![], vec![0, 255, 17]],
        records: vec![
            record(2, 0, vec![]),
            record(1, 1, vec![0]),
            record(1, 0, vec![1, 2]),
            record(1, 0, vec![1]),
            record(1, 0, vec![]),
            record(1, 0, vec![0, 65535]),
            record(1, 0, vec![1]),
            record(0, 11, vec![65535]),
        ],
    };
    let actual = compare(&pool);
    let mut reversed = pool.clone();
    reversed.records.reverse();
    assert_eq!(compare(&reversed), actual);
}

#[test]
fn all_debug_record_kinds_and_empty_pools_are_represented() {
    assert_eq!(compare(&DebugPool::default()), [0; 16]);
    let kinds: Vec<_> = (0..=u8::MAX).filter_map(DebugRecordKind::from_u8).collect();
    assert_eq!(
        kinds.len(),
        12,
        "update the Keleusma kind validator when the catalogue changes"
    );
    let pool = DebugPool {
        records: kinds
            .into_iter()
            .rev()
            .map(|kind| record(u32::MAX, kind.as_u8(), vec![]))
            .collect(),
        ..DebugPool::default()
    };
    compare(&pool);
    for count in [1, 2, 3, 5, 8, 9, 17, 65, 257] {
        let pool = DebugPool {
            records: (0..count)
                .rev()
                .map(|i| record(i % 7, (i % 12) as u8, vec![(i % 17) as u16]))
                .collect(),
            ..DebugPool::default()
        };
        compare(&pool);
    }
}

#[test]
fn debug_strings_types_and_operands_cross_transfer_windows() {
    let mut a = vec![3; 1025];
    a[1024] = 4;
    let mut b = a.clone();
    b[1024] = 2;
    let pool = DebugPool {
        string_pool: vec!["x".repeat(8193)],
        span_pool: vec![(0, 4095, 4097)],
        type_pool: vec![vec![255; 8193]],
        records: vec![
            record(0, 0, a),
            record(0, 0, b),
            record(0, 0, vec![3; 1024]),
        ],
    };
    compare(&pool);
    let mut limit = DebugPool {
        records: vec![record(0, 0, vec![65535; 65535])],
        ..DebugPool::default()
    };
    compare(&limit);
    limit.records[0].operands.push(0);
    assert!(wire_debug_pool_via_kel(&limit).is_err());
}
