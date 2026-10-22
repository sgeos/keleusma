//! Whole-module decoding is compared with the low-level reference codec.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{Module, Value};
use keleusma::selfhost::{compile_src, wire_decode_module_via_kel};
use keleusma::wire_format::{module_from_wire_bytes, parse_wire_sections};
use keleusma::wire_schema;

fn put(bytes: &mut [u8], at: usize, value: usize) {
    bytes[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
}
fn fix_crc(bytes: &mut Vec<u8>) {
    bytes.truncate(bytes.len() - 4);
    let crc = keleusma_wire::crc32(bytes);
    bytes.extend_from_slice(&crc.to_le_bytes());
}
fn sections(module: &Module) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let bytes = module.to_bytes().unwrap();
    let s = parse_wire_sections(&bytes).unwrap();
    (
        bytes[..64].to_vec(),
        s.opcode_stream.to_vec(),
        s.operand_pool.to_vec(),
        s.aux_body.to_vec(),
    )
}
fn assemble(mut header: Vec<u8>, op: &[u8], pool: &[u8], aux: &[u8]) -> Vec<u8> {
    let hlen = header.len();
    header[6..8].copy_from_slice(&(hlen as u16).to_le_bytes());
    // Reverse section order, introduce gaps, and make every start unaligned.
    header.extend_from_slice(&[0xAA; 5]);
    let aux_at = header.len();
    header.extend_from_slice(aux);
    header.extend_from_slice(&[0xBB; 3]);
    let pool_at = header.len();
    header.extend_from_slice(pool);
    header.extend_from_slice(&[0xCC; 3]);
    let op_at = header.len();
    header.extend_from_slice(op);
    for (at, start, len) in [
        (32, op_at, op.len()),
        (40, pool_at, pool.len()),
        (48, aux_at, aux.len()),
    ] {
        put(&mut header, at, start);
        put(&mut header, at + 4, len);
    }
    let total = header.len() + 4;
    put(&mut header, 8, total);
    header.extend_from_slice(&[0; 4]);
    fix_crc(&mut header);
    header
}
fn compare(bytes: &[u8]) -> Option<Module> {
    match (
        wire_decode_module_via_kel(bytes),
        module_from_wire_bytes(bytes),
    ) {
        (Ok(actual), Ok(expected)) => {
            assert_eq!(actual.to_bytes().unwrap(), expected.to_bytes().unwrap());
            assert_eq!(actual.chunks.len(), expected.chunks.len());
            for (a, b) in actual.chunks.iter().zip(&expected.chunks) {
                assert_eq!(a.ops, b.ops);
                assert_eq!(a.debug_pool, b.debug_pool);
            }
            Some(actual)
        }
        (Err(_), Err(_)) => None,
        (a, b) => panic!("module codec mismatch {a:?} versus {b:?}"),
    }
}

#[test]
fn noncanonical_offsets_prefixes_and_trailing_bytes_preserve_values() {
    let module = compile_src("fn main() -> Word { 7 }");
    assert!(compare(&module.to_bytes().unwrap()).is_some());
    let (mut header, op, pool, aux) = sections(&module);
    put(&mut header, 56, 123);
    put(&mut header, 60, 456);
    let mut bytes = assemble(header, &op, &pool, &aux);
    assert!(
        !keleusma::selfhost::wire_framing_is_intact_via_kel(&bytes).unwrap(),
        "control must exceed the canonical reader's admission"
    );
    let actual = compare(&bytes).unwrap();
    assert_eq!(
        (actual.aux_arena_bytes, actual.persistent_composite_bytes),
        (123, 456)
    );
    bytes.extend_from_slice(b"ignored trailing bytes\n");
    assert!(compare(&bytes).is_some());
    for n in [0, 1021, 1022, 1023, 1024, 2048] {
        let mut prefixed = b"#!".to_vec();
        prefixed.extend(vec![b'x'; n]);
        prefixed.push(b'\n');
        prefixed.extend_from_slice(&bytes);
        assert!(compare(&prefixed).is_some(), "prefix {n}");
    }
    assert!(compare(b"#!no newline").is_none());
    let mut unaligned = vec![0];
    unaligned.extend_from_slice(&bytes);
    assert!(compare(&unaligned[1..]).is_some());
}

#[test]
fn selected_opcode_spans_and_unused_pool_parity_match() {
    let module = compile_src("fn main() -> Word { 7 }");
    let (header, op, pool, aux) = sections(&module);
    assert!(pool.is_empty(), "control needs no used pool records");
    let mut a = wire_schema::decode_aux_body(&aux).unwrap();
    for c in &mut a.chunks {
        c.op_byte_offset += 1;
    }
    let aux = wire_schema::encode_aux_body(&a).unwrap();
    let mut shifted = vec![255];
    shifted.extend_from_slice(&op);
    shifted.extend_from_slice(&[255; 3]);
    assert!(
        compare(&assemble(header.clone(), &shifted, &pool, &aux)).is_some(),
        "unaligned chunk and unused invalid opcodes must be accepted"
    );
    let unused = [255, 255, 0, 0, 0, 0, 0, 0]; // unknown tag, valid XOR parity
    assert!(compare(&assemble(header.clone(), &shifted, &unused, &aux)).is_some());
    let mut bad = unused;
    bad[1] ^= 1;
    assert!(
        compare(&assemble(header.clone(), &shifted, &bad, &aux)).is_none(),
        "unused entry parity must still be checked"
    );
    a.chunks[0].op_record_count = u32::MAX;
    assert!(
        compare(&assemble(
            header.clone(),
            &shifted,
            &pool,
            &wire_schema::encode_aux_body(&a).unwrap()
        ))
        .is_none()
    );
    a.chunks.clear();
    let aux = wire_schema::encode_aux_body(&a).unwrap();
    assert!(
        compare(&assemble(header.clone(), &[255; 4], &unused, &aux)).is_some(),
        "zero chunks ignore the opcode section"
    );
    assert!(compare(&assemble(header, &[255; 4], &bad, &aux)).is_none());
}

#[test]
fn extension_metadata_is_checked_without_authenticating_signature_bytes() {
    let module = compile_src("fn main() -> Word { 7 }");
    let (header, op, pool, aux) = sections(&module);
    for (flags, len) in [(2, 136), (6, 224), (4, 64)] {
        let mut a = wire_schema::decode_aux_body(&aux).unwrap();
        a.flags = flags;
        let aux = wire_schema::encode_aux_body(&a).unwrap();
        let mut h = header.clone();
        h.resize(len, 0);
        h[15] = flags;
        if len > 64 {
            h[64] = 1;
            h[66] = 64;
        }
        let good = assemble(h.clone(), &op, &pool, &aux);
        assert!(compare(&good).is_some(), "flags {flags}, length {len}");
        if len > 64 {
            for index in [64, 65, 66, 67, 68, 69, 70, 71] {
                let mut bad = good.clone();
                bad[index] ^= 2;
                fix_crc(&mut bad);
                assert!(compare(&bad).is_none(), "metadata byte {index}");
            }
            let mut opaque = good.clone();
            opaque[72] = 255;
            fix_crc(&mut opaque);
            assert!(
                compare(&opaque).is_some(),
                "this codec does not authenticate the signature"
            );
        }
        h[15] ^= 2;
        assert!(compare(&assemble(h, &op, &pool, &aux)).is_none());
    }
}

#[test]
fn corrupt_headers_bounds_widths_and_mirrors_are_refused() {
    let bytes = compile_src("fn main() -> Word { 7 }").to_bytes().unwrap();
    for end in 0..68 {
        assert!(compare(&bytes[..end]).is_none(), "prefix {end}");
    }
    for index in [0, 4, 6, 8, 12, 13, 14, 15, 16, 20, 24, 28] {
        let mut bad = bytes.clone();
        bad[index] ^= 128;
        fix_crc(&mut bad);
        assert!(compare(&bad).is_none(), "header byte {index}");
    }
    let mut accepted = 0;
    for index in [32, 36, 40, 44, 48, 52] {
        let mut changed = bytes.clone();
        changed[index] ^= 128;
        fix_crc(&mut changed);
        accepted += usize::from(compare(&changed).is_some());
    }
    assert!(
        accepted > 0,
        "offset mutations can still describe valid sections"
    );
    let mut bad = bytes.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    assert!(compare(&bad).is_none());
    for at in [8, 32, 36, 40, 44, 48, 52] {
        let mut bad = bytes.clone();
        put(&mut bad, at, u32::MAX as usize);
        fix_crc(&mut bad);
        assert!(compare(&bad).is_none(), "overflow field {at}");
    }
}

#[test]
fn complete_decoded_module_executes_with_debug_and_persistent_state() {
    use keleusma::debug_meta::{DebugPool, DebugRecord, DebugRecordKind};
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    let mut original = compile_src(
        "shared data io { values: [Word; 3] } private data d { value: Word = 7 } fn add(a: Word, b: Word) -> Word { a + b } fn main(i: Word) -> Word { d.value = add(d.value, io.values[i]); d.value }",
    );
    for c in &mut original.chunks {
        c.debug_pool = Some(DebugPool {
            string_pool: vec!["λ".into()],
            records: vec![DebugRecord {
                op_index: 0,
                kind: DebugRecordKind::LineNumber,
                operands: vec![42],
            }],
            ..DebugPool::default()
        });
    }
    let module = compare(&original.to_bytes().unwrap()).unwrap();
    drop(original);
    let persistent = required_persistent_capacity_for(&module);
    let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
    arena.resize_persistent(persistent).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 2, Value::Int(5)).unwrap();
    for expected in [12, 17] {
        assert!(
            matches!(vm.call_with_shared(&mut shared,&[Value::Int(2)]).unwrap(),VmState::Finished(Value::Int(n)) if n==expected)
        );
    }
}

#[test]
fn debug_payload_rejection_occurs_at_complete_module_layer() {
    let module = compile_src("fn main() -> Word { 7 }");
    let (header, op, pool, aux) = sections(&module);
    for raw in [vec![], vec![255], vec![0; 16]] {
        let mut a = wire_schema::decode_aux_body(&aux).unwrap();
        a.chunks[0].debug_pool_bytes = Some(raw.clone());
        let aux = wire_schema::encode_aux_body(&a).unwrap();
        assert!(keleusma::selfhost::wire_decode_aux_body_via_kel(&aux).is_ok());
        assert_eq!(
            compare(&assemble(header.clone(), &op, &pool, &aux)).is_some(),
            raw.len() == 16
        );
    }
}

#[test]
fn overlapping_sections_are_not_rejected_as_a_new_policy() {
    let module = compile_src("fn main() -> Word { 7 }");
    let mut bytes = module.to_bytes().unwrap();
    let op_len = u32::from_le_bytes(bytes[36..40].try_into().unwrap()) as usize;
    put(&mut bytes, 36, op_len + 4);
    put(&mut bytes, 40, 65); // unused zero-length pool within the opcode span
    fix_crc(&mut bytes);
    assert!(compare(&bytes).is_some());
}
