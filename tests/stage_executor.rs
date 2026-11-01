//! The self-hosted driver reaches its stages through the executor boundary.
//!
//! A host installs a `StageFactory` with `with_stage_factory`, and the driver
//! offers it every stage module before building that stage. These tests show
//! that the factory is consulted, that an executor it returns is the one the
//! driver then drives, and that the compiled output does not change when the
//! returned executor is the virtual machine itself.
//!
//! # What these tests do NOT establish
//!
//! They do not show that every driver entry point builds its stages through
//! the boundary. They count the distinct stage modules a compile and a
//! validation reach, and fail if either count collapses.
#![cfg(feature = "self-host")]

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::selfhost::{
    StageExecutor, StageFactory, StageNative, compile_src, with_stage_factory,
};
use keleusma::target::Target;
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};

const SOURCE: &str = "fn f(n: Word) -> Word { if n > 2 { n * 3 } else { n + 1 } }\n\
                      fn main() -> Word { f(4) + f(1) }";

/// A pass-through executor that counts the operations the driver performs.
struct Counting {
    vm: Vm<'static, 'static>,
    calls: Rc<Cell<usize>>,
    natives: Rc<Cell<usize>>,
}

impl StageExecutor for Counting {
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
        self.calls.set(self.calls.get() + 1);
        self.vm.call_with_shared(shared, args)
    }
    fn resume_with_shared(&mut self, shared: &mut [u8], input: Value) -> Result<VmState, VmError> {
        self.calls.set(self.calls.get() + 1);
        self.vm.resume_with_shared(shared, input)
    }
    fn call_function(&mut self, chunk: usize, args: &[Value]) -> Result<VmState, VmError> {
        self.calls.set(self.calls.get() + 1);
        self.vm.call_function(chunk, args)
    }
    fn register_native(&mut self, name: &str, native: StageNative) {
        self.natives.set(self.natives.get() + 1);
        self.vm.register_native_closure(name, native);
    }
}

/// Records each distinct stage it is offered, and wraps each in `Counting`
/// when `wrap` is set. The arena is leaked because the returned executor must
/// own its borrow for as long as the driver holds it.
struct Recorder {
    seen: RefCell<BTreeSet<Vec<u8>>>,
    offered: Cell<usize>,
    wrap: bool,
    calls: Rc<Cell<usize>>,
    natives: Rc<Cell<usize>>,
}

impl StageFactory for Recorder {
    fn build(&self, module: &Module) -> Option<Box<dyn StageExecutor>> {
        self.offered.set(self.offered.get() + 1);
        self.seen
            .borrow_mut()
            .insert(module.to_bytes().expect("a stage module serializes"));
        if !self.wrap {
            return None;
        }
        let need = required_persistent_capacity_for(module);
        let arena: &'static mut Arena = Box::leak(Box::new(Arena::with_capacity(
            DEFAULT_ARENA_CAPACITY + need,
        )));
        arena.resize_persistent(need).expect("resize");
        let vm = Vm::new(module.clone(), arena).expect("a stage module verifies");
        Some(Box::new(Counting {
            vm,
            calls: Rc::clone(&self.calls),
            natives: Rc::clone(&self.natives),
        }))
    }
}

fn recorder(wrap: bool) -> Rc<Recorder> {
    Rc::new(Recorder {
        seen: RefCell::new(BTreeSet::new()),
        offered: Cell::new(0),
        wrap,
        calls: Rc::new(Cell::new(0)),
        natives: Rc::new(Cell::new(0)),
    })
}

fn compile_self_hosted() -> Vec<u8> {
    keleusma::selfhost::self_hosted_compile(SOURCE, &Target::host())
        .expect("the program is in the self-hosted subset")
        .to_bytes()
        .expect("serializes")
}

#[test]
fn a_declining_factory_is_offered_the_stages_and_changes_nothing() {
    let reference = compile_src(SOURCE).to_bytes().unwrap();
    let factory = recorder(false);
    let bytes = with_stage_factory(factory.clone(), compile_self_hosted);
    assert_eq!(bytes, reference);
    let distinct = factory.seen.borrow().len();
    eprintln!(
        "offered {} stage builds, {} distinct stage modules",
        factory.offered.get(),
        distinct
    );
    // A compile runs the lexer, parse, reconstruct, codegen and wire stages.
    assert!(
        distinct >= 5,
        "only {distinct} distinct stages reached the factory"
    );
}

#[test]
fn validation_reaches_its_stages_through_the_factory() {
    let module = compile_src(SOURCE);
    let factory = recorder(false);
    let verdict = with_stage_factory(factory.clone(), || {
        keleusma::selfhost::verify_module_via_kel(&module)
    });
    assert!(verdict.is_ok(), "{verdict:?}");
    let distinct = factory.seen.borrow().len();
    eprintln!("validation offered {distinct} distinct stage modules");
    // The structural, depth, yield and typed stages. The data-layout stage runs
    // only for a module with data, which this one has not.
    assert!(
        distinct >= 4,
        "only {distinct} distinct stages reached the factory"
    );
}

#[test]
fn a_returned_executor_is_the_one_the_driver_drives() {
    let reference = compile_src(SOURCE).to_bytes().unwrap();
    let factory = recorder(true);
    let bytes = with_stage_factory(factory.clone(), compile_self_hosted);
    assert_eq!(bytes, reference);
    assert!(
        factory.calls.get() > 0,
        "the driver built {} stages through the factory but drove none of them",
        factory.offered.get()
    );
}

#[test]
fn the_factory_is_removed_after_its_scope() {
    let factory = recorder(false);
    with_stage_factory(factory.clone(), || {});
    compile_self_hosted();
    assert_eq!(factory.offered.get(), 0);
}

/// The framing stage is built through the same boundary, and the executor a
/// factory returns receives the ten frame natives, so a native build of
/// `frame.kel` binds the code the virtual machine runs.
#[cfg(feature = "signatures")]
#[test]
fn the_framing_stage_and_its_natives_reach_the_factory() {
    let module = compile_src(SOURCE);
    let key = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let reference = keleusma::crypto_frame::signed_wire_bytes(&module, &key, false).unwrap();
    let factory = recorder(true);
    let signed = with_stage_factory(factory.clone(), || {
        keleusma::crypto_frame::signed_wire_bytes(&module, &key, false)
    })
    .unwrap();
    assert_eq!(signed, reference);
    let frame = keleusma::crypto_frame::frame_module()
        .unwrap()
        .to_bytes()
        .unwrap();
    assert!(
        factory.seen.borrow().contains(&frame),
        "the frame stage did not reach the factory"
    );
    assert!(
        factory.calls.get() > 0,
        "the returned executor was not driven"
    );
    assert_eq!(
        factory.natives.get() % 10,
        0,
        "a frame executor missed a native"
    );
    assert!(
        factory.natives.get() >= 10,
        "no frame natives were registered"
    );
}
