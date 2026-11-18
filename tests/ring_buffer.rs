//! The compiler-managed ring buffer type `Ring<T, N>`, increment R1.
//!
//! R1 of `docs/decisions/RING_BUFFER_DESIGN.md` builds the type surface and the data
//! layout and no operation. These tests pin, against the reference compiler, the R1 row
//! of that design's schedule.
//!
//! - The type parses, and unifies only with a ring of a unifying element and an equal
//!   capacity, never with an array.
//! - One refusal per rule, each asserting that its diagnostic names the restriction.
//! - A ring field lowers to three hidden data fields, `r.len`, `r.head` and `r.buf`, in
//!   `2 + N` consecutive slots, with the private persistent bytes of the three ordinary
//!   fields it stands for.
//! - The byte offsets of a shared ring, including a `Byte` and a `bool` element after the
//!   two words, which the design marked as unverified.
//! - The typed schema hash distinguishes capacity, element type and a plain array.
//! - A private ring loads empty.
//!
//! The byte identity of the self-hosted compiler on ring declarations is pinned in
//! `tests/selfhost_full_language.rs`, by `ring_fields_have_their_pinned_self_hosted_outcomes`.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::ast::{ConstExpr, DataVisibility, PrimType, TypeExpr};
use keleusma::bytecode::{DataInit, Module, Op, SlotVisibility, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::typecheck::{ConstDim, Subst, Type, unify};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

fn compile_src(src: &str) -> Module {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).expect("parse");
    compile(&program).unwrap_or_else(|e| panic!("compile {src:?}: {}", e.message))
}

/// The compile error message for `src`, which must parse.
fn refusal(src: &str) -> String {
    let tokens = tokenize(src).expect("lex");
    let program = parse(&tokens).unwrap_or_else(|e| panic!("parse {src:?}: {}", e.message));
    match compile(&program) {
        Ok(_) => panic!("expected {src:?} to be refused"),
        Err(e) => e.message,
    }
}

/// Assert that `src` is refused with a message containing every one of `needles`.
fn assert_refused(src: &str, needles: &[&str]) {
    let message = refusal(src);
    for needle in needles {
        assert!(
            message.contains(needle),
            "refusal of {src:?} does not name {needle:?}: {message}"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Parsing and unification.

#[test]
fn ring_parses_as_a_built_in_type() {
    let src = "private data d { r: Ring<Word, 8> }\nfn main() -> Word { 0 }";
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let field = &program.data_decls[0].fields[0];
    assert_eq!(program.data_decls[0].visibility, DataVisibility::Private);
    match &field.type_expr {
        TypeExpr::Ring(elem, ConstExpr::Lit(8, _), _) => {
            assert!(matches!(**elem, TypeExpr::Prim(PrimType::Word, _)));
        }
        other => panic!("expected a ring type, got {other:?}"),
    }
}

#[test]
fn ring_capacity_parses_as_a_const_expression_and_a_qualified_element() {
    let src = "private data d { r: Ring<Fixed<16>, 2 + 3 * 2> }\nfn main() -> Word { 0 }";
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    match &program.data_decls[0].fields[0].type_expr {
        TypeExpr::Ring(elem, ConstExpr::Bin(..), _) => {
            assert!(matches!(
                **elem,
                TypeExpr::Prim(PrimType::Fixed(Some(16)), _)
            ));
        }
        other => panic!("expected a ring type with a const expression, got {other:?}"),
    }
    // The folded capacity is 8, so the element array has eight slots.
    let m = compile_src(src);
    let layout = m.data_layout.expect("layout");
    assert_eq!(layout.fields[2].name, "d.r.buf");
    assert_eq!(layout.fields[2].count, 8);
}

#[test]
fn ring_without_its_two_arguments_is_a_parse_error() {
    for src in [
        "private data d { r: Ring }\nfn main() -> Word { 0 }",
        "private data d { r: Ring<Word> }\nfn main() -> Word { 0 }",
    ] {
        let err = parse(&tokenize(src).expect("lex")).expect_err(src);
        assert!(err.message.contains("Ring<T, N>"), "{src:?}: {}", err.message);
    }
}

fn ring(elem: Type, n: i64) -> Type {
    Type::Ring(Box::new(elem), ConstDim::Known(n))
}

#[test]
fn ring_unifies_with_an_equal_ring_and_binds_its_element() {
    let mut s = Subst::new();
    unify(&ring(Type::Word, 8), &ring(Type::Word, 8), &mut s).expect("equal rings unify");
    assert!(s.is_empty());
    let mut s = Subst::new();
    unify(&ring(Type::Var(0), 8), &ring(Type::Byte, 8), &mut s).expect("variable element");
    assert_eq!(Type::Var(0).apply(&s), Type::Byte);
}

#[test]
fn ring_does_not_unify_across_capacity_element_or_with_an_array() {
    let cases = [
        (ring(Type::Word, 8), ring(Type::Word, 9)),
        (ring(Type::Word, 8), ring(Type::Byte, 8)),
        (
            ring(Type::Word, 8),
            Type::Array(Box::new(Type::Word), ConstDim::Known(8)),
        ),
        (
            Type::Array(Box::new(Type::Word), ConstDim::Known(10)),
            ring(Type::Word, 8),
        ),
        (ring(Type::Word, 1), Type::Word),
    ];
    for (a, b) in cases {
        let mut s = Subst::new();
        assert!(
            unify(&a, &b, &mut s).is_err(),
            "{} unified with {}",
            a.display(),
            b.display()
        );
    }
    assert_eq!(ring(Type::Fixed(32), 4).display(), "Ring<Fixed<32>, 4>");
}

// ---------------------------------------------------------------------------------------
// One refusal per rule.

#[test]
fn a_capacity_that_is_not_a_positive_constant_is_refused() {
    assert_refused(
        "private data d { r: Ring<Word, 0> }\nfn main() -> Word { 0 }",
        &["Ring capacity 0", "not positive"],
    );
    assert_refused(
        "private data d { r: Ring<Word, 0 - 1> }\nfn main() -> Word { 0 }",
        &["Ring capacity -1", "not positive"],
    );
    assert_refused(
        "shared data d { r: Ring<Word, n> }\nfn main() -> Word { 0 }",
        &["Ring capacity `n`", "not a constant"],
    );
}

#[test]
fn a_capacity_past_the_indexed_operand_bound_is_refused_at_the_boundary() {
    // `2 + N` must stay below 2^24, so N = 2^24 - 3 is the largest admitted.
    let at = "private data d { r: Ring<Byte, 16777213> }\nfn main() -> Word { 0 }";
    let layout = compile_src(at).data_layout.expect("layout");
    assert_eq!(layout.slot_count(), 16_777_215);
    assert_refused(
        "private data d { r: Ring<Byte, 16777214> }\nfn main() -> Word { 0 }",
        &["Ring capacity 16777214", "too large", "2 + N"],
    );
}

#[test]
fn an_element_outside_the_five_scalars_is_refused() {
    for elem in [
        "Text",
        "Text<4>",
        "(Word, Word)",
        "[Word; 2]",
        "Option<Word>",
        "P",
        "Multiword<2>",
        "Ring<Word, 2>",
        "()",
    ] {
        let src = format!(
            "struct P {{ x: Word }}\nprivate data d {{ r: Ring<{elem}, 4> }}\nfn main() -> Word {{ 0 }}"
        );
        assert_refused(
            &src,
            &[
                "Ring element type",
                "Word, Byte, bool, Fixed and Float",
            ],
        );
    }
}

#[test]
fn every_admissible_element_compiles() {
    #[cfg(feature = "floats")]
    let elems: &[&str] = &["Word", "Byte", "bool", "Fixed", "Fixed<8>", "Float"];
    #[cfg(not(feature = "floats"))]
    let elems: &[&str] = &["Word", "Byte", "bool", "Fixed", "Fixed<8>"];
    for elem in elems {
        for vis in ["private", "shared"] {
            let src = format!("{vis} data d {{ r: Ring<{elem}, 3> }}\nfn main() -> Word {{ 0 }}");
            let layout = compile_src(&src).data_layout.expect("layout");
            assert_eq!(layout.slot_count(), 5, "{src}");
        }
    }
}

/// Every position other than the whole type of a `private data` or `shared data` field,
/// enumerated by class: signatures, bodies, declarations and nested data field types.
#[test]
fn a_ring_in_any_other_position_is_refused() {
    let cases = [
        (
            "fn main() -> Word { let r: Ring<Word, 4> = 0; 0 }",
            "a local binding",
        ),
        (
            "fn f(r: Ring<Word, 4>) -> Word { 0 }\nfn main() -> Word { 0 }",
            "a function parameter",
        ),
        (
            "fn f() -> Ring<Word, 4> { 0 }\nfn main() -> Word { 0 }",
            "a function return or yield type",
        ),
        (
            "loop main(x: Word) -> Ring<Word, 4> { let y = yield 0; }",
            "a function return or yield type",
        ),
        (
            "struct S { r: Ring<Word, 4> }\nfn main() -> Word { 0 }",
            "a struct field",
        ),
        (
            "enum E { A(Ring<Word, 4>) }\nfn main() -> Word { 0 }",
            "an enum variant payload",
        ),
        (
            "newtype N = Ring<Word, 4>;\nfn main() -> Word { 0 }",
            "a newtype",
        ),
        (
            "private data d { r: (Ring<Word, 4>, Word) }\nfn main() -> Word { 0 }",
            "a nested position of a data field type",
        ),
        (
            "private data d { r: [Ring<Word, 4>; 2] }\nfn main() -> Word { 0 }",
            "a nested position of a data field type",
        ),
        (
            "shared data d { r: Option<Ring<Word, 4>> }\nfn main() -> Word { 0 }",
            "a nested position of a data field type",
        ),
        (
            "struct G<T> { x: T }\nprivate data d { r: G<Ring<Word, 4>> }\nfn main() -> Word { 0 }",
            "a nested position of a data field type",
        ),
        (
            "fn main() -> Word { let t: (Word, Ring<Word, 4>) = (0, 0); 0 }",
            "a local binding",
        ),
        (
            "use host::f(Ring<Word, 4>) -> Word\nfn main() -> Word { 0 }",
            "a native signature",
        ),
        (
            "fn main() -> Word { (0 as Ring<Word, 4>); 0 }",
            "a cast target",
        ),
        (
            "trait T { fn f(r: Ring<Word, 4>) -> Word; }\nfn main() -> Word { 0 }",
            "a trait method parameter",
        ),
    ];
    for (src, position) in cases {
        let message = match parse(&tokenize(src).expect("lex")) {
            Ok(program) => match compile(&program) {
                Ok(_) => panic!("expected {src:?} to be refused"),
                Err(e) => e.message,
            },
            Err(e) => panic!("{src:?} did not parse: {}", e.message),
        };
        assert!(
            message.contains(position) && message.contains("only as the declared type"),
            "refusal of {src:?} does not name {position:?}: {message}"
        );
    }
}

#[test]
fn a_ring_in_a_const_data_block_is_refused() {
    assert_refused(
        "const data c { r: Ring<Word, 4> = 0 }\nfn main() -> Word { 0 }",
        &["`const data`", "array"],
    );
}

#[test]
fn an_initializer_on_a_ring_field_is_refused() {
    assert_refused(
        "private data d { r: Ring<Word, 4> = 0 }\nfn main() -> Word { 0 }",
        &["ring field `d.r`", "initializer"],
    );
}

/// No ring operation exists in R1, so every expression form that reaches a ring field,
/// read, write, index, indexed write, method call and iteration, is refused.
#[test]
fn every_expression_naming_a_ring_field_is_refused() {
    let decl = "private data d { r: Ring<Word, 4>, x: Word }\n";
    for body in [
        "fn main() -> Word { d.r }",
        "fn main() -> Word { d.r = 1; 0 }",
        "fn main() -> Word { d.r[0] }",
        "fn main() -> Word { d.r[0] = 1; 0 }",
        "fn main() -> Word { d.r.len() }",
        "fn main() -> Word { d.r.push(1); 0 }",
        "fn main() -> Word { for v in d.r { d.x = v; } 0 }",
        "fn main() -> Word { let s = d.r; 0 }",
    ] {
        let src = format!("{decl}{body}");
        assert_refused(
            &src,
            &["ring field `d.r`", "ring operations are not yet implemented"],
        );
    }
}

// ---------------------------------------------------------------------------------------
// Layout.

#[test]
fn a_ring_field_takes_three_hidden_fields_in_two_plus_n_slots() {
    let m = compile_src(
        "private data d { a: Word, r: Ring<Word, 5>, b: Byte }\nfn main() -> Word { d.b as Word }",
    );
    let layout = m.data_layout.expect("layout");
    let summary: Vec<(&str, u32, u32)> = layout
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.first_slot, f.count))
        .collect();
    assert_eq!(
        summary,
        [
            ("d.a", 0, 1),
            ("d.r.len", 1, 1),
            ("d.r.head", 2, 1),
            ("d.r.buf", 3, 5),
            ("d.b", 8, 1),
        ]
    );
    assert_eq!(layout.private_slot_count(), 9);
    // The ring is three fields, so the field after it is operand four.
    let main = m.chunks.iter().find(|c| c.name == "main").expect("main");
    assert!(main.ops.contains(&Op::GetData(4)), "{:?}", main.ops);
    // Neither the ring's own name nor a source-shaped name addresses a hidden field.
    assert_eq!(layout.slot_of("d.r"), None);
    assert_eq!(layout.slot_of("d.r.buf"), Some(3));
}

/// The ring costs exactly the private bytes of the three ordinary fields it stands for,
/// which is what the expansion promises, and those bytes are pinned for one case.
#[test]
fn a_private_ring_costs_the_persistent_bytes_of_its_three_fields() {
    let elems: &[(&str, u32)] = &[("Word", 8), ("Byte", 1), ("bool", 1), ("Fixed", 8)];
    for &(elem, width) in elems {
        for n in [1u32, 5, 8] {
            let ring = compile_src(&format!(
                "private data d {{ r: Ring<{elem}, {n}> }}\nfn main() -> Word {{ 0 }}"
            ));
            let plain = compile_src(&format!(
                "private data d {{ a: Word, b: Word, c: [{elem}; {n}] }}\nfn main() -> Word {{ 0 }}"
            ));
            let (rl, pl) = (
                ring.data_layout.as_ref().expect("layout"),
                plain.data_layout.as_ref().expect("layout"),
            );
            assert_eq!(rl.private_slot_count(), 2 + n as usize);
            assert_eq!(rl.private_slot_count(), pl.private_slot_count());
            assert_eq!(ring.private_data_bytes, plain.private_data_bytes, "{elem} {n}");
            assert_eq!(
                required_persistent_capacity_for(&ring),
                required_persistent_capacity_for(&plain),
                "{elem} {n}"
            );
            // Packed private storage at the host's 64-bit word: two words, then N
            // elements of the element's width.
            assert_eq!(ring.private_data_bytes, 16 + n * width, "{elem} {n}");
            assert_eq!(ring.shared_data_bytes, 0);
        }
    }
}

/// The host-buffer byte offsets of a shared ring. The design marked the padding of a
/// `Byte` or `bool` element after the two words as unverified. Measured here: there is
/// none, the shared buffer is packed, so the elements start immediately after the head
/// word and a following field starts immediately after the last element.
#[test]
fn a_shared_ring_has_pinned_byte_offsets() {
    for (elem, width) in [("Word", 8u32), ("Byte", 1), ("bool", 1)] {
        let m = compile_src(&format!(
            "shared data io {{ a: Byte, r: Ring<{elem}, 3>, b: Word }}\nfn main() -> Word {{ io.b }}"
        ));
        let layout = m.data_layout.as_ref().expect("layout");
        let at = |name: &str| {
            let f = layout
                .fields
                .iter()
                .find(|f| f.name == name)
                .unwrap_or_else(|| panic!("no field {name}"));
            assert_eq!(f.visibility, SlotVisibility::Shared);
            (f.offset.expect("shared offset"), f.stride, f.count)
        };
        assert_eq!(at("io.a"), (0, 0, 1), "{elem}");
        assert_eq!(at("io.r.len"), (1, 0, 1), "{elem}");
        assert_eq!(at("io.r.head"), (9, 0, 1), "{elem}");
        assert_eq!(at("io.r.buf"), (17, width, 3), "{elem}");
        assert_eq!(at("io.b"), (17 + 3 * width, 0, 1), "{elem}");
        assert_eq!(m.shared_data_bytes, 17 + 3 * width + 8, "{elem}");
        // Element by element, through the host's slot space.
        let first = layout.slot_of("io.r.buf").expect("buffer slot");
        for k in 0..3u32 {
            let slot = layout.shared_slot(first + k as usize).expect("element slot");
            assert_eq!(slot.offset, 17 + k * width, "{elem} element {k}");
        }
    }
}

/// A host addresses a shared ring's parts by their full names, and a write through one
/// part lands in that part's bytes alone.
#[test]
fn a_host_reads_and_writes_a_shared_ring_by_its_hidden_names() {
    let m = compile_src("shared data io { r: Ring<Word, 2>, x: Word }\nfn main() -> Word { io.x }");
    let layout = m.data_layout.clone().expect("layout");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let vm = Vm::new(m, &arena).expect("load");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    assert_eq!(shared.len(), 5 * 8);
    let len = layout.slot_of("io.r.len").expect("len");
    let head = layout.slot_of("io.r.head").expect("head");
    vm.set_shared(&mut shared, len, Value::Int(2)).expect("set len");
    vm.set_shared(&mut shared, head, Value::Int(1)).expect("set head");
    assert_eq!(vm.get_shared(&shared, len).expect("get len"), Value::Int(2));
    assert_eq!(vm.get_shared(&shared, head).expect("get head"), Value::Int(1));
    assert_eq!(&shared[0..8], &2i64.to_le_bytes());
    assert_eq!(&shared[8..16], &1i64.to_le_bytes());
    assert!(shared[16..].iter().all(|&b| b == 0));
}

// ---------------------------------------------------------------------------------------
// The typed schema hash.

#[test]
fn the_schema_hash_distinguishes_capacity_element_and_a_plain_array() {
    for vis in ["shared", "private"] {
        let hash = |ty: &str| {
            compile_src(&format!(
                "{vis} data s {{ r: {ty} }}\nfn main() -> Word {{ 0 }}"
            ))
            .schema_hash
        };
        let hashes = [
            hash("Ring<Word, 8>"),
            hash("Ring<Word, 9>"),
            hash("Ring<Byte, 8>"),
            hash("[Word; 10]"),
        ];
        for (i, a) in hashes.iter().enumerate() {
            assert_ne!(*a, 0, "{vis} hash {i} is the no-data sentinel");
            for b in &hashes[i + 1..] {
                assert_ne!(a, b, "{vis}: two distinct declarations share a hash: {hashes:?}");
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Load.

/// A private ring loads empty. The layout gives its three fields the zero initializer,
/// and a run reads each of them back as zero. No source form reads a ring field in R1,
/// so the run replaces the operand of a compiled `GetData` of the field after the ring
/// with the operand of each hidden word and of element zero, which is the same
/// instruction over a field of the same kind.
#[test]
fn a_private_ring_loads_empty() {
    let src = "private data d { r: Ring<Word, 4>, x: Word }\nfn main() -> Word { d.x }";
    let m = compile_src(src);
    let layout = m.data_layout.clone().expect("layout");
    for name in ["d.r.len", "d.r.head", "d.r.buf"] {
        let f = layout.fields.iter().find(|f| f.name == name).expect(name);
        assert_eq!(f.init, DataInit::Zero, "{name}");
        assert_eq!(f.visibility, SlotVisibility::Private, "{name}");
    }
    let main_at = m.chunks.iter().position(|c| c.name == "main").expect("main");
    assert!(m.chunks[main_at].ops.contains(&Op::GetData(3)));
    for operand in [0u32, 1, 2] {
        let mut patched = m.clone();
        for op in &mut patched.chunks[main_at].ops {
            if *op == Op::GetData(3) {
                *op = Op::GetData(operand);
            }
        }
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        arena
            .resize_persistent(required_persistent_capacity_for(&patched))
            .expect("persistent");
        let mut vm = Vm::new(patched, &arena).expect("load");
        match vm.call(&[]).expect("run") {
            VmState::Finished(v) => assert_eq!(v, Value::Int(0), "operand {operand}"),
            other => panic!("operand {operand}: {other:?}"),
        }
    }
}
