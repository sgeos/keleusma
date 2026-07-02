#![cfg(feature = "verify")]

use keleusma::Arena;
use keleusma::bytecode::{
    BlockType, Chunk, Module, Op, RUNTIME_ADDRESS_BITS_LOG2, RUNTIME_FLOAT_BITS_LOG2,
    RUNTIME_WORD_BITS_LOG2,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError};

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
        enum_layouts: Vec::new(),
        chunks,
        native_names: Vec::new(),
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: RUNTIME_WORD_BITS_LOG2,
        addr_bits_log2: RUNTIME_ADDRESS_BITS_LOG2,
        float_bits_log2: RUNTIME_FLOAT_BITS_LOG2,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        aux_arena_bytes: 0,
        persistent_composite_bytes: 0,
        flags: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
    }
}

// Audit remediation (SECURITY_AUDIT_V0_2_1, poc_newarray_underflow,
// finding 3). A `NewComposite` array of ten elements on an empty operand
// stack would drain ten operands that are not present. The operand-stack-depth
// verifier pass now rejects it at load: `Vm::new` returns a `VerifyError`
// rather than constructing a VM whose execution would underflow. The VM's
// drain guard remains as defense in depth for `new_unchecked` loads.
#[test]
fn newarray_underflow_rejected_by_verifier() {
    let chunk = make_chunk(
        "main",
        vec![
            Op::NewComposite(keleusma::bytecode::NewCompositeOperand::Flat {
                kind: keleusma::value_layout::CompositeKind::Array,
                count: 10,
                byte_size: 80,
            }),
            Op::Return,
        ],
    );
    let module = make_module(vec![chunk]);
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let res = Vm::new(module, &arena);
    assert!(
        matches!(res, Err(VmError::VerifyError(_))),
        "expected the verifier to reject the NewArray operand-stack underflow, got {:?}",
        res.map(|_| "Ok")
    );
}
