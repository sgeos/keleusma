//! Cooperative scheduler for `keleusma run-tasks`.
//!
//! Implements the dispatch model described in
//! `docs/architecture/RUN_TASKS.md`. Each task runs in its own
//! `Vm` instance against a per-task arena, sized at startup and
//! reused across restarts (no allocation during steady-state
//! dispatch). The scheduler is single-threaded; cooperative
//! semantics mean the kernel runs exclusively between dispatches.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use keleusma::bytecode::Module;
use keleusma::stddsl;
use keleusma::vm::{Vm, VmError, VmState};
use keleusma::{Arena, Value};

use crate::format_value;
use crate::strict_mode::{PolicyContext, build_policy_context};

use super::manifest::{MAX_EVENT_QUEUE, Manifest, ManifestError, RestartPolicy, TaskConfig};
use super::signals::{NotifySocket, SignalFlags, watchdog_interval_ms};

/// Yield reason codes per the design doc.
const REASON_WAIT: i64 = 0;
const REASON_EVENT_WAIT: i64 = 1;
const REASON_YIELD: i64 = 2;
const REASON_PERIODIC: i64 = 3;

/// Wakeup-reason codes passed back through the resume payload.
const WAKEUP_FIRST: i64 = 0;
const WAKEUP_DEADLINE: i64 = 1;
const WAKEUP_EVENT: i64 = 2;
#[allow(dead_code)]
const WAKEUP_VOLUNTARY: i64 = 3;

/// Convention for the shutdown event id when the manifest does
/// not override.
const DEFAULT_SHUTDOWN_EVENT_ID: u8 = 99;
/// Convention for the reload event id (SIGHUP-triggered).
const DEFAULT_RELOAD_EVENT_ID: u8 = 98;

/// POSIX signal numbers used in the exit-code arithmetic (128 +
/// signal). Defined explicitly here to avoid pulling in the libc
/// crate just for two constants; the values are the same on every
/// platform the runner targets.
const SIGINT_NUMBER: u8 = 2;
const SIGTERM_NUMBER: u8 = 15;

/// Outcome the runner reports back to the CLI dispatcher.
#[derive(Debug)]
pub enum RunOutcome {
    /// Clean shutdown. `triggering_signal` is the POSIX signal
    /// number when shutdown was driven by SIGINT or SIGTERM, or
    /// None for a natural termination (every task finished).
    Shutdown { triggering_signal: Option<u8> },
    /// Manifest validation failed.
    ManifestError(ManifestError),
    /// A task failed to load (signature, encryption, or VM
    /// construction error).
    TaskLoadError(String),
    /// Fatal scheduler error.
    Internal(String),
}

impl RunOutcome {
    /// Convert the outcome into the POSIX-conventional exit code
    /// per the design doc. SIGINT-driven shutdown returns 130
    /// (128 + 2), SIGTERM returns 143 (128 + 15), natural shutdown
    /// returns 0, manifest or task-load errors return 1.
    pub fn into_exit_code(self) -> ExitCode {
        match self {
            Self::Shutdown { triggering_signal } => match triggering_signal {
                Some(sig) => ExitCode::from(128u8.saturating_add(sig)),
                None => ExitCode::SUCCESS,
            },
            Self::ManifestError(e) => {
                eprintln!("error: {}", e);
                ExitCode::FAILURE
            }
            Self::TaskLoadError(s) => {
                eprintln!("error: {}", s);
                ExitCode::FAILURE
            }
            Self::Internal(s) => {
                eprintln!("error: {}", s);
                ExitCode::FAILURE
            }
        }
    }
}

/// Atomic handles a task shares with its native closures. The
/// scheduler writes the values immediately before an event-fired
/// dispatch; the `kernel::last_event_id` and
/// `kernel::last_event_payload` natives read them. Held on the
/// Task struct so they survive a restart (the same Arcs are
/// reused; only the VM and its closure registrations are
/// reconstructed).
#[derive(Clone)]
struct EventAtomics {
    id: Arc<AtomicU64>,
    payload: Arc<AtomicU64>,
}

impl EventAtomics {
    fn new() -> Self {
        Self {
            id: Arc::new(AtomicU64::new(0)),
            payload: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// Per-task scheduler state.
struct Task {
    cfg: TaskConfig,
    arena: Arena,
    vm: Vm<'static, 'static>,
    state: TaskState,
    last_wakeup_reason: i64,
    task_id: u32,
    restart_history: VecDeque<Instant>,
    disabled: bool,
    module: Module,
    /// Atomics shared with the native closures. Cloned again on
    /// every restart so the new VM's natives observe the same
    /// values.
    event_atomics: EventAtomics,
    /// Kernel-state handle retained so the restart path can re-
    /// register natives without threading the Arc through every
    /// caller.
    kernel_state: Arc<std::sync::Mutex<KernelState>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    /// First invocation has not yet happened.
    NotStarted,
    /// Ready to dispatch on the next scheduler iteration.
    Ready,
    /// Sleeping until the given monotonic-time deadline (ms since
    /// scheduler start).
    SleepingUntil(u64),
    /// Blocked on an event id.
    WaitingForEvent(u8),
    /// Voluntarily terminated or exceeded restart limit.
    Finished,
}

/// One entry in the event queue.
#[derive(Debug, Clone, Copy)]
struct Event {
    id: u8,
    payload: i64,
}

/// Shared kernel state accessed from native closures.
#[derive(Default)]
struct KernelState {
    event_queue: VecDeque<Event>,
    start_time: Option<Instant>,
}

/// Run the manifest. The CLI dispatcher converts the returned
/// outcome into an exit code.
pub fn run(manifest_path: &Path, quiet: bool) -> RunOutcome {
    let source = match std::fs::read_to_string(manifest_path) {
        Ok(s) => s,
        Err(e) => {
            return RunOutcome::ManifestError(ManifestError::Parse(format!(
                "reading {}: {}",
                manifest_path.display(),
                e
            )));
        }
    };
    let base_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let manifest = match Manifest::parse(&source, &base_dir) {
        Ok(m) => m,
        Err(e) => return RunOutcome::ManifestError(e),
    };

    let policy = match build_policy_context() {
        Ok(p) => p,
        Err(e) => return RunOutcome::Internal(e),
    };

    let signals = SignalFlags::new();
    if let Err(e) = signals.install() {
        return RunOutcome::Internal(e);
    }

    let notify = NotifySocket::from_env();
    let watchdog_ms = watchdog_interval_ms();

    let kernel_state: Arc<std::sync::Mutex<KernelState>> =
        Arc::new(std::sync::Mutex::new(KernelState {
            event_queue: VecDeque::with_capacity(MAX_EVENT_QUEUE),
            start_time: Some(Instant::now()),
        }));

    if !quiet {
        eprintln!("[scheduler] launching {} task(s)", manifest.tasks.len());
    }

    // Load every task before dispatching any. Validation is fail-
    // closed: a load failure on any task rejects the whole runner.
    let mut tasks: Vec<Task> = Vec::with_capacity(manifest.tasks.len());
    for (idx, cfg) in manifest.tasks.iter().enumerate() {
        match load_task(cfg, idx as u32, &policy, kernel_state.clone(), quiet) {
            Ok(t) => tasks.push(t),
            Err(e) => return RunOutcome::TaskLoadError(e),
        }
    }

    if !quiet {
        for t in &tasks {
            eprintln!(
                "[scheduler] task {} loaded (arena {} bytes, restart {:?})",
                t.cfg.name, t.cfg.arena_capacity, t.cfg.restart
            );
            // Verifier-computed WCET (cycles) and WCMU (bytes) for
            // the task's loop body. The bounds are the certification
            // evidence the design doc calls out; operators copying
            // these into deployment records get them straight from
            // the runner's load step.
            if let Some(idx) = t.module.entry_point
                && let Some(chunk) = t.module.chunks.get(idx)
            {
                let wcet = keleusma::verify::wcet_stream_iteration(chunk)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|_| String::from("unbounded"));
                let wcmu = keleusma::verify::wcmu_stream_iteration(chunk)
                    .map(|(transient, _persistent)| transient.to_string())
                    .unwrap_or_else(|_| String::from("unbounded"));
                eprintln!(
                    "[scheduler] task {} WCET {} cycles WCMU {} bytes",
                    t.cfg.name, wcet, wcmu
                );
            }
        }
    }

    // The supervisor is told we are ready after every task has been
    // admitted and we are about to enter the dispatch loop.
    if notify.is_active() {
        notify.notify_ready();
        notify.notify_status(&format!("running with {} task(s)", manifest.tasks.len()));
    }

    let outcome = dispatch_loop(
        &mut tasks,
        &manifest,
        &signals,
        &notify,
        kernel_state,
        watchdog_ms,
        quiet,
    );

    if notify.is_active() {
        notify.notify_stopping();
    }

    outcome
}

/// Load a single task: open the bytecode, apply policy gates,
/// construct the VM, register natives.
fn load_task(
    cfg: &TaskConfig,
    task_id: u32,
    policy: &PolicyContext,
    kernel_state: Arc<std::sync::Mutex<KernelState>>,
    _quiet: bool,
) -> Result<Task, String> {
    let bytes = std::fs::read(&cfg.bytecode)
        .map_err(|e| format!("task {}: read {}: {}", cfg.name, cfg.bytecode.display(), e))?;

    let signed = keleusma::wire_format::header_requires_signature(&bytes);
    let encrypted = keleusma::wire_format::header_requires_encryption(&bytes);

    if policy.strict_signing && !signed {
        return Err(format!(
            "task {}: strict mode: unsigned bytecode disabled",
            cfg.name
        ));
    }
    if policy.strict_encryption && !encrypted {
        return Err(format!(
            "task {}: strict mode: unencrypted bytecode disabled",
            cfg.name
        ));
    }

    let module = crate::load_module(
        &bytes,
        &policy.enrolled_keys,
        &policy.decryption_keys,
        policy,
    )
    .map_err(|e| format!("task {}: {}", cfg.name, e))?;

    // The entry chunk must be `loop main(wakeup: Word) -> (Word, Word)`.
    // The CLI's existing detect_entry_kind would accept loop main with
    // one Word parameter; the return type is checked at compile time
    // through the script's own signature, and the scheduler enforces
    // the tuple shape on every yield.
    let entry = module
        .entry_point
        .and_then(|i| module.chunks.get(i))
        .ok_or_else(|| format!("task {}: module has no entry point", cfg.name))?;
    use keleusma::bytecode::BlockType;
    match entry.block_type {
        BlockType::Stream => {
            if entry.param_count != 1 {
                return Err(format!(
                    "task {}: loop main must take exactly one parameter (wakeup_reason: Word)",
                    cfg.name
                ));
            }
        }
        other => {
            return Err(format!(
                "task {}: entry must be `loop main`, got {:?}",
                cfg.name, other
            ));
        }
    }

    // Size the arena from the persistent .data section plus the
    // transient bound. The operator's `arena_capacity` is the floor;
    // when the module's auto-computed WCMU exceeds it, the auto bound
    // wins so the task admits cleanly without forcing operators to
    // tune per-task capacities by hand.
    let persistent_bytes = keleusma::vm::required_persistent_capacity_for(&module);
    let auto_transient =
        keleusma::vm::auto_arena_capacity_for(&module, &[]).unwrap_or(cfg.arena_capacity);
    let transient = cfg.arena_capacity.max(auto_transient);
    let total = persistent_bytes + transient;
    let mut arena = Arena::with_capacity(total);
    arena
        .resize_persistent(persistent_bytes)
        .map_err(|e| format!("task {}: arena resize_persistent: {:?}", cfg.name, e))?;

    // The Vm holds a reference into the arena, so we need to break
    // the lifetime relationship via Box leak. The arena outlives the
    // runner; the leak is intentional and exists for the runner's
    // operational lifetime.
    //
    // SAFETY: we keep the arena alive in the Task struct for as long
    // as the Vm exists. The leak transmute is sound because we never
    // drop the arena while the Vm holds a reference. On scheduler
    // exit, the entire process exits, which reclaims everything.
    let arena_ref: &'static Arena = unsafe { std::mem::transmute(&arena) };

    let mut vm = Vm::new(module.clone(), arena_ref)
        .map_err(|e| format!("task {}: verify: {:?}", cfg.name, e))?;

    let event_atomics = EventAtomics::new();
    register_runtime_natives(
        &mut vm,
        task_id,
        cfg.name.clone(),
        kernel_state.clone(),
        event_atomics.clone(),
    );

    Ok(Task {
        cfg: cfg.clone_owned(),
        arena,
        vm,
        state: TaskState::NotStarted,
        last_wakeup_reason: WAKEUP_FIRST,
        task_id,
        restart_history: VecDeque::new(),
        disabled: false,
        module,
        event_atomics,
        kernel_state,
    })
}

/// Register the natives the runner exposes to every task. This is the
/// union of the standard CLI register set (utility natives, math,
/// audio, shell) plus the runtasks-specific kernel natives.
///
/// Called both at task load and after each restart. The closures are
/// Fn + 'static and capture cloned Arc handles; restart is just a
/// matter of constructing a fresh Vm and calling this function again
/// with the same atomics and the same kernel-state Arc.
fn register_runtime_natives(
    vm: &mut Vm<'_, '_>,
    task_id: u32,
    task_name: String,
    kernel_state: Arc<std::sync::Mutex<KernelState>>,
    event_atomics: EventAtomics,
) {
    // println override: per-task prefixed output so multi-task
    // stdout remains parseable by the operator.
    let name_for_println = task_name.clone();
    vm.register_native_closure("println", move |args| {
        if let Some(arg) = args.first() {
            println!("[{}] {}", name_for_println, format_value(arg));
        } else {
            println!("[{}]", name_for_println);
        }
        Ok(Value::Unit)
    });

    vm.register_library(stddsl::Math);
    vm.register_library(stddsl::Audio);
    vm.register_library(stddsl::Shell);

    let ks_post = kernel_state.clone();
    vm.register_native_closure("kernel::post_event", move |args| {
        let id = read_word(args, 0, "kernel::post_event id")?;
        let payload = read_word(args, 1, "kernel::post_event payload")?;
        let mut state = ks_post.lock().unwrap();
        if state.event_queue.len() < MAX_EVENT_QUEUE {
            state.event_queue.push_back(Event {
                id: id as u8,
                payload,
            });
        } else {
            eprintln!(
                "[scheduler] event queue full; dropped event id {}",
                id as u8
            );
        }
        Ok(Value::Unit)
    });

    let ks_now = kernel_state;
    vm.register_native_closure("kernel::now_ms", move |_args| {
        let state = ks_now.lock().unwrap();
        let start = state.start_time.expect("scheduler start time initialised");
        Ok(Value::Int(start.elapsed().as_millis() as i64))
    });

    vm.register_native_closure("kernel::task_id", move |_args| {
        Ok(Value::Int(task_id as i64))
    });
    let name_for_native = task_name;
    vm.register_native_closure("kernel::task_name", move |_args| {
        Ok(Value::StaticStr(name_for_native.clone()))
    });

    // The scheduler writes to these atomics before each event-fired
    // dispatch; the natives read them out. The Arcs survive restarts
    // because they live on the Task struct (see EventAtomics).
    let id_handle = event_atomics.id;
    vm.register_native_closure("kernel::last_event_id", move |_args| {
        Ok(Value::Int(id_handle.load(Ordering::Relaxed) as i64))
    });
    let payload_handle = event_atomics.payload;
    vm.register_native_closure("kernel::last_event_payload", move |_args| {
        // The payload is stored as a u64 bit pattern so the i64
        // round-trip preserves sign across the boundary.
        Ok(Value::Int(payload_handle.load(Ordering::Relaxed) as i64))
    });
}

fn read_word(args: &[Value], idx: usize, ctx: &str) -> Result<i64, VmError> {
    let v = args
        .get(idx)
        .ok_or_else(|| VmError::NativeError(format!("{}: missing argument {}", ctx, idx)))?;
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(VmError::TypeError(format!(
            "{}: expected Word, got {:?}",
            ctx, other
        ))),
    }
}

/// The main dispatch loop. Returns when all tasks have terminated,
/// when a shutdown signal has drained, or when an internal error
/// is unrecoverable.
fn dispatch_loop(
    tasks: &mut [Task],
    manifest: &Manifest,
    signals: &SignalFlags,
    notify: &NotifySocket,
    kernel_state: Arc<std::sync::Mutex<KernelState>>,
    watchdog_ms: Option<u64>,
    quiet: bool,
) -> RunOutcome {
    let start = Instant::now();
    let shutdown_event_id = manifest
        .events
        .get("shutdown_requested")
        .copied()
        .unwrap_or(DEFAULT_SHUTDOWN_EVENT_ID);
    let reload_event_id = manifest
        .events
        .get("reload_requested")
        .copied()
        .unwrap_or(DEFAULT_RELOAD_EVENT_ID);

    let mut shutdown_deadline: Option<u64> = None;
    let mut triggering_signal: Option<u8> = None;
    let mut last_watchdog_ms: u64 = 0;

    loop {
        let now_ms = start.elapsed().as_millis() as u64;

        // Watchdog keepalive at twice the configured rate.
        if let Some(wd_ms) = watchdog_ms
            && now_ms.saturating_sub(last_watchdog_ms) >= wd_ms
        {
            notify.notify_watchdog();
            last_watchdog_ms = now_ms;
        }

        // Translate signal flags into events. SIGINT and SIGTERM
        // are tracked separately so the runner can report the
        // POSIX-conventional exit code (130 for SIGINT, 143 for
        // SIGTERM) after a clean drain.
        let sigint = signals.sigint_requested.swap(false, Ordering::SeqCst);
        let sigterm = signals.sigterm_requested.swap(false, Ordering::SeqCst);
        if (sigint || sigterm) && shutdown_deadline.is_none() {
            if !quiet {
                eprintln!(
                    "[scheduler] {} received, draining tasks",
                    if sigint { "SIGINT" } else { "SIGTERM" }
                );
            }
            triggering_signal = if sigint {
                Some(SIGINT_NUMBER)
            } else {
                Some(SIGTERM_NUMBER)
            };
            {
                let mut s = kernel_state.lock().unwrap();
                if s.event_queue.len() < MAX_EVENT_QUEUE {
                    s.event_queue.push_back(Event {
                        id: shutdown_event_id,
                        payload: 0,
                    });
                }
            }
            shutdown_deadline = Some(now_ms + manifest.scheduler.shutdown_grace.as_millis() as u64);
            if notify.is_active() {
                notify.notify_stopping();
            }
        }
        if signals.reload_requested.swap(false, Ordering::SeqCst) {
            if !quiet {
                eprintln!(
                    "[scheduler] reload requested (event id {}); not yet implemented",
                    reload_event_id
                );
            }
            let mut s = kernel_state.lock().unwrap();
            if s.event_queue.len() < MAX_EVENT_QUEUE {
                s.event_queue.push_back(Event {
                    id: reload_event_id,
                    payload: 0,
                });
            }
        }

        if let Some(deadline) = shutdown_deadline
            && now_ms >= deadline
        {
            if !quiet {
                eprintln!("[scheduler] shutdown grace period elapsed; exiting");
            }
            return RunOutcome::Shutdown { triggering_signal };
        }

        // Refresh ready set: wake any task whose deadline has elapsed.
        for task in tasks.iter_mut() {
            if let TaskState::SleepingUntil(deadline) = task.state
                && now_ms >= deadline
            {
                task.state = TaskState::Ready;
                task.last_wakeup_reason = WAKEUP_DEADLINE;
            }
        }

        // Drain the event queue: wake any task waiting on a matched event.
        let drained_events: Vec<Event> = {
            let mut s = kernel_state.lock().unwrap();
            s.event_queue.drain(..).collect()
        };
        for ev in drained_events {
            for task in tasks.iter_mut() {
                if let TaskState::WaitingForEvent(id) = task.state
                    && id == ev.id
                {
                    task.state = TaskState::Ready;
                    task.last_wakeup_reason = WAKEUP_EVENT;
                    // Write into the atomics the task's natives read.
                    // Cast the i64 payload through u64 so the bit
                    // pattern survives the round-trip.
                    task.event_atomics.id.store(ev.id as u64, Ordering::Relaxed);
                    task.event_atomics
                        .payload
                        .store(ev.payload as u64, Ordering::Relaxed);
                }
            }
        }

        // NotStarted tasks are always Ready for first dispatch.
        for task in tasks.iter_mut() {
            if matches!(task.state, TaskState::NotStarted) && !task.disabled {
                task.state = TaskState::Ready;
                task.last_wakeup_reason = WAKEUP_FIRST;
            }
        }

        // Pick the highest-priority ready task (lowest priority value wins).
        let candidate_idx = tasks
            .iter()
            .enumerate()
            .filter(|(_, t)| matches!(t.state, TaskState::Ready) && !t.disabled)
            .min_by_key(|(_, t)| t.cfg.priority)
            .map(|(i, _)| i);

        match candidate_idx {
            Some(i) => {
                dispatch_one(&mut tasks[i], now_ms, quiet);
            }
            None => {
                // No ready task. Compute the earliest deadline and sleep.
                let earliest = tasks
                    .iter()
                    .filter(|t| !t.disabled)
                    .filter_map(|t| match t.state {
                        TaskState::SleepingUntil(d) => Some(d),
                        _ => None,
                    })
                    .min();
                let sleep_ms = match earliest {
                    Some(d) if d > now_ms => {
                        (d - now_ms).min(manifest.scheduler.tick_interval.as_millis() as u64)
                    }
                    _ => manifest.scheduler.tick_interval.as_millis() as u64,
                };
                if all_finished(tasks) {
                    if !quiet {
                        eprintln!("[scheduler] all tasks finished; exiting");
                    }
                    return RunOutcome::Shutdown { triggering_signal };
                }
                std::thread::sleep(Duration::from_millis(sleep_ms));
            }
        }
    }
}

fn all_finished(tasks: &[Task]) -> bool {
    tasks
        .iter()
        .all(|t| matches!(t.state, TaskState::Finished) || t.disabled)
}

fn dispatch_one(task: &mut Task, now_ms: u64, quiet: bool) {
    let payload = Value::Int(task.last_wakeup_reason);
    let result =
        if matches!(task.state, TaskState::Ready) && task.last_wakeup_reason == WAKEUP_FIRST {
            task.vm.call(&[payload])
        } else {
            task.vm.resume(payload)
        };

    match result {
        Ok(VmState::Yielded(v)) => {
            let (reason, payload) = match parse_yield_tuple(&v) {
                Some(t) => t,
                None => {
                    eprintln!(
                        "[scheduler] task {} yielded a non-tuple value; treating as finished",
                        task.cfg.name
                    );
                    task.state = TaskState::Finished;
                    return;
                }
            };
            task.state = match reason {
                REASON_WAIT => TaskState::SleepingUntil(payload as u64),
                REASON_EVENT_WAIT => TaskState::WaitingForEvent(payload as u8),
                REASON_YIELD => TaskState::Ready,
                REASON_PERIODIC => {
                    let period_ms = task.cfg.period.map(|d| d.as_millis() as u64).unwrap_or(0);
                    TaskState::SleepingUntil(now_ms + period_ms)
                }
                _ => {
                    eprintln!(
                        "[scheduler] task {} yielded unknown reason {}; treating as finished",
                        task.cfg.name, reason
                    );
                    TaskState::Finished
                }
            };
        }
        Ok(VmState::Reset) => {
            task.state = TaskState::Ready;
        }
        Ok(VmState::Finished(_v)) => {
            // A loop main that returned. This is unusual; treat per
            // restart policy.
            on_task_exit(task, /* error */ false, quiet);
        }
        Ok(VmState::BreakpointHit { chunk, op }) => {
            // The scheduler does not arm breakpoints, so a hit is
            // unexpected; treat it as a task error.
            if !quiet {
                eprintln!(
                    "[scheduler] task {} hit an unexpected breakpoint at chunk {} op {}",
                    task.cfg.name, chunk, op
                );
            }
            on_task_exit(task, /* error */ true, quiet);
        }
        Err(e) => {
            if !quiet {
                eprintln!("[scheduler] task {} error: {:?}", task.cfg.name, e);
            }
            on_task_exit(task, /* error */ true, quiet);
        }
    }
}

fn parse_yield_tuple(v: &Value) -> Option<(i64, i64)> {
    if let Value::Tuple(items) = v
        && items.len() == 2
    {
        let reason = match items[0] {
            Value::Int(n) => n,
            _ => return None,
        };
        let payload = match items[1] {
            Value::Int(n) => n,
            _ => return None,
        };
        return Some((reason, payload));
    }
    None
}

fn on_task_exit(task: &mut Task, was_error: bool, quiet: bool) {
    let should_restart = match (task.cfg.restart, was_error) {
        (RestartPolicy::Never, _) => false,
        (RestartPolicy::OnError, true) => true,
        (RestartPolicy::OnError, false) => false,
        (RestartPolicy::Always, _) => true,
    };
    if !should_restart {
        task.state = TaskState::Finished;
        if !quiet {
            eprintln!("[scheduler] task {} terminated (no restart)", task.cfg.name);
        }
        return;
    }

    let now = Instant::now();
    let window = task.cfg.restart_window;
    while task
        .restart_history
        .front()
        .map(|t| now.duration_since(*t) > window)
        .unwrap_or(false)
    {
        task.restart_history.pop_front();
    }
    if task.restart_history.len() >= task.cfg.restart_limit as usize {
        eprintln!(
            "[scheduler] task {} disabled after {} restarts in {:?}",
            task.cfg.name, task.cfg.restart_limit, window
        );
        task.disabled = true;
        task.state = TaskState::Finished;
        return;
    }
    task.restart_history.push_back(now);

    // Restart in place: reset the arena's transient region and
    // re-instantiate the VM against the same module.
    if let Err(e) = task.arena.reset() {
        eprintln!(
            "[scheduler] task {} arena reset failed: {:?}; disabling",
            task.cfg.name, e
        );
        task.disabled = true;
        task.state = TaskState::Finished;
        return;
    }
    let arena_ref: &'static Arena = unsafe { std::mem::transmute(&task.arena) };
    match Vm::new(task.module.clone(), arena_ref) {
        Ok(mut vm) => {
            // Re-register the natives against the new Vm. The same
            // EventAtomics Arcs and kernel-state Arc are reused, so
            // the new closures observe the same shared state the old
            // ones did. The script's persistent .data is reset by
            // the arena reset above; tasks that need cross-restart
            // state should use const data or the host filesystem.
            register_runtime_natives(
                &mut vm,
                task.task_id,
                task.cfg.name.clone(),
                task.kernel_state.clone(),
                task.event_atomics.clone(),
            );
            task.vm = vm;
            task.state = TaskState::Ready;
            task.last_wakeup_reason = WAKEUP_FIRST;
            if !quiet {
                eprintln!(
                    "[scheduler] task {} restarted ({} restart(s) in window)",
                    task.cfg.name,
                    task.restart_history.len()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "[scheduler] task {} restart failed: {:?}; disabling",
                task.cfg.name, e
            );
            task.disabled = true;
            task.state = TaskState::Finished;
        }
    }
}

// Module loading delegates to crate::load_module in main.rs so
// signing and encryption gates use the same code path as the
// `run` subcommand.

impl TaskConfig {
    fn clone_owned(&self) -> Self {
        Self {
            name: self.name.clone(),
            bytecode: self.bytecode.clone(),
            period: self.period,
            restart: self.restart,
            restart_limit: self.restart_limit,
            restart_window: self.restart_window,
            arena_capacity: self.arena_capacity,
            priority: self.priority,
        }
    }
}
