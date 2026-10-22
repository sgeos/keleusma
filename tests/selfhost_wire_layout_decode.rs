//! Layout decoding preserves shared parsing and independent optional families.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{EnumLayout, EnumVariantDisc, StructTemplate};
use keleusma::selfhost::{
    wire_decode_enum_layouts_via_kel, wire_decode_layout_metadata_via_kel,
    wire_decode_struct_templates_via_kel,
};
use keleusma::wire_schema::{
    self as schema, EnumLayoutRecord, EnumVariantRecord, NameRef, StructTemplateRecord, kind,
};
use keleusma_wire::{WireBuilder, WireRecord};

fn encode<T: WireRecord>(rows: &[T]) -> Vec<u8> {
    let mut out = vec![0; rows.len() * T::STRIDE];
    for (r, b) in rows.iter().zip(out.chunks_exact_mut(T::STRIDE)) {
        r.write_record(b).unwrap();
    }
    out
}
fn raw(
    templates: Option<&[u8]>,
    variants: Option<&[u8]>,
    layouts: Option<&[u8]>,
    bad_span: bool,
) -> Vec<u8> {
    let names = [
        NameRef {
            offset: if bad_span { u32::MAX } else { 0 },
            length: 1,
        },
        NameRef {
            offset: 1,
            length: 5,
        },
        NameRef {
            offset: 6,
            length: 1,
        },
        NameRef {
            offset: 7,
            length: 1,
        },
        NameRef {
            offset: 8,
            length: 1,
        },
    ];
    let mut b = WireBuilder::new();
    let r = b.region(kind::NAMES, 0).unwrap();
    for n in names {
        b.push_record(r, &n);
    }
    let r = b.region(kind::STRING_POOL, 0).unwrap();
    b.push(r, b"SfieldEV\xff");
    for (kind, data) in [
        (kind::STRUCT_TEMPLATES, templates),
        (kind::ENUM_VARIANTS, variants),
        (kind::ENUM_LAYOUTS, layouts),
    ] {
        if let Some(data) = data {
            let r = b.region(kind, 0).unwrap();
            b.push(r, data);
        }
    }
    b.finish().unwrap()
}
fn compare(bytes: &[u8]) {
    let templates = schema::decode_struct_templates(bytes).ok();
    let enums = schema::decode_enum_layouts(bytes).ok();
    assert_eq!(wire_decode_struct_templates_via_kel(bytes).ok(), templates);
    assert_eq!(wire_decode_enum_layouts_via_kel(bytes).ok(), enums);
    let both = wire_decode_layout_metadata_via_kel(bytes).unwrap();
    assert_eq!(both.templates, templates);
    assert_eq!(both.enum_layouts, enums);
}

#[test]
fn templates_and_enum_values_round_trip_with_names_and_discriminants() {
    let templates = vec![
        StructTemplate {
            type_name: "Tλ".into(),
            field_names: vec!["x".into(), "x".into()],
        },
        StructTemplate {
            type_name: "empty".into(),
            field_names: vec![],
        },
    ];
    let enums = vec![EnumLayout {
        type_name: "E".into(),
        variants: vec![
            EnumVariantDisc {
                name: "a".into(),
                disc: i64::MIN,
            },
            EnumVariantDisc {
                name: "b".into(),
                disc: i64::MAX,
            },
        ],
        min_payload: u32::MAX,
    }];
    let bytes = schema::encode_layouts(&templates, &enums).unwrap();
    assert_eq!(
        wire_decode_struct_templates_via_kel(&bytes).unwrap(),
        templates
    );
    assert_eq!(wire_decode_enum_layouts_via_kel(&bytes).unwrap(), enums);
    compare(&bytes);
}

#[test]
fn optional_families_fail_independently_after_shared_range_validation() {
    let good_template = StructTemplateRecord {
        type_name: 0,
        field_names_first: 1,
        field_count: 1,
        reserved: u32::MAX,
    };
    let good_variant = EnumVariantRecord {
        name: 3,
        reserved: u32::MAX,
        disc: i64::MIN,
    };
    let good_layout = EnumLayoutRecord {
        type_name: 2,
        variants_first: 0,
        variants_count: 1,
        min_payload: 15,
    };
    for (t, v, e, bad_span, expected) in [
        (
            good_template,
            good_variant,
            good_layout,
            false,
            (true, true),
        ),
        (
            StructTemplateRecord {
                type_name: 4,
                ..good_template
            },
            good_variant,
            good_layout,
            false,
            (false, true),
        ),
        (
            good_template,
            EnumVariantRecord {
                name: 4,
                ..good_variant
            },
            good_layout,
            false,
            (true, false),
        ),
        (
            good_template,
            good_variant,
            good_layout,
            true,
            (false, true),
        ),
        (
            StructTemplateRecord {
                field_names_first: 6,
                field_count: 0,
                ..good_template
            },
            good_variant,
            good_layout,
            false,
            (false, false),
        ),
        (
            good_template,
            good_variant,
            EnumLayoutRecord {
                variants_count: 2,
                ..good_layout
            },
            false,
            (false, false),
        ),
    ] {
        let bytes = raw(
            Some(&encode(&[t])),
            Some(&encode(&[v])),
            Some(&encode(&[e])),
            bad_span,
        );
        compare(&bytes);
        let r = wire_decode_layout_metadata_via_kel(&bytes).unwrap();
        assert_eq!((r.templates.is_some(), r.enum_layouts.is_some()), expected);
    }
}

#[test]
fn absent_empty_and_malformed_optional_tables_match_reference() {
    let variants = encode(&[EnumVariantRecord {
        name: u32::MAX,
        reserved: 0,
        disc: 0,
    }]);
    for templates in [None, Some(&[][..]), Some(&[0; 8][..])] {
        for layouts in [None, Some(&[][..]), Some(&[0; 8][..])] {
            compare(&raw(templates, Some(&variants), layouts, false));
        }
    }
    compare(&raw(None, Some(&[0; 8]), None, false));
    let empty = WireBuilder::new().finish().unwrap();
    let r = wire_decode_layout_metadata_via_kel(&empty).unwrap();
    assert!(r.templates.is_none() && r.enum_layouts.is_none());
}
