//! RESEARCH SPIKE, not a regression test.
//!
//! Question: is "composites" ONE blocker or TWO?
//!
//! `NATIVE_LOWERING_INVENTORY.md` records the composite class as a single
//! 28-chunk item blocked on operand type recovery, needing an enabling change
//! in `src/verify_typed.rs`. Reading the operands says that is too wide:
//!
//! - Every composite READ op already bakes what a lowering needs.
//!   `GetField::Flat { offset, kind }`, `GetEnumField::Flat { offset, kind }`,
//!   `GetTupleField`, and `GetIndex::Flat { kind }` carry the byte offset and
//!   the scalar kind. The nested forms carry `offset`, `size`, and `variant`.
//!   A read is a load at a known displacement and a known width. **No shape
//!   recovery is involved.**
//! - Only `NewComposite::Flat { kind, count, byte_size }` is short. It carries
//!   the TOTAL body size, not the per-field breakdown, so packing `count`
//!   popped values requires knowing each one's width. That, and only that, is
//!   what type recovery buys.
//!
//! If that split is real, then part of the 28 is reachable with no change to
//! any file this branch does not own. This spike measures the split instead of
//! assuming it, per the branch rule: probe before planning.
//!
//! Run with `cargo test --test spike_composite_split -- --nocapture`.
//! It reports rather than asserts, except for one guard against measuring
//! nothing.

use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

/// Every opcode the lowering handles today, by discriminant.
///
/// This mirrors the lowering BY HAND and a hand mirror rots; the ground-truth
/// test in `spike_corpus_coverage.rs` asks `lower_module` itself. Here a mirror
/// is unavoidable, because the question is counterfactual — what WOULD lower if
/// a class were supported — and a counterfactual cannot be asked of the real
/// entry point, which refuses on the first unsupported op and reports no more.
/// Kept adjacent to that test so the two are updated together.
fn is_lowered(op: &Op) -> bool {
    matches!(
        op,
        Op::GetLocal(_)
            | Op::SetLocal(_)
            | Op::PopN(_)
            | Op::Dup
            | Op::Const(_)
            | Op::PushImmediate(_)
            | Op::CheckedAdd
            | Op::CheckedSub
            | Op::CheckedNeg
            | Op::CheckedMul(0)
            | Op::Div
            | Op::Mod
            | Op::CheckedDiv(0)
            | Op::CheckedMod
            | Op::CmpEq
            | Op::CmpNe
            | Op::CmpLt
            | Op::CmpGt
            | Op::CmpLe
            | Op::CmpGe
            | Op::Not
            | Op::BitAnd
            | Op::BitOr
            | Op::BitXor
            | Op::Shl
            | Op::Shr
            | Op::If(_)
            | Op::Else(_)
            | Op::EndIf
            | Op::Loop(_)
            | Op::EndLoop(_)
            | Op::Break(_)
            | Op::BreakIf(_)
            | Op::Return
            | Op::Trap(_)
            | Op::Call(_, _)
            | Op::WordToByte
            | Op::ByteToWord
            | Op::BoundsCheck(_)
            | Op::GetData(_)
            | Op::SetData(_)
            | Op::GetDataIndexed(..)
            | Op::SetDataIndexed(..)
    )
}

/// A composite op that only READS an existing body. Every one of these bakes
/// its own offset and width, so lowering it needs no recovered operand shape.
fn is_composite_read(op: &Op) -> bool {
    matches!(
        op,
        Op::GetField(..)
            | Op::GetIndex(..)
            | Op::GetTupleField(..)
            | Op::GetEnumField(..)
            | Op::Len
            | Op::IsEnum(..)
            | Op::IsStruct(..)
    )
}

/// A composite op that BUILDS a body. Only this class is short of information.
fn is_composite_ctor(op: &Op) -> bool {
    matches!(op, Op::NewComposite(..))
}

fn is_composite(op: &Op) -> bool {
    is_composite_read(op) || is_composite_ctor(op)
}

/// How a single construction site looks against the packing question.
///
/// **These are descriptive, not a lowering basis.** An earlier revision of this
/// spike treated `count * word_bytes == byte_size` as licensing uniform
/// word-width packing and reported that 22 chunks were reachable "by arithmetic"
/// with zero needing recovery. Reading `Vm`'s handler falsified that:
///
/// - `pack_flat_in_arena` sizes the body as the SUM of per-value
///   `flat_field_size`, taken from each runtime value's own kind. Widths are
///   per value and never assumed uniform.
/// - For a `Tuple` or `Array` the VM passes `min_bytes = 0` and the operand's
///   `byte_size` is, in its own comment, "the verifier annotation only". The
///   equality therefore compares against a number the runtime does not use to
///   lay out the body at all.
/// - For a `Struct` or `Enum` `byte_size` is only a FLOOR, padding an enum to
///   its widest variant, so equality does not pin the field breakdown either.
///
/// The equality is consequently neither necessary nor sufficient. A two-field
/// 16-byte body may be a one-byte `Bool` beside a fifteen-byte nested
/// composite, which the arithmetic cannot distinguish from two words. The
/// counts below are retained only to show how often the coincidence occurs.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Ctor {
    /// Flat, and `count * word_bytes == byte_size` happens to hold. A
    /// coincidence worth counting, not a licence to pack uniformly.
    FlatSizeConsistentWithWords,
    /// Flat, and the equality does not hold.
    FlatSizeInconsistentWithWords,
    /// Boxed body. A different problem entirely (P3 removes the form).
    Boxed,
}

fn classify_ctor(op: &Op, word_bytes: u32) -> Option<Ctor> {
    match op {
        Op::NewComposite(NewCompositeOperand::Flat {
            count, byte_size, ..
        }) => {
            let consistent = (*count as u32).checked_mul(word_bytes) == Some(*byte_size as u32);
            Some(if consistent {
                Ctor::FlatSizeConsistentWithWords
            } else {
                Ctor::FlatSizeInconsistentWithWords
            })
        }
        Op::NewComposite(NewCompositeOperand::Boxed { .. }) => Some(Ctor::Boxed),
        _ => None,
    }
}

/// What stands between a chunk and lowering, once composites are split in two.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Verdict {
    /// Lowers today.
    AlreadyLowers,
    /// Blocked only by composite READS. Every read op bakes its own offset and
    /// width, so this is reachable with no shape recovery at all.
    ReadsOnly,
    /// Blocked only by composites, but a construction is present. Packing needs
    /// each popped value's width, which no instruction records, so this is the
    /// population that needs shape recovery — regardless of whether the baked
    /// `byte_size` happens to equal `count * word_bytes`.
    NeedsWidthRecovery,
    /// Blocked by something outside the composite class as well, so solving
    /// composites alone does not free it.
    BlockedElsewhereToo,
}

fn word_bytes_of(m: &Module) -> u32 {
    (1u32 << m.word_bits_log2) / 8
}

/// Classify an op stream. Takes `&[Op]` rather than `&Chunk` so the controls
/// can state a case directly instead of standing up a whole chunk.
fn classify_ops(ops: &[Op], word_bytes: u32) -> (Verdict, Vec<Ctor>) {
    let mut saw_non_composite_block = false;
    let mut saw_read = false;
    let mut ctors: Vec<Ctor> = Vec::new();

    for op in ops {
        if is_lowered(op) {
            continue;
        }
        if is_composite(op) {
            if is_composite_read(op) {
                saw_read = true;
            }
            if let Some(c) = classify_ctor(op, word_bytes) {
                ctors.push(c);
            }
        } else {
            saw_non_composite_block = true;
        }
    }

    let verdict = if saw_non_composite_block {
        // Blocked by something outside the class, so solving composites does
        // not free it — whether or not composites are also present.
        Verdict::BlockedElsewhereToo
    } else if !saw_read && ctors.is_empty() {
        Verdict::AlreadyLowers
    } else if ctors.is_empty() {
        Verdict::ReadsOnly
    } else {
        // Any construction at all needs per-value widths. The baked size does
        // not decide this; see the note on `Ctor`.
        Verdict::NeedsWidthRecovery
    };

    (verdict, ctors)
}

/// Corpus discovery. Mirrors `spike_corpus_coverage.rs`; the two spikes are
/// separate test binaries and cannot share a helper without a shared module.
fn corpus_sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("..");
    let mut sources: Vec<std::path::PathBuf> = Vec::new();
    for dir in [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ] {
        let d = root.join(dir);
        if let Ok(rd) = std::fs::read_dir(&d) {
            let mut stack: Vec<std::path::PathBuf> =
                rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
            while let Some(p) = stack.pop() {
                if p.is_dir() {
                    if let Ok(rd2) = std::fs::read_dir(&p) {
                        stack.extend(rd2.filter_map(|e| e.ok()).map(|e| e.path()));
                    }
                } else if p.extension().is_some_and(|x| x == "kel") {
                    sources.push(p);
                }
            }
        }
    }
    sources.sort();
    sources
}

#[test]
fn spike_report_composite_split() {
    let mut verdicts: BTreeMap<Verdict, usize> = BTreeMap::new();
    let mut ctor_kinds: BTreeMap<Ctor, usize> = BTreeMap::new();
    let mut compiled_modules = 0usize;

    for path in &corpus_sources() {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        compiled_modules += 1;
        let wb = word_bytes_of(&m);
        for chunk in &m.chunks {
            let (v, ctors) = classify_ops(&chunk.ops, wb);
            *verdicts.entry(v).or_default() += 1;
            for c in ctors {
                *ctor_kinds.entry(c).or_default() += 1;
            }
        }
    }

    let total: usize = verdicts.values().sum();
    println!("\n================ COMPOSITE BLOCKER, SPLIT IN TWO");
    println!("  modules compiled : {compiled_modules}");
    println!("  chunks total     : {total}");
    println!("\n  per-chunk verdict:");
    for (v, n) in &verdicts {
        println!("   {n:5}  {v:?}");
    }

    let reads_only = verdicts.get(&Verdict::ReadsOnly).copied().unwrap_or(0);
    let recovery = verdicts
        .get(&Verdict::NeedsWidthRecovery)
        .copied()
        .unwrap_or(0);
    let composite_blocked = reads_only + recovery;

    println!("\n  of the chunks blocked ONLY by composites ({composite_blocked}):");
    println!("   {reads_only:5}  reachable with NO shape recovery (reads only)");
    println!("   {recovery:5}  need per-value width recovery (a construction is present)");

    println!("\n  construction sites, baked size vs count*word_bytes:");
    println!("  (DESCRIPTIVE ONLY — the VM sizes a body by summing per-value");
    println!("   widths, and for a tuple or array ignores byte_size entirely.)");
    for (c, n) in &ctor_kinds {
        println!("   {n:5}  {c:?}");
    }
    println!("================\n");

    assert!(
        total > 50,
        "measured almost nothing; corpus paths are probably wrong"
    );
}

/// MUST-NOT-FIRE control for the classifier.
///
/// A classifier that answered `ReadsOnly` unconditionally would produce the
/// headline this spike is looking for, which is exactly the bias to guard
/// against. This pins the opposite direction: a chunk holding an opcode from
/// outside the composite class must NOT be reported as composite-blocked, no
/// matter how many composite ops sit beside it.
#[test]
fn control_non_composite_block_is_not_attributed_to_composites() {
    // `Op::Stream` belongs to Workstream B, not to composites.
    let ops = [
        Op::GetField(keleusma::bytecode::StructField::Flat {
            offset: 0,
            kind: keleusma::value_layout::ScalarKind::Int,
        }),
        Op::Stream,
    ];
    let (v, _) = classify_ops(&ops, 8);
    assert_eq!(
        v,
        Verdict::BlockedElsewhereToo,
        "a non-composite blocker must not be attributed to the composite class"
    );
}

/// MUST-FIRE control: the classifier does report a genuine reads-only chunk.
///
/// Paired with the test above because a control that only runs in one direction
/// is not a control — a classifier that answered `BlockedElsewhereToo`
/// unconditionally would pass that one and be useless.
#[test]
fn control_reads_only_chunk_is_reported_as_reads_only() {
    let ops = [
        Op::GetLocal(0),
        Op::GetField(keleusma::bytecode::StructField::Flat {
            offset: 0,
            kind: keleusma::value_layout::ScalarKind::Int,
        }),
        Op::Return,
    ];
    let (v, ctors) = classify_ops(&ops, 8);
    assert_eq!(v, Verdict::ReadsOnly);
    assert!(ctors.is_empty());
}

/// MUST-FIRE control, both directions, for the descriptive size comparison.
#[test]
fn control_size_consistency_is_reported_in_both_directions() {
    let consistent = Op::NewComposite(NewCompositeOperand::Flat {
        kind: keleusma::value_layout::CompositeKind::Tuple,
        count: 3,
        byte_size: 24,
    });
    assert_eq!(
        classify_ctor(&consistent, 8),
        Some(Ctor::FlatSizeConsistentWithWords)
    );

    let inconsistent = Op::NewComposite(NewCompositeOperand::Flat {
        kind: keleusma::value_layout::CompositeKind::Struct,
        count: 3,
        byte_size: 17,
    });
    assert_eq!(
        classify_ctor(&inconsistent, 8),
        Some(Ctor::FlatSizeInconsistentWithWords)
    );
}

/// MUST-FIRE control pinning the correction itself.
///
/// A size-consistent construction must STILL be classified as needing width
/// recovery. This is the exact inference the first revision of this spike got
/// wrong, so it is encoded rather than left to the prose above: if someone
/// later reintroduces the `count * word_bytes == byte_size` shortcut as a
/// lowering basis, this fails.
#[test]
fn control_size_consistent_construction_still_needs_recovery() {
    let ops = [
        Op::GetLocal(0),
        Op::GetLocal(1),
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: keleusma::value_layout::CompositeKind::Tuple,
            count: 2,
            byte_size: 16, // == 2 * 8, the coincidence
        }),
        Op::Return,
    ];
    let (v, ctors) = classify_ops(&ops, 8);
    assert_eq!(
        ctors,
        vec![Ctor::FlatSizeConsistentWithWords],
        "the size coincidence should still be observed"
    );
    assert_eq!(
        v,
        Verdict::NeedsWidthRecovery,
        "a size coincidence must not be read as licensing uniform packing"
    );
}
