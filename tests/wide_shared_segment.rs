// The V2 twenty-four-bit data operands raise the shared-data ceiling from 64 KB
// to 16 MB. These tests exercise the whole path for a shared segment larger than
// 64 KB: the compiler bakes byte offsets and slot indices above 65535, the wire
// format encodes and decodes them across a full serialize/deserialize round trip,
// and the VM reads and writes them through the host buffer, including the widened
// GetData/SetData (a scalar past slot 65535) and GetDataIndexed/SetDataIndexed
// (an array whose length exceeds 65535).
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

// A shared block whose byte array pushes the following scalar past the old 64 KB
// ceiling: `pad` occupies 70000 slots (and 70000 bytes), so `n` is data slot
// 70000 at byte offset 70000, both beyond `u16::MAX`. The body reads an indexed
// element (GetDataIndexed with length 70000) and writes then reads the scalar
// (SetData/GetData at slot 70000); the host seeds one high byte through the
// buffer.
const SRC: &str = "shared data d { pad: [Byte; 70000], n: Word } \
                   fn main(i: Word) -> Word { d.n = 40; d.pad[i] as Word + d.n }";

fn compile_src(src: &str) -> keleusma::bytecode::Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

#[test]
fn wide_shared_segment_compiles_and_runs() {
    let module = compile_src(SRC);
    // The `n` scalar sits at byte offset 70000, beyond the old 16-bit ceiling.
    let dl = module.data_layout.as_ref().expect("data layout");
    let n_offset = dl.shared_layout.last().expect("n slot").offset;
    assert!(
        n_offset > u16::MAX as u32,
        "expected the scalar offset {n_offset} to exceed 65535, proving the widened field is exercised"
    );

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    // Seed the byte at the high array slot the body will read (GetDataIndexed).
    vm.set_shared(&mut shared, 69_000, Value::Byte(3))
        .expect("set_shared high array slot");
    match vm
        .call_with_shared(&mut shared, &[Value::Int(69_000)])
        .expect("call")
    {
        VmState::Finished(Value::Int(n)) => assert_eq!(n, 43), // pad[i]=3 + n=40
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn wide_shared_segment_round_trips_through_the_wire() {
    // Serialize and reload, so the twenty-four-bit operands pass through the
    // wire encoder and decoder, then run the reloaded module.
    let bytes = compile_src(SRC).to_bytes().expect("encode");
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::load_bytes(&bytes, &arena).expect("load");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    // pad[50000] is left zero, so the result is just `n` (40); the point is that
    // the reloaded module's widened GetDataIndexed(len 70000) and GetData/SetData
    // (slot 70000) executed at all.
    match vm
        .call_with_shared(&mut shared, &[Value::Int(50_000)])
        .expect("call")
    {
        VmState::Finished(Value::Int(n)) => assert_eq!(n, 40),
        other => panic!("unexpected result: {other:?}"),
    }
}

#[test]
fn set_and_get_shared_reach_a_high_offset_slot() {
    // The host can address the scalar at slot 70000 (byte offset 70000) through
    // set_shared/get_shared, proving the host-side slot resolution is not capped
    // at 16 bits. A program body that reads `d.n` returns what the host wrote.
    let src = "shared data d { pad: [Byte; 70000], n: Word } \
               fn main(x: Word) -> Word { d.n + x }";
    let module = compile_src(src);
    let n_slot = module
        .data_layout
        .as_ref()
        .unwrap()
        .slots
        .iter()
        .position(|s| s.name == "d.n")
        .expect("n slot") as usize;
    assert!(n_slot > u16::MAX as usize, "n slot {n_slot} exceeds 65535");

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, n_slot, Value::Int(1234))
        .expect("set_shared high slot");
    match vm
        .call_with_shared(&mut shared, &[Value::Int(1)])
        .expect("call")
    {
        VmState::Finished(Value::Int(n)) => assert_eq!(n, 1235),
        other => panic!("unexpected result: {other:?}"),
    }
    // Read the scalar back out at the high slot.
    match vm
        .get_shared(&shared, n_slot)
        .expect("get_shared high slot")
    {
        Value::Int(n) => assert_eq!(n, 1234),
        other => panic!("unexpected shared value: {other:?}"),
    }
}
