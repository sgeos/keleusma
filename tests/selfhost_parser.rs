//! Stage 2 parser (`compiler/kel/parser.kel`), increment 4: the full declaration
//! signature, streamed as a per-declaration record.
//!
//! A throwaway adapter maps the reference tokenizer's output into the parser
//! stage's `(kind, value)` token stream, the Keleusma `loop` consumes it one token
//! per iteration, and it emits a small record per top-level declaration: a `START`
//! element (category from the `fn`/`yield`/`loop` keyword, interned name), then per
//! value parameter a `PARAM` (its name) and a `PTYPE` (its type name), then a
//! `RETTYPE` (the return type name), then `END`. Types are simple named types this
//! increment; nested types (generics, arrays, tuples) and the parsed body are later
//! increments. The host reassembles each record and checks its (category, name,
//! parameter names and types, return type) against the reference parse's functions,
//! including a const-generic type parameter that must not be mistaken for a value
//! parameter and multiheaded functions whose heads are separate declarations.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::ast::{FunctionCategory, Pattern, PrimType, TypeExpr};
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `toks` block in parser.kel: len at 0,
// then the two length-2048 arrays.
const LEN: usize = 0;
const KINDS: usize = 1;
const VALS: usize = 1 + 2048;

/// Map the reference token stream into the parser stage's `(kind, value)` pairs,
/// interning identifier names into `names` so the parser's yielded name id can be
/// resolved back to a string. The trailing `Eof` token is dropped so the token
/// count is exactly the real tokens; the parser reports DONE at `cursor == len`.
fn adapt_tokens(src: &str, names: &mut Vec<String>) -> (Vec<i64>, Vec<i64>) {
    let mut intern = |s: &str| -> i64 {
        if let Some(i) = names.iter().position(|n| n == s) {
            i as i64
        } else {
            names.push(s.to_string());
            (names.len() - 1) as i64
        }
    };
    let tokens = tokenize(src).expect("lex");
    let mut kinds = Vec::new();
    let mut vals = Vec::new();
    for tok in &tokens {
        let (kind, val) = match &tok.kind {
            TokenKind::Fn => (0, 0),
            TokenKind::LowerIdent(s) | TokenKind::UpperIdent(s) => (1, intern(s)),
            TokenKind::LBrace => (2, 0),
            TokenKind::RBrace => (3, 0),
            TokenKind::Yield => (5, 0),
            TokenKind::Loop => (6, 0),
            TokenKind::LParen => (7, 0),
            TokenKind::RParen => (8, 0),
            TokenKind::Colon => (9, 0),
            TokenKind::Comma => (10, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    (kinds, vals)
}

/// A parsed declaration: its category (1 fn, 2 yield, 3 loop), interned name id,
/// and its value parameters as (name id, type name id) in order, plus the return
/// type name id.
type Decl = (i64, i64, Vec<(i64, i64)>, i64);

/// Drive the parser stage over `src`, decoding the START/PARAM/END record stream
/// into one [`Decl`] per top-level declaration.
fn run_parser(src: &str, names: &mut Vec<String>) -> Vec<Decl> {
    let (kinds, vals) = adapt_tokens(src, names);
    let stage = std::fs::read_to_string("compiler/kel/parser.kel").expect("read parser.kel");
    let module =
        compile(&parse(&tokenize(&stage).expect("lex parser.kel")).expect("parse parser.kel"))
            .expect("compile parser.kel");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parser.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, LEN, Value::Int(kinds.len() as i64))
        .expect("len");
    for (i, (&k, &v)) in kinds.iter().zip(vals.iter()).enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(k))
            .expect("kind");
        vm.set_shared(&mut shared, VALS + i, Value::Int(v))
            .expect("val");
    }

    let mut decls: Vec<Decl> = Vec::new();
    let mut cur: Option<Decl> = None;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(kinds.len() * 2 + 16) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let dkind = w.rem_euclid(16);
                let val = w.div_euclid(16);
                match dkind {
                    0 => {}                                                           // PENDING
                    1..=3 => cur = Some((dkind, val, Vec::new(), -1)),                // START
                    4 => cur.as_mut().expect("PARAM before START").2.push((val, -1)), // PARAM name
                    6 => {
                        // PTYPE: fill the type of the parameter just emitted.
                        cur.as_mut()
                            .expect("PTYPE before START")
                            .2
                            .last_mut()
                            .expect("PTYPE before PARAM")
                            .1 = val;
                    }
                    7 => cur.as_mut().expect("RETTYPE before START").3 = val, // RETTYPE
                    5 => decls.push(cur.take().expect("END before START")),   // END
                    15 => {
                        assert!(cur.is_none(), "DONE mid-declaration");
                        return decls;
                    }
                    other => panic!("unexpected declaration kind {other}"),
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected VM state {other:?}"),
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("parser did not reach DONE within the iteration budget");
}

/// The surface name of a simple primitive type, matching the identifier token the
/// adapter interned. The increment-4 tests use primitive types; a named struct or
/// enum type and the nested types are exercised in later increments.
fn type_name(t: &TypeExpr) -> &'static str {
    match t {
        TypeExpr::Prim(p, _) => match p {
            PrimType::Byte => "Byte",
            PrimType::Word => "Word",
            PrimType::Fixed(_) => "Fixed",
            PrimType::Float => "Float",
            PrimType::Bool => "bool",
            PrimType::Text => "Text",
        },
        other => panic!("test uses primitive types only this increment, got {other:?}"),
    }
}

/// The reference parse's top-level functions, in order, as [`Decl`]s: category,
/// interned name id, value parameters as (name id, type name id), and the return
/// type name id. The category encoding matches the stage's dkind: fn 1, yield 2,
/// loop 3.
fn reference_functions(src: &str, names: &[String]) -> Vec<Decl> {
    let id_of = |s: &str| -> i64 {
        names
            .iter()
            .position(|n| n == s)
            .unwrap_or_else(|| panic!("name `{s}` was interned")) as i64
    };
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    program
        .functions
        .iter()
        .map(|f| {
            let cat = match f.category {
                FunctionCategory::Fn => 1,
                FunctionCategory::Yield => 2,
                FunctionCategory::Loop => 3,
            };
            let params = f
                .params
                .iter()
                .map(|p| {
                    let name = match &p.pattern {
                        Pattern::Variable(n, _) => id_of(n),
                        other => {
                            panic!("test uses simple parameter patterns only, got {other:?}")
                        }
                    };
                    let ty = id_of(type_name(
                        p.type_expr.as_ref().expect("annotated parameter"),
                    ));
                    (name, ty)
                })
                .collect();
            (
                cat,
                id_of(&f.name),
                params,
                id_of(type_name(&f.return_type)),
            )
        })
        .collect()
}

// A single function: the parser recognises one declaration and yields its name.
#[test]
fn a_single_function_is_recognised() {
    let src = "fn main() -> Word { 42 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 1);
}

// Several functions in order, including parameters and a nested-brace body.
#[test]
fn functions_are_yielded_in_order() {
    let src = "fn inc(x: Word) -> Word { x + 1 } \
        fn choose(a: Word) -> Word { if a > 0 { a } else { 0 } } \
        fn main() -> Word { choose(inc(2)) }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 3);
}

// A body with deeply nested braces (match arms, blocks) still ends at the correct
// closing brace, so the next declaration is recognised.
#[test]
fn nested_braces_do_not_confuse_the_boundary() {
    let src = "fn a(n: Word) -> Word { match n { 0 => 1, _ => n } } \
        fn b(n: Word) -> Word { if n > 0 { if n > 1 { 2 } else { 1 } } else { 0 } }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 2);
}

// An empty program yields no declarations and reaches DONE immediately.
#[test]
fn an_empty_program_yields_no_declarations() {
    let mut names = Vec::new();
    let got = run_parser("", &mut names);
    assert!(got.is_empty());
}

// The three function categories are distinguished by their keyword.
#[test]
fn categories_are_distinguished() {
    let src = "fn a() -> Word { 0 } \
        yield b(r: Word) -> Word { yield r } \
        loop c(r: Word) -> Word { yield r }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    // Categories: fn 1, yield 2, loop 3.
    assert_eq!(got.iter().map(|d| d.0).collect::<Vec<_>>(), vec![1, 2, 3]);
}

// The value parameters — names and types — and the return type are read, in order.
#[test]
fn parameter_names_types_and_return_type_are_read() {
    let src = "fn a() -> Word { 0 } \
        fn b(x: Byte) -> bool { x == x } \
        fn c(x: Word, y: Byte, z: bool) -> Word { 0 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    // Value-parameter counts, derived from the extracted names.
    assert_eq!(
        got.iter().map(|d| d.2.len()).collect::<Vec<_>>(),
        vec![0, 1, 3]
    );
    // `c`'s parameters are x: Word, y: Byte, z: bool; its return type is Word.
    let id = |s: &str| names.iter().position(|n| n == s).unwrap() as i64;
    assert_eq!(
        got[2].2,
        vec![
            (id("x"), id("Word")),
            (id("y"), id("Byte")),
            (id("z"), id("bool")),
        ]
    );
    assert_eq!(got[2].3, id("Word"));
    // `b`'s return type is bool.
    assert_eq!(got[1].3, id("bool"));
}

// A type parameter before the value parentheses is not mistaken for a value
// parameter, and the value parameter's simple type is read.
#[test]
fn type_parameters_are_not_mistaken_for_parameters() {
    let src = "fn h<const n: Word>(x: Word) -> Word { x }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    // h has one value parameter (x: Word); the `const n` type parameter sits before
    // the value parentheses.
    assert_eq!(got[0].2.len(), 1);
    let id = |s: &str| names.iter().position(|n| n == s).unwrap() as i64;
    assert_eq!(got[0].2, vec![(id("x"), id("Word"))]);
}

// Each head of a multiheaded function is a separate declaration; the reference
// lists each head, and the parser yields each, with the same category and params.
#[test]
fn a_multiheaded_function_yields_each_head() {
    let src = "yield g(r: Word) -> Word when r > 0 { yield r } \
        yield g(r: Word) -> Word { yield 0 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference_functions(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.len(), 2);
    // Both heads: yield category, one parameter named r.
    assert!(got.iter().all(|d| d.0 == 2 && d.2.len() == 1));
}
