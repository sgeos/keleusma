//! Fixed-record fields are transported across bounded windows without narrowing.
#![cfg(feature = "self-host")]

use keleusma::selfhost::{WireSchemaRecord, wire_schema_records_via_kel};
use keleusma::wire_schema::{ConstRecord, EnumAux, NameRef, kind};
use keleusma_wire::WireRecord;

fn encode<T: WireRecord>(records: &[T]) -> Vec<u8> {
    let mut out = vec![0; records.len() * T::STRIDE];
    for (record, bytes) in records.iter().zip(out.chunks_exact_mut(T::STRIDE)) {
        record.write_record(bytes).unwrap();
    }
    out
}

#[test]
fn tables_larger_than_the_window_preserve_every_record() {
    let expected: Vec<_> = (0..9000)
        .map(|n| NameRef {
            offset: n,
            length: u32::MAX - n,
        })
        .collect();
    let bytes = encode(&expected);
    assert!(bytes.len() > 65536);
    let decoded = wire_schema_records_via_kel(kind::NAMES, &bytes).unwrap();
    assert_eq!(decoded.len(), expected.len());
    for (actual, expected) in decoded.iter().zip(&expected) {
        assert_eq!(actual, &WireSchemaRecord::NameRef(*expected));
    }
    let mut unaligned = vec![0];
    unaligned.extend_from_slice(&bytes);
    assert_eq!(
        wire_schema_records_via_kel(kind::NAMES, &unaligned[1..]).unwrap(),
        decoded
    );
}

#[test]
fn signed_and_unsigned_full_width_payloads_preserve_their_bits() {
    let constants: Vec<_> = [0, 1, i64::MAX as u64, 1u64 << 63, u64::MAX]
        .into_iter()
        .map(|payload| ConstRecord {
            tag: u16::MAX,
            flags: u16::MAX,
            aux: u32::MAX,
            payload,
        })
        .collect();
    let enums: Vec<_> = [i64::MIN, -1, 0, 1, i64::MAX]
        .into_iter()
        .map(|discriminant| EnumAux {
            type_name: u32::MAX,
            variant: u32::MAX,
            discriminant,
        })
        .collect();
    assert_eq!(
        wire_schema_records_via_kel(kind::CONSTS, &encode(&constants)).unwrap(),
        constants
            .into_iter()
            .map(WireSchemaRecord::ConstRecord)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        wire_schema_records_via_kel(kind::ENUM_AUX, &encode(&enums)).unwrap(),
        enums
            .into_iter()
            .map(WireSchemaRecord::EnumAux)
            .collect::<Vec<_>>()
    );
}

#[test]
fn byte_pools_and_unknown_kinds_are_not_fixed_record_tables() {
    for kind in [
        0,
        kind::STRING_POOL,
        kind::PARAM_TYPES,
        kind::DEBUG_POOL,
        kind::CRYPTO_SIGNATURES,
        u16::MAX,
    ] {
        let error = wire_schema_records_via_kel(kind, &[]).unwrap_err();
        assert!(
            matches!(error, keleusma::selfhost::SelfHostError::Unsupported { detail }
            if detail.contains("command 218 refused with -290"))
        );
    }
    assert!(wire_schema_records_via_kel(kind::NAMES, &[0; 9]).is_err());
    assert!(
        wire_schema_records_via_kel(kind::HEADER, &[])
            .unwrap()
            .is_empty()
    );
}
