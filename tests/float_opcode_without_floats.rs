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
//! Measured 2026-09-04 on a build with `--no-default-features --features verify`,
//! and **RE-CHECKED 2026-09-12: unchanged, still unrepaired**. The re-check matters
//! because this feature set is not one continuous integration runs, so nothing
//! else would have noticed it drifting.
//!
//! **Reproduce the WHOLE file with `--no-default-features --features verify,compile`.**
//! The bare `verify` above runs the defect pin and SKIPS the control below, which is
//! gated on `compile`. One test passes and the run looks clean, so a reader following
//! the narrower command sees exactly what a complete run looks like while the half
//! that makes the pin meaningful never executed. Measured: `verify` alone runs 1 test,
//! `verify,compile` runs 2.
//!
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
//! # Status: REPAIRED 2026-10-30
//!
//! On the operator's ruling of 2026-10-30, `verify()` refuses a module carrying
//! `IntToFloat` or `FloatToInt` when the feature is absent, which moves the
//! refusal from run time to load time. `Vm::new` runs `verify()`, so it refuses
//! the module too. The table above records the sequence before the repair. The
//! test below pins the sequence after it. Recorded in
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
use keleusma::bytecode::Module;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

/// A module whose only interesting op is `IntToFloat`.
const FIXTURE: &[u8] = include_bytes!("fixtures/int_to_float.kbc");

/// **The repair.** The module decodes, and `verify()` and `Vm::new` both
/// refuse it, naming the opcode and the feature.
///
/// If `from_bytes` starts refusing it instead, the refusal moved earlier
/// still, which is acceptable, and this file should record the new sequence.
#[test]
fn a_float_module_is_refused_at_verification_on_a_build_without_floats() {
    let module = Module::from_bytes(FIXTURE).expect(
        "the fixture no longer decodes. If the wire format moved, regenerate it from \
         `fn main(k: Word) -> Float { k as Float }` on a floats-enabled build; this is a \
         stale fixture rather than a finding.",
    );

    let err = keleusma::verify::verify(&module)
        .expect_err("verify() admitted a float opcode on a build without the feature");
    assert!(
        err.message.contains("IntToFloat") && err.message.contains("`floats` feature"),
        "verify() refused the module for another reason: {}",
        err.message
    );

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    assert!(
        Vm::new(module, &arena).is_err(),
        "Vm::new loaded a float module on a build without the feature"
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
    use keleusma::bytecode::Value;
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
