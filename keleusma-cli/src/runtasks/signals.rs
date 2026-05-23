//! Signal handling and notification-protocol detection for
//! `keleusma run-tasks`.
//!
//! The scheduler reads three atomic flags set from signal handlers
//! installed at startup. On Unix-like operating systems
//! (Linux, FreeBSD, OpenBSD, macOS) the handlers cover SIGINT,
//! SIGTERM, and SIGHUP. On Windows the handlers cover the Ctrl-C and
//! Ctrl-Break console control events; SIGHUP has no Windows
//! equivalent so the SIGHUP flag is never set.
//!
//! The NOTIFY_SOCKET environment-variable protocol is detected on
//! every platform that exposes Unix sockets through the standard
//! library. The protocol mechanics are platform-agnostic; only the
//! Linux systemd supervisor sets the variable by default, but other
//! supervisors are free to adopt the convention.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[cfg(unix)]
use std::os::unix::net::UnixDatagram;

/// Atomic flags written by signal handlers and read by the scheduler.
#[derive(Debug, Clone, Default)]
pub struct SignalFlags {
    pub shutdown_requested: Arc<AtomicBool>,
    pub reload_requested: Arc<AtomicBool>,
}

impl SignalFlags {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install signal handlers for the supported signals on this
    /// platform. Returns on success; an error means a handler could
    /// not be installed and the runner should refuse to start.
    pub fn install(&self) -> Result<(), String> {
        #[cfg(unix)]
        {
            use signal_hook::consts::signal::{SIGHUP, SIGINT, SIGTERM};
            use signal_hook::flag;
            flag::register(SIGINT, self.shutdown_requested.clone())
                .map_err(|e| format!("install SIGINT handler: {}", e))?;
            flag::register(SIGTERM, self.shutdown_requested.clone())
                .map_err(|e| format!("install SIGTERM handler: {}", e))?;
            flag::register(SIGHUP, self.reload_requested.clone())
                .map_err(|e| format!("install SIGHUP handler: {}", e))?;
        }
        #[cfg(windows)]
        {
            use signal_hook::consts::signal::{SIGINT, SIGTERM};
            use signal_hook::flag;
            flag::register(SIGINT, self.shutdown_requested.clone())
                .map_err(|e| format!("install SIGINT handler: {}", e))?;
            // SIGTERM on Windows is delivered for Ctrl-Break in the
            // signal-hook crate's mapping. SIGHUP does not exist on
            // Windows; the reload_requested flag remains untouched.
            flag::register(SIGTERM, self.shutdown_requested.clone())
                .map_err(|e| format!("install SIGTERM handler: {}", e))?;
        }
        Ok(())
    }
}

/// Optional integration with the systemd-style notification protocol.
/// The `NOTIFY_SOCKET` environment variable identifies a Unix-socket
/// path; if set, the runner sends `READY=1`, `STATUS=...`,
/// `STOPPING=1`, and `WATCHDOG=1` messages at the appropriate scheduler
/// transitions. When the variable is unset, all methods on this struct
/// are no-ops.
#[derive(Debug, Default)]
pub struct NotifySocket {
    socket_path: Option<std::path::PathBuf>,
}

impl NotifySocket {
    /// Detect the protocol from the environment. Returns a stub
    /// with no socket configured when `NOTIFY_SOCKET` is unset.
    pub fn from_env() -> Self {
        match std::env::var("NOTIFY_SOCKET").ok() {
            Some(path) if !path.is_empty() => Self {
                socket_path: Some(std::path::PathBuf::from(path)),
            },
            _ => Self::default(),
        }
    }

    /// True when the protocol is active for this process.
    pub fn is_active(&self) -> bool {
        self.socket_path.is_some()
    }

    /// Send a single notification line. The protocol uses
    /// newline-separated key=value pairs in a single datagram.
    fn send(&self, msg: &str) {
        let Some(ref path) = self.socket_path else {
            return;
        };
        // The protocol supports both abstract (Linux-only, leading
        // null byte) and filesystem-path socket addresses. Fall back
        // to the filesystem-path form on platforms that do not
        // support the abstract form; the protocol is rare on
        // non-Linux supervisors and the filesystem form is the
        // portable subset.
        match Self::send_to(path, msg) {
            Ok(()) => {}
            Err(e) => {
                // A failed notification is informational rather than
                // fatal. The scheduler continues without it.
                eprintln!(
                    "[scheduler] notify-socket write failed: {}; continuing without notifications",
                    e
                );
            }
        }
    }

    #[cfg(unix)]
    fn send_to(path: &Path, msg: &str) -> std::io::Result<()> {
        let socket = UnixDatagram::unbound()?;
        // Linux abstract namespace: path begins with a `@` in the env
        // variable and the actual address uses a leading null byte.
        let path_str = path.to_string_lossy();
        if let Some(stripped) = path_str.strip_prefix('@') {
            let mut bytes = Vec::with_capacity(stripped.len() + 1);
            bytes.push(0u8);
            bytes.extend_from_slice(stripped.as_bytes());
            // Abstract sockets are Linux-only; the std library API
            // does not expose them directly. Drop the abstract case
            // here; operators on Linux will see the protocol go to
            // /dev/null but the scheduler still functions.
            let _ = bytes;
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "abstract NOTIFY_SOCKET addresses not yet supported",
            ));
        }
        socket.connect(path)?;
        socket.send(msg.as_bytes())?;
        Ok(())
    }

    #[cfg(not(unix))]
    fn send_to(_path: &Path, _msg: &str) -> std::io::Result<()> {
        // Windows: NOTIFY_SOCKET is not used. Returning Ok keeps the
        // calling code uniform.
        Ok(())
    }

    /// Notify the supervisor that startup is complete.
    pub fn notify_ready(&self) {
        self.send("READY=1\n");
    }

    /// Send a STATUS line. The supervisor records or displays it.
    pub fn notify_status(&self, status: &str) {
        let mut s = String::with_capacity(status.len() + 8);
        s.push_str("STATUS=");
        s.push_str(status);
        s.push('\n');
        self.send(&s);
    }

    /// Notify the supervisor that shutdown has begun.
    pub fn notify_stopping(&self) {
        self.send("STOPPING=1\n");
    }

    /// Send a watchdog keepalive.
    pub fn notify_watchdog(&self) {
        self.send("WATCHDOG=1\n");
    }
}

/// Parse the WATCHDOG_USEC environment variable if present. The
/// systemd protocol uses microseconds. Returns the watchdog deadline
/// in milliseconds, halved per the protocol convention (the runner
/// must emit at twice the configured rate to remain safely under
/// the deadline).
pub fn watchdog_interval_ms() -> Option<u64> {
    let raw = std::env::var("WATCHDOG_USEC").ok()?;
    let usec: u64 = raw.parse().ok()?;
    let ms = usec / 1000;
    Some(ms / 2)
}
