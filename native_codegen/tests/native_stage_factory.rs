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
//! Stages that call a host native are declined and run on the virtual machine,
//! which a compile does not reach and the framing stage does. The validation
//! stages run only through `verify_module_via_kel`, which the second test
//! drives. The programs are small, so the native stages run few turns, and the
//! cost of building every stage again per compile is not measured against the
//! virtual machine here.
//!
//! Dropping the executor's copy of the shared buffer back to the driver fails
//! both tests, which was checked by mutation. A reply delivered wrongly, the
//! reply plus one, passes both, because no stage these paths run reads its
//! reply. That was also checked by mutation, and it is the gap
//! `native_stage_executor.rs` records for the lexer and the parser.
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
