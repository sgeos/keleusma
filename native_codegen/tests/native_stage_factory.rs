//! **OPERATOR QUESTION 19, STEP 3: THE SELF-HOSTED COMPILER WITH NATIVE STAGES.**
//!
//! `keleusma_native::stage_factory::NativeStageFactory` is installed around
//! V0.2.X's self-hosted compile with `keleusma::selfhost::with_stage_factory`,
//! so each stage the driver builds and can run natively runs as native code.
//! The compiled module must be byte-identical to the self-hosted compile on the
//! virtual machine and to the reference compiler's.
//!
//! # What this does not establish
//!
//! The validation stages run only through `verify_module_via_kel`, which the
//! second test drives. `frame.kel` calls ten host natives, which the third test
//! runs through the executor's bridge. That test signs and verifies one module
//! and does not reach a native's error path, so the bridge's refusal of a
//! failing native is not exercised. The programs are small, so the native stages run few turns, and the
//! cost of building every stage again per compile is not measured against the
//! virtual machine here.
//!
//! Dropping the executor's copy of the shared buffer back to the driver fails
//! the compile and validation tests, which was checked by mutation. A reply
//! delivered wrongly, the reply plus one, passes those two, because no stage
//! they run reads its reply, and fails the framing test, whose stage reads its
//! commands. Routing a two-argument native to the wrong closure also fails the
//! framing test. Both were checked by mutation.
mod common;

use keleusma::selfhost::{self_hosted_compile, verify_module_via_kel, with_stage_factory};
use keleusma::target::Target;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::stage_factory::NativeStageFactory;
use std::rc::Rc;

/// Programs in the self-hosted subset reaching calls, branches, a tuple and a
/// stream entry.
const PROGRAMS: &[&str] = &[
    "fn f(n: Word) -> Word { if n > 2 { n * 3 } else { n + 1 } }\n\
     fn main() -> Word { f(4) + f(1) }",
    "fn main(a: Word, b: Word) -> Word { let c = a + b; if c >= 10 { c } else { c * 2 } }",
    "fn swap(a: Word, b: Word) -> (Word, Word) { (b, a) }\n\
     fn main(a: Word) -> Word { let (x, y) = swap(a, a + 5); x - y }",
    "loop main(t: Word) -> Word { let t = yield t + 1; t }",
];

fn reference(source: &str) -> Vec<u8> {
    compile(&parse(&tokenize(source).expect("lex")).expect("parse"))
        .expect("compile")
        .to_bytes()
        .expect("serializes")
}

fn self_hosted(source: &str) -> Vec<u8> {
    self_hosted_compile(source, &Target::host())
        .unwrap_or_else(|e| panic!("{source}: not in the self-hosted subset: {e:?}"))
        .to_bytes()
        .expect("serializes")
}

#[test]
fn native_stages_compile_byte_identically() {
    let factory = Rc::new(NativeStageFactory::default());
    let mut compiled = 0usize;
    for source in PROGRAMS {
        let on_vm = self_hosted(source);
        let built_before = factory.built();
        let native = with_stage_factory(factory.clone(), || self_hosted(source));
        let built = factory.built() - built_before;
        assert_eq!(native, on_vm, "{source}: native stages diverge from the vm");
        assert_eq!(
            native,
            reference(source),
            "{source}: diverges from the reference"
        );
        assert!(built >= 5, "{source}: only {built} stages ran natively");
        eprintln!("  {built} native stages, {} bytes identical", native.len());
        compiled += 1;
    }
    assert_eq!(compiled, PROGRAMS.len());
    eprintln!(
        "  {} builds from {} compiled stage modules",
        factory.built(),
        factory.compiled()
    );
    // The cache: one compile per distinct stage module, shared by every build.
    assert!(
        factory.compiled() <= 15 && factory.built() > factory.compiled(),
        "{} builds compiled {} stage modules, so builds are not sharing code",
        factory.built(),
        factory.compiled()
    );
    assert_eq!(
        factory.declined(),
        0,
        "a compile stage was declined and ran on the virtual machine"
    );
}

#[test]
fn native_validation_stages_agree_with_the_vm() {
    let factory = Rc::new(NativeStageFactory::default());
    for source in PROGRAMS {
        let module =
            compile(&parse(&tokenize(source).expect("lex")).expect("parse")).expect("compile");
        let on_vm = verify_module_via_kel(&module);
        let native = with_stage_factory(factory.clone(), || verify_module_via_kel(&module));
        assert_eq!(
            format!("{native:?}"),
            format!("{on_vm:?}"),
            "{source}: the validation verdicts differ"
        );
        assert!(native.is_ok(), "{source}: {native:?}");
    }
    eprintln!(
        "  {} validation stages native, {} declined",
        factory.built(),
        factory.declined()
    );
    assert!(
        factory.built() >= 4,
        "too few validation stages ran natively"
    );
}

/// `frame.kel` calls ten host natives, which the framing driver registers as
/// closures over its signing state. Built through the factory, the stage binds
/// each to a trampoline, so the framing runs natively. Signing must produce the
/// same bytes as on the virtual machine, and verification must accept the
/// signed module and refuse a tampered one, as it does there.
#[test]
fn the_framing_stage_runs_natively_with_its_natives_bridged() {
    use keleusma::crypto_frame::{frame_module, signed_wire_bytes, verify_signature};
    let module =
        compile(&parse(&tokenize(PROGRAMS[0]).expect("lex")).expect("parse")).expect("compile");
    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let on_vm = signed_wire_bytes(&module, &key, false).expect("vm signing");

    let factory = Rc::new(NativeStageFactory::default());
    let native = with_stage_factory(factory.clone(), || signed_wire_bytes(&module, &key, false))
        .expect("native signing");
    assert_eq!(native, on_vm, "native signing diverges from the vm");
    assert_eq!(factory.declined(), 0, "the framing stage was declined");
    assert!(
        factory.built() > 0,
        "the framing stage was not built natively"
    );
    assert!(
        keleusma_native::stage_factory::NativeExecutor::new(&frame_module().expect("frame"))
            .is_ok(),
        "frame.kel does not build natively with its natives bridged"
    );

    let keys = [key.verifying_key()];
    let accepted = with_stage_factory(factory.clone(), || verify_signature(&native, &keys));
    assert!(
        accepted.is_ok(),
        "native verification refused: {accepted:?}"
    );
    // A byte of the opcode stream, which the signature covers. The last four
    // bytes are the CRC trailer, which the signature excludes by design and the
    // loader checks instead, so a flipped trailer byte passes this check.
    let mut tampered = native.clone();
    tampered[100] ^= 1;
    let refused = with_stage_factory(factory.clone(), || verify_signature(&tampered, &keys));
    let refused_on_vm = verify_signature(&tampered, &keys);
    assert!(
        refused.is_err(),
        "native verification accepted a tampered module"
    );
    assert_eq!(
        format!("{refused:?}"),
        format!("{refused_on_vm:?}"),
        "the refusals differ"
    );
    eprintln!(
        "  {} framing builds native, {} signed bytes identical",
        factory.built(),
        native.len()
    );
}

/// Compile every `step`-th corpus source, in path order, with native stages,
/// and require each the self-hosted compiler accepts to be byte-identical to
/// the reference compiler's module. Returns the sources compared and those
/// outside the self-hosted subset. The driver refuses some sources by a caught
/// panic in `reconstruct.kel`, so a refusal message on the output is expected
/// and does not fail the test.
fn compile_corpus_natively(step: usize) -> (usize, usize) {
    let factory = Rc::new(NativeStageFactory::default());
    let (mut compared, mut outside) = (0usize, 0usize);
    for path in common::corpus_sources().into_iter().step_by(step) {
        let name = path.display().to_string();
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(reference) = common::try_build(&source) else {
            continue;
        };
        let Ok(native) = with_stage_factory(factory.clone(), || {
            self_hosted_compile(&source, &Target::host())
        }) else {
            outside += 1;
            continue;
        };
        assert_eq!(
            native.to_bytes().expect("serializes"),
            reference.to_bytes().expect("serializes"),
            "{name}: native stages diverge from the reference"
        );
        compared += 1;
    }
    eprintln!(
        "  compared {compared} corpus sources, {outside} outside the self-hosted subset, \
         {} builds from {} compiled stage modules, {} declined",
        factory.built(),
        factory.compiled(),
        factory.declined()
    );
    assert_eq!(factory.declined(), 0, "a stage ran on the virtual machine");
    (compared, outside)
}

/// The corpus-wide end-to-end check of question 19, over the whole corpus.
///
/// It takes about six minutes in the suite's build, so it is ignored by
/// default and run with `--ignored`. On 2026-10-30 all 69 sources the subset
/// accepts agreed, and in a run that also compiled each on the virtual machine
/// they agreed three ways, the reference, the virtual machine and the native
/// stages. The virtual machine run is not repeated here, because its agreement
/// with the reference is the driver's own cross-check.
#[test]
#[ignore = "about six minutes; run with --ignored"]
fn every_corpus_source_compiles_identically_with_native_stages() {
    let (compared, _) = compile_corpus_natively(1);
    assert!(
        compared >= 69,
        "only {compared} corpus sources were compared"
    );
}

/// Every fourth corpus source, the default suite's share of the check above.
#[test]
fn a_sample_of_the_corpus_compiles_identically_with_native_stages() {
    let (compared, _) = compile_corpus_natively(4);
    assert!(
        compared >= MIN_SAMPLED,
        "only {compared} sampled sources were compared"
    );
}

/// Sampled sources compared on 2026-10-30. A drop fails the test.
const MIN_SAMPLED: usize = 18;
