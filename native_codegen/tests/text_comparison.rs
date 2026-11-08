//! **ORDER 6: A `Text` IS COMPARED BY CONTENT, OR THE COMPARISON IS REFUSED.**
//!
//! Before Order 6 the backend lowered a comparison of two `Text` operands as an
//! integer comparison of the words that carry them, and those words are
//! addresses. `"ab" == "ab"` was false natively and true in the virtual
//! machine, and comparing a `Text` returned by a call with a literal gave the
//! wrong branch. Both lowered without a refusal, so nothing reported them. The
//! construct census written for Order 6 is what found them.
//!
//! A comparison an operand of which may be `Text`, by the provenance analysis
//! in `text.rs`, now lowers only when both operands are string literals, which
//! this backend lays out as `{ i64 len, bytes }` records and compares by length
//! and bytes. Every other such comparison is refused.
//!
//! # What this does not establish
//!
//! A `Text` returned by a call, passed as a parameter, returned by a native or
//! read from a field is still refused in a comparison. Ordering comparisons of
//! `Text` are refused. Both are recorded in the Order 6 completion record.
mod common;

use keleusma_native::{LowerOptions, module_refusals};

const AGREE: &[&str] = &[
    "fn main(a: Word, b: Word) -> Word { if \"ab\" == \"ab\" { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { if \"ab\" == \"abc\" { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { if \"ab\" != \"ac\" { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { if \"\" == \"\" { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { let s = \"x\"; let t = \"x\"; if s == t { 5 } else { 6 } }",
    "fn main(a: Word, b: Word) -> Word { let s = if a > 0 { \"x\" } else { \"y\" }; if s != \"x\" { 3 } else { 4 } }",
    "fn main(a: Word, b: Word) -> Word { let s = if a > 0 { \"x\" } else { \"y\" }; \
     match s { \"x\" => 10, \"y\" => 20, _ => 30 } }",
];

#[test]
fn literal_text_comparisons_agree_with_the_vm_by_content() {
    for src in AGREE {
        let m = common::build(src);
        assert!(
            module_refusals(&m, LowerOptions::default()).is_empty(),
            "a literal comparison is refused: {src}"
        );
        for (a, b) in [(1, 0), (-1, 0), (0, 7)] {
            for optimise in [false, true] {
                let (vm, native) = common::vm_and_native_two_arg_at(src, a, b, optimise);
                assert_eq!(native, vm, "{src} on ({a}, {b}), optimised {optimise}");
            }
        }
    }
}

/// MUST-NOT-FIRE for the test above, and the case that was wrong before. A
/// `Text` from a call is not a literal record, so comparing it is refused, not
/// lowered as the address comparison that gave the wrong branch.
#[test]
fn a_text_whose_layout_this_backend_does_not_own_is_refused_in_a_comparison() {
    for src in [
        "fn pick(a: Word) -> Text { if a > 0 { \"yes\" } else { \"no\" } } \
         fn main(a: Word, b: Word) -> Word { if pick(a) == \"yes\" { 1 } else { 2 } }",
        "fn same(s: Text, t: Text) -> Word { if s == t { 1 } else { 0 } } \
         fn main(a: Word, b: Word) -> Word { same(\"p\", \"p\") }",
    ] {
        let m = common::build(src);
        let refusals = module_refusals(&m, LowerOptions::default());
        let (_, why) = refusals
            .first()
            .unwrap_or_else(|| panic!("a foreign Text comparison lowered: {src}"));
        assert!(
            why.to_string().contains("may receive a Text"),
            "{src} was refused for another reason: {why}"
        );
    }
}

/// An integer comparison in a chunk that also holds string literals is not
/// affected, because the analysis tracks each value rather than the chunk.
/// The `piano_roll` programs are the corpus instance of this shape.
#[test]
fn integer_comparisons_beside_literals_still_lower() {
    let src = "fn main(a: Word, b: Word) -> Word { let s = \"label\"; let t = \"label\"; \
               let k = if s == t { 1 } else { 0 }; if a < b { k } else { k + 2 } }";
    for (a, b) in [(1, 2), (5, 2)] {
        let (vm, native) = common::vm_and_native_two_arg(src, a, b);
        assert_eq!(native, vm);
    }
}
