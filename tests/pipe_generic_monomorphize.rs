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

#[test]
fn a_pipe_into_a_non_generic_function_is_unchanged() {
    // The pipe and the call compile through different lowerings, and the
    // rewrite leaves the pipe's alone. The program still compiles and the
    // chunk keeps its name.
    let module = compile("fn inc(x: Word) -> Word { x + 1 }\nfn main() -> Word { 1 |> inc() }");
    assert_eq!(chunk_names(&module), vec!["inc", "main"]);
}
