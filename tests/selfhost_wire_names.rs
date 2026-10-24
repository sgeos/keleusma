//! Name lookup preserves borrowed storage and validates only requested text.
#![cfg(feature = "self-host")]

use keleusma::selfhost::wire_name_via_kel;
use keleusma::wire_schema::{ModuleTable, NameRef, kind};
use keleusma_wire::WireBuilder;

fn artifact(names: &[NameRef], pool: &[u8]) -> Vec<u8> {
    let mut b = WireBuilder::new();
    let region = b.region(kind::NAMES, 0).unwrap();
    for name in names {
        b.push_record(region, name);
    }
    let region = b.region(kind::STRING_POOL, 0).unwrap();
    b.push(region, pool);
    b.finish().unwrap()
}

fn compare(bytes: &[u8], index: u32) {
    let view = ModuleTable::parse(bytes).unwrap();
    let expected = view
        .name_bytes(index)
        .and_then(|b| core::str::from_utf8(b).ok());
    match (wire_name_via_kel(bytes, index), expected) {
        (Ok(actual), Some(expected)) => {
            assert_eq!(actual, expected);
            assert_eq!(actual.as_ptr(), expected.as_ptr());
        }
        (Err(_), None) => {}
        (a, b) => panic!("name {index}: {a:?} versus {b:?}"),
    }
}

#[test]
fn requested_names_preserve_aliases_and_ignore_unused_corruption() {
    let bytes = artifact(
        &[
            NameRef {
                offset: 0,
                length: 4,
            },
            NameRef {
                offset: 4,
                length: 1,
            },
            NameRef {
                offset: u32::MAX,
                length: 1,
            },
            NameRef {
                offset: 8,
                length: 0,
            },
        ],
        b"name\xff",
    );
    for index in [0, 1, 2, 3, 4, u32::MAX] {
        compare(&bytes, index);
    }
    assert_eq!(wire_name_via_kel(&bytes, 0).unwrap(), "name");
    assert_eq!(wire_name_via_kel(&bytes, 3).unwrap(), "");
    let mut unaligned = vec![0];
    unaligned.extend_from_slice(&bytes);
    compare(&unaligned[1..], 0);
}

#[test]
fn unicode_names_cross_byte_windows_without_a_name_length_cap() {
    let mut text = "x".repeat(1023);
    text.push_str("\u{10FFFF}\u{800}\u{80}\0");
    text.push_str(&"λ".repeat(40000));
    let bytes = artifact(
        &[NameRef {
            offset: 0,
            length: text.len() as u32,
        }],
        text.as_bytes(),
    );
    assert!(text.len() > 65536);
    compare(&bytes, 0);
    assert_eq!(wire_name_via_kel(&bytes, 0).unwrap(), text);
}

#[test]
fn malformed_unicode_and_missing_regions_are_refused() {
    for value in [
        &b"\xc0\x80"[..],
        &b"\xe0\x9f\xbf"[..],
        &b"\xed\xa0\x80"[..],
        &b"\xf0\x8f\xbf\xbf"[..],
        &b"\xf4\x90\x80\x80"[..],
        &b"\xe2\x82"[..],
    ] {
        let bytes = artifact(
            &[NameRef {
                offset: 0,
                length: value.len() as u32,
            }],
            value,
        );
        compare(&bytes, 0);
        assert!(wire_name_via_kel(&bytes, 0).is_err());
    }
    for kind in [kind::NAMES, kind::STRING_POOL] {
        let mut b = WireBuilder::new();
        b.region(kind, 0).unwrap();
        assert!(wire_name_via_kel(&b.finish().unwrap(), 0).is_err());
    }
}
