//! Shared kernel-construction code.
//!
//! The binaries call into [`three_task_kernel`] (or the
//! arena-sized variant) to get a fully-populated [`Kernel<P>`]
//! holding the LED, sensor, and heartbeat tasks. Each binary's
//! main function then runs the kernel through its platform's
//! executor (the tiny block-on for std, the embassy executor on
//! the N6).
//!
//! Two compilation modes coexist behind the `keleusma-compile`
//! feature.
//!
//! - **Source mode** (`keleusma-compile` on, default).
//!   The runtime carries the full compile pipeline. Task
//!   scripts are bundled as `include_str!` constants and
//!   tokenised, parsed, and compiled at boot inside
//!   [`build_module`]. The shared `prelude.kel` is prepended at
//!   compile time so every task sees the same `Status` and
//!   `StatusErrorCode` enum declarations.
//!
//! - **Precompiled-bytecode mode** (`keleusma-compile` off).
//!   The `build.rs` script invokes the parent crate's compile
//!   pipeline at host build time and emits one
//!   `OUT_DIR/<name>.kel.bin` per script. The runtime carries
//!   only the VM; task modules load through `Module::from_bytes`
//!   on the embedded `include_bytes!` constants.
//!
//! Verification of the loaded module is controlled by the
//! orthogonal `keleusma-verify` feature. `Vm::new` is always
//! called; the keleusma crate's `verify` feature decides
//! whether verification actually runs inside `Vm::new`. With
//! the feature off, `Vm::new` degrades silently to a trust-load,
//! shifting the bounded-memory contract onto whatever build-time
//! verification the artefact ingestion process performed.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use keleusma::bytecode::Value;
#[cfg(feature = "keleusma-compile")]
use keleusma::compiler::compile;
#[cfg(feature = "keleusma-compile")]
use keleusma::lexer::tokenize;
#[cfg(feature = "keleusma-compile")]
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};
use keleusma::{Arena, Module};

use crate::kernel::{Kernel, Task, TaskState, WakeReason};
use crate::natives::register_task_natives;
use crate::platform::Platform;

// --- Source-mode constants. Present only when the compile
// pipeline is in the runtime image. ---

/// Prelude prepended to every task source. Defines the
/// `Status` and `StatusErrorCode` enums shared with the host's
/// native surface. Kept here as a single source of truth so all
/// tasks see identical declarations.
#[cfg(feature = "keleusma-compile")]
const PRELUDE: &str = include_str!("../scripts/prelude.kel");

/// LED blinker task source.
#[cfg(feature = "keleusma-compile")]
pub const SRC_LED: &str = include_str!("../scripts/led.kel");
/// Sensor poller task source.
#[cfg(feature = "keleusma-compile")]
pub const SRC_SENSOR: &str = include_str!("../scripts/sensor.kel");
/// Heartbeat task source.
#[cfg(feature = "keleusma-compile")]
pub const SRC_HEARTBEAT: &str = include_str!("../scripts/heartbeat.kel");

// --- Precompiled-bytecode constants. Present only when the
// compile pipeline is absent and `build.rs` has produced
// bytecode artefacts in OUT_DIR. ---

/// LED blinker task bytecode.
#[cfg(not(feature = "keleusma-compile"))]
pub const BIN_LED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/led.kel.bin"));
/// Sensor poller task bytecode.
#[cfg(not(feature = "keleusma-compile"))]
pub const BIN_SENSOR: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/sensor.kel.bin"));
/// Heartbeat task bytecode.
#[cfg(not(feature = "keleusma-compile"))]
pub const BIN_HEARTBEAT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/heartbeat.kel.bin"));

/// Construct the three-task demonstrator kernel for the given
/// platform. Returns a kernel with three cooperative tasks
/// loaded and ready to run. Uses
/// [`keleusma::vm::DEFAULT_ARENA_CAPACITY`] for each task; pass
/// a smaller value to [`three_task_kernel_with_arena_capacity`]
/// for embedded targets where heap budget is tight.
///
/// The function leaks one arena per task; the kernel holds
/// each VM through a `'static` reference. The leak is a
/// one-time bounded allocation; the long-run behaviour is
/// constant in memory.
pub fn three_task_kernel<P: Platform>() -> Result<Kernel<P>, String> {
    three_task_kernel_with_arena_capacity::<P>(DEFAULT_ARENA_CAPACITY)
}

/// Same as [`three_task_kernel`] but with an explicit per-task
/// arena capacity. Embedded targets use this to fit three tasks
/// within a constrained global heap; the std demonstrator
/// continues through the no-argument wrapper above. See
/// [`build_task_with_arena_capacity`] for the per-task path.
pub fn three_task_kernel_with_arena_capacity<P: Platform>(
    arena_capacity: usize,
) -> Result<Kernel<P>, String> {
    let mut kernel = Kernel::<P>::new();
    #[cfg(feature = "keleusma-compile")]
    {
        kernel.add_task(build_task_with_arena_capacity::<P>(
            "led",
            SRC_LED,
            arena_capacity,
        )?);
        kernel.add_task(build_task_with_arena_capacity::<P>(
            "sensor",
            SRC_SENSOR,
            arena_capacity,
        )?);
        kernel.add_task(build_task_with_arena_capacity::<P>(
            "heartbeat",
            SRC_HEARTBEAT,
            arena_capacity,
        )?);
    }
    #[cfg(not(feature = "keleusma-compile"))]
    {
        kernel.add_task(build_task_with_arena_capacity::<P>(
            "led",
            BIN_LED,
            arena_capacity,
        )?);
        kernel.add_task(build_task_with_arena_capacity::<P>(
            "sensor",
            BIN_SENSOR,
            arena_capacity,
        )?);
        kernel.add_task(build_task_with_arena_capacity::<P>(
            "heartbeat",
            BIN_HEARTBEAT,
            arena_capacity,
        )?);
    }
    Ok(kernel)
}

/// Compile or load a single task and wrap it as a kernel
/// [`Task`]. Convenience wrapper over
/// [`build_task_with_arena_capacity`] using
/// [`keleusma::vm::DEFAULT_ARENA_CAPACITY`].
///
/// The second parameter is either source text or precompiled
/// bytecode, depending on whether the `keleusma-compile`
/// feature is enabled.
#[cfg(feature = "keleusma-compile")]
pub fn build_task<P: Platform>(name: &'static str, src: &str) -> Result<Task, String> {
    build_task_with_arena_capacity::<P>(name, src, DEFAULT_ARENA_CAPACITY)
}

/// Precompiled-bytecode variant of [`build_task`]. See the
/// module-level documentation for the two compilation modes.
#[cfg(not(feature = "keleusma-compile"))]
pub fn build_task<P: Platform>(name: &'static str, bytes: &[u8]) -> Result<Task, String> {
    build_task_with_arena_capacity::<P>(name, bytes, DEFAULT_ARENA_CAPACITY)
}

/// Compile or load a single task and wrap it as a kernel
/// [`Task`] with an explicit arena capacity.
///
/// In source mode (`keleusma-compile` on) the second parameter
/// is the task source; the function prepends the shared
/// prelude and runs the parent's compile pipeline. In
/// precompiled-bytecode mode (`keleusma-compile` off) the
/// second parameter is the rkyv-archived bytecode produced by
/// `build.rs`; the function deserialises through
/// `Module::from_bytes`. Both modes converge on the same
/// downstream steps (arena leak, `Vm::new`, slot
/// initialisation, native registration).
#[cfg(feature = "keleusma-compile")]
pub fn build_task_with_arena_capacity<P: Platform>(
    name: &'static str,
    src: &str,
    arena_capacity: usize,
) -> Result<Task, String> {
    let module = build_module(src).map_err(|e| format!("build_module for {}: {}", name, e))?;
    finish_build_task::<P>(name, module, arena_capacity)
}

#[cfg(not(feature = "keleusma-compile"))]
pub fn build_task_with_arena_capacity<P: Platform>(
    name: &'static str,
    bytes: &[u8],
    arena_capacity: usize,
) -> Result<Task, String> {
    let module = Module::from_bytes(bytes)
        .map_err(|e| format!("load_module for {}: {:?}", name, e))?;
    finish_build_task::<P>(name, module, arena_capacity)
}

/// Shared tail of [`build_task_with_arena_capacity`]. Leaks an
/// arena of the requested capacity, constructs the VM, zeros
/// the data segment, and registers the utility and task native
/// surfaces. Called from both the source and precompiled
/// variants.
///
/// `Vm::new` is called unconditionally. The keleusma crate's
/// `verify` feature, surfaced through this crate's
/// `keleusma-verify`, decides whether structural and
/// resource-bound verification runs inside `Vm::new`.
fn finish_build_task<P: Platform>(
    name: &'static str,
    module: Module,
    arena_capacity: usize,
) -> Result<Task, String> {
    let arena: &'static Arena = Box::leak(Box::new(Arena::with_capacity(arena_capacity)));
    let mut vm: Vm<'static, 'static> =
        Vm::new(module, arena).map_err(|e| format!("vm new for {}: {:?}", name, e))?;
    for slot in 0..vm.data_len() {
        let _ = vm.set_data(slot, Value::Int(0));
    }
    // The utility natives that back f-string interpolation
    // (`to_string`, `concat`, `slice`, `length`, `println`) are
    // no longer registered. Task scripts compile without the
    // `text` surface feature and emit diagnostics through
    // `host::log_event(code, data)` instead of constructing
    // arena-resident strings, so the helpers would be dead
    // weight in the runtime image.
    register_task_natives::<P>(&mut vm);
    Ok(Task {
        name,
        vm,
        state: TaskState::Ready(WakeReason::FirstRun),
        started: false,
    })
}

/// Source-mode helper. Prepend the prelude to the task source
/// and run the parent's compile pipeline. Available only when
/// the `keleusma-compile` feature is enabled.
///
/// Line numbers in compile errors for task scripts are offset
/// by the prelude's line count; the prelude itself is small
/// enough that this is acceptable for the scaffold.
#[cfg(feature = "keleusma-compile")]
fn build_module(src: &str) -> Result<Module, String> {
    let combined = format!("{}\n{}", PRELUDE, src);
    let tokens = tokenize(&combined).map_err(|e| format!("lex error: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("parse error: {:?}", e))?;
    compile(&program).map_err(|e| format!("compile error: {:?}", e))
}
