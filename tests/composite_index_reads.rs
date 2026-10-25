//! Composite element reads under the operator ruling of 2026-10-24.
//!
//! `GetIndex` on a nested composite element, and on a boxed element, now has
//! the total contract that flat Word and Byte reads already had. An invalid
//! index pushes a placeholder, then the index, the four bound limbs and the
//! flag, and performs no out-of-bounds access. The nested placeholder is
//! zero-filled at the element's size, so even hand-built code that reads it
//! stays inside its body. The boxed placeholder is `Unit`, because a boxed
//! element has no flat layout. A bare read reaches the explicit
//! `Trap(IndexOutOfBounds)`, and a handled read dispatches the outcome
//! without any arm observing the placeholder.
//!
//! Composite elements of data-segment arrays already read through the total
//! `GetDataIndexed`, so they are covered here only as controls.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{ArrayElem, Module, Op, TrapKind, Value};
use keleusma::trap_analysis::{TrapAnalysisError, TrapVerdict, classify_traps};
use keleusma::vm::{Vm, VmError, VmState};

fn compile(source: &str) -> Module {
    let ast = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
    keleusma::compiler::compile(&ast).unwrap_or_else(|e| panic!("{source}: {}", e.message))
}

fn agree(module: &Module) -> Result<TrapVerdict, TrapAnalysisError> {
    let reference = classify_traps(module);
    #[cfg(feature = "self-host")]
    assert_eq!(
        keleusma::selfhost::classify_traps_via_kel(module),
        reference
    );
    reference
}

fn run(module: Module, args: &[Value]) -> Result<Value, VmError> {
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(1 << 21);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap();
    let mut shared = vec![0; vm.shared_data_bytes()];
    match vm.call_with_shared(&mut shared, args)? {
        VmState::Finished(v) => Ok(v),
        other => panic!("expected a finished call, got {other:?}"),
    }
}

fn nested_reads(module: &Module) -> usize {
    module
        .chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .filter(|op| matches!(op, Op::GetIndex(ArrayElem::FlatNested { .. })))
        .count()
}

const POINTS: &str = "struct P { a: Word, b: Word } ";

fn points(main: &str) -> String {
    format!("{POINTS}{main}")
}

#[test]
fn a_bare_composite_read_reaches_the_explicit_bounds_trap() {
    let source = points(
        "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }, P { a: 3, b: 4 }]; ps[i].b }",
    );
    let module = compile(&source);
    assert_eq!(nested_reads(&module), 1);
    let ops = &module.chunks.iter().find(|c| c.name == "main").unwrap().ops;
    let at = ops
        .iter()
        .position(|op| matches!(op, Op::GetIndex(ArrayElem::FlatNested { .. })))
        .unwrap();
    assert_eq!(
        ops[at + 4],
        Op::Trap(TrapKind::IndexOutOfBounds.code()),
        "{ops:?}"
    );
    assert_eq!(ops[at + 6], Op::PopN(5));
    assert!(matches!(agree(&module), Ok(TrapVerdict::MayTrap { .. })));
    assert_eq!(
        run(module.clone(), &[Value::Int(1)]).unwrap(),
        Value::Int(4)
    );
    for (index, expected) in [(2, 2), (-1, -1), (i64::MAX, i64::MAX)] {
        match run(module.clone(), &[Value::Int(index)]) {
            Err(VmError::IndexOutOfBounds(i, 2)) => assert_eq!(i, expected),
            other => panic!("index {index}: {other:?}"),
        }
    }
}

#[test]
fn a_handled_composite_read_dispatches_without_a_placeholder_reaching_an_arm() {
    let source = points(
        "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }, P { a: 3, b: 4 }]; \
         ps[i] { ok(p) => p.a * 10 + p.b, invalid_index(j) => j * 100 } }",
    );
    let module = compile(&source);
    assert!(
        !module
            .chunks
            .iter()
            .flat_map(|c| c.ops.iter())
            .any(|op| matches!(op, Op::Trap(_))),
        "a covering invalid_index arm leaves no fallback trap"
    );
    assert_eq!(agree(&module), Ok(TrapVerdict::TrapFree));
    assert_eq!(
        run(module.clone(), &[Value::Int(0)]).unwrap(),
        Value::Int(12)
    );
    assert_eq!(
        run(module.clone(), &[Value::Int(1)]).unwrap(),
        Value::Int(34)
    );
    assert_eq!(
        run(module.clone(), &[Value::Int(7)]).unwrap(),
        Value::Int(700)
    );
    assert_eq!(run(module, &[Value::Int(-3)]).unwrap(), Value::Int(-300));
}

#[test]
fn a_guarded_composite_handler_keeps_the_fallback_trap() {
    let source = points(
        "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }]; \
         ps[i] { ok(p) => p.b, invalid_index(j) when j < 0 => 0 } }",
    );
    let module = compile(&source);
    assert!(matches!(agree(&module), Ok(TrapVerdict::MayTrap { .. })));
    assert_eq!(
        run(module.clone(), &[Value::Int(-4)]).unwrap(),
        Value::Int(0)
    );
    assert!(matches!(
        run(module, &[Value::Int(4)]),
        Err(VmError::IndexOutOfBounds(4, 1))
    ));
}

#[test]
fn a_nested_array_reports_the_failing_level() {
    let source = "fn main(i: Word, j: Word) -> Word { let m = [[1, 2, 3], [4, 5, 6]]; m[i][j] }";
    let module = compile(source);
    assert_eq!(nested_reads(&module), 1);
    let call = |i: i64, j: i64| run(module.clone(), &[Value::Int(i), Value::Int(j)]);
    assert_eq!(call(1, 2).unwrap(), Value::Int(6));
    assert!(matches!(call(2, 0), Err(VmError::IndexOutOfBounds(2, 2))));
    assert!(matches!(call(1, 3), Err(VmError::IndexOutOfBounds(3, 3))));
    assert!(matches!(agree(&module), Ok(TrapVerdict::MayTrap { .. })));
}

#[test]
fn the_nested_placeholder_is_zero_filled_at_the_element_size() {
    // Strip the bare read's trap tail and drop only the suffix, so this
    // hand-built module reads a field of the placeholder itself. A
    // zero-length placeholder would fault inside the field read, which the
    // trap analysis classifies as clear. The zero-filled one reads zero.
    let source = points(
        "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }, P { a: 3, b: 4 }]; ps[i].b }",
    );
    let mut module = compile(&source);
    let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
    let at = main
        .ops
        .iter()
        .position(|op| matches!(op, Op::GetIndex(ArrayElem::FlatNested { .. })))
        .unwrap();
    main.ops.splice(at + 1..at + 7, [Op::PopN(6)]);
    assert!(keleusma::verify::verify(&module).is_ok());
    assert_eq!(agree(&module), Ok(TrapVerdict::TrapFree));
    assert_eq!(
        run(module.clone(), &[Value::Int(9)]).unwrap(),
        Value::Int(0)
    );
    assert_eq!(run(module, &[Value::Int(1)]).unwrap(), Value::Int(4));
}

#[test]
fn the_placeholder_allocation_is_charged_to_the_memory_bound() {
    let module = compile(&points(
        "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }]; ps[i].b }",
    ));
    let main = module.chunks.iter().find(|c| c.name == "main").unwrap();
    let op = main
        .ops
        .iter()
        .find(|op| matches!(op, Op::GetIndex(ArrayElem::FlatNested { .. })))
        .unwrap();
    let Op::GetIndex(ArrayElem::FlatNested { size, .. }) = op else {
        unreachable!()
    };
    assert_eq!(op.heap_alloc(main), u32::from(*size));
    assert!(*size > 0);
}

/// Rewrite the flat array construction and read of a compiled program into
/// their boxed forms, which current compilers emit only for element types
/// without a flat layout.
fn boxed(source: &str) -> Module {
    use keleusma::bytecode::NewCompositeOperand;
    use keleusma::value_layout::CompositeKind;
    let mut module = compile(source);
    let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
    let mut rewritten = (0, 0);
    for op in main.ops.iter_mut() {
        match op {
            Op::NewComposite(NewCompositeOperand::Flat {
                kind: CompositeKind::Array,
                count,
                ..
            }) => {
                *op = Op::NewComposite(NewCompositeOperand::Boxed {
                    kind: CompositeKind::Array,
                    count: *count,
                    meta: 0,
                });
                rewritten.0 += 1;
            }
            Op::GetIndex(ArrayElem::Flat { .. }) => {
                *op = Op::GetIndex(ArrayElem::Boxed);
                rewritten.1 += 1;
            }
            _ => {}
        }
    }
    assert_eq!(rewritten, (1, 1), "{source}");
    module
}

#[test]
fn a_boxed_element_read_is_total() {
    let module = boxed("fn main(i: Word) -> Word { let xs = [10, 20]; xs[i] }");
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(matches!(agree(&module), Ok(TrapVerdict::MayTrap { .. })));
    assert_eq!(
        run(module.clone(), &[Value::Int(1)]).unwrap(),
        Value::Int(20)
    );
    assert!(matches!(
        run(module.clone(), &[Value::Int(2)]),
        Err(VmError::IndexOutOfBounds(2, 2))
    ));
    assert!(matches!(
        run(module, &[Value::Int(-1)]),
        Err(VmError::IndexOutOfBounds(-1, 2))
    ));
    let handled = boxed(
        "fn main(i: Word) -> Word { let xs = [10, 20]; xs[i] { ok(v) => v, invalid_index(j) => j * 100 } }",
    );
    assert!(keleusma::verify::verify(&handled).is_ok());
    assert_eq!(agree(&handled), Ok(TrapVerdict::TrapFree));
    assert_eq!(
        run(handled.clone(), &[Value::Int(0)]).unwrap(),
        Value::Int(10)
    );
    assert_eq!(run(handled, &[Value::Int(9)]).unwrap(), Value::Int(900));
}

#[test]
fn other_flat_scalar_elements_stay_outside_the_proof() {
    // Bool elements keep the faulting contract. The analysis reports them as
    // unsupported rather than as trap-free or possibly trapping.
    let module = compile("fn main(i: Word) -> bool { let xs = [true, false]; xs[i] }");
    assert!(matches!(
        agree(&module),
        Err(TrapAnalysisError::Unsupported { .. })
    ));
}

#[cfg(feature = "self-host")]
#[test]
fn the_keleusma_typed_pass_agrees_on_composite_reads() {
    for source in [
        points(
            "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }, P { a: 3, b: 4 }]; ps[i].b }",
        ),
        points(
            "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }]; ps[i] { ok(p) => p.b, invalid_index(j) => j } }",
        ),
        String::from(
            "fn main(i: Word, j: Word) -> Word { let m = [[1, 2, 3], [4, 5, 6]]; m[i][j] }",
        ),
    ] {
        let module = compile(&source);
        assert_eq!(
            keleusma::selfhost::typed_reject_module_via_kel(&module),
            keleusma::verify::verify(&module).is_err(),
            "{source}"
        );
        assert!(keleusma::verify::verify(&module).is_ok(), "{source}");
    }
}

#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_matches_bare_composite_reads() {
    for source in [
        points(
            "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }, P { a: 3, b: 4 }]; ps[i].b }",
        ),
        points(
            "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }, P { a: 3, b: 4 }]; let p = ps[i]; p.a + p.b }",
        ),
    ] {
        let reference = compile(&source);
        let actual =
            keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
                .unwrap_or_else(|e| panic!("{source}: {e:?}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
}

/// A handled composite read is outside the self-hosted subset. Its parser
/// opens a handled read only on a Word or Byte element, and mirroring the
/// composite form would need parse-stage names the stage does not have. It
/// refuses rather than emitting a module, as for handled multi-index writes.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_refuses_a_handled_composite_read() {
    let source = points(
        "fn main(i: Word) -> Word { let ps = [P { a: 1, b: 2 }]; ps[i] { ok(p) => p.b, invalid_index(j) => j } }",
    );
    compile(&source);
    assert!(
        keleusma::selfhost::self_hosted_compile(&source, &keleusma::target::Target::host())
            .is_err()
    );
}

/// Equality over one-element arrays. With two or more elements the element
/// index constant `1` already exists when the bounds tail interns its `1`, so
/// only one-element arrays expose where each compiler first interns it. A
/// one-element nested array diverged on the constant-pool index until the
/// self-hosted equality emitters interned it before the result constants.
#[cfg(feature = "self-host")]
#[test]
fn one_element_array_equality_matches_the_self_hosted_compiler() {
    for source in [
        "fn f(a: [Word; 1], b: [Word; 1]) -> bool { a == b }",
        "fn f(a: [Byte; 1], b: [Byte; 1]) -> bool { a != b }",
        "fn f(a: [bool; 1], b: [bool; 1]) -> bool { a == b }",
        "struct P { x: Word }\nfn f(a: [P; 1], b: [P; 1]) -> bool { a == b }",
        "struct P { x: Word }\nfn f(a: [(P, Word); 1], b: [(P, Word); 1]) -> bool { a == b }",
        "fn f(a: [[Word; 2]; 1], b: [[Word; 2]; 1]) -> bool { a == b }",
        "fn f(a: [[Word; 1]; 1], b: [[Word; 1]; 1]) -> bool { a == b }",
        "enum E { A(Word), B }\nfn f(a: [E; 1], b: [E; 1]) -> bool { a == b }",
    ] {
        let reference = compile(source);
        let actual =
            keleusma::selfhost::self_hosted_compile(source, &keleusma::target::Target::host())
                .unwrap_or_else(|e| panic!("{source}: {e:?}"));
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
}
