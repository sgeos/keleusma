//! Kernel core. Cooperative scheduler over a fixed set of tasks,
//! each backed by a Keleusma `loop main` virtual machine.
//!
//! The dispatch loop picks the ready task with the earliest
//! pending wakeup, resumes its VM with the wakeup reason, reads
//! the yielded `(reason, payload)` tuple back, updates the
//! task's scheduling state, and repeats. When no task is ready
//! the scheduler asks the platform to `sleep_until` the next
//! pending wakeup.
//!
//! The yield convention.
//!
//! Each task's `loop main` has signature
//!     `loop main(reason: Word) -> (Word, Word)`.
//! The script yields `(reason, payload)` to request the next
//! dispatch. The reason codes are documented in
//! [`YieldReason`]. The payload is an absolute millisecond
//! timestamp for `SleepUntil`, or a reason-specific value for
//! the others.

use alloc::vec::Vec;
use core::marker::PhantomData;

use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState};

use crate::platform::Platform;

/// Reason codes that the kernel and tasks agree on. The
/// numeric form is what crosses the script boundary; the enum
/// is for kernel-internal use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YieldReason {
    /// Wake at the absolute monotonic time given by the
    /// payload (milliseconds since boot).
    SleepUntil = 0,
    /// Yield without a timing constraint. The scheduler picks
    /// the next ready task; this task becomes ready again
    /// immediately.
    Yield = 1,
    /// Wait for an event signalled by `payload` (event id).
    /// Future iterations of this draft will implement the
    /// event bus; the initial scaffold accepts the code but
    /// treats it as a permanent wait.
    WaitForEvent = 2,
}

/// Wakeup reasons the kernel passes back to a task when it
/// resumes. Mirrors [`YieldReason`] in numeric form.
#[derive(Clone, Copy, Debug)]
pub enum WakeReason {
    /// First call to the task since boot.
    FirstRun = 0,
    /// The task's `SleepUntil` deadline has elapsed.
    Timer = 1,
    /// The event the task was waiting for has fired.
    Event = 2,
}

pub struct Task {
    pub name: &'static str,
    pub vm: Vm<'static, 'static>,
    pub state: TaskState,
    pub started: bool,
    /// Maximum declared WCET in pipelined cycles per yield slice
    /// that this task is admitted under. The kernel rejects a hot
    /// swap whose new module declares a WCET above this budget.
    /// `None` disables the check (legacy behaviour). The value is
    /// installed at task construction and not changed by the
    /// scheduler.
    pub wcet_budget_cycles: Option<u32>,
    /// Restart policy on `VmError::Halt`. The kernel tracks
    /// per-task restart counts; on Halt the kernel attempts a
    /// VM reset and re-dispatches the task as long as the count
    /// is below `max_restarts`. `max_restarts == 0` disables the
    /// recovery path and matches the legacy behaviour of marking
    /// the task `Finished` on the first halt error.
    pub max_restarts: u32,
    pub restart_count: u32,
}

#[derive(Clone, Copy, Debug)]
pub enum TaskState {
    Ready(WakeReason),
    SleepingUntil(u64),
    WaitingFor(u8),
    Finished,
}

pub struct Kernel<P: Platform> {
    tasks: Vec<Task>,
    /// Pending events queued for delivery to tasks that
    /// `WaitingFor(event_id)`. The scheduler drains this list at
    /// the top of each dispatch iteration; events that no task is
    /// currently waiting for are dropped. Order is preserved but
    /// not relied on by the scheduler (event arrival is treated
    /// as a level-triggered notification, not edge-counted).
    pending_events: Vec<u8>,
    /// Optional internal event ticker. When set, the scheduler
    /// posts `event_id` every `period_ms` milliseconds of
    /// monotonic time. The demonstrator uses this to simulate
    /// an interrupt source without external coordination; real
    /// deployments wire `post_event` from an ISR or a co-spawned
    /// task that owns the kernel through a mutex or channel.
    event_tick: Option<EventTick>,
    _phantom: PhantomData<P>,
}

#[derive(Clone, Copy, Debug)]
struct EventTick {
    event_id: u8,
    period_ms: u32,
    next_due_ms: u64,
}

impl<P: Platform> Kernel<P> {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            pending_events: Vec::new(),
            event_tick: None,
            _phantom: PhantomData,
        }
    }

    /// Enable the kernel's internal event ticker. The scheduler
    /// then posts `event_id` every `period_ms` milliseconds. The
    /// demonstrator uses this to wake the `event_listener` task
    /// without requiring an external poster; real deployments
    /// disable the ticker and wire `post_event` from a host-side
    /// driver or interrupt handler.
    pub fn enable_event_tick(&mut self, event_id: u8, period_ms: u32) {
        self.event_tick = Some(EventTick {
            event_id,
            period_ms,
            next_due_ms: 0,
        });
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    /// Queue an event for delivery to any task currently in the
    /// `WaitingFor(event_id)` state. The kernel drains the queue
    /// at the top of each scheduler iteration; an event posted
    /// while no task is waiting is dropped. Hosts post events from
    /// any context that can borrow the kernel mutably; on `std`
    /// this is typically a thread that holds `Arc<Mutex<Kernel>>`
    /// or interleaves dispatch with event production through the
    /// async executor's polling, on embassy this is a co-spawned
    /// task that holds the kernel through a channel.
    pub fn post_event(&mut self, event_id: u8) {
        self.pending_events.push(event_id);
    }

    /// Dispatch loop. Runs forever unless every task finishes,
    /// in which case it returns. The latter is only useful for
    /// tests; production tasks are `loop main` and never finish.
    ///
    /// The function is `async` so the platform's `sleep_until`
    /// can yield to an executor (embassy, tokio, or the
    /// minimal block-on for the std demonstrator).
    pub async fn run(&mut self) {
        loop {
            let now = P::now_ms();
            // Feed the platform watchdog at the top of every
            // scheduler iteration. The default `Platform::feed_watchdog`
            // implementation is a no-op; platforms that arm a
            // hardware watchdog override it to pet the timer.
            // The pet point is the scheduler iteration boundary
            // rather than per-dispatch because each task's slice
            // has a verified WCET bound; the scheduler is the
            // outer liveness signal.
            P::feed_watchdog();
            // Fire the internal event ticker if its deadline has
            // passed. The ticker is the demonstrator's stand-in
            // for an interrupt source; the post itself is
            // structurally identical to a call from an ISR
            // handler.
            if let Some(tick) = &mut self.event_tick
                && now >= tick.next_due_ms
            {
                let id = tick.event_id;
                tick.next_due_ms = now + tick.period_ms as u64;
                self.pending_events.push(id);
            }
            // Drain pending events. For each event id, find every
            // task currently `WaitingFor(id)` and promote it to
            // ready with `WakeReason::Event`. Events that no task
            // is waiting for are dropped.
            if !self.pending_events.is_empty() {
                let events: alloc::vec::Vec<u8> = self.pending_events.drain(..).collect();
                for event_id in events {
                    for t in &mut self.tasks {
                        if let TaskState::WaitingFor(waiting_for) = t.state
                            && waiting_for == event_id
                        {
                            t.state = TaskState::Ready(WakeReason::Event);
                        }
                    }
                }
            }
            // Promote sleeping tasks whose deadline has passed.
            for t in &mut self.tasks {
                if let TaskState::SleepingUntil(at) = t.state
                    && at <= now
                {
                    t.state = TaskState::Ready(WakeReason::Timer);
                }
            }
            // Pick the first ready task. Future iterations can
            // add priority, deadline-monotonic, or EDF.
            let idx = self
                .tasks
                .iter()
                .position(|t| matches!(t.state, TaskState::Ready(_)));
            match idx {
                Some(i) => self.dispatch(i),
                None => {
                    // No task ready. Sleep until the earliest
                    // pending wakeup, or break if every task
                    // has finished.
                    let next_wake = self.earliest_wakeup();
                    match next_wake {
                        Some(at) => P::sleep_until(at).await,
                        None => return,
                    }
                }
            }
        }
    }

    fn dispatch(&mut self, i: usize) {
        let reason = match self.tasks[i].state {
            TaskState::Ready(r) => r,
            _ => unreachable!("dispatch called on non-ready task"),
        };
        let reason_value = Value::Int(reason as i64);
        let result = if self.tasks[i].started {
            self.tasks[i].vm.resume(reason_value)
        } else {
            self.tasks[i].started = true;
            self.tasks[i].vm.call(&[reason_value])
        };
        match result {
            Ok(VmState::Yielded(Value::Tuple(t))) if t.len() == 2 => {
                let r = extract_int(&t[0]);
                let payload = extract_int(&t[1]);
                self.tasks[i].state = match r {
                    0 => TaskState::SleepingUntil(payload as u64),
                    1 => TaskState::Ready(WakeReason::Timer),
                    2 => TaskState::WaitingFor(payload as u8),
                    _ => {
                        P::log_event(crate::natives::EV_KERNEL_UNKNOWN_YIELD, r);
                        TaskState::Finished
                    }
                };
            }
            Ok(VmState::Reset) => {
                // `loop main` body wrapped past its terminator.
                // Treat as ready; the next iteration will
                // resume normally.
                self.tasks[i].state = TaskState::Ready(WakeReason::Timer);
            }
            Ok(VmState::Finished(_)) => {
                P::log_event(crate::natives::EV_KERNEL_TASK_FINISHED, 0);
                self.tasks[i].state = TaskState::Finished;
            }
            Ok(_other) => {
                P::log_event(crate::natives::EV_KERNEL_UNEXPECTED_STATE, 0);
                self.tasks[i].state = TaskState::Finished;
            }
            Err(e) => {
                // Map the VmError to a numeric code via the
                // three-way `VmError::category` policy. Concrete
                // error detail (string payloads in `TypeError`,
                // `NativeError`, etc.) is intentionally not
                // surfaced through `log_event`, which carries only
                // a code and a data word; the substantial flash
                // cost of Debug-formatting an arbitrary VmError
                // (~15 KB on this target) is avoided.
                let category = e.category();
                let category_code = match category {
                    keleusma::vm::VmErrorCategory::Halt => 0,
                    keleusma::vm::VmErrorCategory::SoftScript => 1,
                    keleusma::vm::VmErrorCategory::SoftHost => 2,
                };
                P::log_event(crate::natives::EV_KERNEL_VM_ERROR, category_code);
                // Supervised restart. If the task carries a
                // non-zero `max_restarts` budget and has not
                // exhausted it, the kernel resets the VM's
                // transient state (operand stack, call frames,
                // arena) through `Vm::reset_after_error` and
                // marks the task ready for a first-run dispatch.
                // The data segment survives the reset by design,
                // so accumulated state persists across the
                // recovery boundary. Tasks that exhaust their
                // budget or that did not opt in are marked
                // `Finished` as before.
                if self.tasks[i].restart_count < self.tasks[i].max_restarts {
                    self.tasks[i].restart_count += 1;
                    self.tasks[i].vm.reset_after_error();
                    self.tasks[i].started = false;
                    self.tasks[i].state = TaskState::Ready(WakeReason::FirstRun);
                    P::log_event(
                        crate::natives::EV_KERNEL_TASK_RESTARTED,
                        self.tasks[i].restart_count as i64,
                    );
                } else {
                    self.tasks[i].state = TaskState::Finished;
                }
            }
        }
    }

    fn earliest_wakeup(&self) -> Option<u64> {
        let task_deadline = self
            .tasks
            .iter()
            .filter_map(|t| match t.state {
                TaskState::SleepingUntil(at) => Some(at),
                _ => None,
            })
            .min();
        let tick_deadline = self.event_tick.map(|tick| tick.next_due_ms);
        match (task_deadline, tick_deadline) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        }
    }
}

impl<P: Platform> Default for Kernel<P> {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_int(v: &Value) -> i64 {
    match v {
        Value::Int(n) => *n,
        _ => 0,
    }
}
