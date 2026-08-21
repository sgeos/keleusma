//! The shipping self-hosted compiler's constant pool, and the boundary it actually has.
//!
//! # Why one file holds both
//!
//! They were found by the same act. `codegen.kel` emits a tagged constant pool and the
//! shipping driver dropped the tags; looking for what else the driver dropped found that it
//! also dropped every `struct`, `trait` and `impl` DECLARATION, and the two together were
//! the difference between the shipping compiler and the copy of it in
//! `tests/selfhost_codegen.rs` that the construct-support boundary actually measures.
//!
//! # The measurement that motivated all of it
//!
//! The 95 boundary cases in `tests/selfhost_codegen.rs`, run through
//! `keleusma::selfhost::self_host_compile` and compared against the reference:
//!
//! | | before | after |
//! |---|---|---|
//! | byte-identical | 43 | **76** |
//! | differs | 21 | 11 |
//! | faults | 30 | 7 |
//! | reference rejects | 1 | 1 |
//!
//! **Every case that differs after also differed before.** Nothing here made a case worse;
//! the residue is pre-existing and is pinned below so it stays visible.
//!
//! # Proportionality, which belongs beside every claim in this file
//!
//! `self_hosted_compile` — the `--compiler self-hosted` CLI backend — cross-checks against
//! the reference and refuses on divergence. **None of this reached a user as a wrong module.**
//! The exposure was to direct callers of the `self_host_compile*` entry points.

#![cfg(all(feature = "self-host", feature = "compile"))]

use keleusma::bytecode::{ConstValue, Module};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn reference(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("reference compile")
}

/// Every chunk's constant pool, flattened. Chunk order is name-keyed in both compilers.
fn pool(m: &Module) -> Vec<ConstValue> {
    m.chunks.iter().flat_map(|c| c.constants.clone()).collect()
}

/// Ops, constant pool and local count, which is what `self_hosted_compile` cross-checks.
fn agrees(a: &Module, b: &Module) -> bool {
    a.chunks.len() == b.chunks.len()
        && a.chunks.iter().zip(b.chunks.iter()).all(|(x, y)| {
            x.name == y.name
                && x.ops == y.ops
                && x.constants == y.constants
                && x.local_count == y.local_count
        })
}

// -- the three tags ---------------------------------------------------------------------

/// **A STRING CONSTANT IS A STRING, NOT THE INTEGER OF ITS INTERN ID.**
///
/// `codegen.kel` interns a `StaticStr` as tag 1 carrying the LEXER INTERN ID, leaving the
/// host to resolve it. The shipping driver read the tag stream into a discard binding and
/// rebuilt every entry as `ConstValue::Int`, so this program produced `Int(3)`.
///
/// # The negative half is the one that would have caught it
///
/// Asserting equality with the reference is enough today. Asserting that nothing in the pool
/// is the bare intern id is what fails loudly if the resolution is ever removed again, and it
/// is written out rather than left implicit because the original defect was invisible for
/// exactly as long as nobody looked for it.
#[test]
fn a_string_constant_resolves_to_its_bytes_rather_than_its_intern_id() {
    const SRC: &str = "fn f() -> Word { let s = \"hi\"; 1 }";
    let r = reference(SRC);
    let lib = keleusma::selfhost::self_host_compile(SRC);

    assert!(
        pool(&r)
            .iter()
            .any(|c| matches!(c, ConstValue::StaticStr(s) if s == "hi")),
        "the reference no longer bakes a StaticStr for this source, so this test measures \
         something other than what it was written for"
    );
    assert_eq!(pool(&lib), pool(&r));
    assert!(
        !pool(&lib).iter().any(|c| matches!(c, ConstValue::Int(3))),
        "a pool entry came back as the bare intern id, which is the original defect"
    );
}

/// **THE ESCAPE IS PART OF THE CONTRACT, SO THE WITNESS CARRIES ONE.**
///
/// The lexer's name table holds a literal's content AS WRITTEN, so a newline escape is two
/// characters there and one byte in what the reference bakes. A witness without an escape
/// would pass against a host that resolved the id and never unescaped it, which is a
/// different wrong answer from the one this file closed and would have shipped just as
/// quietly.
#[test]
fn a_string_constant_carries_the_escapes_the_reference_bakes() {
    for (src, want) in [
        ("fn f() -> Word { let s = \"a\\nb\"; 1 }", "a\nb"),
        ("fn f() -> Word { let s = \"a\\tb\"; 1 }", "a\tb"),
        ("fn f() -> Word { let s = \"q\\\"z\"; 1 }", "q\"z"),
        ("fn f() -> Word { let s = \"b\\\\c\"; 1 }", "b\\c"),
    ] {
        let r = reference(src);
        assert!(
            pool(&r)
                .iter()
                .any(|c| matches!(c, ConstValue::StaticStr(s) if s == want)),
            "the reference does not bake {want:?} for {src:?}; the case is mis-stated"
        );
        assert_eq!(
            pool(&keleusma::selfhost::self_host_compile(src)),
            pool(&r),
            "escape handling diverged on {src:?}"
        );
    }
}

/// **THE BOOLEAN TAG IS WITNESSED WITHOUT A STRUCT, WHICH IS NOT WHERE I EXPECTED TO FIND IT.**
///
/// `intern_bool` is documented as serving `push_struct_eq`, and a struct declaration is the
/// hardest witness to build here. It is also unnecessary: TUPLE, ARRAY and ENUM equality all
/// lower through the same field-wise comparison and bake the same `Bool(false)`/`Bool(true)`
/// results, and none of them needs a `struct`.
///
/// Recorded because the first plan for this test was a struct-equality witness, which would
/// have made the tag look unreachable while the shipping driver refused struct declarations.
/// The general lesson is the one this line keeps paying for: **the construct that reaches a
/// branch is not always the construct the branch is named after.**
#[test]
fn a_boolean_constant_is_a_boolean_and_needs_no_struct_to_witness_it() {
    for src in [
        "fn f(a: (Word,Word), b: (Word,Word)) -> bool { a == b }",
        "fn f(a: [Word;3], b: [Word;3]) -> bool { a == b }",
    ] {
        let r = reference(src);
        assert!(
            pool(&r).iter().any(|c| matches!(c, ConstValue::Bool(_))),
            "no Bool in the reference pool for {src:?}, so this witnesses nothing"
        );
        assert_eq!(pool(&keleusma::selfhost::self_host_compile(src)), pool(&r));
    }
}

/// **ONE SOURCE EXERCISES ALL THREE TAGS AT ONCE.**
///
/// An all-unit enum equality bakes the enum and variant NAMES as `StaticStr`, the
/// discriminants as `Int`, and the comparison results as `Bool` — tags 1, 0 and 2 in one
/// pool. A single divergence anywhere in the tag mapping fails here, which is a stronger
/// guard than three single-tag cases because it also pins the ORDER the stage interns them in.
#[test]
fn one_enum_equality_exercises_every_tag_the_stage_interns() {
    const SRC: &str = "enum E { A, B }\nfn f(a: E, b: E) -> bool { a == b }";
    let r = reference(SRC);
    let p = pool(&r);
    assert!(
        p.iter().any(|c| matches!(c, ConstValue::StaticStr(_))),
        "no tag-1 entry"
    );
    assert!(
        p.iter().any(|c| matches!(c, ConstValue::Int(_))),
        "no tag-0 entry"
    );
    assert!(
        p.iter().any(|c| matches!(c, ConstValue::Bool(_))),
        "no tag-2 entry"
    );
    assert!(agrees(&keleusma::selfhost::self_host_compile(SRC), &r));
}

/// **EVERY ENTRY POINT, NOT JUST THE ONE THAT WAS LOOKED AT.**
///
/// The tag was dropped at three call sites because each rebuilt the pool inline. They are now
/// one function, and this test is what makes that stay true: a fourth entry point that
/// reintroduces an inline `ConstValue::Int` map passes every other test in this file.
///
/// `self_host_compile_scratch` assembles a whole module rather than splicing onto the
/// reference scaffold, so its chunk set is compared by pool rather than by whole-module
/// agreement.
#[test]
fn every_shipping_entry_point_carries_the_tags() {
    for src in [
        "fn f() -> Word { let s = \"hi\"; 1 }",
        "fn f(a: (Word,Word), b: (Word,Word)) -> bool { a == b }",
        "enum E { A, B }\nfn f(a: E, b: E) -> bool { a == b }",
    ] {
        let want = pool(&reference(src));
        assert_eq!(
            pool(&keleusma::selfhost::self_host_compile(src)),
            want,
            "splice: {src:?}"
        );
        assert_eq!(
            pool(&keleusma::selfhost::self_host_compile_fused(src)),
            want,
            "fused: {src:?}"
        );
        assert_eq!(
            pool(&keleusma::selfhost::self_host_compile_scratch(src)),
            want,
            "scratch: {src:?}"
        );
    }
}

// -- the declaration skip ---------------------------------------------------------------

/// **A `struct` DECLARATION NO LONGER STOPS THE SHIPPING COMPILER.**
///
/// `parse.kel` emits STRUCTSTART/TRAITSTART/IMPLSTART followed by the declaration's own
/// parameter records. The shipping driver had no state for them, so those records reached the
/// function dispatch with nothing open and it panicked by name — on every one of the 29
/// boundary cases whose source declares a struct, 27 of which the boundary records as `Ok`.
///
/// The copy in `tests/selfhost_codegen.rs` carried the skip all along, which is why the
/// boundary never saw this. **The boundary measures the copy.**
#[test]
fn a_struct_declaration_compiles_rather_than_faulting() {
    for src in [
        "struct P { x: Word }\nfn f() -> P { P { x: 1 } }",
        "struct P { x: Word }\nfn f(a: P, b: P) -> bool { a == b }",
        "struct P { x: Word, y: Byte }\nfn f(a: P) -> Word { a.x }",
    ] {
        assert!(
            agrees(&keleusma::selfhost::self_host_compile(src), &reference(src)),
            "diverged on {src:?}"
        );
    }
}

// -- the residue, pinned so it stays visible ---------------------------------------------

/// **SIX CONSTRUCTS THE BOUNDARY CALLS `Ok` THAT THE SHIPPING COMPILER MISCOMPILES.**
///
/// Eager boolean operators and their precedence. Found by the census, PRE-EXISTING, and
/// deliberately not repaired here: the fix is in operator lowering and has nothing to do with
/// the constant pool, and repairing it in the same change would make this file's before/after
/// measurement unattributable.
///
/// # Why this is pinned in the FAILING direction
///
/// A test asserting the divergence fails the day it is fixed, which is the notice that the
/// count in this file's header is stale. Deleting it instead would lose the record that six
/// `Ok` verdicts were wrong.
///
/// The `and`/`or` here are the EAGER operators, not `andalso`/`orelse`.
#[test]
fn the_shipping_compiler_still_diverges_on_the_eager_boolean_operators() {
    let known = [
        "fn f(a: bool, b: bool) -> bool { a and b }",
        "fn f(a: bool, b: bool) -> bool { a or b }",
        "fn f(a: bool, b: bool, c: bool) -> bool { a or b and c }",
        "fn f(a: bool, b: bool, c: bool) -> bool { a or b xor c }",
        "fn f(x: bool, y: Word, z: Word) -> bool { x and y < z }",
        "fn f(a: bool, b: bool, c: bool) -> bool { a and b xor c }",
    ];
    for src in known {
        assert!(
            !agrees(&keleusma::selfhost::self_host_compile(src), &reference(src)),
            "{src:?} now AGREES with the reference. That is a repair, not a failure: fold it \
             into the ordinary coverage, drop it from this list, and correct the census in \
             this file's header"
        );
    }
    // The control. Without it a compiler that diverged on everything would pass above.
    let control = "fn f(a: bool, b: bool) -> bool { a andalso b }";
    assert!(
        agrees(
            &keleusma::selfhost::self_host_compile(control),
            &reference(control)
        ),
        "the short-circuit control diverges too, so the list above does not isolate the \
         EAGER operators and this test attributes the divergence wrongly"
    );
}

/// **A TUPLE WHOSE ELEMENT IS A STRUCT STILL FAULTS, AND IT IS A DIFFERENT FAULT.**
///
/// These six stopped panicking in the declaration dispatch and now fault deeper, in scalar
/// kind decoding (`bad scalar kind tag 131080`). That is a distinct, unfixed gap and not a
/// regression: before the declaration skip they never reached the code that faults.
///
/// Pinned as "faults" rather than "diverges" because the two are different verdicts and this
/// line has recorded four separate occasions where a shared failure message hid two causes.
#[test]
fn a_tuple_element_of_struct_type_still_faults_in_the_shipping_compiler() {
    let known = [
        "struct P { x: Word }\nfn f(a: (P, Word), b: (P, Word)) -> bool { a == b }",
        "struct P { x: Word }\nstruct S { t: (P, Word) }\nfn f(a: S, b: S) -> bool { a == b }",
        "struct P { u: (Word, Word) }\nstruct S { t: (P, Word) }\nfn f(a: S, b: S) -> bool { a == b }",
        "struct P { x: Word }\nfn f(a: [(P, Word); 2], b: [(P, Word); 2]) -> bool { a == b }",
    ];
    for src in known {
        let r = std::panic::catch_unwind(|| keleusma::selfhost::self_host_compile(src));
        assert!(
            r.is_err(),
            "{src:?} no longer faults. If it now compiles, move it out of this list and \
             correct the census in this file's header"
        );
    }
    // The control: a tuple of SCALARS compiles, so the fault above is attributable to the
    // struct element rather than to tuples in general.
    let control = "fn f(a: (Word,Word), b: (Word,Word)) -> bool { a == b }";
    assert!(agrees(
        &keleusma::selfhost::self_host_compile(control),
        &reference(control)
    ));
}
