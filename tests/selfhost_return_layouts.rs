//! Return layouts come from source declarations, independently of declaration order.
#![cfg(feature = "self-host")]

#[test]
fn calls_preserve_declared_return_layouts() {
    let cases = [
        (
            "forward bool",
            "fn main() -> Word { let t = (yes(), 7); t.1 } fn yes() -> bool { true }",
        ),
        (
            "forward tuple",
            "fn main() -> Word { let t = pair(); t.1 } fn pair() -> (bool, Word) { (true, 7) }",
        ),
        (
            "forward nested tuple",
            "fn main() -> Word { let t = pair(); t.0 .1 } fn pair() -> ((bool, Word), Word) { ((true, 7), 8) }",
        ),
        (
            "tuple array",
            "fn make() -> [(bool, Word); 2] { [(true, 7), (false, 8)] } fn main() -> Word { let xs = make(); xs[0].1 }",
        ),
        (
            "forward tuple array",
            "fn main() -> Word { let xs = make(); xs[0].1 } fn make() -> [(bool, Word); 2] { [(true, 7), (false, 8)] }",
        ),
        (
            "forward scalar array member",
            "fn main() -> Word { let t = (make(), false); t.0[1] } fn make() -> [Word; 2] { [1, 7] }",
        ),
        (
            "direct tuple array indexing",
            "fn main() -> Word { make()[0].1 } fn make() -> [(bool, Word); 2] { [(true, 7), (false, 8)] }",
        ),
        (
            "forward struct",
            "struct P { x: Word } fn main() -> Word { let t = (make(), false); t.0.x } fn make() -> P { P { x: 7 } }",
        ),
        (
            "two calls in a tuple",
            "fn main() -> Word { let t = (yes(), pair()); t.1 .1 } fn yes() -> bool { true } fn pair() -> (bool, Word) { (true, 7) }",
        ),
        (
            "return layout survives later functions",
            "fn first() -> Word { let p = pair(); p.1 } fn next() -> Word { let p = pair(); p.1 } fn pair() -> (bool, Word) { (true, 7) } fn main() -> Word { next() }",
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        let verdict = keleusma::selfhost::type_check_source_via_kel(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host());
        let equal =
            matches!(&actual, Ok(m) if m.to_bytes().unwrap() == reference.to_bytes().unwrap());
        eprintln!("{label}: type {verdict:?}, artifact identity {equal}");
        if !matches!(verdict, Ok(true)) || !equal {
            eprintln!("{label}: {:?}", actual.err());
            gaps.push(label);
        }
    }
    assert!(gaps.is_empty(), "return layout gaps {gaps:?}");
}

#[test]
fn return_queries_match_across_token_feeds_and_announce_seeks() {
    use keleusma::selfhost_host::PARSE_TOKEN_SEEK_TAG;
    let source = "fn first() -> Word { let t = pair(); t.1 } fn main() -> Word { let t = pair(); t.1 } fn pair() -> (bool, Word) { (true, 7) }";
    let collecting = keleusma::selfhost::parse_record_trace(source);
    let fused = keleusma::selfhost::parse_record_trace_fused(source);
    assert_eq!(collecting, fused);
    let mut large_jumps = 0;
    let mut seeks = 0;
    for &(kind, target, at) in &fused.1 {
        if kind == PARSE_TOKEN_SEEK_TAG {
            assert!((target - at).abs() <= 1);
            seeks += 1;
        }
    }
    for pair in fused.1.windows(2) {
        if (pair[1].2 - pair[0].2).abs() > 1 {
            assert_eq!(
                pair[1].0, PARSE_TOKEN_SEEK_TAG,
                "unannounced token jump {pair:?}"
            );
            large_jumps += 1;
        }
    }
    assert!(seeks >= 5, "census rewind and two query round trips");
    assert!(
        large_jumps >= 3,
        "the fixture must cross the eight-token window"
    );
}

#[test]
fn forward_results_execute_after_keleusma_serialization() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for source in [
        "fn main() -> Word { let t = (yes(), 7); t.1 } fn yes() -> bool { true }",
        "fn main() -> Word { let t = pair(); t.0 .1 } fn pair() -> ((bool, Word), Word) { ((true, 7), 8) }",
        "fn main() -> Word { make()[0].1 } fn make() -> [(bool, Word); 2] { [(true, 7), (false, 8)] }",
        "struct P { x: Word } fn main() -> Word { let t = (make(), false); t.0.x } fn make() -> P { P { x: 7 } }",
    ] {
        let module =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        let bytes = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
        assert_eq!(
            bytes,
            keleusma::selfhost::compile_src(source).to_bytes().unwrap()
        );
        let module = keleusma::selfhost::wire_decode_module_via_kel(&bytes).unwrap();
        let persistent = required_persistent_capacity_for(&module);
        let mut arena = keleusma::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).unwrap();
        let mut vm = Vm::new(module, &arena).unwrap();
        assert!(
            matches!(vm.call(&[]).unwrap(), VmState::Finished(Value::Int(7))),
            "{source}"
        );
    }
}

#[test]
fn incorrect_forward_result_uses_are_rejected_independently() {
    for source in [
        "fn main() -> Word { let t = pair(); t.0 } fn pair() -> (bool, Word) { (true, 7) }",
        "fn main() -> Word { let n: Word = yes(); n } fn yes() -> bool { true }",
        "fn main() -> bool { let xs = make(); xs[0].1 } fn make() -> [(bool, Word); 2] { [(true, 7), (false, 8)] }",
        "fn main() -> Word { let t: (Word, Word) = pair(); t.1 } fn pair() -> (bool, Word) { (true, 7) }",
    ] {
        let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        assert!(keleusma::compiler::compile(&ast).is_err());
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

const GUARDED_LOCAL_SOURCE: &str = "fn f(x: Word) -> Word when x == 0 { let a = pair(); let b = pair(); a.1 + b.1 } fn f(y: Word) -> Word { let a = pair(); a.1 } fn pair() -> (bool, Word) { (true, 7) } fn main() -> Word { f(0) }";

#[test]
fn queries_cache_within_a_body_and_refresh_for_each_head() {
    use keleusma::selfhost_host::PARSE_TOKEN_SEEK_TAG;
    let source = GUARDED_LOCAL_SOURCE;
    keleusma::selfhost::compile_src(source);
    let (_, trace) = keleusma::selfhost::parse_record_trace_fused(source);
    assert_eq!(
        trace
            .iter()
            .filter(|(kind, _, _)| *kind == PARSE_TOKEN_SEEK_TAG)
            .count(),
        5,
        "one census rewind and one query round trip in each of two bodies"
    );
}

#[test]
fn guarded_head_local_bindings_preserve_artifacts() {
    let source = GUARDED_LOCAL_SOURCE;
    let reference = keleusma::selfhost::compile_src(source);
    assert!(keleusma::selfhost::type_check_source_via_kel(source).unwrap());
    let actual =
        keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host()).unwrap();
    assert_eq!(actual.to_bytes().unwrap(), reference.to_bytes().unwrap());
}

#[test]
fn distant_queries_and_quoted_declarations_preserve_all_compile_paths() {
    let mut source =
        "fn main() -> Word { let ignored = \"fn fake() -> bool { }\"; let t = pair(); t.1 }\n"
            .to_owned();
    for i in 0..80 {
        source.push_str(&format!("fn filler{i}() -> Word {{ {i} }}\n"));
    }
    source.push_str("fn pair() -> (bool, Word) { (true, 7) }");
    let reference = keleusma::selfhost::compile_src(&source);
    for module in [
        keleusma::selfhost::self_host_compile(&source),
        keleusma::selfhost::self_host_compile_fused(&source),
        keleusma::selfhost::self_host_compile_full(&source),
        keleusma::selfhost::self_host_compile_scratch(&source),
    ] {
        assert_eq!(module.to_bytes().unwrap(), reference.to_bytes().unwrap());
    }
    assert_eq!(
        keleusma::selfhost::parse_record_trace(&source),
        keleusma::selfhost::parse_record_trace_fused(&source)
    );
}
