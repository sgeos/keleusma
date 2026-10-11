//! Source initializers must reach the independently assembled persistent state.
#![cfg(feature = "self-host")]

#[test]
fn private_initializers_preserve_complete_artifacts() {
    let cases = [
        (
            "word",
            "private data d { n: Word = 5 } fn main() -> Word { d.n = d.n + 1; d.n }",
        ),
        (
            "negative",
            "private data d { n: Word = -5 } fn main() -> Word { let n = d.n; d.n = 0; n }",
        ),
        (
            "grouped",
            "private data d { n: Word = ((-5)) } fn main() -> Word { let n = d.n; d.n = 0; n }",
        ),
        (
            "byte",
            "private data d { b: Byte = 255 } fn main() -> Word { let n = d.b as Word; d.b = 0 as Byte; n }",
        ),
        (
            "true",
            "private data d { b: bool = true } fn main() -> Word { let n = if d.b { 7 } else { 0 }; d.b = false; n }",
        ),
        (
            "false",
            "private data d { b: bool = false } fn main() -> Word { let n = if d.b { 7 } else { 0 }; d.b = true; n }",
        ),
        (
            "defaults",
            "private data d { n: Word = 9, b: bool, c: Byte, xs: [Word; 2] } fn main() -> Word { let n = d.n; d.n = 0; n }",
        ),
        (
            "shared first",
            "shared data s { n: Word } private data d { n: Word = 5 } fn main() -> Word { d.n = d.n + 1; d.n }",
        ),
        (
            "private first",
            "private data d { n: Word = 5 } shared data s { n: Word } fn main() -> Word { d.n = d.n + 1; d.n }",
        ),
        (
            "interleaved",
            "private data a { n: Word = 5 } shared data s { n: Word } private data b { n: Word = 8 } fn main() -> Word { let n = a.n + b.n; a.n = 0; b.n = 0; n }",
        ),
        (
            "trailing comma",
            "private data d { n: Word = 5, } fn main() -> Word { d.n = d.n + 1; d.n }",
        ),
        (
            "fixed zero",
            "private data d { n: Fixed<16> } fn main(x: Fixed<16>) -> Word { d.n = x; 7 }",
        ),
        #[cfg(feature = "floats")]
        (
            "float zero",
            "private data d { n: Float } fn main(x: Float) -> Word { d.n = x; 7 }",
        ),
        (
            "grouped type",
            "private data d { n: ((Word)) = -1 } fn main() -> Word { let n = d.n; d.n = 0; n }",
        ),
        (
            "implicit comma",
            "private data d { n: Word = 5 m: Byte = 2 } fn main() -> Word { let n = d.n; d.n = 0; n }",
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        match keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()) {
            Ok(actual) => {
                let equal = actual.to_bytes().unwrap() == reference.to_bytes().unwrap();
                eprintln!("{label}: complete artifact identity {equal}");
                if !equal {
                    gaps.push(label);
                }
            }
            Err(error) => {
                eprintln!("{label}: {error:?}");
                gaps.push(label);
            }
        }
    }
    assert!(gaps.is_empty(), "source initializer gaps {gaps:?}");
}

#[test]
fn initializer_values_execute_after_keleusma_serialization_and_decoding() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for (source, expected) in [
        (
            "private data d { n: Word = 5 } fn main() -> Word { d.n = d.n + 1; d.n }",
            6,
        ),
        (
            "private data d { n: Word = -1 } fn main() -> Word { let n = d.n; d.n = 0; n }",
            -1,
        ),
        (
            "private data d { n: Word = ((-5)) } fn main() -> Word { let n = d.n; d.n = 0; n }",
            -5,
        ),
        (
            "private data d { b: Byte = 255 } fn main() -> Word { let n = d.b as Word; d.b = 0 as Byte; n }",
            255,
        ),
        (
            "private data d { b: Byte = -0 } fn main() -> Word { let n = d.b as Word; d.b = 0 as Byte; n }",
            0,
        ),
        (
            "private data d { b: bool = true } fn main() -> Word { let n = if d.b { 7 } else { 0 }; d.b = false; n }",
            7,
        ),
        (
            "private data a { n: Word = 5 } shared data s { n: Byte } private data b { n: Word = 8 } fn main() -> Word { let n = a.n + b.n; a.n = 0; b.n = 0; n }",
            13,
        ),
        (
            "const data c { n: Word = 2 } private data d { n: Word = 5 } shared data s { n: Word } fn main() -> Word { d.n = d.n + c.n; d.n }",
            7,
        ),
    ] {
        let reference = keleusma::selfhost::compile_src(source);
        let pipeline =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        let bytes = keleusma::selfhost::wire_module_artifact_via_kel(&pipeline).unwrap();
        assert_eq!(bytes, reference.to_bytes().unwrap(), "{source}");
        drop(pipeline);
        let decoded = keleusma::selfhost::wire_decode_module_via_kel(&bytes).unwrap();
        for module in [decoded, reference] {
            let persistent = required_persistent_capacity_for(&module);
            let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
            arena.resize_persistent(persistent).unwrap();
            let mut vm = Vm::new(module, &arena).unwrap();
            let mut shared = vec![0; vm.shared_data_bytes()];
            assert!(
                matches!(vm.call_with_shared(&mut shared, &[]).unwrap(), VmState::Finished(Value::Int(value)) if value == expected),
                "{source}"
            );
        }
    }
}

#[test]
fn invalid_initializers_are_rejected_without_reference_masking() {
    for source in [
        "shared data d { n: Word = 5 } fn main() -> Word { 7 }",
        "private data d { b: Byte = 256 } fn main() -> Word { d.b = 0 as Byte; 7 }",
        "private data d { b: Byte = -1 } fn main() -> Word { d.b = 0 as Byte; 7 }",
        "private data d { b: bool = 1 } fn main() -> Word { d.b = false; 7 }",
        "private data d { n: Word = true } fn main() -> Word { d.n = 0; 7 }",
        "private data d { n: Word = (1,) } fn main() -> Word { d.n = 0; 7 }",
        "private data d { n: Word = 1 + 2 } fn main() -> Word { d.n = 0; 7 }",
        "private data d { n: Word = } fn main() -> Word { d.n = 0; 7 }",
        "private data d { xs: [Word; 1] = 5 } fn main() -> Word { d.xs[0] = 0; 7 }",
        "private data d { xs: [Word; 2] = [1,2] } fn main() -> Word { d.xs[0] = 0; 7 }",
        "private data d { n: Text = \"x\" } fn main() -> Word { d.n = \"y\"; 7 }",
    ] {
        let tokens = keleusma::lexer::tokenize(source).unwrap();
        let reference_accepts = keleusma::parser::parse(&tokens)
            .ok()
            .is_some_and(|ast| keleusma::compiler::compile(&ast).is_ok());
        assert!(!reference_accepts, "negative fixture {source}");
        assert!(
            !keleusma::selfhost::type_check_source_via_kel(source).unwrap(),
            "{source}"
        );
        assert!(
            matches!(
                keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()),
                Err(keleusma::selfhost::SelfHostError::PipelineRejected { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn later_declarations_do_not_retype_earlier_data_accesses() {
    let source = "private data d { n: Word } fn early() -> Word { d.n = 1; d.n } shared data s { n: Byte } fn main() -> Word { early() }";
    keleusma::selfhost::compile_src(source);
    assert!(keleusma::selfhost::type_check_source_via_kel(source).unwrap());
    // Relocating an already emitted body remains open. Its address mismatch is not a type error.
    assert!(matches!(
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()),
        Err(keleusma::selfhost::SelfHostError::Unsupported { .. })
    ));
}

#[test]
fn initializer_records_preserve_negative_one_and_slot_bases() {
    use keleusma::selfhost_host::{
        PARSE_DATA_FIELD_BASE_TAG, PARSE_PRIVATE_INIT_KIND_TAG, PARSE_PRIVATE_INIT_NEGATIVE_TAG,
    };
    let source = "private data d { n: Word = -1 } shared data s { n: Byte } fn main() -> Word { d.n = 0; 7 }";
    let (_, records) = keleusma::selfhost::parse_record_trace(source);
    let init: Vec<_> = records
        .iter()
        .filter(|&&(kind, _, _)| {
            kind == PARSE_PRIVATE_INIT_KIND_TAG || kind == PARSE_PRIVATE_INIT_NEGATIVE_TAG
        })
        .map(|&(kind, value, _)| (kind, value))
        .collect();
    assert_eq!(
        init,
        [
            (PARSE_PRIVATE_INIT_KIND_TAG, 3),
            (PARSE_PRIVATE_INIT_NEGATIVE_TAG, 0)
        ]
    );
    let bases: Vec<_> = records
        .iter()
        .filter(|&&(kind, _, _)| kind == PARSE_DATA_FIELD_BASE_TAG)
        .map(|&(_, value, _)| (value / 16777216, value % 16777216))
        .collect();
    assert_eq!(bases, [(0, 1), (1, 0)]);
}

#[test]
fn address_capacity_is_refused_before_expanding_a_data_table() {
    use keleusma::selfhost_host::{PARSE_DIAG_TAG_BASE, ParseRecordError};
    let limit = keleusma::bytecode::MAX_DATA_ADDR;
    assert_eq!(limit, 16777216);
    // Exercise parser transport without allocating millions of runtime slot descriptors.
    for count in [limit, limit + 1] {
        let source = format!("shared data d {{ n: [Byte; {count}] }} fn main() -> Word {{ 7 }}");
        let result = keleusma::selfhost::try_call_type_inputs_from_pipeline(&source);
        if count == limit {
            result.expect("the existing address bound remains admitted");
        } else {
            let error = result.expect_err("address overflow");
            assert!(!error.is_source_rejection());
            assert!(
                matches!(error, ParseRecordError::Diagnostic { code, .. } if code == PARSE_DIAG_TAG_BASE - 37)
            );
        }
    }
}

#[test]
fn data_field_scans_cover_the_advertised_table_capacity() {
    let source = include_str!("../src/selfhost/kel/parse.kel");
    let marker = "0..fields.field_count limit ";
    let mut scans = 0;
    for line in source.lines() {
        if let Some((_, tail)) = line.split_once(marker) {
            let limit: usize = tail.split_whitespace().next().unwrap().parse().unwrap();
            assert_eq!(limit, keleusma::selfhost_host::PARSE_FIELDS_CAP, "{line}");
            scans += 1;
        }
    }
    assert_eq!(scans, 4, "the declaration and lookup scan inventory moved");
}
