//! What a composite actually looks like at the opcode level, and which slice of
//! it needs no allocator at all.
//!
//! The cost spike settled the FORMS: 300 `Flat`, 2 `FlatNested`, 0 `Boxed`. It did
//! not settle the question that decides the design, which is **where a constructed
//! body's bytes live**. In the interpreter a body is packed into the arena by
//! `pack_flat_in_arena`, so the naive native reading is that the backend needs a
//! runtime allocator before it can lower anything.
//!
//! That reading is wrong for at least one real slice. `compiler.rs` computes a
//! `private_composite_layout` giving **every private composite data slot a
//! statically baked pool offset**, described there as "linker-style fixed-address
//! placement of program state". A composite whose home is a private data slot
//! therefore has a compile-time address, and constructing it is a run of stores at
//! baked offsets from a pointer the lowering already receives.
//!
//! This spike measures how much of the corpus that covers, and dumps the op shape
//! the lowering has to reproduce. It exists because the alternative was to guess
//! whether an allocator is a prerequisite, and being wrong in the cautious
//! direction costs an increment that need not have been built.
//!
//! Run with `cargo test --test spike_composite_shape -- --nocapture --test-threads=1`.

use keleusma::bytecode::{Module, NewCompositeOperand, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

fn corpus() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = ["examples/scripts", "src/selfhost/kel"]
        .iter()
        .map(|d| root.join(d))
        .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel")
            && let Ok(src) = std::fs::read_to_string(&p)
            && let Ok(t) = tokenize(&src)
            && let Ok(a) = parse(&t)
            && let Ok(m) = compile(&a)
        {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push((name, m));
        }
    }
    out
}

/// A hand-written program is the CONTROL for every corpus claim below: if the
/// probe cannot see a composite in a source written to contain one, it is
/// measuring its own blindness rather than the corpus.
const PRIVATE_COMPOSITE_SRC: &str = "\
struct P { x: Word, y: Word }
private data st { p: P }
fn build(a: Word) -> Word {
    st.p = P { x: a, y: a + 1 };
    st.p.y
}
loop main(r: Word) -> Word { yield build(r) }
";

/// MUST-FIRE CONTROL. The probe's whole premise is that a private composite slot
/// exists and carries a baked pool offset. If this source produces no layout
/// entry, every corpus figure below is measuring nothing.
#[test]
fn the_control_source_really_has_a_baked_composite_slot() {
    let m = compile(&parse(&tokenize(PRIVATE_COMPOSITE_SRC).expect("lex")).expect("parse"))
        .expect("compile");
    assert!(
        !m.data_layout
            .as_ref()
            .is_some_and(|dl| dl.private_composite_layout.is_empty())
            && m.data_layout.is_some(),
        "the control source produced no private composite layout entry, so this \
         probe cannot distinguish a baked slot from a missing one"
    );
    assert!(
        m.persistent_composite_bytes > 0,
        "a baked slot with a zero-sized pool is not a slot"
    );
    let has_new = m
        .chunks
        .iter()
        .any(|c| c.ops.iter().any(|o| matches!(o, Op::NewComposite(_))));
    assert!(has_new, "the control source emitted no NewComposite");
}

/// The op shape the lowering must reproduce, printed rather than asserted,
/// because the point is to read it before writing the emitter.
#[test]
fn spike_report_private_composite_op_shape() {
    let m = compile(&parse(&tokenize(PRIVATE_COMPOSITE_SRC).expect("lex")).expect("parse"))
        .expect("compile");
    println!("================ CONTROL SOURCE: private composite slot");
    println!(
        "  persistent_composite_bytes : {}",
        m.persistent_composite_bytes
    );
    for e in &m
        .data_layout
        .as_ref()
        .expect("data layout")
        .private_composite_layout
    {
        println!("  layout: slot {} at pool offset {}", e.slot, e.offset);
    }
    for c in &m.chunks {
        println!("  chunk `{}` ({:?})", c.name, c.block_type);
        for (i, op) in c.ops.iter().enumerate() {
            println!("     {i:3}  {op:?}");
        }
    }
    println!("================");
}

/// How much of the corpus's composite construction lands in a slot with a baked
/// address, and how much is a temporary needing somewhere to live.
///
/// **The two need different machinery**, and the split decides whether the first
/// implementation slice needs an allocator. A `NewComposite` immediately followed
/// by a `SetData`/`SetDataIndexed` is writing to a statically placed home; one
/// whose result flows anywhere else is a temporary.
#[test]
fn spike_report_construction_destinations() {
    let mut to_slot = 0usize;
    let mut temporary = 0usize;
    let mut by_module: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for (name, m) in corpus() {
        for c in &m.chunks {
            for (i, op) in c.ops.iter().enumerate() {
                if !matches!(op, Op::NewComposite(_)) {
                    continue;
                }
                // Conservative and stated: only an IMMEDIATELY following store
                // counts as slot-homed. A construction separated from its store
                // by other ops may still be slot-homed, so this UNDER-counts the
                // easy case and never over-counts it. Erring the other way would
                // make the first slice look larger than it is.
                let homed = matches!(
                    c.ops.get(i + 1),
                    Some(Op::SetData(_)) | Some(Op::SetDataIndexed(_, _))
                );
                if homed {
                    to_slot += 1;
                    by_module.entry(name.clone()).or_default().0 += 1;
                } else {
                    temporary += 1;
                    by_module.entry(name.clone()).or_default().1 += 1;
                }
            }
        }
    }

    println!("================ SPIKE: where does a constructed body live?");
    println!("  construction into a BAKED private slot : {to_slot}");
    println!("  construction as a TEMPORARY            : {temporary}");
    println!("  (the first needs no allocator; the second needs somewhere to put it)");
    for (name, (s, t)) in &by_module {
        println!("     {name:32} slot={s:4}  temp={t:4}");
    }
    println!("================");
}

/// The flat operand carries the sizes the emitter bakes. Printed so the store run
/// can be written against real numbers rather than the 8-to-64 range the cost
/// spike reported in aggregate.
#[test]
fn spike_report_flat_operand_detail() {
    let mut sizes: BTreeMap<(u16, u16), usize> = BTreeMap::new();
    for (_, m) in corpus() {
        for c in &m.chunks {
            for op in &c.ops {
                if let Op::NewComposite(NewCompositeOperand::Flat {
                    byte_size, count, ..
                }) = op
                {
                    *sizes.entry((*byte_size, *count)).or_default() += 1;
                }
            }
        }
    }
    println!("================ SPIKE: flat construction shapes");
    println!("  (byte_size, count) -> instances");
    for ((b, n), k) in &sizes {
        println!("     ({b:4}, {n:2}) -> {k:4}");
    }
    println!("================");
}

/// **DOES A CONSTRUCTED BODY ESCAPE ITS CHUNK?**
///
/// The section-scoped bump model reclaims a region when its section exits, so a
/// body still live afterwards would be freed underneath its user. The module
/// carries the answer directly: the typed verifier's per-chunk `ChunkSignature`
/// records the flat shape of each chunk's return and parameters, so a composite
/// crossing a call boundary is visible without a dataflow pass of my own.
///
/// Escape by the two other routes is already measured: `SetData` homing is 0 of
/// 239, and a composite parameter is a composite argument at every call site.
#[test]
fn spike_report_composite_escape() {
    let (mut ret_flat, mut param_flat, mut chunks, mut sigless) = (0usize, 0usize, 0usize, 0usize);
    let mut offenders: Vec<String> = Vec::new();

    for (name, m) in corpus() {
        if m.signatures.is_empty() {
            sigless += m.chunks.len();
            continue;
        }
        for (c, sig) in m.chunks.iter().zip(m.signatures.iter()) {
            chunks += 1;
            if matches!(sig.ret, keleusma::bytecode::WireShape::Flat { .. }) {
                ret_flat += 1;
                offenders.push(format!("{name}::{} returns a flat composite", c.name));
            }
            let np = sig
                .params
                .iter()
                .filter(|p| matches!(p, keleusma::bytecode::WireShape::Flat { .. }))
                .count();
            if np > 0 {
                param_flat += 1;
                offenders.push(format!(
                    "{name}::{} takes {np} flat composite param(s)",
                    c.name
                ));
            }
        }
    }

    println!("================ SPIKE: does a composite escape its chunk?");
    println!("  chunks with a signature            : {chunks}");
    println!("  chunks in modules WITHOUT signatures: {sigless}  (unmeasured, not zero)");
    println!("  chunks RETURNING a flat composite  : {ret_flat}");
    println!("  chunks TAKING a flat composite     : {param_flat}");
    for o in offenders.iter().take(25) {
        println!("     {o}");
    }
    println!("  -> if both are 0, no constructed body outlives the chunk that");
    println!("     built it by call or return, and a per-chunk bump region with");
    println!("     reset-on-entry is sound for this corpus.");
    println!("================");
}

/// **Does the first slice need the abstract interpreter, or a local rule?**
///
/// Placing a field needs its width, and `NewComposite` bakes only the total. The
/// recorded fork was to expose `verify_typed`'s reconstructed operand stack (a
/// change to the shared crate) or to rebuild it inside this package (a fourth copy
/// of a model here). Both are unattractive.
///
/// A third option needs neither: refuse unless every operand's width is evident
/// from the instruction that PRODUCED it. That is a peephole over the preceding
/// ops rather than a dataflow pass, and it is deliberately narrower than the
/// verifier rather than a reimplementation of it. This measures what it covers.
#[test]
fn spike_report_locally_evident_operands() {
    // Producers whose result width needs no context. Word-width arithmetic and
    // comparisons, a scalar constant, and a local read of a word parameter all
    // yield one word. Anything else is refused rather than guessed.
    fn evident_word(op: &Op) -> bool {
        matches!(
            op,
            Op::Const(_)
                | Op::CheckedAdd
                | Op::CheckedSub
                | Op::CheckedMul(_)
                | Op::CheckedDiv(_)
                | Op::CheckedMod
                | Op::Div
                | Op::Mod
                | Op::CmpEq
                | Op::CmpNe
                | Op::CmpLt
                | Op::CmpLe
                | Op::CmpGt
                | Op::CmpGe
                | Op::GetLocal(_)
        )
    }

    // A nested construction's width IS locally evident: `NewComposite(Flat)`
    // bakes its own `byte_size`. Excluding it treated the best-specified
    // producer in the set as unknown.
    fn evident_any(op: &Op) -> bool {
        evident_word(op) || matches!(op, Op::NewComposite(NewCompositeOperand::Flat { .. }))
    }

    let (mut covered, mut refused, mut total) = (0usize, 0usize, 0usize);
    let mut refusal_head: BTreeMap<String, usize> = BTreeMap::new();

    for (_, m) in corpus() {
        for c in &m.chunks {
            for (i, op) in c.ops.iter().enumerate() {
                let Op::NewComposite(NewCompositeOperand::Flat { count, .. }) = op else {
                    continue;
                };
                total += 1;
                let n = *count as usize;
                // The `count` ops immediately before the construction, which is
                // where a straight-line build puts its operands. A construction
                // whose operands are not there at all is refused, which is the
                // conservative direction.
                if i < n {
                    refused += 1;
                    *refusal_head
                        .entry(String::from("<too few preceding ops>"))
                        .or_default() += 1;
                    continue;
                }
                let window = &c.ops[i - n..i];
                if window.iter().all(evident_any) {
                    covered += 1;
                } else {
                    refused += 1;
                    if let Some(bad) = window.iter().find(|o| !evident_any(o)) {
                        let key = format!("{bad:?}");
                        let key = key.split(['(', ' ']).next().unwrap_or("?").to_string();
                        *refusal_head.entry(key).or_default() += 1;
                    }
                }
            }
        }
    }

    println!("================ SPIKE: are operand widths locally evident?");
    println!("  flat construction sites          : {total}");
    println!("  every operand locally evident    : {covered}");
    println!("  refused (needs real shape info)  : {refused}");
    println!("  what blocks the refused ones, most common first:");
    let mut v: Vec<_> = refusal_head.into_iter().collect();
    v.sort_by_key(|(_, n)| core::cmp::Reverse(*n));
    for (k, n) in v.iter().take(12) {
        println!("     {n:4}  {k}");
    }
    println!("  -> a high `covered` means the first slice needs a peephole, not");
    println!("     the abstract interpreter, and neither fork has to be taken.");
    println!("================");
}

/// **The arity rule, pinned.** A module that constructs a composite takes the
/// three trailing pointers even when it declares no data slot.
///
/// Deciding arity per chunk, or along a second dimension, would make a caller
/// reproduce the backend's analysis to get the signature right — the same defect
/// the shared/private pair already refuses. This asserts the rule rather than
/// leaving it to a reader of the emitter.
#[test]
fn a_composite_module_takes_the_region_pointer() {
    use inkwell::context::Context;
    let src = "struct P { x: Word, y: Word }\n\
               fn build(a: Word) -> Word { let p = P { x: a, y: a }; p.y }\n\
               loop main(r: Word) -> Word { yield build(r) }\n";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(
        m.chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| matches!(o, Op::NewComposite(_)))),
        "the control source built no composite, so this asserts nothing"
    );
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    // Lowering may still refuse for unrelated reasons; the signature is decided
    // before any body is emitted, so declare-only is what is under test here.
    let _ = keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default());
    let f = lm.get_function("kel_chunk_0").expect("chunk 0 declared");
    let ptrs = f
        .get_type()
        .get_param_types()
        .iter()
        .filter(|t| t.is_pointer_type())
        .count();
    assert_eq!(
        ptrs, 3,
        "a composite-building module must take shared, private and region pointers"
    );
}
