//! The type of a `yield` expression is the type the host resumes with.
//!
//! The operator ruled on 2026-10-31 that the type of `let r = yield v` is
//! inferred when it can be and that the program is refused at compile time
//! when it cannot. Before the repair the type checker gave every `yield`
//! expression a fresh type variable, so `let r = yield v; r.y` compiled with
//! `r` of no known type and faulted when the field was read.
//!
//! The resume value is the loop's next input. `Vm::resume` checks it against
//! the type of the entry loop's first parameter and stores it in that
//! parameter's slot, so that type is the yield's type inside the loop and
//! inside every `yield` function the loop calls. A loop with no parameter
//! checks nothing, so its yields have no inferable type, and a use that does
//! not fix the type is refused with a request for an annotation.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState};

fn parse(source: &str) -> keleusma::ast::Program {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::parser::parse(&tokens).expect("parse")
}

fn compile(source: &str) -> Result<keleusma::bytecode::Module, String> {
    keleusma::compiler::compile(&parse(source)).map_err(|e| e.message)
}

/// Call `main` with `arg`, then resume once with each of `resumes`, returning
/// every state reached, so a fault at any step is visible.
fn drive(source: &str, arg: Option<Value>, resumes: &[Value]) -> Vec<String> {
    let module = compile(source).unwrap_or_else(|e| panic!("{source}: {e}"));
    let need = keleusma::vm::required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(1 << 20);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(module, &arena).unwrap_or_else(|e| panic!("{source}: {e:?}"));
    let args: Vec<Value> = arg.into_iter().collect();
    let mut states = vec![format!("{:?}", vm.call(&args))];
    for r in resumes {
        states.push(format!("{:?}", vm.resume(r.clone())));
    }
    states
}

fn point(x: i64, y: i64) -> Value {
    Value::struct_value(
        "P".into(),
        vec![("x".into(), Value::Int(x)), ("y".into(), Value::Int(y))],
    )
}

fn yielded(n: i64) -> String {
    format!(
        "{:?}",
        Ok::<VmState, keleusma::vm::VmError>(VmState::Yielded(Value::Int(n)))
    )
}

#[test]
fn an_unannotated_resume_value_takes_the_loop_parameter_type() {
    let source = "struct P { x: Word, y: Word }\n\
                  loop main(p: P) -> Word { let r = yield p.x; yield r.y }";
    assert_eq!(
        drive(source, Some(point(1, 2)), &[point(3, 4)]),
        vec![yielded(1), yielded(4)],
        "{source}"
    );
    let source = "loop main(t: (Word, Word)) -> Word { let r = yield t.0; yield r.1 }";
    assert_eq!(
        drive(
            source,
            Some(Value::tuple(vec![Value::Int(5), Value::Int(6)])),
            &[Value::tuple(vec![Value::Int(7), Value::Int(8)])]
        ),
        vec![yielded(5), yielded(8)],
        "{source}"
    );
}

#[test]
fn a_yield_function_resumes_with_the_entry_loop_parameter_type() {
    let source = "struct P { x: Word, y: Word }\n\
                  yield step(p: P) -> Word { let r = yield p.x; r.y }\n\
                  loop main(p: P) -> Word { let a = step(p); yield a }";
    assert_eq!(
        drive(source, Some(point(1, 2)), &[point(3, 4)]),
        vec![yielded(1), yielded(4)],
        "{source}"
    );
}

#[test]
fn a_use_that_disagrees_with_the_resume_type_is_refused() {
    for (source, expected) in [
        // The annotation names a type other than the resume type.
        (
            "struct P { x: Word, y: Word }\n\
             loop main(p: P) -> Word { let r: Word = yield p.x; yield r }",
            "let binding declared as Word but value has type P",
        ),
        // The resumed `P` would be yielded as a `Word`.
        (
            "struct P { x: Word, y: Word }\n\
             loop main(p: P) -> Word { let r = yield p.x; yield r }",
            "yielded value has type P but the function returns Word",
        ),
    ] {
        let err = compile(source).expect_err(source);
        assert!(err.contains(expected), "{source}: {err}");
    }
}

#[test]
fn a_resume_value_of_no_inferable_type_is_refused_with_a_request_to_annotate() {
    for source in [
        "struct P { x: Word, y: Word }\n\
         loop main() -> Word { let r = yield 1; yield r.x }",
        // Through a second binding, after the read.
        "struct P { x: Word, y: Word }\n\
         loop main() -> Word { let r = yield 1; let s = r; let w = s.y; yield w }",
        "loop main() -> Word { let (a, b) = yield 1; yield a }",
        "struct P { x: Word, y: Word }\n\
         loop main() -> Word { let r = yield 1; yield match r { P { x, y } => x } }",
        // A `yield` function under a `main` that is not a loop.
        "struct P { x: Word, y: Word }\n\
         yield step(v: Word) -> Word { let r = yield v; r.y }\n\
         yield main(v: Word) -> Word { step(v) }",
    ] {
        let err = compile(source).expect_err(source);
        assert!(
            err.contains("cannot infer the type of the value `yield` resumes with")
                && err.contains("annotate"),
            "{source}: {err}"
        );
    }
}

#[test]
fn a_resume_value_that_is_discarded_or_fixed_by_its_use_is_admitted() {
    for (source, expected) in [
        // Discarded, as a statement, a wildcard binding, and the loop's tail.
        (
            "loop main() -> Word { yield 1; let _ = yield 2; yield 3 }",
            1,
        ),
        // Used without reading into it, as the operand of an arithmetic
        // operator and as an unread binding.
        ("loop main() -> Word { let r = yield 1; yield r + 1 }", 1),
        (
            "loop main() -> Word { let r = yield 1; let s = r; yield 2 }",
            1,
        ),
        // Fixed by an annotation.
        ("loop main() -> Word { let r: Word = yield 1; yield r }", 1),
        // The parameter type and the return type agree.
        ("loop main(e: Word) -> Word { let n = yield e + 1; n }", 10),
    ] {
        let arg = source.contains("e: Word").then_some(Value::Int(9));
        assert_eq!(drive(source, arg, &[])[0], yielded(expected), "{source}");
    }
    // The loop's tail yield returns the resumed `P`, which the loop discards,
    // so it need not have the return type. The resume completes the
    // iteration, which reaches the loop's reset.
    let source = "struct P { x: Word, y: Word }\nloop main(p: P) -> Word { yield p.x }";
    assert_eq!(
        drive(source, Some(point(1, 2)), &[point(3, 4)]),
        vec![yielded(1), String::from("Ok(Reset)")],
        "{source}"
    );
}
