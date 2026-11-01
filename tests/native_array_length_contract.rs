//! What a host-registered native's DECLARED array length governs, and what it
//! does not.
//!
//! # Why this file exists
//!
//! The for-in iteration bound is folded from the iterable's static type. For a
//! call, that type is the callee's DECLARED return type, which for a native is
//! whatever its `use` declaration says. Nothing validates that a native actually
//! returns the number of elements it declared.
//!
//! That was raised as a possible soundness gap in the worst-case execution time
//! and memory guarantees while removing the compiler's `Op::Len` emission sites
//! on 2026-09-04. **Measured, it is not one**, and the measurement is worth
//! keeping because the reasoning that suggested otherwise is easy to repeat.
//!
//! # What the measurement established
//!
//! | the native declares | it returns | outcome |
//! |---|---|---|
//! | `[Word; 3]` | 3 elements | iterates 3 times |
//! | `[Word; 3]` | 5 elements | iterates 3 times; **the excess is silently ignored** |
//! | `[Word; 3]` | 1 element | **traps `IndexOutOfBounds`**, loudly |
//! | no signature | anything | **refused at type checking**; never a for-in source |
//!
//! # Why this is a host-contract matter and not a broken bound
//!
//! **The iteration bound is not wrong in any row.** The loop runs exactly the
//! declared count, or it traps. A bound that is met or exceeded loudly is sound;
//! what would be unsound is a loop running MORE times than the analysis
//! predicted, and no row does that.
//!
//! **The memory bound does not come from this type either.** A native's
//! worst-case memory contribution is host-attested per native
//! (`CallResolver::native_wcmu`), not derived from its declared return type. A
//! native that allocates more than it attested has broken its attestation, which
//! is a different contract from the one measured here and is part of the
//! documented trust model rather than a compiler defect.
//!
//! So the residue is narrow and real: **an over-long return is silently
//! truncated**. A host that returns five elements where it declared three gets
//! no diagnostic. That is worth knowing and is recorded rather than repaired,
//! because repairing it means validating a native's return against a declared
//! shape at the call boundary, which is a design change rather than a fix.
//!
//! # The pre-existence, measured rather than assumed
//!
//! Every row above was measured against the pre-change compiler as well, by
//! restoring `src/compiler.rs` from the branch point. All four rows are
//! identical. The `Op::Len` removal changed nothing here, because a native call
//! is an `Expr::Call` and the fold's structural arm for calls has always read
//! the declared return type.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{ArrayBody, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError};

/// How many elements the native below returns. A `fn` pointer is required at
/// registration, so the count cannot be captured and is carried here instead.
static RET_LEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(3);

/// **Serialises the tests in this file, and it is load-bearing.**
///
/// `RET_LEN` is process-global because registration takes a `fn` pointer rather
/// than a closure. Cargo runs the tests in one binary on parallel threads, so
/// without this lock one test sets the count and another reads it. The first
/// revision of this file omitted the lock and the under-long case passed or
/// failed by interleaving -- a test whose verdict depends on thread scheduling
/// is worse than no test, because it launders a coin flip as evidence.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn mk_native(_args: &[Value]) -> Result<Value, VmError> {
    let n = RET_LEN.load(std::sync::atomic::Ordering::SeqCst);
    Ok(Value::Array(ArrayBody::boxed(
        (1..=n).map(|i| Value::Int(i as i64)).collect::<Vec<_>>(),
    )))
}

/// A native declared to return three words, iterated by for-in. The body writes
/// each element to a data field, so the returned word is the LAST element the
/// loop saw, and therefore reports the iteration count.
const DECLARED_THREE: &str = "use host::mk() -> [Word; 3]\n\
     data s { n: Word }\n\
     fn main(k: Word) -> Word { for x in host::mk() { s.n = x; } s.n }";

/// Run the program with the native returning `n` elements.
fn run_with(src: &str, n: usize) -> Result<i64, String> {
    // Held for the whole run: the native reads `RET_LEN` during `call_with_shared`,
    // so releasing the lock before then would reintroduce the race.
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    RET_LEN.store(n, std::sync::atomic::Ordering::SeqCst);
    let tokens = tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let program = parse(&tokens).map_err(|e| format!("parse: {e:?}"))?;
    let module = compile(&program).map_err(|e| format!("compile: {}", e.message))?;
    keleusma::verify::verify(&module).map_err(|e| format!("verify: {e:?}"))?;
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("load: {e:?}"))?;
    vm.register_native("host::mk", mk_native);
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    match vm.call_with_shared(&mut shared, &[Value::Int(0)]) {
        Ok(keleusma::vm::VmState::Finished(Value::Int(v))) => Ok(v),
        Ok(other) => Err(format!("unexpected state: {other:?}")),
        Err(e) => Err(format!("run: {e:?}")),
    }
}

/// The honest case: the native returns what it declared.
#[test]
fn a_native_returning_its_declared_count_iterates_that_many_times() {
    assert_eq!(
        run_with(DECLARED_THREE, 3),
        Ok(3),
        "a native declaring [Word; 3] and returning three elements must iterate three times"
    );
}

/// **An over-long return is silently truncated to the declared length.**
///
/// This is the finding. It is not a broken bound -- the loop runs exactly the
/// declared three times, which is what the analysis predicted -- but the host
/// gets no diagnostic that two of its elements were dropped.
///
/// If this ever starts erroring instead, that is an improvement and this test
/// should record the new contract rather than be deleted.
#[test]
fn an_over_long_native_return_is_silently_truncated() {
    assert_eq!(
        run_with(DECLARED_THREE, 5),
        Ok(3),
        "a native declaring [Word; 3] and returning FIVE elements iterates three times and \
         drops the rest without a diagnostic. If this now errors, the contract was tightened \
         and that is worth recording here"
    );
}

/// An under-long return traps, loudly, at the first index past the end.
///
/// This is the safe direction and it is what keeps the truncation above from
/// being a soundness problem: the loop cannot run past what the native supplied
/// without the index check catching it.
#[test]
fn an_under_long_native_return_traps_rather_than_reading_past_the_end() {
    let err = run_with(DECLARED_THREE, 1)
        .expect_err("a native returning one element where three were declared must not succeed");
    assert!(
        err.contains("IndexOutOfBounds"),
        "the refusal changed identity; whatever stops this now must be at least as strong \
         as the bounds check. Got: {err}"
    );
}

/// **A native with no declared signature cannot be a for-in source at all.**
///
/// This is why the length fold replacing the dynamic length opcode cost no
/// capability: the only native whose array length is unknown is one the type
/// checker will not admit in iterable position in the first place.
///
/// Measured rather than inferred, because "the type checker surely rejects that"
/// is the shape of assumption that put a false comment in the virtual machine.
#[test]
fn an_unsignatured_native_is_refused_in_iterable_position() {
    let src = "use host::mk\n\
         data s { n: Word }\n\
         fn main(k: Word) -> Word { for x in host::mk() { s.n = x; } s.n }";
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    let err = compile(&program)
        .expect_err("an unsignatured native must not be admitted as a for-in source");
    assert!(
        err.message.contains("for-in expects an array"),
        "the refusal changed identity. If an unsignatured native is now admitted, its array \
         length is unknown and the iteration bound cannot be folded, so the emission floor \
         would refuse it -- and that WOULD be a capability change worth recording. Got: {}",
        err.message
    );
}
