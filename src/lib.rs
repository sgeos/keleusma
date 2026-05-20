#![no_std]
extern crate alloc;

// Always-on runtime modules. These compile under any feature
// combination and form the minimum surface needed to load and
// run precompiled bytecode.
pub mod address;
pub mod bytecode;
pub mod kstring;
pub mod marshall;
pub mod opaque;
pub mod utility_natives;
pub mod vm;
pub mod word;

// Audio natives use floating-point arithmetic throughout (note
// frequency, phase, filter coefficients) and are only useful on
// hosts that enable the `floats` feature. Without floats the
// bundle's native signatures cannot satisfy the `IntoNativeFn`
// trait bounds because `KeleusmaType for f64` is not in scope.
#[cfg(feature = "floats")]
pub mod audio_natives;

// Parametric floating-point trait for sub-64-bit native runtimes
// (B16). The trait and its `f32` / `f64` impls are always
// compiled so the generic `Vm<W, A, F>` shape carries a Float
// type parameter regardless of the `floats` feature; the
// floating-point variants of `Value` and the floating-point
// opcodes remain gated by `floats` per their existing design.
pub mod float;

// The stddsl bundle exposes Math and other DSL helpers that pin
// f64 parameters and returns. Gated alongside `floats` for the
// same reason as `audio_natives`.
#[cfg(feature = "floats")]
pub mod stddsl;

// Compile-pipeline modules. Gated behind the `compile` feature
// (default on). With the feature off, the runtime accepts only
// precompiled bytecode through `Module::from_bytes` and
// `Vm::view_bytes_zero_copy`. Hosts that ship precompiled
// bytecode for the smallest possible runtime binary leave this
// feature off.
#[cfg(feature = "compile")]
pub mod ast;
#[cfg(feature = "compile")]
pub mod compiler;
#[cfg(feature = "compile")]
pub mod interval;
#[cfg(feature = "compile")]
pub mod lexer;
#[cfg(feature = "compile")]
pub mod monomorphize;
#[cfg(feature = "compile")]
pub mod parser;
#[cfg(feature = "compile")]
pub mod target;
#[cfg(feature = "compile")]
pub mod token;
#[cfg(feature = "compile")]
pub mod typecheck;
#[cfg(feature = "compile")]
pub mod visitor;

// Verifier modules. Gated behind the `verify` feature (default
// on). With the feature off, `Vm::new` skips structural and
// resource-bound verification and behaves like
// `Vm::new_unchecked` from the caller's perspective. The
// compiler's call to the verifier at the end of
// `compile_with_target` is likewise gated; with the feature off
// the compiler leaves the bytecode header's WCET and WCMU
// fields at 0 (auto).
#[cfg(feature = "verify")]
pub mod text_size;
#[cfg(feature = "verify")]
pub mod verify;

pub use keleusma_arena::{
    Arena, ArenaHandle, BottomHandle, Budget, EpochSaturated, Stale, TopHandle,
};
pub use kstring::KString;

pub use address::Address;
pub use bytecode::{
    CostModel, GenericValue, Module, NOMINAL_COST_MODEL, OpCost, OpCostContext,
    VALUE_SLOT_SIZE_BYTES, Value, nominal_op_cycles,
};
pub use float::Float;
pub use keleusma_macros::KeleusmaType;
pub use marshall::{IntoFallibleNativeFn, IntoNativeFn, KeleusmaType};
pub use opaque::{HostOpaque, host_arc};
#[cfg(feature = "verify")]
pub use text_size::{TextSize, op_cost_context};
pub use vm::{NativeCtx, OverflowPolicy, VerifyWarning, VmError, VmOptions, WarningKind};
pub use word::Word;
