//! **THE OPERAND-EFFECT TABLE AGREES WITH THE RUNTIME, OPCODE BY OPCODE.**
//!
//! `verify::op_depth_effect` states, for each opcode, how many operands it
//! requires and its net effect on the operand stack. The proven-execution
//! certificate (`verify_execution`) is computed entirely from that table, and
//! the structural depth pass relies on it too. Until 2026-10-20 the only check
//! on the table compared it with a second STATIC model (`stack_growth` minus
//! `stack_shrink`), so two tables could agree with each other and both be
//! wrong about the virtual machine.
//!
//! This module measures the runtime instead. Every instruction position of
//! each subject carries a breakpoint, so the VM suspends before every
//! instruction it executes. At each stop the operand depth of the current frame
//! is recorded, defined as the stack length less the frame base and the
//! chunk's locals. When execution next stops in the same frame, the observed
//! change must equal the table's net effect, the depth before the instruction
//! must cover its requirement, and every operand below the consumed window must
//! be unchanged. The last condition is what catches an instruction that pops
//! more than it declares and pushes the surplus back.
//!
//! # Subjects
//!
//! Most subjects are source programs, so their reach is whatever the compiler
//! emits. Several opcodes have NO compiler emission site at all. Source `+`,
//! `-`, `/` and `%` now lower to the checked forms, indexing lowers through
//! `GetIndex` rather than `BoundsCheck`, and `Len`, `Dup` and `IsStruct` are
//! folded away. Those opcodes are reached by patched subjects, which take a
//! compiled template and replace its entry chunk's instructions. A patched
//! subject is loaded through `new_unchecked`, because the subject here is the
//! runtime's behaviour and not the verifier's admission of the program.
//!
//! # What is reached, and what is not
//!
//! The coverage assertion at the end makes reach explicit. Every opcode in the
//! canonical table must be OBSERVED and CHECKED at least once, apart from the
//! exclusions listed with their reasons, and the exclusion list is asserted
//! exact in both directions. A newly added opcode that no subject reaches
//! therefore fails here, rather than passing by never being measured.
//!
//! Each opcode is checked on the operand forms and outcomes its subjects
//! happen to exercise. Both outcomes are exercised for `BoundsCheck` and the
//! division family. An operand form no subject uses is not checked.
//!
//! **One error class is invisible to this method.** An UNDERSTATED requirement
//! on an opcode that only peeks, such as `Dup`, `IsEnum`, `IsStruct` or
//! `BoundsCheck`, leaves the operands below the declared window untouched, so
//! the snapshot comparison cannot see it. An understated requirement on an
//! opcode that pops is caught, and so is a wrong net effect on any opcode.
//! Both were confirmed on 2026-10-20 by corrupting one table entry of each
//! kind and observing this test fail. With the length checks retained, such
//! an error yields `StackUnderflow`, not undefined behaviour.
//!
//! An instruction followed by a frame change is checked at the next stop in
//! its own frame. That covers `Call`, whose result appears in the caller after
//! the callee returns, and `Yield`, whose reply is supplied by the resume.

extern crate std;

use super::*;
use crate::bytecode::{Module, Op};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

/// Opcodes no subject checks, each with its reason. Asserted exact.
const EXCLUDED: &[(&str, &str)] = &[
    (
        "Return",
        "Truncates to the frame base and leaves the frame. Its requirement of one \
         operand is checked by verify_execution's own Return rule, and the caller \
         side is checked through Call.",
    ),
    (
        "Reset",
        "Truncates the operand stack to the locals and re-enters after Stream. Its \
         effect is a truncation, not a table delta, and is checked by the reset \
         reentry controls in tests/proven_execution.rs.",
    ),
    (
        "Trap",
        "Terminates execution, so no later stop exists in its frame.",
    ),
    (
        "CallExternalNative",
        "Emitted only for natives without a verified signature, which a compiled \
         program cannot produce here. CallVerifiedNative shares the same table \
         entry and dispatch arm.",
    ),
];

/// Each program is run with every instruction position armed.
/// Programs whose entry takes a parameter receive `Word 3`.
const CORPUS: &[&str] = &[
    "fn main(x: Word) -> Word { x + 7 }",
    "fn main(x: Word) -> Word { let a = x - 1; let b = a * 2; let c = b / 2; c % 5 }",
    "fn main(x: Word) -> bool { x == 3 }",
    "fn main(x: Word) -> bool { x != 3 }",
    "fn main(x: Word) -> bool { x < 3 }",
    "fn main(x: Word) -> bool { x > 3 }",
    "fn main(x: Word) -> bool { x <= 3 }",
    "fn main(x: Word) -> bool { x >= 3 }",
    "fn main(x: Word) -> bool { not (x == 3) }",
    "fn main(x: Word) -> Word { -x }",
    "fn main(x: Word) -> Word { (x band 6) bor (x bxor 1) }",
    "fn main(x: Word) -> Word { (x lsl 2) lsr 1 }",
    "fn main(x: Word) -> Word { if x > 1 { x } else { 0 } }",
    "fn main(x: Word) -> Word { for i in 0..4 { if i == x { break; } } 0 }",
    "fn helper(a: Word, b: Word) -> Word { a + b } fn main(x: Word) -> Word { helper(x, 4) }",
    "fn main(x: Word) -> Word { let a = [x, 2, 3]; a[1] + a[2] }",
    "fn main(x: Word) -> Word { let a = [x, 2, 3]; a[x] { ok(v) => v, invalid_index(i) => i } }",
    "struct P { a: Word, b: Word } fn main(x: Word) -> Word { let p = P { a: x, b: 2 }; p.a + p.b }",
    // Nested composite reads became total on 2026-10-24: the bare tail and a handled
    // read whose index is out of range, which pushes the zero-filled placeholder.
    "struct P { a: Word, b: Word } fn main(x: Word) -> Word { let ps = [P { a: x, b: 2 }, P { a: 1, b: 3 }]; ps[1].b }",
    "struct P { a: Word, b: Word } fn main(x: Word) -> Word { let ps = [P { a: x, b: 2 }]; ps[x + 5] { ok(p) => p.b, invalid_index(i) => i } }",
    "fn main(x: Word) -> Word { let t = (x, 5); t.0 + t.1 }",
    "enum E { A(Word), B } fn main(x: Word) -> Word { let e = E::A(x); match e { E::A(v) => v, E::B => 0 } }",
    "struct P { a: Word } fn main(x: Word) -> Word { let p = P { a: x }; match p { P { a } => a, _ => 0 } }",
    "fn main(x: Word) -> Byte { x as Byte }",
    "fn main(x: Word) -> Word { (x as Byte) as Word }",
    "fn main(x: Word) -> Word { ((x as Float) * 2.0) as Word }",
    "fn main(x: Word) -> Word { let f: Fixed<8> = x as Fixed<8>; let g = f * f; let h = g / f; h as Word }",
    "fn main(x: Word) -> Word { x + 1 { ok(v) => v, overflow(h, l) => l } }",
    "fn main(x: Word) -> Word { x - 1 { ok(v) => v, overflow(h, l) => l } }",
    "fn main(x: Word) -> Word { x * 2 { ok(v) => v, overflow(h, l) => l } }",
    "fn main(x: Word) -> Word { -x { ok(v) => v, overflow(h, l) => l } }",
    "fn main(x: Word) -> Word { x / 2 { ok(v) => v, zero_divisor(n) => n } }",
    "fn main(x: Word) -> Word { x % 2 { ok(v) => v, zero_divisor(n) => n } }",
    "private data d { x: Word } fn main(y: Word) -> Word { d.x = y; d.x }",
    "private data d { x: [Word; 2] } fn main(y: Word) -> Word { d.x[1] = y; d.x[1] }",
    "private data d { x: [Word; 2] } fn main(y: Word) -> Word { d.x[y - 2] = 1; 0 }",
    "use nine() -> Word\nfn main(x: Word) -> Word { nine() + x }",
    "yield main(x: Word) -> Word { let y = yield x + 1; y + 2 }",
    "loop main(x: Word) -> Word { yield x + 1 }",
];

/// The template every patched subject starts from. Its entry chunk keeps its
/// constant pool, which holds the string literal `IsStruct` and `Len` name.
const TEMPLATE: &str = "fn main(x: Word) -> Word { let s = \"abc\"; x }";

/// Instruction sequences for opcodes with no compiler emission site. Each
/// ends in `Return`, which requires at least one operand. `STR` stands for
/// the template's string constant and is resolved before loading.
const STR: u16 = u16::MAX;
fn patched_subjects() -> Vec<(&'static str, Vec<Op>)> {
    use Op::*;
    // `PushImmediate` encodes `Int(n)` as operand `n + 4`.
    let int = |n: u8| PushImmediate(n + 4);
    let pair = |op: Op, a: u8, b: u8| -> Vec<Op> { alloc::vec![int(a), int(b), op, Return] };
    alloc::vec![
        // Word addition and subtraction are the checked forms. Add and Sub
        // admit Byte, Fixed and Float.
        (
            "Add on Fixed",
            alloc::vec![int(7), WordToFixed(8), int(2), WordToFixed(8), Add, Return],
        ),
        (
            "Sub on Fixed",
            alloc::vec![int(7), WordToFixed(8), int(2), WordToFixed(8), Sub, Return],
        ),
        ("Div by nonzero", pair(Div, 7, 2)),
        ("Div by zero", pair(Div, 7, 0)),
        ("Mod by nonzero", pair(Mod, 7, 2)),
        ("Mod by zero", pair(Mod, 7, 0)),
        // Word negation is CheckedNeg. Neg admits Byte, Fixed and Float.
        (
            "Neg on Fixed",
            alloc::vec![int(7), WordToFixed(8), Neg, Return]
        ),
        ("Dup", alloc::vec![int(7), Dup, Return]),
        (
            "BoundsCheck in range",
            alloc::vec![int(1), BoundsCheck(2), Return]
        ),
        (
            "BoundsCheck out of range",
            alloc::vec![int(5), BoundsCheck(2), Return]
        ),
        (
            "BoundsCheck negative",
            alloc::vec![int(0), int(1), CheckedSub, PopN(2), BoundsCheck(2), Return],
        ),
        ("Len on a string", alloc::vec![Const(STR), Len, Return]),
        (
            "IsStruct on a Word",
            alloc::vec![int(7), IsStruct(STR), Return]
        ),
    ]
}

#[derive(Clone)]
struct Pending {
    op: Op,
    position: (usize, usize),
    depth: usize,
    /// Debug rendering of the operands below the consumed window. Flat
    /// composites print their arena handle, so this is representation
    /// identity. `PartialEq` is not used because flat composite handles read
    /// back from the stack do not compare equal to themselves under it.
    below: String,
}

fn operand_floor(vm: &Vm<'_, '_>, chunks: &[crate::bytecode::Chunk]) -> usize {
    let frame = vm.frames.last().expect("a stopped VM has a frame");
    frame.base + chunks[frame.chunk_idx].local_count as usize
}

fn name(op: &Op) -> String {
    let id = crate::wire_format::opcode_id_of(op).0;
    crate::wire_format::opcode_table()
        .iter()
        .find(|(_, i)| *i == id)
        .map(|(n, _)| String::from(*n))
        .unwrap_or_else(|| alloc::format!("unnamed opcode {id}"))
}

fn compile_source(source: &str) -> Result<Module, String> {
    let tokens = crate::lexer::tokenize(source).map_err(|e| alloc::format!("lex: {e:?}"))?;
    let ast = crate::parser::parse(&tokens).map_err(|e| alloc::format!("parse: {e:?}"))?;
    crate::compiler::compile(&ast).map_err(|e| alloc::format!("compile: {e:?}"))
}

/// Step `module` through every instruction it executes and compare each
/// observed stack change with the table.
fn check_module(
    label: &str,
    module: Module,
    verified: bool,
    checked: &mut BTreeMap<String, usize>,
    failures: &mut Vec<String>,
) {
    let chunks = module.chunks.clone();
    let need = required_persistent_capacity_for(&module);
    let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).unwrap();
    let loaded = if verified {
        Vm::new(module, &arena)
    } else {
        // SAFETY: the patched subjects are small fixed sequences whose operands
        // are pushed immediately before use. The test measures the runtime's
        // effect, so the verifier is deliberately not the gate here.
        unsafe { Vm::new_unchecked(module, &arena) }
    };
    let mut vm = match loaded {
        Ok(vm) => vm,
        Err(e) => {
            failures.push(alloc::format!("{label}\n  does not load: {e:?}"));
            return;
        }
    };
    vm.register_fn("nine", || -> i64 { 9 });
    for (c, chunk) in chunks.iter().enumerate() {
        for i in 0..chunk.ops.len() {
            vm.set_breakpoint(c, i);
        }
    }
    let wants_argument = chunks
        .iter()
        .find(|c| c.name == "main")
        .is_some_and(|c| c.param_count > 0);
    let args: &[Value] = if wants_argument {
        &[Value::Int(3)]
    } else {
        &[]
    };

    // One pending instruction per frame depth.
    let mut pending: Vec<Option<Pending>> = Vec::new();
    let mut state = vm.call(args);
    let mut steps = 0usize;
    loop {
        steps += 1;
        assert!(steps < 100_000, "{label}: runaway stepping");
        match state {
            Ok(VmState::BreakpointHit { chunk, op }) => {
                let frames = vm.frames.len();
                pending.truncate(frames);
                pending.resize(frames, None);
                let floor = operand_floor(&vm, &chunks);
                let depth = vm.stack.len() - floor;
                if let Some(before) = pending[frames - 1].take() {
                    let (required, net) =
                        crate::verify::op_depth_effect(&before.op, &chunks[before.position.0]);
                    let observed = depth as i64 - before.depth as i64;
                    let problem = if (before.depth as i64) < required as i64 {
                        Some(alloc::format!(
                            "ran at depth {} below its requirement {required}",
                            before.depth
                        ))
                    } else if observed != net as i64 {
                        Some(alloc::format!("table net {net}, observed net {observed}"))
                    } else {
                        let kept = before.depth - required as usize;
                        let now = alloc::format!("{:?}", &vm.stack[floor..floor + kept]);
                        (now != before.below).then(|| {
                            alloc::format!(
                                "changed operands below its {required}-operand window\n    \
                                 before {}\n    after  {now}",
                                before.below
                            )
                        })
                    };
                    match problem {
                        Some(p) => failures.push(alloc::format!(
                            "{label}\n  {:?} at {:?}: {p}",
                            before.op,
                            before.position
                        )),
                        None => *checked.entry(name(&before.op)).or_default() += 1,
                    }
                }
                let current = chunks[chunk].ops[op];
                let (required, _) = crate::verify::op_depth_effect(&current, &chunks[chunk]);
                let kept = depth.saturating_sub(required.max(0) as usize);
                pending[frames - 1] = Some(Pending {
                    op: current,
                    position: (chunk, op),
                    depth,
                    below: alloc::format!("{:?}", &vm.stack[floor..floor + kept]),
                });
                state = vm.resume_from_breakpoint();
            }
            Ok(VmState::Yielded(_)) => state = vm.resume(Value::Int(5)),
            // A `loop` entry has completed one full iteration.
            Ok(VmState::Finished(_)) | Ok(VmState::Reset) => break,
            Err(e) => {
                failures.push(alloc::format!("{label}\n  runtime error {e:?}"));
                break;
            }
        }
    }
}

#[test]
fn every_opcode_net_effect_matches_the_runtime() {
    let mut checked: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    for source in CORPUS {
        match compile_source(source) {
            Ok(module) => check_module(source, module, true, &mut checked, &mut failures),
            Err(e) => failures.push(alloc::format!("{source}\n  {e}")),
        }
    }

    let template = compile_source(TEMPLATE).expect("template compiles");
    let entry = template
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("template has main");
    let string_const = template.chunks[entry]
        .constants
        .iter()
        .position(|c| alloc::format!("{c:?}").contains("abc"))
        .expect("template carries its string literal") as u16;
    for (label, ops) in patched_subjects() {
        let mut module = template.clone();
        module.chunks[entry].ops = ops
            .into_iter()
            .map(|op| match op {
                Op::Const(STR) => Op::Const(string_const),
                Op::IsStruct(STR) => Op::IsStruct(string_const),
                other => other,
            })
            .collect();
        check_module(label, module, false, &mut checked, &mut failures);
    }

    assert!(
        failures.is_empty(),
        "operand-effect disagreements or broken subjects:\n{}",
        failures.join("\n")
    );

    // Coverage. Every canonical opcode is checked or explicitly excluded.
    let excluded: Vec<&str> = EXCLUDED.iter().map(|(n, _)| *n).collect();
    let mut inexact: Vec<String> = Vec::new();
    for (op, _) in crate::wire_format::opcode_table() {
        let checked_here = checked.contains_key(*op);
        let is_excluded = excluded.contains(op);
        if checked_here && is_excluded {
            inexact.push(alloc::format!(
                "{op} is excluded but a subject now checks it; remove the exclusion"
            ));
        } else if !checked_here && !is_excluded {
            inexact.push(alloc::format!("{op} was never checked by any subject"));
        }
    }
    assert!(
        inexact.is_empty(),
        "opcode coverage is not exact:\n{}\nchecked counts: {checked:?}",
        inexact.join("\n")
    );
}
