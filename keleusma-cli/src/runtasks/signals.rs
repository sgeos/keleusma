//! Signal handling and notification-protocol detection for
//! `keleusma run-tasks`.
//!
//! The scheduler reads two atomic flags set from signal handlers
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
/// SIGINT and SIGTERM are tracked separately so the runner can return
/// the conventional POSIX exit code (128 + signal number) after a
/// clean drain. SIGHUP is reserved for future configuration-reload
/// work.
#[derive(Debug, Clone, Default)]
pub struct SignalFlags {
    pub sigint_requested: Arc<AtomicBool>,
    pub sigterm_requested: Arc<AtomicBool>,
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
            flag::register(SIGINT, self.sigint_requested.clone())
                .map_err(|e| format!("install SIGINT handler: {}", e))?;
            flag::register(SIGTERM, self.sigterm_requested.clone())
                .map_err(|e| format!("install SIGTERM handler: {}", e))?;
            flag::register(SIGHUP, self.reload_requested.clone())
                .map_err(|e| format!("install SIGHUP handler: {}", e))?;
        }
        #[cfg(windows)]
        {
            use signal_hook::consts::signal::{SIGINT, SIGTERM};
            use signal_hook::flag;
            flag::register(SIGINT, self.sigint_requested.clone())
                .map_err(|e| format!("install SIGINT handler: {}", e))?;
            // SIGTERM on Windows is delivered for Ctrl-Break in the
            // signal-hook crate's mapping. SIGHUP does not exist on
            // Windows; the reload_requested flag remains untouched.
            flag::register(SIGTERM, self.sigterm_requested.clone())
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
        let path_str = path.to_string_lossy();
        // Linux abstract namespace: the env variable begins with a
        // `@` and the actual socket address starts with a null byte.
        // The std library exposes this through
        // `std::os::linux::net::SocketAddrExt::from_abstract_name`
        // on Linux; on other Unix-likes (FreeBSD, OpenBSD, macOS) the
        // abstract namespace does not exist, so fall through to the
        // filesystem-path form. Operators on those platforms should
        // configure their supervisor to use a filesystem path.
        if let Some(stripped) = path_str.strip_prefix('@') {
            return send_to_abstract(stripped, msg);
        }
        let socket = UnixDatagram::unbound()?;
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

/// Connect to an abstract-namespace Unix socket on Linux. Only Linux
/// supports the abstract namespace.
#[cfg(target_os = "linux")]
fn send_to_abstract(name: &str, msg: &str) -> std::io::Result<()> {
    use std::os::linux::net::SocketAddrExt;
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes())?;
    let socket = UnixDatagram::unbound()?;
    socket.connect_addr(&addr)?;
    socket.send(msg.as_bytes())?;
    Ok(())
}

/// Non-Linux Unix platforms (FreeBSD, OpenBSD, macOS) do not have
/// the abstract namespace; reject the call rather than fall back to
/// a misleading filesystem path.
#[cfg(all(unix, not(target_os = "linux")))]
fn send_to_abstract(_name: &str, _msg: &str) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "abstract NOTIFY_SOCKET addresses are Linux-only; configure the supervisor to use a filesystem-path socket",
    ))
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn send_to_abstract(_name: &str, _msg: &str) -> std::io::Result<()> {
    Ok(())
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
