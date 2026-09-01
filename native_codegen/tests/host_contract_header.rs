//! **DOES THE EMITTED ARTEFACT STATE WHAT THE HOST MUST SUPPLY?**
//!
//! The linkage census found that 43 of 45 undefined symbols across the corpus
//! are host-registered natives — the embedder's half of the contract — and that
//! the generated header declared **none of them**. An embedder met the
//! requirement at link time, as a mangled undefined symbol.
//!
//! # What the declarations actually buy, measured rather than assumed
//!
//! **The obvious claim is false.** Declaring a native does NOT turn a MISSING
//! definition into a compile error; that stays a link error. What it catches is
//! a **MISMATCHED** one. Verified both ways with a C compiler: a host defining
//! the native with the wrong arity compiles **clean** without the declaration
//! and fails with `conflicting types` with it.
//!
//! **That is the more valuable guarantee**, and it makes the boundary
//! symmetric: the backend already refuses a module whose two call sites disagree
//! on a native's arity, because LLVM would accept the call and the host would
//! read a garbage argument. The host side now checks the same thing.
//!
//! # Why the list is read off the lowered module
//!
//! A module records native NAMES and return shapes. **Arity comes from the call
//! sites and is resolved during lowering, so it exists nowhere else.** Emitting
//! a prototype from the name alone would have been an invented signature, and C
//! would have accepted the mismatched call.

mod common;

use inkwell::context::Context;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, NATIVE_SYMBOL_PREFIX, host_native_declarations, lower_module};

fn witness_source() -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../examples/scripts/external_native_witness.kel"),
    )
    .expect("the external-native witness module")
}

fn declarations_for(src: &str) -> Vec<(String, u32, bool)> {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    host_native_declarations(&lm)
}

/// **NON-VACUITY.** Every claim below is about a list; all of them hold for an
/// empty one.
#[test]
fn a_module_with_a_native_reports_it_with_its_arity() {
    let decls = declarations_for(&witness_source());
    assert!(
        !decls.is_empty(),
        "the witness module declares a native and none was reported, so every \
         other check here is vacuous"
    );
    let (name, argc, returns_float) = &decls[0];
    assert!(
        name.starts_with(NATIVE_SYMBOL_PREFIX),
        "the reported symbol {name} does not carry the host-contract prefix"
    );
    assert_eq!(*argc, 1, "the witness native takes one argument");
    assert!(!returns_float, "the witness native returns a word");
}

/// **CONTROL, must-fire.** Without it, the list could be non-empty because the
/// extractor reports every function rather than only the host's.
#[test]
fn a_module_with_no_natives_reports_none() {
    let decls = declarations_for("fn main(w: Word) -> Word {\n  let d = w + 1;\n  w * d\n}\n");
    assert!(
        decls.is_empty(),
        "a module calling no native reported {decls:?}; the extractor is \
         reporting functions this backend DEFINES, which are not the host's \
         contract"
    );
}

/// **THE PROPERTY THE HEADER EXISTS FOR, CHECKED BY A C COMPILER.**
///
/// A declaration is a claim that a compiler accepts it and that it constrains a
/// definition. Reading the generated text establishes neither.
#[test]
fn the_declaration_rejects_a_mismatched_definition_and_a_bare_one_does_not() {
    let decls = declarations_for(&witness_source());
    let (name, _, _) = &decls[0];

    let dir = std::env::temp_dir().join(format!("kel_hdr_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");

    // The header's shape for this native, from the same source the example uses.
    let decl = format!("#include <stdint.h>\nint64_t {name}(int64_t);\n");
    // A host defining it with the WRONG arity.
    let wrong_def = format!("int64_t {name}(int64_t a, int64_t b){{return a+b;}}\n");

    let with = dir.join("with.c");
    std::fs::write(&with, format!("{decl}{wrong_def}")).expect("write");
    let without = dir.join("without.c");
    std::fs::write(&without, format!("#include <stdint.h>\n{wrong_def}")).expect("write");

    let compile_c = |src: &std::path::Path| -> bool {
        std::process::Command::new("cc")
            .arg("-c")
            .arg("-o")
            .arg(src.with_extension("o"))
            .arg(src)
            .output()
            .expect("a C compiler is required for this test")
            .status
            .success()
    };

    assert!(
        compile_c(&without),
        "the mismatched definition failed to compile even WITHOUT the \
         declaration, so this test is not measuring what the declaration adds"
    );
    assert!(
        !compile_c(&with),
        "the mismatched definition compiled WITH the declaration present. The \
         declaration is not constraining the definition, so the header buys \
         nothing over a comment."
    );
}
