//! **WHICH COMPUTED VALUES CAN FILL A COMPOSITE FIELD, OR AN ARRAY ELEMENT?**
//!
//! # The finding
//!
//! `NewComposite` refuses an operand of unknown packed width — correctly, since
//! storing one would mispack the body. **Several arms pushed `Width::Unknown` for
//! a result whose shape was perfectly well determined**: a word divided by a word
//! is a word, a byte xored with a byte is a byte.
//!
//! Measured 2026-09-17, before the repair, over eleven `Word` producers:
//!
//! | could fill a field | could not |
//! |---|---|
//! | `param`, `add`, `sub`, `mul`, `neg`, literal, compare, call | `div`, `mod`, `band`, `bor`, `bxor`, and **all four shifts** |
//!
//! **A refusal is not a wrong answer**, and this one erred safely for as long as
//! it existed. What it cost was capability, silently: no composite field could
//! hold the result of a division, a modulo, or any bitwise operation.
//!
//! # How it was found, twice
//!
//! The `Byte` half came from a generated expression tree on its fourth program;
//! the `Word` half from a generated composite on its second. **Neither could have
//! come from an enumerated cell**: a width lost on the way OUT of an operation is
//! invisible until something consumes the result, and the 90-cell and 42-cell
//! matrices each apply one operator and consume nothing.
//!
//! # The population is every producer, not the five that were broken
//!
//! A census keyed to what the analyst already noticed finds nothing they had not
//! already noticed — this package has recorded that twice. Every producer is
//! checked in a field position, so a regression in any of them fails here.

mod common;

/// `(label, expression)` for values of each type. The expression must be
/// type-correct in a field of the stated type.
const WORD_PRODUCERS: &[(&str, &str)] = &[
    ("param", "b"),
    ("literal", "7"),
    ("add", "(a + b)"),
    ("sub", "(a - b)"),
    ("mul", "(a * b)"),
    ("neg", "(-a)"),
    ("div", "(a / b)"),
    ("mod", "(a % b)"),
    ("band", "(a band b)"),
    ("bor", "(a bor b)"),
    ("bxor", "(a bxor b)"),
    ("lsl", "(a lsl 2)"),
    ("lsr", "(a lsr 2)"),
    ("asl", "(a asl 2)"),
    ("asr", "(a asr 2)"),
    ("compare", "(if a > b { 1 } else { 0 })"),
    ("call", "g(a)"),
];

const BYTE_PRODUCERS: &[(&str, &str)] = &[
    ("cast", "(a as Byte)"),
    ("add", "((a as Byte) + (b as Byte))"),
    ("sub", "((a as Byte) - (b as Byte))"),
    ("mul", "((a as Byte) * (b as Byte))"),
    ("div", "((a as Byte) / (b as Byte))"),
    ("mod", "((a as Byte) % (b as Byte))"),
    ("band", "((a as Byte) band (b as Byte))"),
    ("bor", "((a as Byte) bor (b as Byte))"),
    ("bxor", "((a as Byte) bxor (b as Byte))"),
    ("lsl", "((a as Byte) lsl 2)"),
    ("lsr", "((a as Byte) lsr 2)"),
];

/// Bool-valued producers. **Added after `Op::Not` was found refused** while a
/// plain comparison was admitted: the comparison arm pushed `Scalar(1)` and the
/// negation arm pushed nothing, for results of exactly the same shape.
const BOOL_PRODUCERS: &[(&str, &str)] = &[
    ("compare", "(a > b)"),
    ("not", "not (a > b)"),
    ("andalso", "((a > 0) andalso (b > 0))"),
    ("orelse", "((a > 0) orelse (b > 0))"),
    ("and", "((a > 0) and (b > 0))"),
    ("or", "((a > 0) or (b > 0))"),
    ("xor", "((a > 0) xor (b > 0))"),
    ("literal", "true"),
];

/// A two-field struct whose FIRST field holds the produced value. **The second
/// field is the detector**: a wrong width shifts it, and reading both with
/// distinct multipliers makes the shift observable.
fn field_source(ty: &str, value: &str, read: &str) -> String {
    format!(
        "fn g(z: Word) -> Word {{ z + 1 }}\n\
         struct P {{ x: {ty}, y: Word }}\n\
         fn main(a: Word, b: Word) -> Word {{ \
           let p: P = P {{ x: {value}, y: b }}; \
           ({read} * 1000) + p.y }}"
    )
}

fn refusal(src: &str) -> Option<String> {
    keleusma_native::module_refusals(
        &common::build(src),
        keleusma_native::LowerOptions::default(),
    )
    .first()
    .map(|(_, e)| format!("{e:?}"))
}

#[test]
fn every_word_producer_can_fill_a_composite_field() {
    let mut broken = Vec::new();
    for (name, v) in WORD_PRODUCERS {
        if let Some(e) = refusal(&field_source("Word", v, "(p.x % 9)")) {
            broken.push((*name, e));
        }
    }
    assert!(
        broken.is_empty(),
        "word producer(s) whose result cannot fill a composite field: {broken:?}.\n\n\
         `NewComposite` refuses an unknown packed width, and these results have a \
         perfectly determined shape. If a narrowing is deliberate, say which \
         operand combination it covers and why refusing beats propagating."
    );
}

#[test]
fn every_byte_producer_can_fill_a_composite_field() {
    let mut broken = Vec::new();
    for (name, v) in BYTE_PRODUCERS {
        if let Some(e) = refusal(&field_source("Byte", v, "(p.x as Word)")) {
            broken.push((*name, e));
        }
    }
    assert!(
        broken.is_empty(),
        "byte producer(s) whose result cannot fill a composite field: {broken:?}"
    );
}

/// **The values must be right, not merely admitted.** Admitting a wrong width
/// would mispack rather than refuse, which is the worse failure.
#[test]
fn the_admitted_fields_pack_where_the_reference_packs_them() {
    for (name, v) in WORD_PRODUCERS {
        let src = field_source("Word", v, "(p.x % 9)");
        let (vm, native) = common::vm_and_native_two_arg(&src, 9, 5);
        assert_eq!(
            vm, native,
            "word producer `{name}` MISPACKS: reference {vm}, native {native}\n  {src}"
        );
    }
    for (name, v) in BYTE_PRODUCERS {
        let src = field_source("Byte", v, "(p.x as Word)");
        let (vm, native) = common::vm_and_native_two_arg(&src, 9, 5);
        assert_eq!(
            vm, native,
            "byte producer `{name}` MISPACKS: reference {vm}, native {native}\n  {src}"
        );
    }
}

#[test]
fn every_bool_producer_can_fill_a_composite_field() {
    let mut broken = Vec::new();
    for (name, v) in BOOL_PRODUCERS {
        if let Some(e) = refusal(&field_source("bool", v, "(if p.x { 5 } else { 2 })")) {
            broken.push((*name, e));
        }
    }
    assert!(
        broken.is_empty(),
        "bool producer(s) whose result cannot fill a composite field: {broken:?}.\n\n\
         A bool is one byte, exactly as a comparison's result is. `Op::Not` was \
         refused here while a plain comparison was admitted, for values of the \
         same shape."
    );
}

#[test]
fn the_admitted_bool_fields_pack_where_the_reference_packs_them() {
    for (name, v) in BOOL_PRODUCERS {
        let src = field_source("bool", v, "(if p.x { 5 } else { 2 })");
        let (vm, native) = common::vm_and_native_two_arg(&src, 9, 5);
        assert_eq!(
            vm, native,
            "bool producer `{name}` MISPACKS: reference {vm}, native {native}\n  {src}"
        );
    }
}

// ---------------------------------------------------------------------------
// THE SAME RULE GOVERNS ARRAY ELEMENTS, AND NOTHING PINNED THAT
// ---------------------------------------------------------------------------
//
// The repairs were framed as being about composite FIELDS, because that is where
// the refusals surfaced. **Measured 2026-09-17, they govern array ELEMENTS
// identically**: reverting `preserved_scalar_width` refuses `div`, `mod` and
// `band` in an array literal exactly as it does in a field.
//
// **So the repair was broader than the framing, and the array position was
// unpinned.** A capability restored by accident and guarded nowhere is one
// refactor away from being lost again without anything going red.

/// An array literal whose FIRST element holds the produced value, with the
/// second element read too so a shifted element cannot hide.
fn element_source(ty: &str, value: &str, read: &str) -> String {
    format!(
        "fn g(z: Word) -> Word {{ z + 1 }}\n\
         fn main(a: Word, b: Word) -> Word {{ \
           let xs: [{ty}; 2] = [{value}, {value}]; \
           ({read} * 1000) + b }}"
    )
}

#[test]
fn every_producer_can_fill_an_array_element() {
    let mut broken = Vec::new();
    for (name, v) in WORD_PRODUCERS {
        if let Some(e) = refusal(&element_source("Word", v, "(xs[0] % 9)")) {
            broken.push((format!("word/{name}"), e));
        }
    }
    for (name, v) in BYTE_PRODUCERS {
        if let Some(e) = refusal(&element_source("Byte", v, "(xs[0] as Word)")) {
            broken.push((format!("byte/{name}"), e));
        }
    }
    for (name, v) in BOOL_PRODUCERS {
        if let Some(e) = refusal(&element_source("bool", v, "(if xs[0] { 5 } else { 2 })")) {
            broken.push((format!("bool/{name}"), e));
        }
    }
    assert!(
        broken.is_empty(),
        "producer(s) whose result cannot fill an ARRAY element: {broken:?}.\n\n\
         Array elements consume a packed width exactly as composite fields do. \
         This position was unpinned until 2026-09-17, so a repair that restored it \
         could have been lost again silently."
    );
}

#[test]
fn the_admitted_array_elements_agree_with_the_reference() {
    for (name, v) in WORD_PRODUCERS {
        let src = element_source("Word", v, "(xs[0] % 9)");
        let (vm, native) = common::vm_and_native_two_arg(&src, 9, 5);
        assert_eq!(vm, native, "word element `{name}` diverges\n  {src}");
    }
    for (name, v) in BOOL_PRODUCERS {
        let src = element_source("bool", v, "(if xs[0] { 5 } else { 2 })");
        let (vm, native) = common::vm_and_native_two_arg(&src, 9, 5);
        assert_eq!(vm, native, "bool element `{name}` diverges\n  {src}");
    }
}
