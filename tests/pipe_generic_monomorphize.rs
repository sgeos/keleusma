//! A pipe into a generic function is monomorphized as the plain call is.
//!
//! The V0.3.X line found that `t |> id()` compiled the generic `id` as one
//! chunk with `Opaque` parameter and return kinds, whereas `id(t)` produced the
//! specialization `id__Word`. The operator ruled on 2026-10-31 that the pipe
//! is to monomorphize exactly as the plain call does. The reference compiler
//! now rewrites a pipe whose target is a generic function into the call it
//! stands for before type checking, so both forms reach monomorphization as
//! the same call. A pipe into a non-generic function is left as it was.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::bytecode::Module;

fn compile(source: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    let program = keleusma::parser::parse(&tokens).expect("parse");
    keleusma::compiler::compile(&program).unwrap_or_else(|e| panic!("{source}: {}", e.message))
}

fn chunk_names(module: &Module) -> Vec<String> {
    module.chunks.iter().map(|c| c.name.clone()).collect()
}

#[test]
fn a_pipe_into_a_generic_function_compiles_to_the_plain_call_module() {
    for (piped, called) in [
        (
            "fn id<T>(x: T) -> T { x }\nloop main(t: Word) -> Word { let v: Word = t |> id(); let r = yield v; yield r }",
            "fn id<T>(x: T) -> T { x }\nloop main(t: Word) -> Word { let v: Word = id(t); let r = yield v; yield r }",
        ),
        (
            "fn id<T>(x: T) -> T { x }\nfn main() -> Word { 1 |> id() }",
            "fn id<T>(x: T) -> T { x }\nfn main() -> Word { id(1) }",
        ),
        // The placeholder form, with the piped value in second position.
        (
            "fn second<T>(a: Word, b: T) -> T { b }\nfn main() -> Word { 7 |> second(1, _) }",
            "fn second<T>(a: Word, b: T) -> T { b }\nfn main() -> Word { second(1, 7) }",
        ),
        // Two instances in one function, which a pipe typed against the
        // generic's own parameter variable could not both satisfy.
        (
            "fn id<T>(x: T) -> T { x }\nfn main() -> Word { let b = true |> id(); if b { 1 |> id() } else { 2 } }",
            "fn id<T>(x: T) -> T { x }\nfn main() -> Word { let b = id(true); if b { id(1) } else { 2 } }",
        ),
        // A generic pipe whose operand is itself a pipe into a non-generic
        // function. The rewrite makes it `id(x |> d())`, and that argument is
        // typed by `d`'s return as the call `d(x)` is. Found 2026-10-31 by the
        // self-hosted differential, where the reference kept `id` generic.
        (
            "fn id<T>(x: T) -> T { x }\nfn d(x: Word) -> Word { x + x }\nfn main(x: Word) -> Word { x |> d() |> id() |> d() }",
            "fn id<T>(x: T) -> T { x }\nfn d(x: Word) -> Word { x + x }\nfn main(x: Word) -> Word { d(id(d(x))) }",
        ),
        (
            "fn id<T>(x: T) -> T { x }\nfn d(x: Word) -> Word { x + x }\nfn main(x: Word) -> Word { id(x |> d()) }",
            "fn id<T>(x: T) -> T { x }\nfn d(x: Word) -> Word { x + x }\nfn main(x: Word) -> Word { id(d(x)) }",
        ),
    ] {
        let piped_module = compile(piped);
        let called_module = compile(called);
        assert!(
            chunk_names(&piped_module)
                .iter()
                .any(|n| n.starts_with("id__") || n.starts_with("second__")),
            "{piped}: {:?}",
            chunk_names(&piped_module)
        );
        assert_eq!(
            piped_module.to_bytes().unwrap(),
            called_module.to_bytes().unwrap(),
            "{piped}"
        );
    }
}

/// A generic call whose argument is an unannotated resume binding specializes
/// as the annotated form does.
///
/// The V0.3.X line found that `let r = yield v; yield r |> id()` failed with
/// `undefined function `id`` once another instance of `id` existed, and kept
/// the generic chunk `id` when none did, while `let r: Word = yield v`
/// produced `id__Word`. The type checker infers the resume type from
/// `loop main`'s first parameter, but monomorphization did not see it. The
/// operator ruled on 2026-10-31 to infer where possible and to refuse where
/// impossible.
#[test]
fn a_generic_call_on_an_unannotated_resume_binding_is_specialized() {
    const ID: &str = "fn id<T>(x: T) -> T { x }\n";
    for (inferred, annotated) in [
        (
            format!(
                "{ID}loop main(t: Word) -> Word {{ let v: Word = t |> id(); let r = yield v; yield r |> id() }}"
            ),
            format!(
                "{ID}loop main(t: Word) -> Word {{ let v: Word = t |> id(); let r: Word = yield v; yield r |> id() }}"
            ),
        ),
        (
            format!(
                "{ID}loop main(t: Word) -> Word {{ let v: Word = id(t); let r = yield v; yield id(r) }}"
            ),
            format!(
                "{ID}loop main(t: Word) -> Word {{ let v: Word = id(t); let r: Word = yield v; yield id(r) }}"
            ),
        ),
        (
            format!("{ID}loop main(t: Word) -> Word {{ let r = yield 1; yield r |> id() }}"),
            format!("{ID}loop main(t: Word) -> Word {{ let r: Word = yield 1; yield r |> id() }}"),
        ),
        // A `yield` function called from the loop resumes through the same
        // entry check, so its resume binding has the same type.
        (
            format!(
                "{ID}yield g() -> Word {{ let r = yield 1; id(r) }}\nloop main(t: Word) -> Word {{ let a = g(); yield a }}"
            ),
            format!(
                "{ID}yield g() -> Word {{ let r: Word = yield 1; id(r) }}\nloop main(t: Word) -> Word {{ let a = g(); yield a }}"
            ),
        ),
    ] {
        let inferred_module = compile(&inferred);
        let annotated_module = compile(&annotated);
        let names = chunk_names(&inferred_module);
        assert!(
            names.iter().any(|n| n == "id__Word") && !names.iter().any(|n| n == "id"),
            "{inferred}: {names:?}"
        );
        assert_eq!(
            inferred_module.to_bytes().unwrap(),
            annotated_module.to_bytes().unwrap(),
            "{inferred}"
        );
    }
}

/// With no `loop main` parameter and no use that fixes it, the resume type
/// cannot be inferred, and the type checker's request for an annotation
/// applies rather than a generic chunk or an undefined function after
/// monomorphization. A plain call and a pipe are both refused, with and
/// without a second instance of the template.
#[test]
fn an_uninferable_resume_binding_in_a_generic_call_asks_for_an_annotation() {
    for source in [
        "fn id<T>(x: T) -> T { x }\nloop main() -> Word { let r = yield 1; let s = id(r); yield 0 }",
        "fn id<T>(x: T) -> T { x }\nloop main() -> Word { let r = yield 1; let s = r |> id(); yield id(2) }",
    ] {
        let tokens = keleusma::lexer::tokenize(source).expect("lex");
        let program = keleusma::parser::parse(&tokens).expect("parse");
        let err = keleusma::compiler::compile(&program).expect_err(source);
        assert!(
            err.message.contains("annotate") && !err.message.contains("undefined function"),
            "{source}: {}",
            err.message
        );
    }
}

#[test]
fn a_pipe_into_a_non_generic_function_is_unchanged() {
    // The pipe and the call compile through different lowerings, and the
    // rewrite leaves the pipe's alone. The program still compiles and the
    // chunk keeps its name.
    let module = compile("fn inc(x: Word) -> Word { x + 1 }\nfn main() -> Word { 1 |> inc() }");
    assert_eq!(chunk_names(&module), vec!["inc", "main"]);
}

/// A resume value whose type no `loop main` parameter declares, but which a
/// later use in the same function fixes, specializes a generic call as the
/// annotated form does.
///
/// The operator ruled on 2026-10-31, option B, that monomorphization is fed
/// the type checker's types. Before the repair the type checker inferred the
/// binding's type from the later use, so it did not refuse the call, while
/// monomorphization saw only `loop main`'s parameter and kept the generic
/// chunk `id`. The type checker now hands monomorphization the type it
/// resolved for each such `yield` at the end of its function.
#[test]
fn a_resume_value_fixed_by_a_later_use_specializes_as_the_annotated_form() {
    const ID: &str = "fn id<T>(x: T) -> T { x }\n";
    for (inferred, annotated, instance) in [
        (
            format!(
                "{ID}loop main() -> Word {{ let r = yield 1; let s = id(r); let u: Word = r; yield 0 }}"
            ),
            format!(
                "{ID}loop main() -> Word {{ let r: Word = yield 1; let s = id(r); let u: Word = r; yield 0 }}"
            ),
            "id__Word",
        ),
        (
            format!(
                "{ID}loop main() -> Word {{ let r = yield 1; let s = r |> id(); let u: Byte = r; yield 0 }}"
            ),
            format!(
                "{ID}loop main() -> Word {{ let r: Byte = yield 1; let s = r |> id(); let u: Byte = r; yield 0 }}"
            ),
            "id__Byte",
        ),
        // The later use is an argument to a function with a declared parameter.
        (
            format!(
                "{ID}fn w(x: Word) -> Word {{ x }}\nloop main() -> Word {{ let r = yield 1; let s = id(r); yield w(r) }}"
            ),
            format!(
                "{ID}fn w(x: Word) -> Word {{ x }}\nloop main() -> Word {{ let r: Word = yield 1; let s = id(r); yield w(r) }}"
            ),
            "id__Word",
        ),
        // A `yield` function under a loop with no parameter.
        (
            format!(
                "{ID}yield g() -> Word {{ let r = yield 1; let s = id(r); r }}\nloop main() -> Word {{ let a = g(); yield a }}"
            ),
            format!(
                "{ID}yield g() -> Word {{ let r: Word = yield 1; let s = id(r); r }}\nloop main() -> Word {{ let a = g(); yield a }}"
            ),
            "id__Word",
        ),
        // Two instances in one function, one of them from the resume value.
        (
            format!(
                "{ID}loop main() -> Word {{ let b = id(true); let r = yield 1; let s = id(r); let u: Word = r; yield 0 }}"
            ),
            format!(
                "{ID}loop main() -> Word {{ let b = id(true); let r: Word = yield 1; let s = id(r); let u: Word = r; yield 0 }}"
            ),
            "id__Word",
        ),
    ] {
        let inferred_module = compile(&inferred);
        let annotated_module = compile(&annotated);
        let names = chunk_names(&inferred_module);
        assert!(
            names.iter().any(|n| n == instance) && !names.iter().any(|n| n == "id"),
            "{inferred}: {names:?}"
        );
        assert_eq!(
            inferred_module.to_bytes().unwrap(),
            annotated_module.to_bytes().unwrap(),
            "{inferred}"
        );
    }
}

/// A `yield` passed straight to a generic call, with its type fixed by a later
/// use of the call's result, is specialized. No annotated spelling of this form
/// exists, so the specialization itself is the observation.
#[test]
fn a_yield_argument_fixed_by_a_later_use_is_specialized() {
    let source = "fn id<T>(x: T) -> T { x }\nloop main() -> Word { let s = id(yield 1); let u: Word = s; yield 0 }";
    let names = chunk_names(&compile(source));
    assert!(
        names.iter().any(|n| n == "id__Word") && !names.iter().any(|n| n == "id"),
        "{source}: {names:?}"
    );
}

mod name_resolution {
    //! A pipe and a plain call resolve their target name identically in every
    //! scope.
    //!
    //! A local binding shadows a top-level function of the same name, as `let`
    //! shadowing does for any name. A local is not callable, so a call that names
    //! one is refused. Before the repair the plain call `inc(3)` under
    //! `let inc = 5` was refused while the pipe `3 |> inc()` called the function,
    //! and for a generic `id` both forms compiled a call to the function, since
    //! monomorphization renamed the call before the compiler saw the local. The
    //! operator ruled on 2026-10-31 that the pipe and the plain call resolve the
    //! name identically. Both forms are now refused with the same message wherever
    //! a local of the target's name is in scope, and both call the function
    //! wherever none is.

    fn compile(source: &str) -> Result<keleusma::bytecode::Module, String> {
        let tokens = keleusma::lexer::tokenize(source).expect("lex");
        let program = keleusma::parser::parse(&tokens).expect("parse");
        keleusma::compiler::compile(&program).map_err(|e| e.message)
    }

    const ID: &str = "fn id<T>(x: T) -> T { x }\n";
    const INC: &str = "fn inc(x: Word) -> Word { x + 1 }\n";

    /// Each case is a pipe and the plain call it stands for, in a scope where a
    /// local of the target's name is bound.
    fn shadowed_pairs() -> Vec<(String, String, &'static str)> {
        vec![
            (
                format!("{ID}fn main() -> Word {{ let id = 5; 3 |> id() }}"),
                format!("{ID}fn main() -> Word {{ let id = 5; id(3) }}"),
                "id",
            ),
            (
                format!("{INC}fn main() -> Word {{ let inc = 5; 3 |> inc() }}"),
                format!("{INC}fn main() -> Word {{ let inc = 5; inc(3) }}"),
                "inc",
            ),
            // A parameter.
            (
                format!("{ID}fn main(id: Word) -> Word {{ 3 |> id() }}"),
                format!("{ID}fn main(id: Word) -> Word {{ id(3) }}"),
                "id",
            ),
            (
                format!("{INC}fn main(inc: Word) -> Word {{ 3 |> inc() }}"),
                format!("{INC}fn main(inc: Word) -> Word {{ inc(3) }}"),
                "inc",
            ),
            // A match-arm binding.
            (
                format!("{ID}fn main(x: Word) -> Word {{ match x {{ id => 3 |> id() }} }}"),
                format!("{ID}fn main(x: Word) -> Word {{ match x {{ id => id(3) }} }}"),
                "id",
            ),
            // A loop variable.
            (
                format!("{INC}fn main() -> Word {{ for inc in 0..2 {{ let v = 3 |> inc(); }} 0 }}"),
                format!("{INC}fn main() -> Word {{ for inc in 0..2 {{ let v = inc(3); }} 0 }}"),
                "inc",
            ),
            // The placeholder form.
            (
                String::from(
                    "fn second<T>(a: Word, b: T) -> T { b }\nfn main() -> Word { let second = 1; 7 |> second(1, _) }",
                ),
                String::from(
                    "fn second<T>(a: Word, b: T) -> T { b }\nfn main() -> Word { let second = 1; second(1, 7) }",
                ),
                "second",
            ),
            // A native named by a bare `use`.
            (
                String::from("use f\nfn main() -> Word { let f = 1; 3 |> f() }"),
                String::from("use f\nfn main() -> Word { let f = 1; f(3) }"),
                "f",
            ),
        ]
    }

    #[test]
    fn a_pipe_and_a_call_naming_a_shadowing_local_are_refused_alike() {
        for (piped, called, name) in shadowed_pairs() {
            let piped_err = compile(&piped).expect_err(&piped);
            let called_err = compile(&called).expect_err(&called);
            assert!(
                piped_err.contains(&format!("`{name}` is a local variable, not a callable")),
                "{piped}: {piped_err}"
            );
            assert_eq!(piped_err, called_err, "{piped}");
        }
    }

    /// Where the local's scope has ended, both forms name the function again and
    /// compile to the same module.
    #[test]
    fn a_pipe_and_a_call_after_the_local_scope_ends_compile_identically() {
        for (piped, called) in [
            (
                format!(
                    "{ID}fn main() -> Word {{ for id in 0..2 {{ let v = id + 1; }} 3 |> id() }}"
                ),
                format!("{ID}fn main() -> Word {{ for id in 0..2 {{ let v = id + 1; }} id(3) }}"),
            ),
            (
                format!(
                    "{ID}fn main(x: Word) -> Word {{ let a = match x {{ id => id + 1 }}; a |> id() }}"
                ),
                format!(
                    "{ID}fn main(x: Word) -> Word {{ let a = match x {{ id => id + 1 }}; id(a) }}"
                ),
            ),
            // A local in another function does not shadow.
            (
                format!(
                    "{ID}fn other(id: Word) -> Word {{ id }}\nfn main() -> Word {{ 3 |> id() }}"
                ),
                format!("{ID}fn other(id: Word) -> Word {{ id }}\nfn main() -> Word {{ id(3) }}"),
            ),
        ] {
            let piped_module = compile(&piped).unwrap_or_else(|e| panic!("{piped}: {e}"));
            let called_module = compile(&called).unwrap_or_else(|e| panic!("{called}: {e}"));
            assert_eq!(
                piped_module.to_bytes().unwrap(),
                called_module.to_bytes().unwrap(),
                "{piped}"
            );
        }
    }
}
