// The twenty-four-bit data operands raise the shared-data ceiling from 64 KB
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
    let n_offset = dl.shared_slots().last().expect("n slot").offset;
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
        .fields
        .iter()
        .find(|f| f.name == "d.n")
        .map(|f| f.first_slot as usize)
        .expect("n slot");
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

// OPTION B KEEPS THE HOST'S ELEMENT INDICES.
//
// Since option B of the array wire-slot design, ruled on 2026-10-31, a data
// operand names a field rather than an element, so the operands of a module
// whose arrays precede its scalars are renumbered. The operator ruled on
// 2026-11-04, item 4, that the host keeps addressing elements and the virtual
// machine translates. These tests are written the way a host was written
// before option B, against element indices computed by counting the elements
// of every field before the one addressed, and they pin that the values read
// and written are the same.
//
// The layout interleaves arrays and scalars of two kinds so that every field
// after the first has a field index different from its element index. In
// element space `a` occupies 0 to 2, `b` is 3, `c` occupies 4 and 5, and `n` is
// 6. In field space they are 0, 1, 2 and 3.
const INTERLEAVED: &str = "shared data h { a: [Word; 3], b: Word, c: [Byte; 2], n: Word } \
    fn main(i: Word) -> Word { \
        h.b = h.a[i] + h.n; \
        h.c[1] = h.c[0]; \
        h.a[0] = h.b * 2; \
        h.n \
    }";

/// Element indices a host computed before option B.
const A0: usize = 0;
const A2: usize = 2;
const B: usize = 3;
const C0: usize = 4;
const C1: usize = 5;
const N: usize = 6;

#[test]
fn a_host_written_against_element_indices_reads_and_writes_the_same_values() {
    let module = compile_src(INTERLEAVED);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).expect("verify");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, A2, Value::Int(30)).unwrap();
    vm.set_shared(&mut shared, N, Value::Int(12)).unwrap();
    vm.set_shared(&mut shared, C0, Value::Byte(9)).unwrap();
    match vm
        .call_with_shared(&mut shared, &[Value::Int(2)])
        .expect("call")
    {
        VmState::Finished(Value::Int(n)) => assert_eq!(n, 12),
        other => panic!("unexpected result: {other:?}"),
    }
    // The program wrote `b`, `c[1]` and `a[0]`, and the host reads each at
    // the element index it always used.
    assert_eq!(vm.get_shared(&shared, B).unwrap(), Value::Int(42));
    assert_eq!(vm.get_shared(&shared, C1).unwrap(), Value::Byte(9));
    assert_eq!(vm.get_shared(&shared, A0).unwrap(), Value::Int(84));
    // The host's bound is the element count, not the field count.
    assert!(vm.get_shared(&shared, N).is_ok());
    assert!(vm.get_shared(&shared, N + 1).is_err());
}

/// The operands are field indices, which is what makes the translation above
/// necessary rather than vacuous. Before option B the scalar `b` was written
/// at slot 3 and `n` read at slot 6, so this test fails on the element-space
/// compiler.
#[test]
fn the_operands_name_fields_while_the_layout_keeps_element_slots() {
    use keleusma::bytecode::Op;
    let module = compile_src(INTERLEAVED);
    let dl = module.data_layout.as_ref().expect("data layout");
    assert_eq!(dl.field_count(), 4);
    assert_eq!(dl.slot_count(), 7);
    assert_eq!(dl.slot_of("h.b"), Some(B));
    assert_eq!(dl.field_index_of("h.b"), Some(1));
    assert_eq!(dl.slot_of("h.n"), Some(N));
    assert_eq!(dl.field_index_of("h.n"), Some(3));
    let ops: Vec<Op> = module.chunks.iter().flat_map(|c| c.ops.clone()).collect();
    assert!(ops.contains(&Op::SetData(1)), "b is written as field 1");
    assert!(ops.contains(&Op::GetData(3)), "n is read as field 3");
    assert!(ops.contains(&Op::GetDataIndexed(0, 3)), "a is field 0");
    assert!(ops.contains(&Op::SetDataIndexed(2, 2)), "c is field 2");
    assert!(
        !ops.contains(&Op::GetData(6)) && !ops.contains(&Op::SetData(3)),
        "no operand is an element index"
    );
    // The translation the runtime applies.
    assert_eq!(dl.operand_slot(1), Some(B));
    assert_eq!(dl.operand_slot(3), Some(N));
    assert_eq!(dl.operand_slot(4), None);
    assert_eq!(dl.element_addressed(Op::GetData(3)), Op::GetData(N as u32));
    assert_eq!(
        dl.element_addressed(Op::SetDataIndexed(2, 2)),
        Op::SetDataIndexed(C0 as u32, 2)
    );
    // An invalid operand is left for the consumer's own bound check.
    assert_eq!(dl.element_addressed(Op::GetData(9)), Op::GetData(9));
}

/// A span names one field. A nonempty span longer than its field is refused
/// at load, and an empty span is valid up to and including the field count,
/// which is the element-space rule restated in field space.
#[test]
fn a_data_span_is_bounded_by_its_own_field() {
    use keleusma::bytecode::Op;
    let module = compile_src(INTERLEAVED);
    let dl = module.data_layout.as_ref().expect("data layout");
    assert_eq!(dl.operand_span(0, 3), Some(0));
    assert_eq!(dl.operand_span(0, 4), None, "a has three elements");
    assert_eq!(dl.operand_span(2, 2), Some(C0));
    assert_eq!(dl.operand_span(2, 3), None, "c has two elements");
    assert_eq!(
        dl.operand_span(4, 0),
        Some(7),
        "the empty span past the end"
    );
    assert_eq!(dl.operand_span(4, 1), None);
    assert_eq!(dl.operand_span(5, 0), None);
    for (op, admitted) in [
        (Op::GetDataIndexed(0, 3), true),
        (Op::GetDataIndexed(0, 4), false),
        (Op::GetDataIndexed(1, 2), false),
        (Op::SetDataIndexed(2, 3), false),
        (Op::GetData(3), true),
        (Op::GetData(4), false),
        (Op::SetData(4), false),
    ] {
        let mut m = module.clone();
        // Replace the first data op of the probe's kind, keeping the stack
        // shape, and ask the structural verifier.
        let at = m.chunks[0]
            .ops
            .iter()
            .position(|o| std::mem::discriminant(o) == std::mem::discriminant(&op))
            .expect("a data op of the probe's kind");
        m.chunks[0].ops[at] = op;
        let verdict = keleusma::verify::verify(&m).is_ok();
        assert_eq!(verdict, admitted, "{op:?}");
    }
}
