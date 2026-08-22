//! Indexing an array whose elements are arrays: `a[0][1]`.
//!
//! # What was wrong
//!
//! `parse.kel` emitted records, and they were the WRONG records:
//!
//! ```text
//!   a[1]      ->  Local(0), Literal(1), Index                 -- correct
//!   a[0][1]   ->  Local(0), Literal(0), Index, Literal(1), ArrayLit
//! ```
//!
//! The second `[1]` parsed as an ARRAY LITERAL. A `let` binding recorded its value as an array
//! of WORDS whatever the elements actually were, so the first index emitted a scalar read and
//! nothing armed a second one; the following `[` then fell through to the literal branch.
//!
//! # The recorded cost estimate was wrong, and checking is what made this tractable
//!
//! The specification carried by the construct-support table said a fix needed three coordinated
//! pieces: a binding record for an array-typed element, a nested-variant postfix phase, and
//! re-arming after an index. **Two of the three already existed.** The `]` handler already emits
//! `GetIndex(FlatNested{size, Array})` and already re-arms the scalar index postfix, and
//! `step_structarrayaccess` is already generic over the variant. Only the binding side was
//! missing.
//!
//! That is the same lesson as the unreached stage commands, run in the other direction: **check
//! whether the code exists before costing work that depends on it.** There it revealed hidden
//! cost; here it revealed hidden progress.
//!
//! # Proportionality
//!
//! `self_hosted_compile` cross-checks ops, constant pool and local count against the reference
//! and refuses on divergence, so this produced a loud error rather than a wrong module for the
//! CLI path. The exposure was to direct callers of the `self_host_compile*` entry points.

#![cfg(all(feature = "self-host", feature = "compile"))]

use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn reference(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("reference compile")
}

/// Ops, constant pool and local count — the three fields `self_hosted_compile` cross-checks.
fn agrees(src: &str) -> bool {
    let (a, b) = (keleusma::selfhost::self_host_compile(src), reference(src));
    a.chunks.len() == b.chunks.len()
        && a.chunks.iter().zip(b.chunks.iter()).all(|(x, y)| {
            x.name == y.name
                && x.ops == y.ops
                && x.constants == y.constants
                && x.local_count == y.local_count
        })
}

/// **BOTH FORMS LOWER BYTE-IDENTICALLY, AND THEY HAD DIFFERENT CAUSES.**
///
/// The direct chain and the split form are asserted separately because they failed for different
/// reasons and were fixed by different edits. The direct chain needed the binding to record an
/// array-typed element; the split form additionally needed the EXTRACTED inner array to be
/// recordable as an array value, or `b` bound as a scalar and the `[1]` had nothing to arm.
///
/// The split form is also what rules out chaining as the cause: it diverged too, so the defect
/// was indexing a nested array at all rather than indexing twice in succession.
#[test]
fn indexing_an_array_of_arrays_lowers_byte_identically() {
    assert!(
        agrees("fn f() -> Word { let a = [[1, 2], [3, 4]]; a[0][1] }"),
        "the direct chain diverged. If the ops stop short after the literal, the binding is not \
         recording an array-typed element; if a second index never appears, the postfix is not \
         being re-armed"
    );
    assert!(
        agrees("fn f() -> Word { let a = [[1, 2], [3, 4]]; let b = a[0]; b[1] }"),
        "the split form diverged. This one fails when the EXTRACTED inner array is not recorded \
         as a bindable array value, which is a different cause from the direct chain"
    );
}

/// **THE STATE RECORD DOES NOT LEAK INTO A LATER BINDING.**
///
/// The fix marks a pending array binding at the point the nested index is emitted — and that
/// point is reached whenever such an index is emitted, INCLUDING when the extracted value is
/// never bound to anything. A record set on every nested index and consumed only by a `let` is
/// exactly the shape of state that leaks into an unrelated later binding.
///
/// So this is not reasoning about the clear-sites; it is five shapes measured. The last two
/// matter most: a nested index used inside an expression with no binding at all, and two in
/// sequence where the first could contaminate the second.
#[test]
fn a_nested_index_does_not_contaminate_a_later_binding() {
    for src in [
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; let c = a[0][1]; let d = 7; d }",
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; let c = a[0][0] + 1; let d = 7; d }",
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; let c = a[0][1]; let e = [5, 6]; e[0] }",
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; let p = a[0][1]; let q = a[1][0]; p + q }",
        "fn f() -> Word { let a = [[1, 2], [3, 4]]; a[0][1] + a[1][0] }",
    ] {
        assert!(agrees(src), "state leaked into a later binding: {src:?}");
    }
}

/// **THE CONTROLS, WHICH ARE WHAT MAKE THE ASSERTIONS ABOVE MEAN ANYTHING.**
///
/// Each isolates one thing the repair touched and must not have broken:
///
/// - the nested array LITERAL, repaired separately by carrying element size per nesting level. A
///   flat "last array closed" flag leaked across siblings and gave 64 where 32 was right — worse
///   than the bug it replaced — and the index work edits adjacent state.
/// - a flat array index, the path that always worked and shares the postfix.
/// - an array-of-STRUCT index, which routes through the same `step_structarrayaccess` the repair
///   now arms with a different variant. If arming it for arrays broke it for structs, that is
///   here rather than in a later bisect.
#[test]
fn the_adjacent_constructs_did_not_regress() {
    assert!(
        agrees("fn f() -> Word { let a = [[1, 2], [3, 4]]; 1 }"),
        "the nested array LITERAL regressed; its outer composite is sized by its elements and \
         that sizing is per nesting level, not per statement"
    );
    assert!(
        agrees("fn f() -> Word { let a = [1, 2]; a[1] }"),
        "a flat array index regressed"
    );
    assert!(
        agrees("struct P { x: Word }\nfn f() -> Word { let a = [P { x: 1 }, P { x: 2 }]; a[0].x }"),
        "an array-of-struct index regressed, which means arming the shared postfix with the \
         Array variant broke it for the Struct variant"
    );
}
