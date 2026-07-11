//! Stage 2 parser (`compiler/kel/parser.kel`), increment 6: the function signature
//! (including array types) and the shared and private `data` block declaration,
//! streamed as a per-declaration record.
//!
//! A throwaway adapter maps the reference tokenizer's output into the parser
//! stage's `(kind, value)` token stream, the Keleusma `loop` consumes it one token
//! per iteration, and it emits a small record per top-level declaration. A function
//! is a `START` element (category from the `fn`/`yield`/`loop` keyword, interned
//! name), then per value parameter a `PARAM` (its name) and a `PTYPE` (its type
//! name), then a `RETTYPE` (the return type name), then `END`. A data block is a
//! `DSTART` element (visibility from the `shared`/`private` modifier, interned name),
//! then per field a `PARAM` and a `PTYPE`, then `END`; it has no `RETTYPE`. A `PTYPE`
//! or `RETTYPE` naming an array element is immediately followed by an `ASIZE`
//! carrying the literal length, so the host reconstructs `[T; N]`. The element type
//! is a simple named type and the length a literal this increment; an arbitrarily
//! nested element, a const length, the other nested types (generics, tuples), the
//! parsed body, and the const data block are later increments. The host reassembles
//! each record and checks the function declarations and data block declarations
//! against the reference parse's two collections, including a const-generic type
//! parameter that must not be mistaken for a value parameter, multiheaded functions
//! whose heads are separate declarations, and array-typed fields.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::ast::{DataVisibility, FunctionCategory, Pattern, PrimType, TypeExpr};
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
            TokenKind::LBracket => (11, 0),
            TokenKind::IntLit(n) => (12, *n),
            TokenKind::Data => (13, 0),
            TokenKind::Shared => (14, 0),
            TokenKind::Private => (15, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    (kinds, vals)
}

/// A parsed type: a simple named type (its name id), or an array of a named element
/// type with a literal length. `Missing` is the placeholder before a type is read,
/// used for the return type of a declaration whose RETTYPE has not yet arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeRepr {
    Missing,
    Named(i64),
    Array(i64, i64),
}

/// A parsed function declaration: its category (1 fn, 2 yield, 3 loop), interned
/// name id, its value parameters as (name id, type) in order, and the return type.
type Decl = (i64, i64, Vec<(i64, TypeRepr)>, TypeRepr);

/// A parsed data block declaration: its visibility (0 shared, 1 private), interned
/// name id, and its fields as (name id, type) in order.
type DataBlock = (i64, i64, Vec<(i64, TypeRepr)>);

/// Everything the parser yields, in the two collections the reference AST keeps them
/// in: function declarations and data block declarations.
#[derive(Debug, Default, PartialEq)]
struct Parsed {
    funcs: Vec<Decl>,
    data: Vec<DataBlock>,
}

/// Which type an `ASIZE` element upgrades to an array: the last parameter's type, or
/// the return type. Set by the preceding PTYPE or RETTYPE.
#[derive(Clone, Copy)]
enum ArrTarget {
    None,
    Param,
    Ret,
}

/// The declaration currently being assembled from the record stream: a function
/// (opened by START) or a data block (opened by DSTART). Both carry a field/parameter
/// list; only a function carries a return type.
enum Cur {
    Func(Decl),
    Data(DataBlock),
}

impl Cur {
    /// The parameter list (function) or field list (data block).
    fn fields(&mut self) -> &mut Vec<(i64, TypeRepr)> {
        match self {
            Cur::Func(d) => &mut d.2,
            Cur::Data(d) => &mut d.2,
        }
    }
}

/// Drive the parser stage over `src`, decoding the record stream into the function
/// and data block declarations, each in source order within its kind.
fn run_parser(src: &str, names: &mut Vec<String>) -> Parsed {
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

    let mut parsed = Parsed::default();
    let mut cur: Option<Cur> = None;
    // The type the next ASIZE upgrades to an array, set by the preceding PTYPE/RETTYPE.
    let mut arr_target = ArrTarget::None;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(kinds.len() * 2 + 16) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let dkind = w.rem_euclid(16);
                let val = w.div_euclid(16);
                match dkind {
                    0 => {} // PENDING
                    1..=3 => {
                        // START of a function declaration.
                        cur = Some(Cur::Func((dkind, val, Vec::new(), TypeRepr::Missing)));
                        arr_target = ArrTarget::None;
                    }
                    9 => {
                        // DSTART of a data block; val packs name*4 + visibility.
                        let vis = val.rem_euclid(4);
                        let name = val.div_euclid(4);
                        cur = Some(Cur::Data((vis, name, Vec::new())));
                        arr_target = ArrTarget::None;
                    }
                    4 => cur
                        .as_mut()
                        .expect("PARAM before START/DSTART")
                        .fields()
                        .push((val, TypeRepr::Missing)), // PARAM/field name
                    6 => {
                        // PTYPE: the type of the parameter or field just emitted.
                        cur.as_mut()
                            .expect("PTYPE before START/DSTART")
                            .fields()
                            .last_mut()
                            .expect("PTYPE before PARAM")
                            .1 = TypeRepr::Named(val);
                        arr_target = ArrTarget::Param;
                    }
                    7 => {
                        // RETTYPE; a function only.
                        match cur.as_mut().expect("RETTYPE before START") {
                            Cur::Func(d) => d.3 = TypeRepr::Named(val),
                            Cur::Data(_) => panic!("RETTYPE inside a data block"),
                        }
                        arr_target = ArrTarget::Ret;
                    }
                    8 => {
                        // ASIZE: upgrade the type just emitted to an array of this length.
                        let c = cur.as_mut().expect("ASIZE before START/DSTART");
                        match arr_target {
                            ArrTarget::Param => {
                                let slot =
                                    &mut c.fields().last_mut().expect("ASIZE before field").1;
                                let elem = match *slot {
                                    TypeRepr::Named(e) => e,
                                    _ => panic!("ASIZE without a preceding element type"),
                                };
                                *slot = TypeRepr::Array(elem, val);
                            }
                            ArrTarget::Ret => match c {
                                Cur::Func(d) => {
                                    let elem = match d.3 {
                                        TypeRepr::Named(e) => e,
                                        _ => panic!("ASIZE without a preceding element type"),
                                    };
                                    d.3 = TypeRepr::Array(elem, val);
                                }
                                Cur::Data(_) => panic!("return-type ASIZE inside a data block"),
                            },
                            ArrTarget::None => panic!("ASIZE with no pending type"),
                        }
                        arr_target = ArrTarget::None;
                    }
                    5 => match cur.take().expect("END before START/DSTART") {
                        Cur::Func(d) => parsed.funcs.push(d),
                        Cur::Data(d) => parsed.data.push(d),
                    }, // END
                    15 => {
                        assert!(cur.is_none(), "DONE mid-declaration");
                        return parsed;
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

/// The surface name of a simple named type, matching the identifier token the
/// adapter interned. Primitive types map to their surface spelling; a named struct
/// or enum type keeps its name. Array and the other nested types are handled by
/// [`type_repr`]; a nested element type is a later increment.
fn type_name(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Prim(p, _) => match p {
            PrimType::Byte => "Byte",
            PrimType::Word => "Word",
            PrimType::Fixed(_) => "Fixed",
            PrimType::Float => "Float",
            PrimType::Bool => "bool",
            PrimType::Text => "Text",
        }
        .to_string(),
        TypeExpr::Named(n, _, _, _) => n.clone(),
        other => panic!("test uses simple named types only this increment, got {other:?}"),
    }
}

/// The reference parse's top-level declarations as a [`Parsed`]: the function
/// declarations (category, name id, parameters as (name id, type), return type; the
/// category encoding matches the stage's dkind fn 1, yield 2, loop 3) and the data
/// block declarations (visibility 0 shared / 1 private, name id, fields as
/// (name id, type)), each in source order within its kind.
fn reference(src: &str, names: &[String]) -> Parsed {
    let id_of = |s: &str| -> i64 {
        names
            .iter()
            .position(|n| n == s)
            .unwrap_or_else(|| panic!("name `{s}` was interned")) as i64
    };
    // A simple named type or an array of one with a literal length; the two forms the
    // stage reads this increment.
    let type_repr = |t: &TypeExpr| -> TypeRepr {
        match t {
            TypeExpr::Array(elem, len, _) => {
                let n = len
                    .as_lit()
                    .expect("test uses literal array lengths only this increment");
                TypeRepr::Array(id_of(&type_name(elem)), n)
            }
            other => TypeRepr::Named(id_of(&type_name(other))),
        }
    };
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let funcs = program
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
                    let ty = type_repr(p.type_expr.as_ref().expect("annotated parameter"));
                    (name, ty)
                })
                .collect();
            (cat, id_of(&f.name), params, type_repr(&f.return_type))
        })
        .collect();
    let data = program
        .data_decls
        .iter()
        .map(|d| {
            let vis = match d.visibility {
                DataVisibility::Shared => 0,
                DataVisibility::Private => 1,
                DataVisibility::Const => panic!("const data is a later increment"),
            };
            let fields = d
                .fields
                .iter()
                .map(|fld| (id_of(&fld.name), type_repr(&fld.type_expr)))
                .collect();
            (vis, id_of(&d.name), fields)
        })
        .collect();
    Parsed { funcs, data }
}

// A single function: the parser recognises one declaration and yields its name.
#[test]
fn a_single_function_is_recognised() {
    let src = "fn main() -> Word { 42 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs.len(), 1);
}

// Several functions in order, including parameters and a nested-brace body.
#[test]
fn functions_are_yielded_in_order() {
    let src = "fn inc(x: Word) -> Word { x + 1 } \
        fn choose(a: Word) -> Word { if a > 0 { a } else { 0 } } \
        fn main() -> Word { choose(inc(2)) }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs.len(), 3);
}

// A body with deeply nested braces (match arms, blocks) still ends at the correct
// closing brace, so the next declaration is recognised.
#[test]
fn nested_braces_do_not_confuse_the_boundary() {
    let src = "fn a(n: Word) -> Word { match n { 0 => 1, _ => n } } \
        fn b(n: Word) -> Word { if n > 0 { if n > 1 { 2 } else { 1 } } else { 0 } }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs.len(), 2);
}

// An empty program yields no declarations and reaches DONE immediately.
#[test]
fn an_empty_program_yields_no_declarations() {
    let mut names = Vec::new();
    let got = run_parser("", &mut names);
    assert!(got.funcs.is_empty());
    assert!(got.data.is_empty());
}

// The three function categories are distinguished by their keyword.
#[test]
fn categories_are_distinguished() {
    let src = "fn a() -> Word { 0 } \
        yield b(r: Word) -> Word { yield r } \
        loop c(r: Word) -> Word { yield r }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    // Categories: fn 1, yield 2, loop 3.
    assert_eq!(
        got.funcs.iter().map(|d| d.0).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

// The value parameters — names and types — and the return type are read, in order.
#[test]
fn parameter_names_types_and_return_type_are_read() {
    let src = "fn a() -> Word { 0 } \
        fn b(x: Byte) -> bool { x == x } \
        fn c(x: Word, y: Byte, z: bool) -> Word { 0 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    // Value-parameter counts, derived from the extracted names.
    assert_eq!(
        got.funcs.iter().map(|d| d.2.len()).collect::<Vec<_>>(),
        vec![0, 1, 3]
    );
    // `c`'s parameters are x: Word, y: Byte, z: bool; its return type is Word.
    let id = |s: &str| names.iter().position(|n| n == s).unwrap() as i64;
    assert_eq!(
        got.funcs[2].2,
        vec![
            (id("x"), TypeRepr::Named(id("Word"))),
            (id("y"), TypeRepr::Named(id("Byte"))),
            (id("z"), TypeRepr::Named(id("bool"))),
        ]
    );
    assert_eq!(got.funcs[2].3, TypeRepr::Named(id("Word")));
    // `b`'s return type is bool.
    assert_eq!(got.funcs[1].3, TypeRepr::Named(id("bool")));
}

// A type parameter before the value parentheses is not mistaken for a value
// parameter, and the value parameter's simple type is read.
#[test]
fn type_parameters_are_not_mistaken_for_parameters() {
    let src = "fn h<const n: Word>(x: Word) -> Word { x }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    // h has one value parameter (x: Word); the `const n` type parameter sits before
    // the value parentheses.
    assert_eq!(got.funcs[0].2.len(), 1);
    let id = |s: &str| names.iter().position(|n| n == s).unwrap() as i64;
    assert_eq!(got.funcs[0].2, vec![(id("x"), TypeRepr::Named(id("Word")))]);
}

// An array type `[T; N]` is read as an array of the element type with the literal
// length, in both a parameter position and the return position.
#[test]
fn array_types_are_read() {
    let src = "fn a(buf: [Word; 2048]) -> Word { 0 } \
        fn b(x: Word) -> [Byte; 16] { x }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    let id = |s: &str| names.iter().position(|n| n == s).unwrap() as i64;
    // `a`'s single parameter is an array `[Word; 2048]`; its return type is a simple
    // Word.
    assert_eq!(
        got.funcs[0].2,
        vec![(id("buf"), TypeRepr::Array(id("Word"), 2048))]
    );
    assert_eq!(got.funcs[0].3, TypeRepr::Named(id("Word")));
    // `b`'s parameter is a simple Word; its return type is an array `[Byte; 16]`.
    assert_eq!(got.funcs[1].2, vec![(id("x"), TypeRepr::Named(id("Word")))]);
    assert_eq!(got.funcs[1].3, TypeRepr::Array(id("Byte"), 16));
}

// Each head of a multiheaded function is a separate declaration; the reference
// lists each head, and the parser yields each, with the same category and params.
#[test]
fn a_multiheaded_function_yields_each_head() {
    let src = "yield g(r: Word) -> Word when r > 0 { yield r } \
        yield g(r: Word) -> Word { yield 0 }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    assert_eq!(got.funcs.len(), 2);
    // Both heads: yield category, one parameter named r.
    assert!(got.funcs.iter().all(|d| d.0 == 2 && d.2.len() == 1));
}

// A shared and a private data block are recognised, with their visibility, name, and
// fields (name and type, including an array-typed field). Interleaved with a function
// to prove the two declaration kinds are separated correctly.
#[test]
fn data_blocks_are_recognised() {
    let src = "shared data toks { len: Word, kinds: [Word; 2048] } \
        fn use_it(x: Word) -> Word { x } \
        private data ps { cursor: Word, flag: bool }";
    let mut names = Vec::new();
    let got = run_parser(src, &mut names);
    let want = reference(src, &names);
    assert_eq!(got, want);
    let id = |s: &str| names.iter().position(|n| n == s).unwrap() as i64;
    // Two data blocks, in source order, and the one function between them.
    assert_eq!(got.funcs.len(), 1);
    assert_eq!(got.data.len(), 2);
    // `shared data toks`: visibility 0, fields len: Word and kinds: [Word; 2048].
    assert_eq!(
        got.data[0],
        (
            0,
            id("toks"),
            vec![
                (id("len"), TypeRepr::Named(id("Word"))),
                (id("kinds"), TypeRepr::Array(id("Word"), 2048)),
            ]
        )
    );
    // `private data ps`: visibility 1, fields cursor: Word and flag: bool.
    assert_eq!(
        got.data[1],
        (
            1,
            id("ps"),
            vec![
                (id("cursor"), TypeRepr::Named(id("Word"))),
                (id("flag"), TypeRepr::Named(id("bool"))),
            ]
        )
    );
}
