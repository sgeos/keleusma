//! A private composite slot beyond the placement record's `u16` is refused.
//!
//! # The defect
//!
//! The data slot index space is 24 bits, and each private composite slot's pool
//! placement is recorded with a `u16` slot. The compiler skipped a slot above
//! `u16::MAX` with no diagnostic, so its writes had no pool offset. Measured on
//! 2026-10-30, a private `[P; 70000]` produced 65,536 records for 70,000
//! elements. The runtime effect was not measured. Widening the record is a wire
//! change, so the module is refused at compile time instead.
//!
//! # What is established
//!
//! The boundary. With no shared data, private composite slots 0 to 65,535 are
//! recorded, so `[P; 65536]` compiles with one record per element, and
//! `[P; 65537]` is refused naming the field.
#![cfg(feature = "compile")]

fn compile(src: &str) -> Result<keleusma::bytecode::Module, String> {
    let tokens = keleusma::lexer::tokenize(src).map_err(|e| format!("{e:?}"))?;
    let program = keleusma::parser::parse(&tokens).map_err(|e| format!("{e:?}"))?;
    keleusma::compiler::compile(&program).map_err(|e| e.message)
}

fn source(n: u32) -> String {
    format!(
        "struct P {{ x: Word }}\nprivate data d {{ a: [P; {n}] }}\n\
         fn main() -> Word {{ d.a[0] = P {{ x: 1 }}; 0 }}"
    )
}

#[test]
fn the_last_recordable_composite_slot_compiles_with_a_record_per_element() {
    let m = compile(&source(65_536)).expect("[P; 65536] must compile");
    let records = m
        .data_layout
        .as_ref()
        .map(|l| l.private_composite_layout.len())
        .unwrap_or(0);
    assert_eq!(records, 65_536, "one placement record per element");
}

#[test]
fn a_composite_slot_beyond_the_record_width_is_refused() {
    let err = compile(&source(65_537)).expect_err("[P; 65537] compiled");
    assert!(
        err.contains("`d.a`") && err.contains("65536 composite slots"),
        "the refusal does not name the field and the limit: {err}"
    );
}
