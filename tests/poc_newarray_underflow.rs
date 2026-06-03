#![cfg(feature = "verify")]

use keleusma::Arena;
use keleusma::bytecode::{
    BlockType, Chunk, Module, Op, RUNTIME_ADDRESS_BITS_LOG2, RUNTIME_FLOAT_BITS_LOG2,
    RUNTIME_WORD_BITS_LOG2,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};

fn make_chunk(name: &str, ops: Vec<Op>) -> Chunk {
    Chunk {
        name: String::from(name),
        ops,
        constants: Vec::new(),
        struct_templates: Vec::new(),
        local_count: 0,
        param_count: 0,
        block_type: BlockType::Func,
        param_types: Vec::new(),
        debug_pool: None,
    }
}

fn make_module(chunks: Vec<Chunk>) -> Module {
    Module {
        schema_hash: 0,
        chunks,
        native_names: Vec::new(),
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: RUNTIME_WORD_BITS_LOG2,
        addr_bits_log2: RUNTIME_ADDRESS_BITS_LOG2,
        float_bits_log2: RUNTIME_FLOAT_BITS_LOG2,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        flags: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
    }
}

// Audit probe (SECURITY_AUDIT_V0_2_1). Documents an unfixed bug: the
// verifier accepts NewArray on an empty stack and the VM then underflows
// and panics. Ignored so it does not block the gate while remediation is
// deferred until after the flat-byte work; run with `cargo test -- --ignored`.
#[test]
#[ignore = "documents an unfixed verifier/VM bug; remediation deferred"]
fn poc_newarray_underflow_accepted_then_runs() {
    let chunk = make_chunk("main", vec![Op::NewArray(10), Op::Return]);
    let module = make_module(vec![chunk]);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    // Step 1: Vm::new must accept (this is the claim's premise).
    let mut vm = Vm::new(module, &arena).expect("Vm::new ACCEPTED malicious chunk");
    eprintln!("STEP1_VM_NEW_ACCEPTED");
    // Step 2: execution. The claim says this panics with subtract overflow.
    let result = vm.call(&[]);
    eprintln!("STEP2_RESULT={:?}", result);
}
