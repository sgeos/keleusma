//! Debug reading preserves input order, opaque blobs and trailing-byte behavior.
#![cfg(feature = "self-host")]
use keleusma::debug_meta::{DebugPool, DebugRecord, DebugRecordKind};
use keleusma::selfhost::{wire_decode_debug_pool_via_kel, wire_decode_parameter_tags_via_kel};
use keleusma::wire_schema;

fn raw(pool: &DebugPool) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(pool.string_pool.len() as u32).to_le_bytes());
    for s in &pool.string_pool {
        bytes.extend_from_slice(&(s.len() as u32).to_le_bytes());
        bytes.extend_from_slice(s.as_bytes());
    }
    bytes.extend_from_slice(&(pool.span_pool.len() as u32).to_le_bytes());
    for &(file, offset, len) in &pool.span_pool {
        bytes.extend_from_slice(&file.to_le_bytes());
        bytes.extend_from_slice(&offset.to_le_bytes());
        bytes.extend_from_slice(&len.to_le_bytes());
    }
    bytes.extend_from_slice(&(pool.type_pool.len() as u32).to_le_bytes());
    for t in &pool.type_pool {
        bytes.extend_from_slice(&(t.len() as u32).to_le_bytes());
        bytes.extend_from_slice(t);
    }
    bytes.extend_from_slice(&(pool.records.len() as u32).to_le_bytes());
    for r in &pool.records {
        bytes.extend_from_slice(&r.op_index.to_le_bytes());
        bytes.push(r.kind.as_u8());
        bytes.extend_from_slice(&(r.operands.len() as u16).to_le_bytes());
        for n in &r.operands {
            bytes.extend_from_slice(&n.to_le_bytes());
        }
    }
    bytes
}

#[test]
fn noncanonical_debug_order_and_unresolved_indices_match_reference() {
    let expected = DebugPool {
        string_pool: vec!["λ\0\u{10FFFF}".into(), String::new()],
        span_pool: vec![(65535, u32::MAX, u32::MAX)],
        type_pool: vec![vec![255, 0, 128]],
        records: (0..12)
            .map(|kind| DebugRecord {
                op_index: 100 - u32::from(kind),
                kind: DebugRecordKind::from_u8(kind).unwrap(),
                operands: vec![65535; usize::from(kind % 4)],
            })
            .collect(),
    };
    let mut bytes = raw(&expected);
    bytes.extend_from_slice(&[255, 1, 2]);
    assert_eq!(DebugPool::decode(&bytes).unwrap(), expected);
    assert_eq!(wire_decode_debug_pool_via_kel(&bytes).unwrap(), expected);
    assert_ne!(
        raw(&expected),
        expected.encode(),
        "control must be noncanonical"
    );
}

#[test]
fn large_strings_types_and_operand_lists_cross_transport_windows() {
    let expected = DebugPool {
        string_pool: vec!["λ".repeat(5000)],
        span_pool: vec![(0, 1, 2)],
        type_pool: vec![vec![255; 3000]],
        records: vec![DebugRecord {
            op_index: u32::MAX,
            kind: DebugRecordKind::VerifierWitness,
            operands: (0..65535).collect(),
        }],
    };
    let bytes = raw(&expected);
    assert!(bytes.len() > 65536);
    assert_eq!(wire_decode_debug_pool_via_kel(&bytes).unwrap(), expected);
    let mut unaligned = vec![0];
    unaligned.extend_from_slice(&bytes);
    assert_eq!(
        wire_decode_debug_pool_via_kel(&unaligned[1..]).unwrap(),
        expected
    );
}

#[test]
fn parameter_ranges_ignore_unused_bytes_and_preserve_float_tag() {
    for tag in 0..=255u8 {
        let expected = wire_schema::type_tag_from_byte(tag);
        match (
            wire_decode_parameter_tags_via_kel(Some(&[tag]), (0, 1)),
            expected,
        ) {
            (Ok(a), Some(b)) => assert_eq!(a, vec![b]),
            (Err(_), None) => {}
            (a, b) => panic!("parameter tag {tag}: {a:?} versus {b:?}"),
        }
    }
    let pool = [0, 1, 2, 3, 4, 5, 6, 7, 8, 255];
    let expected = wire_schema::ParamTypeTable::from_bytes(&pool)
        .tags((1, 8))
        .unwrap();
    assert_eq!(
        wire_decode_parameter_tags_via_kel(Some(&pool), (1, 8)).unwrap(),
        expected
    );
    assert!(
        wire_decode_parameter_tags_via_kel(None, (u32::MAX, u32::MAX))
            .unwrap()
            .is_empty()
    );
    assert!(wire_decode_parameter_tags_via_kel(Some(&[]), (0, 1)).is_err());
    assert!(
        wire_decode_parameter_tags_via_kel(Some(&[]), (0, 0))
            .unwrap()
            .is_empty()
    );
    let long = vec![5; 4097];
    assert_eq!(
        wire_decode_parameter_tags_via_kel(Some(&long), (0, 4097))
            .unwrap()
            .len(),
        4097
    );
}

#[test]
fn truncation_invalid_text_and_unknown_record_kinds_are_rejected() {
    let pool = DebugPool {
        string_pool: vec!["λ".into()],
        span_pool: vec![(65535, u32::MAX, 0)],
        type_pool: vec![vec![255]],
        records: vec![DebugRecord {
            op_index: 9,
            kind: DebugRecordKind::CallSite,
            operands: vec![65535; 16],
        }],
    };
    let bytes = raw(&pool);
    for end in 0..bytes.len() {
        assert!(DebugPool::decode(&bytes[..end]).is_err());
        assert!(
            wire_decode_debug_pool_via_kel(&bytes[..end]).is_err(),
            "prefix {end}"
        );
    }
    let mut invalid = bytes.clone();
    invalid[8] = 255;
    assert!(DebugPool::decode(&invalid).is_err());
    assert!(wire_decode_debug_pool_via_kel(&invalid).is_err());
    let mut kinds = raw(&DebugPool {
        records: vec![DebugRecord {
            op_index: 0,
            kind: DebugRecordKind::CallSite,
            operands: vec![],
        }],
        ..DebugPool::default()
    });
    for kind in 0..=255u8 {
        kinds[20] = kind;
        assert_eq!(
            wire_decode_debug_pool_via_kel(&kinds).is_ok(),
            DebugPool::decode(&kinds).is_ok(),
            "kind {kind}"
        );
    }
    assert!(wire_decode_debug_pool_via_kel(&u32::MAX.to_le_bytes()).is_err());
}
