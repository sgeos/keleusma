//! **THE NO-FLOATS SENTINEL, WHICH THIS PACKAGE HAD NEVER EXERCISED.**
//!
//! `float_bits_log2 == 0` is the sentinel for *a target with no floating point*,
//! used by `Target::embedded_8` and `embedded_16` alongside `has_floats: false`.
//! **It is not a one-bit format.** The `v0.2.3` line found that while preparing
//! unrelated work, after a planned refusal of "any width below 32 bits" would
//! have rejected every module built for those targets.
//!
//! Checking this package against it found that **no test here built a module for
//! a no-floats target at all**, so the whole path was uncovered. These tests are
//! that coverage, and they pin what was measured rather than what was expected.
//!
//! # ⚠ THE PREDICTION FAILED ON ITS OWN INTERESTING FALSIFIER
//!
//! Predicted: *"`float_type` is never called with a zero width."* **False.**
//! `lower_module` computes the entry ABI's float type unconditionally, so it is
//! called with zero for every no-floats module. The result is provably unused —
//! it is consumed only where a signature shape is a float, and such a module has
//! none — but the prediction as written is refuted and is recorded as refuted.
//!
//! **The falsifier that fired is the one the brief called the interesting one**,
//! and it fired because it demanded an instrument rather than a reading of the
//! call sites. The reading had said "unreachable".
//!
//! # What was decided, and it is to change nothing in the whitelist
//!
//! `float_type`'s default arm widens any unrecognised width to `f64`. That looks
//! like the silent-wrong-number hazard the same file warns against two functions
//! above it. **Measured, it is not reachable through any operation**: an
//! unlowerable non-zero width is refused before a float type is used, and no IR
//! is emitted at all. Threading an `Option` through six call sites and into
//! closures, to remove a hazard nothing can reach, would risk a real defect in a
//! backend measured correct. See `docs/decisions/NO_FLOAT_SENTINEL.md`.

mod common;

use inkwell::context::Context;
use keleusma::target::Target;
use keleusma::{compiler::compile_with_target, lexer::tokenize, parser::parse};
use keleusma_native::{LowerError, LowerOptions, lower_module};

const NO_FLOAT_SRC: &str = "fn main(w: Word) -> Word {\n  let d = w + 1;\n  w * d\n}\n";
const FLOAT_SRC: &str =
    "fn main(w: Word) -> Word {\n  let a = w as Float;\n  let s = a + 1.5;\n  s as Word\n}\n";

fn lower_for(src: &str, t: &Target) -> Result<Result<String, LowerError>, String> {
    let ast = parse(&tokenize(src).expect("lex")).expect("parse");
    let m = compile_with_target(&ast, t).map_err(|e| e.message)?;
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    Ok(match lower_module(&ctx, &lm, &m, LowerOptions::default()) {
        Ok(_) => {
            lm.verify().expect("LLVM module verification");
            Ok(lm.print_to_string().to_string())
        }
        Err(e) => Err(e),
    })
}

fn no_float_64bit() -> Target {
    // A 64-bit machine with no floating-point unit. An ordinary configuration,
    // and the only one that reaches the float question: the two embedded targets
    // are refused for WORD width long before a float type could be built.
    let mut t = Target::host();
    t.has_floats = false;
    t.float_bits_log2 = 0;
    t
}

/// The sentinel reaches the backend and is lowered, rather than being mistaken
/// for a one-bit float format.
#[test]
fn a_module_for_a_target_with_no_floats_lowers() {
    let t = no_float_64bit();
    assert_eq!(
        1u32 << t.float_bits_log2 >> 3,
        0,
        "the sentinel must give a zero byte width, or this test is probing something else"
    );
    let ir = lower_for(NO_FLOAT_SRC, &t)
        .expect("a float-free program must compile for a float-free target")
        .expect("and it must lower");
    assert!(
        ir.contains("kel_chunk_0"),
        "the module lowered to nothing, so the claim above is vacuous:\n{ir}"
    );
}

/// **WHY the zero width is harmless, measured rather than asserted.** No float
/// operation can exist in such a module, because the front end refuses one.
#[test]
fn a_float_operation_cannot_exist_under_a_no_floats_target() {
    let err = lower_for(FLOAT_SRC, &no_float_64bit())
        .expect_err("a float program must not compile for a float-free target");
    assert!(
        err.contains("floating-point"),
        "the refusal is not about floating point, so it may be incidental: {err}"
    );
}

/// **A DISTINCT FINDING, kept separate on purpose.** The two shipped embedded
/// targets never reach the float question at all: they are refused for word
/// width. Folding this into the float result would have credited the float
/// handling with a refusal it did not make.
#[test]
fn the_embedded_targets_are_refused_for_word_width_not_float_width() {
    for (label, t) in [
        ("embedded_16", Target::embedded_16()),
        ("embedded_8", Target::embedded_8()),
    ] {
        let outcome = lower_for(NO_FLOAT_SRC, &t).expect("compiles for the target");
        match outcome {
            Err(LowerError::UnsupportedWordWidth(_)) => {}
            other => {
                panic!("{label} was expected to be refused for WORD width; instead: {other:?}")
            }
        }
    }
}

/// **THE HAZARD THAT WOULD MATTER, AND IT IS NOT REACHABLE.**
///
/// A width this backend does not lower, non-zero and with floats enabled. If the
/// module lowered, `float_type`'s default arm would have widened it to `f64` and
/// the program would compute in the wrong precision — a plausible wrong number
/// rather than a fault, which is the exact failure this backend says it refuses.
#[test]
fn an_unlowerable_non_zero_float_width_is_refused_with_no_code_emitted() {
    let mut t = Target::host();
    t.has_floats = true;
    t.float_bits_log2 = 4; // 16-bit: two bytes, which this backend does not lower
    assert_eq!(1u32 << t.float_bits_log2 >> 3, 2);

    let outcome = lower_for(FLOAT_SRC, &t).expect("a 16-bit float target compiles");
    let err = match outcome {
        Err(e) => e,
        Ok(ir) => panic!(
            "a two-byte float width LOWERED instead of being refused. If it \
             widened to f64 the program computes in the wrong precision:\n{ir}"
        ),
    };
    let msg = format!("{err:?}");
    assert!(
        msg.contains("2 bytes"),
        "the refusal does not name the offending width, so it may be refusing \
         for an unrelated reason: {msg}"
    );
    assert!(
        msg.contains("4 and 8"),
        "the refusal names the wrong lowered set. It said only 8 was lowered \
         while 4 is lowered too, which predates f32 support: {msg}"
    );
}

/// **THE CLASS GUARD, because five instances of one mistake is a class.**
///
/// The `f32` increment taught the backend to lower a four-byte float and left
/// every refusal message saying that only an eight-byte one is lowered. Five
/// separate messages — the shared-slot ABI, the entry ABI, `GetIndex`,
/// `GetField`, and a doc comment — told a reader that `f32` is unsupported while
/// the code beside them supported it.
///
/// **A refusal that names the wrong supported set is worse than a bare
/// refusal.** It does not merely fail to help; it tells someone to stop using
/// something that works.
///
/// Pinning the individual strings would be brittle and would miss the sixth. This
/// pins the CLASS: no source line may claim eight is the only lowered float
/// width, for as long as `float_width_lowered` admits four.
#[test]
fn no_refusal_claims_eight_is_the_only_lowered_float_width() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    )
    .expect("the backend source");

    // Guard the guard: if four ever stops being lowered, this test must stop
    // demanding that the messages say so, and it should fail loudly rather than
    // quietly enforcing a stale rule of its own.
    assert!(
        src.contains("matches!(float_bytes, 4 | 8)"),
        "the lowered float set is no longer `4 | 8`. This test enforces that \
         refusal messages name four AND eight; re-derive it against the new set \
         rather than deleting it."
    );

    // **NON-VACUITY.** The assertion below is that a phrase is ABSENT, which
    // holds for an empty file, a wrong path, or a reader that silently returns
    // nothing. Require the CORRECTED phrasing to be present, so the test is
    // reading the file it thinks it is and the absence means something.
    let corrected = src.matches("4- and 8-byte Floats").count();
    assert!(
        corrected >= 4,
        "found only {corrected} corrected width phrases in the backend source;          the reader is probably not seeing the file it means to check, which          would make the absence below worthless"
    );

    let offenders: Vec<(usize, String)> = src
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            l.contains("only an 8-byte Float")
                || l.contains("only 8 is lowered")
                || l.contains("Only 8 is lowered")
        })
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect();

    assert!(
        offenders.is_empty(),
        "{} line(s) claim eight is the only lowered float width while four is \
         lowered too:\n  {}",
        offenders.len(),
        offenders
            .iter()
            .map(|(n, l)| format!("{n}: {l}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
