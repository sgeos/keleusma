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

extern crate std;

use std::process::Command;

use crate::address::Address;
use crate::bytecode::GenericValue;
use crate::float::Float;
use crate::vm::{GenericVm, VmError};
use crate::word::Word;

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
