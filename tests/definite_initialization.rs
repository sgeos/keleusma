//! Definite initialization, widened. No read may observe the virtual machine's
//! empty-slot `Unit`.
//!
//! The operator ruled on 2026-10-28 that a read must not observe that marker,
//! and on 2026-10-30, "Adopt C. Then attempt to maintain guarantees and widen to
//! A and B together." Option (c) narrowed the rule to a local that no path
//! writes. Options (a) and (b) make the compiler write every local on every path
//! and give every private slot its type's zero at load, and the rule,
//! `keleusma::verify_init`, then proves the stronger property.
//!
//! # What is established
//!
//! A local that some path leaves unwritten is refused at its read, including a
//! local written on one arm of an if only and a stream local written only before
//! `Stream`. The handled indexed write is accepted, and removing any of the
//! pre-writes the compiler adds for it is refused. Every compiled private slot
//! carries a non-`Unit` initializer short of the residual cases below, so a
//! composite private slot read before any write is accepted and reads its
//! type's zero at run time. A private slot whose initializer is `Unit` is held
//! to a write before any read on every path. Each rejected case is a mutated
//! compiled module the structural checks otherwise accept, so the rejection is
//! the rule's, and each accepted case is its control.
//!
//! # What is not established
//!
//! That a native backend may drop the run-time kind of a read. The rule makes
//! that sound for locals and private data, and no native backend exists to
//! exercise it.

#![cfg(all(feature = "compile", feature = "verify"))]

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

/// A composite private slot read before a write on some path is accepted,
/// because the slot now starts as its type's zero rather than as `Unit`. The
/// persistence idiom depends on it.
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

/// Every private slot of every admissible type but `()` gets a non-`Unit`
/// initializer, a composite its type's zero and `Text` the empty static
/// string. Counted, so that a type which silently kept `Unit` fails.
#[test]
fn every_private_slot_gets_a_non_unit_initializer() {
    use keleusma::bytecode::ConstValue;
    let src = "struct P { x: Word, y: Byte }\nenum E { A, B(Word) }\n\
               private data d { p: P, t: Text, q: [P; 3], e: E, o: Option<Word>, \
               u: (Word, bool), n: Word, s: [Text; 2] }\n\
               fn main() -> Word { d.n = d.n + 1; d.n }";
    let module = compiled(src).unwrap_or_else(|e| panic!("{e}"));
    let inits = &module
        .data_layout
        .as_ref()
        .expect("a data layout")
        .private_init;
    assert_eq!(inits.len(), 11, "{inits:?}");
    assert!(
        inits.iter().all(|v| *v != ConstValue::Unit),
        "a Unit initializer remains: {inits:?}"
    );
    assert_eq!(inits[1], ConstValue::StaticStr(String::new()));
    assert_eq!(verdict(&module), Ok(()));
}

/// A slot of type `()` keeps `Unit`, its only value, and is the residual case
/// the rule holds to a write before any read on every path. A module from an
/// older compiler, whose composite slot starts as `Unit`, is held alike.
#[test]
fn a_unit_initialized_private_slot_must_be_written_before_its_read() {
    use keleusma::bytecode::ConstValue;
    // The compiler verifies what it emits, so the refusal is its error.
    let src = "private data d { u: (), n: Word }\n\
               fn main(k: Word) -> Word { if k > 0 { d.u = (); } let v = d.u; d.n }";
    assert_rule_rejects(
        compiled(src).map(|_| ()),
        "a () slot read before a write on some path",
    );
    let src = "private data d { u: (), n: Word }\n\
               fn main(k: Word) -> Word { d.u = (); let v = d.u; d.n }";
    let module = compiled(src).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        module.data_layout.as_ref().unwrap().private_init[0],
        ConstValue::Unit
    );
    assert_eq!(verdict(&module), Ok(()), "written before the read");

    // The older compiler's module, mutated from a current one.
    let src = format!(
        "{POINT}fn main(n: Word) -> Word {{ if n > 0 {{ d.p = P {{ x: 1, y: 2 }}; }} let q = d.p; q.x }}"
    );
    let mut module = compiled(&src).unwrap();
    assert_eq!(verdict(&module), Ok(()), "control");
    module.data_layout.as_mut().unwrap().private_init[0] = ConstValue::Unit;
    assert_rule_rejects(verdict(&module), "a Unit composite read before a write");
    // Written by a callee on every path, the read is accepted.
    let src = format!(
        "{POINT}fn init() -> Word {{ d.p = P {{ x: 1, y: 2 }}; 0 }}\n\
         fn main() -> Word {{ let z = init(); let q = d.p; q.x + z }}"
    );
    let mut module = compiled(&src).unwrap();
    module.data_layout.as_mut().unwrap().private_init[0] = ConstValue::Unit;
    assert_eq!(verdict(&module), Ok(()), "a callee writes the slot");
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
/// arm only, are both refused at the same read.
#[test]
fn a_write_on_either_arm_is_refused_whichever_arm_is_longer() {
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
        assert_rule_rejects(verdict(&module), src);
    }
}

/// The handled indexed write reads its `old` and `new` slots in the arm
/// dispatch under the flag test that guards the success branch. The compiler
/// writes each bound slot with the element's zero before that test, which is
/// what makes the lowering pass the rule. Each control is accepted, and
/// removing the pre-write is refused, so the pre-write is load-bearing.
#[test]
fn the_handled_write_pre_writes_are_what_admits_it() {
    for (src, zero) in [
        (
            "private data d { x: [Word; 2] } fn main(i: Word) -> Word { d.x[i] = 7 { ok(o, n) => o * 100 + n, invalid_index(j) => j } }",
            vec![Op::PushImmediate(4)],
        ),
        (
            "private data d { x: [Byte; 2] } fn main(i: Word) -> Byte { d.x[i] = 7 as Byte { ok(o, _) => o, invalid_index(_) => 0 as Byte } }",
            vec![Op::PushImmediate(4), Op::WordToByte],
        ),
        (
            "private data d { x: [Word; 2] } fn main(i: Word) -> Word { d.x[i] = 7 { ok(_, n) => n, invalid_index(j) => j } }",
            vec![Op::PushImmediate(4)],
        ),
    ] {
        let module = compiled(src).unwrap_or_else(|e| panic!("{src}\n{e}"));
        assert_eq!(verdict(&module), Ok(()), "control: {src}");
        let ops = &module.chunks.iter().find(|c| c.name == "main").unwrap().ops;
        // Every pre-write, the zero followed by its `SetLocal`.
        let n = zero.len();
        let sites: Vec<usize> = (0..ops.len().saturating_sub(n))
            .filter(|&ip| ops[ip..ip + n] == zero[..] && matches!(ops[ip + n], Op::SetLocal(_)))
            .collect();
        assert!(!sites.is_empty(), "no pre-write in {src}");
        for ip in sites {
            let mut m = module.clone();
            let main = m.chunks.iter_mut().find(|c| c.name == "main").unwrap();
            main.ops[ip + n] = Op::PopN(1);
            assert_rule_rejects(verdict(&m), &format!("{src} without the pre-write at {ip}"));
        }
    }
}

/// A handled write on an element that is neither `Word` nor `Byte` pre-writes
/// its bound slots with the element's zero, an immediate for `bool` and a pool
/// constant for a struct, and is accepted.
#[test]
fn a_handled_write_on_any_element_type_is_accepted() {
    for src in [
        "private data d { x: [bool; 2] } fn main(i: Word) -> bool { d.x[i] = true { ok(o, _) => o, invalid_index(_) => false } }",
        "struct P { x: Word, y: Word }\nprivate data d { x: [P; 2] } fn main(i: Word) -> Word { d.x[i] = P { x: 1, y: 2 } { ok(o, n) => o.x + n.y, invalid_index(j) => j } }",
    ] {
        let module = compiled(src).unwrap_or_else(|e| panic!("{src}\n{e}"));
        assert_eq!(verdict(&module), Ok(()), "{src}");
    }
}

/// Run `main` with no arguments on a fresh virtual machine, or after a hot
/// swap to `swap` seeded with `Unit` in every private slot.
fn run_main(module: Module, swap: Option<Module>) -> keleusma::bytecode::Value {
    use keleusma::Arena;
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
    let need = required_persistent_capacity_for(&module)
        .max(swap.as_ref().map_or(0, required_persistent_capacity_for));
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(module, &arena).expect("verify");
    if let Some(next) = swap {
        let count = next
            .data_layout
            .as_ref()
            .map_or(0, |dl| dl.private_init.len());
        vm.replace_module(next, vec![Value::Unit; count])
            .expect("hot swap");
    }
    match vm.call(&[]).expect("call") {
        VmState::Finished(v) => v,
        other => panic!("expected a finished call, got {other:?}"),
    }
}

/// A composite private slot read before any write reads its type's zero, on a
/// fresh virtual machine and after a hot swap that seeds `Unit`, which now
/// means the module's own initializer. Before option (b) the slot held `Unit`.
#[test]
fn a_composite_private_slot_read_before_any_write_reads_its_zero() {
    use keleusma::bytecode::Value;
    let src = "struct P { x: Word, y: Word }\n\
               private data d { p: P, q: [P; 2], n: Word }\n\
               fn main() -> Word { let a = d.p; let b = d.q[1]; d.n = d.n + 1; \
               a.x + a.y + b.x + b.y + d.n }";
    let module = compiled(src).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(run_main(module.clone(), None), Value::Int(1));
    assert_eq!(run_main(module.clone(), Some(module)), Value::Int(1));
}
