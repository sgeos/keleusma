#![no_std]
#![deny(missing_docs)]
//! Keleusma is a Total Functional Stream Processor that compiles
//! to bytecode and runs on a stack-based virtual machine. The crate
//! targets `no_std + alloc` environments and is designed for embedded
//! scripting where definitive worst-case execution time and worst-case
//! memory usage bounds matter.
//!
//! The crate has no built-in standard library. All domain functionality
//! is provided by host-registered native functions through
//! [`vm::Vm::register_fn`], [`vm::Vm::register_native`], and the
//! related entry points. The bundled libraries in [`stddsl`] are
//! examples of the registration pattern, not a closed set.
//!
//! # Quick start
//!
//! ```ignore
//! // Requires the `compile` cargo feature (default on).
//! use keleusma::compiler::compile;
//! use keleusma::lexer::tokenize;
//! use keleusma::parser::parse;
//! use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
//! use keleusma::{Arena, Value};
//!
//! let source = r#"
//!     fn double(x: Word) -> Word { x * 2 }
//!     fn main(n: Word) -> Word { n |> double() }
//! "#;
//!
//! let tokens = tokenize(source).expect("lex");
//! let program = parse(&tokens).expect("parse");
//! let module = compile(&program).expect("compile");
//! let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
//! let mut vm = Vm::new(module, &arena).expect("verify");
//!
//! match vm.call(&[Value::Int(21)]).unwrap() {
//!     VmState::Finished(value) => println!("{:?}", value),
//!     _ => unreachable!(),
//! }
//! // prints: Int(42)
//! ```
//!
//! # Cargo features
//!
//! The crate exposes orthogonal feature gates so hosts can strip
//! pipeline stages they do not need.
//!
//! - `compile` (default on): lexer, parser, type checker,
//!   monomorphizer, compiler. Drop when the host ships precompiled
//!   bytecode.
//! - `verify` (default on): structural verifier and WCET/WCMU
//!   resource-bounds pass. With this off, [`vm::Vm::new`] behaves
//!   like [`vm::Vm::new_unchecked`].
//! - `floats` (default on): the `Float` surface type, `Value::Float`,
//!   `Op::IntToFloat`/`FloatToInt`, and the [`audio_natives`] and
//!   [`stddsl::Math`]/[`stddsl::Audio`] bundles. Drop to eliminate the
//!   soft-float `compiler_builtins` routines from the runtime image.
//! - `signatures` (default off): Ed25519 module signing and
//!   load-time verification.
//! - `shell` (default off): the [`stddsl::Shell`] bundle. Requires
//!   `std`.
//! - `sdl3-example` (default off): builds the bundled SDL3 audio
//!   piano-roll example. `cmake`-builds SDL3 from source.
//!
//! Seven mutually-exclusive `narrow-word-*`, `narrow-address-*`, and
//! `narrow-float-32` parametric-runtime selectors gate the
//! framing-level upper bound on bytecode widths for hosts that ship
//! only a sub-64-bit `GenericVm` instance.
//!
//! # Further reading
//!
//! The repository's `README.md` and `docs/` knowledge graph describe
//! the language design, execution model, instruction set, wire
//! format, and conservative-verification stance in depth.

extern crate alloc;

/// Address-type abstraction used by the parametric [`vm::GenericVm`]
/// to model the runtime's address width.
pub mod address;
/// Runtime values, instructions, the [`Module`] type, and the cost
/// model.
pub mod bytecode;
/// Strippable debug metadata: the chunk-local [`debug_meta::DebugPool`]
/// section and its canonical byte encoding. Parallel infrastructure for
/// B29; the foundational data model and serialization, not yet attached
/// to [`bytecode::Chunk`] nor emitted by the compiler.
pub mod debug_meta;
/// Authenticated encryption of compiled [`Module`]s under the
/// optional `encryption` feature. Implements the V0.2.1 hybrid
/// asymmetric key wrapping (X25519 ECDH plus HKDF-SHA-256 plus
/// AES-256-GCM). Feature-gated because the crypto stack adds
/// meaningful binary footprint for hosts that do not need it.
#[cfg(feature = "encryption")]
pub mod encryption;
/// Flat-byte composite representation helpers and the
/// [`flat_value::FlatComposite`] container. Parallel
/// infrastructure for B28's runtime composite-value
/// representation refactor. Not yet consumed by the runtime.
pub mod flat_value;
/// Arena-resident dynamic strings ([`KString`]) at the host-VM
/// boundary.
pub mod kstring;
/// Type-driven marshalling between host Rust types and runtime
/// [`Value`]s for native function registration.
pub mod marshall;
/// Opaque host-value support: the [`opaque::HostOpaque`] trait and
/// the [`opaque::host_arc`] constructor that produces
/// `Value::Opaque(Arc<dyn HostOpaque>)`.
pub mod opaque;
/// Bundled utility natives. V0.2.0 ships only `println` here; other
/// utilities are host-registered.
pub mod utility_natives;
/// Layout descriptors for composite Keleusma values. Parallel
/// infrastructure for B28's runtime composite-value
/// representation refactor. Not yet consumed by the runtime.
pub mod value_layout;
/// The stack-based virtual machine and its coroutine driver.
pub mod vm;
/// Wire-format encoding and decoding of compiled [`Module`]s.
pub mod wire_format;
/// Word-type abstraction used by the parametric [`vm::GenericVm`]
/// to model the runtime's word width.
pub mod word;

// Audio natives use floating-point arithmetic throughout (note
// frequency, phase, filter coefficients) and are only useful on
// hosts that enable the `floats` feature. Without floats the
// bundle's native signatures cannot satisfy the `IntoNativeFn`
// trait bounds because `KeleusmaType for f64` is not in scope.
/// Bundled audio-DSP natives (`audio::midi_to_freq`,
/// `audio::db_to_linear`, and similar). Requires the `floats` feature.
#[cfg(feature = "floats")]
pub mod audio_natives;

/// Parametric floating-point trait used by [`vm::GenericVm`]. The
/// trait and its `f32` and `f64` impls are always compiled so the
/// generic shape carries a `Float` type parameter regardless of the
/// `floats` feature; the floating-point variants of [`Value`] and the
/// floating-point opcodes remain gated by `floats`.
pub mod float;

/// Bundled standard-library DSLs ([`stddsl::Math`], [`stddsl::Audio`],
/// [`stddsl::Shell`]) registered through [`vm::Vm::register_library`].
#[cfg(feature = "floats")]
pub mod stddsl;

// Compile-pipeline modules. Gated behind the `compile` feature
// (default on). With the feature off, the runtime accepts only
// precompiled bytecode through `Module::from_bytes` and
// `Vm::view_bytes_zero_copy`. Hosts that ship precompiled
// bytecode for the smallest possible runtime binary leave this
// feature off.
/// Abstract syntax tree node definitions.
#[cfg(feature = "compile")]
pub mod ast;
/// AST to bytecode emission.
#[cfg(feature = "compile")]
pub mod compiler;
/// Closed-signed-interval lattice over `i64` used by the type
/// checker and the refinement-elision pass.
#[cfg(feature = "compile")]
pub mod interval;
/// Compile-time layout pass. Bridges AST type expressions to
/// the [`value_layout::LayoutDescriptor`] byte-layout
/// descriptors used by subsequent B28 phases for composite
/// allocation and field access.
#[cfg(feature = "compile")]
pub mod layout_pass;
/// Source-text tokenisation (lexer).
#[cfg(feature = "compile")]
pub mod lexer;
/// Compile-time monomorphization of generic functions, structs, and
/// enums.
#[cfg(feature = "compile")]
pub mod monomorphize;
/// Token-to-AST recursive-descent parser.
#[cfg(feature = "compile")]
pub mod parser;
/// Target descriptor (word/address/float widths, endianness) used by
/// [`compiler::compile_with_target`] for cross-architecture
/// portability.
#[cfg(feature = "compile")]
pub mod target;
/// Token definitions and keyword recognition.
#[cfg(feature = "compile")]
pub mod token;
/// Hindley-Milner type checker with generics, traits, and bounds.
#[cfg(feature = "compile")]
pub mod typecheck;
/// Visitor and mutable-visitor traits with default walk methods over
/// the AST.
#[cfg(feature = "compile")]
pub mod visitor;
/// Canonical zero value per type and lowest-valid resolution for
/// refined newtypes. Parallel infrastructure for B35's Partial
/// Operation Handling; native code generation is the intended
/// consumer. Not yet consumed by the runtime.
#[cfg(feature = "compile")]
pub mod zero_value;

// Verifier modules. Gated behind the `verify` feature (default
// on). With the feature off, `Vm::new` skips structural and
// resource-bound verification and behaves like
// `Vm::new_unchecked` from the caller's perspective. The
// compiler's call to the verifier at the end of
// `compile_with_target` is likewise gated; with the feature off
// the compiler leaves the bytecode header's WCET and WCMU
// fields at 0 (auto).
/// Abstract-interpretation text-size lattice used by the WCMU pass
/// for dynamic-text allocations.
#[cfg(feature = "verify")]
pub mod text_size;
/// Structural verifier plus WCET and WCMU resource-bounds pass.
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
pub use keleusma_macros::{KeleusmaError, KeleusmaType};
pub use marshall::{IntoFallibleNativeFn, IntoNativeFn, KeleusmaType};
pub use opaque::{HostOpaque, host_arc};
#[cfg(feature = "verify")]
pub use text_size::{TextSize, op_cost_context};
pub use vm::{NativeCtx, OverflowPolicy, VerifyWarning, VmError, VmOptions, WarningKind};
pub use word::Word;
