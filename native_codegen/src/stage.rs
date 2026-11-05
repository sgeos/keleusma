//! A self-hosted compiler stage run as native code, operator question 19.
//!
//! This is step 2 of `docs/decisions/NATIVE_SELF_HOSTED_COMPILER_PLAN.md`. The
//! driver in root `src/selfhost/` runs each stage on the virtual machine
//! through `call_with_shared` and `resume_with_shared`, and resumes again under
//! the same reply whenever the machine reports a `Reset`. A native stage takes
//! one call per turn instead. [`NativeStage::turn`] is that turn, delivering a
//! reply and returning the next yielded word, which is the rule the stage
//! differential's `lockstep` established against the virtual machine.
//!
//! The executor owns the stage's shared segment, as the driver's own command
//! machines do, so each of the three buffers the native entry writes carries a
//! canary this type checks after every turn.
//!
//! # What this does not do yet
//!
//! - Host natives are bound as C function pointers, [`HostNative`], through
//!   the JIT's symbol mapping. A caller with closures, as the driver has, binds
//!   trampolines, which `crate::stage_factory` does for `frame.kel`.
//! - A native is bound infallible, as `lower_module` lowers every native call
//!   outside a coroutine. A native that reports a soft failure on the virtual
//!   machine has no way to report it here.
//! - The driver reaches this through `crate::stage_factory`, which answers
//!   V0.2.X's `StageFactory` since absorption 118 and bridges the driver's
//!   closure natives to `HostNative` trampolines.
//!
//! # The leaked context
//!
//! The JIT engine borrows its LLVM context for its whole life, so each stage
//! leaks one context to give that borrow a `'static` lifetime. The plan lowers
//! each stage once per process and caches it, which bounds the leak by the
//! fourteen stage programs. A caller that builds stages repeatedly leaks one
//! context per build.

use crate::{LowerError, LowerOptions, host_native_declarations, lower_module, native_symbol};
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use keleusma::bytecode::Module;
use std::rc::Rc;

const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;

type Entry = unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64;

/// A host native for a native stage, a C function over words, by arity.
///
/// The arity is part of the type so that a binding whose parameter count
/// differs from the native's call sites is refused when the stage is built
/// rather than called through a mismatched signature.
#[derive(Clone, Copy, Debug)]
pub enum HostNative {
    /// No argument.
    Arity0(unsafe extern "C" fn() -> i64),
    /// One argument.
    Arity1(unsafe extern "C" fn(i64) -> i64),
    /// Two arguments.
    Arity2(unsafe extern "C" fn(i64, i64) -> i64),
    /// Three arguments.
    Arity3(unsafe extern "C" fn(i64, i64, i64) -> i64),
}

impl HostNative {
    fn arity(self) -> u32 {
        match self {
            HostNative::Arity0(_) => 0,
            HostNative::Arity1(_) => 1,
            HostNative::Arity2(_) => 2,
            HostNative::Arity3(_) => 3,
        }
    }

    fn address(self) -> usize {
        match self {
            HostNative::Arity0(f) => f as usize,
            HostNative::Arity1(f) => f as usize,
            HostNative::Arity2(f) => f as usize,
            HostNative::Arity3(f) => f as usize,
        }
    }
}

/// A failure while running a native stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageError {
    /// A canary past the end of one of the three buffers was overwritten, so
    /// the stage wrote outside the extent its contract publishes. The stage
    /// must not be run again.
    Overrun(&'static str),
    /// A caller's shared buffer is not the length the stage's contract
    /// publishes, so the stage was not called.
    SharedSize {
        /// The published length.
        expected: usize,
        /// The length supplied.
        actual: usize,
    },
}

impl core::fmt::Display for StageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StageError::Overrun(buffer) => {
                write!(f, "the native stage wrote past its {buffer} buffer")
            }
            StageError::SharedSize { expected, actual } => write!(
                f,
                "the shared buffer holds {actual} bytes and the stage publishes {expected}"
            ),
        }
    }
}

impl std::error::Error for StageError {}

/// One stage program, lowered and compiled once. Each [`NativeStage`] made
/// from it with [`CompiledStage::instance`] owns fresh buffers and shares the
/// code, so a caller that runs the same stage many times compiles it once.
pub struct CompiledStage {
    // Kept alive for `entry`, which points into the engine's code.
    _engine: ExecutionEngine<'static>,
    entry: JitFunction<'static, Entry>,
    shared_bytes: usize,
    private_words: usize,
    region_words: usize,
    image: Vec<u8>,
}

/// One run of a stage, with the buffers its native entry writes.
pub struct NativeStage {
    code: Rc<CompiledStage>,
    shared: Vec<u8>,
    private: Vec<u64>,
    region: Vec<u64>,
    overrun: Option<StageError>,
}

impl NativeStage {
    /// A stage that calls no host native. See [`Self::with_natives`].
    pub fn new(module: &Module) -> Result<Self, LowerError> {
        Self::with_natives(module, &[])
    }

    /// Compile `module` with [`CompiledStage::with_natives`] and make one run
    /// of it.
    pub fn with_natives(
        module: &Module,
        natives: &[(&str, HostNative)],
    ) -> Result<Self, LowerError> {
        Ok(CompiledStage::with_natives(module, natives)?.instance())
    }
}

impl CompiledStage {
    /// Lower `module`, bind each host native it calls to the function of the
    /// same name in `natives`, and compile it. Each run made from the result
    /// starts with a zeroed shared segment and the private initializers
    /// installed.
    ///
    /// Refuses a module with no entry point, one whose entry is not the
    /// four-parameter stream entry the stage differential drives, and one whose
    /// private initializer this backend cannot place. Refuses a native the
    /// module calls with no binding, a binding whose arity differs from the
    /// call sites, a native returning a float, and a binding for a native the
    /// module does not call, which is most often a misspelt name.
    pub fn with_natives(
        module: &Module,
        natives: &[(&str, HostNative)],
    ) -> Result<Rc<Self>, LowerError> {
        let entry_index = module
            .entry_point
            .ok_or_else(|| LowerError::UnsupportedShape("a stage needs an entry point".into()))?;
        let image = match (
            &module.data_layout,
            crate::region::private_init_image(module),
        ) {
            (Some(_), None) => {
                return Err(LowerError::UnsupportedShape(
                    "the stage's private initializer cannot be placed by this backend".into(),
                ));
            }
            (_, image) => image.unwrap_or_default(),
        };

        let shared_bytes = keleusma::vm::shared_data_bytes_for(module);
        // The private extent is the published contract, the slot array plus the
        // persistent supplement, as the stage differential sizes it.
        let private_words = (keleusma::vm::required_persistent_capacity_for(module)
            + crate::region::persistent_supplement_bytes(module) as usize)
            .div_ceil(8);
        let region_words =
            (crate::region::region_total_bytes(module, entry_index, 0) as usize).div_ceil(8);

        let context: &'static Context = Box::leak(Box::new(Context::create()));
        let llvm = context.create_module("kel_stage");
        lower_module(context, &llvm, module, LowerOptions::default())?;
        llvm.verify()
            .map_err(|e| LowerError::Internal(format!("LLVM verification: {e}")))?;
        let symbol = format!("kel_chunk_{entry_index}");
        let function = llvm
            .get_function(&symbol)
            .ok_or_else(|| LowerError::Internal(format!("no entry function `{symbol}`")))?;
        // A wrong signature is undefined behaviour inside JIT code, so the
        // parameter count is checked before any call is made through it.
        if function.count_params() != 4 {
            return Err(LowerError::UnsupportedShape(format!(
                "entry `{symbol}` takes {} parameters, and a stage is driven with a reply \
                 and the three buffer pointers",
                function.count_params()
            )));
        }
        let bindings = bind(&llvm, natives)?;
        let engine = llvm
            .create_jit_execution_engine(OptimizationLevel::Default)
            .map_err(|e| LowerError::Internal(format!("JIT engine: {e}")))?;
        for (symbol, native) in &bindings {
            let declaration = llvm
                .get_function(symbol)
                .ok_or_else(|| LowerError::Internal(format!("no declaration `{symbol}`")))?;
            engine.add_global_mapping(&declaration, native.address());
        }
        // SAFETY: the signature was checked above against the four-parameter
        // entry `lower_module` emits for a stream stage.
        let entry = unsafe { engine.get_function::<Entry>(&symbol) }
            .map_err(|e| LowerError::Internal(format!("JIT symbol `{symbol}`: {e}")))?;

        Ok(Rc::new(Self {
            _engine: engine,
            entry,
            shared_bytes,
            private_words,
            region_words,
            image,
        }))
    }

    /// A fresh run of this stage, with a zeroed shared segment and region and
    /// the private initializers installed, each buffer followed by a canary.
    pub fn instance(self: &Rc<Self>) -> NativeStage {
        let mut shared = vec![0u8; self.shared_bytes + 8];
        shared[self.shared_bytes..].copy_from_slice(&CANARY.to_le_bytes());
        let mut private = vec![0u64; self.private_words + 1];
        for (word, chunk) in private.iter_mut().zip(self.image.chunks(8)) {
            let mut bytes = [0u8; 8];
            bytes[..chunk.len()].copy_from_slice(chunk);
            *word = u64::from_ne_bytes(bytes);
        }
        private[self.private_words] = CANARY;
        let mut region = vec![0u64; self.region_words + 1];
        region[self.region_words] = CANARY;
        NativeStage {
            code: Rc::clone(self),
            shared,
            private,
            region,
            overrun: None,
        }
    }
}

impl NativeStage {
    /// The stage's shared segment, `shared_data_bytes_for` bytes long.
    pub fn shared(&self) -> &[u8] {
        &self.shared[..self.code.shared_bytes]
    }

    /// The stage's shared segment, for the host's writes between turns.
    pub fn shared_mut(&mut self) -> &mut [u8] {
        &mut self.shared[..self.code.shared_bytes]
    }

    /// Deliver `reply` and return the word the stage yields next.
    ///
    /// A stage that has once overrun a buffer is refused without being called
    /// again.
    pub fn turn(&mut self, reply: i64) -> Result<i64, StageError> {
        if let Some(e) = &self.overrun {
            return Err(e.clone());
        }
        // SAFETY: each pointer addresses a buffer at least as long as the
        // extent the stage's contract publishes, plus a canary word.
        let word = unsafe {
            self.code.entry.call(
                reply,
                self.shared.as_mut_ptr(),
                self.private.as_mut_ptr().cast(),
                self.region.as_mut_ptr().cast(),
            )
        };
        let overrun = if self.shared[self.code.shared_bytes..] != CANARY.to_le_bytes() {
            Some("shared")
        } else if self.private[self.code.private_words] != CANARY {
            Some("private")
        } else if self.region[self.code.region_words] != CANARY {
            Some("region")
        } else {
            None
        };
        if let Some(buffer) = overrun {
            let e = StageError::Overrun(buffer);
            self.overrun = Some(e.clone());
            return Err(e);
        }
        Ok(word)
    }

    /// [`Self::turn`] over a shared segment the caller owns, which must be
    /// exactly the published length. The stage's own shared buffer is not used.
    ///
    /// This avoids copying the caller's buffer in and out on every turn, which
    /// measured 62,542 MB over 118,014 turns for two compiles of one program on
    /// 2026-10-30, against 81 ms of native execution. The cost is that a write
    /// past the shared segment has no canary to land on. The private and region
    /// buffers keep theirs.
    pub fn turn_with_shared(&mut self, shared: &mut [u8], reply: i64) -> Result<i64, StageError> {
        if let Some(e) = &self.overrun {
            return Err(e.clone());
        }
        if shared.len() != self.code.shared_bytes {
            return Err(StageError::SharedSize {
                expected: self.code.shared_bytes,
                actual: shared.len(),
            });
        }
        // SAFETY: `shared` is exactly the published extent, which the length
        // check above establishes, and the other two pointers address buffers
        // at least as long as their published extents, plus a canary word.
        let word = unsafe {
            self.code.entry.call(
                reply,
                shared.as_mut_ptr(),
                self.private.as_mut_ptr().cast(),
                self.region.as_mut_ptr().cast(),
            )
        };
        let overrun = if self.private[self.code.private_words] != CANARY {
            Some("private")
        } else if self.region[self.code.region_words] != CANARY {
            Some("region")
        } else {
            None
        };
        if let Some(buffer) = overrun {
            let e = StageError::Overrun(buffer);
            self.overrun = Some(e.clone());
            return Err(e);
        }
        Ok(word)
    }
}

/// Match each native `llvm` declares with its binding in `natives`, by the
/// backend's own symbol spelling, and check the arity and return kind.
fn bind(
    llvm: &inkwell::module::Module<'_>,
    natives: &[(&str, HostNative)],
) -> Result<Vec<(String, HostNative)>, LowerError> {
    let mut supplied: Vec<(String, &str, HostNative)> = natives
        .iter()
        .map(|(name, native)| (native_symbol(name), *name, *native))
        .collect();
    let mut bound = Vec::new();
    for (symbol, arity, returns_float) in host_native_declarations(llvm) {
        let at = supplied
            .iter()
            .position(|(s, _, _)| *s == symbol)
            .ok_or_else(|| {
                LowerError::UnsupportedShape(format!(
                    "the stage calls the host native `{symbol}`, which is not bound"
                ))
            })?;
        let (_, name, native) = supplied.swap_remove(at);
        if returns_float {
            return Err(LowerError::UnsupportedShape(format!(
                "the host native `{name}` returns a float, which a word binding cannot"
            )));
        }
        if native.arity() != arity {
            return Err(LowerError::UnsupportedShape(format!(
                "the host native `{name}` is called with {arity} arguments and bound with {}",
                native.arity()
            )));
        }
        bound.push((symbol, native));
    }
    if let Some((_, name, _)) = supplied.first() {
        return Err(LowerError::UnsupportedShape(format!(
            "a binding was supplied for `{name}`, which the stage does not call"
        )));
    }
    Ok(bound)
}
