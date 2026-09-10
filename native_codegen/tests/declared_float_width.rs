//! **A DECLARED `Float` MUST BE SIZED BY THE MODULE'S WIDTH, NOT BY EIGHT.**
//!
//! `width_of_declared_shape` collapses a declared shape to a byte width, and it
//! used to hard-code all three widths it passes to the layout model. The address
//! one is defensible — `Opaque` is refused at every reachable route. **The float
//! one was a real defect**, and the comment on `OperandKind` asserted the premise
//! that made it look safe: *"a `Float` and a `Word` are both eight bytes"*, which
//! is false under `narrow-float-32`.
//!
//! # The symptom was a REFUSAL, not a wrong number, and that distinction is the
//! # reason this was latent rather than dangerous
//!
//! Measured before repairing. A struct with a `Float` field, built from a
//! declared-`Float` call result, was **rejected** under `narrow-float-32`:
//!
//! ```text
//! NewComposite at op 3 packs 16 bytes but the instruction bakes 12;
//! the layout model has drifted
//! ```
//!
//! Sixteen is `8 + 8`; twelve is `4 + 8`. **The backend refused a program it
//! should lower and blamed the layout model for a constant of its own.**
//!
//! It stayed loud rather than silent **only because the packed size is
//! cross-checked against the size the instruction bakes.** That guard converts a
//! mispack into a refusal — the same property that makes the `Opaque` refusal
//! protective. The danger was never being wrong; it was being wrong silently.
//!
//! # Why the plain path did not catch it
//!
//! A declared-`Float` call result that stays on the operand stack and is
//! converted **agrees on both configurations even with the wrong width**, because
//! nothing consumes the width there. **The width only matters where bytes are
//! packed.** So a test of the obvious shape would have passed and proved nothing,
//! which is why this file exercises the composite path specifically.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma::{lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

/// A declared-`Float` call result packed into a composite body. The width decides
/// how many bytes are written, so a wrong one mispacks — or, with the size
/// cross-check in place, refuses.
const CALL_INTO_COMPOSITE: &str = "\
struct P { x: Float, n: Word }
fn scale(w: Word) -> Float {
  (w as Float) * 1.5
}
fn main(w: Word) -> Word {
  let p = P { x: scale(w), n: w };
  (p.x as Word) + p.n
}
";

/// The plain path, kept as the CONTRAST that shows why it is insufficient.
const CALL_PLAIN: &str = "\
fn scale(w: Word) -> Float {
  (w as Float) * 1.5
}
fn main(w: Word) -> Word {
  let f = scale(w);
  f as Word
}
";

fn agree_on(src: &str, label: &str) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).unwrap_or_else(|e| {
        panic!(
            "{label}: the backend refused a program it should lower: {e:?}. If this \
             names a packed size against a baked size, a width in this backend is \
             hard-coded again."
        )
    });
    lm.verify().expect("LLVM module verification");
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    // **ASSERT THE SIGNATURE BEFORE NAMING IT.** A harness naming
    // `extern "C" fn(..)` by hand cannot see the callee's signature change, and
    // this line has one recorded instance of ten tests passing on the calling
    // convention's good manners after exactly that. This subject builds no
    // composite and is no stream, so it carries its parameters and nothing else;
    // if that stops being true, the call below reads registers the callee never
    // wrote.
    // ⚠ **THIS ASSERTION FIRED THE MOMENT IT WAS ADDED: left 4, right 1.**
    //
    // The subject BUILDS A COMPOSITE, so its entry carries the three trailing
    // pointers — and this harness had been calling it through
    // `extern "C" fn(i64) -> i64`. The shared, private and region pointers were
    // whatever the calling convention left in those registers, and the composite
    // lowering wrote through one of them.
    //
    // **The test passed anyway**, which is precisely the recorded failure mode:
    // ten tests in `yield_sequence.rs` once passed the same way. An agreement
    // reached through registers the caller never set is not evidence about the
    // packed width or about anything else.
    //
    // Found by auditing a completion condition that required every hand-written
    // signature to be guarded — not by a failure.
    // **DERIVED, NOT ASSUMED.** A first fix hard-coded "plus three pointers",
    // which was right for the composite-building subject and WRONG FOR ITS
    // SIBLING — the same mistake in the other direction, caught within a minute
    // by the sibling test. Only two entry shapes are valid: the declared
    // parameters alone, or those plus the three arena pointers.
    let declared = u32::from(m.chunks[entry].param_count);
    let actual = lm
        .get_function(&format!("kel_chunk_{entry}"))
        .expect("entry function")
        .count_params();
    assert!(
        actual == declared || actual == declared + 3,
        "{label}: the entry takes {actual} parameters, which is neither the \
         declared {declared} nor those plus the three arena pointers. The calls \
         below name their signatures by hand and cannot describe a third shape"
    );
    let with_pointers = actual == declared + 3;

    // Buffers the callee may legitimately write. Sized from the planner rather
    // than guessed, and filled with a non-zero pattern so a read of
    // uninitialised space is distinguishable from a real zero.
    let mut shared = vec![0u8; 4096];
    let mut privs = vec![0u8; keleusma::vm::required_persistent_capacity_for(&m) + 4096];
    let mut region =
        vec![0xCDu8; keleusma_native::region::host_arena_supplement_bytes(&m) as usize + 4096];

    // **NON-VACUITY.** Values chosen so the float multiply MOVES the result: an
    // agreement on inputs where the float path is the identity would prove
    // nothing about the width.
    let mut moved = false;
    for arg in [0i64, 1, 3, 7, 41, -5] {
        let mm = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
        let cap = auto_arena_capacity_for(&mm, &[]).expect("arena capacity");
        let arena = keleusma_arena::Arena::with_capacity(cap);
        let mut vm = Vm::new(mm, &arena).expect("vm");
        let vmv = match vm.call(&[Value::Int(arg)]).expect("vm run") {
            keleusma::vm::VmState::Finished(Value::Int(v)) => v,
            other => panic!("{label}: unexpected VM outcome {other:?}"),
        };
        let sym = format!("kel_chunk_{entry}");
        let nv = if with_pointers {
            let f = unsafe {
                ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
            }
            .expect("entry symbol");
            unsafe {
                f.call(
                    arg,
                    shared.as_mut_ptr(),
                    privs.as_mut_ptr(),
                    region.as_mut_ptr(),
                )
            }
        } else {
            let f = unsafe { ee.get_function::<unsafe extern "C" fn(i64) -> i64>(&sym) }
                .expect("entry symbol");
            unsafe { f.call(arg) }
        };
        assert_eq!(
            vmv, nv,
            "{label}: native disagrees with the reference at {arg}"
        );
        if vmv != arg {
            moved = true;
        }
    }
    assert!(
        moved,
        "{label}: the program returns its own argument for every probe, so the \
         agreement above is vacuous"
    );
}

/// **THE ONE THAT USED TO FAIL.** Under `narrow-float-32` this was refused before
/// the width was threaded through.
#[test]
fn a_declared_float_call_result_packs_into_a_composite_at_the_modules_width() {
    agree_on(CALL_INTO_COMPOSITE, "composite");
}

/// **THE CONTRAST, and it is the point.** This shape agreed even with the wrong
/// width, because nothing on it consumes the width. A file that tested only this
/// would have been green throughout the defect's lifetime.
#[test]
fn the_plain_call_path_agrees_and_would_not_have_caught_it() {
    agree_on(CALL_PLAIN, "plain");
}
