//! Definite initialization. No read may observe an empty slot.
//!
//! The operator ruled on 2026-10-28 that the verifier rejects a module in which
//! a read may observe the virtual machine's empty-slot `Unit`, a local before
//! its first write or a private data slot that has no load-time initializer
//! before its first write. The rule is `keleusma::verify_init`.
//!
//! # What is established
//!
//! Each rejected case is a module the structural checks otherwise accept, so
//! the rejection is the new rule's. Each accepted case is its control, the same
//! shape with the write on every path. The private-data cases are source
//! programs, since the compiler emits such reads. The local cases mutate a
//! compiled chunk, since the compiler never emits a read of an unwritten local.
//!
//! # What is NOT established
//!
//! Every program the repository compiles is accepted, which the full test
//! suite measures. That the rule is complete, that it accepts every module in
//! which no such read is possible, is not claimed. It rejects a read on a path
//! the program cannot take at run time.

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

#[test]
fn a_composite_private_slot_read_before_a_write_on_some_path_is_rejected() {
    for body in [
        "fn main(n: Word) -> Word { if n > 0 { d.p = P { x: 1, y: 2 }; } let q = d.p; q.x }",
        // The callee reads, and one caller has not written.
        "fn get() -> Word { let q = d.p; q.x }\nfn main(n: Word) -> Word { if n > 0 { d.p = P { x: 1, y: 2 }; } get() }",
        // A stream reads on its first pass before the write that later passes see.
        "loop main(e: Word) -> Word { let q = d.p; d.p = P { x: e, y: e }; yield q.x }",
    ] {
        let src = format!("{POINT}{body}");
        // Compilation runs the verifier, so the rule rejects at compile time.
        match compiled(&src) {
            Err(message) => assert!(
                message.contains(RULE),
                "{src}\nrejected otherwise: {message}"
            ),
            Ok(module) => assert_rule_rejects(verdict(&module), &src),
        }
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

#[test]
fn a_local_written_on_one_branch_only_is_rejected() {
    let src = "fn main(n: Word) -> Word { let a = 0; if n > 0 { a = 1; } a }";
    let module = compiled(src);
    // The source form writes on every path. The mutation removes the write
    // before the branch, leaving the write inside it.
    let Ok(mut module) = module else {
        // A language without local reassignment cannot express this form. The
        // two-branch form below is the fallback.
        let src = "fn main(n: Word) -> Word { let a = if n > 0 { 1 } else { 2 }; a }";
        let mut module = compiled(src).unwrap();
        assert_eq!(verdict(&module), Ok(()), "control");
        assert!(drop_first_write(&mut module, 1));
        assert_rule_rejects(verdict(&module), "local 1 never written");
        return;
    };
    assert_eq!(verdict(&module), Ok(()), "control");
    assert!(drop_first_write(&mut module, 1));
    assert_rule_rejects(verdict(&module), "local 1 written on one branch");
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
