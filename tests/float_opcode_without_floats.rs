//! A module using floats **verifies, loads, and then traps** on a runtime built
//! without the `floats` feature.
//!
//! # The hazard
//!
//! `VmError::InvalidBytecode` means "this artefact should never have been
//! produced". It is **the class `verify()` exists to exclude**. A module that
//! passes verification, loads, and then raises it is a hole in the load-time
//! guarantee rather than a bad program.
//!
//! Measured 2026-09-04 on a build with `--no-default-features --features verify`:
//!
//! | step | result |
//! |---|---|
//! | `Module::from_bytes` | accepted |
//! | `verify()` | **accepted** |
//! | `Vm::new` | **loaded** |
//! | call | **`InvalidBytecode("Op::IntToFloat requires the `floats` feature")`** |
//!
//! # Why nothing catches it earlier
//!
//! Two independent reasons, and closing either would be enough.
//!
//! 1. **`verify()` has no `floats` gating at all.** Not one conditional in
//!    `src/verify.rs` mentions the feature, so the structural pass has no notion
//!    that a float opcode is inadmissible on this build.
//! 2. **The header width check cannot reject it.** Load compares the module's
//!    `float_bits_log2` against `RUNTIME_FLOAT_BITS_LOG2` and admits when
//!    `got <= max_supported` -- but that constant is **not** gated on the
//!    `floats` feature. A build without floats still advertises the full width,
//!    so the comparison passes.
//!
//! # Why this is not a corrupt-artefact case
//!
//! Nothing here is hand-built or damaged. The fixture is the ordinary output of
//! the reference compiler for `fn main(k: Word) -> Float { k as Float }`, and the
//! runtime is an ordinary supported configuration -- omitting floats is the
//! point of the feature, and an embedded target is exactly where it is used. The
//! two are produced by different builds, which is the normal deployment shape for
//! a language that ships precompiled bytecode.
//!
//! # Proportionality, which must be stated every time
//!
//! The trap is **loud**. It is a clean error at call time, not a wrong answer, a
//! crash, or memory unsafety. What is wrong is the LAYER: this project's stance
//! is that a module the runtime cannot execute is refused at load, and this one
//! is admitted and then refused mid-call. Exposure is to a host that builds
//! without `floats` and runs a module produced by a build that had them.
//!
//! # Status: PINNED, NOT REPAIRED
//!
//! The repair belongs in `verify()` -- refuse a module carrying a float opcode
//! when the feature is absent, moving the refusal from run time to load time.
//! It is deliberately not done here, because a repair validated by the same
//! reading that found the defect is not validated, and because the feature set
//! this lives in is not one continuous integration runs. Recorded in
//! `docs/decisions/INVALID_BYTECODE_CENSUS.md`.
//!
//! # The fixture, and how to regenerate it
//!
//! `fixtures/int_to_float.kbc` is `fn main(k: Word) -> Float { k as Float }`
//! compiled by the reference compiler on a floats-enabled build and written with
//! `Module::to_bytes`. It cannot be produced inside this test, because the build
//! that runs the test is precisely the one that cannot compile a float program.
//! If the wire format changes and this fixture stops loading, regenerate it from
//! that source on a floats-enabled build rather than deleting the test: a
//! `from_bytes` failure here is a stale fixture, and the assertions below say so.

#![cfg(all(feature = "verify", not(feature = "floats")))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

/// A module whose only interesting op is `IntToFloat`.
const FIXTURE: &[u8] = include_bytes!("fixtures/int_to_float.kbc");

/// **The ratchet.** Every step up to the call succeeds, and the call traps.
///
/// If this test fails because the module is now refused EARLIER -- by
/// `from_bytes`, by `verify()`, or by `Vm::new` -- the hole has been closed and
/// this file should record the new sequence rather than be deleted. That is the
/// outcome it exists to detect.
#[test]
fn a_float_module_verifies_and_loads_and_then_traps_at_the_call() {
    let module = Module::from_bytes(FIXTURE).expect(
        "the fixture no longer decodes. If the wire format moved, regenerate it from \
         `fn main(k: Word) -> Float { k as Float }` on a floats-enabled build; this is a \
         stale fixture rather than a finding.",
    );

    keleusma::verify::verify(&module).expect(
        "verify() now REJECTS a float-using module on a build without the feature. That is \
         the repair this file recommends: record the new behaviour here instead of deleting \
         the test.",
    );

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect(
        "Vm::new now rejects a float-using module on a build without the feature. That also \
         closes the hole, at the load layer; record it here.",
    );

    let err = vm
        .call(&[Value::Int(3)])
        .expect_err("the call SUCCEEDED, so this build executes float opcodes after all");

    let text = alloc::format!("{err:?}");
    assert!(
        text.contains("InvalidBytecode"),
        "the call now fails with something other than InvalidBytecode: {text}. If the runtime \
         reports a plain unsupported-operation error instead, that is an improvement in \
         classification and worth recording, because InvalidBytecode asserts the ARTEFACT is \
         malformed and this artefact is not."
    );
}

extern crate alloc;

/// **The control: a module with no float opcode compiles, verifies, loads, and
/// RUNS on this very build.**
///
/// Without it the test above could pass because this build refuses to run
/// anything at all, which would make it evidence of nothing. The first revision
/// of this control reused the float fixture and asserted only that it loaded,
/// which separated nothing -- the same vacuity this repository has recorded in
/// six other costumes. It compiles its own program instead.
#[test]
#[cfg(feature = "compile")]
fn a_module_without_float_opcodes_runs_on_this_build() {
    use keleusma::compiler::compile;
    use keleusma::lexer::tokenize;
    use keleusma::parser::parse;

    let src = "fn main(k: Word) -> Word { k + 1 }";
    let module = compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
        .expect("a float-free program must still compile on a build without the feature");
    keleusma::verify::verify(&module).expect("a float-free module must verify here");

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("a float-free module must load here");
    let state = vm.call(&[Value::Int(41)]).expect(
        "a float-free module must RUN here; if it does not, the trap observed above \
                 says nothing specific about float opcodes",
    );
    assert!(
        alloc::format!("{state:?}").contains("42"),
        "the control ran but produced {state:?} rather than 42, so this build is not \
         executing correctly and the finding above is not attributable to floats"
    );
}
