//! The hot-swap schema hash covers each data slot's declared type.
//!
//! The operator ruled on 2026-10-28 that the schema hash cover slot types.
//! Before, it covered slot names and visibility only, so a swap between two
//! modules whose data fields shared names but not types, a reordered struct or
//! a `Word` field become `Byte`, passed `Vm::replace_module` and then read the
//! carried values by the wrong layout.
//!
//! # What these tests do NOT establish
//!
//! An enum's own definition is not expanded into the token, so a change inside
//! an enum used as a data field type does not change the hash.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, required_persistent_capacity_for};

fn compiled(source: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse"))
        .unwrap_or_else(|e| panic!("{source}\n{}", e.message))
}

const BODY: &str = "fn main() -> Word { d.n = d.n + 1; d.n }";

fn with_data(decls: &str) -> Module {
    compiled(&format!("{decls}\n{BODY}"))
}

#[test]
fn a_type_change_with_the_same_names_changes_the_hash() {
    let base = with_data("struct P { x: Word, y: Byte }\nprivate data d { p: P, n: Word }");
    let reordered = with_data("struct P { y: Byte, x: Word }\nprivate data d { p: P, n: Word }");
    let retyped = with_data("struct P { x: Byte, y: Byte }\nprivate data d { p: P, n: Word }");
    let scalar = with_data(
        "struct P { x: Word, y: Byte }\nprivate data d { p: P, n: Word }\nshared data s { w: Byte }",
    );
    let scalar_word = with_data(
        "struct P { x: Word, y: Byte }\nprivate data d { p: P, n: Word }\nshared data s { w: Word }",
    );
    assert_ne!(base.schema_hash, 0);
    assert_ne!(base.schema_hash, reordered.schema_hash);
    assert_ne!(base.schema_hash, retyped.schema_hash);
    assert_ne!(scalar.schema_hash, scalar_word.schema_hash);
}

#[test]
fn the_hash_does_not_depend_on_where_the_declarations_sit() {
    let a = with_data("struct P { x: Word, y: Byte }\nprivate data d { p: P, n: Word }");
    let b = with_data(
        "\n\n// moved\nstruct P {\n    x: Word,\n    y: Byte,\n}\n\nprivate data d { p: P, n: Word }",
    );
    assert_eq!(a.schema_hash, b.schema_hash);
}

#[test]
fn a_swap_to_a_reordered_struct_is_refused() {
    let first = with_data("struct P { x: Word, y: Byte }\nprivate data d { p: P, n: Word }");
    let second = with_data("struct P { y: Byte, x: Word }\nprivate data d { p: P, n: Word }");
    let need =
        required_persistent_capacity_for(&first).max(required_persistent_capacity_for(&second));
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(first, &arena).expect("verifies");
    let err = vm
        .replace_module(second, vec![Value::Unit, Value::Int(0)])
        .expect_err("the reordered struct changes the schema");
    assert!(format!("{err:?}").contains("schema mismatch"), "{err:?}");
}

/// The self-hosted compiler computes the same hash, byte for byte, which the
/// artifact comparison checks along with every other field.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_computes_the_same_hash() {
    use keleusma::selfhost::{compile_src, self_hosted_compile};
    use keleusma::target::Target;
    for source in [
        "struct P { x: Word, y: Byte }\nprivate data d { p: P, n: Word }\nfn main() -> Word { d.n = d.n + 1; d.n }",
        "shared data s { a: [Word; 3], w: Word }\nprivate data d { n: Word, b: [Byte; 2] }\nfn main() -> Word { d.n = s.w; d.n }",
    ] {
        let reference = compile_src(source);
        let actual = self_hosted_compile(source, &Target::host())
            .unwrap_or_else(|e| panic!("{source}: {e}"));
        assert_eq!(actual.schema_hash, reference.schema_hash, "{source}");
        assert_eq!(
            actual.to_bytes().unwrap(),
            reference.to_bytes().unwrap(),
            "{source}"
        );
    }
}
