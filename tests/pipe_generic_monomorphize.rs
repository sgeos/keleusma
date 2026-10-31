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
