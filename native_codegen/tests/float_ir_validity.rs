//! **DOES EVERY FLOAT OPERATOR PRODUCE VALID IR, IN BOTH FLOAT CONFIGURATIONS?**
//!
//! # The defect that caused this file
//!
//! `Op::Neg` on a `Float` open-coded its conversion back to bits as
//! `build_bit_cast(n, i64t)`. That is correct for an eight-byte float and
//! **invalid IR for a four-byte one**, because a bitcast may not change the bit
//! width. Under `narrow-float-32`, `-(x as Float)` produced a module that failed
//! `lm.verify()`: **float negation was broken outright in a supported
//! configuration.**
//!
//! Every other float path already routed through the `float_to_bits` helper,
//! which narrows to `i32` and extends. **The negation arm duplicated the
//! conversion instead of calling it, and so it was the one that drifted.**
//!
//! # Why nothing caught it
//!
//! `scalar_operator_matrix.rs` enumerates *"every cell, from the types and the
//! operators"* — and its type list is `byte`, `word`, `fixed`. **`Float` is not in
//! it.** Its `raw` helper panics on a float with a message calling it a
//! non-scalar, and its header discusses float dispatch. So the omission looks
//! deliberate and was nowhere stated.
//!
//! **A scalar type absent from the scalar-operator matrix is exactly the shape
//! this package keeps finding**: an instrument whose population is narrower than
//! its description.
//!
//! # What this checks, and why it is cheap
//!
//! Lowering plus `verify()`, for every float operator. **No execution**, so it
//! costs milliseconds and needs none of the bit-pattern argument marshalling that
//! has produced probe defects here before. Invalid IR is a property of the
//! lowering alone, and that is what broke.
//!
//! **The gate runs both float configurations**, so this file is checked at four
//! bytes and at eight without doing anything configuration-specific itself.

use inkwell::context::Context;

mod common;

/// Every float-producing construct the surface admits.
const FLOAT_OPS: &[(&str, &str)] = &[
    ("cast", "(a as Float)"),
    ("neg", "(-(a as Float))"),
    ("add", "((a as Float) + (b as Float))"),
    ("sub", "((a as Float) - (b as Float))"),
    ("mul", "((a as Float) * (b as Float))"),
    ("div", "((a as Float) / (b as Float))"),
    ("mod", "((a as Float) % (b as Float))"),
    ("neg_of_sum", "(-((a as Float) + (b as Float)))"),
    ("neg_twice", "(-(-(a as Float)))"),
    // ⚠ **A to_word ROW WAS REMOVED HERE, AND IT WAS MY DEFECT, NOT THE
    // BACKEND'S.** It read `((a as Float) as Word)`, and the wrapper below
    // already appends `as Word` — so the subject cast a `Word` to a `Word` and
    // the backend correctly refused *"operand kind is Int, not Float"*.
    //
    // **The seventh probe this session to implicate itself** rather than the
    // lowering. The others: a float argument passed as `i64::MIN`, a float return
    // read as an integer, a hand-named signature taking a SIGBUS, a degenerate
    // stream handed to the general driver, `let mut` in a language with no
    // mutable local, and a census blind to its own subject. The conversion is
    // covered by every row here, since each is wrapped in `as Word`.
    //
    // The name is written WITHOUT backticks because the row no longer exists and
    // the citation checker resolves backticked identifiers against real ones.
    // **This is the third correction note this session to cite the very thing it
    // retired** -- the guard caught all three.
];

/// Lower and verify. `None` means the module was refused, which is a different
/// outcome from invalid IR and is reported as such.
fn ir_state(src: &str) -> Result<(), String> {
    let m = common::build(src);
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        .map_err(|e| format!("refused: {e:?}"))?;
    lm.verify().map_err(|e| {
        format!(
            "INVALID IR: {}",
            e.to_string().lines().next().unwrap_or("").trim()
        )
    })
}

#[test]
fn every_float_operator_lowers_to_valid_ir() {
    let mut broken = Vec::new();
    for (name, expr) in FLOAT_OPS {
        let src = format!("fn main(a: Word, b: Word) -> Word {{ ({expr}) as Word }}");
        if let Err(e) = ir_state(&src) {
            broken.push((*name, e));
        }
    }
    assert!(
        broken.is_empty(),
        "float operator(s) that do not lower to valid IR: {broken:?}.\n\n\
         **`bitcast` may not change a bit width**, so a conversion written for an \
         eight-byte float is invalid for a four-byte one. Every float path should \
         go through the width-aware helper rather than open-coding the cast — the \
         one arm that open-coded it is what made this file necessary."
    );
}

/// **A float stored in a composite field, which is how the negation defect was
/// actually reached.** Kept because the path differs: the value is packed rather
/// than returned.
#[test]
fn every_float_operator_lowers_to_valid_ir_inside_a_composite() {
    let mut broken = Vec::new();
    for (name, expr) in FLOAT_OPS {
        let src = format!(
            "struct P {{ x: Float, y: Word }}\n\
             fn main(a: Word, b: Word) -> Word {{ \
               let p: P = P {{ x: {expr}, y: b }}; \
               ((p.x as Word) * 1000) + p.y }}"
        );
        if let Err(e) = ir_state(&src) {
            broken.push((*name, e));
        }
    }
    assert!(
        broken.is_empty(),
        "float operator(s) invalid inside a composite: {broken:?}"
    );
}

/// **NON-VACUOUS.** A float operator must actually reach the float path, or this
/// file verifies integer code and reports it as float coverage.
#[test]
fn the_subjects_really_do_emit_float_operations() {
    let m = common::build("fn main(a: Word, b: Word) -> Word { ((-(a as Float))) as Word }");
    let names: Vec<String> = m
        .chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .map(|o| {
            let n = format!("{o:?}");
            n.split(['(', ' ', '{']).next().unwrap().to_string()
        })
        .collect();
    assert!(
        names.iter().any(|n| n == "WordToFloat" || n == "Neg"),
        "the negation subject emits {names:?}, which contains no float conversion \
         or negation; this file would be verifying integer code"
    );
}
