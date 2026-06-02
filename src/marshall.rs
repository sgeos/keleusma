//! Static marshalling between Rust types and the runtime [`GenericValue`] enum.
//!
//! This module provides the [`KeleusmaType`] trait for fixed-size, fixed-layout
//! interop types and the [`IntoNativeFn`] and [`IntoFallibleNativeFn`] trait
//! families that allow the host to register Rust functions of arbitrary
//! arity directly with the VM. The [`crate::vm::Vm::register_fn`] and
//! [`crate::vm::Vm::register_fn_fallible`] methods are the user-facing entry
//! points.
//!
//! ## Parametric over (Word, Float)
//!
//! Step 6 of B16 lifted these traits to be generic over the runtime's
//! word and float types. The bundled `Vm` aliases `Value =
//! GenericValue<i64, f64>`, so existing call sites continue to compile
//! unchanged. Hosts targeting narrower runtimes parameterise their
//! `register_fn` calls through a local type alias; see the cookbook
//! recipe for the pattern.
//!
//! The Rust-side type the host writes against does not have to match
//! the script's word width. `impl KeleusmaType<W, F> for i64` truncates
//! through [`Word::from_i64_wrap`] when `W` is narrower; the script
//! sees the truncated value. Hosts that want native-width Rust types
//! can add their own `KeleusmaType<W, F>` impls.
//!
//! See R30 in `docs/decisions/RESOLVED.md` for the design decision and
//! `docs/reference/RELATED_WORK.md` Section 9 for the comparison with
//! Rhai's dynamic marshalling.

extern crate alloc;
use alloc::format;
use alloc::vec::Vec;

use crate::bytecode::GenericValue;
use crate::float::Float;
use crate::vm::VmError;
use crate::word::Word;

/// A type that can cross the host-script boundary.
///
/// Implementations are parametric over the runtime's word type `W`
/// and float type `F`. All implementations have statically known
/// size. Implementations exist for primitives, the unit type,
/// fixed-arity tuples, fixed-length arrays, and `Option<T>`. Host
/// structs and enums become implementations through the
/// `#[derive(KeleusmaType)]` derive macro defined in the
/// `keleusma-macros` crate.
pub trait KeleusmaType<W: Word, F: Float>: Sized {
    /// Convert from a runtime [`GenericValue`] to the Rust type.
    ///
    /// Returns a [`VmError::TypeError`] if the value does not match the
    /// expected shape.
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError>;

    /// Convert from the Rust type into a runtime [`GenericValue`].
    fn into_value(self) -> GenericValue<W, F>;

    /// The flat-composite scalar kind this type occupies when it is a
    /// tuple field, or `None` when it is not a flat-eligible scalar
    /// (B28 P2).
    ///
    /// Used to read an element out of a flat tuple body at the host
    /// boundary, where the value is pure bytes and the Rust type
    /// supplies the layout. The default is `None`, treated as a
    /// non-flat field, so existing external implementations remain
    /// valid without change. `Float` returns `None` because float
    /// fields keep the boxed representation (byte equality would change
    /// `+0.0`/`-0.0` and `NaN` semantics).
    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        None
    }
}

// -- Primitive impls --

impl<W: Word, F: Float> KeleusmaType<W, F> for i64 {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::Int(n) => Ok(W::to_i64(*n)),
            other => Err(VmError::TypeError(format!(
                "expected Word, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        GenericValue::Int(W::from_i64_wrap(self))
    }

    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        Some(crate::value_layout::ScalarKind::Int)
    }
}

impl<W: Word, F: Float> KeleusmaType<W, F> for u8 {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::Byte(b) => Ok(*b),
            other => Err(VmError::TypeError(format!(
                "expected Byte, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        GenericValue::Byte(self)
    }

    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        Some(crate::value_layout::ScalarKind::Byte)
    }
}

#[cfg(feature = "floats")]
impl<W: Word, F: Float> KeleusmaType<W, F> for f64 {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::Float(f) => Ok(F::to_f64(*f)),
            GenericValue::Int(n) => Ok(W::to_i64(*n) as f64),
            other => Err(VmError::TypeError(format!(
                "expected Float, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        GenericValue::Float(F::from_f64(self))
    }
}

impl<W: Word, F: Float> KeleusmaType<W, F> for bool {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::Bool(b) => Ok(*b),
            other => Err(VmError::TypeError(format!(
                "expected bool, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        GenericValue::Bool(self)
    }

    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        Some(crate::value_layout::ScalarKind::Bool)
    }
}

impl<W: Word, F: Float> KeleusmaType<W, F> for () {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::Unit => Ok(()),
            other => Err(VmError::TypeError(format!(
                "expected unit, got {}",
                other.type_name()
            ))),
        }
    }

    fn into_value(self) -> GenericValue<W, F> {
        GenericValue::Unit
    }

    fn flat_field_kind() -> Option<crate::value_layout::ScalarKind> {
        Some(crate::value_layout::ScalarKind::Unit)
    }
}

// -- Option<T> --

impl<W: Word, F: Float, T: KeleusmaType<W, F>> KeleusmaType<W, F> for Option<T> {
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::None => Ok(Option::None),
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

    fn into_value(self) -> GenericValue<W, F> {
        match self {
            Some(t) => t.into_value(),
            Option::None => GenericValue::None,
        }
    }
}

// -- Fixed-length arrays --

impl<W: Word, F: Float, T: KeleusmaType<W, F> + Clone, const N: usize> KeleusmaType<W, F>
    for [T; N]
{
    fn from_value(v: &GenericValue<W, F>) -> Result<Self, VmError> {
        match v {
            GenericValue::Array(items) => {
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

    fn into_value(self) -> GenericValue<W, F> {
        let items: Vec<GenericValue<W, F>> = self.into_iter().map(|t| t.into_value()).collect();
        GenericValue::Array(items)
    }
}

// -- Tuples --

macro_rules! impl_tuple {
    ($($name:ident: $idx:tt),*) => {
        impl<W: Word, FloatT: Float, $($name: KeleusmaType<W, FloatT>),*>
            KeleusmaType<W, FloatT> for ($($name,)*)
        {
            #[allow(clippy::unused_unit, unused_assignments, non_snake_case)]
            fn from_value(v: &GenericValue<W, FloatT>) -> Result<Self, VmError> {
                let expected = [$(stringify!($name),)*].len();
                match v {
                    GenericValue::Tuple(crate::bytecode::TupleBody::Boxed(items)) => {
                        if items.len() != expected {
                            return Err(VmError::TypeError(format!(
                                "expected tuple of arity {}, got {}",
                                expected,
                                items.len()
                            )));
                        }
                        Ok(($($name::from_value(&items[$idx])?,)*))
                    }
                    // A flat tuple body is pure bytes; the Rust element
                    // types supply the per-field kinds so each scalar is
                    // read at its packed offset (B28 P2). Runtime widths
                    // match the module widths on the bundled runtime.
                    GenericValue::Tuple(crate::bytecode::TupleBody::Flat(fc)) => {
                        let word_bytes = (1usize << <W as Word>::BITS_LOG2) / 8;
                        let float_bytes = (1usize << <FloatT as Float>::BITS_LOG2) / 8;
                        let bytes = fc.as_bytes();
                        let kinds = [$(<$name as KeleusmaType<W, FloatT>>::flat_field_kind(),)*];
                        let mut vals: Vec<GenericValue<W, FloatT>> = Vec::with_capacity(expected);
                        let mut offset = 0usize;
                        for k in kinds {
                            let k = k.ok_or_else(|| {
                                VmError::TypeError(format!(
                                    "flat tuple field is not a flat scalar at arity {}",
                                    expected
                                ))
                            })?;
                            vals.push(GenericValue::<W, FloatT>::read_scalar_le(
                                bytes, offset, k, word_bytes, float_bytes,
                            ));
                            offset += k.size_in_bytes(word_bytes, float_bytes);
                        }
                        Ok(($($name::from_value(&vals[$idx])?,)*))
                    }
                    other => Err(VmError::TypeError(format!(
                        "expected tuple, got {}",
                        other.type_name()
                    ))),
                }
            }

            #[allow(non_snake_case)]
            fn into_value(self) -> GenericValue<W, FloatT> {
                let ($($name,)*) = self;
                // Route through the shared constructor so a host-built
                // tuple has the same representation as a script-built one
                // of the same type, which tuple equality relies on.
                GenericValue::tuple_with_widths(
                    ::alloc::vec![$($name.into_value(),)*],
                    (1usize << <W as Word>::BITS_LOG2) / 8,
                    (1usize << <FloatT as Float>::BITS_LOG2) / 8,
                )
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
pub type BoxedNativeFn<W, F> = alloc::boxed::Box<
    dyn for<'a> Fn(
        &crate::vm::NativeCtx<'a>,
        &[GenericValue<W, F>],
    ) -> Result<GenericValue<W, F>, VmError>,
>;

/// A function-like value whose Rust signature can be wrapped as a native
/// function. The tuple `Args` is the argument tuple inferred from the
/// closure or function signature. `R` is the return type.
///
/// Implementations exist for arities 0 through 4 with infallible return
/// types. Use [`IntoFallibleNativeFn`] for functions that return
/// `Result<R, VmError>`.
pub trait IntoNativeFn<W: Word, F: Float, Args, R> {
    /// Wrap `self` as a boxed native function pointer with
    /// argument and return marshalling applied at the boundary.
    fn into_native_fn(self) -> BoxedNativeFn<W, F>;
}

/// A function-like value whose Rust return type is `Result<R, VmError>`.
pub trait IntoFallibleNativeFn<W: Word, F: Float, Args, R> {
    /// Wrap `self` as a boxed native function pointer. `Err` returns
    /// from the wrapped function surface as [`VmError::NativeError`].
    fn into_native_fn(self) -> BoxedNativeFn<W, F>;
}

macro_rules! impl_into_native_fn {
    ($arity:expr; $($name:ident: $idx:tt),*) => {
        impl<W: Word, FloatT: Float, Func, $($name,)* R>
            IntoNativeFn<W, FloatT, ($($name,)*), R> for Func
        where
            Func: Fn($($name,)*) -> R + 'static,
            $($name: KeleusmaType<W, FloatT>,)*
            R: KeleusmaType<W, FloatT>,
        {
            #[allow(unused_variables, clippy::let_unit_value, non_snake_case)]
            fn into_native_fn(self) -> BoxedNativeFn<W, FloatT> {
                alloc::boxed::Box::new(
                    move |_ctx: &crate::vm::NativeCtx<'_>, args: &[GenericValue<W, FloatT>]|
                        -> Result<GenericValue<W, FloatT>, VmError> {
                        if args.len() != $arity {
                            return Err(VmError::NativeError(format!(
                                "native function expected {} argument(s), got {}",
                                $arity,
                                args.len()
                            )));
                        }
                        $(
                            let $name = <$name as KeleusmaType<W, FloatT>>::from_value(&args[$idx])?;
                        )*
                        Ok(self($($name,)*).into_value())
                    },
                )
            }
        }

        impl<W: Word, FloatT: Float, Func, $($name,)* R>
            IntoFallibleNativeFn<W, FloatT, ($($name,)*), R> for Func
        where
            Func: Fn($($name,)*) -> Result<R, VmError> + 'static,
            $($name: KeleusmaType<W, FloatT>,)*
            R: KeleusmaType<W, FloatT>,
        {
            #[allow(unused_variables, clippy::let_unit_value, non_snake_case)]
            fn into_native_fn(self) -> BoxedNativeFn<W, FloatT> {
                alloc::boxed::Box::new(
                    move |_ctx: &crate::vm::NativeCtx<'_>, args: &[GenericValue<W, FloatT>]|
                        -> Result<GenericValue<W, FloatT>, VmError> {
                        if args.len() != $arity {
                            return Err(VmError::NativeError(format!(
                                "native function expected {} argument(s), got {}",
                                $arity,
                                args.len()
                            )));
                        }
                        $(
                            let $name = <$name as KeleusmaType<W, FloatT>>::from_value(&args[$idx])?;
                        )*
                        self($($name,)*).map(<R as KeleusmaType<W, FloatT>>::into_value)
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
#[cfg(all(test, feature = "floats"))]
mod tests {
    use super::*;
    use crate::bytecode::Value;

    #[test]
    fn primitive_roundtrip() {
        assert_eq!(
            <i64 as KeleusmaType<i64, f64>>::from_value(&Value::Int(42)).unwrap(),
            42
        );
        assert_eq!(
            <f64 as KeleusmaType<i64, f64>>::from_value(&Value::Float(2.5)).unwrap(),
            2.5
        );
        assert!(<bool as KeleusmaType<i64, f64>>::from_value(&Value::Bool(true)).unwrap());
        <() as KeleusmaType<i64, f64>>::from_value(&Value::Unit).unwrap();

        assert_eq!(
            <i64 as KeleusmaType<i64, f64>>::into_value(42i64),
            Value::Int(42)
        );
        assert_eq!(
            <f64 as KeleusmaType<i64, f64>>::into_value(2.5f64),
            Value::Float(2.5)
        );
        assert_eq!(
            <bool as KeleusmaType<i64, f64>>::into_value(true),
            Value::Bool(true)
        );
        assert_eq!(<() as KeleusmaType<i64, f64>>::into_value(()), Value::Unit);
    }

    #[test]
    fn i64_to_f64_widening() {
        assert_eq!(
            <f64 as KeleusmaType<i64, f64>>::from_value(&Value::Int(7)).unwrap(),
            7.0
        );
    }

    #[test]
    fn type_mismatch_errors() {
        let err = <i64 as KeleusmaType<i64, f64>>::from_value(&Value::Bool(true)).unwrap_err();
        match err {
            VmError::TypeError(msg) => assert!(msg.contains("expected Word")),
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[test]
    fn option_roundtrip() {
        let some = <Option<i64> as KeleusmaType<i64, f64>>::into_value(Some(42i64));
        assert_eq!(some, Value::Int(42));
        let none = <Option<i64> as KeleusmaType<i64, f64>>::into_value(Option::<i64>::None);
        assert_eq!(none, Value::None);

        let recovered: Option<i64> =
            <Option<i64> as KeleusmaType<i64, f64>>::from_value(&Value::Int(42)).unwrap();
        assert_eq!(recovered, Some(42));
        let recovered_none: Option<i64> =
            <Option<i64> as KeleusmaType<i64, f64>>::from_value(&Value::None).unwrap();
        assert_eq!(recovered_none, Option::None);
    }

    #[test]
    fn tuple_roundtrip() {
        let t = (1i64, 2.0f64, true);
        let v = <(i64, f64, bool) as KeleusmaType<i64, f64>>::into_value(t);
        match &v {
            Value::Tuple(items) => assert_eq!(items.elements().len(), 3),
            other => panic!("expected tuple, got {:?}", other),
        }
        let r: (i64, f64, bool) =
            <(i64, f64, bool) as KeleusmaType<i64, f64>>::from_value(&v).unwrap();
        assert_eq!(r, (1, 2.0, true));
    }

    #[test]
    fn array_roundtrip() {
        let a: [i64; 3] = [10, 20, 30];
        let v = <[i64; 3] as KeleusmaType<i64, f64>>::into_value(a);
        let r: [i64; 3] = <[i64; 3] as KeleusmaType<i64, f64>>::from_value(&v).unwrap();
        assert_eq!(r, [10, 20, 30]);
    }

    #[test]
    fn array_length_mismatch() {
        let v = Value::Array(::alloc::vec![Value::Int(1), Value::Int(2)]);
        let err = <[i64; 3] as KeleusmaType<i64, f64>>::from_value(&v).unwrap_err();
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
        let native = <_ as IntoNativeFn<i64, f64, (), i64>>::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let r = native(&ctx(&arena), &[]).unwrap();
        assert_eq!(r, Value::Int(42));
    }

    #[test]
    fn into_native_fn_arity_one() {
        let f = |x: i64| x * 2;
        let native = <_ as IntoNativeFn<i64, f64, (i64,), i64>>::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let r = native(&ctx(&arena), &[Value::Int(7)]).unwrap();
        assert_eq!(r, Value::Int(14));
    }

    #[test]
    fn into_native_fn_arity_two() {
        let f = |a: i64, b: i64| a + b;
        let native = <_ as IntoNativeFn<i64, f64, (i64, i64), i64>>::into_native_fn(f);
        let arena = keleusma_arena::Arena::with_capacity(64);
        let r = native(&ctx(&arena), &[Value::Int(3), Value::Int(4)]).unwrap();
        assert_eq!(r, Value::Int(7));
    }

    #[test]
    fn into_native_fn_arity_mismatch_errors() {
        let f = |x: i64| x;
        let native = <_ as IntoNativeFn<i64, f64, (i64,), i64>>::into_native_fn(f);
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
        let native = <_ as IntoFallibleNativeFn<i64, f64, (i64,), i64>>::into_native_fn(f);
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
        let err = <i64 as KeleusmaType<i64, f64>>::from_value(&Value::Float(1.5)).unwrap_err();
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("Float"), "got message: {}", msg)
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }
}
