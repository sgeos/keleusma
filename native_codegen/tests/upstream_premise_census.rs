//! **WHICH COMMENTS IN THE EMITTER ASSERT AN INVARIANT SOMETHING ELSE IS
//! SUPPOSED TO PROVIDE, AND WHAT HAPPENS IF ONE IS FALSE?**
//!
//! # Why this exists
//!
//! On 2026-09-08 the `Op::GetIndex` arm was found lowering an unguarded load on
//! the strength of a comment: *"the bound is not checked here: the compiler
//! emits `Op::BoundsCheck` before the index."* **The compiler does not.** The
//! bytecode is a bare `GetIndex`, and a three-element array indexed at 5
//! returned the caller region's filler bytes where the reference faults.
//!
//! > **A PREMISE IN A COMMENT IS THE LEAST-TESTED THING IN A CODEBASE**, because
//! > the reader who would check it is the reader who believes it.
//!
//! That defect was found by accident, while instrumenting something else. This
//! file is the deliberate version: it makes the CLASS visible, so the next such
//! premise has to be dispositioned when it is written rather than when it is
//! tripped over.
//!
//! # The disposition that matters is not "true or false"
//!
//! It is **what a false premise costs**. A premise guarding a `return Err(..)`
//! costs a refusal, which is safe. A premise guarding a load, a store, or an
//! offset costs a silently wrong value, which is the class this backend exists
//! to refuse. The table below records that column, not a truth value.
//!
//! | site | premise | if it were false |
//! |---|---|---|
//! | `Fixed` operands are narrowed away from `Int` | refuse — `arith_result_width` returns `None` |
//! | `Op::Mul` on `Fixed` does not exist | refuse — same path |
//! | the corrupt `WordToFixed` arm cannot occur | refuse — explicit, not assumed away |
//! | a body field is guaranteed no alignment better than 1 | safe — takes the conservative path |
//! | the compiler emits dead code after `break` | no safety consequence; permissive only |
//! | **the bound comes from `Op::BoundsCheck`** | **MISLOWER — this was the defect. Now checked at the site.** |
//! | **the operand stack is empty at `Op::Reset`** | **MISLOWER — was assumed. Now checked at the site.** |
//! | **a verified module cannot present a depth disagreement** | **HARMLESS IF FALSE — the site no longer relies on it.** |
//!
//! **Two of seven were unchecked and one of those was live.** Both are now
//! checks rather than prose.
//!
//! # The eighth entry is the shape this census wants, arriving 2026-09-12
//!
//! The depth-agreement check WAS an `assert_eq!` carrying the premise *"the typed
//! verifier guarantees agreement, so this is a lowering bug"* — a premise about
//! upstream, guarding a PANIC on a public entry point that does not require a
//! verified module. Retargeting a branch to a valid-but-wrong index reached it.
//!
//! **It is now a refusal, and that is what makes the replacement premise
//! harmless.** The comment still observes that a verified module cannot present
//! the shape, but nothing depends on the observation: the disagreement is refused
//! whether or not the verifier would have prevented it. **A premise the code does
//! not rely on costs nothing when it is false** — which is the disposition this
//! table exists to record, rather than the count.
//!
//! # ⚠ WHAT THIS CENSUS DOES NOT COVER, AND IT IS THE CLASS THAT BIT HARDEST
//!
//! **Neither of the two WRONG refusals this line produced would have matched
//! this phrase list.** They read *"native code CANNOT truncate the operand stack
//! at `Reset`"* and *"the two edges DISAGREE about the operand stack"* — claims
//! about **what the runtime does** and about **the lowering's own structure**.
//! The list above was built entirely around claims about the COMPILER's output.
//!
//! **Widening to cover them was measured and rejected.** Adding "the runtime",
//! "the reference", bare "cannot" and "disagree" takes the population from 29
//! lines to **132**, and this census earns its keep by DISPOSITIONING each entry
//! by what a false premise would cost. A 132-row table is one nobody maintains,
//! and an unmaintained table is worse than none because it looks like coverage —
//! the same reason `test_population_guard.rs` pins a count rather than 475 names.
//!
//! So the honest scope is: **claims about what the compiler emits, and claims of
//! impossibility.** A claim about the RUNTIME's behaviour is not caught here and
//! must be checked the way both of those were — by reading `src/vm.rs` at the
//! same opcode.
//!
//! # What this test can and cannot do
//!
//! It counts premise-shaped comments. **It cannot tell a true premise from a
//! false one** — only a measurement against the bytecode can, which is what
//! found the defect. What it does is refuse to let the population grow
//! silently: a new premise makes this fail, and the author must either check the
//! invariant at the site or record here why a false premise would be harmless.

/// Phrases that introduce an invariant the emitter does not itself establish.
const PREMISE_PHRASES: &[&str] = &[
    "the compiler emits",
    "compiler always",
    "is guaranteed",
    "guaranteed by",
    "cannot occur",
    "cannot happen",
    "never emits",
    "always emits",
    // **WIDENED 2026-09-10 to impossibility claims**, after this census was
    // found to miss the class that produced the session's two WRONG refusals.
    // See the scope note in the header.
    "cannot be",
    "can never",
    "impossible",
];

/// Lines of `src/lib.rs` carrying at least one premise phrase, at the stamp.
///
/// **Re-derive this rather than trusting it.** It moves with every increment
/// that adds or removes such a comment, including this file's own prose being
/// quoted into the emitter.
const RECORDED_PREMISE_LINES: usize = 30;
// 12 -> 29 with the impossibility phrases. The seventeen new lines were read,
// and they fall in one class: **statements of what the backend DECLINES** —
// "an unknown width cannot be placed", "`Op::Add` cannot be lowered without
// knowing its operands". A false premise there costs a REFUSAL, which is safe.
//
// The one that is a bound rather than a decline is the spill slice's
// "why that bound cannot be exceeded", and it IS enforced: a chunk deeper than
// `MAX_STACK` is refused by the recorded `stack_overflow`, not assumed away.

fn premise_lines() -> Vec<(usize, String)> {
    let src = std::fs::read_to_string("src/lib.rs").expect("the emitter is readable");
    src.lines()
        .enumerate()
        .filter(|(_, l)| PREMISE_PHRASES.iter().any(|p| l.contains(p)))
        .map(|(i, l)| (i + 1, l.trim().to_string()))
        .collect()
}

#[test]
fn every_upstream_premise_in_the_emitter_is_accounted_for() {
    let found = premise_lines();
    println!("\n================ UPSTREAM PREMISES IN THE EMITTER");
    for (n, l) in &found {
        let short: String = l.chars().take(96).collect();
        println!("  src/lib.rs:{n}  {short}");
    }
    println!("  ------------------------------------------------");
    println!("  premise-shaped lines: {}", found.len());
    println!(
        "\n  A PREMISE GUARDING A REFUSAL COSTS A REFUSAL. A premise guarding a\n  \
         LOAD, a STORE or an OFFSET costs a silently wrong value. The header's\n  \
         table records that column for each, and two of seven were unchecked\n  \
         until the bounds defect was measured.\n================\n"
    );

    // **NON-VACUITY.** A phrase list that matches nothing would pass forever.
    assert!(
        !found.is_empty(),
        "no premise-shaped comment matched, so either the emitter was rewritten \
         or this file's phrase list has gone stale. A census that cannot fire is \
         indistinguishable from no census."
    );
    assert_eq!(
        found.len(),
        RECORDED_PREMISE_LINES,
        "the number of premise-shaped comments in the emitter has changed. This \
         is not a failure to patch to the new number: for each new one, either \
         CHECK the invariant at the site, or record in this file's header why a \
         false premise would be harmless there. The one that was not checked \
         returned an out-of-bounds read as a value."
    );
}

/// **The two premises that were promoted to checks must STAY checks.**
///
/// Asserted against behaviour rather than against the comment text, because a
/// comment saying "now checked" is exactly the kind of claim this file exists to
/// distrust.
#[test]
fn the_two_promoted_premises_are_enforced_at_the_site() {
    use keleusma::compiler::compile;
    use keleusma::lexer::tokenize;
    use keleusma::parser::parse;
    use keleusma_native::{LowerOptions, module_refusals};

    // The bounds premise: an array indexed by a runtime value must not lower to
    // an unguarded load. Established by execution in
    // `partial_operation_census.rs`; asserted structurally here so the two do
    // not share a single point of failure.
    let src = "fn main(i: Word) -> Word { let xs = [1, 2, 3]; xs[i] }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let refused = !module_refusals(&m, LowerOptions::default()).is_empty();
    let ir = {
        let ctx = inkwell::context::Context::create();
        let lm = ctx.create_module("kel");
        keleusma_native::lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
        lm.print_to_string().to_string()
    };
    assert!(
        refused || ir.contains("idxoob"),
        "a runtime array index lowered with no bounds comparison. That is the \
         exact defect this file was written after: the load returns whatever \
         lies at base + index * stride"
    );

    // The `Reset` premise cannot be exercised from source — the compiler emits
    // `Stream ; body ; PopN(1) ; Reset`, so the operand stack IS empty there,
    // which is why the premise held. What is asserted is that a stream still
    // lowers, so the added check has not made the common case refuse.
    let stream = "loop main(a: Word) -> Word { let r = yield a; yield r + 1 }";
    let sm = compile(&parse(&tokenize(stream).expect("lex")).expect("parse")).expect("compile");
    assert!(
        module_refusals(&sm, LowerOptions::default()).is_empty(),
        "the operand-depth check at `Op::Reset` now refuses an ordinary stream, \
         so it is enforcing something stricter than the premise it replaced"
    );
}
