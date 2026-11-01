//! The boundary between the self-hosted driver and whatever runs a stage program.
//!
//! Every stage of the self-hosted toolchain is a Keleusma program that the driver seeds through
//! a shared buffer and then drives by calls and resumes. The driver reaches a stage only through
//! [`StageExecutor`], defined in [`crate::stage_executor`]. The Rust virtual machine implements it
//! with no change in behaviour, so the self-hosted gates measure this boundary.
//!
//! A host may run the stages by other means, a native build of each stage in particular, by
//! installing a [`StageFactory`] for the extent of a closure with [`with_stage_factory`]. The
//! factory is offered each stage module before the driver builds it and may return its own
//! executor, or `None` to leave that stage on the virtual machine. A factory that returns an
//! executor takes responsibility for that executor agreeing with the virtual machine on the
//! shared buffer layout and on every state it reports. The driver checks nothing further.
//!
//! # Why dynamic dispatch
//!
//! The crate prefers trait-bounded generics. Here the executor is chosen at run time by the host,
//! and a type parameter would have to be threaded through every driver function and its public
//! entry points. One virtual call per stage operation is small next to running the stage, and
//! the factory is consulted once per stage construction.
//!
//! # Scope and threads
//!
//! The installed factory is per thread. The driver runs every stage on the thread that called
//! it, and only compiles stage sources on short-lived wide-stack threads, so a factory installed
//! around a driver call is offered every stage that call runs. A host that runs a driver call on
//! another thread installs the factory there.

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;

use crate::Arena;
use crate::bytecode::Module;
pub use crate::stage_executor::{StageExecutor, StageNative};
use crate::vm::{Vm, VmError};

/// Builds the executor for a stage module, or declines it.
pub trait StageFactory {
    /// An executor for `module`, the stage program the driver is about to run, or `None` to run
    /// it on the virtual machine. `module` is the program the reference compiler produced from
    /// the stage source, so a factory identifies a stage by comparing modules. The driver has not
    /// yet verified it, and a returned executor is not verified by the driver either.
    fn build(&self, module: &Module) -> Option<Box<dyn StageExecutor>>;
}

std::thread_local! {
    static FACTORY: RefCell<Option<Rc<dyn StageFactory>>> = const { RefCell::new(None) };
}

/// Run `body` with `factory` offered every stage this thread builds, and restore the previous
/// factory afterwards, also when `body` panics.
pub fn with_stage_factory<R>(factory: Rc<dyn StageFactory>, body: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<dyn StageFactory>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            FACTORY.with(|f| *f.borrow_mut() = previous);
        }
    }
    let previous = FACTORY.with(|f| f.borrow_mut().replace(factory));
    let _restore = Restore(previous);
    body()
}

/// The executor for a stage module. The installed factory is asked first, and the virtual
/// machine runs the stage when it declines or none is installed, which verifies the module as
/// [`Vm::new`] does.
pub(crate) fn stage_vm<'arena>(
    module: Module,
    arena: &'arena Arena,
) -> Result<Box<dyn StageExecutor + 'arena>, VmError> {
    let installed = FACTORY.with(|f| f.borrow().clone());
    if let Some(factory) = installed
        && let Some(executor) = factory.build(&module)
    {
        return Ok(executor);
    }
    Ok(Box::new(Vm::new(module, arena)?))
}
