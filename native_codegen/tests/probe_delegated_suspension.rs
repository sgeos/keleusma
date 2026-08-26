//! **What does delegated suspension actually look like in bytecode?**
//!
//! **HISTORICAL AS OF 2026-08-15, and retained deliberately.** `codegen.kel` WAS
//! the last refusal. The record described it as a design problem: it had no
//! `Yield` of its own and a `Reentrant` callee, so `resume_after_enter` writing
//! slot 0 of the ENTRY chunk could not be modelled by the degenerate-stream
//! transform, which turns a `Yield` into a return.
//!
//! The `v0.2.3` line then changed the module (`aaa87a01`), applying the nine-line
//! refactor this line had requested: `emit_next` became a plain `fn` and `main`
//! yields what it returns. **`codegen.kel` now lowers with no flag and is not a
//! delegated-suspension case.** The backend refuses nothing in the corpus.
//!
//! This file is kept because the MECHANISM it measured is still implemented and
//! still flagged off, and because the synthetic reproducer below is now its only
//! subject. What the file no longer provides is a real-module witness.
//!
//! Every increment of this arc has falsified a recorded claim, so this reads the
//! bytecode before any design is written. Prints, does not assert, except for the
//! structural facts the design will rest on.
use keleusma::bytecode::{BlockType, Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

fn module_of(path: &str) -> Module {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

/// Chunks that contain an `Op::Yield`, with their block type and yield count.
fn yielding_chunks(m: &Module) -> Vec<(usize, String, BlockType, usize)> {
    m.chunks
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            let n = c.ops.iter().filter(|o| matches!(o, Op::Yield)).count();
            (n > 0).then(|| (i, c.name.clone(), c.block_type, n))
        })
        .collect()
}

#[test]
fn what_shape_is_codegen_kels_suspension() {
    let m = module_of("../src/selfhost/kel/codegen.kel");
    let entry = m.entry_point.expect("entry point");

    println!("\n================ codegen.kel");
    println!(
        "  entry = chunk {entry} `{}` block_type={:?} params={} ops={}",
        m.chunks[entry].name,
        m.chunks[entry].block_type,
        m.chunks[entry].param_count,
        m.chunks[entry].ops.len()
    );
    println!("\n  ENTRY OPS (the whole chunk, because it is short):");
    for (ip, op) in m.chunks[entry].ops.iter().enumerate() {
        let note = match op {
            Op::Call(t, n) => format!(
                "   -> chunk {t} `{}` {:?} argc={n}",
                m.chunks[*t as usize].name, m.chunks[*t as usize].block_type
            ),
            _ => String::new(),
        };
        println!("    {ip:>3} {op:?}{note}");
    }

    let ys = yielding_chunks(&m);
    println!("\n  CHUNKS CONTAINING `Yield`: {}", ys.len());
    for (i, name, bt, n) in &ys {
        println!("    chunk {i:>4} {name:<24} {bt:?} yields={n}");
    }

    // For each yielding chunk, is every `Yield` in TAIL position -- that is, is
    // the yield the last thing the chunk does before returning? That is the
    // property that decides whether a return-based lowering can model it.
    println!("\n  PER-YIELD TAIL ANALYSIS (ops after each Yield, to the chunk end):");
    for (i, name, _, _) in ys.iter().take(12) {
        let ops = &m.chunks[*i].ops;
        for (ip, op) in ops.iter().enumerate() {
            if !matches!(op, Op::Yield) {
                continue;
            }
            let tail: Vec<String> = ops[ip + 1..].iter().map(|o| format!("{o:?}")).collect();
            println!("    chunk {i:>4} {name:<20} yield@{ip:<4} tail = {tail:?}");
        }
    }

    // Which chunks CALL a yielding chunk, and from where?
    println!("\n  CALL SITES OF A YIELDING CHUNK:");
    let yset: Vec<usize> = ys.iter().map(|(i, _, _, _)| *i).collect();
    for (ci, c) in m.chunks.iter().enumerate() {
        for (ip, op) in c.ops.iter().enumerate() {
            if let Op::Call(t, _) = op
                && yset.contains(&(*t as usize))
            {
                let tail: Vec<String> = c.ops[ip + 1..]
                    .iter()
                    .take(6)
                    .map(|o| format!("{o:?}"))
                    .collect();
                println!(
                    "    chunk {ci:>4} `{}` {:?} calls {t} `{}` at {ip}, then {tail:?}",
                    c.name, c.block_type, m.chunks[*t as usize].name
                );
            }
        }
    }

    assert!(!ys.is_empty(), "codegen.kel must contain a Yield somewhere");
}

/// **THE CLAIM THIS PROBE WAS BUILT ON IS NOW FALSE, AND THAT IS THE REFACTOR
/// LANDING RATHER THAN THE DIAGNOSIS BEING WRONG.**
///
/// The delegated-suspension refusal rested on `codegen.kel`'s entry having NO
/// `Yield` of its own, so the degenerate-stream transform — which turns a `Yield`
/// into a return — had nothing in the entry to transform.
///
/// The `v0.2.3` line changed the module in `aaa87a01`, applying the nine-line
/// refactor this line requested: `emit_next` became a plain `fn` and `main` does
/// the yielding. **The entry now has a `Yield`, the module lowers with no flag,
/// and it is no longer a delegated-suspension case at all.**
///
/// The assertion is INVERTED rather than deleted. A probe whose premise has been
/// removed by someone else's change should say so where a reader meets it;
/// deleting it would leave the design document's diagnosis looking unexamined.
/// The original diagnosis was correct for the module as it stood.
#[test]
fn the_entry_chunk_now_yields_because_the_refactor_moved_it_there() {
    let m = module_of("../src/selfhost/kel/codegen.kel");
    let entry = m.entry_point.expect("entry point");
    let n = m.chunks[entry]
        .ops
        .iter()
        .filter(|o| matches!(o, Op::Yield))
        .count();
    assert!(
        n > 0,
        "the entry chunk contains no `Yield`. Since `aaa87a01` it should contain \
         one, because `main` yields what `emit_next` returns. If this fires, that \
         refactor has been reverted and `codegen.kel` is a delegated-suspension \
         case again -- which would also make it refuse, so check \
         `module_refusals` before rederiving anything."
    );
}

/// A synthetic module of the SAME SHAPE as `codegen.kel`: a `Stream` entry whose
/// whole body is a tail call to a `Reentrant` chunk that yields in tail position.
const SHAPE: &str = "\
private data st { n: Word }

fn step() -> Word {
  st.n = st.n + 1;
  st.n
}

yield emit(resume: Word) -> Word {
  yield step()
}

loop main(resume: Word) -> Word {
  emit(resume)
}
";

/// Does the synthetic reproducer compile, and does it have the shape claimed?
#[test]
fn the_synthetic_shape_reproduces_codegen_kels_structure() {
    let m = compile(&parse(&tokenize(SHAPE).expect("lex")).expect("parse")).expect("compile");
    let entry = m.entry_point.expect("entry");
    println!("\n================ synthetic delegated-suspension shape");
    println!(
        "  entry chunk {entry} `{}` {:?} params={}",
        m.chunks[entry].name, m.chunks[entry].block_type, m.chunks[entry].param_count
    );
    for (ip, op) in m.chunks[entry].ops.iter().enumerate() {
        println!("    {ip:>3} {op:?}");
    }
    for (i, c) in m.chunks.iter().enumerate() {
        if c.ops.iter().any(|o| matches!(o, Op::Yield)) {
            println!("  yielding chunk {i} `{}` {:?}:", c.name, c.block_type);
            for (ip, op) in c.ops.iter().enumerate() {
                println!("    {ip:>3} {op:?}");
            }
        }
    }
    println!(
        "\n  refusals: {:?}",
        keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default())
            .iter()
            .map(|(n, e)| format!("{n}: {e}"))
            .collect::<Vec<_>>()
    );

    let entry_yields = m.chunks[entry]
        .ops
        .iter()
        .filter(|o| matches!(o, Op::Yield))
        .count();
    assert_eq!(
        entry_yields, 0,
        "the synthetic entry must delegate its suspension, like codegen.kel's"
    );
}

/// **The load-bearing semantic claim, executed rather than reasoned.**
///
/// In the virtual machine the resume value reaches the suspended CALLEE's operand
/// stack, but `emit`'s next op is `Return`, and the entry discards the call
/// result with `PopN(1)`. So the value pushed back into the callee is DEAD, and
/// the only live path for a resume value is the entry's slot 0, which
/// `resume_after_enter` writes.
///
/// If that holds, a return-based native lowering models this shape exactly, and
/// no continuation or suspended-frame state is needed.
#[test]
fn the_resume_value_reaches_the_entrys_slot_zero_and_is_dead_in_the_callee() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};

    // `emit` yields `step()`, which ignores `resume` entirely, so a run that only
    // counted would prove nothing about where the resume value goes. This source
    // ADDS the resume value, making the observable depend on it.
    let src = "\
private data st { n: Word }

fn step(r: Word) -> Word {
  st.n = st.n + 1;
  st.n * 100 + r
}

yield emit(resume: Word) -> Word {
  yield step(resume)
}

loop main(resume: Word) -> Word {
  emit(resume)
}
";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (1 << 20);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");

    let mut got = Vec::new();
    let mut st = vm.call(&[Value::Int(7)]).expect("first call");
    // **SENTINEL CLASS: cannot receive one.** This file compiles its own inline
    // source and loads no self-hosted stage module, so the `pe_tag_base()` /
    // `rc_fail_base()` convention cannot reach this value. Classified 2026-08-26.
    for reply in [11i64, 22, 33, 44] {
        match st {
            VmState::Yielded(Value::Int(v)) => got.push(v),
            ref other => panic!("expected a yield, got {other:?}"),
        }
        st = vm.resume(Value::Int(reply)).expect("resume");
        if matches!(st, VmState::Reset) {
            st = vm.resume(Value::Int(reply)).expect("resume after reset");
        }
    }
    println!("\n  yields with replies 11,22,33,44 after an initial arg of 7: {got:?}");

    // Iteration k yields `st.n * 100 + resume`, where resume is the value the
    // host supplied for that iteration. If the reply reaches the entry's slot 0
    // and flows back down through the call, the sequence is 107, 211, 322, 433.
    assert_eq!(
        got,
        vec![107, 211, 322, 433],
        "the resume value did not travel entry-slot-0 -> callee argument. This is \
         the semantics a return-based native lowering would model, so if it does \
         not hold, the delegated-suspension design in \
         docs/decisions/NATIVE_DELEGATED_SUSPENSION.md rests on a false premise."
    );
}
