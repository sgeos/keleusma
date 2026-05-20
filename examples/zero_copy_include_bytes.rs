//! Demonstrate true zero-copy execution of bytecode from an
//! aligned in-memory buffer.
//!
//! The zero-copy execution path requires the wire-format
//! auxiliary body to start at an 8-byte-aligned offset within
//! the input slice. The wire format places the auxiliary body
//! at the offset declared in the framing header (after the
//! opcode stream and operand pool, both 8-byte aligned). The
//! buffer's base needs 8-byte alignment for the body alignment
//! to hold; on a `Vec<u8>` produced by `rkyv::util::AlignedVec`
//! this holds by construction.
//!
//! This example compiles the demonstration script at runtime,
//! copies the resulting bytes into an `AlignedVec<8>`, and
//! constructs a `Vm` that borrows the aligned slice through
//! `Vm::view_bytes_zero_copy`. No owned `Module` is
//! materialized at any point during execution.
//!
//! Hosts that ship precompiled bytecode through `include_bytes!`
//! generate the byte fixture at build time (typically through a
//! `build.rs` writing to `OUT_DIR`) rather than checking the
//! `.kel.bin` artefact into the repository. See
//! `examples/rtos/build.rs` for the canonical build-time
//! precompile pattern. Precompiled `.kel.bin` artefacts are
//! gitignored because the wire format may shift across V0.2.x
//! patch releases.
//!
//! Run this example with:
//!
//!     cargo run --example zero_copy_include_bytes

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::{Value, vm::Vm, vm::VmState};

fn main() {
    // Compile the demonstration script at startup. The source
    // would normally live in the host's source tree or a
    // bundled `.kel` file; for this example it lives inline.
    let source = include_str!("zero_copy_demo.kel");
    let tokens = tokenize(source).expect("lex");
    let program = parse(&tokens).expect("parse");
    let module = compile(&program).expect("compile");
    let bytes = module.to_bytes().expect("encode");

    // Copy into an `AlignedVec<8>` so the buffer base is
    // 8-byte aligned. The wire format's section offsets
    // preserve alignment relative to the base.
    let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
    aligned.extend_from_slice(&bytes);

    // Construct a VM that borrows the bytecode buffer
    // directly. No body copy and no owned `Module`
    // materialization at execution time.
    //
    // SAFETY. The bytecode was produced by the trusted
    // Keleusma compiler in this same process; the host attests
    // through the unsafe marker that the wire format and
    // structural invariants hold.
    let arena = keleusma::Arena::with_capacity(64 * 1024);
    let mut vm: Vm<'_, '_> =
        unsafe { Vm::view_bytes_zero_copy(aligned.as_slice(), &arena).expect("framing valid") };

    println!("buffer len {} bytes", aligned.len());
    println!("buffer base address 0x{:x}", aligned.as_ptr() as usize);

    match vm.call(&[]) {
        Ok(VmState::Finished(Value::Int(v))) => {
            println!("program returned {}", v);
            assert_eq!(v, 42);
        }
        Ok(other) => panic!("unexpected state {:?}", other),
        Err(e) => panic!("execution error {:?}", e),
    }

    println!("zero-copy execution succeeded");
}
