//! **A VERDICT FOR `IsStruct`, THE LAST OPCODE WITH NONE.**
//!
//! `README.md`'s disposition table carries three entries for the `63 of 66` ISA
//! census. Two are settled: `Reset` is accepted by a route the census does not
//! instrument, and `Len` is refused. The third reads **"no verdict available; no
//! corpus witness and no producer found by a bounded search"** — and an absent
//! verdict is not a negative verdict, which is the whole point of
//! `verdictless_opcodes.rs`.
//!
//! # Why no source can supply the witness
//!
//! The reference compiler emits `Op::IsStruct` only where a struct pattern's
//! annotated type differs from the pattern's own struct name, and its own note at
//! `src/compiler.rs` records that the type checker REFUSES every such program
//! before lowering. It states plainly that the opcode "HAS NO PRODUCER FOUND BY A
//! BOUNDED SEARCH, AND THAT IS NOT THE SAME AS UNREACHABLE": it is specified with
//! peek-not-pop semantics, the reference virtual machine implements it, and
//! `Vm::new_unchecked` exists so bytecode can reach the machine without the
//! compiler at all.
//!
//! So the opcode is live in the reference and unreachable from source. **Hand
//! assembly is the established technique here for exactly this situation** —
//! `probe_nesting_and_breaks.rs` assembles a chunk by hand because "the compiler
//! does not emit unbalanced breaks, which is the very property being tested, and
//! therefore cannot be used to test it."
//!
//! # What this file claims
//!
//! Only what it drives. It states, by DRIVING the backend rather than reading it,
//! what happens when a module containing `Op::IsStruct` is offered for lowering.
//! It does not claim the opcode is unreachable, nor that refusing is the only
//! defensible behaviour.

use keleusma::bytecode::{ConstValue, Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, module_refusals};

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// A module that lowers cleanly, used as the control.
const SUBJECT: &str = "struct P { x: Word }\n\
                       fn main(a: Word) -> Word { let p = P { x: a }; p.x }";

/// Inject `GetLocal(0); IsStruct(k); PopN(2)` at the head of the named chunk.
///
/// `IsStruct` PEEKS and pushes a Bool, so the sequence pushes two entries and the
/// `PopN(2)` restores the depth. Without it a refusal could be about stack
/// balance rather than about the opcode, which would confound the verdict.
fn inject_is_struct(m: &mut Module, chunk_name: &str) -> usize {
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == chunk_name)
        .expect("the control module must contain the chunk being mutated");
    let c = &mut m.chunks[idx];
    c.constants.push(ConstValue::StaticStr("P".into()));
    let k = (c.constants.len() - 1) as u16;
    let mut ops = vec![Op::GetLocal(0), Op::IsStruct(k), Op::PopN(2)];
    ops.extend(c.ops.iter().cloned());
    c.ops = ops;
    idx
}

#[test]
fn the_control_lowers_without_refusal() {
    // **NON-VACUITY.** If the control is already refused, the mutant's refusal
    // says nothing about `IsStruct`.
    let m = compile_src(SUBJECT);
    let refusals = module_refusals(&m, LowerOptions::default());
    assert!(
        refusals.is_empty(),
        "the control must lower cleanly or the mutant proves nothing; got {refusals:?}"
    );
}

#[test]
fn is_struct_offered_to_the_backend_has_a_verdict() {
    let mut m = compile_src(SUBJECT);
    inject_is_struct(&mut m, "main");
    let refusals = module_refusals(&m, LowerOptions::default());

    println!("\n  IsStruct verdict, DRIVEN not read:");
    for (chunk, err) in &refusals {
        println!("    {chunk}: {err:?}");
    }
    assert!(
        !refusals.is_empty(),
        "a module carrying `Op::IsStruct` lowered with NO refusal. That is the \
         outcome worth knowing about: the backend has no arm for this opcode, so \
         silent acceptance would mean it emitted code for an instruction it does \
         not implement."
    );
    let mentions = refusals
        .iter()
        .any(|(_, e)| format!("{e:?}").contains("IsStruct"));
    assert!(
        mentions,
        "the backend refused, but no refusal names `IsStruct`: {refusals:?}. A \
         refusal for an unrelated reason is not a verdict on this opcode."
    );
}
