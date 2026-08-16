//! The operand-stack model's two readers, and the one that was served wrong
//! numbers.
//!
//! `Op::stack_growth()` and `Op::stack_shrink()` were read by two consumers that
//! want DIFFERENT quantities from them:
//!
//! - [`keleusma::verify`]'s worst-case-memory walk wants `growth` = the
//!   TRANSIENT reach above the current depth, and `growth - shrink` = the NET
//!   that propagates into every later operation's base.
//! - [`keleusma::text_size`] wants them as LITERAL pop and push counts for its
//!   shadow stack.
//!
//! Those two readings coincide only for an operation that does not both pop and
//! push. For one that does, they cannot both be satisfied by one pair of
//! numbers, and the field-read operations are exactly that shape: they pop a
//! composite and push a field value.
//!
//! The resolution is that `growth`/`shrink` are now EXCLUSIVELY the peak model,
//! and [`keleusma::verify::op_depth_effect`] — which returns
//! `(required, delta)` and already encoded the true semantics — is the pop/push
//! model. Each reader gets the numbers it actually wants.

#![cfg(feature = "compile")]

use keleusma::bytecode::{Chunk, Op};
use keleusma::verify::op_depth_effect;

fn chunk_named(src: &str, name: &str) -> Chunk {
    let m = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(src).expect("lex")).expect("parse"),
    )
    .expect("compile");
    m.chunks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no chunk named {name}"))
        .clone()
}

/// The operand peak under the model the worst-case-memory walk uses.
fn peak_under_growth_shrink(ops: &[Op]) -> i32 {
    let (mut cur, mut peak) = (0i32, 0i32);
    for op in ops {
        let g = op.stack_growth() as i32;
        let s = op.stack_shrink() as i32;
        peak = peak.max((cur + g).max(0));
        cur += g - s;
    }
    peak
}

/// The operand peak under `op_depth_effect`, which is the independent model.
fn peak_under_depth_effect(ops: &[Op], chunk: &Chunk) -> i32 {
    let (mut cur, mut peak) = (0i32, 0i32);
    for op in ops {
        let (_required, delta) = op_depth_effect(op, chunk);
        peak = peak.max(cur.max(cur + delta));
        cur += delta;
    }
    peak
}

/// THE CONTROL. The two models must agree on the operand peak.
///
/// **This fails before the repair and passes after**, which is the only reason
/// it is worth having: the whole existing suite passes in both states, and so
/// does the self-hosted differential — `analyze.kel` consumes `growth`/`shrink`
/// as host-seeded arrays, so it reproduces whatever the reference says. A
/// differential against the model under test cannot detect that the model is
/// wrong, which is why this compares against `op_depth_effect` instead.
///
/// # WHAT THIS TEST DOES NOT ESTABLISH, AND WHAT NOW DOES
///
/// **Its coverage is a property of its case list, not of the opcode set.** It
/// compares the two models over the five sources below. None of them yields, so
/// it could never have reported `Op::Yield`, whose peak-model net was already
/// wrong when this test was written — and none reaches a fixed-point multiply
/// or divide, whose entries were also wrong and went unreported until something
/// ranged over the opcodes.
///
/// The check that does range over the whole opcode set is
/// `the_two_operand_stack_models_agree_across_the_whole_opcode_set`, in
/// `src/verify.rs`. It lives there rather than here because completeness is
/// asserted against the canonical wire-format opcode table, which is private to
/// the crate. It found two disagreeing opcodes on its first run.
///
/// **Adding a sixth case here does not close the class.** It closes one
/// instance and leaves the next invisible. These cases are kept because they
/// name the specific defect they were written for and fail loudly on it; they
/// are not the instrument for finding the next one.
///
/// Measured before the repair: `two-fields-added` reported a peak of 1 against
/// a true 3, and its Stream twin reported 96 WCMU bytes where 128 is correct.
/// The understatement scales with the number of field reads, and it propagates:
/// a wrong NET lowers the base every later operation's peak is computed from.
#[test]
fn the_peak_model_agrees_with_the_depth_model() {
    const CASES: &[(&str, &str, &str)] = &[
        (
            "one-field-then-push",
            "struct S { a: Word, b: Word }\n\
             fn f(s: S) -> Word { s.a + 1 }\n\
             fn main() -> Word { f(S { a: 1, b: 2 }) }",
            "f",
        ),
        (
            "two-fields-added",
            "struct S { a: Word, b: Word }\n\
             fn g(s: S) -> Word { s.a + s.b }\n\
             fn main() -> Word { g(S { a: 1, b: 2 }) }",
            "g",
        ),
        (
            "tuple-field",
            "fn h(t: (Word, Word)) -> Word { t.0 + 1 }\n\
             fn main() -> Word { h((1, 2)) }",
            "h",
        ),
        // CONTROLS THAT MUST NOT MOVE. `GetIndex` shares a match arm with the
        // field reads and was reported as a fourth instance of the same defect.
        // It is not one: it genuinely pops two and pushes one, so its net of -1
        // is correct and the arm grouping is what made it look wrong.
        (
            "index-control",
            "fn k(xs: [Word; 4]) -> Word { xs[0] + 1 }\n\
             fn main() -> Word { k([1, 2, 3, 4]) }",
            "k",
        ),
        // The checked-arithmetic family. Its net is +1 and its transient reach
        // is one slot, because the virtual machine pops BOTH operands before
        // pushing any result. Both are already right; this pins them so the
        // repair cannot loosen a correct bound while fixing a wrong one.
        (
            "checked-arithmetic-control",
            "fn c(a: Word, b: Word) -> Word { a * b }\n\
             fn main() -> Word { c(3, 4) }",
            "c",
        ),
    ];

    let mut compared = 0;
    for (label, src, name) in CASES {
        let chunk = chunk_named(src, name);
        let peak_model = peak_under_growth_shrink(&chunk.ops);
        let depth_model = peak_under_depth_effect(&chunk.ops, &chunk);
        assert_eq!(
            peak_model, depth_model,
            "{label}: the peak model says {peak_model} and the depth model says {depth_model}. \
             A peak LOWER than the depth model is unsound: the worst-case-memory bound would be \
             smaller than the operand stack the chunk actually needs."
        );
        compared += 1;
    }
    assert_eq!(compared, CASES.len(), "not every case was compared");
}

/// The per-operation net must be what the virtual machine actually does.
///
/// Stated operation by operation rather than as a walk, so a failure names the
/// operation rather than a chunk. `op_depth_effect` is the reference for the
/// net because it encodes the true pop and push counts.
#[test]
fn every_field_read_has_the_net_the_vm_gives_it() {
    let chunk = chunk_named(
        "struct S { a: Word }\nfn f(s: S) -> Word { s.a }\nfn main() -> Word { f(S { a: 1 }) }",
        "f",
    );
    // (operation, what the VM does: pops, pushes)
    let cases: &[(Op, i32, i32)] = &[
        (
            Op::GetField(keleusma::bytecode::StructField::Boxed { name_const: 0 }),
            1,
            1,
        ),
        (Op::Len, 1, 1),
    ];
    for (op, pops, pushes) in cases {
        let net = op.stack_growth() as i32 - op.stack_shrink() as i32;
        assert_eq!(
            net,
            pushes - pops,
            "{op:?}: the peak model's net is {net}, the virtual machine's is {}",
            pushes - pops
        );
        let (_required, delta) = op_depth_effect(op, &chunk);
        assert_eq!(
            delta,
            pushes - pops,
            "{op:?}: the depth model disagrees with the virtual machine"
        );
    }
}

/// The pop and push counts come from `op_depth_effect`, not from the peak model.
///
/// This is the property the text-size shadow stack depends on, and it is the
/// reason that consumer was moved: it pops `shrink` and pushes `growth` entries,
/// which for a pop-and-push operation is not the number of either. Under
/// `op_depth_effect` the counts are `required` and `required + delta`.
#[test]
fn the_depth_model_gives_true_pop_and_push_counts() {
    let chunk = chunk_named(
        "struct S { a: Word }\nfn f(s: S) -> Word { s.a }\nfn main() -> Word { f(S { a: 1 }) }",
        "f",
    );
    // (operation, true pops, true pushes) as the virtual machine performs them.
    let cases: &[(Op, i32, i32)] = &[
        (
            Op::GetField(keleusma::bytecode::StructField::Boxed { name_const: 0 }),
            1,
            1,
        ),
        (
            Op::GetIndex(keleusma::bytecode::ArrayElem::Flat {
                kind: keleusma::value_layout::ScalarKind::Int,
            }),
            2,
            1,
        ),
        (Op::CheckedAdd, 2, 3),
        (Op::CheckedNeg, 1, 3),
        (Op::Dup, 1, 2),
    ];
    for (op, pops, pushes) in cases {
        let (required, delta) = op_depth_effect(op, &chunk);
        assert_eq!(required, *pops, "{op:?}: wrong pop count");
        assert_eq!(
            required + delta,
            *pushes,
            "{op:?}: wrong push count (required {required}, delta {delta})"
        );
    }
}
