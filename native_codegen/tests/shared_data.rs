//! Shared data-segment slots, differential against the VM.
//!
//! # Why this file is separate from `differential.rs`
//!
//! A module with shared slots has a different calling convention. The host
//! buffer arrives as a trailing pointer parameter, so the entry point is not the
//! `fn(i64, i64) -> i64` the other harness drives, and the VM side must be
//! entered through `call_with_shared` rather than `call`.
//!
//! # The oracle here is stronger than elsewhere
//!
//! Every other differential test in this package compares a returned value. This
//! one compares **the host buffer as well**, byte for byte, after both runs.
//! That matters because a data-segment store is an effect rather than a result,
//! and a lowering that wrote the right value to the wrong offset, or the right
//! offset at the wrong width, would return an identical answer while corrupting
//! an adjacent slot. Comparing only the return value would not see it.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, auto_arena_capacity_for, shared_data_bytes_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};

/// Run `src` on the VM with an exactly-sized shared buffer, returning the
/// result and the buffer's final contents.
fn vm_shared(src: &str, args: &[i64]) -> (i64, Vec<u8>) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let n = shared_data_bytes_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut buf = vec![0u8; n];
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    let st = vm.call_with_shared(&mut buf, &vals).expect("vm run");
    match st {
        keleusma::vm::VmState::Finished(Value::Int(v)) => (v, buf),
        other => panic!("unexpected VM outcome: {other:?}"),
    }
}

/// Lower every chunk, JIT the entry, and call it with the same buffer shape.
fn native_shared(src: &str, args: &[i64]) -> (i64, Vec<u8>) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let n = shared_data_bytes_for(&m);
    let idx = m
        .chunks
        .iter()
        .position(|c| c.name == "main")
        .expect("entry chunk");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    lm.verify().expect("LLVM module verification");
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let mut buf = vec![0u8; n];
    let sym = format!("kel_chunk_{idx}");
    let out = match args.len() {
        2 => {
            let f =
                unsafe { ee.get_function::<unsafe extern "C" fn(i64, i64, *mut u8) -> i64>(&sym) }
                    .expect("symbol");
            unsafe { f.call(args[0], args[1], buf.as_mut_ptr()) }
        }
        k => panic!("harness does not drive {k}-argument entry points"),
    };
    (out, buf)
}

fn assert_shared_agrees(src: &str, args: &[i64]) {
    let (nv, nb) = native_shared(src, args);
    let (vv, vb) = vm_shared(src, args);
    assert_eq!(
        nv, vv,
        "return value differs for {src:?} args {args:?}: native={nv} vm={vv}"
    );
    assert_eq!(
        nb, vb,
        "SHARED BUFFER differs for {src:?} args {args:?}\n  native={nb:?}\n  vm    ={vb:?}"
    );
}

#[test]
fn shared_word_slots_agree_with_the_vm_in_value_and_in_buffer() {
    // A store followed by a load, so both directions are exercised, and the
    // wrapping corner because a data slot is a plain word with no checked form.
    let src = "shared data state { counter: Word, flag: Byte }
               fn main(a: Word, b: Word) -> Word { state.counter = a + b; state.counter }";
    for args in [[7, 5], [0, 0], [-3, 1], [i64::MAX, 1], [i64::MIN, -1]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_narrow_slot_does_not_disturb_its_neighbour() {
    // **THE CASE THAT JUSTIFIES COMPARING BUFFERS.** `flag` is one byte at
    // offset 8 and `counter` is eight bytes at offset 0. A lowering that stored
    // the byte at the wrong width would overwrite bytes 9 through 15, which no
    // return value would reveal, because the function returns `counter` and the
    // damage is past it.
    let src = "shared data state { counter: Word, flag: Byte, tail: Word }
               fn main(a: Word, b: Word) -> Word {
                   state.counter = a; state.tail = b; state.flag = 255 as Byte; state.counter
               }";
    for args in [[1, 2], [-1, -1], [i64::MAX, i64::MIN]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_narrow_slot_reads_back_zero_extended() {
    // **ADDED BECAUSE A MUTATION FOUND THE GAP.** Changing the narrow load from
    // zero-extension to sign-extension left every other test in this file
    // passing, because they all WRITE a byte slot and none READS one back. The
    // sign of a byte-slot read is unobservable unless a test returns it.
    //
    // `200` is the discriminating value: zero-extended it is 200, sign-extended
    // it is -56. Values below 128 agree under both and would prove nothing,
    // which is the same symmetry trap that produced three vacuous tests earlier
    // in this work.
    let src = "shared data state { flag: Byte, counter: Word }
               fn main(a: Word, b: Word) -> Word {
                   state.flag = a as Byte; (state.flag) as Word + b
               }";
    for args in [[200, 0], [255, 0], [128, 0], [127, 0], [0, 0], [1, 5]] {
        assert_shared_agrees(src, &args);
    }
}

#[test]
fn a_private_slot_is_refused_rather_than_lowered() {
    // Private storage is a later increment whose native layout is this
    // backend's own choice. Until it exists, a private access must refuse
    // rather than read the arena at a guessed offset.
    let src = "private data hidden { n: Word }
               fn main(a: Word, b: Word) -> Word { hidden.n = a; hidden.n }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    let err = lower_module(&ctx, &lm, &m, LowerOptions::default());
    assert!(
        err.is_err(),
        "a private data slot must be refused until its native layout is settled"
    );
}
