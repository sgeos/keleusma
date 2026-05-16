//! Host-supplied opaque types referenced from Keleusma scripts.
//!
//! Opaque types are the surface for Rust values that Keleusma
//! scripts hold by reference without introspecting their internal
//! structure. The script-side surface is a named primitive type
//! (e.g. `MyHandle`) declared in function signatures; the runtime
//! representation is `Value::Opaque(Arc<dyn HostOpaque>)`. Native
//! functions registered by the host produce and consume opaque
//! values.
//!
//! ## Design choice: trait object over type parameter
//!
//! The dyn-free alternative is to parameterise `Value`, `Vm`,
//! `NativeCtx`, and the native-function-trait surface by a host
//! opaque type `O`. That approach is type-safe by construction but
//! propagates a type parameter through every public signature and
//! every implementation block in `bytecode.rs`, `vm.rs`,
//! `marshall.rs`, and every native registration call. The present
//! design uses a custom marker trait, [`HostOpaque`], behind an
//! `Arc<dyn HostOpaque>` reference. The trait is small: a name
//! method for diagnostics, a sealed-supertrait `TypeId` lookup,
//! and the standard `Send + Sync + 'static` bounds. Hosts implement
//! [`HostOpaque`] for any Rust type they wish to expose. Native
//! functions extract a typed reference via [`dyn HostOpaque::downcast_ref`]
//! which checks the dynamic type through a `TypeId` comparison
//! without invoking `core::any::Any`.
//!
//! ## Cross-yield discipline
//!
//! Opaque values are host-managed through `Arc` and have a
//! lifetime independent of the arena. They may appear in the
//! dialogue type at a yield, may flow into the data segment, and
//! survive arena resets and hot code swaps. The cross-yield
//! prohibition on [`crate::bytecode::Value::KStr`] does not apply
//! because the storage is not arena-resident.
//!
//! ## WCMU contribution
//!
//! Opaque values contribute zero to the script-side WCMU bound
//! because the allocation is host-managed. Hosts that want their
//! own opaque heap bounded supply a per-native attestation through
//! [`crate::vm::Vm::set_native_bounds`].

extern crate alloc;
use alloc::sync::Arc;
use core::any::TypeId;

/// Implementation-detail supertrait that surfaces the host's
/// concrete `TypeId` through the trait object's vtable. A blanket
/// implementation covers every `'static` type, so hosts never
/// implement this trait directly.
///
/// The trait lives in a private module so external callers cannot
/// observe its method beyond the surface that [`HostOpaque`]
/// exposes through inherent methods on `dyn HostOpaque`.
mod sealed {
    use core::any::TypeId;

    pub trait HostOpaqueTypeId {
        fn host_type_id(&self) -> TypeId;
    }

    impl<T: 'static> HostOpaqueTypeId for T {
        fn host_type_id(&self) -> TypeId {
            TypeId::of::<T>()
        }
    }
}

/// Marker trait for host-managed opaque types referenced from
/// Keleusma scripts.
///
/// The host implements this trait for every Rust type it wishes to
/// expose to scripts as an opaque value. The trait carries only
/// the metadata Keleusma needs at the runtime boundary; the host's
/// type remains structurally opaque to the script.
///
/// ## Required methods
///
/// - [`type_name`](HostOpaque::type_name) returns the script-side
///   type name. The type checker matches this name against the
///   `Type::Opaque(name)` declared in function signatures.
///
/// ## Implicit methods
///
/// The sealed supertrait [`sealed::HostOpaqueTypeId`] supplies the
/// host's concrete `TypeId` through a blanket implementation. Hosts
/// do not implement it directly; any `'static + Send + Sync` type
/// that implements [`HostOpaque`] automatically participates.
///
/// ## Thread-safety bounds
///
/// `HostOpaque` requires `Send + Sync + 'static`. The `Send + Sync`
/// pair makes `Arc<dyn HostOpaque>` safe to share across threads;
/// hosts running in a single-threaded environment satisfy this
/// trivially. The `'static` bound rules out borrowed-data opaque
/// types because their lifetime is not statically bounded against
/// the script's reference lifetime through the VM.
pub trait HostOpaque: sealed::HostOpaqueTypeId + Send + Sync + 'static {
    /// The script-side name of this opaque type. Used by the type
    /// checker to match `Type::Opaque(name)` declarations against
    /// runtime values, and by `Value::type_name` in diagnostic
    /// messages.
    fn type_name(&self) -> &'static str;
}

impl dyn HostOpaque {
    /// Concrete `TypeId` for the underlying host type, fetched
    /// through dynamic dispatch on the sealed supertrait.
    pub fn dyn_type_id(&self) -> TypeId {
        sealed::HostOpaqueTypeId::host_type_id(self)
    }

    /// Attempt to borrow the underlying value as a concrete `T`.
    ///
    /// Returns `Some(&T)` when the dynamic `TypeId` of the
    /// underlying value matches `TypeId::of::<T>()`. Returns
    /// `None` otherwise. Implemented through a `TypeId` comparison
    /// rather than `core::any::Any::downcast_ref`, so the host
    /// trait surface does not depend on `Any`.
    pub fn downcast_ref<T: HostOpaque>(&self) -> Option<&T> {
        if self.dyn_type_id() == TypeId::of::<T>() {
            // SAFETY: the TypeId check above confirms the underlying
            // pointer points to a `T`. The trait object's data
            // pointer points to the concrete value; the cast
            // recovers a typed reference with the same lifetime as
            // `&self`.
            let ptr = self as *const dyn HostOpaque as *const T;
            Some(unsafe { &*ptr })
        } else {
            None
        }
    }
}

impl core::fmt::Debug for dyn HostOpaque {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "<opaque {}>", self.type_name())
    }
}

/// Convenience wrapper for `Arc::new` that returns an opaque-typed
/// `Arc<dyn HostOpaque>` ready to drop into a [`crate::bytecode::Value::Opaque`].
///
/// Useful at native-function return sites where the host
/// constructs a new opaque value:
///
/// ```ignore
/// use keleusma::{Value, host_arc, HostOpaque};
///
/// struct MyState { /* ... */ }
/// impl HostOpaque for MyState {
///     fn type_name(&self) -> &'static str { "MyState" }
/// }
///
/// vm.register_fn("make_state", || -> Value {
///     Value::Opaque(host_arc(MyState { /* ... */ }))
/// });
/// ```
pub fn host_arc<T: HostOpaque>(value: T) -> Arc<dyn HostOpaque> {
    Arc::new(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Foo(i64);
    impl HostOpaque for Foo {
        fn type_name(&self) -> &'static str {
            "Foo"
        }
    }

    struct Bar(#[allow(dead_code)] &'static str);
    impl HostOpaque for Bar {
        fn type_name(&self) -> &'static str {
            "Bar"
        }
    }

    #[test]
    fn type_name_dispatches_through_trait_object() {
        let foo: Arc<dyn HostOpaque> = Arc::new(Foo(42));
        let bar: Arc<dyn HostOpaque> = Arc::new(Bar("hello"));
        assert_eq!(foo.type_name(), "Foo");
        assert_eq!(bar.type_name(), "Bar");
    }

    #[test]
    fn downcast_ref_returns_typed_reference_on_match() {
        let foo: Arc<dyn HostOpaque> = Arc::new(Foo(42));
        let typed = foo.as_ref().downcast_ref::<Foo>().expect("downcast Foo");
        assert_eq!(typed.0, 42);
    }

    #[test]
    fn downcast_ref_returns_none_on_mismatch() {
        let foo: Arc<dyn HostOpaque> = Arc::new(Foo(42));
        assert!(foo.as_ref().downcast_ref::<Bar>().is_none());
    }

    #[test]
    fn dyn_type_id_matches_concrete_type_id() {
        let foo: Arc<dyn HostOpaque> = Arc::new(Foo(42));
        assert_eq!(foo.as_ref().dyn_type_id(), TypeId::of::<Foo>());
    }

    #[test]
    fn host_arc_helper_returns_opaque_trait_object() {
        let arc = host_arc(Foo(7));
        assert_eq!(arc.type_name(), "Foo");
        assert_eq!(arc.as_ref().downcast_ref::<Foo>().unwrap().0, 7);
    }

    #[test]
    fn debug_renders_type_name() {
        use alloc::format;
        let foo: Arc<dyn HostOpaque> = Arc::new(Foo(42));
        let s = format!("{:?}", foo.as_ref());
        assert_eq!(s, "<opaque Foo>");
    }
}
