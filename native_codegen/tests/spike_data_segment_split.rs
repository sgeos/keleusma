//! RESEARCH SPIKE: how does the data-segment workstream split, and how much of
//! it is soundly lowerable today?
//!
//! The corpus coverage spike identified the data segment as 81 percent of
//! blocked chunks and therefore the next increment. This one asks the follow-up
//! question that determines whether the increment is possible: the segment
//! partitions into SHARED slots, which live in a host-owned byte buffer with an
//! explicitly specified little-endian layout table, and PRIVATE slots, which
//! live in the arena as `GenericValue` records.
//!
//! **That distinction is a soundness boundary, not a convenience.**
//! `GenericValue` is declared `#[derive(Debug, Clone)]` with NO `#[repr]`, so
//! its in-memory layout is unspecified: the compiler may reorder fields, resize
//! the discriminant, or apply niche optimisation. Native code that computed a
//! slot address and decoded a tag at a fixed offset would depend on a layout
//! the language does not guarantee. Shared slots carry no such problem, because
//! their encoding is part of the wire format.
//!
//! Reports rather than asserts, except for a guard against measuring nothing.
use keleusma::bytecode::{Op, SlotVisibility};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

#[test]
fn spike_report_data_segment_split() {
    let root = std::path::Path::new("..");
    let mut srcs: Vec<std::path::PathBuf> = Vec::new();
    for d in [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ] {
        let mut stack = vec![root.join(d)];
        while let Some(p) = stack.pop() {
            if p.is_dir() {
                if let Ok(rd) = std::fs::read_dir(&p) {
                    stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
                }
            } else if p.extension().is_some_and(|x| x == "kel") {
                srcs.push(p);
            }
        }
    }
    srcs.sort();
    let (mut shared, mut private, mut unknown) = (0usize, 0usize, 0usize);
    let mut per_op: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut modules_with_data = 0usize;
    for p in &srcs {
        let Ok(t) = std::fs::read_to_string(p) else {
            continue;
        };
        let Ok(tk) = tokenize(&t) else { continue };
        let Ok(a) = parse(&tk) else { continue };
        let Ok(m) = compile(&a) else { continue };
        // shared slots occupy the low indices; count them.
        let Some(dl) = m.data_layout.as_ref() else {
            continue;
        };
        let n_shared = dl
            .slots
            .iter()
            .filter(|s| s.visibility == SlotVisibility::Shared)
            .count();
        if !dl.slots.is_empty() {
            modules_with_data += 1;
        }
        for c in &m.chunks {
            for op in &c.ops {
                let (name, slot) = match op {
                    Op::GetData(s) => ("GetData", Some(*s as usize)),
                    Op::SetData(s) => ("SetData", Some(*s as usize)),
                    Op::GetDataIndexed(b, _) => ("GetDataIndexed", Some(*b as usize)),
                    Op::SetDataIndexed(b, _) => ("SetDataIndexed", Some(*b as usize)),
                    _ => ("", None),
                };
                if let Some(s) = slot {
                    let e = per_op.entry(name).or_default();
                    if s < n_shared {
                        shared += 1;
                        e.0 += 1;
                    } else if s < dl.slots.len() {
                        private += 1;
                        e.1 += 1;
                    } else {
                        unknown += 1;
                    }
                }
            }
        }
    }
    println!("\n=== data-segment accesses, shared vs private ===");
    println!("  modules declaring data slots: {modules_with_data}");
    println!("  SHARED  {shared}");
    println!("  PRIVATE {private}");
    println!("  slot index beyond table (unclassifiable) {unknown}");
    let tot = shared + private + unknown;
    if tot > 0 {
        println!(
            "  shared share of data accesses: {:.1}%",
            100.0 * shared as f64 / tot as f64
        );
    }
    for (k, (s, p)) in &per_op {
        println!("    {k:16} shared={s:5} private={p:5}");
    }
    // UNIT-LEVEL: how many chunks would SHARED-ONLY support actually unblock?
    // Instance counts mislead; a chunk touching one private slot stays blocked.
    let lowered = |op: &Op| {
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
        )
    };
    let (mut blocked, mut by_shared_only, mut needs_private, mut other_blocker) =
        (0usize, 0usize, 0usize, 0usize);
    for p in &srcs {
        let Ok(t) = std::fs::read_to_string(p) else {
            continue;
        };
        let Ok(tk) = tokenize(&t) else { continue };
        let Ok(a) = parse(&tk) else { continue };
        let Ok(m) = compile(&a) else { continue };
        let n_shared = m
            .data_layout
            .as_ref()
            .map(|dl| {
                dl.slots
                    .iter()
                    .filter(|s| s.visibility == SlotVisibility::Shared)
                    .count()
            })
            .unwrap_or(0);
        for c in &m.chunks {
            if c.ops.iter().all(&lowered) {
                continue;
            }
            blocked += 1;
            let mut priv_hit = false;
            let mut nondata = false;
            for op in &c.ops {
                if lowered(op) {
                    continue;
                }
                match op {
                    Op::GetData(s)
                    | Op::SetData(s)
                    | Op::GetDataIndexed(s, _)
                    | Op::SetDataIndexed(s, _) => {
                        if (*s as usize) >= n_shared {
                            priv_hit = true;
                        }
                    }
                    _ => nondata = true,
                }
            }
            if nondata {
                other_blocker += 1;
            } else if priv_hit {
                needs_private += 1;
            } else {
                by_shared_only += 1;
            }
        }
    }
    println!("\n=== UNIT LEVEL: what shared-only data support would unblock ===");
    println!("  blocked chunks total                     {blocked}");
    println!("  unblocked by SHARED-only data support    {by_shared_only}");
    println!("  data-only but need PRIVATE slots too     {needs_private}");
    println!("  blocked by something other than data     {other_blocker}");
    assert!(
        blocked > 100 && shared + private > 1000,
        "the spike measured almost nothing (blocked={blocked}, accesses={}); \
         the corpus paths are probably wrong",
        shared + private
    );
}
