//! Definite initialization, narrowed. No read may observe a local that no path
//! writes.
//!
//! The operator ruled on 2026-10-28 that a read must not observe the virtual
//! machine's empty-slot `Unit`, and on 2026-10-30, "Adopt C. Then attempt to
//! maintain guarantees and widen to A and B together." Option (c) narrows the
//! rule to what the current compiler already guarantees. The rule is
//! `keleusma::verify_init`.
//!
//! # What is established
//!
//! A local that no path writes is refused at its read, including a stream local
//! written only before `Stream`. Each rejected case is a mutated compiled
//! module the structural checks otherwise accept, so the rejection is the new
//! rule's. Each accepted case is its control.
//!
//! # What is pinned as admitted, and why
//!
//! A local written on one path only, and a composite private slot read before a
//! write on some path, are both admitted. The narrowed rule refuses only what is
//! unwritten on every path, so that the compiler's handled indexed-write lowering
//! and the private-data persistence idiom stay valid. The widening, in which the
//! compiler writes every path and composite private fields gain initializers,
//! is what would move these to a refusal, and these pins will fail when it does.

use keleusma::bytecode::{Module, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::verify;

const RULE: &str = "definite-initialization rule";

fn compiled(src: &str) -> Result<Module, String> {
    let toks = tokenize(src).map_err(|e| format!("{e:?}"))?;
    let prog = parse(&toks).map_err(|e| format!("{e:?}"))?;
    compile(&prog).map_err(|e| e.message)
}

fn verdict(module: &Module) -> Result<(), String> {
    verify(module).map_err(|e| e.message)
}

fn assert_rule_rejects(result: Result<(), String>, what: &str) {
    match result {
        Err(message) => assert!(
            message.contains(RULE),
            "{what}: rejected otherwise: {message}"
        ),
        Ok(()) => panic!("{what}: accepted"),
    }
}

const POINT: &str = "struct P { x: Word, y: Word }\nprivate data d { p: P }\n";

#[test]
fn a_composite_private_slot_written_on_every_path_is_accepted() {
    for body in [
        "fn main() -> Word { d.p = P { x: 1, y: 2 }; let q = d.p; q.x }",
        "fn main(n: Word) -> Word { if n > 0 { d.p = P { x: 1, y: 2 }; } else { d.p = P { x: 3, y: 4 }; } let q = d.p; q.x }",
        // Written by the callee, read by the caller.
        "fn init() -> Word { d.p = P { x: 1, y: 2 }; 0 }\nfn main() -> Word { let z = init(); let q = d.p; q.x + z }",
        // Written by the caller, read by the callee.
        "fn get() -> Word { let q = d.p; q.x }\nfn main() -> Word { d.p = P { x: 1, y: 2 }; get() }",
    ] {
        let src = format!("{POINT}{body}");
        let module = compiled(&src).unwrap_or_else(|e| panic!("{src}\n{e}"));
        assert_eq!(verdict(&module), Ok(()), "{src}");
    }
}

/// The narrowed rule does not check private data, so a composite private slot
/// read before a write on some path is admitted. The persistence idiom depends
/// on it, and a hot swap may seed the slot from the host's `initial_data`.
#[test]
fn a_composite_private_slot_read_before_a_write_on_some_path_is_admitted() {
    for body in [
        "fn main(n: Word) -> Word { if n > 0 { d.p = P { x: 1, y: 2 }; } let q = d.p; q.x }",
        // The callee reads, and one caller has not written.
        "fn get() -> Word { let q = d.p; q.x }\nfn main(n: Word) -> Word { if n > 0 { d.p = P { x: 1, y: 2 }; } get() }",
        // A stream reads on its first pass before the write that later passes see.
        "loop main(e: Word) -> Word { let q = d.p; d.p = P { x: e, y: e }; yield q.x }",
    ] {
        let src = format!("{POINT}{body}");
        let module = compiled(&src).unwrap_or_else(|e| panic!("{src}\n{e}"));
        assert_eq!(verdict(&module), Ok(()), "{src}");
    }
}

/// A scalar private slot is written at load from its initializer, so a read
/// before any write is admitted.
#[test]
fn a_scalar_private_slot_needs_no_write_before_its_read() {
    let src = "private data c { n: Word = 5 }\nfn main() -> Word { let v = c.n; c.n = v + 1; v }";
    let module = compiled(src).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(verdict(&module), Ok(()));
}

/// Replace the first `SetLocal(slot)` in `main` with a pop of the same value,
/// so the local is never written and the stack is unchanged.
fn drop_first_write(module: &mut Module, slot: u16) -> bool {
    for op in &mut module
        .chunks
        .iter_mut()
        .find(|c| c.name == "main")
        .unwrap()
        .ops
    {
        if *op == Op::SetLocal(slot) {
            *op = Op::PopN(1);
            return true;
        }
    }
    false
}

#[test]
fn a_local_read_before_any_write_is_rejected() {
    let src = "fn main(n: Word) -> Word { let a = n + 1; a }";
    let mut module = compiled(src).unwrap();
    assert_eq!(verdict(&module), Ok(()), "control");
    assert!(drop_first_write(&mut module, 1), "no write of local 1");
    assert_rule_rejects(verdict(&module), "local 1 never written");
}

/// A local that one path writes and another does not is admitted, since the
/// narrowed rule refuses only a local that no path writes. The source writes on
/// every path, and the mutation removes the write before the branch.
#[test]
fn a_local_written_on_one_branch_only_is_admitted() {
    let src = "fn main(n: Word) -> Word { let a = if n > 0 { 1 } else { 2 }; a }";
    let mut module = compiled(src).unwrap();
    assert_eq!(verdict(&module), Ok(()), "control");
    // Remove the write on one branch only. The join then holds a path that
    // writes local 1, so the read is admitted.
    let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
    let writes: Vec<usize> = main
        .ops
        .iter()
        .enumerate()
        .filter(|(_, op)| **op == Op::SetLocal(1))
        .map(|(ip, _)| ip)
        .collect();
    assert!(!writes.is_empty(), "no write of local 1");
    if writes.len() >= 2 {
        main.ops[writes[0]] = Op::PopN(1);
    }
    assert_eq!(verdict(&module), Ok(()), "local 1 written on one path");
}

/// Reset clears every local, parameters included, so a stream may not read a
/// local after `Stream` that it wrote only before it. Slot 0, the parameter, is
/// the exception, since each resume writes it.
#[test]
fn a_stream_may_not_read_a_local_written_only_before_stream() {
    let src = "loop main(e: Word) -> Word { let n = yield e; yield n }";
    let mut module = compiled(src).unwrap();
    assert_eq!(verdict(&module), Ok(()), "control");
    let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
    let stream = main.ops.iter().position(|op| *op == Op::Stream).unwrap();
    let read = main
        .ops
        .iter()
        .skip(stream)
        .find_map(|op| match op {
            // Slot 0 is the stream's parameter, which each resume rewrites.
            Op::GetLocal(i) if *i >= 1 => Some(*i),
            _ => None,
        })
        .expect("a local read after Stream");
    // Write the read local before `Stream` and remove every write after it.
    main.ops.insert(stream, Op::SetLocal(read));
    main.ops.insert(stream, Op::PushImmediate(3));
    let mut writes_after = 0;
    for op in main.ops.iter_mut().skip(stream + 3) {
        if *op == Op::SetLocal(read) {
            *op = Op::PopN(1);
            writes_after += 1;
        }
    }
    // Inserted ops shift every forward target at or past the insertion point.
    for op in &mut main.ops {
        match op {
            Op::If(t)
            | Op::Else(t)
            | Op::Loop(t)
            | Op::EndLoop(t)
            | Op::Break(t)
            | Op::BreakIf(t)
                if (*t as usize) >= stream =>
            {
                *t += 2
            }
            _ => {}
        }
    }
    let message = verdict(&module).err().unwrap_or_default();
    assert!(
        message.contains(RULE),
        "local {read}, {writes_after} later writes removed: {message}"
    );
}

/// The verdict does not depend on which arm of an if the worklist reaches the
/// join from first. A local written on the longer arm only, and on the shorter
/// arm only, are both admitted. The rule once checked each read as the worklist
/// popped it, and so refused the first, since the shorter arm reached the join
/// before the arm that writes.
#[test]
fn a_write_on_either_arm_is_admitted_whichever_arm_is_longer() {
    for src in [
        "fn main(n: Word) -> Word { let a = 7; let r = if n > 0 { let b = n * 2 + 3 * n - 1; b } else { 5 }; r + a }",
        "fn main(n: Word) -> Word { let a = 7; let r = if n > 0 { 5 } else { let b = n * 2 + 3 * n - 1; b }; r + a }",
        "fn main(n: Word) -> Word { let a = 7; let r = if n > 0 { let b = 5; b } else { n * 2 + 3 * n - 1 }; r + a }",
    ] {
        let mut module = compiled(src).unwrap();
        assert_eq!(verdict(&module), Ok(()), "control: {src}");
        assert!(drop_first_write(&mut module, 1), "no write of local 1");
        // Move the arm's local onto local 1, so that local 1 is written on
        // that arm only and read after the join.
        let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
        for op in &mut main.ops {
            match op {
                Op::SetLocal(2) => *op = Op::SetLocal(1),
                Op::GetLocal(2) => *op = Op::GetLocal(1),
                _ => {}
            }
        }
        assert_eq!(verdict(&module), Ok(()), "{src}");
    }
}
