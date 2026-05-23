//! Shell-script utilities for Keleusma scripts.
//!
//! Registers `shell::getenv`, `shell::run`, `shell::run_checked`,
//! and `shell::exit`. The bundle is gated on the `shell` cargo
//! feature because the implementations depend on `std::env` and
//! `std::process`. Enabling the feature adds a `std` dependency
//! to the keleusma library, which is incompatible with the
//! `no_std` build profile.
//!
//! ## Function contracts
//!
//! - `shell::getenv(name: Text) -> Option<Text>` — returns
//!   `Option::Some(value)` when the variable is set and
//!   `Option::None` when it is unset.
//! - `shell::has_env(name: Text) -> bool` — companion to
//!   `getenv` that reports whether the variable is set without
//!   producing an Option-wrapped value.
//! - `shell::run(cmd: Text) -> (Word, Text)` — executes `cmd`
//!   through `sh -c` and returns `(exit_code, stdout)`. A
//!   non-zero exit code is not an error; the caller decides
//!   what to do with it.
//! - `shell::run_checked(cmd: Text) -> Text` — executes `cmd`
//!   through `sh -c` and returns stdout. A non-zero exit code
//!   produces a `VmError::NativeError` with the captured exit
//!   code and stderr in the message.
//! - `shell::exit(code: Word) -> ()` — terminates the host
//!   process with `code` as the exit status. The Keleusma VM
//!   does not return.
//! - `shell::sleep_ms(milliseconds: Word) -> ()` — sleeps the
//!   current thread for the requested duration. Negative or zero
//!   inputs return immediately.
//! - `shell::now_unix_ms() -> Word` — returns the current Unix
//!   timestamp in milliseconds, clamped to the Word range.
//! - `shell::read_file(path: Text) -> Text` — reads the file at
//!   `path` and returns its contents as Text. Traps via
//!   `NativeError` on any I/O failure.
//! - `shell::write_file(path: Text, content: Text) -> ()` —
//!   writes `content` to `path`, replacing any existing file.
//!   Traps via `NativeError` on any I/O failure.
//! - `shell::append_file(path: Text, content: Text) -> ()` —
//!   appends `content` to `path`, creating the file when absent.
//!   Traps via `NativeError` on any I/O failure.
//! - `shell::file_exists(path: Text) -> bool` — returns true when
//!   `path` resolves to an existing filesystem entry. Symlinks
//!   are followed.
//! - `shell::write_err(text: Text) -> ()` — writes `text` to
//!   stderr without a trailing newline.
//! - `shell::writeln_err(text: Text) -> ()` — writes `text` to
//!   stderr with a trailing newline.
//! - `shell::pid() -> Word` — returns the current process
//!   identifier.
//! - `shell::hostname() -> Text` — returns the host name reported
//!   by the operating system. Traps when the host name cannot be
//!   retrieved (no platform fallback).
//! - `shell::arg_count() -> Word` — returns the number of command
//!   line arguments visible to the host process.
//! - `shell::arg(index: Word) -> Option<Text>` — returns the
//!   command line argument at `index`. `Option::None` when out
//!   of range. Argument zero is the host executable path.
//! - `shell::setenv(name: Text, value: Text) -> ()` — sets an
//!   environment variable in the host process for subsequent
//!   subprocesses spawned through `shell::run`. The change is
//!   process local.
//! - `shell::pwd() -> Text` — returns the current working
//!   directory. Traps when the directory cannot be read.
//! - `shell::cd(path: Text) -> ()` — changes the current working
//!   directory. Traps on failure.
//! - `shell::run_timeout(cmd: Text, ms: Word) -> (Word, Text)` —
//!   executes `cmd` through `sh -c` with a wall-clock deadline.
//!   Returns `(exit_code, stdout)` on completion; traps on timeout
//!   after killing the subprocess.

extern crate std;

use std::io::Write;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::address::Address;
use crate::bytecode::GenericValue;
use crate::float::Float;
use crate::vm::{GenericVm, VmError};
use crate::word::Word;

/// Register the shell-bundle natives (`shell::getenv`,
/// `shell::has_env`, `shell::run`, `shell::run_checked`,
/// `shell::exit`) on `vm`. Called by
/// [`crate::stddsl::Shell::register`](super::Shell).
pub fn register<'a, 'arena, W: Word, A: Address, F: Float>(
    vm: &mut GenericVm<'a, 'arena, W, A, F>,
) {
    // The KeleusmaType marshalling family does not currently
    // support `String` arguments or tuple return types, so the
    // shell natives use the lower-level `register_native` entry
    // point and pattern-match on `GenericValue` directly.
    vm.register_native("shell::getenv", getenv_native::<W, F>);
    vm.register_native("shell::has_env", has_env_native::<W, F>);
    vm.register_native("shell::run", run_native::<W, F>);
    vm.register_native("shell::run_checked", run_checked_native::<W, F>);
    vm.register_native("shell::exit", exit_native::<W, F>);
    vm.register_native("shell::sleep_ms", sleep_ms_native::<W, F>);
    vm.register_native("shell::now_unix_ms", now_unix_ms_native::<W, F>);
    vm.register_native("shell::read_file", read_file_native::<W, F>);
    vm.register_native("shell::write_file", write_file_native::<W, F>);
    vm.register_native("shell::append_file", append_file_native::<W, F>);
    vm.register_native("shell::file_exists", file_exists_native::<W, F>);
    vm.register_native("shell::write_err", write_err_native::<W, F>);
    vm.register_native("shell::writeln_err", writeln_err_native::<W, F>);
    vm.register_native("shell::pid", pid_native::<W, F>);
    vm.register_native("shell::hostname", hostname_native::<W, F>);
    vm.register_native("shell::arg_count", arg_count_native::<W, F>);
    vm.register_native("shell::arg", arg_native::<W, F>);
    vm.register_native("shell::setenv", setenv_native::<W, F>);
    vm.register_native("shell::pwd", pwd_native::<W, F>);
    vm.register_native("shell::cd", cd_native::<W, F>);
    vm.register_native("shell::run_timeout", run_timeout_native::<W, F>);
}

fn has_env_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::has_env: expected exactly one argument",
        )));
    }
    let name: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::has_env: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    Ok(GenericValue::Bool(std::env::var(name).is_ok()))
}

fn exit_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::exit: expected exactly one argument",
        )));
    }
    let code = match args[0] {
        GenericValue::Int(n) => W::to_i64(n),
        ref v => {
            return Err(VmError::TypeError(std::format!(
                "shell::exit: expected Word, got {}",
                v.type_name()
            )));
        }
    };
    std::process::exit(code as i32);
}

fn getenv_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::getenv: expected exactly one argument",
        )));
    }
    let name: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::getenv: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    match std::env::var(name) {
        Ok(value) => Ok(GenericValue::Enum {
            type_name: std::string::String::from("Option"),
            variant: std::string::String::from("Some"),
            fields: std::vec![GenericValue::StaticStr(value)],
        }),
        Err(std::env::VarError::NotPresent) => Ok(GenericValue::None),
        Err(std::env::VarError::NotUnicode(_)) => Err(VmError::NativeError(std::format!(
            "shell::getenv: {} is not valid Unicode",
            name
        ))),
    }
}

fn run_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::run: expected exactly one argument",
        )));
    }
    let cmd: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::run: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| VmError::NativeError(std::format!("shell::run: failed to spawn sh: {}", e)))?;
    let exit_code = output.status.code().unwrap_or(-1) as i64;
    let stdout = std::string::String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(GenericValue::Tuple(std::vec![
        GenericValue::Int(W::from_i64_wrap(exit_code)),
        GenericValue::StaticStr(stdout),
    ]))
}

fn run_checked_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::run_checked: expected exactly one argument",
        )));
    }
    let cmd: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::run_checked: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .map_err(|e| {
            VmError::NativeError(std::format!(
                "shell::run_checked: failed to spawn sh: {}",
                e
            ))
        })?;
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = std::string::String::from_utf8_lossy(&output.stderr);
        return Err(VmError::NativeError(std::format!(
            "shell::run_checked: command exited with code {}: {}",
            code,
            stderr
        )));
    }
    let stdout = std::string::String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(GenericValue::StaticStr(stdout))
}

fn sleep_ms_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::sleep_ms: expected exactly one argument",
        )));
    }
    let ms = match args[0] {
        GenericValue::Int(n) => W::to_i64(n),
        ref v => {
            return Err(VmError::TypeError(std::format!(
                "shell::sleep_ms: expected Word, got {}",
                v.type_name()
            )));
        }
    };
    if ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    Ok(GenericValue::Unit)
}

fn now_unix_ms_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if !args.is_empty() {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::now_unix_ms: expected zero arguments",
        )));
    }
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| {
        VmError::NativeError(std::format!(
            "shell::now_unix_ms: system clock is before the Unix epoch: {}",
            e
        ))
    })?;
    let ms = dur.as_millis();
    let clamped = if ms > i64::MAX as u128 {
        i64::MAX
    } else {
        ms as i64
    };
    Ok(GenericValue::Int(W::from_i64_wrap(clamped)))
}

fn read_file_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::read_file: expected exactly one argument",
        )));
    }
    let path: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::read_file: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    let bytes = std::fs::read(path)
        .map_err(|e| VmError::NativeError(std::format!("shell::read_file: {}: {}", path, e)))?;
    let text = std::string::String::from_utf8(bytes).map_err(|e| {
        VmError::NativeError(std::format!(
            "shell::read_file: {}: invalid UTF-8: {}",
            path,
            e
        ))
    })?;
    Ok(GenericValue::StaticStr(text))
}

fn write_file_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::write_file: expected exactly two arguments",
        )));
    }
    let path: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::write_file: expected Text for path, got {}",
            args[0].type_name()
        ))
    })?;
    let content: &str = args[1].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::write_file: expected Text for content, got {}",
            args[1].type_name()
        ))
    })?;
    std::fs::write(path, content)
        .map_err(|e| VmError::NativeError(std::format!("shell::write_file: {}: {}", path, e)))?;
    Ok(GenericValue::Unit)
}

fn append_file_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::append_file: expected exactly two arguments",
        )));
    }
    let path: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::append_file: expected Text for path, got {}",
            args[0].type_name()
        ))
    })?;
    let content: &str = args[1].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::append_file: expected Text for content, got {}",
            args[1].type_name()
        ))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| {
            VmError::NativeError(std::format!("shell::append_file: open {}: {}", path, e))
        })?;
    file.write_all(content.as_bytes()).map_err(|e| {
        VmError::NativeError(std::format!("shell::append_file: write {}: {}", path, e))
    })?;
    Ok(GenericValue::Unit)
}

fn file_exists_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::file_exists: expected exactly one argument",
        )));
    }
    let path: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::file_exists: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    Ok(GenericValue::Bool(std::path::Path::new(path).exists()))
}

fn write_err_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::write_err: expected exactly one argument",
        )));
    }
    let text: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::write_err: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    handle
        .write_all(text.as_bytes())
        .map_err(|e| VmError::NativeError(std::format!("shell::write_err: {}", e)))?;
    Ok(GenericValue::Unit)
}

fn writeln_err_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::writeln_err: expected exactly one argument",
        )));
    }
    let text: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::writeln_err: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    writeln!(handle, "{}", text)
        .map_err(|e| VmError::NativeError(std::format!("shell::writeln_err: {}", e)))?;
    Ok(GenericValue::Unit)
}

fn pid_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if !args.is_empty() {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::pid: expected zero arguments",
        )));
    }
    Ok(GenericValue::Int(W::from_i64_wrap(
        std::process::id() as i64
    )))
}

fn hostname_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if !args.is_empty() {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::hostname: expected zero arguments",
        )));
    }
    // The Rust standard library does not expose a portable
    // hostname accessor. Use the platform-conventional `hostname`
    // command. Traps when the command is unavailable.
    let output = Command::new("hostname").output().map_err(|e| {
        VmError::NativeError(std::format!(
            "shell::hostname: failed to spawn hostname: {}",
            e
        ))
    })?;
    if !output.status.success() {
        return Err(VmError::NativeError(std::format!(
            "shell::hostname: hostname command exited with code {}",
            output.status.code().unwrap_or(-1)
        )));
    }
    let stdout = std::string::String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim_end_matches(['\n', '\r']);
    Ok(GenericValue::StaticStr(std::string::String::from(trimmed)))
}

fn arg_count_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if !args.is_empty() {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::arg_count: expected zero arguments",
        )));
    }
    let n = std::env::args().count() as i64;
    Ok(GenericValue::Int(W::from_i64_wrap(n)))
}

fn arg_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::arg: expected exactly one argument",
        )));
    }
    let index = match args[0] {
        GenericValue::Int(n) => W::to_i64(n),
        ref v => {
            return Err(VmError::TypeError(std::format!(
                "shell::arg: expected Word, got {}",
                v.type_name()
            )));
        }
    };
    if index < 0 {
        return Ok(GenericValue::None);
    }
    let mut iter = std::env::args();
    let value = iter.nth(index as usize);
    match value {
        Some(v) => Ok(GenericValue::Enum {
            type_name: std::string::String::from("Option"),
            variant: std::string::String::from("Some"),
            fields: std::vec![GenericValue::StaticStr(v)],
        }),
        None => Ok(GenericValue::None),
    }
}

fn setenv_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::setenv: expected exactly two arguments",
        )));
    }
    let name: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::setenv: expected Text for name, got {}",
            args[0].type_name()
        ))
    })?;
    let value: &str = args[1].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::setenv: expected Text for value, got {}",
            args[1].type_name()
        ))
    })?;
    // SAFETY: set_var is marked unsafe in the 2024 edition
    // because concurrent modification of the process environment
    // is unsound on some platforms. The Keleusma VM is
    // single-threaded; native invocations cannot race with one
    // another or with the host. The script-driven environment
    // change is intentional and operator-trusted (strict signing
    // gates the bytecode source).
    unsafe {
        std::env::set_var(name, value);
    }
    Ok(GenericValue::Unit)
}

fn pwd_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if !args.is_empty() {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::pwd: expected zero arguments",
        )));
    }
    let cwd = std::env::current_dir()
        .map_err(|e| VmError::NativeError(std::format!("shell::pwd: {}", e)))?;
    let s = cwd.to_string_lossy().into_owned();
    Ok(GenericValue::StaticStr(s))
}

fn cd_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::cd: expected exactly one argument",
        )));
    }
    let path: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::cd: expected Text, got {}",
            args[0].type_name()
        ))
    })?;
    std::env::set_current_dir(path)
        .map_err(|e| VmError::NativeError(std::format!("shell::cd: {}: {}", path, e)))?;
    Ok(GenericValue::Unit)
}

fn run_timeout_native<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    if args.len() != 2 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::run_timeout: expected exactly two arguments",
        )));
    }
    let cmd: &str = args[0].as_str().ok_or_else(|| {
        VmError::TypeError(std::format!(
            "shell::run_timeout: expected Text for cmd, got {}",
            args[0].type_name()
        ))
    })?;
    let ms = match args[1] {
        GenericValue::Int(n) => W::to_i64(n),
        ref v => {
            return Err(VmError::TypeError(std::format!(
                "shell::run_timeout: expected Word for ms, got {}",
                v.type_name()
            )));
        }
    };
    if ms <= 0 {
        return Err(VmError::NativeError(std::format!(
            "shell::run_timeout: timeout must be positive, got {}",
            ms
        )));
    }
    let timeout = Duration::from_millis(ms as u64);
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            VmError::NativeError(std::format!(
                "shell::run_timeout: failed to spawn sh: {}",
                e
            ))
        })?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|e| {
                    VmError::NativeError(std::format!(
                        "shell::run_timeout: failed to collect output: {}",
                        e
                    ))
                })?;
                let exit_code = status.code().unwrap_or(-1) as i64;
                let stdout = std::string::String::from_utf8_lossy(&output.stdout).into_owned();
                return Ok(GenericValue::Tuple(std::vec![
                    GenericValue::Int(W::from_i64_wrap(exit_code)),
                    GenericValue::StaticStr(stdout),
                ]));
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(VmError::NativeError(std::format!(
                        "shell::run_timeout: command exceeded timeout of {}ms",
                        ms
                    )));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                return Err(VmError::NativeError(std::format!(
                    "shell::run_timeout: wait failed: {}",
                    e
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    type V = GenericValue<i64, f64>;

    #[test]
    fn sleep_ms_zero_returns_immediately() {
        let result = sleep_ms_native::<i64, f64>(&[V::Int(0)]).expect("sleep_ms(0)");
        assert!(matches!(result, V::Unit));
    }

    #[test]
    fn sleep_ms_negative_returns_immediately() {
        let result = sleep_ms_native::<i64, f64>(&[V::Int(-5)]).expect("sleep_ms(-5)");
        assert!(matches!(result, V::Unit));
    }

    #[test]
    fn sleep_ms_rejects_wrong_arity() {
        let err = sleep_ms_native::<i64, f64>(&[]).expect_err("arity");
        match err {
            VmError::NativeError(m) => assert!(m.contains("expected exactly one")),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn sleep_ms_rejects_non_word() {
        let err = sleep_ms_native::<i64, f64>(&[V::Bool(true)]).expect_err("type");
        assert!(matches!(err, VmError::TypeError(_)));
    }

    #[test]
    fn now_unix_ms_positive() {
        let result = now_unix_ms_native::<i64, f64>(&[]).expect("now_unix_ms()");
        match result {
            V::Int(n) => assert!(n > 1_700_000_000_000),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn now_unix_ms_rejects_args() {
        let err = now_unix_ms_native::<i64, f64>(&[V::Int(0)]).expect_err("args");
        assert!(matches!(err, VmError::NativeError(_)));
    }

    #[test]
    fn file_exists_true_and_false() {
        let mut tmp_path = std::env::temp_dir();
        tmp_path.push("keleusma_shell_test_exists.txt");
        let path_str = tmp_path.to_string_lossy().to_string();
        let _ = std::fs::remove_file(&tmp_path);
        let none =
            file_exists_native::<i64, f64>(&[V::StaticStr(path_str.clone())]).expect("file_exists");
        assert!(matches!(none, V::Bool(false)));
        std::fs::write(&tmp_path, b"").expect("write tmp");
        let some =
            file_exists_native::<i64, f64>(&[V::StaticStr(path_str.clone())]).expect("file_exists");
        assert!(matches!(some, V::Bool(true)));
        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn write_read_append_roundtrip() {
        let mut tmp_path = std::env::temp_dir();
        tmp_path.push("keleusma_shell_test_io.txt");
        let path_str = tmp_path.to_string_lossy().to_string();
        let _ = std::fs::remove_file(&tmp_path);

        write_file_native::<i64, f64>(&[
            V::StaticStr(path_str.clone()),
            V::StaticStr(std::string::String::from("first\n")),
        ])
        .expect("write");

        append_file_native::<i64, f64>(&[
            V::StaticStr(path_str.clone()),
            V::StaticStr(std::string::String::from("second\n")),
        ])
        .expect("append");

        let result = read_file_native::<i64, f64>(&[V::StaticStr(path_str.clone())]).expect("read");
        match result {
            V::StaticStr(s) => assert_eq!(s, "first\nsecond\n"),
            other => panic!("wrong variant: {:?}", other),
        }
        let _ = std::fs::remove_file(&tmp_path);
    }

    #[test]
    fn read_file_traps_on_missing() {
        let err = read_file_native::<i64, f64>(&[V::StaticStr(std::string::String::from(
            "/nonexistent/keleusma_shell_test_missing.txt",
        ))])
        .expect_err("missing");
        match err {
            VmError::NativeError(m) => assert!(m.contains("/nonexistent")),
            other => panic!("wrong variant: {:?}", other),
        }
    }

    #[test]
    fn write_file_rejects_arity() {
        let err = write_file_native::<i64, f64>(&[V::StaticStr(std::string::String::from("x"))])
            .expect_err("arity");
        match err {
            VmError::NativeError(m) => assert!(m.contains("two")),
            other => panic!("wrong variant: {:?}", other),
        }
    }
}
