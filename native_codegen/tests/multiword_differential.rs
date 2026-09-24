//! **SIX MULTI-WORD OPERATIONS LOWER, AND UNTIL NOW NOTHING COMPARED THEM.**
//!
//! # The gap, measured before it was written down
//!
//! `Multiword<N, F>` had **one mention across every backend test and source file**,
//! and it was incidental: a shape in `len_producer_census.rs` that the REFERENCE
//! rejects. Probed with the real construction form:
//!
//! | operation | result |
//! |---|---|
//! | index, add, subtract, compare, shift, per-limb bitwise, fixed-point add | **lower, zero refusals** |
//! | **multiply** | **REFUSED** — an operand of unknown packed width reaching `NewComposite` |
//!
//! # ⚠ THE CONSTRUCTION SYNTAX WAS GUESSED WRONG FIRST, AND IT MATTERED
//!
//! `5 as Multiword<2, 0>` does not compile — *"cannot cast Word to Multiword<2>"*.
//! The parameter is a **LIMB COUNT**, and the value is built from a tuple of that
//! many limbs: `(42, 7) as Multiword<2>`.
//!
//! **Seven probes with the wrong syntax all reported "refused by the reference"**,
//! and a brief written from them would have claimed the backend cannot do any of
//! this. The wrong form was copied from the census subject above — which is itself
//! a shape the reference rejects, so it looked authoritative. The correct form came
//! from the reference's own suite.
//!
//! # A Multiword is a BODY, and that shapes the harness
//!
//! Its operands carry `Width::Body`. **Confusing `Body` with `Scalar` writes a
//! POINTER into a parent body** — this line's recorded silent-wrong-answer class —
//! so nothing here passes a multi-word value across the harness boundary. Every
//! subject extracts a `Word` by indexing, and that `Word` is what is compared.
//!
//! # ⚠ THE BACKEND HAS NO MULTI-WORD CODE AT ALL, AND THAT IS THE ARCHITECTURAL FACT
//!
//! Searching the emitter finds **no multi-word opcode and no occurrence of "limb"**.
//! The family reaches this backend as **composite construction, indexed element
//! reads, and per-limb scalar arithmetic**, which the reference compiler expands it
//! into. So there is no multi-word arm to test in isolation; what this file
//! establishes is that the COMPOSITION agrees.
//!
//! That also settles where a perturbation must go, and one attempt missed:
//!
//! | perturbation | outcome |
//! |---|---|
//! | `Op::GetField(Flat)` reads 8 bytes past its offset | **NOT detected** — multi-word indexing does not go through field access |
//! | the indexed element STRIDE is 8 bytes too large | **DETECTED** by *index the high limb*, in both float configurations |
//!
//! The second fires only on an index other than zero, which is why both a low-limb
//! and a high-limb subject are present. **The first is recorded because it failed**:
//! a perturbation that misses tells you where the path is not, and reporting only
//! the one that fired would have implied broader coverage than this file has.
//!
//! # ⚠ AT LEAST ONE LIMB COMES FROM A RUNTIME PARAMETER
//!
//! A wholly literal subject can be folded by the middle end, and the suite runs a
//! phase with the shipping middle end ENABLED. **A folded subject would compare two
//! constants and establish nothing about the lowering.** Every subject below takes
//! its first limb from the parameter.

mod common;

/// Subjects. Each returns a `Word` extracted by indexing, and each takes its first
/// limb from the runtime parameter `a`.
const SUBJECTS: &[(&str, &str)] = &[
    (
        "index the low limb",
        "fn main(a: Word, b: Word) -> Word { let m = (a, b) as Multiword<2>; m[0] }",
    ),
    (
        "index the high limb",
        "fn main(a: Word, b: Word) -> Word { let m = (a, b) as Multiword<2>; m[1] }",
    ),
    (
        "add, low limb",
        "fn main(a: Word, b: Word) -> Word { let x = (a, 0) as Multiword<2>; \
           let y = (b, 0) as Multiword<2>; let z = x + y; z[0] }",
    ),
    (
        "subtract, low limb",
        "fn main(a: Word, b: Word) -> Word { let x = (a, 0) as Multiword<2>; \
           let y = (b, 0) as Multiword<2>; let z = x - y; z[0] }",
    ),
    (
        "add, carry into the high limb",
        "fn main(a: Word, b: Word) -> Word { let x = (a, b) as Multiword<2>; \
           let y = (a, b) as Multiword<2>; let z = x + y; z[1] }",
    ),
    (
        "shift left by a constant",
        "fn main(a: Word, b: Word) -> Word { let x = (a, b) as Multiword<2>; let z = x lsl 1; z[0] }",
    ),
    (
        "per-limb bitwise and",
        "fn main(a: Word, b: Word) -> Word { let x = (a, 0) as Multiword<2>; \
           let y = (b, 0) as Multiword<2>; let z = x band y; z[0] }",
    ),
    (
        "per-limb bitwise or, high limb",
        "fn main(a: Word, b: Word) -> Word { let x = (a, a) as Multiword<2>; \
           let y = (0, b) as Multiword<2>; let z = x bor y; z[1] }",
    ),
    (
        "fixed-point add at sixteen fraction bits",
        "fn main(a: Word, b: Word) -> Word { let x = (a, 0) as Multiword<2, 16>; \
           let y = (b, 0) as Multiword<2, 16>; let z = x + y; z[0] }",
    ),
];

/// Argument PAIRS driven through each subject. Small and positive, so a subtraction
/// stays defined and an addition cannot approach a limb boundary by accident. The
/// pair `(5, 5)` is included because several operations are only distinguishable
/// when the two operands are EQUAL.
const ARGS: &[(i64, i64)] = &[(0, 0), (1, 3), (3, 1), (5, 5), (17, 2), (255, 7)];

/// **THE DIFFERENTIAL.**
#[test]
fn every_multiword_subject_agrees_with_the_reference() {
    for (label, src) in SUBJECTS {
        let m = common::build(src);
        let refusals =
            keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default());
        assert!(
            refusals.is_empty(),
            "`{label}` is refused: {refusals:?}. If that is deliberate it belongs \
             beside the multiply refusal with its cause, not silently here."
        );

        for &(a, b) in ARGS {
            let (vm, native) = common::vm_and_native_two_arg(src, a, b);
            assert_eq!(
                vm, native,
                "DIVERGENCE on `{label}` at (a, b) = ({a}, {b}):\n  {src}\n  \
                 reference = {vm}\n  native    = {native}\n\nThis establishes that \
                 the two implementations disagree, NOT which of them is right."
            );
        }
    }
}

/// **MULTIPLY IS REFUSED, WITH ITS CAUSE AND A CONTROL.**
///
/// `NewComposite` receives an operand whose packed width it cannot establish. That
/// is the same class as the resumed float reply before 2026-09-18, and the refusal
/// is sound for the same reason: it fails closed rather than packing at a guessed
/// size.
///
/// **The control is what stops this overclaiming.** `add` on the identical shape
/// LOWERS, so the refusal is specific to multiply rather than general to the
/// family — which is exactly the distinction the float-reply pin needed.
#[test]
fn multiword_multiply_is_refused_and_addition_is_not() {
    let mul = "fn main(a: Word, b: Word) -> Word { let x = (a, 0) as Multiword<2>; \
               let y = (b, 0) as Multiword<2>; let z = x * y; z[0] }";
    let refusals = keleusma_native::module_refusals(
        &common::build(mul),
        keleusma_native::LowerOptions::default(),
    );
    let Some((_, e)) = refusals.first() else {
        panic!(
            "multi-word multiply now LOWERS. That is the gap closing and it is good \
             news — but it must then be driven against the reference like every \
             other operation here, so this test must be replaced by a subject rather \
             than deleted."
        );
    };
    let msg = format!("{e:?}");
    assert!(
        msg.contains("NewComposite") && msg.contains("unknown packed width"),
        "multi-word multiply is refused for a different reason now: {msg}. The \
         recorded cause is an operand whose packed width is unknown."
    );

    // **THE CONTROL.** Without it, this test would support "the backend cannot do
    // multi-word arithmetic", which is false: eight other operations lower.
    let add = "fn main(a: Word, b: Word) -> Word { let x = (a, 0) as Multiword<2>; \
               let y = (b, 0) as Multiword<2>; let z = x + y; z[0] }";
    assert!(
        keleusma_native::module_refusals(
            &common::build(add),
            keleusma_native::LowerOptions::default(),
        )
        .is_empty(),
        "multi-word ADD is refused too, so the refusal above is not specific to \
         multiply and this test supports a broader claim than the facts do"
    );
}

/// **THE SUBJECTS ARE NOT CONSTANT-FOLDABLE.**
///
/// The suite runs a phase with the shipping middle end enabled. A wholly literal
/// subject could be folded to a constant on both sides, and the differential would
/// compare two constants while establishing nothing about the multi-word lowering.
///
/// Every subject must therefore mention the parameter. **Asserted rather than
/// intended**, because this is the kind of property that decays as subjects are
/// added.
#[test]
fn every_subject_uses_its_runtime_parameter() {
    for (label, src) in SUBJECTS {
        assert!(
            src.contains("(a,") || src.contains(" a)"),
            "`{label}` does not build a limb from the parameter, so the middle end \
             could fold it and the comparison would establish nothing: {src}"
        );
    }
    // **NON-VACUITY.** A subject that ignored its parameter must fail this check.
    let ignores = "fn main(a: Word) -> Word { let m = (1, 2) as Multiword<2>; m[0] }";
    assert!(
        !(ignores.contains("(a,") || ignores.contains(" a)")),
        "the check accepts a subject that ignores its parameter, so it guards nothing"
    );
}
