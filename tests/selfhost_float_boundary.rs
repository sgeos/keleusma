//! Where the self-hosted subset actually stops on floats.
//!
//! **"No floats" is wrong, and so was the first correction of it.** The
//! command-line help said the subset excludes floats. That was replaced with
//! "only the float LITERAL is excluded; a float-typed signature compiles",
//! which is also wrong: float addition on float-typed parameters, with no
//! literal anywhere, diverges. This file measures the boundary rather than
//! describing it, so the next person to write a sentence about it has a table
//! to write from.
//!
//! # What the divergence is
//!
//! For FLOAT operands the self-hosted codegen emits the CHECKED arithmetic
//! opcode where the reference emits the plain one. On `Word` operands the two
//! agree, so this is specific to the float path rather than to arithmetic.
//!
//! # The cause, which retracts a stronger claim made first
//!
//! This file first argued that the self-hosted side "appears to be the wrong
//! one", on the grounds that `tests/float_arith_width.rs` records plain `+` on
//! floats emitting the unchecked `Op::Add` and "never the checked path", and
//! that float overflow is not a trap condition. **That framing was too strong,
//! and reading `codegen.kel` retracts it.**
//!
//! Its own comment states the rule: the operator code ALONE selects the op
//! word — `Add` to `CheckedAdd`, `Sub` to `CheckedSub`, `Mul` to `CheckedMul`,
//! while `Div` maps to plain `Div` and `Mod` to plain `Mod`. **No operand type
//! enters the decision**, and the file contains no float or type vocabulary at
//! all.
//!
//! That accounts for every row measured below. Division and comparison agree
//! because they have no checked variant in the mapping. `Word` agrees because
//! the reference emits the checked form there too. Float `+`, `-` and `*`
//! diverge because the reference has operand types and chooses the plain form,
//! and the self-hosted codegen has no types to choose with.
//!
//! So this is a genuine capability gap and the table's `scope/` filing is
//! CORRECT. It is not a mislabelled defect, and the fix is not a branch
//! correction: it requires a type channel into codegen, which is a substantial
//! change to a stage source.
//!
//! # How this differs from the other two diverging entries
//!
//! The construct-support boundary table carries exactly three `Diverges` rows.
//! The other two are struct-equality cases, and measuring them separates the
//! kinds of divergence rather than lumping them together:
//!
//! - `eq/struct_tuple_of_impure_struct__GAP` and
//!   `eq/struct_field_array_of_tuple__GAP` both report `CmpEq` against the
//!   reference's `SetLocal`. That is a STRUCTURAL difference — a direct compare
//!   where the reference stages through a local — and the table's own comment
//!   records that the flat array-equality family "has no nested form" and that
//!   these cases were "previously admitted and silently mis-compiled". A missing
//!   codegen form, correctly filed as a gap, with the cross-check now catching
//!   what used to pass silently.
//! - The float case has no missing form. The self-hosted side emits the same
//!   operation, in its CHECKED variant, where the reference emits the plain one.
//!
//! **That contrast is why the float entry stands out.** Two of the three
//! divergences name something the self-hosted codegen does not implement; the
//! third names a variant it chose.
//!
//! # Why this is characterised and not fixed here
//!
//! The fix lives in a `.kel` stage source. Stage sources bear on the pending
//! capacity question for the input path, so changing one is not this test's
//! call. The measurement is the deliverable.

#![cfg(feature = "self-host")]

use keleusma::target::Target;

/// Compile `src` through the self-hosted backend; return `Err`'s text.
fn attempt(src: &str) -> Result<(), String> {
    keleusma::selfhost::self_hosted_compile(src, &Target::host())
        .map(|_| ())
        .map_err(|e| format!("{e}"))
}

fn binop(op: &str, ty: &str, ret: &str) -> String {
    format!(
        "fn f(a: {ty}, b: {ty}) -> {ret} {{ a {op} b }}\nfn main(x: {ty}) -> {ret} {{ f(x, x) }}"
    )
}

/// **THE FLOAT OPERATOR BOUNDARY, MEASURED PER OPERATOR.**
///
/// Three arithmetic operators diverge and division does not. Asserting the
/// accepted half matters as much as the refused half: a change that pushed all
/// float use out of the subset would leave a refusal-only test passing while
/// every sentence written from it became wrong.
#[test]
fn float_add_sub_mul_diverge_and_divide_does_not() {
    // Diverging, with the checked-versus-plain opcode named in the message.
    for (op, checked) in [
        ("+", "CheckedAdd"),
        ("-", "CheckedSub"),
        ("*", "CheckedMul"),
    ] {
        let err = attempt(&binop(op, "Float", "Float")).expect_err(&format!(
            "float `{op}` now compiles through the self-hosted backend. If the codegen was \
             brought into line with the reference, this is a FIX and the operator belongs \
             in the accepted list below"
        ));
        assert!(
            err.contains("diverges"),
            "float `{op}` no longer fails as a divergence, so it is being refused somewhere \
             earlier and this test no longer measures the codegen. Message was: {err}"
        );
        assert!(
            err.contains(checked),
            "float `{op}` diverges without naming `{checked}`. The characterisation in this \
             file rests on the self-hosted side emitting the CHECKED opcode; if the opcode \
             changed, the file's account of the cause is stale. Message was: {err}"
        );
    }

    // Accepted: division, comparison, and a float-typed signature doing neither.
    assert!(
        attempt(&binop("/", "Float", "Float")).is_ok(),
        "float division now fails too, so the divergence is broader than the three \
         operators this file names"
    );
    assert!(
        attempt(&binop("<", "Float", "bool")).is_ok(),
        "float comparison now fails, so the divergence is not confined to arithmetic"
    );
    assert!(
        attempt("fn f(a: Float) -> Float { a }\nfn main(x: Float) -> Float { f(x) }").is_ok(),
        "a float-typed signature that performs no arithmetic no longer compiles, so floats \
         are now wholly outside the subset and every sentence describing the boundary as \
         operator-specific needs rewriting"
    );
}

/// **THE SAME OPERATORS ON `Word` AGREE**, which is what makes the finding
/// specific to the float path rather than to checked arithmetic in general.
///
/// Without this control the divergence could be read as "the self-hosted
/// codegen picks checked opcodes", which would be a much larger claim.
#[test]
fn the_same_operators_on_word_do_not_diverge() {
    for op in ["+", "-", "*"] {
        assert!(
            attempt(&binop(op, "Word", "Word")).is_ok(),
            "`{op}` on Word now diverges too. The float finding is stated as specific to \
             float operands; if Word diverges as well, the cause is broader than this file \
             claims and the account should be rewritten rather than this assertion relaxed"
        );
    }
}

/// A float LITERAL fails earlier than the cross-check, and is named.
///
/// This is the half the construct scan covers. It is here so the three float
/// outcomes — named refusal, divergence, and acceptance — sit in one file.
#[test]
fn a_float_literal_is_refused_before_the_cross_check_and_is_named() {
    let err = attempt("fn main() -> Float { 1.5 + 2.5 }")
        .expect_err("a float literal now compiles through the self-hosted backend");
    assert!(
        err.contains("a floating-point literal"),
        "the refusal no longer names the literal. Message was: {err}"
    );
    assert!(
        !err.contains("diverges"),
        "a float literal now reaches the cross-check. It used to fail earlier, in a stage, \
         which is why the construct scan names it rather than the divergence message. \
         Message was: {err}"
    );
}
