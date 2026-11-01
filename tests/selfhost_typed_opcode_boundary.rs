//! What the self-hosted codegen's TYPELESS opcode mapping diverges on.
//!
//! **Updated in Order 6 slices 5 and 5c.** `Float` operands now carry scalar kind five from
//! the parser, and codegen selects the plain `Add`, `Sub`, `Mul` and `Neg` for them, so the
//! `Float` row agrees throughout. The matrix and the account below are the measurement from
//! before that change, kept for the `Fixed` and `Byte` rows, which are unchanged.
//!
//! `codegen.kel` selects the op word from the operator code ALONE. Its comment
//! above `push_binop` states the rule: `Add` to `CheckedAdd`, `Sub` to
//! `CheckedSub`, `Mul` to `CheckedMul`, while `Div` and `Mod` map to their plain
//! forms. **No operand type enters the decision**, and the file contains no
//! float, fixed-point or type vocabulary at all.
//!
//! The reference compiler does have operand types, so wherever it picks a
//! different op for a non-`Word` operand, the cross-check refuses. This file
//! measures where that happens, per operand type and per operator.
//!
//! # The measured matrix
//!
//! Censused against the operator list in `codegen.kel`'s own mapping comment.
//! **Every cell below was run.** An earlier version left cells blank on the
//! assumption that the language did not admit them; three of those were admitted,
//! and one of the three diverges. `n/a` now means a REFERENCE TYPE ERROR, checked,
//! not a cell nobody tried.
//!
//! | operand | `+` | `-` | `*` | `/` | `%` | unary `-` | compare | bitwise | shift |
//! |---|---|---|---|---|---|---|---|---|---|
//! | `Word`  | ok | ok | ok | ok | ok | ok | ok | ok | ok |
//! | `Byte`  | ok | ok | ok | ok | ok | **chk** | ok | ok | ok |
//! | `Float` | ok | ok | ok | ok | ok | ok | ok | n/a | n/a |
//! | `Fixed<16>` | **chk** | **chk** | **`FixedMul`** | **`FixedDiv`** | ok | **chk** | ok | n/a | n/a |
//!
//! `chk` is the checked-versus-plain difference; the named ops are the
//! scale-aware ones. Booleans (`and`, `or`, `xor`, `not`) agree, and so does
//! `bnot` on `Byte`.
//!
//! **Unary negation diverges for EVERY non-`Word` operand**, `Byte` included.
//! The rest of the divergence is `+`, `-`, `*` on `Float` and `Fixed<N>`, plus
//! fixed `*` and `/`. Everything else agrees.
//!
//! That set matches the tree's own residual-tag note, which lists `Op::Add`,
//! `Op::Sub`, `Op::Mul` and `Op::CheckedNeg` together: precisely the operations
//! for which the reference has a plain form a typeless codegen cannot select.
//!
//! # Two kinds of divergence, and the second one matters more
//!
//! - **Checked versus plain.** Float and fixed `+`/`-`, and float `*`. The
//!   operation is the same and only the overflow-checking variant differs.
//! - **Generic versus scale-aware.** Fixed `*` and `/`, where the reference
//!   emits `FixedMul(16)` and `FixedDiv(16)`. A fixed-point multiply needs the
//!   scale correction; the generic op computes a DIFFERENT VALUE. **A silent
//!   mis-compile here would be numerically wrong, not merely differently
//!   checked**, which is precisely what the cross-check exists to stop.
//!
//! # Why the earlier framings in this file were wrong
//!
//! Three claims were made and withdrawn before this one, each too confident at
//! the edge of its evidence:
//!
//! 1. "The subset excludes floats" — a float-typed signature compiles.
//! 2. "Only the float LITERAL is excluded" — float `+` on float parameters, with
//!    no literal anywhere, diverges.
//! 3. "Specific to the float path" — fixed-point diverges too, and more badly.
//! 4. "`Word` and `Byte` agree throughout" — `Byte` unary negation diverges.
//!
//! The third fell to a PREDICTION. Once the cause was known it implied something
//! not yet looked at, and `Byte` arithmetic was the cheap test; it AGREED,
//! refuting that prediction, while the same run showed `Fixed` diverging. So the
//! claim needed widening for a reason the prediction had not anticipated.
//!
//! **The fourth fell to a test already in the tree.** `tests/op_tag_tables.rs`
//! records that `codegen.kel` emits `checkedneg` for unary negation, that its
//! decoder has no arm producing `Op::Neg` at all, and that the reference emits
//! `Op::Neg` for **`Byte`** negation. The conclusion was sitting there; this
//! file's matrix had left the `Byte` unary cell blank on the assumption that the
//! language did not admit it. **A blank cell is a claim, and that one was never
//! run.**
//!
//! # Which side is wrong
//!
//! A divergence establishes that the two disagree and not which is at fault; on
//! 2026-08-31 the reference was the wrong side. Here the self-hosted side simply
//! cannot make the distinction, having no operand types, so `scope/float_arith__GAP`
//! is a genuine CAPABILITY gap and its filing is correct. This file earlier called
//! it a possible defect; that was withdrawn when the cause was located.
//!
//! # Why this is characterised and not fixed
//!
//! The fix is a type channel into `codegen.kel`. Stage sources bear on the
//! pending capacity question for the input path, so the measurement is the
//! deliverable and the change is not this test's call.

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

/// **FLOAT ARITHMETIC AGREES ON EVERY OPERATOR SINCE ORDER 6 SLICE 5.**
///
/// The parser now types two Float operands as scalar kind five and codegen selects the
/// plain `Add`, `Sub` and `Mul` the reference emits, so the three operators that diverged
/// before agree, beside division, remainder, comparison and a float-typed signature, which
/// always did. A return to the checked forms would fail here, by name.
#[test]
fn float_arithmetic_agrees_on_every_operator() {
    for op in ["+", "-", "*", "/", "%"] {
        let outcome = attempt(&binop(op, "Float", "Float"));
        assert!(
            outcome.is_ok(),
            "float `{op}` no longer compiles byte-identically, so the slice 5 lowering \
             regressed: {outcome:?}"
        );
    }
    assert!(
        attempt(&binop("<", "Float", "bool")).is_ok(),
        "float comparison now fails, so the divergence is not confined to arithmetic"
    );
    assert!(
        attempt("fn f(a: Float) -> Float { a }\nfn main(x: Float) -> Float { f(x) }").is_ok(),
        "a float-typed signature that performs no arithmetic no longer compiles"
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

/// A float LITERAL compiles byte-identically since Order 6 slice 5b, which lexes it and
/// interns it as a Float pool constant. It used to be refused in a stage and named by the
/// construct scan, whose float arm left with this change.
#[test]
fn a_float_literal_compiles_since_order6_slice5b() {
    let outcome = attempt("fn main() -> Float { 1.5 + 2.5 }");
    assert!(
        outcome.is_ok(),
        "a float literal no longer compiles byte-identically: {outcome:?}"
    );
}

/// **FIXED-POINT DIVERGES IN TWO DIFFERENT WAYS, AND THE SECOND IS THE SERIOUS ONE.**
///
/// `+` and `-` are the same checked-versus-plain difference floats show. `*` and `/`
/// are not: the reference emits the SCALE-AWARE `FixedMul`/`FixedDiv`, which the
/// typeless codegen cannot select. A fixed-point multiply without the scale
/// correction computes a different value, so this is a wrong-result hazard rather
/// than an overflow-checking difference, and it is exactly what the cross-check is
/// for.
#[test]
fn fixed_point_diverges_on_both_the_checked_and_the_scale_aware_ops() {
    const TY: &str = "Fixed<16>";

    // Same kind as the float case: only the checked variant differs.
    for (op, checked) in [("+", "CheckedAdd"), ("-", "CheckedSub")] {
        let err = attempt(&binop(op, TY, TY)).expect_err(&format!(
            "fixed-point `{op}` now compiles; if the codegen gained \
                 operand types this is a FIX and the row belongs in the accepted list"
        ));
        assert!(
            err.contains(checked),
            "fixed-point `{op}` no longer names `{checked}`, so the account of the cause in \
             this file's header is stale. Message was: {err}"
        );
    }

    // Different kind: the reference's op is scale-aware, not merely unchecked.
    for (op, reference_op) in [("*", "FixedMul"), ("/", "FixedDiv")] {
        let err = attempt(&binop(op, TY, TY)).expect_err(&format!(
            "fixed-point `{op}` now compiles through the self-hosted backend"
        ));
        assert!(
            err.contains(reference_op),
            "fixed-point `{op}` no longer diverges against `{reference_op}`. That op carries \
             the scale correction, and its absence is why this case is a wrong-VALUE hazard \
             rather than a checking difference; if the reference stopped emitting it, this \
             file's account needs rewriting. Message was: {err}"
        );
    }

    // ACCEPTED, so the refusals above are not "everything fixed-point fails".
    assert!(
        attempt(&binop("%", TY, TY)).is_ok(),
        "fixed-point `%` now fails too, so the divergence is broader than the four \
         operators named here"
    );
    assert!(
        attempt(&binop("<", TY, "bool")).is_ok(),
        "fixed-point comparison now fails, so the divergence is not confined to arithmetic"
    );
}

/// **`Byte` AGREES ON EVERY BINARY OPERATOR AND DIVERGES ONLY ON UNARY NEGATION.**
///
/// The split matters. `Byte` arithmetic refuted the prediction that any operand
/// type the reference treats unchecked would diverge — which is why the claim is
/// not simply "every non-`Word` type". But its unary negation DOES diverge, so the
/// claim is not "`Byte` agrees" either. That second half is asserted in
/// `unary_negation_diverges_on_fixed_and_byte_operands`; this test covers the
/// binary operators and `%` and comparison, all of which the first matrix left
/// blank without running them.
#[test]
fn byte_agrees_on_every_binary_operator() {
    for op in ["+", "-", "*", "/", "%"] {
        assert!(
            attempt(&binop(op, "Byte", "Byte")).is_ok(),
            "`{op}` on Byte now diverges. Byte is documented as agreeing on every BINARY \
             operator and diverging only on unary negation; if a binary operator has joined, \
             the account should be rewritten rather than this assertion relaxed"
        );
    }
    assert!(
        attempt(&binop("<", "Byte", "bool")).is_ok(),
        "Byte comparison now diverges, so the divergence is not confined to arithmetic"
    );
    assert!(
        attempt("fn f(a: Byte) -> Byte { bnot a }\nfn main(x: Byte) -> Byte { f(x) }").is_ok(),
        "Byte `bnot` now diverges, so unary negation is not the only unary operator affected"
    );
}

/// **UNARY NEGATION DIVERGES ON `Fixed` AND `Byte`, AND AGREES ON `Word` AND `Float`.**
///
/// `CheckedNeg` against the reference's plain `Neg`. `Float` left the diverging list in
/// Order 6 slice 5c, which emits `Neg` for a Float operand. `Byte` is in this list and
/// was missing from the first version, where the matrix left its unary cell blank
/// on the assumption that the language did not admit it. It does, and it diverges.
///
/// # The answer was already in the tree
///
/// `tests/op_tag_tables.rs` records that `codegen.kel` emits `checkedneg` for
/// unary negation, that its decoder has no arm producing `Op::Neg` at all, and
/// that the reference emits `Op::Neg` for **`Byte`** negation. Reading it is what
/// exposed the blank cell. **A census that skips a cell and a census that measures
/// it agree on every row it did run**, which is why the gap survived three
/// readings of the matrix.
#[test]
fn unary_negation_diverges_on_fixed_and_byte_operands() {
    for ty in ["Fixed<16>", "Byte"] {
        let src = format!("fn f(a: {ty}) -> {ty} {{ -a }}\nfn main(x: {ty}) -> {ty} {{ f(x) }}");
        let err = attempt(&src).expect_err(&format!(
            "unary negation on {ty} now compiles; if the codegen gained operand types this \
             is a FIX and the row belongs in the accepted set"
        ));
        assert!(
            err.contains("CheckedNeg"),
            "{ty} negation no longer names `CheckedNeg`, so the account of the cause in this \
             file's header is stale. Message was: {err}"
        );
    }

    // ACCEPTED, so the refusals above are not "negation always fails".
    assert!(
        attempt("fn f(a: Word) -> Word { -a }\nfn main(x: Word) -> Word { f(x) }").is_ok(),
        "negation on Word now diverges too, so the cause is broader than the header claims"
    );
    assert!(
        attempt("fn f(a: Float) -> Float { -a }\nfn main(x: Float) -> Float { f(x) }").is_ok(),
        "negation on Float no longer compiles byte-identically, so slice 5c regressed"
    );
}

/// **THE BITWISE, SHIFT AND BOOLEAN OPERATORS ALL AGREE**, which closes the
/// matrix rather than extending the finding.
///
/// `Byte` reaches the bitwise and shift operators through promote-operate-truncate,
/// a different path from its arithmetic, and was the most plausible place for the
/// divergence to reappear. It does not. **A census that closes cleanly is worth
/// the same as one that finds something**, because the alternative is a matrix
/// with cells filled in by inference.
#[test]
fn the_bitwise_shift_and_boolean_operators_do_not_diverge() {
    for ty in ["Word", "Byte"] {
        for op in ["band", "bor", "bxor"] {
            assert!(
                attempt(&binop(op, ty, ty)).is_ok(),
                "`{op}` on {ty} now diverges. The header states the divergence is confined to \
                 arithmetic and unary negation on Float and Fixed; a bitwise operator joining \
                 it means the cause is broader and the account needs rewriting"
            );
        }
        for op in ["lsl", "lsr", "asr", "asl"] {
            let src = format!(
                "fn f(a: {ty}) -> {ty} {{ a {op} 2 }}\nfn main(x: {ty}) -> {ty} {{ f(x) }}"
            );
            assert!(
                attempt(&src).is_ok(),
                "`{op}` on {ty} now diverges, so the shift family has joined the divergence"
            );
        }
    }
    for op in ["and", "or", "xor"] {
        assert!(
            attempt(&binop(op, "bool", "bool")).is_ok(),
            "boolean `{op}` now diverges, so the divergence is not confined to the numeric \
             operators"
        );
    }
}
