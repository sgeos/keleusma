//! Adversarial reproduction for the Op::Call arg_count / local_count
//! subtraction underflow claim. Temporary verifier artifact.

#![cfg(all(feature = "verify"))]

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
fn call_arg_count_exceeds_stack_depth() {
    // chunk0: Call(1, 10) with an empty operand stack -> stack.len() - 10 underflows.
    let chunk0 = mk_chunk("main", vec![Op::Call(1, 10), Op::Return], 0);
    let chunk1 = mk_chunk("callee", vec![Op::Return], 0);
    let module = mk_module(vec![chunk0, chunk1]);

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let new_result = Vm::new(module, &arena);
    eprintln!("Vm::new result: {:?}", new_result.as_ref().map(|_| "Ok"));
    let mut vm = match new_result {
        Ok(vm) => vm,
        Err(e) => {
            eprintln!("Vm::new REJECTED: {:?}", e);
            return;
        }
    };

    // Run the entry point. Watch for panic vs error.
    let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| vm.call(&[])));
    match run {
        Ok(Ok(_state)) => eprintln!("ran cleanly (no panic, no error)"),
        Ok(Err(e)) => eprintln!("returned Err (no panic): {:?}", e),
        Err(_) => eprintln!("PANICKED during execution"),
    }
}

// silence unused warning if VmError import unused
#[allow(dead_code)]
fn _use(_e: VmError) {}
