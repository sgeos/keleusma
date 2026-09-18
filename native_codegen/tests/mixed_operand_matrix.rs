//! **THE MIXED-TYPE HALF OF THE OPERATOR MATRIX. ALL 84 CELLS ARE REFUSED.**
//!
//! # Why drive a matrix that is entirely negative
//!
//! `scalar_operator_matrix.rs` drives SAME-type pairs and found `Fixed % Fixed`.
//! Mixed pairs were named as an open gap in two successive hand-offs and never
//! measured. They matter because a promote-or-convert path is where a width or a
//! scale goes missing, and because the operand-kind lattice is least exercised
//! there.
//!
//! The answer is uniform: **Keleusma has no implicit numeric conversion.** That
//! converts *"nobody tried mixed operands"* into *"the reference refuses all of
//! them, for this stated reason"*, which is worth an instrument even though no
//! defect came out of it.
//!
//! # ⚠ A REFUSAL IS WORTHLESS WITHOUT ITS CAUSE
//!
//! Two increments ago an unknown type name was filed as a fact about comparisons,
//! inside the table built to stop exactly that. So this file checks four things
//! rather than one:
//!
//! 1. **The cause** — the message names both operand types.
//! 2. **A control** — the same expression with a matched pair compiles, so the
//!    refusal is about the PAIR.
//! 3. **The alternative excluded** — `-> Byte` and `-> Word` give the same
//!    message at the same span, so the RETURN type is not the cause.
//! 4. **The workaround** — an explicit cast compiles, so the conversion exists
//!    and is merely not implicit.
//!
//! # The same nominal type at different parameters is a different type
//!
//! `Fixed<16>` and `Fixed<32>` are mutually unassignable. Bare `Fixed` is
//! **`Fixed<32>`** — the compiler emits `FixedMul(32)` for it.
//!
//! **That default was assumed wrong once already.** The previous increment's
//! prose described the `Fixed % Fixed` divergence as `4.0` for `200.0 % 7.0`,
//! reading the raw operands as Q16.16. The divergence, its mechanism and its fix
//! are unaffected — the integer remainder of the raw words is the same under
//! either reading, and the reference traps regardless of scale — but the decimal
//! figures were wrong, and they are corrected in `MIXED_OPERAND_BRIEF.md`.
//!
//! # What this says about the backend
//!
//! The backend **never sees a mixed scalar pair from source**, so the mixed-kind
//! arm of `arith_result_kind` is UNREACHABLE FROM THE SURFACE. It stays reachable
//! through hand-built bytecode and `Vm::new_unchecked`. Recorded as unreachable,
//! **not** as verified.

use keleusma::bytecode::Module;

mod common;

/// Compile and return the reference's own error message.
fn refusal_of(src: &str) -> Result<Module, String> {
    let toks = keleusma::lexer::tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let ast = keleusma::parser::parse(&toks).map_err(|e| format!("parse: {e:?}"))?;
    keleusma::compiler::compile(&ast).map_err(|e| e.message)
}

/// **ALL FOUR SCALAR TYPES.** `Float` was absent until 2026-09-18 while this file
/// described itself as enumerating *"every ordered pair of DISTINCT scalar
/// types"* — the same gap, in the same shape, as the one that let float negation
/// ship broken past `scalar_operator_matrix.rs`. Found by asking which OTHER
/// instrument here enumerates scalar types, rather than by a new defect.
const TYPES: &[(&str, &str)] = &[
    ("byte", "Byte"),
    ("word", "Word"),
    ("fixed", "Fixed"),
    ("float", "Float"),
];
const OPS: &[&str] = &["+", "-", "*", "/", "%"];
const CMPS: &[&str] = &["<", "=="];

/// Fewer cells than this means the enumeration broke.
///
/// **42 until `Float` joined the type list.** Four types give twelve ordered
/// distinct pairs against seven operators.
const CELL_FLOOR: usize = 84;

/// Every ordered pair of DISTINCT scalar types against the operator surface.
fn cells() -> Vec<(String, Result<Module, String>)> {
    let mut out = Vec::new();
    for (ln, lt) in TYPES {
        for (rn, rt) in TYPES {
            if ln == rn {
                continue;
            }
            for op in OPS {
                out.push((
                    format!("{ln} {op} {rn}"),
                    refusal_of(&format!("fn main(a: {lt}, b: {rt}) -> {lt} {{ a {op} b }}")),
                ));
            }
            for op in CMPS {
                out.push((
                    format!("{ln} {op} {rn}"),
                    refusal_of(&format!("fn main(a: {lt}, b: {rt}) -> bool {{ a {op} b }}")),
                ));
            }
        }
    }
    out
}

#[test]
fn every_mixed_pair_is_refused_and_the_message_names_both_types() {
    let observed = cells();
    assert!(
        observed.len() >= CELL_FLOOR,
        "the enumeration produced {} cells, below the floor of {CELL_FLOOR}; a \
         short enumeration passes while testing nothing",
        observed.len()
    );

    let mut accepted = Vec::new();
    let mut unexplained = Vec::new();
    for (label, outcome) in &observed {
        match outcome {
            Ok(_) => accepted.push(label.clone()),
            Err(msg) => {
                // **The message must be about the TYPES.** A refusal for an
                // unrelated reason would sit here looking like support for the
                // claim while supporting nothing — which is exactly how an
                // unknown type name was once filed as a fact about comparisons.
                let names_a_type = ["Byte", "Word", "Fixed", "Float"]
                    .iter()
                    .filter(|t| msg.contains(*t))
                    .count();
                if !msg.contains("type error") || names_a_type < 2 {
                    unexplained.push(format!("{label}: {msg}"));
                }
            }
        }
    }

    assert!(
        accepted.is_empty(),
        "{} mixed pair(s) now COMPILE: {accepted:?}. The reference has gained an \
         implicit conversion, and the backend's mixed-kind arithmetic — recorded \
         as unreachable from source — is now reachable and untested.",
        accepted.len()
    );
    assert!(
        unexplained.is_empty(),
        "{} mixed pair(s) are refused for a reason that does not name two operand \
         types: {unexplained:?}. A refusal for an unrelated reason supports no \
         claim about mixed operands.",
        unexplained.len()
    );
}

/// **The three checks that stop the matrix above from meaning nothing.**
#[test]
fn the_refusal_is_about_the_pair_and_the_conversion_exists() {
    // 1. A CONTROL: the matched pair compiles, so it is the mixing that fails.
    assert!(
        refusal_of("fn main(a: Word, b: Word) -> Word { a + b }").is_ok(),
        "the matched-pair control no longer compiles, so the matrix above says \
         nothing about mixing"
    );

    // 2. THE ALTERNATIVE EXCLUDED: the return type is not the cause. Both
    //    spellings fail with the SAME message.
    let as_byte = refusal_of("fn main(a: Byte, b: Word) -> Byte { a + b }").unwrap_err();
    let as_word = refusal_of("fn main(a: Byte, b: Word) -> Word { a + b }").unwrap_err();
    assert_eq!(
        as_byte, as_word,
        "the two return-type spellings now fail differently, so the refusal may be \
         about the RETURN type rather than the operand pair"
    );
    assert!(
        as_byte.contains("cannot add Byte and Word"),
        "the mixed-add refusal no longer names both operand types: {as_byte}"
    );

    // 3. THE WORKAROUND: the conversion exists and is merely not implicit.
    assert!(
        refusal_of("fn main(a: Byte, b: Word) -> Word { (a as Word) + b }").is_ok(),
        "an explicit widening cast no longer compiles. `refused` would then mean \
         IMPOSSIBLE rather than NOT IMPLICIT, which is a different claim"
    );
}

/// The same nominal type at different const parameters is a different type.
#[test]
fn fixed_point_widths_do_not_mix_and_the_bare_spelling_is_q32() {
    let msg = refusal_of("fn main(a: Fixed<16>, b: Fixed<32>) -> Fixed<16> { a + b }").unwrap_err();
    assert!(
        msg.contains("Fixed<16>") && msg.contains("Fixed<32>"),
        "mixed fixed-point widths are no longer refused by width: {msg}"
    );

    // **The default the previous increment's prose got wrong.** Bare `Fixed`
    // bakes 32 fraction bits, which is why its `FixedMul` operand reads 32.
    let m = common::build("fn main(a: Fixed, b: Fixed) -> Fixed { a * b }");
    let ops = format!("{:?}", m.chunks[m.entry_point.expect("entry")].ops);
    assert!(
        ops.contains("FixedMul(32)"),
        "bare `Fixed` no longer bakes 32 fraction bits: {ops}. Every prose figure \
         describing a bare-`Fixed` measurement is scaled by this number."
    );
}

/// **THE FLOAT PAIRS ARE REALLY DRIVEN, AND REFUSED FOR THE STATED CAUSE.**
///
/// The matrix above passes on an aggregate: every cell refused, every message
/// naming two types. **An aggregate cannot say which cells exist.** When `Float`
/// joined `TYPES` the count moved from 42 to 84 and the whole file stayed green,
/// which is exactly what it would have done had the new cells been refused for
/// some unrelated reason — a lexer error, say, or an unknown type name. That
/// mistake has been made in this package before, and it was made in this very
/// file's subject area.
///
/// So the float half is asserted by name, in both operand positions, with the
/// message quoted rather than counted.
#[test]
fn the_float_pairs_are_present_and_refused_by_type() {
    for (src, needle) in [
        (
            "fn main(a: Word, b: Float) -> Word { a + b }",
            "cannot add Word and Float",
        ),
        (
            "fn main(a: Float, b: Word) -> Float { a + b }",
            "cannot add Float and Word",
        ),
        (
            "fn main(a: Byte, b: Float) -> bool { a < b }",
            "cannot order Byte and Float",
        ),
    ] {
        let msg = refusal_of(src).expect_err(
            "a mixed pair involving Float is ACCEPTED. Keleusma gained an implicit \
             numeric conversion, which is a language change this matrix exists to \
             notice.",
        );
        assert!(
            msg.contains(needle),
            "`{src}` is refused, but not for the recorded cause. Expected a \
             message containing `{needle}`, got: {msg}. A refusal for a different \
             reason would sit in the matrix above looking like support for the \
             no-implicit-conversion claim while supporting nothing."
        );
    }

    // **AND THE ENUMERATION REALLY CARRIES THEM.** The aggregate test checks a
    // floor; this checks that the float labels are among the cells counted.
    let labels: Vec<String> = cells().into_iter().map(|(l, _)| l).collect();
    for want in ["float + word", "word + float", "float < fixed"] {
        assert!(
            labels.iter().any(|l| l == want),
            "the enumeration produced no cell `{want}`, so the floor of \
             {CELL_FLOOR} is being met by something other than the float pairs"
        );
    }
}
