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
    _phantom: PhantomData<P>,
}

impl<P: Platform> Kernel<P> {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            _phantom: PhantomData,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
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
                        P::log_event(
                            crate::natives::EV_KERNEL_UNKNOWN_YIELD,
                            r,
                        );
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
                let category_code = match e.category() {
                    keleusma::vm::VmErrorCategory::Halt => 0,
                    keleusma::vm::VmErrorCategory::SoftScript => 1,
                    keleusma::vm::VmErrorCategory::SoftHost => 2,
                };
                P::log_event(crate::natives::EV_KERNEL_VM_ERROR, category_code);
                self.tasks[i].state = TaskState::Finished;
            }
        }
    }

    fn earliest_wakeup(&self) -> Option<u64> {
        self.tasks
            .iter()
            .filter_map(|t| match t.state {
                TaskState::SleepingUntil(at) => Some(at),
                _ => None,
            })
            .min()
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
