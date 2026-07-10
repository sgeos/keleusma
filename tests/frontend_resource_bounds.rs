//! The compiler front end must be bounded against pathological source, a
//! compile-time denial-of-service surface distinct from the runtime WCET and
//! WCMU bounds (Rex-review lesson 3). The parser caps recursion depth and the
//! monomorphizer caps specialization count; this suite guards the type-inference
//! path, where a chain of type-doubling tuple bindings such as
//! `let t = (p, p)` otherwise grows the inferred type, and inference time,
//! exponentially in the source length.

use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::typecheck::check;

/// A chain of `n` type-doubling tuple bindings. The type of `t{k}` has on the
/// order of `2^k` nodes, so without a bound the type checker runs in
/// exponential time and memory.
fn type_doubling_chain(n: usize) -> String {
    let mut s = String::from("fn main() -> Word {\n  let t0 = (0, 0);\n");
    for i in 1..n {
        s.push_str(&format!("  let t{i} = (t{}, t{});\n", i - 1, i - 1));
    }
    s.push_str("  0\n}\n");
    s
}

fn typecheck(src: &str) -> Result<(), String> {
    let mut program = parse(&tokenize(src).expect("lex")).expect("parse");
    check(&mut program).map_err(|e| e.message)
}

#[test]
fn type_doubling_chain_is_rejected_not_hung() {
    // At n = 20 the inferred type would have ~2^20 nodes. With the bound this
    // returns a size error in milliseconds. Without it, the checker either
    // returns Ok after building a multi-million-node type (this assertion then
    // fails fast) or, at larger n, does not terminate. Either way a regression
    // is caught rather than silently accepted.
    let err = typecheck(&type_doubling_chain(20))
        .expect_err("a type-doubling chain must be rejected, not accepted or hung");
    assert!(
        err.contains("too large"),
        "expected a tuple-type-size rejection, got: {err}"
    );
}

#[test]
fn deeper_chain_still_terminates_and_is_rejected() {
    // A far deeper chain that would be hopeless to type-check unbounded must
    // still be rejected promptly; reaching this assertion at all proves the
    // walk terminated.
    assert!(typecheck(&type_doubling_chain(40)).is_err());
}

#[test]
fn ordinary_tuples_are_unaffected() {
    // Real tuple types are tiny and must not trip the bound.
    assert!(typecheck("fn main() -> Word { let p = (1, 2, 3); p.0 + p.1 + p.2 }").is_ok());
    // A modestly nested tuple type is fine.
    assert!(
        typecheck("fn main() -> Word { let a = (1, 2); let b = (a, a); let x = b.0; x.0 }").is_ok()
    );
    // A wide-but-shallow tuple is fine.
    let wide = format!(
        "fn main() -> Word {{ let t = ({}); t.0 }}",
        "0, ".repeat(64) + "0"
    );
    assert!(typecheck(&wide).is_ok());
}
