//! **IS 64-BIT THE ONLY ACCEPTED WORD WIDTH, OR ONLY THE ONE THAT HAPPENS TO WORK?**
//!
//! `check_word_width` is a hand-written `word_bits_log2 == 6`. What tested it
//! before this file: two constructed targets (`embedded_16`, `embedded_8`), and a
//! census that sets width 5 while asking a **different** question — whether a
//! module-level refusal is visible to `module_refusals`.
//!
//! **Nothing enumerated the widths, and nothing asserted that 6 is the ONLY
//! accepted one.** That admits a concrete failure: widen the guard to admit
//! 32-bit without updating every width-dependent site, and both embedded targets
//! stay refused — they are 8 and 16 — so those tests stay green while a 32-bit
//! module lowers with 64-bit semantics.
//!
//! This line already built exactly this sentinel for FLOATS, enumerating every
//! `float_bits_log2` and asserting the partition is complete. **The word axis has
//! the same shape and the same hand-written equality and never got it.**
//!
//! # What this does NOT establish
//!
//! It pins a **refusal**. It says nothing about whether a narrow width would
//! lower correctly, which is the differential's question and a much larger
//! increment.
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerError, LowerOptions, module_refusals};

/// Every representable width selector. `word_bits_log2` is a `u8` naming a power
/// of two, and 0..=7 spans 1 through 128 bits, which covers every value the
/// target descriptor can carry.
const WIDTHS: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];

fn base_module() -> Module {
    compile(&parse(&tokenize("fn main() -> Word { 0 }").expect("lex")).expect("parse"))
        .expect("compile")
}

/// Did the backend refuse this module FOR ITS WORD WIDTH specifically?
///
/// An unrelated failure would satisfy a looser check and hide a missing guard,
/// which is the reason `no_float_sentinel` keeps the width refusal separate from
/// the float one rather than folding them.
fn refused_for_width(m: &Module) -> Option<bool> {
    let reported = module_refusals(m, LowerOptions::default());
    if reported.is_empty() {
        return None;
    }
    Some(
        reported
            .iter()
            .any(|(_, e)| matches!(e, LowerError::UnsupportedWordWidth(_))),
    )
}

#[test]
fn exactly_one_word_width_is_accepted_and_the_partition_is_complete() {
    let base = base_module();

    // NON-VACUITY OF THE CONSTRUCTION. The compiler stamps this build's own
    // width, so the widths below are imposed after compilation. If the build were
    // not 64-bit, imposing 6 would be the mutation and imposing the build's own
    // width would be the no-op, and this test would measure something other than
    // what it claims.
    assert_eq!(
        base.word_bits_log2, 6,
        "this build's Word is not 64 bits, so imposing widths below would not \
         mean what this test says it means"
    );

    let mut accepted: Vec<u8> = Vec::new();
    let mut refused: Vec<u8> = Vec::new();

    for w in WIDTHS {
        let mut m = base.clone();
        m.word_bits_log2 = w;
        match refused_for_width(&m) {
            None => accepted.push(w),
            Some(true) => refused.push(w),
            Some(false) => panic!(
                "width {w} was refused, but NOT for its word width. An incidental \
                 refusal satisfies a loose check and would hide a missing guard."
            ),
        }
    }

    assert_eq!(
        accepted,
        vec![6],
        "the accepted set is not exactly {{6}}. If a width was ADDED, every \
         width-dependent site must be audited: the embedded targets are 8 and 16, \
         so they stay refused and their tests stay green while a wider-but-not-64 \
         module would lower with 64-bit semantics. accepted={accepted:?} \
         refused={refused:?}"
    );

    // COMPLETENESS. Without this, a width falling through both arms would vanish
    // rather than fail.
    assert_eq!(
        accepted.len() + refused.len(),
        WIDTHS.len(),
        "the partition does not account for every width: {} accepted plus {} \
         refused against {} enumerated",
        accepted.len(),
        refused.len(),
        WIDTHS.len()
    );

    println!(
        "\n  word widths enumerated : {}\n  accepted               : {accepted:?}\n  \
         refused for width      : {refused:?}\n  \
         This pins a REFUSAL. It is not evidence that a narrow width would lower\n  \
         correctly; that is the differential's question.\n",
        WIDTHS.len()
    );
}
