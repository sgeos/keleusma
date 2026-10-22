//! For-limit outcome dispatch retains bindings, guard behavior and default traps.
#![cfg(feature = "self-host")]

#[test]
fn loop_outcome_programs_preserve_complete_artifacts() {
    let cases = [
        (
            "empty handlers",
            "fn main() -> Word { let i = 7; for i in 0..2 limit 2 {} on { ok => {}, limit => {} } i }",
        ),
        (
            "ok count",
            "private data d { n: Word } fn main() -> Word { for i in 0..3 limit 4 {} on { ok(c) => { d.n = c; }, limit => {} } d.n }",
        ),
        (
            "break index",
            "private data d { n: Word } fn main() -> Word { for i in 0..5 limit 5 { if i == 2 { break; } } on { ok => {}, break(bi) => { d.n = bi; }, limit => {} } d.n }",
        ),
        (
            "limit index",
            "private data d { n: Word } fn main() -> Word { for i in 0..8 limit 2 {} on { ok => {}, limit(si) => { d.n = si; } } d.n }",
        ),
        (
            "break falls through ok",
            "private data d { n: Word } fn main() -> Word { for i in 0..5 limit 5 { break; } on { ok(c) => { d.n = c; } } d.n }",
        ),
        (
            "guarded break",
            "private data d { n: Word } fn main() -> Word { for i in 0..5 limit 5 { if i == 2 { break; } } on { ok => {}, break(bi) when bi == 2 => { d.n = 7; }, limit => {} } d.n }",
        ),
        (
            "guarded limit",
            "private data d { n: Word } fn main() -> Word { for i in 0..8 limit 2 {} on { ok => {}, limit(si) when si == 2 => { d.n = 7; } } d.n }",
        ),
        (
            "reordered arms",
            "private data d { n: Word } fn main() -> Word { for i in 0..2 limit 2 {} on { limit(n) => { d.n = n; }, break(n) => { d.n = n; }, ok(n) => { d.n = n; } } d.n }",
        ),
        (
            "handler local",
            "private data d { n: Word } fn main() -> Word { for i in 0..2 limit 2 {} on { ok(c) => { let n = c + 5; d.n = n; }, limit => {} } d.n }",
        ),
        (
            "break from handler",
            "private data d { n: Word } fn main() -> Word { for i in 0..2 limit 2 { for j in 0..2 limit 2 {} on { ok => { break; } } } on { ok => {}, break(n) => { d.n = n; } } d.n }",
        ),
        (
            "outcomes in multiple heads",
            "private data d { n: Word } fn pick(n: Word) -> Word when n == 0 { for i in 0..2 limit 2 {} on { ok(c) => { d.n = c; } } d.n } fn pick(n: Word) -> Word { for i in 1..4 limit 4 {} on { ok(c) => { d.n = c; } } d.n } fn main() -> Word { pick(1) }",
        ),
        (
            "plain loops in multiple heads",
            "fn pick(n: Word) -> Word when n == 0 { for i in 0..2 limit 2 {} 7 } fn pick(n: Word) -> Word { for i in 1..4 limit 4 {} 8 } fn main() -> Word { pick(1) }",
        ),
        (
            "bare loops in multiple heads",
            "fn pick(n: Word) -> Word when n == 0 { for i in 0..2 {} 7 } fn pick(n: Word) -> Word { for i in 1..4 {} 8 } fn main() -> Word { pick(1) }",
        ),
        (
            "contextual on tail",
            "fn main() -> Word { let on = 7; for i in 0..2 limit 2 {} on }",
        ),
        (
            "empty on block",
            "fn main() -> Word { for i in 0..2 limit 2 {} on {} 7 }",
        ),
        (
            "wildcard binding",
            "fn main() -> Word { for i in 0..2 limit 2 {} on { ok(_) => {}, limit(_) => {} } 7 }",
        ),
        (
            "handler conditional",
            "private data d { n: Word } fn main() -> Word { for i in 0..2 limit 2 {} on { ok(c) => { if c == 2 { d.n = c; } }, limit => {} } d.n }",
        ),
        (
            "nested outcome",
            "private data d { n: Word } fn main() -> Word { for i in 0..2 limit 2 {} on { ok(c) => { for j in 0..c limit 2 {} on { ok(n) => { d.n = n; }, limit => {} } }, limit => {} } d.n }",
        ),
        (
            "loop in handler",
            "private data d { n: Word } fn main() -> Word { for i in 0..2 limit 2 {} on { ok(c) => { for j in 0..c limit 2 { d.n = d.n + j; } }, limit => {} } d.n }",
        ),
        (
            "reordered temporaries",
            "private data d { n: Word } fn main() -> Word { let shift = 1; for i in 0..2 limit 2 {} on { ok(c) => { for j in 0..c limit 2 { d.n = c lsr shift; } }, break(b) => { let x = b lsr shift; d.n = x; }, limit(n) => { d.n = n lsr shift; } } let x = d.n + shift; x }",
        ),
        (
            "guard with match",
            "private data d { n: Word } fn main() -> Word { for i in 0..4 limit 2 {} on { ok => {}, limit(n) when match n { 2 => true, _ => false } => { d.n = n; } } d.n }",
        ),
        (
            "guard with data read",
            "private data d { n: Word, yes: bool } fn main() -> Word { d.yes = true; for i in 0..4 limit 2 {} on { ok => {}, limit(n) when d.yes => { d.n = n; } } d.n }",
        ),
        (
            "outer scope restored",
            "fn main() -> Word { let n = 7; for i in 0..2 limit 2 { let n = false; } on { ok(n) => { let m = n + 1; }, limit(n) => {} } n }",
        ),
    ];
    let mut gaps = Vec::new();
    for (label, source) in cases {
        let reference = keleusma::selfhost::compile_src(source);
        let result =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host());
        let equal = matches!(&result, Ok(module) if module.to_bytes().unwrap() == reference.to_bytes().unwrap());
        eprintln!("{label}: artifact identity {equal}");
        if !equal {
            eprintln!("{label}: {:?}", result.err());
            gaps.push(label);
        }
    }
    assert!(gaps.is_empty(), "outcome gaps {gaps:?}");
}

/// Negative controls must be rejected before the reference compiler supplies a verdict.
#[test]
fn outcome_source_errors_are_independently_rejected() {
    use keleusma::selfhost::SelfHostError;
    for source in [
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok => {}, limit(n) when n => {} } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok(n) => { let b: bool = n; } } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok(n) => {} } n }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok => { let n = i; } } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 { let n = 1; } on { ok => { let x = n; } } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok => {} } i }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok => { break; } } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok when true => {} } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { limit => {} } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok => {}, ok => {} } 0 }",
        "fn main() -> Word { for i in 0..2 limit 2 {} on { ok(n)(m) => {} } 0 }",
    ] {
        let result =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host());
        assert!(
            matches!(result, Err(SelfHostError::PipelineRejected { .. })),
            "{source}: {result:?}"
        );
    }
}

/// Execute the Keleusma-serialized and decoded module under the normal safe verifier.
#[test]
fn decoded_outcome_dispatch_preserves_results_and_traps() {
    use keleusma::Arena;
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for (body, arms, expected) in [
        (
            "",
            "ok(c) => { d.n = c; }, limit(n) => { d.n = n + 5; }",
            Some(7),
        ),
        (
            "",
            "ok => {}, limit(n) when n == 2 => { d.n = 7; }",
            Some(7),
        ),
        ("", "ok => {}, limit(n) when n == 3 => { d.n = 7; }", None),
        ("", "ok => {}", None),
        (
            "if i == 1 { break; }",
            "ok => {}, break(n) when n == 1 => { d.n = 7; }",
            Some(7),
        ),
        (
            "if i == 1 { break; }",
            "ok => { d.n = 99; }, break(n) when n == 2 => { d.n = 7; }",
            Some(0),
        ),
        ("if i == 1 { break; }", "ok(c) => { d.n = c + 6; }", Some(7)),
    ] {
        let source = format!(
            "private data d {{ n: Word }} fn main() -> Word {{ d.n = 0; for i in 0..5 limit 2 {{ {body} }} on {{ {arms} }} d.n }}"
        );
        let module =
            keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
                .unwrap();
        let bytes = keleusma::selfhost::wire_module_artifact_via_kel(&module).unwrap();
        let decoded = keleusma::selfhost::wire_decode_module_via_kel(&bytes).unwrap();
        let need = required_persistent_capacity_for(&decoded);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(decoded, &arena).unwrap();
        let result = vm.call(&[]);
        match expected {
            Some(value) => assert!(
                matches!(result, Ok(VmState::Finished(Value::Int(n))) if n == value),
                "{result:?}"
            ),
            None => assert!(
                matches!(result, Err(keleusma::vm::VmError::LoopLimitExceeded)),
                "{result:?}"
            ),
        }
    }
}

/// The supported nesting bound accepts its last level and refuses the next as capacity.
#[test]
fn outcome_nesting_preserves_the_loop_capacity_boundary() {
    use keleusma::selfhost::SelfHostError;
    for depth in [8, 9] {
        let mut body = "d.n = 7;".to_owned();
        for _ in 0..depth {
            body = format!("for i in 0..1 limit 1 {{}} on {{ ok => {{ {body} }} }}");
        }
        let source = format!("private data d {{ n: Word }} fn main() -> Word {{ {body} d.n }}");
        let reference = keleusma::selfhost::compile_src(&source);
        let actual =
            keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host());
        if depth == 8 {
            assert_eq!(
                actual.unwrap().to_bytes().unwrap(),
                reference.to_bytes().unwrap()
            );
        } else {
            assert!(
                matches!(actual, Err(SelfHostError::TypeCheckUnavailable { .. })),
                "{actual:?}"
            );
        }
    }
}

/// The reference and pipeline must both execute the binding selected by source scope.
#[test]
fn outcome_handlers_use_outer_names_and_explicit_bindings() {
    use keleusma::Arena;
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    for source in [
        "private data d { n: Word } fn main() -> Word { let __for_end = 7; for i in 0..2 limit 2 {} on { ok => { d.n = __for_end; } } d.n }",
        "private data d { n: Word } fn main() -> Word { let __for_outcome = true; for i in 0..5 limit 2 {} on { ok => {}, limit(si) when __for_outcome => { d.n = 7; } } d.n }",
        "private data d { n: Word } fn main() -> Word { let i = 7; for i in 0..2 limit 2 {} on { ok => { d.n = i; } } d.n }",
        "private data d { n: Word } fn main() -> Word { let i = 7; for i in 0..5 limit 2 {} on { ok => {}, limit(si) when i == 7 => { d.n = i; } } d.n }",
        "private data d { n: Word } fn main() -> Word { let i = 7; for i in 0..5 limit 5 { break; } on { ok => {}, break(si) when i == 7 => { d.n = i; } } d.n }",
        "private data d { n: Word } fn main() -> Word { let i = true; for i in 0..5 limit 2 {} on { ok => {}, limit(si) when i => { d.n = 7; } } d.n }",
        "private data d { n: Word } fn main() -> Word { let i = [7]; for i in 0..2 limit 2 {} on { ok => { d.n = i[0]; } } d.n }",
        "private data d { n: Word } fn main() -> Word { let i = 99; for i in 0..2 limit 2 {} on { ok(i) => { d.n = i + 5; } } d.n }",
    ] {
        let reference = keleusma::selfhost::compile_src(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap();
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
        for module in [reference, actual] {
            let need = required_persistent_capacity_for(&module);
            let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
            arena.resize_persistent(need).unwrap();
            let mut vm = Vm::new(module, &arena).unwrap();
            assert!(
                matches!(vm.call(&[]).unwrap(), VmState::Finished(Value::Int(7))),
                "{source}"
            );
        }
    }
}
