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
//! state.
//!
//! # Host natives
//!
//! The driver registers a stage's natives as boxed Rust closures through
//! `register_native`, and native code calls C functions. Each native a stage
//! calls is therefore bound to a trampoline, a C function fixed by its arity
//! and its slot in the stage's native list, when the stage is built. During a
//! turn the executor lends its closures to a thread-local table, and the
//! trampoline calls the closure in its slot with its arguments as words and
//! returns the word it gives. This is how `frame.kel` runs natively.
//!
//! A native's error cannot reach native code, because the lowering treats a
//! native call outside a coroutine as infallible. The trampoline records the
//! first error and returns zero, the stage runs on to its next yield, and the
//! turn then fails with that error. This is stricter than the virtual machine,
//! which stops at a native whose call does not reify failure and hands a
//! failure flag to the script at one that does, as every call in `frame.kel`
//! does. V0.2.X states that a correct framing run reaches no native error, so
//! the two agree wherever the virtual machine's run is correct, and a native
//! run refuses where the script would have seen a failure flag. A native
//! returning anything but a word fails the turn the same way. At most [`MAX_BRIDGED_NATIVES`] distinct natives of arity at
//! most three are bridged, and a stage needing more is declined.
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
//! - The driver owns the shared buffer and passes it to every turn, and a turn
//!   runs the native code on it directly through `NativeStage::turn_with_shared`.
//!   Copying it in and out instead, so that a canary could follow it, cost about
//!   forty-five times the native execution on 2026-10-30. A write past the shared
//!   segment is therefore undetected here, and the private and region buffers
//!   keep their canaries.
//! - [`NativeStageFactory`] compiles each distinct stage module once and leaks
//!   one LLVM context for it, as `NativeStage` documents. A factory lives as
//!   long as its caller keeps it, so the leak is bounded by the stage modules
//!   that caller's factories see. One inner virtual machine per compiled stage
//!   serves every run of it.

use crate::stage::{CompiledStage, HostNative, NativeStage};
use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::selfhost::{StageExecutor, StageFactory, StageNative};
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};
use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::rc::Rc;

/// The most distinct host natives one stage may have bridged. `frame.kel`
/// calls ten.
pub const MAX_BRIDGED_NATIVES: usize = 16;

thread_local! {
    /// The closures of the stage whose turn is running, by bridge slot.
    static ACTIVE: RefCell<Vec<Option<StageNative>>> = const { RefCell::new(Vec::new()) };
    /// The first error a bridged native reported during the running turn.
    static FAULT: RefCell<Option<VmError>> = const { RefCell::new(None) };
}

fn dispatch(slot: usize, args: &[i64]) -> i64 {
    let mut values = [Value::Int(0), Value::Int(0), Value::Int(0)];
    for (v, &a) in values.iter_mut().zip(args) {
        *v = Value::Int(a);
    }
    let result = ACTIVE.with(|active| match active.borrow().get(slot) {
        Some(Some(native)) => native(&values[..args.len()]),
        _ => Err(VmError::NativeError(format!(
            "bridge slot {slot} has no registered native"
        ))),
    });
    let fault = match result {
        Ok(Value::Int(word)) => return word,
        Ok(other) => VmError::TypeError(format!(
            "a bridged native returned {other:?}, and native code takes a word"
        )),
        Err(e) => e,
    };
    FAULT.with(|f| {
        f.borrow_mut().get_or_insert(fault);
    });
    0
}

extern "C" fn bridge0<const S: usize>() -> i64 {
    dispatch(S, &[])
}
extern "C" fn bridge1<const S: usize>(a: i64) -> i64 {
    dispatch(S, &[a])
}
extern "C" fn bridge2<const S: usize>(a: i64, b: i64) -> i64 {
    dispatch(S, &[a, b])
}
extern "C" fn bridge3<const S: usize>(a: i64, b: i64, c: i64) -> i64 {
    dispatch(S, &[a, b, c])
}

macro_rules! slots {
    ($f:ident, $t:ty) => {
        [
            $f::<0> as $t,
            $f::<1>,
            $f::<2>,
            $f::<3>,
            $f::<4>,
            $f::<5>,
            $f::<6>,
            $f::<7>,
            $f::<8>,
            $f::<9>,
            $f::<10>,
            $f::<11>,
            $f::<12>,
            $f::<13>,
            $f::<14>,
            $f::<15>,
        ]
    };
}

/// The trampoline for `slot` taking `arity` words, or `None` past the limits.
fn trampoline(slot: usize, arity: u8) -> Option<HostNative> {
    type F0 = unsafe extern "C" fn() -> i64;
    type F1 = unsafe extern "C" fn(i64) -> i64;
    type F2 = unsafe extern "C" fn(i64, i64) -> i64;
    type F3 = unsafe extern "C" fn(i64, i64, i64) -> i64;
    let t0: [F0; MAX_BRIDGED_NATIVES] = slots!(bridge0, F0);
    let t1: [F1; MAX_BRIDGED_NATIVES] = slots!(bridge1, F1);
    let t2: [F2; MAX_BRIDGED_NATIVES] = slots!(bridge2, F2);
    let t3: [F3; MAX_BRIDGED_NATIVES] = slots!(bridge3, F3);
    match arity {
        0 => t0.get(slot).copied().map(HostNative::Arity0),
        1 => t1.get(slot).copied().map(HostNative::Arity1),
        2 => t2.get(slot).copied().map(HostNative::Arity2),
        3 => t3.get(slot).copied().map(HostNative::Arity3),
        _ => None,
    }
}

/// The natives `module` calls, each with its one arity, in first-call order.
fn called_natives(module: &Module) -> Result<Vec<(String, u8)>, String> {
    let mut out: Vec<(String, u8)> = Vec::new();
    for op in module.chunks.iter().flat_map(|c| &c.ops) {
        let (Op::CallVerifiedNative(index, n) | Op::CallExternalNative(index, n)) = *op else {
            continue;
        };
        // The high bit asks the virtual machine to reify a native's failure as a
        // flag. The low seven bits are the argument count.
        let argc = n & 0x7F;
        let name = module
            .native_names
            .get(usize::from(index))
            .ok_or_else(|| format!("native index {index} is past the native table"))?;
        match out.iter().find(|(n, _)| n == name) {
            Some((_, a)) if *a != argc => {
                return Err(format!(
                    "`{name}` is called with {a} and with {argc} arguments"
                ));
            }
            Some(_) => {}
            None => out.push((name.clone(), argc)),
        }
    }
    Ok(out)
}

/// A stage run natively, with an inner virtual machine for its layout queries
/// and its plain function calls.
pub struct NativeExecutor {
    stage: NativeStage,
    // The natives the stage calls, by bridge slot, and the closures the driver
    // registered for them.
    natives: Vec<String>,
    closures: Vec<Option<StageNative>>,
    layout: Rc<RefCell<InnerVm>>,
    started: bool,
}

/// A virtual machine of a stage's module, kept for its layout queries and its
/// plain function calls, with the arena it borrows.
struct InnerVm {
    // Dropped before `arena`, which it borrows. See `Drop`.
    vm: ManuallyDrop<Vm<'static, 'static>>,
    arena: *mut Arena,
}

impl InnerVm {
    fn new(module: &Module) -> Result<Self, String> {
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
        match vm {
            Ok(vm) => Ok(Self {
                vm: ManuallyDrop::new(vm),
                arena,
            }),
            Err(e) => {
                // SAFETY: nothing borrows the arena, since no machine was built.
                drop(unsafe { Box::from_raw(arena) });
                Err(e)
            }
        }
    }
}

impl Drop for InnerVm {
    fn drop(&mut self) {
        // SAFETY: the machine is dropped first and never used again, and then
        // the arena it borrowed is freed exactly once.
        unsafe {
            ManuallyDrop::drop(&mut self.vm);
            drop(Box::from_raw(self.arena));
        }
    }
}

/// A stage compiled once, with the natives it calls in bridge slot order.
pub struct CompiledExecutor {
    code: Rc<CompiledStage>,
    natives: Vec<String>,
    // Shared by every run, since the driver uses it only for layout queries
    // and for the protocol constant functions, which read no state.
    layout: Rc<RefCell<InnerVm>>,
}

impl CompiledExecutor {
    /// Compile `module` with each native it calls bound to its trampoline, or
    /// say why it cannot be.
    pub fn new(module: &Module) -> Result<Self, String> {
        let called = called_natives(module)?;
        let mut bindings = Vec::with_capacity(called.len());
        for (slot, (name, arity)) in called.iter().enumerate() {
            let native = trampoline(slot, *arity).ok_or_else(|| {
                format!(
                    "`{name}`, native {slot} of arity {arity}, is past the bridge's {MAX_BRIDGED_NATIVES} \
                     slots of arity at most three"
                )
            })?;
            bindings.push((name.as_str(), native));
        }
        let code = CompiledStage::with_natives(module, &bindings).map_err(|e| e.to_string())?;
        let natives = called.into_iter().map(|(name, _)| name).collect();
        let layout = Rc::new(RefCell::new(InnerVm::new(module)?));
        Ok(Self {
            code,
            natives,
            layout,
        })
    }
}

impl NativeExecutor {
    /// Build `module` natively, or say why it cannot be.
    pub fn new(module: &Module) -> Result<Self, String> {
        Self::from_compiled(module, &CompiledExecutor::new(module)?)
    }

    /// A fresh run of `compiled`, which must have been compiled from `module`.
    /// Its layout queries go to the inner virtual machine `compiled` holds.
    pub fn from_compiled(_module: &Module, compiled: &CompiledExecutor) -> Result<Self, String> {
        let natives = compiled.natives.clone();
        let closures = natives.iter().map(|_| None).collect();
        Ok(Self {
            stage: compiled.code.instance(),
            natives,
            closures,
            layout: Rc::clone(&compiled.layout),
            started: false,
        })
    }

    fn turn(&mut self, shared: &mut [u8], reply: &Value) -> Result<VmState, VmError> {
        let Value::Int(reply) = reply else {
            return Err(VmError::TypeError(format!(
                "a native stage takes a word reply, not {reply:?}"
            )));
        };

        if let Some(i) = self.closures.iter().position(Option::is_none) {
            return Err(VmError::NativeError(format!(
                "the stage calls `{}`, which the driver did not register",
                self.natives[i]
            )));
        }
        // Lend the closures to the trampolines for this turn only.
        ACTIVE.with(|a| std::mem::swap(&mut *a.borrow_mut(), &mut self.closures));
        FAULT.with(|f| *f.borrow_mut() = None);
        let word = self.stage.turn_with_shared(shared, *reply);
        ACTIVE.with(|a| std::mem::swap(&mut *a.borrow_mut(), &mut self.closures));
        let word = word.map_err(|e| VmError::NativeError(e.to_string()))?;
        if let Some(fault) = FAULT.with(|f| f.borrow_mut().take()) {
            return Err(fault);
        }
        Ok(VmState::Yielded(Value::Int(word)))
    }
}

impl StageExecutor for NativeExecutor {
    fn shared_data_bytes(&self) -> usize {
        self.layout.borrow().vm.shared_data_bytes()
    }
    fn get_shared(&self, buf: &[u8], slot: usize) -> Result<Value, VmError> {
        self.layout.borrow().vm.get_shared(buf, slot)
    }
    fn set_shared(&self, buf: &mut [u8], slot: usize, value: Value) -> Result<(), VmError> {
        self.layout.borrow().vm.set_shared(buf, slot, value)
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
        self.layout.borrow_mut().vm.call_function(chunk, args)
    }
    fn register_native(&mut self, name: &str, native: StageNative) {
        // A native the stage does not call is not bound, as the virtual
        // machine accepts a registration it never calls.
        if let Some(slot) = self.natives.iter().position(|n| n == name) {
            self.closures[slot] = Some(native);
        }
    }
}

/// A stage module's serialized bytes and its compiled form, or `None` when it
/// was declined.
type CacheEntry = (Vec<u8>, Option<Rc<CompiledExecutor>>);

/// A factory that builds every stage it can natively and counts what it did.
///
/// Each distinct stage module is compiled once per factory and cached by its
/// serialized bytes, so the driver's repeated builds of one stage share its
/// code and each gets fresh buffers. A module the factory once declined is
/// declined again without another attempt.
#[derive(Default)]
pub struct NativeStageFactory {
    built: Cell<usize>,
    declined: Cell<usize>,
    compiled: Cell<usize>,
    cache: RefCell<Vec<CacheEntry>>,
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

    /// Distinct stage modules compiled so far, which the cache bounds.
    pub fn compiled(&self) -> usize {
        self.compiled.get()
    }

    fn compiled_for(&self, module: &Module) -> Option<Rc<CompiledExecutor>> {
        let key = module.to_bytes().ok()?;
        if let Some((_, hit)) = self.cache.borrow().iter().find(|(k, _)| *k == key) {
            return hit.clone();
        }
        let made = CompiledExecutor::new(module).ok().map(Rc::new);
        if made.is_some() {
            self.compiled.set(self.compiled.get() + 1);
        }
        self.cache.borrow_mut().push((key, made.clone()));
        made
    }
}

impl StageFactory for NativeStageFactory {
    fn build(&self, module: &Module) -> Option<Box<dyn StageExecutor>> {
        let executor = self
            .compiled_for(module)
            .ok_or_else(String::new)
            .and_then(|compiled| NativeExecutor::from_compiled(module, &compiled));
        match executor {
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
