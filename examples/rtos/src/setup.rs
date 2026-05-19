//! Shared kernel-construction code.
//!
//! The two binaries (the std demonstrator and any future
//! embedded entry point) call into [`three_task_kernel`] to
//! get a fully-populated [`Kernel<P>`] holding the LED,
//! sensor, and heartbeat tasks. Each binary's main function
//! then runs the kernel through its platform's executor (the
//! tiny block-on for std, the embassy executor on the N6).
//!
//! The function is generic over the platform so the same
//! task construction code works for any backend.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;

use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::utility_natives::register_utility_natives;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};
use keleusma::{Arena, Module};

use crate::kernel::{Kernel, Task, TaskState, WakeReason};
use crate::natives::register_task_natives;
use crate::platform::Platform;

/// Prelude prepended to every task source. Defines the
/// `Status` and `StatusErrorCode` enums shared with the host's
/// native surface. Kept here as a single source of truth so all
/// tasks see identical declarations.
const PRELUDE: &str = include_str!("../scripts/prelude.kel");

/// LED blinker task source. Exposed so binaries can construct
/// kernels with custom logging interleaved between task builds.
pub const SRC_LED: &str = include_str!("../scripts/led.kel");
/// Sensor poller task source.
pub const SRC_SENSOR: &str = include_str!("../scripts/sensor.kel");
/// Heartbeat task source.
pub const SRC_HEARTBEAT: &str = include_str!("../scripts/heartbeat.kel");

/// Construct the three-task demonstrator kernel for the given
/// platform. Returns a kernel with three cooperative tasks
/// loaded and ready to run.
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
    Ok(kernel)
}

/// Compile a single task script and wrap it as a kernel
/// [`Task`]. Leaks one arena with [`DEFAULT_ARENA_CAPACITY`]
/// bytes and registers the platform's native function surface
/// against the resulting VM. Convenience wrapper over
/// [`build_task_with_arena_capacity`] for the std demonstrator
/// where heap is abundant.
pub fn build_task<P: Platform>(name: &'static str, src: &str) -> Result<Task, String> {
    build_task_with_arena_capacity::<P>(name, src, DEFAULT_ARENA_CAPACITY)
}

/// Compile a single task script and wrap it as a kernel
/// [`Task`] with an explicit arena capacity.
///
/// Embedded targets pass a smaller capacity than
/// [`DEFAULT_ARENA_CAPACITY`] (64 KB) to keep the global heap
/// footprint within the RAM budget declared in `memory.x`. The
/// demonstrator scripts are short enough that 16 KB per arena
/// is sufficient for their operand-stack and call-frame
/// allocations. Hosts that script-load arbitrary tasks can size
/// arenas using [`keleusma::vm::auto_arena_capacity_for`] once
/// the module has been built.
///
/// The function leaks one arena. The leak is a one-time
/// bounded allocation; long-run behaviour is constant in
/// memory.
pub fn build_task_with_arena_capacity<P: Platform>(
    name: &'static str,
    src: &str,
    arena_capacity: usize,
) -> Result<Task, String> {
    let module = build_module(src).map_err(|e| format!("build_module for {}: {}", name, e))?;
    let arena: &'static Arena = Box::leak(Box::new(Arena::with_capacity(arena_capacity)));
    let mut vm: Vm<'static, 'static> =
        Vm::new(module, arena).map_err(|e| format!("vm new for {}: {:?}", name, e))?;
    for slot in 0..vm.data_len() {
        let _ = vm.set_data(slot, Value::Int(0));
    }
    // The utility natives provide `to_string`, `concat`, and the
    // other helpers that f-string interpolation desugars to.
    // Without them, scripts that use f-strings fail at compile
    // time with `undefined function "concat"`.
    register_utility_natives(&mut vm);
    register_task_natives::<P>(&mut vm);
    Ok(Task {
        name,
        vm,
        state: TaskState::Ready(WakeReason::FirstRun),
        started: false,
    })
}

fn build_module(src: &str) -> Result<Module, String> {
    // Prepend the prelude so every task sees the shared
    // `Status` and `StatusErrorCode` enum declarations. Line
    // numbers in compile errors for task scripts are offset by
    // the prelude's line count; the prelude itself is small
    // enough that this is acceptable for the scaffold.
    let combined = format!("{}\n{}", PRELUDE, src);
    let tokens = tokenize(&combined).map_err(|e| format!("lex error: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("parse error: {:?}", e))?;
    compile(&program).map_err(|e| format!("compile error: {:?}", e))
}
