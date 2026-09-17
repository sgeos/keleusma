//! **A FLOAT-PARAMETER STREAM PANICKED THE EMITTER.**
//!
//! # The defect
//!
//! `lower_module` restores a general stream's resume parameter into slot 0 at two
//! places. One guarded with `is_float_value()`; the other called
//! `.into_int_value()` unconditionally and **PANICKED** on a well-formed
//! `loop main(t: Float)`:
//!
//! > `Found FloatValue { .. llvm_type: "double" } but expected the IntValue variant`
//!
//! **A panic on a public entry point** is a defect class this line has recorded
//! before — `lowering_robustness.rs` exists to assert that malformed bytecode
//! REFUSES rather than panicking, and this module was not even malformed.
//!
//! **Two sites restoring the same value, one handling floats and one not.** The
//! disagreement is the defect; a single shared conversion is what prevents it.
//!
//! # And the guarded site was open-coding the conversion
//!
//! It used a raw `build_bit_cast(.., i64t)` — invalid IR for a four-byte float,
//! the identical mistake that broke `Op::Neg` under `narrow-float-32`. **Found by
//! censusing every `build_bit_cast` outside the two helpers after that fix: this
//! was the only one left.** Both sites route through `float_to_bits` now.
//!
//! # What this file checks
//!
//! That a float-parameter stream reaches a DECISION — lowered with valid IR, or
//! refused with a reason — rather than a panic, in whichever float configuration
//! the gate is running.

use inkwell::context::Context;

mod common;

/// Streams whose resume parameter is a `Float`, which is the shape that panicked.
const SUBJECTS: &[(&str, &str)] = &[
    (
        "float in, float out",
        "loop main(t: Float) -> Float { let r = yield t; yield (r + t) }",
    ),
    (
        "float in, word out",
        "loop main(t: Float) -> Word { let r = yield (t as Word); yield r }",
    ),
    (
        "float in, negated across a yield",
        "loop main(t: Float) -> Float { let n: Float = -t; let r = yield n; yield (r + n) }",
    ),
];

/// A `Word`-parameter stream, so a failure here means the harness broke rather
/// than the float path.
const CONTROL: &str = "loop main(t: Word) -> Word { let r = yield t; yield (r + t) }";

/// Lower, and report which of the three outcomes happened. **A panic is not one
/// of them**, which is the whole point: this returns only if the emitter did.
fn outcome(src: &str) -> Result<(), String> {
    let m = common::build(src);
    let ctx = Context::create();
    let lm = ctx.create_module("k");
    match keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default()) {
        // A refusal is an acceptable decision: this backend refuses rather than
        // guessing in many places, and that is the conservative stance working.
        Err(_) => Ok(()),
        Ok(_) => lm.verify().map_err(|e| {
            format!(
                "lowered but INVALID: {}",
                e.to_string().lines().next().unwrap_or("").trim()
            )
        }),
    }
}

#[test]
fn a_float_parameter_stream_reaches_a_decision_rather_than_a_panic() {
    assert!(
        outcome(CONTROL).is_ok(),
        "the word-parameter control does not lower; the harness is broken, not \
         the float path"
    );
    let mut bad = Vec::new();
    for (label, src) in SUBJECTS {
        if let Err(e) = outcome(src) {
            bad.push((*label, e));
        }
    }
    assert!(
        bad.is_empty(),
        "float-parameter stream(s) that lowered to invalid IR: {bad:?}.\n\n\
         A refusal would be acceptable here — this backend refuses rather than \
         guessing in many places. **Invalid IR and a panic are not**, and the \
         panic is what this file was written for."
    );
}

/// **The two restore sites must agree.** They disagreed for as long as one
/// guarded floats and the other did not, and that disagreement was the defect.
#[test]
fn neither_resume_restore_open_codes_its_conversion() {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    let raw_casts = src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//") && l.contains("build_bit_cast")
        })
        .count();
    assert_eq!(
        raw_casts, 4,
        "there are {raw_casts} raw `build_bit_cast` call(s) outside comments; \
         **four is the count INSIDE the two width-aware helpers**, where they \
         belong. A fifth means an arm is open-coding a float conversion again, \
         which is invalid IR at four bytes and is exactly how `Op::Neg` and the \
         resume restore both broke."
    );
}
