//! Completed language-trap classifications remain distinct from unavailable proof.
#![cfg(all(feature = "compile", feature = "verify"))]
use keleusma::bytecode::{ArrayElem, ConstValue, EnumField, Module, Op, StructField, TupleField};
use keleusma::trap_analysis::{
    TRAP_OPCODE_CAP, TrapAnalysisError, TrapVerdict, UnsupportedTrapOperation, classify_traps,
};
use keleusma::value_layout::{CompositeKind, ScalarKind};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap()
}
fn agree(module: &Module) -> Result<TrapVerdict, TrapAnalysisError> {
    let reference = classify_traps(module);
    #[cfg(feature = "self-host")]
    {
        let actual = keleusma::selfhost::classify_traps_via_kel(module);
        match (&reference, &actual) {
            (
                Err(TrapAnalysisError::InvalidModule { .. }),
                Err(TrapAnalysisError::InvalidModule { .. }),
            ) => {}
            _ => assert_eq!(actual, reference),
        }
    }
    reference
}

#[test]
fn bare_and_fully_handled_source_classes_agree() {
    for (source, trapping) in [
        ("fn main() -> Word { 7 }", false),
        ("fn main(a: Word, b: Word) -> Word { a / b }", true),
        ("fn main(a: Word, b: Word) -> Word { a % b }", true),
        (
            "fn main(a: Word, b: Word) -> Word { a % b { ok(v) => v, zero_divisor(n) => n } }",
            false,
        ),
        (
            "fn main(a: Byte, b: Byte) -> Byte { a / b { ok(v) => v, zero_divisor(n) => n } }",
            false,
        ),
        ("fn main() -> Word { let a = [7]; a[2] }", true),
        (
            "fn main() -> Word { let a = [7]; a[2] { ok(v) => v, invalid_index(i) => i } }",
            false,
        ),
        (
            "shared data d { a: [Word; 2] } fn main() -> Word { d.a[2] { ok(v) => v, invalid_index(i) => i } }",
            false,
        ),
        (
            "shared data d { a: [Byte; 2] } fn main() -> Word { d.a[2] { ok(v) => v as Word, invalid_index(i) => i } }",
            false,
        ),
        (
            "fn main(a: Word, b: Word) -> Word { a % b { ok(v) => v, zero_divisor(n) when n == 1 => n } }",
            true,
        ),
        // Indexed writes. A bare write keeps its default trap, a fully handled write
        // removes it, and a write whose invalid_index arm is guarded keeps it.
        (
            "private data d { a: [Word; 2] } fn main(i: Word) -> Word { d.a[i] = 1; 0 }",
            true,
        ),
        (
            "private data d { a: [Word; 2] } fn main(i: Word) -> Word { d.a[i] = 1 { ok(o, n) => o + n, invalid_index(j) => j } }",
            false,
        ),
        (
            "private data d { a: [Word; 2] } fn main(i: Word) -> Word { d.a[i] = 1 { ok(_, n) => n, invalid_index(j) when j > 2 => j } }",
            true,
        ),
    ] {
        let module = compile(source);
        assert_eq!(
            matches!(agree(&module).unwrap(), TrapVerdict::MayTrap { .. }),
            trapping,
            "{source}"
        );
        #[cfg(feature = "self-host")]
        {
            let actual =
                keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                    .unwrap();
            assert_eq!(actual.to_bytes().unwrap(), module.to_bytes().unwrap());
            assert_eq!(agree(&actual).unwrap(), classify_traps(&module).unwrap());
        }
    }
}

/// Handled multi-index writes, which only the reference compiler emits. The
/// two analyses must still agree on its modules. The bare write keeps a
/// per-level trap, a fully handled write has none, and an `invalid_index` arm
/// with a literal pattern leaves the default trap reachable.
#[test]
fn multi_index_write_classes_agree() {
    for (source, trapping) in [
        (
            "private data d { m: [[Word; 3]; 2] } fn main(i: Word, j: Word) -> Word { d.m[i][j] = 1; 0 }",
            true,
        ),
        (
            "private data d { m: [[Word; 3]; 2] } fn main(i: Word, j: Word) -> Word { d.m[i][j] = 1 { ok(o, n) => o + n, invalid_index(k) => k } }",
            false,
        ),
        (
            "private data d { m: [[Word; 3]; 2] } fn main(i: Word, j: Word) -> Word { d.m[i][j] = 1 { ok(_, n) => n, invalid_index(0) => 0 } }",
            true,
        ),
    ] {
        let module = compile(source);
        assert_eq!(
            matches!(agree(&module).unwrap(), TrapVerdict::MayTrap { .. }),
            trapping,
            "{source}"
        );
    }
}

#[test]
fn unreachable_traps_and_chunks_are_scanned() {
    let mut m = compile("fn main() -> Word { 7 }");
    m.chunks[0].ops.push(Op::Trap(5));
    let first = keleusma::trap_analysis::TrapLocation {
        chunk: 0,
        instruction: m.chunks[0].ops.len() - 1,
    };
    assert_eq!(agree(&m).unwrap(), TrapVerdict::MayTrap { first });
    let m = compile("fn unused(a: Word, b: Word) -> Word { a / b } fn main() -> Word { 7 }");
    assert!(matches!(agree(&m).unwrap(), TrapVerdict::MayTrap { .. }));
}

/// Fixed division reports a zero divisor as an outcome since 2026-10-22, so
/// both analyses treat `FixedDiv` as clear. A bare source division keeps the
/// compiler's default trap, and a handled one with a catch-all
/// `zero_divisor` arm has none.
#[test]
fn fixed_division_classes_agree() {
    for (source, trapping) in [
        (
            "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b }",
            true,
        ),
        (
            "fn main(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b { ok(v) => v, zero_divisor(n) => n } }",
            false,
        ),
    ] {
        let module = compile(source);
        assert_eq!(
            matches!(agree(&module).unwrap(), TrapVerdict::MayTrap { .. }),
            trapping,
            "{source}"
        );
    }
    let mut m = compile("fn main() -> Word { 7 }");
    m.chunks[0].ops = vec![Op::PushImmediate(4), Op::Return, Op::FixedDiv(0)];
    assert_eq!(agree(&m).unwrap(), TrapVerdict::TrapFree);
}

#[test]
fn unsupported_forms_never_become_trap_free_or_possibly_trapping() {
    let cases = [
        (
            Op::GetIndex(ArrayElem::Boxed),
            UnsupportedTrapOperation::IndexedValue,
        ),
        (
            Op::GetIndex(ArrayElem::Flat {
                kind: ScalarKind::Bool,
            }),
            UnsupportedTrapOperation::IndexedValue,
        ),
        (
            Op::GetIndex(ArrayElem::FlatNested {
                size: 8,
                variant: CompositeKind::Tuple,
            }),
            UnsupportedTrapOperation::IndexedValue,
        ),
        (
            Op::GetField(StructField::Boxed { name_const: 0 }),
            UnsupportedTrapOperation::BoxedField,
        ),
        (
            Op::GetTupleField(TupleField::Boxed { index: 0 }),
            UnsupportedTrapOperation::BoxedField,
        ),
        (
            Op::GetEnumField(EnumField::Boxed { index: 0 }),
            UnsupportedTrapOperation::BoxedField,
        ),
        (
            Op::CallVerifiedNative(0, 0),
            UnsupportedTrapOperation::NativeCall,
        ),
        (
            Op::CallExternalNative(0, 128),
            UnsupportedTrapOperation::NativeCall,
        ),
    ];
    for (op, expected) in cases {
        for trap in [false, true] {
            let mut m = compile("fn main() -> Word { 0 }");
            m.native_names = vec!["host".into()];
            m.chunks[0].constants = vec![ConstValue::StaticStr("field".into())];
            m.chunks[0].ops = vec![Op::PushImmediate(4), Op::Return];
            if trap {
                m.chunks[0].ops.push(Op::Trap(5));
            }
            let instruction = m.chunks[0].ops.len();
            m.chunks[0].ops.push(op);
            assert_eq!(
                agree(&m),
                Err(TrapAnalysisError::Unsupported {
                    location: keleusma::trap_analysis::TrapLocation {
                        chunk: 0,
                        instruction
                    },
                    operation: expected
                })
            );
        }
    }
}

#[test]
fn malformed_modules_are_not_completed_verdicts() {
    for ops in [
        vec![Op::PopN(1), Op::Return],
        vec![Op::Trap(65535)],
        vec![Op::PushImmediate(255), Op::Return],
        vec![Op::PushImmediate(1), Op::If(0), Op::Return],
    ] {
        let mut m = compile("fn main() -> Word { 0 }");
        m.chunks[0].ops = ops;
        assert!(matches!(
            agree(&m),
            Err(TrapAnalysisError::InvalidModule { .. })
        ));
    }
    for width in [0, 2, 7, 255] {
        let mut m = compile("fn main() -> Word { 0 }");
        m.word_bits_log2 = width;
        assert!(matches!(
            agree(&m),
            Err(TrapAnalysisError::InvalidModule { .. })
        ));
    }
}

#[test]
fn exact_opcode_capacity_is_admitted_and_the_next_size_is_unavailable() {
    let mut m = compile("fn main() -> Word { 0 }");
    m.chunks[0].ops = (0..TRAP_OPCODE_CAP / 2 - 1)
        .flat_map(|_| [Op::PushImmediate(4), Op::PopN(1)])
        .chain([Op::PushImmediate(4), Op::Return])
        .collect();
    assert_eq!(m.chunks[0].ops.len(), TRAP_OPCODE_CAP);
    assert_eq!(agree(&m).unwrap(), TrapVerdict::TrapFree);
    m.chunks[0].ops.insert(0, Op::PushImmediate(4));
    assert_eq!(
        agree(&m),
        Err(TrapAnalysisError::Capacity {
            chunk: 0,
            limit: TRAP_OPCODE_CAP,
            actual: TRAP_OPCODE_CAP + 1
        })
    );
}

#[test]
fn total_handbuilt_arithmetic_is_classified_without_source_trust() {
    for op in [Op::Div, Op::Mod, Op::CheckedDiv(0), Op::CheckedMod] {
        let mut m = compile("fn main() -> Word { 0 }");
        m.chunks[0].ops = vec![
            Op::PushImmediate(11),
            Op::PushImmediate(4),
            op,
            Op::PopN(2),
            Op::Return,
        ];
        assert_eq!(agree(&m).unwrap(), TrapVerdict::TrapFree);
        let arena = keleusma::Arena::with_capacity(16384);
        let mut vm = keleusma::vm::Vm::new(m, &arena).unwrap();
        assert!(matches!(
            vm.call(&[]).unwrap(),
            keleusma::vm::VmState::Finished(keleusma::bytecode::Value::Int(7))
        ));
    }
}

#[cfg(feature = "self-host")]
#[test]
fn every_current_stage_has_a_completed_matching_classification() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/selfhost/kel");
    let mut paths: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "kel"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 14, "inventory every actual stage");
    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let module = compile(&source);
        let reference =
            classify_traps(&module).unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
        let actual = keleusma::selfhost::classify_traps_via_kel(&module)
            .unwrap_or_else(|e| panic!("{}: {e:?}", path.display()));
        assert_eq!(actual, reference, "{}", path.display());
        assert!(
            matches!(actual, TrapVerdict::MayTrap { .. }),
            "stage operations retain explicit failure dispatch"
        );
        eprintln!("{}: {actual:?}", path.display());
    }
}
