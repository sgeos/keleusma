//! Which small source actually produces an opcode `lower_module` refuses?
//!
//! Written after FOUR consecutive guesses at such a source, each costing a
//! compile-and-run cycle and none informed by anything. The instrument to answer
//! it existed the whole time. This asks all candidates in one run.
use inkwell::context::Context;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

#[test]
fn probe_which_sources_are_refused() {
    let cases: &[(&str, &str)] = &[
        (
            "nested struct",
            "struct I { a: Word }\nstruct O { i: I, b: Word }\nfn main(a: Word, b: Word) -> Word { let o = O { i: I { a: a }, b: b }; o.i.a }",
        ),
        (
            "enum match",
            "enum E { A(Word), B(Word) }\nfn main(a: Word, b: Word) -> Word { let e = E::A(a); match e { E::A(x) => x, E::B(y) => y + b } }",
        ),
        (
            "array of array",
            "fn main(a: Word, b: Word) -> Word { let xs = [a, b]; let ys = [xs, xs]; ys[0][1] }",
        ),
        (
            "tuple",
            "fn main(a: Word, b: Word) -> Word { let t = (a, b); t.1 }",
        ),
        (
            "byte field",
            "struct P { a: Byte, b: Word }\nfn main(a: Word, b: Word) -> Word { let p = P { a: 1 as Byte, b: b }; p.b }",
        ),
        (
            "struct in array",
            "struct P { x: Word }\nfn main(a: Word, b: Word) -> Word { let xs = [P { x: a }, P { x: b }]; xs[1].x }",
        ),
    ];
    println!("================ PROBE: what does lower_module refuse?");
    for (name, src) in cases {
        match parse(&tokenize(src).unwrap_or_default())
            .ok()
            .and_then(|a| compile(&a).ok())
        {
            None => println!("  {name:18} REFERENCE REJECTED (not a backend gap)"),
            Some(m) => {
                let ctx = Context::create();
                let lm = ctx.create_module("kel");
                match keleusma_native::lower_module(
                    &ctx,
                    &lm,
                    &m,
                    keleusma_native::LowerOptions::default(),
                ) {
                    Ok(_) => println!("  {name:18} LOWERS"),
                    Err(e) => println!("  {name:18} REFUSED: {e:?}"),
                }
            }
        }
    }
    println!("================");
}
