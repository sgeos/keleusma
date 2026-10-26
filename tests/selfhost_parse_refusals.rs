//! Named example admission through the fallible self-hosted parser.
//! Parsing alone does not establish compilation or execution. Every refused
//! example is accepted by the reference and must remain unavailable evidence,
//! rather than an invalid-source verdict, in the shipping compiler.

#![cfg(feature = "self-host")]

use keleusma::selfhost::try_parse_functions;
use std::path::PathBuf;

/// Unsupported examples paired with their structured diagnostic code.
const REFUSED: &[(&str, i64)] = &[
    ("04_for_in.kel", 31),
    // `05_pipeline.kel` and `06_multiheaded.kel` used to "parse" only because the parser
    // skipped a token, were refused in Order 6 slice 1, and parse and compile
    // byte-identically since slice 3b, which lowered the pipe and literal heads.
    ("07_refinement.kel", 21),
    ("08_method_dispatch.kel", 19),
    // `09_big_numbers.kel` and `10_multbyte.kel` left this table in Order 6 slice 3, when
    // tuple-destructuring `let` began to parse. Both now compile byte-identically.
    ("11_signed.kel", 21),
];

/// The eleven named scripts in the original survey. Other development lines may
/// add examples independently. Removals still fail the explicit path checks.
const CORPUS: &[&str] = &[
    "01_arithmetic.kel",
    "02_struct_field.kel",
    "03_enum_match.kel",
    "04_for_in.kel",
    "05_pipeline.kel",
    "06_multiheaded.kel",
    "07_refinement.kel",
    "08_method_dispatch.kel",
    "09_big_numbers.kel",
    "10_multbyte.kel",
    "11_signed.kel",
];

fn scripts() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/scripts");
    CORPUS
        .iter()
        .map(|name| {
            let p = dir.join(name);
            assert!(
                p.is_file(),
                "{name} is named in this test's corpus and is not present. If it \
                 was renamed or removed, update CORPUS deliberately -- silently \
                 checking ten scripts while claiming eleven is the failure this \
                 assertion exists to prevent."
            );
            p
        })
        .collect()
}

#[test]
fn every_shipped_example_is_parsed_or_refused_by_name() {
    let paths = scripts();
    assert_eq!(
        paths.len(),
        CORPUS.len(),
        "the named corpus and the resolved paths disagree"
    );

    let mut refused: Vec<(String, String)> = Vec::new();
    let mut accepted: Vec<(String, usize)> = Vec::new();

    for path in &paths {
        let name = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .to_string();
        let src = std::fs::read_to_string(path).expect("script is readable");
        match try_parse_functions(&src) {
            Ok(program) => accepted.push((name, program.functions.len())),
            Err(e) => refused.push((name, e.message)),
        }
    }

    let refused_names: Vec<&str> = refused.iter().map(|(n, _)| n.as_str()).collect();
    let expected_names: Vec<&str> = REFUSED.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        refused_names, expected_names,
        "the set of refused example scripts moved. If a refusal was CLOSED, \
         update this table and say so where the count is published; if a new \
         one appeared, investigate its admission and execution behavior."
    );

    for ((name, _message), (_, expected_code)) in refused.iter().zip(REFUSED) {
        let source = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("examples/scripts")
                .join(name),
        )
        .unwrap();
        assert_unavailable(&source, *expected_code);
    }

    // Non-vacuity. A front end that accepted everything and produced nothing
    // would satisfy every assertion above, and that is close to what the
    // reported failure mode looked like from the outside.
    for (name, count) in &accepted {
        assert!(
            *count > 0,
            "{name} parsed to ZERO functions, so acceptance here means the \
             stage produced nothing rather than that it succeeded"
        );
    }
    assert_eq!(
        accepted.len(),
        paths.len() - REFUSED.len(),
        "accepted and refused do not partition the corpus"
    );
}

#[test]
fn a_refusal_is_returned_rather_than_ending_the_process() {
    // The point of the fallible entry point, stated as an executable claim: a
    // source that panics the original API comes back as a value here. If this
    // test runs to completion at all, the process was not aborted.
    let src = std::fs::read_to_string(
        // `04_for_in.kel` rather than `09_big_numbers.kel`, which parses since Order 6
        // slice 3.
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/scripts/04_for_in.kel"),
    )
    .expect("readable");

    let err = try_parse_functions(&src).expect_err("this source is refused today");
    assert!(
        !err.message.is_empty(),
        "a refusal with an empty message is worse than a panic, because the \
         caller has a value and no reason"
    );
    assert_eq!(
        err.to_string(),
        err.message,
        "Display must show the message, since that is the whole payload"
    );
}

fn assert_unavailable(source: &str, expected_code: i64) {
    use keleusma::selfhost::{
        SelfHostError, self_hosted_compile, try_call_type_inputs_from_pipeline,
    };
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).expect("reference accepts the unsupported control");
    let error = try_call_type_inputs_from_pipeline(source).expect_err("unsupported syntax");
    assert!(!error.is_source_rejection(), "{error}");
    assert!(
        matches!(error, ParseRecordError::Diagnostic { code, .. }
        if code == PARSE_DIAG_TAG_BASE - expected_code),
        "{error}"
    );
    assert!(matches!(
        self_hosted_compile(source, &keleusma::target::Target::host()),
        Err(SelfHostError::Unsupported { .. })
    ));
}

#[test]
fn unsupported_syntax_is_not_an_undefined_binding_verdict() {
    for (source, code) in [
        ("signed fn main() -> Word { 7 }", 21),
        (
            "fn main() -> Word { let xs = [1, 2]; for x in xs { let y = x + 1; } 7 }",
            31,
        ),
        (
            "newtype Count = Word; fn main() -> Word { Count(7) as Word }",
            21,
        ),
        // A tuple-destructuring `let` left this list in Order 6 slice 3 and is now a
        // supported control below.
    ] {
        assert_unavailable(source, code);
    }
}

#[test]
fn supported_loop_and_branch_controls_keep_complete_artifacts() {
    for source in [
        "fn main() -> Word { for i in 0..4 { let x = i + 1; } 7 }",
        "fn main() -> Word { for i in 0..4 limit 4 { let x = i + 1; } 7 }",
        "fn main() -> Word { if true { if false { 2 } else { 7 } } else { 3 } }",
        "struct P { x: Word } fn main() -> Word { let p = P { x: 7 }; p.x }",
        "fn main() -> Word { let (x, y) = (3, 4); x + y }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let reference = keleusma::compiler::compile(&ast).unwrap();
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
    }
}

#[test]
fn genuinely_undefined_names_still_reject_independently() {
    use keleusma::selfhost::{SelfHostError, self_hosted_compile};
    let source = "fn main() -> Word { missing + 1 }";
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    assert!(keleusma::compiler::compile(&ast).is_err());
    assert!(matches!(
        self_hosted_compile(source, &keleusma::target::Target::host()),
        Err(SelfHostError::PipelineRejected { .. })
    ));
}

#[test]
fn checked_arithmetic_refusal_cases_now_compile_and_execute() {
    for source in [
        "fn main() -> Word { 1 + 2 { ok(v) => v, overflow(_, l) => l, underflow(_, l) => l } }",
        "fn main() -> Word { if true { 1 + 2 { ok(v) => v, overflow(_, l) => l, underflow(_, l) => l } } else { 7 } }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let reference = keleusma::compiler::compile(&ast).unwrap();
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
        let arena = keleusma::Arena::with_capacity(65536);
        let mut vm = keleusma::vm::Vm::new(actual, &arena).unwrap();
        assert!(matches!(
            vm.call(&[]).unwrap(),
            keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(3))
        ));
    }
}
