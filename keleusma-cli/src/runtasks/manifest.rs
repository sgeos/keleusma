//! Manifest parsing and validation for `keleusma run-tasks`.
//!
//! The manifest is TOML. The schema is documented in
//! `docs/architecture/RUN_TASKS.md`. This module produces a validated
//! `Manifest` struct from a TOML source string, rejecting malformed
//! input fail-closed.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use crate::duration;

/// Maximum number of tasks admissible in a single manifest. The
/// fixed cap simplifies the scheduler's data structures and bounds
/// the worst-case resident set. Operators with more tasks should
/// split into multiple runner processes.
pub const MAX_TASKS: usize = 16;

/// Maximum capacity of the event queue. Operators with bursty
/// workloads beyond this should write their own host.
pub const MAX_EVENT_QUEUE: usize = 64;

const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
const DEFAULT_TICK_INTERVAL: Duration = Duration::from_millis(10);
const DEFAULT_RESTART_LIMIT: u32 = 5;
const DEFAULT_RESTART_WINDOW: Duration = Duration::from_secs(60);
const DEFAULT_ARENA_CAPACITY: usize = 64 * 1024;

/// Errors surfaced by manifest parsing and validation.
#[derive(Debug)]
pub enum ManifestError {
    /// TOML well-formedness failure.
    Parse(String),
    /// Required field absent.
    #[allow(dead_code)]
    MissingField(String),
    /// Field has the wrong shape (range, type, format).
    InvalidField(String),
    /// Duplicate task name.
    DuplicateTaskName(String),
    /// Duplicate event name.
    DuplicateEventName(String),
    /// Too many tasks declared.
    TooManyTasks(usize),
    /// Bytecode file does not exist on disk.
    BytecodeMissing(PathBuf),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(s) => write!(f, "manifest: {}", s),
            Self::MissingField(s) => write!(f, "manifest: missing field: {}", s),
            Self::InvalidField(s) => write!(f, "manifest: invalid field: {}", s),
            Self::DuplicateTaskName(s) => {
                write!(f, "manifest: duplicate task name: `{}`", s)
            }
            Self::DuplicateEventName(s) => {
                write!(f, "manifest: duplicate event name: `{}`", s)
            }
            Self::TooManyTasks(n) => write!(
                f,
                "manifest: {} tasks declared; maximum is {}",
                n, MAX_TASKS
            ),
            Self::BytecodeMissing(p) => {
                write!(f, "manifest: bytecode file does not exist: {:?}", p)
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// Restart policy for a single task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Task exit or error is terminal for that task.
    Never,
    /// Task is restarted on any `VmError`. Voluntary termination is
    /// terminal.
    OnError,
    /// Task is restarted on both error and voluntary termination.
    Always,
}

impl RestartPolicy {
    fn parse(s: &str) -> Result<Self, ManifestError> {
        match s {
            "never" => Ok(Self::Never),
            "on_error" => Ok(Self::OnError),
            "always" => Ok(Self::Always),
            other => Err(ManifestError::InvalidField(format!(
                "restart = `{}`; expected one of: never, on_error, always",
                other
            ))),
        }
    }
}

/// Top-level manifest after parsing and validation. Fields are
/// concrete types rather than `Option<String>` strings; durations
/// have been parsed through [`duration::parse`].
#[derive(Debug)]
pub struct Manifest {
    pub scheduler: SchedulerConfig,
    pub tasks: Vec<TaskConfig>,
    pub events: BTreeMap<String, u8>,
}

/// Scheduler-wide configuration drawn from the `[scheduler]` table.
#[derive(Debug)]
pub struct SchedulerConfig {
    pub tick_interval: Duration,
    pub shutdown_grace: Duration,
}

/// One entry from the `[[task]]` table array.
#[derive(Debug)]
pub struct TaskConfig {
    pub name: String,
    pub bytecode: PathBuf,
    pub period: Option<Duration>,
    pub restart: RestartPolicy,
    pub restart_limit: u32,
    pub restart_window: Duration,
    pub arena_capacity: usize,
    pub priority: i32,
}

// ---- Raw TOML structures used only for deserialisation. ----

#[derive(Deserialize)]
struct RawManifest {
    #[serde(default)]
    scheduler: Option<RawScheduler>,
    #[serde(default)]
    task: Vec<RawTask>,
    #[serde(default)]
    events: BTreeMap<String, i64>,
}

#[derive(Deserialize)]
struct RawScheduler {
    tick_interval: Option<String>,
    shutdown_grace: Option<String>,
}

#[derive(Deserialize)]
struct RawTask {
    name: String,
    bytecode: String,
    period: Option<String>,
    restart: Option<String>,
    restart_limit: Option<u32>,
    restart_window: Option<String>,
    arena_capacity: Option<String>,
    priority: Option<i32>,
}

impl Manifest {
    /// Parse a manifest from a TOML source string and validate it.
    ///
    /// The `base_dir` argument is the directory the manifest file
    /// lives in; relative `bytecode` paths are resolved against it.
    pub fn parse(source: &str, base_dir: &std::path::Path) -> Result<Self, ManifestError> {
        let raw: RawManifest =
            toml::from_str(source).map_err(|e| ManifestError::Parse(e.message().to_string()))?;

        if raw.task.is_empty() {
            return Err(ManifestError::InvalidField(String::from(
                "[[task]] array must declare at least one task",
            )));
        }
        if raw.task.len() > MAX_TASKS {
            return Err(ManifestError::TooManyTasks(raw.task.len()));
        }

        // Scheduler configuration with defaults.
        let scheduler = {
            let raw_sched = raw.scheduler.unwrap_or(RawScheduler {
                tick_interval: None,
                shutdown_grace: None,
            });
            let tick_interval = match raw_sched.tick_interval {
                Some(s) => duration::parse(&s).map_err(|e| {
                    ManifestError::InvalidField(format!("scheduler.tick_interval: {}", e))
                })?,
                None => DEFAULT_TICK_INTERVAL,
            };
            let shutdown_grace = match raw_sched.shutdown_grace {
                Some(s) => duration::parse(&s).map_err(|e| {
                    ManifestError::InvalidField(format!("scheduler.shutdown_grace: {}", e))
                })?,
                None => DEFAULT_SHUTDOWN_GRACE,
            };
            SchedulerConfig {
                tick_interval,
                shutdown_grace,
            }
        };

        // Tasks.
        let mut seen_names = std::collections::BTreeSet::new();
        let mut tasks = Vec::with_capacity(raw.task.len());
        for rt in raw.task {
            if !seen_names.insert(rt.name.clone()) {
                return Err(ManifestError::DuplicateTaskName(rt.name));
            }
            let bytecode = {
                let p = std::path::PathBuf::from(&rt.bytecode);
                let resolved = if p.is_absolute() { p } else { base_dir.join(p) };
                if !resolved.exists() {
                    return Err(ManifestError::BytecodeMissing(resolved));
                }
                resolved
            };
            let period = match rt.period {
                Some(s) => Some(duration::parse(&s).map_err(|e| {
                    ManifestError::InvalidField(format!("task[{}].period: {}", rt.name, e))
                })?),
                None => None,
            };
            let restart = match rt.restart {
                Some(s) => RestartPolicy::parse(&s)?,
                None => RestartPolicy::OnError,
            };
            let restart_limit = rt.restart_limit.unwrap_or(DEFAULT_RESTART_LIMIT);
            if !(1..=1000).contains(&restart_limit) {
                return Err(ManifestError::InvalidField(format!(
                    "task[{}].restart_limit = {}; must be in 1..=1000",
                    rt.name, restart_limit
                )));
            }
            let restart_window = match rt.restart_window {
                Some(s) => duration::parse(&s).map_err(|e| {
                    ManifestError::InvalidField(format!("task[{}].restart_window: {}", rt.name, e))
                })?,
                None => DEFAULT_RESTART_WINDOW,
            };
            if restart_window < Duration::from_secs(1) || restart_window > Duration::from_secs(3600)
            {
                return Err(ManifestError::InvalidField(format!(
                    "task[{}].restart_window: must be between 1s and 1h",
                    rt.name
                )));
            }
            let arena_capacity = match rt.arena_capacity {
                Some(s) => parse_capacity(&s).map_err(|e| {
                    ManifestError::InvalidField(format!("task[{}].arena_capacity: {}", rt.name, e))
                })?,
                None => DEFAULT_ARENA_CAPACITY,
            };
            let priority = rt.priority.unwrap_or(0);
            tasks.push(TaskConfig {
                name: rt.name,
                bytecode,
                period,
                restart,
                restart_limit,
                restart_window,
                arena_capacity,
                priority,
            });
        }

        // Events. Validate id range and uniqueness; numeric ids must
        // fit in a Byte because the yield-reason EventWait payload is
        // a single Word, but only the low 8 bits identify the event.
        let mut events: BTreeMap<String, u8> = BTreeMap::new();
        for (name, id) in raw.events {
            if !(0..=255).contains(&id) {
                return Err(ManifestError::InvalidField(format!(
                    "events.{} = {}; event id must be in 0..=255",
                    name, id
                )));
            }
            if events.values().any(|&v| v == id as u8) {
                return Err(ManifestError::DuplicateEventName(format!(
                    "id {} declared more than once",
                    id
                )));
            }
            events.insert(name, id as u8);
        }

        Ok(Manifest {
            scheduler,
            tasks,
            events,
        })
    }
}

/// Parse a capacity spec like `64KB` or `4MB` or a bare byte count.
/// Returns the byte count.
fn parse_capacity(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if let Some(stripped) = s.strip_suffix("MB") {
        let n: usize = stripped
            .trim()
            .parse()
            .map_err(|e| format!("cannot parse `{}` as integer: {}", stripped, e))?;
        Ok(n * 1024 * 1024)
    } else if let Some(stripped) = s.strip_suffix("KB") {
        let n: usize = stripped
            .trim()
            .parse()
            .map_err(|e| format!("cannot parse `{}` as integer: {}", stripped, e))?;
        Ok(n * 1024)
    } else {
        let n: usize = s
            .parse()
            .map_err(|e| format!("cannot parse `{}` as integer: {}", s, e))?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_tmp_bytecode(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("keleusma_runtasks_test_{}.bin", name));
        // Minimal KELE magic so the file at least looks like bytecode
        // for the existence check.
        std::fs::write(&p, b"KELE").expect("write tmp bytecode");
        p
    }

    #[test]
    fn minimal_manifest_parses() {
        let bc = write_tmp_bytecode("min");
        let toml_src = format!(
            r#"
[[task]]
name = "t1"
bytecode = "{}"
"#,
            bc.display()
        );
        let m = Manifest::parse(&toml_src, std::path::Path::new("/")).expect("parse");
        assert_eq!(m.tasks.len(), 1);
        assert_eq!(m.tasks[0].name, "t1");
        assert_eq!(m.tasks[0].restart, RestartPolicy::OnError);
        assert_eq!(m.scheduler.shutdown_grace, DEFAULT_SHUTDOWN_GRACE);
        let _ = std::fs::remove_file(&bc);
    }

    #[test]
    fn rejects_empty_task_array() {
        let toml_src = "";
        let err = Manifest::parse(toml_src, std::path::Path::new("/")).expect_err("empty");
        assert!(matches!(err, ManifestError::InvalidField(_)));
    }

    #[test]
    fn rejects_duplicate_task_name() {
        let bc = write_tmp_bytecode("dup");
        let toml_src = format!(
            r#"
[[task]]
name = "t1"
bytecode = "{0}"

[[task]]
name = "t1"
bytecode = "{0}"
"#,
            bc.display()
        );
        let err = Manifest::parse(&toml_src, std::path::Path::new("/")).expect_err("dup");
        assert!(matches!(err, ManifestError::DuplicateTaskName(_)));
        let _ = std::fs::remove_file(&bc);
    }

    #[test]
    fn rejects_too_many_tasks() {
        let bc = write_tmp_bytecode("many");
        let mut toml_src = String::new();
        for i in 0..(MAX_TASKS + 1) {
            toml_src.push_str(&format!(
                "[[task]]\nname = \"t{}\"\nbytecode = \"{}\"\n\n",
                i,
                bc.display()
            ));
        }
        let err = Manifest::parse(&toml_src, std::path::Path::new("/")).expect_err("too many");
        assert!(matches!(err, ManifestError::TooManyTasks(_)));
        let _ = std::fs::remove_file(&bc);
    }

    #[test]
    fn rejects_missing_bytecode() {
        let toml_src = r#"
[[task]]
name = "t1"
bytecode = "/nonexistent/missing.bin"
"#;
        let err = Manifest::parse(toml_src, std::path::Path::new("/")).expect_err("missing");
        assert!(matches!(err, ManifestError::BytecodeMissing(_)));
    }

    #[test]
    fn rejects_bad_restart() {
        let bc = write_tmp_bytecode("badrestart");
        let toml_src = format!(
            r#"
[[task]]
name = "t1"
bytecode = "{}"
restart = "occasionally"
"#,
            bc.display()
        );
        let err = Manifest::parse(&toml_src, std::path::Path::new("/")).expect_err("bad restart");
        assert!(matches!(err, ManifestError::InvalidField(_)));
        let _ = std::fs::remove_file(&bc);
    }

    #[test]
    fn parses_full_manifest() {
        let bc = write_tmp_bytecode("full");
        let toml_src = format!(
            r#"
[scheduler]
tick_interval = "20ms"
shutdown_grace = "10s"

[[task]]
name = "sensor"
bytecode = "{0}"
period = "100ms"
restart = "always"
restart_limit = 10
restart_window = "5m"
arena_capacity = "128KB"
priority = 1

[events]
data_ready = 1
shutdown_requested = 99
"#,
            bc.display()
        );
        let m = Manifest::parse(&toml_src, std::path::Path::new("/")).expect("parse");
        assert_eq!(m.scheduler.tick_interval, Duration::from_millis(20));
        assert_eq!(m.scheduler.shutdown_grace, Duration::from_secs(10));
        assert_eq!(m.tasks[0].period, Some(Duration::from_millis(100)));
        assert_eq!(m.tasks[0].restart, RestartPolicy::Always);
        assert_eq!(m.tasks[0].restart_limit, 10);
        assert_eq!(m.tasks[0].restart_window, Duration::from_secs(300));
        assert_eq!(m.tasks[0].arena_capacity, 128 * 1024);
        assert_eq!(m.tasks[0].priority, 1);
        assert_eq!(m.events.get("data_ready"), Some(&1));
        assert_eq!(m.events.get("shutdown_requested"), Some(&99));
        let _ = std::fs::remove_file(&bc);
    }

    #[test]
    fn parse_capacity_units() {
        assert_eq!(parse_capacity("65536").unwrap(), 65536);
        assert_eq!(parse_capacity("64KB").unwrap(), 65536);
        assert_eq!(parse_capacity("4MB").unwrap(), 4 * 1024 * 1024);
    }
}
