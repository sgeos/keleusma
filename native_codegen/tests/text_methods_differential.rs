//! **THE `text` METHODS OVER `Text<N>` AGREE WITH THE VIRTUAL MACHINE.**
//!
//! Upstream 701 made `t.len()`, `t.capacity()`, `a.concat(b)`, `a.eq(b)` and
//! `t.slice(start, end)` compiler intrinsics behind the `text` feature, lowered
//! to existing opcodes with no opcode, native or wire format change. This file
//! is the native differential for them. Each source returns a `Word`, and a
//! `bool` result is observed through a branch, because the two-argument
//! harness compares integers. The slice bounds and the choice of operand come
//! from the arguments, so the clamping and the joins are driven at run time and
//! not folded away.
//!
//! # What it found
//!
//! Both `concat` cases were refused at first, by `NewComposite` finding an
//! operand of unknown packed width. `concat` duplicates its result length into
//! the result tuple, and `Op::Dup` re-pushed its operand unlabelled outside the
//! owned-bodies path, dropping the width and the kind. It now keeps both, and
//! these cases are its regression witness.
//!
//! # What this does not establish
//!
//! A `Text<N>` returned from `main` is not compared, for the reason
//! `text_surface.rs` records for a `Text` return. Only argument pairs within
//! the listed set are driven.

mod common;

const CASES: &[&str] = &[
    "fn main(a: Word, b: Word) -> Word { (\"abc\" as Text<5>).len() }",
    "fn main(a: Word, b: Word) -> Word { (\"abc\" as Text<5>).capacity() }",
    "fn main(a: Word, b: Word) -> Word { if (\"ab\" as Text<4>).eq(\"ab\" as Text<4>) { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { if (\"ab\" as Text<4>).eq(\"ac\" as Text<4>) { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { if (\"ab\" as Text<2>).eq(\"abc\" as Text<3>) { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { (\"ab\" as Text<2>).concat(\"cde\" as Text<3>).len() }",
    "fn main(a: Word, b: Word) -> Word { (\"abcdef\" as Text<8>).slice(a, b).len() }",
    "fn main(a: Word, b: Word) -> Word { let t = if a > 0 { \"x\" as Text<3> } else { \"yz\" as Text<3> }; t.len() }",
    "fn main(a: Word, b: Word) -> Word { if (\"abcd\" as Text<4>).slice(a, b).eq(\"bc\" as Text<4>) { 1 } else { 0 } }",
    "fn main(a: Word, b: Word) -> Word { let t = (\"ab\" as Text<2>).concat(\"cd\" as Text<2>); if t.eq(\"abcd\" as Text<4>) { a } else { b } }",
];

/// Argument pairs that put the slice bounds before, inside, across and past
/// the text, reversed, and negative.
const ARGUMENTS: &[(i64, i64)] = &[(0, 0), (1, 3), (-2, 10), (3, 1), (2, 6), (0, 4)];

#[test]
fn text_methods_agree_with_the_vm() {
    let mut compared = 0;
    for src in CASES {
        for &(a, b) in ARGUMENTS {
            for optimise in [false, true] {
                let (vm, native) = common::vm_and_native_two_arg_at(src, a, b, optimise);
                assert_eq!(native, vm, "{src} on ({a}, {b}), optimised {optimise}");
                compared += 1;
            }
        }
    }
    // Non-vacuity. Every case lowers and is compared on every pair.
    assert_eq!(compared, CASES.len() * ARGUMENTS.len() * 2);
}

