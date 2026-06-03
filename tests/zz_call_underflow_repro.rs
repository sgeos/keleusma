//! Audit regression (SECURITY_AUDIT_V0_2_1, Call arity, findings 4/16).
//! The verifier rejects a Call whose argument count exceeds the callee's
//! local-slot count, so the dispatch frame setup can never underflow
//! `local_count - arg_count` (and the operand stack cannot underflow on
//! the arguments either). The runtime additionally uses checked
//! subtraction as defense in depth for new_unchecked loads.

#![cfg(feature = "verify")]

use keleusma::bytecode::{BlockType, Chunk, Op};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError};
use keleusma::{Arena, Module};

fn mk_chunk(name: &str, ops: Vec<Op>, local_count: u16) -> Chunk {
    Chunk {
        name: name.to_string(),
        ops,
        constants: vec![],
        struct_templates: vec![],
        local_count,
        param_count: 0,
        block_type: BlockType::Func,
        param_types: vec![],
        debug_pool: None,
    }
}

fn mk_module(chunks: Vec<Chunk>) -> Module {
    Module {
        schema_hash: 0,
        chunks,
        native_names: vec![],
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: keleusma::bytecode::RUNTIME_WORD_BITS_LOG2,
        addr_bits_log2: keleusma::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
        float_bits_log2: keleusma::bytecode::RUNTIME_FLOAT_BITS_LOG2,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        flags: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
    }
}

#[test]
fn call_arg_count_exceeds_callee_locals_rejected() {
    // Call(1, 10) into a callee declaring zero local slots: ten arguments
    // exceed the callee frame, which the old code computed as the
    // underflowing `local_count - arg_count`. The verifier must reject it.
    let chunk0 = mk_chunk("main", vec![Op::Call(1, 10), Op::Return], 0);
    let chunk1 = mk_chunk("callee", vec![Op::Return], 0);
    let module = mk_module(vec![chunk0, chunk1]);

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let res = Vm::new(module, &arena);
    assert!(
        matches!(res, Err(VmError::VerifyError(_))),
        "expected the verifier to reject the over-arity Call, got {:?}",
        res.map(|_| "Ok")
    );
}
