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

// Audit remediation (SECURITY_AUDIT_V0_2_1, poc_newarray_underflow). The
// VM now guards the operand-stack drain, so NewArray with too few operands
// returns a clean error instead of panicking on `len - n`. Rejecting an
// operand-stack-depth underflow at load time is the broader finding-3
// verifier-completeness work, tracked separately; the invariant asserted
// here is that the malformed chunk never panics the VM.
#[test]
fn newarray_underflow_is_a_clean_error_not_a_panic() {
    let chunk = make_chunk("main", vec![Op::NewArray(10), Op::Return]);
    let module = make_module(vec![chunk]);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    match Vm::new(module, &arena) {
        // Acceptable: a future verifier depth pass rejects this at load.
        Err(_) => {}
        // Otherwise the VM must return a clean error, never panic.
        Ok(mut vm) => {
            let result = vm.call(&[]);
            assert!(
                result.is_err(),
                "expected a clean error for the NewArray underflow, got {:?}",
                result.map(|_| "Ok")
            );
        }
    }
}
