//! **DO THE REPAIRED WIDTHS SURVIVE A YIELD?**
//!
//! # Why this intersection specifically
//!
//! Six arm-groups were repaired on 2026-09-17 so that a computed value carries its
//! packed width. Every subject that verified them **computes and consumes within a
//! single call**.
//!
//! A stream is different, and this line has the scar to prove it: one of the ten
//! recorded defects was **a local live across a `yield`, wiped by the entry
//! preamble on every resume**. A value that crosses a suspension is spilled and
//! restored, and **nothing has asked whether its WIDTH crosses with it.**
//!
//! If a width were lost on the resume path, a composite built after the yield from
//! a value computed before it would refuse — or, worse, if a width were restored
//! WRONGLY, it would mispack. Both are reachable only by driving a stream.
//!
//! # What the subjects do
//!
//! Compute with an operator from the repaired family, carry the result across a
//! yield, and use it afterwards — including one that builds a composite from it.
//! **Driven tick for tick against the reference**, which is the only check that
//! distinguishes a restored width from a plausible-looking one.
//!
//! # ⚠ AND THE FIRST VERSION OF THIS FILE TESTED A DIFFERENT PATH THAN IT CLAIMED
//!
//! There are TWO width tables, and they are not the same mechanism:
//!
//! - **`local_widths`** — what a local carries. Every subject that stores its
//!   value in a `let` uses this.
//! - **`spilled`** — the `(width, kind)` pairs of operands *beneath* the yielded
//!   value, saved at the suspension and restored at the resume point. Its own
//!   comment warns that a value restored at the wrong width is *"a silently wrong
//!   number rather than a fault"*.
//!
//! **The original five subjects all carried their values in LOCALS**, so none of
//! them touched the spill slice. Measured: corrupting the restored widths to
//! `Unknown` left every one of them — and `stream_depth.rs` — passing.
//!
//! The last subject was added for that reason. It leaves a `Byte` on the operand
//! stack across the suspension and consumes it afterwards with a byte add, which
//! needs a matched one-byte pair. **With the restore corrupted it refuses; with
//! the restore intact it agrees.** That is the only thing in this package known to
//! detect a width lost on the resume path.

mod common;

/// `(label, source)`. Each carries a value from a repaired arm across a yield.
const SUBJECTS: &[(&str, &str)] = &[
    (
        "byte quotient across a yield",
        "loop main(t: Word) -> Word { \
           let q: Byte = (t as Byte) / (2 as Byte); \
           let r = yield (q as Word); \
           yield ((q + (r as Byte)) as Word) }",
    ),
    (
        "word bitwise across a yield",
        "loop main(t: Word) -> Word { \
           let v: Word = t band 15; \
           let r = yield v; \
           yield (v bxor r) }",
    ),
    (
        "word shift across a yield",
        "loop main(t: Word) -> Word { \
           let v: Word = t lsr 1; \
           let r = yield v; \
           yield (v + r) }",
    ),
    (
        "a composite built from a quotient, inside a stream",
        "struct P { x: Byte, y: Word }\n\
         loop main(t: Word) -> Word { \
           let p: P = P { x: (t as Byte) / (2 as Byte), y: t }; \
           let r = yield ((p.x as Word) + p.y); \
           yield r }",
    ),
    (
        "a fixed product across a yield",
        "loop main(t: Word) -> Word { \
           let f: Fixed = (t as Fixed) * (2 as Fixed); \
           let r = yield (f as Word); \
           yield (r + 1) }",
    ),
    // ⚠ **THE ONLY SUBJECT HERE THAT REACHES THE SPILL SLICE.** See the note
    // below: the others carry their values in LOCALS, which use a different
    // table. This one leaves a `Byte` on the OPERAND STACK across the
    // suspension and then consumes it with a byte add, which needs a matched
    // one-byte pair — so a width lost on the resume path refuses here, and is
    // invisible everywhere else.
    (
        "a byte left on the operand stack across a yield",
        "loop main(t: Word) -> Word { \
           let x: Word = ((((t as Byte) / (2 as Byte)) + ((yield t) as Byte)) as Word); \
           yield x }",
    ),
];

/// Replies that VARY, so a resume path ignoring its input cannot pass.
fn replies(n: usize) -> Vec<i64> {
    (0..n).map(|i| (i as i64 % 5) + 1).collect()
}

/// Deep enough that a value restored wrongly on SOME resumes, not all, still
/// shows. The previously-found defect wiped a local on **every** resume, which a
/// shallow run would have caught; a subtler one might not.
const TICKS: usize = 60;

#[test]
fn the_repaired_widths_survive_a_yield() {
    for (label, src) in SUBJECTS {
        let refusals = keleusma_native::module_refusals(
            &common::build(src),
            keleusma_native::LowerOptions::default(),
        );
        assert!(
            refusals.is_empty(),
            "`{label}` does not lower: {refusals:?}. A value from a repaired arm \
             that cannot cross a yield is a seventh gap in that family, not a \
             property of streams."
        );

        let r = replies(TICKS);
        let vm = common::general_vm_sequence(src, 1, &r);
        let native = common::general_native_sequence(src, 1, &r);
        assert!(
            vm.len() >= TICKS / 2,
            "`{label}` produced only {} values; a short run agrees trivially",
            vm.len()
        );
        assert_eq!(
            vm, native,
            "`{label}` DIVERGES across {TICKS} ticks. A value computed before a \
             suspension is spilled and restored, and this is the only check that \
             asks whether its packed WIDTH is restored with it. The recorded \
             defect in this area wiped a local on every resume; a width restored \
             WRONGLY would mispack instead. This establishes that the two \
             implementations disagree, NOT which is right."
        );
    }
}
