//! Demonstrate true zero-copy execution of bytecode included from a
//! file at compile time via `include_bytes!`.
//!
//! The `include_bytes!` macro returns `&'static [u8; N]` whose alignment
//! is one byte. The zero-copy execution path requires the rkyv body to
//! be at an 8-byte-aligned address within the slice. The wire format
//! places the body at offset 16, so 8-byte body alignment follows from
//! 8-byte alignment of the slice base. The `repr(C, align(16))`
//! wrapper around the included array forces 16-byte alignment of the
//! base, which satisfies the 8-byte body alignment by construction.
//!
//! With the alignment in place, `unsafe Vm::view_bytes_zero_copy`
//! borrows the static slice directly. The runtime runs the program
//! against `&ArchivedModule` read from the static buffer with no
//! owned `Module` materialized. This is the embedded distribution
//! pattern where compiled bytecode lives in `.rodata` and the runtime
//! executes it in place.
//!
//! Source for the included bytecode is in
//! `examples/zero_copy_demo.kel`. To regenerate the binary after a
//! wire format change, run:
//!
//!     cargo run --example regenerate_zero_copy_bytecode
//!
//! Run this example with:
//!
//!     cargo run --example zero_copy_include_bytes

use keleusma::{Value, vm::Vm, vm::VmState};

/// Length of the included bytecode binary. Hardcoded to match the
/// file size. If the wire format changes and the file size changes,
/// regenerate the binary and update this constant.
const BYTECODE_LEN: usize = 252;

/// Align the included byte array to 16 bytes so the rkyv body at
/// offset 16 is 8-byte aligned. The body alignment is required by
/// the zero-copy execution path.
#[repr(C, align(16))]
struct AlignedBytecode([u8; BYTECODE_LEN]);

/// Bytecode loaded at compile time from a binary fixture compiled
/// from `examples/zero_copy_demo.kel`. The wrapper struct controls
/// alignment.
static BYTECODE: AlignedBytecode = AlignedBytecode(*include_bytes!("zero_copy_demo.kel.bin"));

fn main() {
    // Construct a VM that borrows the bytecode buffer directly. No
    // body copy and no `Module` materialization. The VM lifetime is
    // tied to the static buffer's `'static` lifetime.
    //
    // SAFETY. The bytecode was produced by the trusted Keleusma
    // compiler in `regenerate_zero_copy_bytecode` and was previously
    // verified at that step. The host attests through the unsafe
    // marker that the wire format and structural invariants hold.
    let mut vm: Vm<'static> =
        unsafe { Vm::view_bytes_zero_copy(&BYTECODE.0).expect("framing valid") };

    println!("buffer len {} bytes", BYTECODE.0.len());
    println!("buffer base address 0x{:x}", BYTECODE.0.as_ptr() as usize);
    println!(
        "body offset 16 address 0x{:x} (expected 8-byte alignment)",
        BYTECODE.0.as_ptr() as usize + 16
    );

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
