//! The operations a host performs on a running stage program, as a trait.
//!
//! A stage is a Keleusma program that a host seeds through a shared buffer and then drives by
//! calls and resumes, as the self-hosted toolchain, the parse-record transport and the
//! cryptographic framing driver do. [`StageExecutor`](crate::stage_executor::StageExecutor) is exactly that set of operations, and the
//! Rust virtual machine is its reference implementation. Another executor, such as a native build
//! of a stage, takes responsibility for agreeing with the virtual machine on the shared buffer
//! layout and on every state it reports.
//!
//! The trait is object safe, so a host that chooses the executor at run time can hold a
//! `Box<dyn StageExecutor>`. The self-hosted driver does so and installs a factory for its stages
//! through `selfhost::with_stage_factory`.

use alloc::boxed::Box;

use crate::bytecode::Value;
use crate::vm::{Vm, VmError, VmState};

/// A native function as a stage receives it.
pub type StageNative = Box<dyn Fn(&[Value]) -> Result<Value, VmError>>;

/// The operations the driver performs on a running stage.
///
/// The method names and contracts are those of the same methods on [`Vm`], which is the
/// reference implementation.
pub trait StageExecutor {
    /// The size of the shared buffer the stage's data layout declares.
    fn shared_data_bytes(&self) -> usize;
    /// Read scalar shared slot `slot` from `buf`.
    fn get_shared(&self, buf: &[u8], slot: usize) -> Result<Value, VmError>;
    /// Write scalar shared slot `slot` into `buf`.
    fn set_shared(&self, buf: &mut [u8], slot: usize, value: Value) -> Result<(), VmError>;
    /// Enter the stage's entry chunk with `args`, over `shared`.
    fn call_with_shared(&mut self, shared: &mut [u8], args: &[Value]) -> Result<VmState, VmError>;
    /// Resume the stage with `input`, over `shared`.
    fn resume_with_shared(&mut self, shared: &mut [u8], input: Value) -> Result<VmState, VmError>;
    /// Call chunk `chunk` directly with `args`, outside the stream protocol.
    fn call_function(&mut self, chunk: usize, args: &[Value]) -> Result<VmState, VmError>;
    /// Register a host native under `name`.
    fn register_native(&mut self, name: &str, native: StageNative);
}

impl StageExecutor for Vm<'_, '_> {
    fn shared_data_bytes(&self) -> usize {
        Vm::shared_data_bytes(self)
    }
    fn get_shared(&self, buf: &[u8], slot: usize) -> Result<Value, VmError> {
        Vm::get_shared(self, buf, slot)
    }
    fn set_shared(&self, buf: &mut [u8], slot: usize, value: Value) -> Result<(), VmError> {
        Vm::set_shared(self, buf, slot, value)
    }
    fn call_with_shared(&mut self, shared: &mut [u8], args: &[Value]) -> Result<VmState, VmError> {
        Vm::call_with_shared(self, shared, args)
    }
    fn resume_with_shared(&mut self, shared: &mut [u8], input: Value) -> Result<VmState, VmError> {
        Vm::resume_with_shared(self, shared, input)
    }
    fn call_function(&mut self, chunk: usize, args: &[Value]) -> Result<VmState, VmError> {
        Vm::call_function(self, chunk, args)
    }
    fn register_native(&mut self, name: &str, native: StageNative) {
        self.register_native_closure(name, native);
    }
}

impl<T: StageExecutor + ?Sized> StageExecutor for Box<T> {
    fn shared_data_bytes(&self) -> usize {
        (**self).shared_data_bytes()
    }
    fn get_shared(&self, buf: &[u8], slot: usize) -> Result<Value, VmError> {
        (**self).get_shared(buf, slot)
    }
    fn set_shared(&self, buf: &mut [u8], slot: usize, value: Value) -> Result<(), VmError> {
        (**self).set_shared(buf, slot, value)
    }
    fn call_with_shared(&mut self, shared: &mut [u8], args: &[Value]) -> Result<VmState, VmError> {
        (**self).call_with_shared(shared, args)
    }
    fn resume_with_shared(&mut self, shared: &mut [u8], input: Value) -> Result<VmState, VmError> {
        (**self).resume_with_shared(shared, input)
    }
    fn call_function(&mut self, chunk: usize, args: &[Value]) -> Result<VmState, VmError> {
        (**self).call_function(chunk, args)
    }
    fn register_native(&mut self, name: &str, native: StageNative) {
        (**self).register_native(name, native);
    }
}
