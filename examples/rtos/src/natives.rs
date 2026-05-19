//! Native function surface that tasks call.
//!
//! Each native is a thin wrapper around a [`Platform`] method.
//! Tasks `use host::xxx` and the matching native is registered
//! against the task's VM at construction. The natives are
//! zero-overhead because the platform is a generic parameter
//! and the dispatch monomorphises down to a direct function
//! call.
//!
//! Validation. Natives that take a resource index validate it
//! against [`crate::platform::PlatformResources`] before
//! forwarding the call to the platform. An out-of-range index
//! returns `Status::Err(StatusErrorCode::Invalid…)` without
//! touching the underlying hardware. The platform-side methods
//! may therefore assume the index is in range.
//!
//! Return discipline. Natives that can fail validation return a
//! `Status` enum value. The script-side enum is declared in
//! `scripts/prelude.kel`. Read natives that need to convey both
//! a status and a data word return a `(Status, Word)` tuple.
//! Natives that cannot fail return their natural type (a Word
//! for introspection queries; Unit for log, where script-side
//! discard is the expected use).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmError};

use crate::platform::Platform;

/// Discriminants for the script-side `StatusErrorCode` enum.
/// Kept in lock-step with `scripts/prelude.kel`. The values
/// here are what the host stuffs into the `Status::Err(Word)`
/// payload; the script casts the inner word back to a
/// `StatusErrorCode` for human-readable reporting.
#[derive(Clone, Copy, Debug)]
#[repr(i64)]
pub enum StatusErrorCode {
    InvalidPin = 1,
    InvalidChannel = 2,
    InvalidController = 3,
    InvalidAddress = 4,
    NotSupported = 5,
    Busy = 6,
    Timeout = 7,
    BadArgument = 8,
}

/// Build a `Status::Ok` value. The empty `fields` vector
/// matches the prelude declaration `Ok = 0` (no payload).
fn status_ok() -> Value {
    Value::Enum {
        type_name: String::from("Status"),
        variant: String::from("Ok"),
        fields: Vec::new(),
    }
}

/// Build a `Status::Err(code)` value. The payload is the
/// `StatusErrorCode` discriminant as a Word.
fn status_err(code: StatusErrorCode) -> Value {
    Value::Enum {
        type_name: String::from("Status"),
        variant: String::from("Err"),
        fields: vec![Value::Int(code as i64)],
    }
}

/// Register every native the kernel's tasks expect. Call once
/// per task VM at construction time.
pub fn register_task_natives<P: Platform>(vm: &mut Vm) {
    // Time, log, and existing GPIO / sensor surface.
    register_clock_now::<P>(vm);
    register_log::<P>(vm);
    register_gpio_set::<P>(vm);
    register_sensor_read::<P>(vm);

    // Resource introspection.
    register_resource_counts::<P>(vm);

    // DSL bus natives. Validated against PlatformResources.
    register_usart::<P>(vm);
    register_spi::<P>(vm);
    register_i2c::<P>(vm);
    register_adc::<P>(vm);
}

fn register_clock_now<P: Platform>(vm: &mut Vm) {
    vm.register_native_closure(
        "host::clock_now",
        Box::new(|args: &[Value]| -> Result<Value, VmError> {
            check_arity("clock_now", 0, args)?;
            Ok(Value::Int(P::now_ms() as i64))
        }),
    );
}

fn register_log<P: Platform>(vm: &mut Vm) {
    vm.register_native_closure(
        "host::log",
        Box::new(|args: &[Value]| -> Result<Value, VmError> {
            check_arity("log", 1, args)?;
            let line = as_str(&args[0])?;
            P::log(line);
            Ok(Value::Unit)
        }),
    );
}

/// `host::gpio_set(pin: Word, high: Word) -> Status`.
/// Rejects `pin >= RESOURCES.gpio_pin_count` with
/// `Status::Err(InvalidPin)`.
fn register_gpio_set<P: Platform>(vm: &mut Vm) {
    vm.register_native_closure(
        "host::gpio_set",
        Box::new(|args: &[Value]| -> Result<Value, VmError> {
            check_arity("gpio_set", 2, args)?;
            let pin_word = as_i64(&args[0])?;
            let high = as_i64(&args[1])? != 0;
            if !(0..P::RESOURCES.gpio_pin_count as i64).contains(&pin_word) {
                return Ok(status_err(StatusErrorCode::InvalidPin));
            }
            P::gpio_set(pin_word as u8, high);
            Ok(status_ok())
        }),
    );
}

/// `host::sensor_read(channel: Word) -> Word`. Backwards-
/// compatible legacy native used by the demonstrator's sensor
/// task. Out-of-range indices return 0 (the demonstrator does
/// not check). New scripts should prefer `host::adc_read`
/// which returns `(Status, Word)`.
fn register_sensor_read<P: Platform>(vm: &mut Vm) {
    vm.register_native_closure(
        "host::sensor_read",
        Box::new(|args: &[Value]| -> Result<Value, VmError> {
            check_arity("sensor_read", 1, args)?;
            let ch = as_i64(&args[0])?;
            if !(0..P::RESOURCES.sensor_channel_count as i64).contains(&ch) {
                return Ok(Value::Int(0));
            }
            Ok(Value::Int(P::sensor_read(ch as u8) as i64))
        }),
    );
}

/// Resource-introspection natives. Each returns a `Word` equal
/// to the matching `PlatformResources` field. Cannot fail.
fn register_resource_counts<P: Platform>(vm: &mut Vm) {
    vm.register_native_closure(
        "host::gpio_pin_count",
        Box::new(|args: &[Value]| {
            check_arity("gpio_pin_count", 0, args)?;
            Ok(Value::Int(P::RESOURCES.gpio_pin_count as i64))
        }),
    );
    vm.register_native_closure(
        "host::sensor_channel_count",
        Box::new(|args: &[Value]| {
            check_arity("sensor_channel_count", 0, args)?;
            Ok(Value::Int(P::RESOURCES.sensor_channel_count as i64))
        }),
    );
    vm.register_native_closure(
        "host::uart_count",
        Box::new(|args: &[Value]| {
            check_arity("uart_count", 0, args)?;
            Ok(Value::Int(P::RESOURCES.uart_count as i64))
        }),
    );
    vm.register_native_closure(
        "host::spi_count",
        Box::new(|args: &[Value]| {
            check_arity("spi_count", 0, args)?;
            Ok(Value::Int(P::RESOURCES.spi_count as i64))
        }),
    );
    vm.register_native_closure(
        "host::i2c_count",
        Box::new(|args: &[Value]| {
            check_arity("i2c_count", 0, args)?;
            Ok(Value::Int(P::RESOURCES.i2c_count as i64))
        }),
    );
    vm.register_native_closure(
        "host::timer_count",
        Box::new(|args: &[Value]| {
            check_arity("timer_count", 0, args)?;
            Ok(Value::Int(P::RESOURCES.timer_count as i64))
        }),
    );
}

fn register_usart<P: Platform>(vm: &mut Vm) {
    // `host::usart_write(controller: Word, byte: Word) -> Status`
    vm.register_native_closure(
        "host::usart_write",
        Box::new(|args: &[Value]| {
            check_arity("usart_write", 2, args)?;
            let ctrl = as_i64(&args[0])?;
            let byte = as_i64(&args[1])?;
            if !(0..P::RESOURCES.uart_count as i64).contains(&ctrl) {
                return Ok(status_err(StatusErrorCode::InvalidController));
            }
            if !(0..=0xFF).contains(&byte) {
                return Ok(status_err(StatusErrorCode::BadArgument));
            }
            P::usart_write(ctrl as u8, byte as u8);
            Ok(status_ok())
        }),
    );
    // `host::usart_read(controller: Word) -> (Status, Word)`
    vm.register_native_closure(
        "host::usart_read",
        Box::new(|args: &[Value]| {
            check_arity("usart_read", 1, args)?;
            let ctrl = as_i64(&args[0])?;
            if !(0..P::RESOURCES.uart_count as i64).contains(&ctrl) {
                return Ok(Value::Tuple(vec![
                    status_err(StatusErrorCode::InvalidController),
                    Value::Int(0),
                ]));
            }
            let byte = P::usart_read(ctrl as u8);
            Ok(Value::Tuple(vec![status_ok(), Value::Int(byte as i64)]))
        }),
    );
}

fn register_spi<P: Platform>(vm: &mut Vm) {
    // `host::spi_write(controller: Word, byte: Word) -> Status`
    vm.register_native_closure(
        "host::spi_write",
        Box::new(|args: &[Value]| {
            check_arity("spi_write", 2, args)?;
            let ctrl = as_i64(&args[0])?;
            let byte = as_i64(&args[1])?;
            if !(0..P::RESOURCES.spi_count as i64).contains(&ctrl) {
                return Ok(status_err(StatusErrorCode::InvalidController));
            }
            if !(0..=0xFF).contains(&byte) {
                return Ok(status_err(StatusErrorCode::BadArgument));
            }
            P::spi_write(ctrl as u8, byte as u8);
            Ok(status_ok())
        }),
    );
    // `host::spi_read(controller: Word) -> (Status, Word)`
    vm.register_native_closure(
        "host::spi_read",
        Box::new(|args: &[Value]| {
            check_arity("spi_read", 1, args)?;
            let ctrl = as_i64(&args[0])?;
            if !(0..P::RESOURCES.spi_count as i64).contains(&ctrl) {
                return Ok(Value::Tuple(vec![
                    status_err(StatusErrorCode::InvalidController),
                    Value::Int(0),
                ]));
            }
            let byte = P::spi_read(ctrl as u8);
            Ok(Value::Tuple(vec![status_ok(), Value::Int(byte as i64)]))
        }),
    );
}

fn register_i2c<P: Platform>(vm: &mut Vm) {
    // `host::i2c_write(controller: Word, addr: Word, byte: Word) -> Status`
    vm.register_native_closure(
        "host::i2c_write",
        Box::new(|args: &[Value]| {
            check_arity("i2c_write", 3, args)?;
            let ctrl = as_i64(&args[0])?;
            let addr = as_i64(&args[1])?;
            let byte = as_i64(&args[2])?;
            if !(0..P::RESOURCES.i2c_count as i64).contains(&ctrl) {
                return Ok(status_err(StatusErrorCode::InvalidController));
            }
            // Seven-bit address space; the eighth bit is the
            // R/W flag and the host owns it.
            if !(0..=0x7F).contains(&addr) {
                return Ok(status_err(StatusErrorCode::InvalidAddress));
            }
            if !(0..=0xFF).contains(&byte) {
                return Ok(status_err(StatusErrorCode::BadArgument));
            }
            P::i2c_write(ctrl as u8, addr as u8, byte as u8);
            Ok(status_ok())
        }),
    );
    // `host::i2c_read(controller: Word, addr: Word) -> (Status, Word)`
    vm.register_native_closure(
        "host::i2c_read",
        Box::new(|args: &[Value]| {
            check_arity("i2c_read", 2, args)?;
            let ctrl = as_i64(&args[0])?;
            let addr = as_i64(&args[1])?;
            if !(0..P::RESOURCES.i2c_count as i64).contains(&ctrl) {
                return Ok(Value::Tuple(vec![
                    status_err(StatusErrorCode::InvalidController),
                    Value::Int(0),
                ]));
            }
            if !(0..=0x7F).contains(&addr) {
                return Ok(Value::Tuple(vec![
                    status_err(StatusErrorCode::InvalidAddress),
                    Value::Int(0),
                ]));
            }
            let byte = P::i2c_read(ctrl as u8, addr as u8);
            Ok(Value::Tuple(vec![status_ok(), Value::Int(byte as i64)]))
        }),
    );
}

fn register_adc<P: Platform>(vm: &mut Vm) {
    // `host::adc_read(channel: Word) -> (Status, Word)`. The
    // Word in the success tuple is the raw ADC reading.
    vm.register_native_closure(
        "host::adc_read",
        Box::new(|args: &[Value]| {
            check_arity("adc_read", 1, args)?;
            let ch = as_i64(&args[0])?;
            if !(0..P::RESOURCES.sensor_channel_count as i64).contains(&ch) {
                return Ok(Value::Tuple(vec![
                    status_err(StatusErrorCode::InvalidChannel),
                    Value::Int(0),
                ]));
            }
            let val = P::adc_read(ch as u8);
            Ok(Value::Tuple(vec![status_ok(), Value::Int(val as i64)]))
        }),
    );
}

fn as_i64(v: &Value) -> Result<i64, VmError> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(VmError::TypeError(format!(
            "expected Word, got {}",
            other.type_name()
        ))),
    }
}

fn as_str(v: &Value) -> Result<&str, VmError> {
    match v {
        Value::StaticStr(s) => Ok(s.as_str()),
        // `Value::KStr` would need an arena context to deref;
        // the demonstrator only uses static strings so the
        // KStr case is left as a `TypeError` for now.
        other => Err(VmError::TypeError(format!(
            "expected static Text, got {}",
            other.type_name()
        ))),
    }
}

fn check_arity(name: &str, expected: usize, args: &[Value]) -> Result<(), VmError> {
    if args.len() != expected {
        return Err(VmError::NativeError(format!(
            "host::{}: expected {} argument(s), got {}",
            name,
            expected,
            args.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_ok_construction() {
        let s = status_ok();
        match s {
            Value::Enum {
                type_name,
                variant,
                fields,
            } => {
                assert_eq!(type_name, "Status");
                assert_eq!(variant, "Ok");
                assert!(fields.is_empty());
            }
            other => panic!("expected Value::Enum, got {:?}", other),
        }
    }

    #[test]
    fn status_err_construction_carries_code() {
        for (code, expected_disc) in [
            (StatusErrorCode::InvalidPin, 1),
            (StatusErrorCode::InvalidChannel, 2),
            (StatusErrorCode::InvalidController, 3),
            (StatusErrorCode::InvalidAddress, 4),
            (StatusErrorCode::NotSupported, 5),
            (StatusErrorCode::Busy, 6),
            (StatusErrorCode::Timeout, 7),
            (StatusErrorCode::BadArgument, 8),
        ] {
            let s = status_err(code);
            match s {
                Value::Enum {
                    type_name,
                    variant,
                    fields,
                } => {
                    assert_eq!(type_name, "Status");
                    assert_eq!(variant, "Err");
                    assert_eq!(fields.len(), 1);
                    match &fields[0] {
                        Value::Int(n) => assert_eq!(
                            *n, expected_disc,
                            "discriminant for {:?} should be {}",
                            code, expected_disc
                        ),
                        other => panic!("expected payload Value::Int, got {:?}", other),
                    }
                }
                other => panic!("expected Value::Enum, got {:?}", other),
            }
        }
    }
}
