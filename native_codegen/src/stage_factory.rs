//! Native stages offered to the self-hosted driver, operator question 19 step 3.
//!
//! V0.2.X's driver reaches every stage through `keleusma::selfhost::StageFactory`
//! since absorption 118. [`NativeStageFactory`] answers that factory with a
//! [`NativeExecutor`] for each stage it can run natively, and declines the rest,
//! which the driver then runs on the virtual machine. A compile run inside
//! `keleusma::selfhost::with_stage_factory` with this factory is therefore the
//! self-hosted compiler with its stages compiled to native code.
//!
//! # What runs natively and what does not
//!
//! A stage's turns, `call_with_shared` and `resume_with_shared`, run as one
//! native call each through [`NativeStage::turn`]. The executor's other
//! operations go to an inner virtual machine built from the same module. Its
//! shared slot accessors only read the module's layout, and `call_function` is
//! what the driver uses to read the protocol constants, small functions over no
//! state. A stage that calls a host native is declined, because the natives the
//! driver registers are boxed Rust closures and the native code calls C
//! functions, so `frame.kel` stays on the virtual machine.
//!
//! # The turn rule
//!
//! The virtual machine reports a `Reset` at the end of each stream iteration
//! and the driver resumes it. A native turn runs on through the reset to the
//! next yield, so this executor never reports `Reset`. That is the rule the
//! stage differential established, and it agrees with the driver because the
//! virtual machine no longer delivers a resume's input after a `Reset`.
//!
//! # Costs this accepts
//!
//! - The driver owns the shared buffer and passes it to every turn, while the
//!   native stage keeps its own with a canary. Each turn copies the buffer in
//!   and out, so the canary still detects an overrun, at a cost proportional
//!   to the buffer's size per turn.
//! - Each build lowers and compiles its stage again and leaks one LLVM context,
//!   as `NativeStage` documents. Nothing is cached between builds.

use crate::stage::NativeStage;
use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::selfhost::{StageExecutor, StageFactory, StageNative};
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};
use std::cell::Cell;
use std::mem::ManuallyDrop;

/// A stage run natively, with an inner virtual machine for its layout queries
/// and its plain function calls.
pub struct NativeExecutor {
    stage: NativeStage,
    // Dropped before `arena`, which it borrows. See `Drop`.
    vm: ManuallyDrop<Vm<'static, 'static>>,
    arena: *mut Arena,
    started: bool,
}

impl NativeExecutor {
    /// Build `module` natively, or say why it cannot be.
    pub fn new(module: &Module) -> Result<Self, String> {
        if calls_a_host_native(module) {
            return Err(String::from(
                "the stage calls a host native, which the driver registers as a closure",
            ));
        }
        let stage = NativeStage::new(module).map_err(|e| e.to_string())?;
        let need = required_persistent_capacity_for(module);
        let arena = Box::into_raw(Box::new(Arena::with_capacity(
            DEFAULT_ARENA_CAPACITY + need,
        )));
        // SAFETY: `arena` was just allocated and is freed only in `Drop`, after
        // the virtual machine that borrows it has been dropped.
        let arena_ref: &'static mut Arena = unsafe { &mut *arena };
        let vm = arena_ref
            .resize_persistent(need)
            .map_err(|e| format!("{e:?}"))
            .and_then(|()| Vm::new(module.clone(), arena_ref).map_err(|e| format!("{e:?}")));
        let vm = match vm {
            Ok(vm) => vm,
            Err(e) => {
                // SAFETY: nothing borrows the arena, since no machine was built.
                drop(unsafe { Box::from_raw(arena) });
                return Err(e);
            }
        };
        Ok(Self {
            stage,
            vm: ManuallyDrop::new(vm),
            arena,
            started: false,
        })
    }

    fn turn(&mut self, shared: &mut [u8], reply: &Value) -> Result<VmState, VmError> {
        let Value::Int(reply) = reply else {
            return Err(VmError::TypeError(format!(
                "a native stage takes a word reply, not {reply:?}"
            )));
        };
        if shared.len() != self.stage.shared().len() {
            return Err(VmError::NativeError(format!(
                "the shared buffer holds {} bytes and the stage expects {}",
                shared.len(),
                self.stage.shared().len()
            )));
        }
        self.stage.shared_mut().copy_from_slice(shared);
        let word = self
            .stage
            .turn(*reply)
            .map_err(|e| VmError::NativeError(e.to_string()))?;
        shared.copy_from_slice(self.stage.shared());
        Ok(VmState::Yielded(Value::Int(word)))
    }
}

impl Drop for NativeExecutor {
    fn drop(&mut self) {
        // SAFETY: the machine is dropped first and never used again, and then
        // the arena it borrowed is freed exactly once.
        unsafe {
            ManuallyDrop::drop(&mut self.vm);
            drop(Box::from_raw(self.arena));
        }
    }
}

impl StageExecutor for NativeExecutor {
    fn shared_data_bytes(&self) -> usize {
        self.vm.shared_data_bytes()
    }
    fn get_shared(&self, buf: &[u8], slot: usize) -> Result<Value, VmError> {
        self.vm.get_shared(buf, slot)
    }
    fn set_shared(&self, buf: &mut [u8], slot: usize, value: Value) -> Result<(), VmError> {
        self.vm.set_shared(buf, slot, value)
    }
    fn call_with_shared(&mut self, shared: &mut [u8], args: &[Value]) -> Result<VmState, VmError> {
        let [reply] = args else {
            return Err(VmError::TypeError(format!(
                "a native stage is entered with one reply, not {} arguments",
                args.len()
            )));
        };
        self.started = true;
        self.turn(shared, reply)
    }
    fn resume_with_shared(&mut self, shared: &mut [u8], input: Value) -> Result<VmState, VmError> {
        // The driver starts a stage by trying a resume and calling it on this
        // error, as it does with the virtual machine.
        if !self.started {
            return Err(VmError::NotSuspended);
        }
        self.turn(shared, &input)
    }
    fn call_function(&mut self, chunk: usize, args: &[Value]) -> Result<VmState, VmError> {
        self.vm.call_function(chunk, args)
    }
    fn register_native(&mut self, name: &str, native: StageNative) {
        // Not reached for a stage this executor builds, which calls no native.
        self.vm.register_native_closure(name, native);
    }
}

fn calls_a_host_native(module: &Module) -> bool {
    module.chunks.iter().any(|c| {
        c.ops
            .iter()
            .any(|op| matches!(op, Op::CallVerifiedNative(..) | Op::CallExternalNative(..)))
    })
}

/// A factory that builds every stage it can natively and counts what it did.
#[derive(Default)]
pub struct NativeStageFactory {
    built: Cell<usize>,
    declined: Cell<usize>,
}

impl NativeStageFactory {
    /// Stages built natively so far.
    pub fn built(&self) -> usize {
        self.built.get()
    }

    /// Stages declined so far, which the driver ran on the virtual machine.
    pub fn declined(&self) -> usize {
        self.declined.get()
    }
}

impl StageFactory for NativeStageFactory {
    fn build(&self, module: &Module) -> Option<Box<dyn StageExecutor>> {
        match NativeExecutor::new(module) {
            Ok(executor) => {
                self.built.set(self.built.get() + 1);
                Some(Box::new(executor))
            }
            Err(_) => {
                self.declined.set(self.declined.get() + 1);
                None
            }
        }
    }
}
