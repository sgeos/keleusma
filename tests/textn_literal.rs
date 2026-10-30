//! `Text<N>` values and the literal cast `"..." as Text<N>`.
//!
//! # What this increment builds
//!
//! A `Text<N>` with a positive literal capacity is admitted as the type of a
//! local, a parameter, a return value, a tuple element, a struct field, an
//! array element, an `Option` or enum payload, and a `private` data field. The
//! one way to make a value is the literal cast, ruled by the operator on
//! 2026-10-30 as the case for literals with compile-time refusals. The value is
//! the flat body `Tuple(Int length, Array[Byte; N])` with the literal's UTF-8
//! bytes zero padded to `N`.
//!
//! # What it deliberately does not build
//!
//! The operations, concatenation and the rest, wait on how a native signature
//! can be generic over a capacity. The language has no `len` surface to extend,
//! so the length is observed here from the host side by reading the flat body.
//! Every other cast, all arithmetic, and all comparison stay type errors, and
//! the tests below pin that they do, because a comparison the type checker
//! admitted would verify and then fault at runtime.
//!
//! See `docs/decisions/TEXT_CAPACITY_TYPE.md`.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{
    ArrayBody, EnumBody, Module, NewCompositeOperand, Op, StructBody, TupleBody, Value,
};
use keleusma::compiler::compile;
use keleusma::flat_value::FlatComposite;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for,
};

/// Byte width of this build's word, derived from the crate rather than
/// restated, so a narrow-word selector does not make the expectations wrong.
fn word_bytes() -> usize {
    keleusma::target::Target::host().word_bits() as usize / 8
}

/// The flat body a literal cast must produce, built independently of the
/// compiler from the specification in the decision record.
fn expected_body(text: &str, cap: usize) -> Vec<u8> {
    let wb = word_bytes();
    let mut out = (text.len() as u64).to_le_bytes()[..wb].to_vec();
    out.extend_from_slice(text.as_bytes());
    out.resize(wb + cap, 0);
    out
}

fn compile_src(src: &str) -> Module {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    compile(&program).unwrap_or_else(|e| panic!("compile failed for {src}: {}", e.message))
}

/// Compile end to end and return the message. A lex or parse failure panics,
/// so that a fixture with a syntax error cannot pass as a refusal.
fn compile_err(src: &str) -> String {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    match compile(&program) {
        Ok(_) => panic!("expected a refusal, but this compiled: {src}"),
        Err(e) => e.message,
    }
}

/// The result of running `main` with no arguments under strict verification.
/// The bytes of a flat result are copied out while the arena is alive.
enum Outcome {
    Flat(Vec<u8>),
    Other(String),
}

fn run(src: &str) -> Outcome {
    let module = compile_src(src);
    keleusma::verify::verify(&module).expect("verify() must accept the module");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("strict construction must verify");
    let state = vm.call(&[]).expect("main must run");
    let bytes = |fc: &FlatComposite| fc.resolve(&arena).expect("live body").to_vec();
    match state {
        VmState::Finished(Value::Tuple(TupleBody::Flat(fc)))
        | VmState::Finished(Value::Array(ArrayBody::Flat(fc)))
        | VmState::Finished(Value::Struct(StructBody::Flat(fc)))
        | VmState::Finished(Value::Enum(EnumBody::Flat(fc))) => Outcome::Flat(bytes(&fc)),
        other => Outcome::Other(format!("{other:?}")),
    }
}

fn run_flat(src: &str) -> Vec<u8> {
    match run(src) {
        Outcome::Flat(b) => b,
        Outcome::Other(s) => panic!("expected a flat composite result for {src}, got {s}"),
    }
}

// --- The value and its positions ---

#[test]
fn a_literal_cast_returns_the_length_and_the_zero_padded_body() {
    let body = run_flat("fn main() -> Text<8> { \"abc\" as Text<8> }");
    assert_eq!(body, expected_body("abc", 8));
    // Stated separately so a failure names the field that is wrong.
    let wb = word_bytes();
    assert_eq!(body.len(), wb + 8, "the body is exactly word_bytes + N");
    assert_eq!(body[0], 3, "the length word holds the byte count");
    assert!(
        body[wb + 3..].iter().all(|b| *b == 0),
        "the bytes past the length are zero, because a later equality compares whole bodies"
    );
}

#[test]
fn a_literal_cast_is_admitted_at_every_value_position() {
    let positions: &[(&str, &str, Vec<u8>)] = &[
        (
            "let annotation",
            "fn main() -> Text<8> { let t: Text<8> = \"abc\" as Text<8>; t }",
            expected_body("abc", 8),
        ),
        (
            "parameter and return",
            "fn id(t: Text<8>) -> Text<8> { t }\nfn main() -> Text<8> { id(\"abc\" as Text<8>) }",
            expected_body("abc", 8),
        ),
        (
            "tuple element, read back",
            "fn main() -> Text<4> { let p = (1, \"cd\" as Text<4>); p.1 }",
            expected_body("cd", 4),
        ),
        (
            "struct field, read back",
            "struct S { a: Word, t: Text<8> }\n\
             fn main() -> Text<8> { let s = S { a: 1, t: \"abc\" as Text<8> }; s.t }",
            expected_body("abc", 8),
        ),
        (
            "array element, read back",
            "fn main() -> Text<4> { let xs = [\"ab\" as Text<4>, \"cd\" as Text<4>]; xs[1] }",
            expected_body("cd", 4),
        ),
        (
            "private data field, written and read back",
            "private data d { t: Text<8> }\n\
             fn main() -> Text<8> { d.t = \"abc\" as Text<8>; d.t }",
            expected_body("abc", 8),
        ),
        (
            "branch result",
            "fn main() -> Text<4> { if true { \"a\" as Text<4> } else { \"b\" as Text<4> } }",
            expected_body("a", 4),
        ),
        (
            "const generic capacity",
            "fn f<const n: Word>() -> Text<n> { \"ab\" as Text<n> }\n\
             fn main() -> Text<4> { f::<4>() }",
            expected_body("ab", 4),
        ),
    ];
    for (name, src, want) in positions {
        assert_eq!(&run_flat(src), want, "{name}");
    }
}

#[test]
fn an_enclosing_composite_packs_the_text_body_inline() {
    let wb = word_bytes();
    let mut want = 1u64.to_le_bytes()[..wb].to_vec();
    want.extend(expected_body("abc", 8));
    assert_eq!(
        run_flat("fn main() -> (Word, Text<8>) { (1, \"abc\" as Text<8>) }"),
        want,
        "a tuple holding a Text<8> is word + (word + 8) bytes with no handle"
    );
    let mut both = expected_body("ab", 4);
    both.extend(expected_body("cd", 4));
    assert_eq!(
        run_flat("fn main() -> [Text<4>; 2] { [\"ab\" as Text<4>, \"cd\" as Text<4>] }"),
        both,
        "an array of Text<4> is two whole bodies end to end"
    );
    assert_eq!(
        run_flat(
            "struct S { a: Word, t: Text<8> }\n\
             fn main() -> S { S { a: 1, t: \"abc\" as Text<8> } }"
        ),
        want,
        "a struct field is laid out as the tuple field is"
    );
}

#[test]
fn option_and_enum_payloads_carry_the_body() {
    // The enum body leads with its discriminant word, then the payload.
    for src in [
        "fn main() -> Option<Text<8>> { Option::Some(\"a\" as Text<8>) }",
        "enum E { V(Text<8>) }\nfn main() -> E { E::V(\"a\" as Text<8>) }",
    ] {
        let body = run_flat(src);
        let wb = word_bytes();
        assert_eq!(body.len(), wb + wb + 8, "{src}");
        assert_eq!(&body[wb..], expected_body("a", 8).as_slice(), "{src}");
    }
}

#[test]
fn a_text_value_crosses_a_yield() {
    // A flat `Text<N>` carries no pointer, so unlike dynamic `Text` it may be
    // yielded. The yielded body is read before the resume resets the arena.
    let module =
        compile_src("loop main(e: Word) -> Text<4> { let n = yield \"hi\" as Text<4>; n }");
    keleusma::verify::verify(&module).expect("verify");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("strict construction must verify");
    match vm.call(&[Value::Int(0)]).expect("run") {
        VmState::Yielded(Value::Tuple(TupleBody::Flat(fc))) => {
            assert_eq!(fc.resolve(&arena).expect("live"), expected_body("hi", 4));
        }
        other => panic!("expected a yielded flat tuple, got {other:?}"),
    }
}

// --- The capacity boundary ---

#[test]
fn a_literal_exactly_as_long_as_the_capacity_is_admitted() {
    assert_eq!(
        run_flat("fn main() -> Text<3> { \"abc\" as Text<3> }"),
        expected_body("abc", 3)
    );
}

#[test]
fn a_literal_one_byte_too_long_is_refused_naming_both_lengths() {
    let msg = compile_err("fn main() -> Text<2> { \"abc\" as Text<2> }");
    assert!(
        msg.contains("3 bytes") && msg.contains("Text<2>"),
        "the refusal must name the literal length and the capacity: {msg}"
    );
    assert!(msg.contains("refused rather than truncated"), "{msg}");
}

#[test]
fn the_capacity_counts_utf8_bytes_not_characters() {
    // U+00E9 is two bytes in UTF-8. One character fills a Text<2> exactly and
    // does not fit a Text<1>, which a character count would have admitted.
    assert_eq!(
        run_flat("fn main() -> Text<2> { \"\u{e9}\" as Text<2> }"),
        expected_body("\u{e9}", 2)
    );
    let msg = compile_err("fn main() -> Text<1> { \"\u{e9}\" as Text<1> }");
    assert!(msg.contains("2 bytes") && msg.contains("Text<1>"), "{msg}");
}

#[test]
fn an_interior_nul_is_content_and_counts_toward_the_length() {
    assert_eq!(
        run_flat("fn main() -> Text<4> { \"a\\0b\" as Text<4> }"),
        expected_body("a\0b", 4)
    );
}

#[test]
fn a_capacity_too_small_after_monomorphization_is_refused() {
    let msg = compile_err(
        "fn f<const n: Word>() -> Text<n> { \"abcde\" as Text<n> }\n\
         fn main() -> Text<4> { f::<4>() }",
    );
    assert!(msg.contains("5 bytes") && msg.contains("Text<4>"), "{msg}");
}

#[test]
fn a_zero_capacity_is_refused_in_every_position() {
    for src in [
        "fn main() -> Word { let t: Text<0> = \"\" as Text<0>; 1 }",
        "fn f(t: Text<0>) -> Word { 1 }\nfn main() -> Word { 1 }",
        "fn f() -> Text<0> { \"\" as Text<0> }\nfn main() -> Word { 1 }",
        "struct S { t: Text<0> }\nfn main() -> Word { 1 }",
        "fn main() -> Word { let t = \"\" as Text<0>; 1 }",
        "private data d { t: Text<0> }\nfn main() -> Word { d.t = \"\" as Text<0>; 1 }",
    ] {
        let msg = compile_err(src);
        assert!(msg.contains("Text<0> is not a legal type"), "{src}: {msg}");
    }
}

// --- What stays refused ---

#[test]
fn only_a_string_literal_may_be_cast_to_text_n() {
    for (name, src) in [
        (
            "static text through a local",
            "fn main() -> Word { let s = \"a\"; let t = s as Text<4>; 1 }",
        ),
        ("a word", "fn main() -> Word { let t = 1 as Text<4>; 1 }"),
        (
            "an identity cast",
            "fn main() -> Text<4> { let t = \"a\" as Text<4>; t as Text<4> }",
        ),
        (
            "a widening cast",
            "fn main() -> Text<8> { let t = \"a\" as Text<4>; t as Text<8> }",
        ),
    ] {
        let msg = compile_err(src);
        assert!(msg.contains("only a string literal"), "{name}: {msg}");
    }
}

#[test]
fn a_text_n_cannot_be_cast_to_anything() {
    for src in [
        "fn main() -> Text { let t = \"a\" as Text<4>; t as Text }",
        "fn main() -> Word { let t = \"a\" as Text<4>; t as Word }",
    ] {
        let msg = compile_err(src);
        assert!(msg.contains("the only cast involving"), "{src}: {msg}");
    }
}

#[test]
fn a_static_literal_is_not_implicitly_a_text_n() {
    let msg = compile_err("fn main() -> Text<8> { let t: Text<8> = \"abc\"; t }");
    assert!(msg.contains("Text<8>") && msg.contains("Text"), "{msg}");
    let msg = compile_err("fn main() -> Text<8> { let t: Text<8> = \"abc\" as Text<4>; t }");
    assert!(
        msg.contains("Text<4>"),
        "two capacities are two types: {msg}"
    );
}

#[test]
fn arithmetic_comparison_and_destructuring_are_refused() {
    for (name, src, needle) in [
        (
            "addition",
            "fn main() -> Word { let t = \"a\" as Text<4>; let u = t + t; 1 }",
            "cannot add",
        ),
        (
            "equality",
            "fn main() -> bool { let t = \"a\" as Text<4>; t == t }",
            "comparison of a capacity-bounded text",
        ),
        (
            "ordering",
            "fn main() -> bool { let t = \"a\" as Text<4>; t < t }",
            "comparison of a capacity-bounded text",
        ),
        (
            "equality of a tuple holding one",
            "fn main() -> bool { let p = (1, \"a\" as Text<4>); p == p }",
            "comparison of a capacity-bounded text",
        ),
        (
            "equality of a struct holding one",
            "struct S { t: Text<4> }\n\
             fn main() -> bool { let p = S { t: \"a\" as Text<4> }; p == p }",
            "comparison of a capacity-bounded text",
        ),
        (
            "tuple field read",
            "fn main() -> Word { let t = \"a\" as Text<4>; t.0 }",
            "non-tuple",
        ),
        (
            "index",
            "fn main() -> Word { let t = \"a\" as Text<4>; let x = t[0]; 1 }",
            "non-array",
        ),
        (
            "let destructuring",
            "fn main() -> Word { let t = \"a\" as Text<4>; let (a, b) = t; a }",
            "tuple pattern",
        ),
        (
            "match destructuring",
            "fn main() -> Word { let t = \"a\" as Text<4>; match t { (a, b) => 1 } }",
            "tuple pattern",
        ),
        (
            "equality reached through a generic function",
            "fn same<T>(a: T, b: T) -> bool { a == b }\n\
             fn main() -> bool { let t = \"a\" as Text<4>; same(t, t) }",
            "comparison of a capacity-bounded text",
        ),
        (
            "a const data field initialised from a literal",
            "const data c { t: Text<8> = \"abc\" }\nfn main() -> Word { 1 }",
            "incompatible",
        ),
    ] {
        let msg = compile_err(src);
        assert!(msg.contains(needle), "{name}: {msg}");
    }
}

#[test]
fn a_shared_data_field_is_refused() {
    let msg = compile_err(
        "shared data d { t: Text<8> }\nfn main() -> Word { d.t = \"abc\" as Text<8>; 1 }",
    );
    assert!(msg.contains("shared"), "{msg}");
}

#[test]
fn a_native_signature_is_refused() {
    let msg = compile_err(
        "use host::f(Text<8>) -> Word\nfn main() -> Word { host::f(\"a\" as Text<8>) }",
    );
    assert!(msg.contains("native signature"), "{msg}");
}

#[test]
fn bare_text_is_unaffected() {
    // MUST-FIRE CONTROL. Without it the admissions above could have broken
    // static text and this file would still pass.
    match run("fn main() -> Text { \"hi\" }") {
        Outcome::Other(s) => assert!(s.contains("Finished"), "{s}"),
        Outcome::Flat(_) => panic!("static text must not become a flat composite"),
    }
}

// --- Verification and the memory bound ---

#[test]
fn the_literal_is_one_pooled_constant_of_word_bytes_plus_n() {
    // The cast is one `Op::Const` over a pooled constant, so its body lives in
    // the virtual machine's const pool and the load allocates nothing.
    for cap in [4usize, 8, 40] {
        let src = format!("fn main() -> Text<{cap}> {{ \"abc\" as Text<{cap}> }}");
        let module = compile_src(&src);
        keleusma::verify::verify(&module).expect("verify");
        let main = module
            .chunks
            .iter()
            .find(|c| c.name == "main")
            .expect("main");
        assert!(
            !main.ops.iter().any(|op| matches!(op, Op::NewComposite(_))),
            "Text<{cap}>: the literal cast must not construct at runtime"
        );
        let need = required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        arena.resize_persistent(need).expect("arena");
        let vm = Vm::new(module, &arena).expect("strict construction must verify");
        assert_eq!(
            vm.const_pool_bytes(),
            word_bytes() + cap,
            "Text<{cap}>: the pooled body is word_bytes + N"
        );
    }
}

#[test]
fn an_enclosing_allocation_is_sized_word_bytes_plus_n_and_the_bound_follows() {
    // A struct holding a `Text<N>` allocates word (field `a`) + word + N.
    let src = |cap: usize| {
        format!(
            "struct S {{ a: Word, t: Text<{cap}> }}\n\
             fn main() -> Word {{ let s = S {{ a: 1, t: \"abc\" as Text<{cap}> }}; s.a }}"
        )
    };
    let wb = word_bytes();
    let mut bounds = Vec::new();
    for cap in [8usize, 40] {
        let module = compile_src(&src(cap));
        keleusma::verify::verify(&module).expect("verify");
        let main = module
            .chunks
            .iter()
            .find(|c| c.name == "main")
            .expect("main");
        let sizes: Vec<u16> = main
            .ops
            .iter()
            .filter_map(|op| match op {
                Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) => Some(*byte_size),
                _ => None,
            })
            .collect();
        assert_eq!(
            sizes,
            vec![(wb + wb + cap) as u16],
            "Text<{cap}>: one flat allocation of word + word + N"
        );
        bounds.push(auto_arena_capacity_for(&module, &[]).expect("the bound is provable"));
    }
    assert_eq!(
        bounds[1] - bounds[0],
        32,
        "raising N by 32 raises the worst-case memory bound by exactly 32 bytes"
    );
}

// --- The self-hosted compiler ---

/// The stages under `src/selfhost/kel/` do not lower `Text<N>`. The shipping
/// cross-check must refuse it as unsupported in every position, never report a
/// divergence or panic, and must still report the reference's own rejection
/// when the reference rejects the program.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_refuses_text_n_cleanly() {
    use keleusma::selfhost::{SelfHostError, self_hosted_compile};
    let target = keleusma::target::Target::host();
    for src in [
        "fn main(a: Text<8>) -> Word { 1 }",
        "fn main() -> Text<8> { \"abc\" as Text<8> }",
        "fn main() -> Word { let t = \"abc\" as Text<8>; 1 }",
        "fn main() -> Word { let t: Text<8> = \"abc\" as Text<8>; 1 }",
        "fn main() -> (Word, Text<4>) { (1, \"a\" as Text<4>) }",
        "struct S { a: Word, t: Text<8> }\nfn main() -> Word { 1 }",
        "private data d { t: Text<8> }\nfn main() -> Word { d.t = \"a\" as Text<8>; 1 }",
    ] {
        match self_hosted_compile(src, &target) {
            Err(SelfHostError::Unsupported { detail }) => {
                assert!(detail.contains("Text<N>"), "{src}: {detail}")
            }
            other => panic!("{src}: expected an unsupported refusal, got {other:?}"),
        }
    }
    match self_hosted_compile("fn main() -> Text<2> { \"abc\" as Text<2> }", &target) {
        Err(SelfHostError::ReferenceRejected { detail }) => {
            assert!(detail.contains("does not fit"), "{detail}")
        }
        other => panic!("expected the reference's rejection, got {other:?}"),
    }
}
