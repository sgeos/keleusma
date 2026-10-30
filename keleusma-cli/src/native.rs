//! `keleusma native`, which delegates native output to the detached `kel-native` command.
//!
//! The operator ruled on 2026-10-28 that the `keleusma` command delegates native output to
//! `kel-native`, which the V0.3.X line builds in its `native_codegen` package. This command
//! adds no native code generation of its own. It finds `kel-native`, passes every argument
//! through unchanged, and returns its exit status, so `keleusma native build ...` and
//! `kel-native build ...` behave the same.
//!
//! `kel-native` is located in this order.
//!
//! 1. The path in the `KELEUSMA_NATIVE` environment variable, when it is set.
//! 2. A `kel-native` beside the running `keleusma` executable, as an install puts them.
//! 3. The first `kel-native` on `PATH`.
//!
//! **Trust.** Each of the three places is chosen by whoever controls the environment or the
//! install, the same trust a shell gives to a command name. The command runs with the caller's
//! privileges and receives the caller's arguments, and nothing else.
//!
//! # Exit status
//!
//! The delegate's own status. 69, the conventional "service unavailable" code, when no
//! `kel-native` is found, and 70 when it is found but cannot be started or ends on a signal.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The delegate's file name on this platform.
fn delegate_name() -> OsString {
    OsString::from(format!("kel-native{}", env::consts::EXE_SUFFIX))
}

/// Locate `kel-native`, or `None` when it is not installed in any of the three places.
fn locate() -> Option<PathBuf> {
    if let Some(explicit) = env::var_os("KELEUSMA_NATIVE") {
        // An explicit choice is honoured as given, and its absence is reported, not
        // silently replaced by a different binary.
        return Some(PathBuf::from(explicit));
    }
    let name = delegate_name();
    let sibling = env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(&name)));
    if let Some(path) = sibling.filter(|p| p.is_file()) {
        return Some(path);
    }
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(&name))
        .find(|candidate| candidate.is_file())
}

/// Run `keleusma native <args>`.
pub fn run(args: &[String]) -> ExitCode {
    let Some(delegate) = locate() else {
        eprintln!(
            "error: native output needs the `kel-native` command, which was not found beside \
             `keleusma` or on PATH. Install it from the native_codegen package, or set \
             KELEUSMA_NATIVE to its path."
        );
        return ExitCode::from(69);
    };
    run_delegate(&delegate, args)
}

fn run_delegate(delegate: &Path, args: &[String]) -> ExitCode {
    match Command::new(delegate).args(args).status() {
        Ok(status) => match status.code().and_then(|c| u8::try_from(c).ok()) {
            Some(code) => ExitCode::from(code),
            None => {
                eprintln!(
                    "error: `{}` ended without an exit code ({status})",
                    delegate.display()
                );
                ExitCode::from(70)
            }
        },
        Err(e) => {
            eprintln!("error: could not run `{}`: {e}", delegate.display());
            ExitCode::from(70)
        }
    }
}
