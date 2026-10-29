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
//! - A stage that calls a host native is refused. `frame.kel` calls ten, and
//!   binding them to the driver's Rust closures through the JIT's symbol
//!   mapping is the remainder of step 2.
//! - Nothing in the driver calls this. The driver is written against the
//!   concrete `Vm` type, and the executor boundary that would let it call this
//!   is step 1, which V0.2.X was asked for.
//!
//! # The leaked context
//!
//! The JIT engine borrows its LLVM context for its whole life, so each stage
//! leaks one context to give that borrow a `'static` lifetime. The plan lowers
//! each stage once per process and caches it, which bounds the leak by the
//! fourteen stage programs. A caller that builds stages repeatedly leaks one
//! context per build.

use crate::{LowerError, LowerOptions, lower_module};
use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::execution_engine::{ExecutionEngine, JitFunction};
use keleusma::bytecode::{Module, Op};

const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;

type Entry = unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64;

/// A failure while running a native stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageError {
    /// A canary past the end of one of the three buffers was overwritten, so
    /// the stage wrote outside the extent its contract publishes. The stage
    /// must not be run again.
    Overrun(&'static str),
}

impl core::fmt::Display for StageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StageError::Overrun(buffer) => {
                write!(f, "the native stage wrote past its {buffer} buffer")
            }
        }
    }
}

impl std::error::Error for StageError {}

/// One stage program, lowered and compiled once, with the buffers its native
/// entry writes.
pub struct NativeStage {
    // Kept alive for `entry`, which points into the engine's code.
    _engine: ExecutionEngine<'static>,
    entry: JitFunction<'static, Entry>,
    shared: Vec<u8>,
    shared_bytes: usize,
    private: Vec<u64>,
    private_words: usize,
    region: Vec<u64>,
    region_words: usize,
    overrun: Option<StageError>,
}

impl NativeStage {
    /// Lower `module`, compile it, and prepare a zeroed shared segment with the
    /// private initializers installed.
    ///
    /// Refuses a module with no entry point, one whose entry is not the
    /// four-parameter stream entry the stage differential drives, one that
    /// calls a host native, and one whose private initializer this backend
    /// cannot place.
    pub fn new(module: &Module) -> Result<Self, LowerError> {
        let entry_index = module
            .entry_point
            .ok_or_else(|| LowerError::UnsupportedShape("a stage needs an entry point".into()))?;
        let calls_native = module
            .chunks
            .iter()
            .flat_map(|c| &c.ops)
            .any(|op| matches!(op, Op::CallVerifiedNative(..) | Op::CallExternalNative(..)));
        if calls_native {
            return Err(LowerError::UnsupportedShape(
                "the stage calls a host native, and binding host natives to a native stage \
                 is not implemented yet"
                    .into(),
            ));
        }
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
        let engine = llvm
            .create_jit_execution_engine(OptimizationLevel::Default)
            .map_err(|e| LowerError::Internal(format!("JIT engine: {e}")))?;
        // SAFETY: the signature was checked above against the four-parameter
        // entry `lower_module` emits for a stream stage.
        let entry = unsafe { engine.get_function::<Entry>(&symbol) }
            .map_err(|e| LowerError::Internal(format!("JIT symbol `{symbol}`: {e}")))?;

        let mut shared = vec![0u8; shared_bytes + 8];
        shared[shared_bytes..].copy_from_slice(&CANARY.to_le_bytes());
        let mut private = vec![0u64; private_words + 1];
        for (word, chunk) in private.iter_mut().zip(image.chunks(8)) {
            let mut bytes = [0u8; 8];
            bytes[..chunk.len()].copy_from_slice(chunk);
            *word = u64::from_ne_bytes(bytes);
        }
        private[private_words] = CANARY;
        let mut region = vec![0u64; region_words + 1];
        region[region_words] = CANARY;
        Ok(Self {
            _engine: engine,
            entry,
            shared,
            shared_bytes,
            private,
            private_words,
            region,
            region_words,
            overrun: None,
        })
    }

    /// The stage's shared segment, `shared_data_bytes_for` bytes long.
    pub fn shared(&self) -> &[u8] {
        &self.shared[..self.shared_bytes]
    }

    /// The stage's shared segment, for the host's writes between turns.
    pub fn shared_mut(&mut self) -> &mut [u8] {
        &mut self.shared[..self.shared_bytes]
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
            self.entry.call(
                reply,
                self.shared.as_mut_ptr(),
                self.private.as_mut_ptr().cast(),
                self.region.as_mut_ptr().cast(),
            )
        };
        let overrun = if self.shared[self.shared_bytes..] != CANARY.to_le_bytes() {
            Some("shared")
        } else if self.private[self.private_words] != CANARY {
            Some("private")
        } else if self.region[self.region_words] != CANARY {
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
