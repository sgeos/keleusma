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
//! - `shell::getenv(name: Text) -> Text` — returns the value of
//!   the environment variable, or the empty string when unset.
//!   The Option-typed shape selected during design discussion
//!   would have required the compiler's pattern matcher to
//!   understand the runtime convention that `Some(v)` is `v`
//!   directly. The pre-existing pattern-match code emits an
//!   `IsEnum` check that fails against unwrapped values; this
//!   limitation is tracked separately. The shell-idiomatic
//!   empty-string convention is the practical alternative.
//! - `shell::has_env(name: Text) -> bool` — companion to
//!   `getenv` that reports whether the variable is set,
//!   distinguishing an unset variable from one set to the empty
//!   string.
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

extern crate std;

use std::process::Command;

use crate::bytecode::Value;
use crate::vm::{Vm, VmError};

pub fn register<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
    // The KeleusmaType marshalling family does not currently
    // support `String` arguments or tuple return types, so the
    // shell natives use the lower-level `register_native` entry
    // point and pattern-match on `Value` directly.
    vm.register_native("shell::getenv", getenv_native);
    vm.register_native("shell::has_env", has_env_native);
    vm.register_native("shell::run", run_native);
    vm.register_native("shell::run_checked", run_checked_native);
    vm.register_native("shell::exit", exit_native);
}

fn has_env_native(args: &[Value]) -> Result<Value, VmError> {
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
    Ok(Value::Bool(std::env::var(name).is_ok()))
}

fn exit_native(args: &[Value]) -> Result<Value, VmError> {
    if args.len() != 1 {
        return Err(VmError::NativeError(std::string::String::from(
            "shell::exit: expected exactly one argument",
        )));
    }
    let code = match args[0] {
        Value::Int(n) => n,
        ref v => {
            return Err(VmError::TypeError(std::format!(
                "shell::exit: expected Word, got {}",
                v.type_name()
            )));
        }
    };
    std::process::exit(code as i32);
}

fn getenv_native(args: &[Value]) -> Result<Value, VmError> {
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
        Ok(value) => Ok(Value::StaticStr(value)),
        Err(std::env::VarError::NotPresent) => Ok(Value::StaticStr(std::string::String::new())),
        Err(std::env::VarError::NotUnicode(_)) => Err(VmError::NativeError(std::format!(
            "shell::getenv: {} is not valid Unicode",
            name
        ))),
    }
}

fn run_native(args: &[Value]) -> Result<Value, VmError> {
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
    Ok(Value::Tuple(std::vec![
        Value::Int(exit_code),
        Value::StaticStr(stdout),
    ]))
}

fn run_checked_native(args: &[Value]) -> Result<Value, VmError> {
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
    Ok(Value::StaticStr(stdout))
}
