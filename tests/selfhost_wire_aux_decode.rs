//! Whole auxiliary decoding must preserve optional-table and selected-span policies.
#![cfg(feature = "self-host")]
use keleusma::selfhost::{compile_src, wire_decode_aux_body_via_kel};
use keleusma::wire_schema::{self as schema, ChunkRecord, HeaderRecord, kind};
use keleusma_wire::{WireBuilder, WireRecord, WireView};
use std::collections::BTreeMap;

fn tables(source: &str) -> BTreeMap<u16, Vec<u8>> {
    let module = compile_src(source);
    let bytes = module.to_bytes().unwrap();
    let aux = keleusma::wire_format::parse_wire_sections(&bytes)
        .unwrap()
        .aux_body;
    let view = WireView::parse(aux).unwrap();
    (0..view.region_count())
        .map(|i| {
            let r = view.region_at(i).unwrap();
            (r.kind, view.region_bytes(&r).unwrap().to_vec())
        })
        .collect()
}
fn encode(tables: &BTreeMap<u16, Vec<u8>>) -> Vec<u8> {
    let mut b = WireBuilder::new();
    for (&kind, bytes) in tables {
        let r = b.region(kind, 0).unwrap();
        b.push(r, bytes);
    }
    b.finish().unwrap()
}
fn compare(tables: &BTreeMap<u16, Vec<u8>>) -> bool {
    let bytes = encode(tables);
    match (
        wire_decode_aux_body_via_kel(&bytes),
        schema::decode_aux_body(&bytes),
    ) {
        (Ok(actual), Ok(expected)) => {
            // Canonical re-encoding observes every owned field, including constants' bits.
            assert_eq!(
                schema::encode_aux_body(&actual).unwrap(),
                schema::encode_aux_body(&expected).unwrap()
            );
            true
        }
        (Err(_), Err(_)) => false,
        (a, b) => panic!("auxiliary mismatch {a:?} versus {b:?}"),
    }
}
fn set_chunk(tables: &mut BTreeMap<u16, Vec<u8>>, offset: usize, value: u32) {
    tables.get_mut(&kind::CHUNKS).unwrap()[offset..offset + 4]
        .copy_from_slice(&value.to_le_bytes());
}

#[test]
fn complete_auxiliary_values_and_opaque_debug_payloads_match() {
    for source in [
        "fn main() -> Word { 7 }",
        "shared data d { x: Word, } fn main() -> Word { d.x = 7; d.x }",
        "private data d { x: Word = 4, } fn main() -> Word { d.x = d.x + 1; d.x }",
        "struct Pair { a: Word, b: Byte, } fn main() -> Word { let p = Pair { a: 7, b: 1 as Byte }; p.a }",
    ] {
        let mut t = tables(source);
        assert!(compare(&t));
        t.insert(kind::DEBUG_POOL, vec![255, 0, 1]);
        set_chunk(&mut t, ChunkRecord::OFFSET_DEBUG_FIRST, 0);
        set_chunk(&mut t, ChunkRecord::OFFSET_DEBUG_LEN, 3);
        assert!(compare(&t), "opaque debug bytes are not interpreted here");
        for (first, len) in [(0, 0), (3, 0), (4, 0), (0, 4), (u32::MAX, 3), (1, u32::MAX)] {
            set_chunk(&mut t, ChunkRecord::OFFSET_DEBUG_FIRST, first);
            set_chunk(&mut t, ChunkRecord::OFFSET_DEBUG_LEN, len);
            assert!(compare(&t));
        }
        t.remove(&kind::DEBUG_POOL);
        assert!(compare(&t));
    }
}

#[test]
fn parameter_absence_and_selected_ranges_preserve_reference_admission() {
    let mut t = tables("fn main() -> Word { 7 }");
    t.remove(&kind::PARAM_TYPES);
    set_chunk(&mut t, ChunkRecord::OFFSET_PARAM_TYPES_FIRST, u32::MAX);
    set_chunk(&mut t, ChunkRecord::OFFSET_PARAM_TYPES_COUNT, u32::MAX);
    assert!(compare(&t));
    t.insert(kind::PARAM_TYPES, vec![255, 5, 0]);
    set_chunk(&mut t, ChunkRecord::OFFSET_PARAM_TYPES_FIRST, 1);
    set_chunk(&mut t, ChunkRecord::OFFSET_PARAM_TYPES_COUNT, 1);
    assert!(
        compare(&t),
        "Float is a tag even without float execution support"
    );
    for (first, count, expected) in [
        (0, 1, false),
        (8, 0, true),
        (9, 0, false),
        (1, 8, false),
        (1, u32::MAX, false),
    ] {
        set_chunk(&mut t, ChunkRecord::OFFSET_PARAM_TYPES_FIRST, first);
        set_chunk(&mut t, ChunkRecord::OFFSET_PARAM_TYPES_COUNT, count);
        assert_eq!(compare(&t), expected, "range {first}/{count}");
    }
}

#[test]
fn missing_and_malformed_metadata_are_distinct_from_unused_tables() {
    let base = tables("fn main() -> Word { 7 }");
    for kind in [kind::NAMES, kind::STRING_POOL, kind::HEADER] {
        let mut t = base.clone();
        t.remove(&kind);
        assert!(!compare(&t));
    }
    for kind in [
        kind::NAMES,
        kind::HEADER,
        kind::CHUNKS,
        kind::NATIVES,
        kind::NATIVE_RETURNS,
    ] {
        let mut t = base.clone();
        t.insert(kind, vec![0]);
        // The container pads every region to eight bytes. A zero name or native
        // record is valid, whereas header/chunk records need more than one word.
        assert_eq!(
            compare(&t),
            !matches!(kind, kind::HEADER | kind::CHUNKS),
            "kind {kind}"
        );
    }
    for block in [0, 1, 2, 3, 4, 255] {
        let mut t = base.clone();
        t.get_mut(&kind::CHUNKS).unwrap()[ChunkRecord::OFFSET_BLOCK_TYPE] = block;
        assert_eq!(compare(&t), (1..=3).contains(&block));
    }
    for field in [
        ChunkRecord::OFFSET_NAME,
        ChunkRecord::OFFSET_TEMPLATES_FIRST,
        ChunkRecord::OFFSET_TEMPLATES_COUNT,
    ] {
        let mut t = base.clone();
        set_chunk(&mut t, field, u32::MAX);
        assert!(!compare(&t));
    }
    let mut no_chunks = base.clone();
    no_chunks.remove(&kind::CHUNKS);
    no_chunks.insert(kind::CONSTS, vec![255]);
    assert!(compare(&no_chunks), "unused constants are not decoded");
    let h = no_chunks.get_mut(&kind::HEADER).unwrap();
    h[HeaderRecord::OFFSET_ENTRY_POINT..HeaderRecord::OFFSET_ENTRY_POINT + 4]
        .copy_from_slice(&123u32.to_le_bytes());
    h[HeaderRecord::OFFSET_WORD_BITS_LOG2] = 255;
    let extra = h.clone();
    h.extend(extra);
    assert!(
        compare(&no_chunks),
        "auxiliary decoding copies widths and uses the first header"
    );
    assert_eq!(HeaderRecord::STRIDE, 32);
}

#[test]
fn native_vectors_remain_independent_and_optional_families_default() {
    let module = compile_src("fn main() -> Word { 7 }");
    let bytes = module.to_bytes().unwrap();
    let mut aux = schema::decode_aux_body(
        keleusma::wire_format::parse_wire_sections(&bytes)
            .unwrap()
            .aux_body,
    )
    .unwrap();
    aux.native_names = vec!["λ".into(), "other".into()];
    aux.native_return_shapes = vec![keleusma::bytecode::WireShape::Scalar { kind: 1 }];
    let bytes = schema::encode_aux_body(&aux).unwrap();
    let view = WireView::parse(&bytes).unwrap();
    let mut t: BTreeMap<_, _> = (0..view.region_count())
        .map(|i| {
            let r = view.region_at(i).unwrap();
            (r.kind, view.region_bytes(&r).unwrap().to_vec())
        })
        .collect();
    assert!(compare(&t));
    t.remove(&kind::NATIVES);
    assert!(compare(&t), "return shapes survive absent native names");
    t.remove(&kind::NATIVE_RETURNS);
    t.insert(kind::SIGNATURES, vec![255]);
    t.insert(kind::STRUCT_TEMPLATES, vec![255]);
    t.insert(kind::ENUM_LAYOUTS, vec![255]);
    assert!(compare(&t));
}

#[test]
fn decoded_auxiliary_fields_drive_persistent_execution() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    let mut module = compile_src(
        "shared data io { values: [Word; 3] } private data d { value: Word = 7 } fn main(i: Word) -> Word { d.value = d.value + io.values[i]; d.value }",
    );
    let artifact = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
    let bytes = keleusma::wire_format::parse_wire_sections(&artifact)
        .unwrap()
        .aux_body;
    let aux = wire_decode_aux_body_via_kel(bytes).unwrap();
    assert_eq!(module.chunks.len(), aux.chunks.len());
    // Keep only instruction bodies from the original. Every auxiliary field is replaced.
    for (chunk, decoded) in module.chunks.iter_mut().zip(aux.chunks) {
        chunk.name = decoded.name;
        chunk.constants = decoded.constants;
        chunk.struct_templates = decoded.struct_templates;
        chunk.local_count = decoded.local_count;
        chunk.param_count = decoded.param_count;
        chunk.block_type = decoded.block_type;
        chunk.param_types = decoded.param_types;
        chunk.debug_pool = decoded
            .debug_pool_bytes
            .map(|b| keleusma::selfhost::wire_decode_debug_pool_via_kel(&b).unwrap());
    }
    module.native_names = aux.native_names;
    module.entry_point = aux.entry_point;
    module.data_layout = aux.data_layout;
    module.word_bits_log2 = aux.word_bits_log2;
    module.addr_bits_log2 = aux.addr_bits_log2;
    module.float_bits_log2 = aux.float_bits_log2;
    module.wcet_cycles = aux.wcet_cycles;
    module.wcmu_bytes = aux.wcmu_bytes;
    module.flags = aux.flags;
    module.shared_data_bytes = aux.shared_data_bytes;
    module.private_data_bytes = aux.private_data_bytes;
    module.schema_hash = aux.schema_hash;
    module.enum_layouts = aux.enum_layouts;
    module.signatures = aux.signatures;
    module.native_return_shapes = aux.native_return_shapes;
    let persistent = required_persistent_capacity_for(&module);
    let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
    arena.resize_persistent(persistent).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 2, Value::Int(5)).unwrap();
    for expected in [12, 17] {
        assert!(
            matches!(vm.call_with_shared(&mut shared, &[Value::Int(2)]).unwrap(), VmState::Finished(Value::Int(n)) if n == expected)
        );
    }
}
