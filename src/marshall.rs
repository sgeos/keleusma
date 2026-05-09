//! Static marshalling between Rust types and the runtime `Value` enum.
//!
//! This module provides the `KeleusmaType` trait for fixed-size, fixed-layout
//! interop types and the `IntoNativeFn` and `IntoFallibleNativeFn` trait
//! families that allow the host to register Rust functions of arbitrary
//! arity directly with the VM. The `Vm::register_fn` and
//! `Vm::register_fn_fallible` methods are the user-facing entry points.
//!
//! See R30 in `docs/decisions/RESOLVED.md` for the design decision and
//! `docs/reference/RELATED_WORK.md` Section 9 for the comparison with
//! Rhai's dynamic marshalling.

extern crate alloc;
use alloc::format;
use alloc::vec::Vec;

use crate::bytecode::Value;
use crate::vm::VmError;

/// A type that can cross the host-script boundary.
///
/// All implementations have statically known size. Implementations exist
/// for primitives, the unit type, fixed-arity tuples, fixed-length arrays,
/// and `Option<T>`. Host structs and enums become implementations through
/// the `#[derive(KeleusmaType)]` derive macro defined in the
/// `keleusma-macros` crate.
pub trait KeleusmaType: Sized {
    /// Convert from a runtime `Value` to the Rust type.
    ///
    /// Returns a `VmError::TypeError` if the value does not match the
    /// expected shape.
    fn from_value(v: &Value) -> Result<Self, VmError>;

    /// Convert from the Rust type into a runtime `Value`.
    fn into_value(self) -> Value;
}

// -- Primitive impls --

impl KeleusmaType for i64 {
    fn from_value(v: &Value) -> Result<Self, VmError> {
        match v {
            Value::Int(n) => Ok(*n),
            other => Err(VmError::TypeError(format!(
                "expected i64, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> Value {
        Value::Int(self)
    }
}

impl KeleusmaType for f64 {
    fn from_value(v: &Value) -> Result<Self, VmError> {
        match v {
            Value::Float(f) => Ok(*f),
            Value::Int(n) => Ok(*n as f64),
            other => Err(VmError::TypeError(format!(
                "expected f64, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> Value {
        Value::Float(self)
    }
}

impl KeleusmaType for bool {
    fn from_value(v: &Value) -> Result<Self, VmError> {
        match v {
            Value::Bool(b) => Ok(*b),
            other => Err(VmError::TypeError(format!(
                "expected bool, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> Value {
        Value::Bool(self)
    }
}

impl KeleusmaType for () {
    fn from_value(v: &Value) -> Result<Self, VmError> {
        match v {
            Value::Unit => Ok(()),
            other => Err(VmError::TypeError(format!(
                "expected unit, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> Value {
        Value::Unit
    }
}

// -- Option<T> --

impl<T: KeleusmaType> KeleusmaType for Option<T> {
    fn from_value(v: &Value) -> Result<Self, VmError> {
        match v {
            Value::None => Ok(Option::None),
            other => {
                // The runtime represents Option::Some(x) as the inner value
                // wrapped in a single-field struct named "Some" by the compiler
                // when constructed via WrapSome. Here we accept either a
                // direct inner value or the Some-wrapped form depending on
                // how the host produced it. In practice, the compiler emits
                // WrapSome which yields a Value variant that does not exist
                // separately. The convention is that any non-None value
                // is treated as Some. This matches the existing VM behavior.
                T::from_value(other).map(Some)
            }
        }
    }

    fn into_value(self) -> Value {
        match self {
            Some(t) => t.into_value(),
            Option::None => Value::None,
        }
    }
}

// -- Fixed-length arrays --

impl<T: KeleusmaType + Clone, const N: usize> KeleusmaType for [T; N] {
    fn from_value(v: &Value) -> Result<Self, VmError> {
        match v {
            Value::Array(items) => {
                if items.len() != N {
                    return Err(VmError::TypeError(format!(
                        "expected array of length {}, got {}",
                        N,
                        items.len()
                    )));
                }
                let mut converted: Vec<T> = Vec::with_capacity(N);
                for item in items.iter() {
                    converted.push(T::from_value(item)?);
                }
                converted.try_into().map_err(|_| {
                    VmError::TypeError(format!("failed to convert array of length {}", N))
                })
            }
            other => Err(VmError::TypeError(format!(
                "expected array, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> Value {
        let items: Vec<Value> = self.into_iter().map(|t| t.into_value()).collect();
        Value::Array(items)
    }
}

// -- Tuples --

macro_rules! impl_tuple {
    ($($name:ident: $idx:tt),*) => {
        impl<$($name: KeleusmaType),*> KeleusmaType for ($($name,)*) {
            #[allow(clippy::unused_unit, unused_assignments, non_snake_case)]
            fn from_value(v: &Value) -> Result<Self, VmError> {
                match v {
                    Value::Tuple(items) => {
                        let expected = [$(stringify!($name),)*].len();
                        if items.len() != expected {
                            return Err(VmError::TypeError(format!(
                                "expected tuple of arity {}, got {}",
                                expected,
                                items.len()
                            )));
                        }
                        Ok(($($name::from_value(&items[$idx])?,)*))
                    }
                    other => Err(VmError::TypeError(format!(
                        "expected tuple, got {}",
                        other.type_name()
                    ))),
                }
            }

            #[allow(non_snake_case)]
            fn into_value(self) -> Value {
                let ($($name,)*) = self;
                Value::Tuple(::alloc::vec![$($name.into_value(),)*])
            }
        }
    };
}

impl_tuple!(A: 0, B: 1);
impl_tuple!(A: 0, B: 1, C: 2);
impl_tuple!(A: 0, B: 1, C: 2, D: 3);
impl_tuple!(A: 0, B: 1, C: 2, D: 3, E: 4);

// -- IntoNativeFn family --

/// The boxed call convention used by the VM for native functions.
///
/// All native functions internally accept a [`crate::vm::NativeCtx`]
/// to support arena-aware natives. Marshalled functions registered
/// through this trait family ignore the context.
type BoxedNativeFn = alloc::boxed::Box<
    dyn for<'a> Fn(&crate::vm::NativeCtx<'a>, &[Value]) -> Result<Value, VmError>,
>;

/// A function-like value whose Rust signature can be wrapped as a native
/// function. The tuple `Args` is the argument tuple inferred from the
/// closure or function signature. `R` is the return type.
///
/// Implementations exist for arities 0 through 4 with infallible return
/// types. Use `IntoFallibleNativeFn` for functions that return
/// `Result<R, VmError>`.
pub trait IntoNativeFn<Args, R> {
    fn into_native_fn(self) -> BoxedNativeFn;
}

/// A function-like value whose Rust return type is `Result<R, VmError>`.
pub trait IntoFallibleNativeFn<Args, R> {
    fn into_native_fn(self) -> BoxedNativeFn;
}

macro_rules! impl_into_native_fn {
    ($arity:expr; $($name:ident: $idx:tt),*) => {
        impl<F, $($name,)* R> IntoNativeFn<($($name,)*), R> for F
        where
            F: Fn($($name,)*) -> R + 'static,
            $($name: KeleusmaType,)*
            R: KeleusmaType,
        {
            #[allow(unused_variables, clippy::let_unit_value, non_snake_case)]
            fn into_native_fn(self) -> BoxedNativeFn {
                alloc::boxed::Box::new(
                    move |_ctx: &crate::vm::NativeCtx<'_>, args: &[Value]| -> Result<Value, VmError> {
                        if args.len() != $arity {
                            return Err(VmError::NativeError(format!(
                                "native function expected {} argument(s), got {}",
                                $arity,
                                args.len()
                            )));
                        }
                        $(
                            let $name = $name::from_value(&args[$idx])?;
                        )*
                        Ok(self($($name,)*).into_value())
                    },
                )
            }
        }

        impl<F, $($name,)* R> IntoFallibleNativeFn<($($name,)*), R> for F
        where
            F: Fn($($name,)*) -> Result<R, VmError> + 'static,
            $($name: KeleusmaType,)*
            R: KeleusmaType,
        {
            #[allow(unused_variables, clippy::let_unit_value, non_snake_case)]
            fn into_native_fn(self) -> BoxedNativeFn {
                alloc::boxed::Box::new(
                    move |_ctx: &crate::vm::NativeCtx<'_>, args: &[Value]| -> Result<Value, VmError> {
                        if args.len() != $arity {
                            return Err(VmError::NativeError(format!(
                                "native function expected {} argument(s), got {}",
                                $arity,
                                args.len()
                            )));
                        }
                        $(
                            let $name = $name::from_value(&args[$idx])?;
                        )*
                        self($($name,)*).map(KeleusmaType::into_value)
                    },
                )
            }
        }
    };
}

impl_into_native_fn!(0;);
impl_into_native_fn!(1; A: 0);
impl_into_native_fn!(2; A: 0, B: 1);
impl_into_native_fn!(3; A: 0, B: 1, C: 2);
impl_into_native_fn!(4; A: 0, B: 1, C: 2, D: 3);

// Tests live alongside the trait. Integration tests across vm.rs cover
// register_fn registration end to end.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitive_roundtrip() {
        assert_eq!(i64::from_value(&Value::Int(42)).unwrap(), 42);
        assert_eq!(f64::from_value(&Value::Float(2.5)).unwrap(), 2.5);
        assert!(bool::from_value(&Value::Bool(true)).unwrap());
        <()>::from_value(&Value::Unit).unwrap();

        assert_eq!(42i64.into_value(), Value::Int(42));
        assert_eq!(2.5f64.into_value(), Value::Float(2.5));
        assert_eq!(true.into_value(), Value::Bool(true));
        assert_eq!(().into_value(), Value::Unit);
    }

    #[test]
    fn i64_to_f64_widening() {
        assert_eq!(f64::from_value(&Value::Int(7)).unwrap(), 7.0);
    }

    #[test]
    fn type_mismatch_errors() {
        let err = i64::from_value(&Value::Bool(true)).unwrap_err();
        match err {
            VmError::TypeError(msg) => assert!(msg.contains("expected i64")),
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[test]
    fn option_roundtrip() {
        let some = Some(42i64).into_value();
        assert_eq!(some, Value::Int(42));
        let none = Option::<i64>::None.into_value();
        assert_eq!(none, Value::None);

        let recovered: Option<i64> = Option::<i64>::from_value(&Value::Int(42)).unwrap();
        assert_eq!(recovered, Some(42));
        let recovered_none: Option<i64> = Option::<i64>::from_value(&Value::None).unwrap();
        assert_eq!(recovered_none, Option::None);
    }

    #[test]
    fn tuple_roundtrip() {
        let t = (1i64, 2.0f64, true);
        let v = t.into_value();
        match &v {
            Value::Tuple(items) => assert_eq!(items.len(), 3),
            other => panic!("expected tuple, got {:?}", other),
        }
        let r: (i64, f64, bool) = <(i64, f64, bool)>::from_value(&v).unwrap();
        assert_eq!(r, (1, 2.0, true));
    }

    #[test]
    fn array_roundtrip() {
        let a: [i64; 3] = [10, 20, 30];
        let v = a.into_value();
        let r: [i64; 3] = <[i64; 3]>::from_value(&v).unwrap();
        assert_eq!(r, [10, 20, 30]);
    }

    #[test]
    fn array_length_mismatch() {
        let v = Value::Array(::alloc::vec![Value::Int(1), Value::Int(2)]);
        let err = <[i64; 3]>::from_value(&v).unwrap_err();
        match err {
            VmError::TypeError(msg) => assert!(msg.contains("length")),
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    fn ctx(arena: &keleusma_arena::Arena) -> crate::vm::NativeCtx<'_> {
        crate::vm::NativeCtx { arena }
    }

    #[test]
    fn into_native_fn_arity_zero() {
        let f = || 42i64;
        let native = IntoNativeFn::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let r = native(&ctx(&arena), &[]).unwrap();
        assert_eq!(r, Value::Int(42));
    }

    #[test]
    fn into_native_fn_arity_one() {
        let f = |x: i64| x * 2;
        let native = IntoNativeFn::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let r = native(&ctx(&arena), &[Value::Int(7)]).unwrap();
        assert_eq!(r, Value::Int(14));
    }

    #[test]
    fn into_native_fn_arity_two() {
        let f = |a: i64, b: i64| a + b;
        let native = IntoNativeFn::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let r = native(&ctx(&arena), &[Value::Int(3), Value::Int(4)]).unwrap();
        assert_eq!(r, Value::Int(7));
    }

    #[test]
    fn into_native_fn_arity_mismatch_errors() {
        let f = |x: i64| x;
        let native = IntoNativeFn::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let err = native(&ctx(&arena), &[Value::Int(1), Value::Int(2)]).unwrap_err();
        match err {
            VmError::NativeError(msg) => assert!(msg.contains("expected 1 argument")),
            other => panic!("expected NativeError, got {:?}", other),
        }
    }

    #[test]
    fn into_fallible_native_fn_propagates_error() {
        let f = |x: i64| -> Result<i64, VmError> {
            if x == 0 {
                Err(VmError::DivisionByZero)
            } else {
                Ok(100 / x)
            }
        };
        let native = IntoFallibleNativeFn::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let r = native(&ctx(&arena), &[Value::Int(5)]).unwrap();
        assert_eq!(r, Value::Int(20));
        let err = native(&ctx(&arena), &[Value::Int(0)]).unwrap_err();
        match err {
            VmError::DivisionByZero => {}
            other => panic!("expected DivisionByZero, got {:?}", other),
        }
    }

    #[test]
    fn type_error_message_contains_typename() {
        let err = i64::from_value(&Value::Float(1.5)).unwrap_err();
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("Float"), "got message: {}", msg)
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }
}
