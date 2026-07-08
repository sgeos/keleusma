//! audit C11: a fixed-size array must have a positive length. A zero-length
//! array occupies no storage and traps on every index, so it is rejected at
//! compile time in every declaration position (struct and enum fields,
//! function parameters and returns, local annotations) and whether the length
//! is a literal or a const parameter that monomorphizes to zero. The layout
//! pass carries the same bound as a backstop.
#![cfg(feature = "compile")]
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn compiles(src: &str) -> bool {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .map(|a| compile(&a).is_ok())
        .unwrap_or(false)
}

#[test]
fn zero_length_arrays_are_rejected() {
    assert!(!compiles(
        "struct S { a: [Word; 0] } fn main() -> Word { 0 }"
    ));
    assert!(!compiles(
        "struct S { a: [Word; 0], b: Word } fn main() -> Word { let s = S { a: [], b: 5 }; s.b }"
    ));
    assert!(!compiles("enum E { V([Word; 0]) } fn main() -> Word { 0 }"));
    assert!(!compiles(
        "fn z() -> [Word; 0] { [] } fn main() -> Word { 0 }"
    ));
    assert!(!compiles("fn main() -> Word { let a: [Word; 0] = []; 0 }"));
    // A const-generic array parameter that monomorphizes to length zero is
    // rejected at the post-monomorphization re-typecheck, the same gate the
    // const-dimension-mismatch test exercises.
    assert!(!compiles(
        "fn first<const n: Word>(a: [Word; n]) -> Word { a[0] }\n\
         fn main() -> Word { first::<0>([]) }"
    ));
}

#[test]
fn positive_length_arrays_still_compile() {
    assert!(compiles(
        "struct S { a: [Word; 3] } fn main() -> Word { 0 }"
    ));
    // The same const-generic form at a positive dimension compiles.
    assert!(compiles(
        "fn first<const n: Word>(a: [Word; n]) -> Word { a[0] }\n\
         fn main() -> Word { first::<3>([10, 20, 30]) }"
    ));
}
