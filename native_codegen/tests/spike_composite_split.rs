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
///
/// # Renamed 2026-08-14, and NOT retired with the other two
///
/// Two files carried a predicate called `is_lowered`; both are gone, replaced
/// by `module_refusals`, which is the real lowering. **This one is a different
/// thing that happened to share their name.** It answers a COUNTERFACTUAL — what
/// would lower if composites were supported — and no query against the real
/// entry point can answer that, for the reason above.
///
/// The name is changed so it no longer reads as a third copy of a retired
/// model, and `the_counterfactual_never_overstates` guards the one direction
/// that costs anything: claiming coverage the lowering does not deliver.
fn lowers_ignoring_composites(op: &Op) -> bool {
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
        if lowers_ignoring_composites(op) {
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

/// PROVENANCE PROBE: where does a read-only chunk's composite body come from?
///
/// The split above says 5 chunks are blocked by reads alone and therefore need
/// no shape recovery. That is a claim about the READ, and it is not yet a claim
/// that the chunk lowers, because a read needs a body to read FROM. This probe
/// names the chunks and dumps their op streams so the source of each body is
/// established by inspection rather than assumed.
///
/// The candidate sources and what each would cost:
///
/// - a `GetData` shared composite slot — currently REFUSED by
///   `resolve_shared_scalar` as "Workstream C", so it is not free;
/// - a parameter or local — needs a calling convention for passing a composite,
///   which is Workstream D and a provisional ABI decision;
/// - a `Call` result — same;
/// - a construction earlier in the same chunk — impossible here by definition,
///   since these chunks contain no `NewComposite`.
///
/// If every read-only chunk draws its body from a refused or undecided source,
/// then "reachable with no shape recovery" is true and USELESS, and the 5 is not
/// an available increment. Printing rather than asserting, because the question
/// is what the sources ARE.
#[test]
fn spike_report_read_only_chunk_provenance() {
    println!("\n================ PROVENANCE of the reads-only chunks");
    let mut found = 0usize;
    for path in &corpus_sources() {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        let wb = word_bytes_of(&m);
        for chunk in &m.chunks {
            let (v, _) = classify_ops(&chunk.ops, wb);
            if v != Verdict::ReadsOnly {
                continue;
            }
            found += 1;
            println!(
                "\n  --- {} :: chunk `{}` ({} ops, {} params)",
                path.display(),
                chunk.name,
                chunk.ops.len(),
                chunk.param_count
            );
            for (i, op) in chunk.ops.iter().enumerate() {
                let mark = if lowers_ignoring_composites(op) {
                    " "
                } else if is_composite(op) {
                    "C"
                } else {
                    "?"
                };
                println!("   {mark} {i:3}  {op:?}");
            }
        }
    }
    println!("\n  reads-only chunks found: {found}");
    println!("================\n");
}

/// THE CONJUNCTION CHECK: is a reads-only chunk worth anything on its own?
///
/// `lower_module` refuses a whole module on the first opcode it cannot handle,
/// so a lowerable chunk inside an unlowerable module buys nothing a consumer can
/// see. The provenance probe shows every reads-only chunk draws its body from a
/// parameter or a `Call` result — never from thin air. Something therefore had
/// to CONSTRUCT that body, and construction is the blocked half of the split.
///
/// This asks the question directly: of the modules containing a reads-only
/// chunk, how many contain no `NewComposite` anywhere? Those, and only those,
/// are modules the read half could free on its own.
#[test]
fn spike_report_reads_only_conjunction() {
    let mut modules_with_reads_only = 0usize;
    let mut also_construct = 0usize;
    let mut free_standing: Vec<String> = Vec::new();

    for path in &corpus_sources() {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        let wb = word_bytes_of(&m);

        let has_reads_only = m
            .chunks
            .iter()
            .any(|c| classify_ops(&c.ops, wb).0 == Verdict::ReadsOnly);
        if !has_reads_only {
            continue;
        }
        modules_with_reads_only += 1;

        let constructs_somewhere = m.chunks.iter().any(|c| c.ops.iter().any(is_composite_ctor));
        if constructs_somewhere {
            also_construct += 1;
        } else {
            free_standing.push(path.display().to_string());
        }
    }

    println!("\n================ CONJUNCTION: do reads alone free a module?");
    println!("  modules holding a reads-only chunk : {modules_with_reads_only}");
    println!("  ... which ALSO construct somewhere : {also_construct}");
    println!(
        "  ... freed by the read half alone   : {}",
        free_standing.len()
    );
    for p in &free_standing {
        println!("     {p}");
    }
    println!("================\n");
}

/// SEEDING PRECONDITION for the proposed shape stack.
///
/// The restated plan is a per-value width stack living inside `native_codegen`,
/// seeded from `module.signatures` — the typed verifier's `ChunkSignature`
/// table, which the reference compiler already populates and which is `pub`
/// along with `WireShape`. That plan has an unexamined precondition: the table
/// must actually be present and carry real shapes. A table that is absent, or
/// present and uniformly `WireShape::Top`, seeds nothing and the plan collapses
/// to "recover everything from the op stream alone".
///
/// The typed verifier is explicitly sound under an absent table — it defers
/// rather than rejects — so an all-`Top` corpus would be silently consistent
/// with a green test suite. Nothing else measures this, which is exactly the
/// shape of assumption this branch keeps getting caught by.
///
/// Reports the census. The one assertion guards against measuring nothing.
#[test]
fn spike_report_signature_seeding_quality() {
    let (mut modules, mut with_table, mut chunks_seen, mut chunks_with_entry) = (0, 0, 0, 0);
    let (mut p_top, mut p_scalar, mut p_flat) = (0usize, 0usize, 0usize);
    let (mut ret_top, mut ret_known) = (0usize, 0usize);
    // Restricted to the population the plan actually has to serve.
    let (mut needs_recovery_chunks, mut needs_recovery_seeded) = (0usize, 0usize);

    for path in &corpus_sources() {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(toks) = tokenize(&src) else { continue };
        let Ok(ast) = parse(&toks) else { continue };
        let Ok(m) = compile(&ast) else { continue };
        modules += 1;
        if !m.signatures.is_empty() {
            with_table += 1;
        }
        let wb = word_bytes_of(&m);

        for (i, chunk) in m.chunks.iter().enumerate() {
            chunks_seen += 1;
            let sig = m.signatures.get(i);
            if sig.is_some() {
                chunks_with_entry += 1;
            }
            if let Some(s) = sig {
                for p in &s.params {
                    match p {
                        keleusma::bytecode::WireShape::Top => p_top += 1,
                        keleusma::bytecode::WireShape::Scalar { .. } => p_scalar += 1,
                        keleusma::bytecode::WireShape::Flat { .. } => p_flat += 1,
                    }
                }
                match s.ret {
                    keleusma::bytecode::WireShape::Top => ret_top += 1,
                    _ => ret_known += 1,
                }
            }

            if classify_ops(&chunk.ops, wb).0 == Verdict::NeedsWidthRecovery {
                needs_recovery_chunks += 1;
                // "Seeded" here means every declared parameter has a known
                // shape, which is the condition for the stack to start from
                // solid ground rather than from Top.
                let seeded = sig.is_some_and(|s| {
                    s.params.len() >= chunk.param_count as usize
                        && s.params
                            .iter()
                            .all(|p| !matches!(p, keleusma::bytecode::WireShape::Top))
                });
                if seeded {
                    needs_recovery_seeded += 1;
                }
            }
        }
    }

    println!("\n================ SEEDING: is the signature table real?");
    println!("  modules                        : {modules}");
    println!("  ... carrying a signature table : {with_table}");
    println!("  chunks                         : {chunks_seen}");
    println!("  ... with a table entry         : {chunks_with_entry}");
    println!("\n  parameter shapes across all entries:");
    println!("   {p_scalar:5}  Scalar (known width)");
    println!("   {p_flat:5}  Flat   (known composite body size)");
    println!("   {p_top:5}  Top    (UNKNOWN — seeds nothing)");
    println!("\n  return shapes: {ret_known} known, {ret_top} Top");
    println!("\n  the population the plan must serve:");
    println!("   {needs_recovery_chunks:5}  chunks needing width recovery");
    println!("   {needs_recovery_seeded:5}  ... whose every parameter is seeded");
    println!("================\n");

    assert!(
        chunks_seen > 50,
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

/// **The counterfactual must never OVERSTATE.**
///
/// A model that under-states wastes nothing — figures built on it are merely
/// conservative. A model that claims an op lowers where the real lowering
/// refuses promises coverage that does not exist, and every estimate built on it
/// is then wrong in the expensive direction.
///
/// Checkable without resolving the counterfactual: over any module whose ops the
/// model says ALL lower — so it is asserting no composite is involved — the real
/// lowering must accept it.
#[test]
fn the_counterfactual_never_overstates() {
    let mut checked = 0usize;
    let mut overstated: Vec<String> = Vec::new();
    for path in corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        if !m
            .chunks
            .iter()
            .all(|c| c.ops.iter().all(lowers_ignoring_composites))
        {
            continue;
        }
        checked += 1;
        if !keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default())
            .is_empty()
        {
            overstated.push(
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into(),
            );
        }
    }
    println!("  modules where the counterfactual claims full coverage: {checked}");
    assert!(
        overstated.is_empty(),
        "the counterfactual OVERSTATES on {} module(s): it calls every op lowered \
         while the real lowering refuses them. Modules: {:?}",
        overstated.len(),
        overstated
    );
}
